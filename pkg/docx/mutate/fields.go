package mutate

import (
	"errors"
	"fmt"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	// ErrFieldNotFound is returned when a selector resolves to no field.
	ErrFieldNotFound = errors.New("field not found")
	// ErrFieldParaOutOfRange is returned when a target paragraph index does not exist.
	ErrFieldParaOutOfRange = errors.New("field target paragraph out of range")
	// ErrInvalidFieldCode is returned when a field code is empty.
	ErrInvalidFieldCode = errors.New("invalid field code")
	// ErrFieldInTable is returned when a selector targets a body table block. Fields
	// nested inside tables are listed by `docx fields list` (Editable=false) but are
	// not addressable by the part:block:field selector grammar.
	ErrFieldInTable = errors.New("field target is a table; table-nested fields are not addressable by the part:block:field selector")
)

// knownFieldCodes is the set of instruction codes this slice fully supports. Other
// codes are accepted (some are user-defined) but flagged as a warning.
var knownFieldCodes = map[string]bool{
	"PAGE":     true,
	"NUMPAGES": true,
	"DATE":     true,
	"TIME":     true,
	"FILENAME": true,
	"AUTHOR":   true,
	"SUBJECT":  true,
	"TITLE":    true,
}

// FieldCodeBase returns the leading instruction keyword of a field code, upper-cased
// (e.g. "PAGE" from " PAGE \\* MERGEFORMAT ").
func FieldCodeBase(code string) string {
	fields := strings.Fields(code)
	if len(fields) == 0 {
		return ""
	}
	return strings.ToUpper(fields[0])
}

// IsKnownFieldCode reports whether the leading keyword of code is a recognized
// instruction this tool understands.
func IsKnownFieldCode(code string) bool {
	return knownFieldCodes[FieldCodeBase(code)]
}

// InsertFieldRequest inserts a new simple field into a target paragraph.
type InsertFieldRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	PartURI     string // target part; defaults to DocumentURI when empty
	BlockIndex  int    // 1-based body block (for the document part) or paragraph index (header/footer)
	FieldCode   string // instruction, e.g. "PAGE"
	ResultText  string // initial cached result text (optional)
}

// InsertFieldResult reports the inserted field.
type InsertFieldResult struct {
	PartURI       string `json:"partUri"`
	BlockIndex    int    `json:"blockIndex"`
	FieldIndex    int    `json:"fieldIndex"`
	FieldType     string `json:"fieldType"`
	Instruction   string `json:"instruction"`
	CachedResult  string `json:"cachedResult"`
	Location      string `json:"location"`
	ParagraphText string `json:"paragraphText"`
	KnownCode     bool   `json:"knownCode"`
}

// SetFieldResultRequest updates a field's cached result text by selector.
type SetFieldResultRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	PartURI      string // target part; defaults to DocumentURI when empty
	BlockIndex   int    // 1-based block/paragraph index within the part
	FieldIndex   int    // 0-based index among fields in that block/paragraph
	Result       string
	ExpectedHash string // optional guard against instruction+result content
}

// SetFieldResultResult reports the updated field.
type SetFieldResultResult struct {
	PartURI        string `json:"partUri"`
	BlockIndex     int    `json:"blockIndex"`
	FieldIndex     int    `json:"fieldIndex"`
	FieldType      string `json:"fieldType"`
	Instruction    string `json:"instruction"`
	PreviousResult string `json:"previousResult"`
	CachedResult   string `json:"cachedResult"`
	Location       string `json:"location"`
}

// FieldContentHash hashes the semantic identity of a field (instruction + result)
// for stale-guarding.
func FieldContentHash(instruction, result string) string {
	return docxinspect.CommentContentHash(instruction, "", result)
}

