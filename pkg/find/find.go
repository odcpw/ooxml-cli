// Package find implements semantic cross-object text search over OOXML
// packages (PPTX, XLSX, DOCX). It returns, for each hit, the package type,
// a human location, stable selectors, the matched value, surrounding context,
// and a pre-filled, runnable mutation command so an agent can locate-then-edit
// without re-parsing the package or guessing command syntax.
//
// The package is pure: it takes an already-opened opc.PackageSession and option
// values, and returns hits. It performs no I/O beyond reading parts from the
// session and never mutates the package.
package find

import (
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"

	docxextract "github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmodel "github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxextract "github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	pptxselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmodel "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
)

// ContractVersion identifies the stable shape of the find result contract.
const ContractVersion = "ooxml-find.v1"

// MatchType restricts which hit kinds a search returns.
type MatchType string

const (
	// MatchAll searches every supported target (default).
	MatchAll MatchType = "all"
	// MatchText searches PPTX/DOCX text and XLSX cell values.
	MatchText MatchType = "text"
	// MatchFormula searches XLSX cell formulas only.
	MatchFormula MatchType = "formula"
	// MatchName searches XLSX defined names only.
	MatchName MatchType = "name"
)

// ValidMatchTypes lists the accepted --type values.
var ValidMatchTypes = []MatchType{MatchAll, MatchText, MatchFormula, MatchName}

// ParseMatchType validates a --type value.
func ParseMatchType(s string) (MatchType, error) {
	switch MatchType(s) {
	case MatchAll, MatchText, MatchFormula, MatchName:
		return MatchType(s), nil
	default:
		return "", fmt.Errorf("invalid --type %q (expected one of: all, text, formula, name)", s)
	}
}

// HitKind classifies a single hit so callers can branch on it.
type HitKind string

const (
	KindPPTXText    HitKind = "pptx-text"    // slide shape/table-cell visible text
	KindPPTXNotes   HitKind = "pptx-notes"   // speaker notes text
	KindXLSXValue   HitKind = "xlsx-value"   // worksheet cell value
	KindXLSXFormula HitKind = "xlsx-formula" // worksheet cell formula
	KindXLSXName    HitKind = "xlsx-name"    // workbook defined name
	KindDOCXText    HitKind = "docx-text"    // document body paragraph/table text
)

// Options controls a search.
type Options struct {
	Query      string
	Type       MatchType
	IgnoreCase bool
	Regex      bool
	// Max caps the number of returned hits (0 = unlimited). The cap is applied
	// at the hit level in file order so results stay deterministic.
	Max int
}

// matcher is the compiled, reusable predicate for one search.
type matcher struct {
	query      string
	ignoreCase bool
	re         *regexp.Regexp
}

// newMatcher compiles the query once for the whole search.
func newMatcher(opts Options) (*matcher, error) {
	if opts.Query == "" {
		return nil, fmt.Errorf("query must not be empty")
	}
	m := &matcher{query: opts.Query, ignoreCase: opts.IgnoreCase}
	if opts.Regex {
		pattern := opts.Query
		if opts.IgnoreCase {
			pattern = "(?i)" + pattern
		}
		re, err := regexp.Compile(pattern)
		if err != nil {
			return nil, fmt.Errorf("invalid regular expression: %w", err)
		}
		m.re = re
	}
	return m, nil
}

// matchSubstring reports whether value matches and returns the exact matched
// substring (the concrete literal that matched, never the regex pattern). The
// matched substring is what goes into a generated mutation command so the
// command stays a literal that the mutating tool can find.
func matchSubstring(query, value string, ignoreCase bool, re *regexp.Regexp) (string, bool) {
	if value == "" {
		return "", false
	}
	if re != nil {
		loc := re.FindStringIndex(value)
		if loc == nil {
			return "", false
		}
		return value[loc[0]:loc[1]], true
	}
	if ignoreCase {
		idx, end := caseFoldedIndex(value, query)
		if idx < 0 {
			return "", false
		}
		return value[idx:end], true
	}
	idx := strings.Index(value, query)
	if idx < 0 {
		return "", false
	}
	return value[idx : idx+len(query)], true
}

func caseFoldedIndex(value, query string) (int, int) {
	foldedValue, starts, ends := foldWithOriginalByteMap(value)
	foldedQuery := strings.ToLower(query)
	idx := strings.Index(foldedValue, foldedQuery)
	if idx < 0 || idx >= len(starts) {
		return -1, -1
	}
	last := idx + len(foldedQuery) - 1
	if last < 0 || last >= len(ends) {
		return -1, -1
	}
	return starts[idx], ends[last]
}

