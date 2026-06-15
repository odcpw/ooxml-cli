package mutate

import (
	"errors"
	"fmt"
	"regexp"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// ErrReplacementCountMismatch is returned when the actual number of replacements
// performed does not match the caller-supplied expected count.
var ErrReplacementCountMismatch = errors.New("replacement count mismatch")

// textSegment tracks a single w:t element and the byte range its text occupies in
// the paragraph's concatenated string.
type textSegment struct {
	elem  *etree.Element
	start int
	end   int
}

type replaceTarget struct {
	blockIndex     int
	blockKind      model.BlockKind
	tableIndex     int
	rowIndex       int
	columnIndex    int
	paragraphIndex int
	element        *etree.Element
}

// FindReplaceRequest describes a document-wide find/replace over the main body.
type FindReplaceRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	// Pattern is the compiled matcher. Build it with BuildFindReplacePattern so
	// literal/regex/match-case/whole-word modes are handled consistently.
	Pattern *regexp.Regexp
	// Replace is inserted literally for each match (no regexp group expansion).
	Replace string
	// ExpectCount, when set, guards the total number of replacements. A nil
	// pointer disables the guard; a non-nil pointer (including 0) enforces it.
	ExpectCount *int
}

// FindReplaceResult summarizes the outcome of a find/replace pass.
type FindReplaceResult struct {
	TotalReplacements    int                       `json:"totalReplacements"`
	AffectedBlockCount   int                       `json:"affectedBlockCount"`
	AffectedBlockIndices []int                     `json:"affectedBlockIndices"`
	BlockSummaries       []FindReplaceBlockSummary `json:"blockSummaries"`
}

// FindReplaceBlockSummary captures the before/after state of one changed paragraph.
type FindReplaceBlockSummary struct {
	Index               int    `json:"index"`
	Kind                string `json:"kind"`
	Style               string `json:"style,omitempty"`
	TableIndex          int    `json:"tableIndex,omitempty"`
	RowIndex            int    `json:"rowIndex,omitempty"`
	ColumnIndex         int    `json:"columnIndex,omitempty"`
	ParagraphIndex      int    `json:"paragraphIndex,omitempty"`
	ContentHash         string `json:"contentHash"`
	PreviousHash        string `json:"previousHash"`
	ReplacementsInBlock int    `json:"replacementsInBlock"`
	PreviousText        string `json:"previousText"`
	Text                string `json:"text"`
}

// BuildFindReplacePattern compiles a matcher for the requested find string and
// matching options. Literal patterns are quoted; regex patterns are used as-is.
func BuildFindReplacePattern(find string, regex, matchCase, wholeWord bool) (*regexp.Regexp, error) {
	if find == "" {
		return nil, fmt.Errorf("find pattern is required")
	}
	expr := find
	if !regex {
		expr = regexp.QuoteMeta(find)
	}
	if wholeWord {
		expr = `\b(?:` + expr + `)\b`
	}
	if !matchCase {
		expr = "(?i)" + expr
	}
	compiled, err := regexp.Compile(expr)
	if err != nil {
		return nil, fmt.Errorf("invalid find pattern: %w", err)
	}
	return compiled, nil
}

// FindReplaceInDocument performs a document-wide find/replace across w:t runs in
// top-level body paragraphs and paragraphs inside table cells, handling text
// split across multiple runs.
func FindReplaceInDocument(req *FindReplaceRequest) (*FindReplaceResult, error) {
	if req == nil {
		return nil, fmt.Errorf("find replace request is nil")
	}
	if req.Pattern == nil {
		return nil, fmt.Errorf("find replace pattern is required")
	}
	doc, bodyElem, err := locateBodyForReplace(req.Package, req.DocumentURI)
	if err != nil {
		return nil, err
	}

	result := &FindReplaceResult{
		AffectedBlockIndices: make([]int, 0),
		BlockSummaries:       make([]FindReplaceBlockSummary, 0),
	}
	affectedBlocks := make(map[int]bool)

	for _, target := range replaceTargets(bodyElem) {
		previousText := docxbody.ParagraphText(target.element)
		style := docxbody.ParagraphStyle(target.element)
		count := replaceInParagraph(target.element, req.Pattern, req.Replace)
		if count == 0 {
			continue
		}
		newText := docxbody.ParagraphText(target.element)
		result.TotalReplacements += count
		if !affectedBlocks[target.blockIndex] {
			affectedBlocks[target.blockIndex] = true
			result.AffectedBlockIndices = append(result.AffectedBlockIndices, target.blockIndex)
		}
		result.BlockSummaries = append(result.BlockSummaries, FindReplaceBlockSummary{
			Index:               target.blockIndex,
			Kind:                replaceTargetKind(target),
			Style:               style,
			TableIndex:          target.tableIndex,
			RowIndex:            target.rowIndex,
			ColumnIndex:         target.columnIndex,
			ParagraphIndex:      target.paragraphIndex,
			ContentHash:         replaceTargetHash(target, style, newText),
			PreviousHash:        replaceTargetHash(target, style, previousText),
			ReplacementsInBlock: count,
			PreviousText:        previousText,
			Text:                newText,
		})
	}
	result.AffectedBlockCount = len(result.AffectedBlockIndices)

	if req.ExpectCount != nil && *req.ExpectCount != result.TotalReplacements {
		return nil, fmt.Errorf("%w: expected %d replacements, found %d", ErrReplacementCountMismatch, *req.ExpectCount, result.TotalReplacements)
	}

	ensureDocumentTableScaffolds(doc.Root(), doc.Root().Space)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return result, nil
}

