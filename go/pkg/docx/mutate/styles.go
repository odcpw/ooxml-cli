package mutate

import (
	"errors"
	"fmt"
	"sort"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	// ErrStyleNotFound indicates the requested styleId is absent from
	// word/styles.xml among styles of the expected type.
	ErrStyleNotFound = errors.New("style not found")
	// ErrStyleTypeMismatch indicates the styleId exists but its w:type does
	// not match the requested target (e.g. a character style on a paragraph).
	ErrStyleTypeMismatch = errors.New("style type mismatch")
)

// StyleNotFoundError carries the candidate style ids of the expected type so
// callers can surface helpful suggestions.
type StyleNotFoundError struct {
	StyleID    string
	StyleType  string
	Candidates []string
}

func (e *StyleNotFoundError) Error() string {
	if len(e.Candidates) == 0 {
		return fmt.Sprintf("%v: %q (%s); no %s styles defined", ErrStyleNotFound, e.StyleID, e.StyleType, e.StyleType)
	}
	return fmt.Sprintf("%v: %q (%s); available %s styles: %v", ErrStyleNotFound, e.StyleID, e.StyleType, e.StyleType, e.Candidates)
}

func (e *StyleNotFoundError) Unwrap() error { return ErrStyleNotFound }

// StyleTypeMismatchError carries the actual type of an existing style that did
// not match the requested target.
type StyleTypeMismatchError struct {
	StyleID      string
	WantType     string
	ActualType   string
	WantedTarget string
}

func (e *StyleTypeMismatchError) Error() string {
	return fmt.Sprintf("%v: %q is a %s style but %s target requires a %s style", ErrStyleTypeMismatch, e.StyleID, e.ActualType, e.WantedTarget, e.WantType)
}

func (e *StyleTypeMismatchError) Unwrap() error { return ErrStyleTypeMismatch }

// ApplyParagraphStyleRequest applies a paragraph style (w:pStyle) to one body
// paragraph selected by 1-based block index.
type ApplyParagraphStyleRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	StylesURI    string
	Index        int
	StyleID      string
	ExpectedHash string
	Validate     bool
}

// ApplyRunStyleRequest applies a run style (w:rStyle) to every run of one body
// paragraph selected by 1-based block index.
type ApplyRunStyleRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	StylesURI    string
	Index        int
	StyleID      string
	ExpectedHash string
	Validate     bool
}

// ApplyTableStyleRequest applies a table style (w:tblStyle) to one body table
// selected by 1-based block index.
type ApplyTableStyleRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	StylesURI    string
	Index        int
	StyleID      string
	ExpectedHash string
	Validate     bool
}

// ApplyStyleResult is the readback for any of the style-apply mutations.
type ApplyStyleResult struct {
	// Index is the user-facing selector: the body block index for
	// paragraph/run targets, the 1-based table number for table targets.
	Index int `json:"index"`
	// BlockIndex is the resolved 1-based body block index of the mutated block.
	BlockIndex    int    `json:"blockIndex"`
	BlockKind     string `json:"blockKind"`
	Target        string `json:"target"`
	Style         string `json:"style"`
	PreviousStyle string `json:"previousStyle,omitempty"`
	ContentHash   string `json:"contentHash"`
	PreviousHash  string `json:"previousHash"`
	// ParaID is the durable w14:paraId marker of the styled paragraph for
	// paragraph/run targets (injected if absent). Empty for table targets.
	ParaID string `json:"paraId,omitempty"`
}

// ApplyParagraphStyle sets w:pStyle on the paragraph at the given index.
func ApplyParagraphStyle(req *ApplyParagraphStyleRequest) (*ApplyStyleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("apply paragraph style request is nil")
	}
	if req.StyleID == "" {
		return nil, fmt.Errorf("styleId is required")
	}
	if req.Validate {
		if err := validateStyleExists(req.Package, req.StylesURI, req.StyleID, "paragraph", "paragraph"); err != nil {
			return nil, err
		}
	}

	doc, paragraph, prefix, err := locateParagraph(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}
	block := docxbody.BodyBlock{Index: req.Index, Kind: model.BlockKindParagraph, Element: paragraph}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}

	previous := docxbody.ParagraphStyle(paragraph)
	setParagraphStyle(doc.Root(), prefix, paragraph, req.StyleID)
	paraID := stampMarkerForParagraph(doc.Root(), paragraph)
	ensureDocumentTableScaffolds(doc.Root(), prefix)
	newReport := extract.ReportBlock(block, false)

	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &ApplyStyleResult{
		Index:         req.Index,
		BlockIndex:    req.Index,
		BlockKind:     "paragraph",
		Target:        "paragraph",
		Style:         req.StyleID,
		PreviousStyle: previous,
		ContentHash:   newReport.ContentHash,
		PreviousHash:  report.ContentHash,
		ParaID:        paraID,
	}, nil
}