func foldWithOriginalByteMap(value string) (string, []int, []int) {
	var b strings.Builder
	starts := []int{}
	ends := []int{}
	for start, r := range value {
		end := start + len(string(r))
		folded := strings.ToLower(string(r))
		b.WriteString(folded)
		for range []byte(folded) {
			starts = append(starts, start)
			ends = append(ends, end)
		}
	}
	return b.String(), starts, ends
}

// match runs the predicate against a candidate value.
func (m *matcher) match(value string) (string, bool) {
	return matchSubstring(m.query, value, m.ignoreCase, m.re)
}

// Hit is a single search result.
type Hit struct {
	Index           int     `json:"index"`
	PackageType     string  `json:"packageType"`
	Kind            HitKind `json:"kind"`
	Location        string  `json:"location"`
	PartURI         string  `json:"partUri,omitempty"`
	PrimarySelector string  `json:"primarySelector"`
	// Handle is a stable, paste-safe object handle for this hit's scope, when a
	// native scope id is available. For a PPTX shape-text hit it is a SHAPE handle
	// (H:pptx/s:<sldId>/shape:n:<cNvPrId>) that confines a replacement to the ONE
	// matched shape and survives slide+shape reorder/insert/delete. When the
	// matched shape's cNvPr id cannot be uniquely determined the hit falls back to
	// a slide handle (H:pptx/s:<sldId>) and carries a MutationNote saying the op is
	// slide-wide. PPTX notes hits carry a slide handle. Additive; omitted when no
	// scope id exists.
	Handle          string            `json:"handle,omitempty"`
	Selectors       []string          `json:"selectors"`
	MatchedValue    string            `json:"matchedValue"`
	Context         string            `json:"context"`
	MutationCommand string            `json:"mutationCommand"`
	MutationNote    string            `json:"mutationNote,omitempty"`
	Metadata        map[string]string `json:"metadata,omitempty"`

	// op is the structured, apply-derivable mutation for this hit. It is the
	// in-memory source of truth from which MutationCommand is rendered and from
	// which HitsToOps builds apply operations. It is never serialized (find's
	// JSON contract is unchanged) and is read only in-package. A zero op (empty
	// Command) marks a hit with no semantic mutation command.
	op opSpec
}

// Result is the full, stable find contract.
type Result struct {
	ContractVersion string `json:"contractVersion"`
	PackageType     string `json:"packageType"`
	Query           string `json:"query"`
	Type            string `json:"type"`
	IgnoreCase      bool   `json:"ignoreCase"`
	Regex           bool   `json:"regex"`
	Max             int    `json:"max"`
	Truncated       bool   `json:"truncated"`
	TotalHits       int    `json:"totalHits"`
	Hits            []Hit  `json:"hits"`
}

// wantsText reports whether the current type selection includes free text.
func wantsText(t MatchType) bool { return t == MatchAll || t == MatchText }

// Search dispatches to the package-type-specific searcher and returns a stable
// Result. The session must already be opened; packageType is the opc-detected
// type string ("pptx", "xlsx", or "docx").
func Search(session opc.PackageSession, packageType string, opts Options) (*Result, error) {
	if opts.Type == "" {
		opts.Type = MatchAll
	}
	m, err := newMatcher(opts)
	if err != nil {
		return nil, err
	}

	var hits []Hit
	switch packageType {
	case "pptx":
		hits, err = searchPPTX(session, m, opts)
	case "xlsx":
		hits, err = searchXLSX(session, m, opts)
	case "docx":
		hits, err = searchDOCX(session, m, opts)
	default:
		return nil, fmt.Errorf("unsupported package type: %s", packageType)
	}
	if err != nil {
		return nil, err
	}

	truncated := false
	if opts.Max > 0 && len(hits) > opts.Max {
		hits = hits[:opts.Max]
		truncated = true
	}
	for i := range hits {
		hits[i].Index = i
	}
	if hits == nil {
		hits = []Hit{}
	}

	return &Result{
		ContractVersion: ContractVersion,
		PackageType:     packageType,
		Query:           opts.Query,
		Type:            string(opts.Type),
		IgnoreCase:      opts.IgnoreCase,
		Regex:           opts.Regex,
		Max:             opts.Max,
		Truncated:       truncated,
		TotalHits:       len(hits),
		Hits:            hits,
	}, nil
}