// InsertField appends a w:fldSimple field to the target paragraph. The target part
// is the main document (block index addresses body blocks) or a header/footer part
// (block index addresses paragraphs).
func InsertField(req *InsertFieldRequest) (*InsertFieldResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert field request is nil")
	}
	if strings.TrimSpace(req.FieldCode) == "" {
		return nil, ErrInvalidFieldCode
	}

	partURI := req.PartURI
	if partURI == "" {
		partURI = req.DocumentURI
	}

	doc, err := req.Package.ReadXMLPart(partURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read part %s: %w", partURI, err)
	}
	root := doc.Root()
	prefix := root.Space

	paragraph, blockIndex, err := locateTargetParagraph(root, partURI, req.DocumentURI, req.BlockIndex)
	if err != nil {
		return nil, err
	}

	instr := normalizeInstruction(req.FieldCode)
	fld := newElement(prefix, "fldSimple")
	fld.CreateAttr(qualifiedWordAttrName(root, prefix, "instr"), instr)
	run := newElement(prefix, "r")
	if req.ResultText != "" {
		appendTextChildren(run, prefix, req.ResultText)
	} else {
		// An empty result run keeps the field well-formed and gives Word a slot.
		run.AddChild(newElement(prefix, "t"))
	}
	fld.AddChild(run)
	paragraph.AddChild(fld)

	if err := req.Package.ReplaceXMLPart(partURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace part %s: %w", partURI, err)
	}

	// Compute the new field's index among this paragraph's fields.
	fieldIndex := paragraphFieldCount(paragraph) - 1
	if fieldIndex < 0 {
		fieldIndex = 0
	}

	return &InsertFieldResult{
		PartURI:       partURI,
		BlockIndex:    blockIndex,
		FieldIndex:    fieldIndex,
		FieldType:     docxinspect.FieldTypeSimple,
		Instruction:   strings.TrimSpace(instr),
		CachedResult:  req.ResultText,
		Location:      fieldLocationFor(partURI, req.DocumentURI, blockIndex),
		ParagraphText: docxbody.ParagraphText(paragraph),
		KnownCode:     IsKnownFieldCode(req.FieldCode),
	}, nil
}

// SetFieldResult updates the cached result text of an existing field selected by
// part + block index + field index.
func SetFieldResult(req *SetFieldResultRequest) (*SetFieldResultResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set field result request is nil")
	}

	partURI := req.PartURI
	if partURI == "" {
		partURI = req.DocumentURI
	}

	doc, err := req.Package.ReadXMLPart(partURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read part %s: %w", partURI, err)
	}
	root := doc.Root()

	paragraph, blockIndex, err := locateTargetParagraph(root, partURI, req.DocumentURI, req.BlockIndex)
	if err != nil {
		return nil, err
	}

	field := selectFieldInParagraph(paragraph, req.FieldIndex)
	if field == nil {
		return nil, fmt.Errorf("%w: %s field %d", ErrFieldNotFound, fieldLocationFor(partURI, req.DocumentURI, blockIndex), req.FieldIndex)
	}

	prevInstr, prevResult := readFieldInPlace(field)
	if req.ExpectedHash != "" {
		got := FieldContentHash(strings.TrimSpace(prevInstr), prevResult)
		if got != req.ExpectedHash {
			return nil, fmt.Errorf("%w: field expected %s but found %s", ErrFieldHashMismatch, req.ExpectedHash, got)
		}
	}

	fieldType := docxinspect.FieldTypeSimple
	if field.kind == fieldKindComplex {
		fieldType = docxinspect.FieldTypeComplex
	}
	setFieldResultText(field, req.Result)

	if err := req.Package.ReplaceXMLPart(partURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace part %s: %w", partURI, err)
	}

	return &SetFieldResultResult{
		PartURI:        partURI,
		BlockIndex:     blockIndex,
		FieldIndex:     req.FieldIndex,
		FieldType:      fieldType,
		Instruction:    strings.TrimSpace(prevInstr),
		PreviousResult: prevResult,
		CachedResult:   req.Result,
		Location:       fieldLocationFor(partURI, req.DocumentURI, blockIndex),
	}, nil
}