func replaceTargets(bodyElem *etree.Element) []replaceTarget {
	var targets []replaceTarget
	tableIndex := 0
	for _, block := range docxbody.Blocks(bodyElem) {
		switch block.Kind {
		case model.BlockKindParagraph:
			targets = append(targets, replaceTarget{
				blockIndex: block.Index,
				blockKind:  block.Kind,
				element:    block.Element,
			})
		case model.BlockKindTable:
			tableIndex++
			targets = append(targets, tableReplaceTargets(block, tableIndex)...)
		}
	}
	return targets
}

func tableReplaceTargets(block docxbody.BodyBlock, tableIndex int) []replaceTarget {
	var targets []replaceTarget
	for rowIdx, row := range childElementsByLocalName(block.Element, "tr") {
		for colIdx, cell := range childElementsByLocalName(row, "tc") {
			targets = append(targets, cellParagraphTargets(cell, replaceTarget{
				blockIndex:  block.Index,
				blockKind:   block.Kind,
				tableIndex:  tableIndex,
				rowIndex:    rowIdx + 1,
				columnIndex: colIdx + 1,
			})...)
		}
	}
	return targets
}

func cellParagraphTargets(cell *etree.Element, base replaceTarget) []replaceTarget {
	var targets []replaceTarget
	paragraphIndex := 0
	var walk func(elem *etree.Element)
	walk = func(elem *etree.Element) {
		for _, child := range elem.ChildElements() {
			if docxbody.LocalName(child.Tag) == "p" {
				paragraphIndex++
				target := base
				target.paragraphIndex = paragraphIndex
				target.element = child
				targets = append(targets, target)
				continue
			}
			walk(child)
		}
	}
	walk(cell)
	return targets
}

func replaceTargetKind(target replaceTarget) string {
	if target.blockKind == model.BlockKindTable {
		return "tableCell"
	}
	return string(target.blockKind)
}

func replaceTargetHash(target replaceTarget, style, text string) string {
	if target.blockKind == model.BlockKindTable {
		return extract.BlockContentHash(model.BlockKindTable, "", fmt.Sprintf("%d:%d:%d:%s", target.rowIndex, target.columnIndex, target.paragraphIndex, text))
	}
	return extract.BlockContentHash(model.BlockKindParagraph, style, text)
}

// replaceInParagraph concatenates the text of all w:t descendants in document
// order, matches the pattern over the joined string, and splices replacement
// text back into the originating w:t elements. Untouched w:t elements keep their
// exact text (and therefore their run formatting). It returns the match count.
func replaceInParagraph(paragraph *etree.Element, pattern *regexp.Regexp, replace string) int {
	textNodes := namespaces.FindDescendants(paragraph, namespaces.NsW, "t")
	if len(textNodes) == 0 {
		return 0
	}

	var (
		segments []textSegment
		full     []byte
	)
	for _, t := range textNodes {
		txt := t.Text()
		start := len(full)
		full = append(full, txt...)
		segments = append(segments, textSegment{elem: t, start: start, end: len(full)})
	}

	matches := pattern.FindAllIndex(full, -1)
	if len(matches) == 0 {
		return 0
	}

	// Walk the joined string left-to-right. Unmatched byte ranges are emitted to
	// the segments that own them; each match's replacement text is emitted wholly
	// into the segment containing the match start. Matched bytes are dropped.
	out := make([][]byte, len(segments))
	emit := func(a, b int) {
		for i := range segments {
			lo := max(a, segments[i].start)
			hi := min(b, segments[i].end)
			if lo < hi {
				out[i] = append(out[i], full[lo:hi]...)
			}
		}
	}

	applied, cursor := 0, 0
	for _, mr := range matches {
		matchStart, matchEnd := mr[0], mr[1]
		if matchStart == matchEnd {
			// Defensive: skip zero-length matches to avoid an emit loop.
			continue
		}
		emit(cursor, matchStart)
		startSeg := segmentIndexAt(segments, matchStart)
		out[startSeg] = append(out[startSeg], replace...)
		cursor = matchEnd
		applied++
	}
	emit(cursor, len(full))
	if applied == 0 {
		return 0
	}

	for i, seg := range segments {
		value := string(out[i])
		seg.elem.SetText(value)
		applySpacePreserve(seg.elem, value)
	}
	return applied
}

// segmentIndexAt returns the index of the segment whose half-open byte range
// [start,end) contains the given offset, or -1 if none.
func segmentIndexAt(segments []textSegment, offset int) int {
	for i, seg := range segments {
		if offset >= seg.start && offset < seg.end {
			return i
		}
	}
	return -1
}

func applySpacePreserve(t *etree.Element, value string) {
	attr := t.SelectAttr("xml:space")
	if needsSpacePreserve(value) {
		if attr == nil {
			t.CreateAttr("xml:space", "preserve")
		}
		return
	}
	if attr != nil {
		t.RemoveAttr("xml:space")
	}
}

func locateBodyForReplace(session opc.PackageSession, documentURI string) (*etree.Document, *etree.Element, error) {
	if session == nil {
		return nil, nil, fmt.Errorf("package session is nil")
	}
	if documentURI == "" {
		return nil, nil, fmt.Errorf("document URI is required")
	}
	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read document part %s: %w", documentURI, err)
	}
	bodyElem, err := docxbody.FindBody(doc.Root())
	if err != nil {
		return nil, nil, err
	}
	return doc, bodyElem, nil
}