// contextSnippet trims a possibly-long value to a readable, single-line
// context string centered loosely on the start of the value.
func contextSnippet(value string) string {
	const maxLen = 160
	s := strings.ReplaceAll(value, "\n", " ")
	s = strings.ReplaceAll(s, "\r", " ")
	s = strings.TrimSpace(s)
	if len(s) <= maxLen {
		return s
	}
	return s[:maxLen] + "…"
}

// shellArg quotes a value for safe inclusion in a generated POSIX shell
// command. Empty becomes ”. It mirrors the CLI's pptxXLSXCommandArg helper so
// generated commands paste-run identically.
func shellArg(value string) string {
	if value == "" {
		return "''"
	}
	if !strings.ContainsAny(value, " \t\r\n'\"\\$`<>|&;()[]{}*?!") {
		return value
	}
	return "'" + strings.ReplaceAll(value, "'", "'\"'\"'") + "'"
}

// ---------------------------------------------------------------------------
// PPTX
// ---------------------------------------------------------------------------

func searchPPTX(session opc.PackageSession, m *matcher, opts Options) ([]Hit, error) {
	// PPTX only carries text-style hits; formula/name selections yield nothing.
	if !wantsText(opts.Type) {
		return []Hit{}, nil
	}

	graph, err := pptxinspect.ParsePresentation(session)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	// Tally sldId occurrences so a duplicated sldId omits its slide handle
	// (a handle for a non-unique sldId would mis-resolve).
	sldIDCounts := make(map[uint32]int, len(graph.Slides))
	for _, slideRef := range graph.Slides {
		if slideRef.SlideID != 0 {
			sldIDCounts[slideRef.SlideID]++
		}
	}

	var hits []Hit
	for _, slideRef := range graph.Slides {
		slideNum := slideRef.SlideNumber

		slideDoc, derr := session.ReadXMLPart(slideRef.PartURI)
		if derr != nil || slideDoc == nil || slideDoc.Root() == nil {
			continue
		}
		spTree := findSpTree(slideDoc.Root())

		// Build the slide catalog so a per-shape handle is minted ONLY for a
		// cNvPr id that uniquely resolves on this slide. Gating on the resolver's
		// own ambiguity rule guarantees an emitted shape handle never aborts a
		// later find->apply op with HANDLE_AMBIGUOUS. A catalog build failure
		// degrades gracefully to slide-scope (handled per-hit below).
		var catalog *pptxselectors.SlideCatalog
		if cat, cerr := pptxselectors.BuildSlideCatalogFromGraph(session, graph, slideNum); cerr == nil {
			catalog = cat
		}

		// Shape paragraph text. We match per-paragraph so the matched value is a
		// real visible text node (what `replace text-occurrences` operates on).
		if spTree != nil {
			for _, sp := range childShapes(spTree, "sp") {
				txBody := childByLocal(sp, "txBody")
				if txBody == nil {
					continue
				}
				block := pptxinspect.ExtractTextBody(txBody)
				if block == nil {
					continue
				}
				shapeID, shapeOK := shapeCNvPrID(sp, "nvSpPr")
				for _, para := range block.Paragraphs {
					if matched, ok := m.match(para.Text); ok {
						hits = append(hits, pptxTextHit(slideRef.PartURI, slideNum, slideRef.SlideID, shapeID, shapeOK, matched, para.Text, sldIDCounts, catalog))
					}
				}
			}

			// Table cell text (graphicFrame -> a:graphic -> a:graphicData -> a:tbl).
			for _, gf := range childShapes(spTree, "graphicFrame") {
				tbl := findTbl(gf)
				if tbl == nil {
					continue
				}
				info := pptxinspect.ParseTable(tbl)
				if info == nil {
					continue
				}
				// A table cell has no cNvPr of its own; scope to the enclosing
				// graphicFrame shape (its nvGraphicFramePr/cNvPr id). Replacement is
				// then confined to that ONE table shape, not the whole slide.
				shapeID, shapeOK := shapeCNvPrID(gf, "nvGraphicFramePr")
				for _, row := range info.Cells {
					for _, cell := range row {
						if matched, ok := m.match(cell); ok {
							hits = append(hits, pptxTextHit(slideRef.PartURI, slideNum, slideRef.SlideID, shapeID, shapeOK, matched, cell, sldIDCounts, catalog))
						}
					}
				}
			}
		}

		// Speaker notes. There is no semantic mutation command for notes today,
		// so the hit carries an explanatory note instead of a dead command.
		report, nerr := pptxextract.ExtractNotesForSlide(session, slideRef)
		if nerr == nil && report != nil && report.Notes != nil {
			for _, para := range report.Notes.Paragraphs {
				if matched, ok := m.match(para.Text); ok {
					hits = append(hits, pptxNotesHit(report.PartURI, slideNum, slideRef.SlideID, matched, para.Text, sldIDCounts))
				}
			}
		}
	}
	return hits, nil
}