// ErrFieldHashMismatch is returned when an --expect-hash guard does not match.
var ErrFieldHashMismatch = errors.New("field hash mismatch")

// locateTargetParagraph resolves a paragraph element by 1-based index within the
// target part. For the main document part, the index addresses body blocks (and the
// block must be a paragraph). For header/footer parts it addresses w:p children.
func locateTargetParagraph(root *etree.Element, partURI, documentURI string, blockIndex int) (*etree.Element, int, error) {
	if blockIndex < 1 {
		return nil, 0, fmt.Errorf("%w: %d", ErrFieldParaOutOfRange, blockIndex)
	}
	if partURI == documentURI || strings.HasSuffix(partURI, "/document.xml") {
		body, err := docxbody.FindBody(root)
		if err != nil {
			return nil, 0, err
		}
		blocks := docxbody.Blocks(body)
		if blockIndex > len(blocks) {
			return nil, 0, fmt.Errorf("%w: %d", ErrFieldParaOutOfRange, blockIndex)
		}
		target := blocks[blockIndex-1]
		if target.Kind == "table" {
			return nil, 0, fmt.Errorf("%w (block %d)", ErrFieldInTable, blockIndex)
		}
		if target.Kind != "paragraph" {
			return nil, 0, fmt.Errorf("%w: block %d is %s, not a paragraph", ErrFieldParaOutOfRange, blockIndex, target.Kind)
		}
		return target.Element, blockIndex, nil
	}
	// Header/footer part: index addresses w:p children.
	paragraphs := namespaces.FindChildren(root, namespaces.NsW, "p")
	if blockIndex > len(paragraphs) {
		return nil, 0, fmt.Errorf("%w: %d", ErrFieldParaOutOfRange, blockIndex)
	}
	return paragraphs[blockIndex-1], blockIndex, nil
}

// fieldKind distinguishes the two field encodings handled in place.
type fieldKind int

const (
	fieldKindSimple fieldKind = iota
	fieldKindComplex
)

// fieldInPlace is a located field element within a paragraph, ready to mutate.
type fieldInPlace struct {
	kind fieldKind
	// For simple fields, simple is the w:fldSimple element.
	simple *etree.Element
	// For complex fields, the bookend runs and the instrText/t elements they wrap.
	beginRun, separateRun, endRun *etree.Element
	// resultRuns holds the w:t elements between separate and end; instrRuns holds the
	// w:instrText elements between begin and separate.
	resultRuns []*etree.Element
	instrRuns  []*etree.Element
}

// selectFieldInParagraph returns the field at 0-based fieldIndex among all fields in
// document order (simple and complex interleaved by position).
func selectFieldInParagraph(paragraph *etree.Element, fieldIndex int) *fieldInPlace {
	fields := locateFieldsInParagraph(paragraph)
	if fieldIndex < 0 || fieldIndex >= len(fields) {
		return nil
	}
	return fields[fieldIndex]
}

// paragraphFieldCount counts simple + complex fields in a paragraph.
func paragraphFieldCount(paragraph *etree.Element) int {
	return len(locateFieldsInParagraph(paragraph))
}

// locateFieldsInParagraph walks the paragraph children in order and returns each
// field (simple or complex) as a fieldInPlace.
func locateFieldsInParagraph(paragraph *etree.Element) []*fieldInPlace {
	var fields []*fieldInPlace
	var cur *fieldInPlace
	var afterSep bool
	depth := 0

	for _, child := range paragraph.ChildElements() {
		name := docxbody.LocalName(child.Tag)
		if name == "fldSimple" && depth == 0 {
			fields = append(fields, &fieldInPlace{kind: fieldKindSimple, simple: child})
			continue
		}
		if name != "r" {
			continue
		}
		for _, runChild := range child.ChildElements() {
			switch docxbody.LocalName(runChild.Tag) {
			case "fldChar":
				ct, _ := namespaces.Attr(runChild, namespaces.NsW, "fldCharType")
				switch ct {
				case "begin":
					if depth == 0 {
						cur = &fieldInPlace{kind: fieldKindComplex, beginRun: child}
						afterSep = false
					}
					depth++
				case "separate":
					if depth == 1 && cur != nil {
						cur.separateRun = child
						afterSep = true
					}
				case "end":
					depth--
					if depth == 0 && cur != nil {
						cur.endRun = child
						fields = append(fields, cur)
						cur = nil
						afterSep = false
					}
				}
			case "instrText":
				if depth == 1 && cur != nil && !afterSep {
					cur.instrRuns = append(cur.instrRuns, runChild)
				}
			case "t":
				if depth == 1 && cur != nil && afterSep {
					cur.resultRuns = append(cur.resultRuns, runChild)
				}
			}
		}
	}
	return fields
}