// ApplyRunStyle sets w:rStyle on all runs of the paragraph at the given index.
func ApplyRunStyle(req *ApplyRunStyleRequest) (*ApplyStyleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("apply run style request is nil")
	}
	if req.StyleID == "" {
		return nil, fmt.Errorf("styleId is required")
	}
	if req.Validate {
		if err := validateStyleExists(req.Package, req.StylesURI, req.StyleID, "character", "run"); err != nil {
			return nil, err
		}
	}

	doc, paragraph, prefix, err := locateParagraph(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}
	block := docxbody.BodyBlock{Index: req.Index, Kind: model.BlockKindParagraph, Element: paragraph}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}

	previous := docxbody.RunStyle(paragraph)
	setRunStyleForParagraph(doc.Root(), prefix, paragraph, req.StyleID)
	paraID := stampMarkerForParagraph(doc.Root(), paragraph)
	ensureDocumentTableScaffolds(doc.Root(), prefix)
	newReport := extract.ReportBlock(block, false)

	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &ApplyStyleResult{
		Index:         req.Index,
		BlockIndex:    req.Index,
		BlockKind:     "paragraph",
		Target:        "run",
		Style:         req.StyleID,
		PreviousStyle: previous,
		ContentHash:   newReport.ContentHash,
		PreviousHash:  report.ContentHash,
		ParaID:        paraID,
	}, nil
}

// ApplyTableStyle sets w:tblStyle on the table at the given 1-based table index.
func ApplyTableStyle(req *ApplyTableStyleRequest) (*ApplyStyleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("apply table style request is nil")
	}
	if req.StyleID == "" {
		return nil, fmt.Errorf("styleId is required")
	}
	if req.Validate {
		if err := validateStyleExists(req.Package, req.StylesURI, req.StyleID, "table", "table"); err != nil {
			return nil, err
		}
	}

	doc, block, prefix, err := locateTable(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	table := block.Element

	previous := docxbody.TableStyle(table)
	ensureTableScaffold(doc.Root(), prefix, table)
	setTableStyle(doc.Root(), prefix, table, req.StyleID)
	newReport := extract.ReportBlock(block, false)

	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &ApplyStyleResult{
		Index:         req.Index,
		BlockIndex:    block.Index,
		BlockKind:     "table",
		Target:        "table",
		Style:         req.StyleID,
		PreviousStyle: previous,
		ContentHash:   newReport.ContentHash,
		PreviousHash:  report.ContentHash,
	}, nil
}

// validateStyleExists confirms the styleId exists in styles.xml with the
// expected w:type. A missing styleId yields a StyleNotFoundError carrying
// candidate ids; a type mismatch yields a StyleTypeMismatchError.
func validateStyleExists(session opc.PackageSession, stylesURI, styleID, styleType, target string) error {
	if stylesURI == "" {
		return &StyleNotFoundError{StyleID: styleID, StyleType: styleType, Candidates: nil}
	}
	styles, err := docxinspect.ParseStyles(session, stylesURI)
	if err != nil {
		return fmt.Errorf("failed to parse styles part %s: %w", stylesURI, err)
	}
	style, ok := docxinspect.FindStyle(styles, styleID)
	if !ok {
		return &StyleNotFoundError{StyleID: styleID, StyleType: styleType, Candidates: suggestStyles(styles, styleType)}
	}
	if style.Type != styleType {
		return &StyleTypeMismatchError{StyleID: styleID, WantType: styleType, ActualType: style.Type, WantedTarget: target}
	}
	return nil
}

// suggestStyles returns the sorted styleIds of the given type.
func suggestStyles(styles []model.StyleInfo, styleType string) []string {
	out := make([]string, 0)
	for _, style := range styles {
		if style.Type == styleType {
			out = append(out, style.StyleID)
		}
	}
	sort.Strings(out)
	return out
}

func setParagraphStyle(root *etree.Element, prefix string, paragraph *etree.Element, styleID string) {
	pPr := namespaces.FindChild(paragraph, namespaces.NsW, "pPr")
	if pPr == nil {
		pPr = newElement(prefix, "pPr")
		paragraph.InsertChildAt(0, pPr)
	}
	setStyleChild(root, prefix, pPr, "pStyle", styleID)
}

func setRunStyleForParagraph(root *etree.Element, prefix string, paragraph *etree.Element, styleID string) {
	for _, run := range namespaces.FindChildren(paragraph, namespaces.NsW, "r") {
		rPr := namespaces.FindChild(run, namespaces.NsW, "rPr")
		if rPr == nil {
			rPr = newElement(prefix, "rPr")
			run.InsertChildAt(0, rPr)
		}
		setStyleChild(root, prefix, rPr, "rStyle", styleID)
	}
}

func setTableStyle(root *etree.Element, prefix string, table *etree.Element, styleID string) {
	tblPr := namespaces.FindChild(table, namespaces.NsW, "tblPr")
	if tblPr == nil {
		tblPr = newElement(prefix, "tblPr")
		table.InsertChildAt(0, tblPr)
	}
	setStyleChild(root, prefix, tblPr, "tblStyle", styleID)
}

func ensureDocumentTableScaffolds(root *etree.Element, prefix string) {
	body, err := docxbody.FindBody(root)
	if err != nil {
		return
	}
	for _, child := range body.ChildElements() {
		if docxbody.LocalName(child.Tag) == "tbl" {
			ensureTableScaffold(root, prefix, child)
		}
	}
}

// setStyleChild updates an existing style child's @w:val or creates one as the
// first child of the properties element, preserving WordprocessingML ordering.
func setStyleChild(root *etree.Element, prefix string, props *etree.Element, localName, styleID string) {
	attrName := qualifiedWordAttrName(root, prefix, "val")
	if existing := namespaces.FindChild(props, namespaces.NsW, localName); existing != nil {
		existing.CreateAttr(attrName, styleID)
		return
	}
	style := newElement(prefix, localName)
	style.CreateAttr(attrName, styleID)
	props.InsertChildAt(0, style)
}