func pptxTextHit(partURI string, slide int, slideID uint32, shapeID int, shapeIDFound bool, matched, full string, sldIDCounts map[uint32]int, catalog *pptxselectors.SlideCatalog) Hit {
	// Prefer a SHAPE-SCOPED op so the replacement is confined to the ONE matched
	// shape (the cardinal find->ops fix): an agent executing the op for hit N must
	// not rewrite the same substring in sibling shapes on the slide. A shape handle
	// is minted only when the slide sldId is unique AND the shape's cNvPr id
	// uniquely resolves on the slide (the resolver's own ambiguity rule), so the
	// emitted op never aborts a later apply with HANDLE_AMBIGUOUS. When no shape
	// handle is available the op falls back to slide-scope and carries a
	// MutationNote so a shape-specific hit is never silently widened to the slide.
	shapeHandle := pptxShapeHandle(slideID, shapeID, shapeIDFound, sldIDCounts, catalog)
	metadata := map[string]string{"slide": fmt.Sprintf("%d", slide)}
	if shapeIDFound {
		metadata["shapeId"] = fmt.Sprintf("%d", shapeID)
	}

	if shapeHandle != "" {
		// The shape handle is the sole, self-describing target: it encodes its own
		// slide scope, so the op carries only --for-shape (no --for-slides). The
		// handle value is rendered directly into the human command (there is no
		// positional shape selector to show instead).
		op := opSpec{
			Command: "pptx replace text-occurrences",
			Args: []opArg{
				{"match-text", matched},
				{"new-text", newOpPlaceholder},
				{"for-shape", shapeHandle},
			},
			ReplaceKey: "new-text",
			HandleKey:  "for-shape",
			Handle:     shapeHandle,
		}
		return Hit{
			PackageType:     "pptx",
			Kind:            KindPPTXText,
			Location:        fmt.Sprintf("slide:%d shape:%d", slide, shapeID),
			PartURI:         partURI,
			PrimarySelector: fmt.Sprintf("slide:%d", slide),
			Handle:          shapeHandle,
			Selectors:       []string{fmt.Sprintf("slide:%d", slide)},
			MatchedValue:    matched,
			Context:         contextSnippet(full),
			MutationCommand: op.humanCommand(),
			Metadata:        metadata,
			op:              op,
		}
	}

	// Fallback: no usable shape handle. Scope to the slide and NOTE the widening.
	slideHandle := pptxSlideHandle(slideID, sldIDCounts)
	op := opSpec{
		Command: "pptx replace text-occurrences",
		Args: []opArg{
			{"match-text", matched},
			{"new-text", newOpPlaceholder},
			{"for-slides", fmt.Sprintf("%d", slide)},
		},
		ReplaceKey: "new-text",
		// The slide handle restricts the replacement to the SAME slide by durable
		// sldId, surviving slide reorder/insert/delete that an earlier batch op may
		// cause. Falls back to the positional --for-slides number when no handle.
		HandleKey: "for-slides",
		Handle:    slideHandle,
	}
	return Hit{
		PackageType:     "pptx",
		Kind:            KindPPTXText,
		Location:        fmt.Sprintf("slide:%d", slide),
		PartURI:         partURI,
		PrimarySelector: fmt.Sprintf("slide:%d", slide),
		Handle:          slideHandle,
		Selectors:       []string{fmt.Sprintf("slide:%d", slide)},
		MatchedValue:    matched,
		Context:         contextSnippet(full),
		MutationCommand: op.humanCommand(),
		MutationNote:    "shape scope unavailable (shape cNvPr id missing or not unique on slide); this op is SLIDE-WIDE and may rewrite the same text in sibling shapes",
		Metadata:        metadata,
		op:              op,
	}
}