// readFieldInPlace returns the current instruction and result text of a located field.
func readFieldInPlace(f *fieldInPlace) (string, string) {
	if f.kind == fieldKindSimple {
		instr, _ := namespaces.Attr(f.simple, namespaces.NsW, "instr")
		var b strings.Builder
		for _, t := range namespaces.FindDescendants(f.simple, namespaces.NsW, "t") {
			b.WriteString(t.Text())
		}
		return instr, b.String()
	}
	var instr strings.Builder
	for _, it := range f.instrRuns {
		instr.WriteString(it.Text())
	}
	var result strings.Builder
	for _, t := range f.resultRuns {
		result.WriteString(t.Text())
	}
	return instr.String(), result.String()
}

// setFieldResultText replaces a field's cached result text in place, preserving the
// instruction and (for complex fields) the w:fldChar bookends.
func setFieldResultText(f *fieldInPlace, text string) {
	if f.kind == fieldKindSimple {
		prefix := f.simple.Space
		// Remove existing result runs, then add one carrying the new text.
		for _, child := range f.simple.ChildElements() {
			if docxbody.LocalName(child.Tag) == "r" {
				f.simple.RemoveChild(child)
			}
		}
		run := newElement(prefix, "r")
		if text != "" {
			appendTextChildren(run, prefix, text)
		} else {
			run.AddChild(newElement(prefix, "t"))
		}
		f.simple.AddChild(run)
		return
	}

	// Complex field: result runs live between separate and end. Remove the existing
	// result text runs and insert a single replacement run before the end run.
	prefix := f.endRun.Space
	paragraph := f.endRun.Parent()
	removedRuns := make(map[*etree.Element]bool)
	for _, t := range f.resultRuns {
		run := t.Parent()
		if run == nil {
			continue
		}
		if runContainsFldChar(run) {
			run.RemoveChild(t)
			continue
		}
		if !removedRuns[run] {
			if rp := run.Parent(); rp != nil {
				rp.RemoveChild(run)
			}
			removedRuns[run] = true
		}
	}
	run := newElement(prefix, "r")
	if text != "" {
		appendTextChildren(run, prefix, text)
	} else {
		run.AddChild(newElement(prefix, "t"))
	}
	paragraph.InsertChildAt(f.endRun.Index(), run)
}

func runContainsFldChar(run *etree.Element) bool {
	for _, child := range run.ChildElements() {
		if docxbody.LocalName(child.Tag) == "fldChar" {
			return true
		}
	}
	return false
}

// normalizeInstruction pads a field code with the single leading/trailing spaces Word
// uses for w:instr values, unless the caller already padded it.
func normalizeInstruction(code string) string {
	trimmed := strings.TrimSpace(code)
	return " " + trimmed + " "
}

// fieldLocationFor renders a body:/header1: style location string.
func fieldLocationFor(partURI, documentURI string, blockIndex int) string {
	prefix := "body"
	if partURI != documentURI && !strings.HasSuffix(partURI, "/document.xml") {
		name := partURI
		if idx := strings.LastIndex(name, "/"); idx >= 0 {
			name = name[idx+1:]
		}
		prefix = strings.TrimSuffix(name, ".xml")
	}
	return fmt.Sprintf("%s:%d", prefix, blockIndex)
}