// shapeCNvPrID extracts a top-level shape's native cNvPr@id via its non-visual
// properties container (e.g. "nvSpPr" for sp, "nvGraphicFramePr" for
// graphicFrame). It returns the id and whether a numeric id was found.
func shapeCNvPrID(shape *etree.Element, nvLocal string) (int, bool) {
	if shape == nil {
		return 0, false
	}
	nvPr := childByLocal(shape, nvLocal)
	if nvPr == nil {
		return 0, false
	}
	cNvPr := childByLocal(nvPr, "cNvPr")
	if cNvPr == nil {
		return 0, false
	}
	idAttr := cNvPr.SelectAttr("id")
	if idAttr == nil {
		return 0, false
	}
	id, err := strconv.Atoi(idAttr.Value)
	if err != nil {
		return 0, false
	}
	return id, true
}

// pptxShapeHandle mints a shape handle (H:pptx/s:<sldId>/shape:n:<cNvPrId>) for a
// find hit, or "" when no stable, unambiguously-resolvable shape handle exists.
// It returns "" when: the cNvPr id was not found; the slide has no native sldId
// or that sldId is shared by more than one slide (a non-unique scope would
// mis-resolve); or the cNvPr id is not unique among the slide's top-level shapes
// (the resolver would refuse with HANDLE_AMBIGUOUS). Gating on the catalog's own
// ambiguity rule guarantees a minted handle resolves to exactly one shape.
func pptxShapeHandle(slideID uint32, shapeID int, shapeIDFound bool, sldIDCounts map[uint32]int, catalog *pptxselectors.SlideCatalog) string {
	if !shapeIDFound || slideID == 0 {
		return ""
	}
	if sldIDCounts != nil && sldIDCounts[slideID] > 1 {
		return ""
	}
	if catalog == nil || catalog.IsShapeIDAmbiguous(shapeID) {
		return ""
	}
	return pptxhandle.FormatShape(slideID, shapeID)
}

// pptxSlideHandle mints a slide handle (H:pptx/s:<sldId>) for a find hit, or ""
// when the slide has no native sldId OR when the sldId is shared by more than one
// slide (a handle for a non-unique sldId would mis-resolve), so the additive
// field is omitted. sldIDCounts maps each sldId to the number of slides carrying
// it.
func pptxSlideHandle(slideID uint32, sldIDCounts map[uint32]int) string {
	if slideID == 0 {
		return ""
	}
	if sldIDCounts != nil && sldIDCounts[slideID] > 1 {
		return ""
	}
	return pptxhandle.FormatSlide(slideID)
}

func pptxNotesHit(partURI string, slide int, slideID uint32, matched, full string, sldIDCounts map[uint32]int) Hit {
	return Hit{
		PackageType:     "pptx",
		Kind:            KindPPTXNotes,
		Location:        fmt.Sprintf("slide:%d notes", slide),
		PartURI:         partURI,
		PrimarySelector: fmt.Sprintf("slide:%d", slide),
		Handle:          pptxSlideHandle(slideID, sldIDCounts),
		Selectors:       []string{fmt.Sprintf("slide:%d", slide)},
		MatchedValue:    matched,
		Context:         contextSnippet(full),
		MutationCommand: "",
		MutationNote:    "speaker-notes text has no semantic mutation command; edit the notes part directly",
		Metadata:        map[string]string{"slide": fmt.Sprintf("%d", slide)},
		// No structured op: notes have no semantic mutation command, so this hit
		// is skipped by HitsToOps.
	}
}

// findSpTree locates the cSld/spTree element, namespace-tolerant.
func findSpTree(root *etree.Element) *etree.Element {
	if sp := root.FindElement("{" + pptxns.NsP + "}cSld/{" + pptxns.NsP + "}spTree"); sp != nil {
		return sp
	}
	return root.FindElement("cSld/spTree")
}

// childShapes returns direct children of spTree with the given local name,
// namespace-tolerant.
func childShapes(spTree *etree.Element, local string) []*etree.Element {
	if shapes := spTree.FindElements("{" + pptxns.NsP + "}" + local); len(shapes) > 0 {
		return shapes
	}
	return spTree.FindElements(local)
}

// childByLocal returns the first child element matching local name, ignoring ns.
func childByLocal(parent *etree.Element, local string) *etree.Element {
	for _, c := range parent.ChildElements() {
		if localName(c.Tag) == local {
			return c
		}
	}
	return nil
}

// findTbl descends a graphicFrame to its a:tbl element by local name.
func findTbl(gf *etree.Element) *etree.Element {
	graphic := childByLocal(gf, "graphic")
	if graphic == nil {
		return nil
	}
	data := childByLocal(graphic, "graphicData")
	if data == nil {
		return nil
	}
	return childByLocal(data, "tbl")
}

func localName(tag string) string {
	if i := strings.Index(tag, ":"); i >= 0 {
		return tag[i+1:]
	}
	return tag
}

// ---------------------------------------------------------------------------
// XLSX
// ---------------------------------------------------------------------------

func searchXLSX(session opc.PackageSession, m *matcher, opts Options) ([]Hit, error) {
	workbook, err := xlsxinspect.ParseWorkbook(session)
	if err != nil {
		return nil, fmt.Errorf("failed to parse workbook: %w", err)
	}

	var hits []Hit

	// Map each sheet NAME to its native sheetId and tally sheetId occurrences so
	// the handle surface can omit a handle for any non-unique sheetId (the same
	// omit-on-duplicate contract as inspect/slides list). Sheet names are unique
	// within a workbook, so name -> sheetId is a safe lookup.
	sheetIDByName := make(map[string]string, len(workbook.Sheets))
	sheetIDCounts := make(map[string]int, len(workbook.Sheets))
	for _, ref := range workbook.Sheets {
		if ref.SheetID != "" {
			sheetIDByName[ref.Name] = ref.SheetID
			sheetIDCounts[ref.SheetID]++
		}
	}

	// Cell values and formulas (skipped entirely when --type name).
	if opts.Type != MatchName {
		ctx, cerr := xlsxsheet.LoadContext(session, workbook)
		if cerr != nil {
			return nil, fmt.Errorf("failed to load workbook context: %w", cerr)
		}
		for _, ref := range workbook.Sheets {
			if ref.PartURI == "" {
				continue
			}
			report, rerr := xlsxsheet.Read(session, ref, ctx, xlsxsheet.ReadOptions{
				IncludeData:  true,
				IncludeEmpty: false,
			})
			if rerr != nil || report == nil {
				continue
			}
			for _, row := range report.Rows {
				for _, cell := range row.Cells {
					hits = append(hits, xlsxCellHits(ref.Name, cell, m, opts, sheetIDByName, sheetIDCounts)...)
				}
			}
		}
	}

	// Defined names (only when type allows names: all or name).
	if opts.Type == MatchAll || opts.Type == MatchName {
		names, nerr := xlsxinspect.ListDefinedNames(session)
		if nerr == nil {
			nameCounts := xlsxWorkbookScopedNameCounts(names)
			for _, dn := range names {
				hits = append(hits, xlsxNameHits(dn, m, nameCounts)...)
			}
		}
	}

	return hits, nil
}

// xlsxCellHits emits value and/or formula hits for one cell, honoring --type.
// sheetIDByName/sheetIDCounts supply the native sheetId (and its uniqueness) so
// each hit can carry a stable cell handle, omitted for an unknown or duplicated
// sheetId. A cell handle survives sheet reorder/rename but NOT a row/column
// insert that shifts the A1 address (see pkg/xlsx/handle).
func xlsxCellHits(sheet string, cell xlsxmodel.Cell, m *matcher, opts Options, sheetIDByName map[string]string, sheetIDCounts map[string]int) []Hit {
	var hits []Hit

	cellHandle := ""
	if id, ok := sheetIDByName[sheet]; ok && sheetIDCounts[id] == 1 && cell.Ref != "" {
		cellHandle = xlsxhandle.FormatCell(id, cell.Ref)
	}

	// Value match (counts as "text").
	if wantsText(opts.Type) {
		if matched, ok := m.match(cell.Value); ok {
			op := opSpec{
				Command: "xlsx cells set",
				Args: []opArg{
					{"sheet", sheet},
					{"cell", cell.Ref},
					{"value", newOpPlaceholder},
				},
				ReplaceKey: "value",
				// A cell handle carries the sheetId scope (surviving sheet
				// reorder/rename) and is authoritative over --sheet. It is
				// address-positional within the grid: a row/column INSERT that
				// shifts the A1 address fails cleanly with HANDLE_STALE (the address
				// empties), but a row/column DELETE can shift a populated cell ONTO
				// the address and write the wrong cell silently. HitsToOps therefore
				// reports cell-handle ops as position-dependent, and apply rejects a
				// batch that shifts rows/columns before such an op. Falls back to
				// positional when no handle.
				HandleKey: "cell",
				Handle:    cellHandle,
			}
			hits = append(hits, Hit{
				PackageType:     "xlsx",
				Kind:            KindXLSXValue,
				Location:        fmt.Sprintf("sheet:%s ref:%s", sheet, cell.Ref),
				PrimarySelector: fmt.Sprintf("%s!%s", sheet, cell.Ref),
				Handle:          cellHandle,
				Selectors:       []string{fmt.Sprintf("%s!%s", sheet, cell.Ref), cell.Ref},
				MatchedValue:    matched,
				Context:         contextSnippet(cell.Value),
				MutationCommand: op.humanCommand(),
				Metadata:        map[string]string{"sheet": sheet, "ref": cell.Ref},
				op:              op,
			})
		}
	}

	// Formula match.
	if (opts.Type == MatchAll || opts.Type == MatchFormula) && cell.Formula != "" {
		if matched, ok := m.match(cell.Formula); ok {
			op := opSpec{
				Command: "xlsx cells set",
				Args: []opArg{
					{"sheet", sheet},
					{"cell", cell.Ref},
					{"formula", newOpPlaceholder},
				},
				ReplaceKey: "formula",
				HandleKey:  "cell",
				Handle:     cellHandle,
			}
			hits = append(hits, Hit{
				PackageType:     "xlsx",
				Kind:            KindXLSXFormula,
				Location:        fmt.Sprintf("sheet:%s ref:%s", sheet, cell.Ref),
				PrimarySelector: fmt.Sprintf("%s!%s", sheet, cell.Ref),
				Handle:          cellHandle,
				Selectors:       []string{fmt.Sprintf("%s!%s", sheet, cell.Ref), cell.Ref},
				MatchedValue:    matched,
				Context:         contextSnippet(cell.Formula),
				MutationCommand: op.humanCommand(),
				Metadata:        map[string]string{"sheet": sheet, "ref": cell.Ref, "formula": cell.Formula},
				op:              op,
			})
		}
	}

	return hits
}

// xlsxNameHits emits a hit when the query matches a defined name's name or ref.
func xlsxNameHits(dn xlsxmodel.DefinedName, m *matcher, workbookNameCounts map[string]int) []Hit {
	matched, ok := m.match(dn.Name)
	field := "name"
	if !ok {
		matched, ok = m.match(dn.Ref)
		field = "ref"
	}
	if !ok {
		return nil
	}
	loc := fmt.Sprintf("name:%s", dn.Name)
	if dn.Scope != "" {
		loc = fmt.Sprintf("name:%s scope:%s", dn.Name, dn.Scope)
	}
	// A defined name is a native, position-independent address. Mint a handle
	// only for workbook-scoped names (the handle grammar is workbook-scoped);
	// sheet-scoped names keep their legacy selectors only.
	nameHandle := ""
	if dn.Scope == "workbook" && dn.Name != "" && workbookNameCounts[dn.Name] == 1 {
		nameHandle = xlsxhandle.FormatDefinedName(dn.Name)
	}
	op := opSpec{
		Command: "xlsx names update",
		Args: []opArg{
			{"name", dn.Name},
			{"ref", newOpPlaceholder},
		},
		ReplaceKey: "ref",
		// The defined-name handle resolves by native name (position-independent),
		// so it survives any sheet/row/column structural edit.
		HandleKey: "name",
		Handle:    nameHandle,
	}
	return []Hit{{
		PackageType:     "xlsx",
		Kind:            KindXLSXName,
		Location:        loc,
		PrimarySelector: dn.Name,
		Handle:          nameHandle,
		Selectors:       []string{dn.Name},
		MatchedValue:    matched,
		Context:         contextSnippet(fmt.Sprintf("%s = %s", dn.Name, dn.Ref)),
		MutationCommand: op.humanCommand(),
		Metadata:        map[string]string{"name": dn.Name, "ref": dn.Ref, "matchedField": field},
		op:              op,
	}}
}

func xlsxWorkbookScopedNameCounts(names []xlsxmodel.DefinedName) map[string]int {
	counts := make(map[string]int)
	for _, name := range names {
		if name.Scope == "workbook" && strings.TrimSpace(name.Name) != "" {
			counts[name.Name]++
		}
	}
	return counts
}

// ---------------------------------------------------------------------------
// DOCX
// ---------------------------------------------------------------------------

func searchDOCX(session opc.PackageSession, m *matcher, opts Options) ([]Hit, error) {
	// DOCX only carries text-style hits; formula/name selections yield nothing.
	if !wantsText(opts.Type) {
		return []Hit{}, nil
	}

	documentURI, err := docxinspect.FindMainDocumentPart(session)
	if err != nil {
		return nil, fmt.Errorf("failed to locate main document part: %w", err)
	}

	blocks, err := docxextract.ExtractBlocks(&docxextract.ExtractBlocksRequest{
		Session:     session,
		DocumentURI: documentURI,
	})
	if err != nil {
		return nil, fmt.Errorf("failed to extract document blocks: %w", err)
	}

	var hits []Hit
	for _, block := range blocks.Blocks {
		// Match per line so the matched value is a real text fragment. Body and
		// table text are tab/newline-joined inside a block; split before matching.
		for _, line := range splitBlockLines(block) {
			if matched, ok := m.match(line); ok {
				hits = append(hits, docxTextHit(documentURI, block, matched, line))
			}
		}
	}
	return hits, nil
}

// splitBlockLines breaks a block's text into candidate match units. Tables join
// rows by newline and cells by tab; paragraphs are single lines.
func splitBlockLines(block docxextract.BlockReport) []string {
	if block.Kind == docxmodel.BlockKindTable {
		var lines []string
		for _, raw := range strings.Split(block.Text, "\n") {
			lines = append(lines, strings.Split(raw, "\t")...)
		}
		return lines
	}
	return []string{block.Text}
}

func docxTextHit(partURI string, block docxextract.BlockReport, matched, full string) Hit {
	globalOp := opSpec{
		Command: "docx replace",
		Args: []opArg{
			{"find", matched},
			{"replace", newOpPlaceholder},
		},
		ReplaceKey: "replace",
	}
	op := opSpec{}
	mutationCommand := globalOp.humanCommand()
	mutationNote := "DOCX global replace is shown for manual use only; find --to-ops skips this hit unless it can emit a paragraph-handle operation"

	if block.Kind == docxmodel.BlockKindParagraph && block.Handle != "" {
		replaceToken := collisionFreeReplaceToken(full)
		textTemplate := strings.Replace(full, matched, replaceToken, 1)
		op = opSpec{
			Command: "docx paragraphs set",
			Args: []opArg{
				{"handle", block.Handle},
				{"text", textTemplate},
			},
			ReplaceKey:   "text",
			ReplaceToken: replaceToken,
			HandleKey:    "handle",
			Handle:       block.Handle,
		}
		mutationCommand = op.humanCommand()
		mutationNote = ""
	}
	hit := Hit{
		PackageType:     "docx",
		Kind:            KindDOCXText,
		Location:        fmt.Sprintf("block:%d", block.Index),
		PartURI:         partURI,
		PrimarySelector: block.ID,
		Selectors:       []string{block.ID, fmt.Sprintf("block:%d", block.Index)},
		MatchedValue:    matched,
		Context:         contextSnippet(full),
		MutationCommand: mutationCommand,
		MutationNote:    mutationNote,
		Metadata: map[string]string{
			"block": fmt.Sprintf("%d", block.Index),
			"kind":  string(block.Kind),
		},
		op: op,
	}
	// Surface a stable paragraph handle ONLY when the paragraph already carries a
	// w14:paraId marker AND that marker is unique. find is pure-read: it never
	// injects, so a marker-less paragraph has no handle here (a mutate will inject
	// one and return it). block.Handle is already blanked by ExtractBlocks for a
	// duplicate (ambiguous) marker, so a non-unique marker is never advertised.
	hit.Handle = block.Handle
	return hit
}

func collisionFreeReplaceToken(text string) string {
	if !strings.Contains(text, newOpPlaceholder) {
		return newOpPlaceholder
	}
	for i := 1; ; i++ {
		token := fmt.Sprintf("<OOXML_NEW_%d>", i)
		if !strings.Contains(text, token) {
			return token
		}
	}
}

// SortHitsByPosition is a stable helper retained for callers that build hits out
// of order. The per-format searchers already emit in file order, so this is a
// no-op for them, but it keeps ordering guarantees explicit and testable.
func SortHitsByPosition(hits []Hit) {
	sort.SliceStable(hits, func(i, j int) bool { return hits[i].Index < hits[j].Index })
}
