package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// FontSpec describes requested font changes. Pointers/flags distinguish
// "unspecified" (leave the cell's existing value) from an explicit value.
type FontSpec struct {
	Name      string
	HasName   bool
	Size      float64
	HasSize   bool
	Bold      *bool
	Italic    *bool
	Underline *bool
	Color     string
	HasColor  bool
}

// FillSpec describes a solid cell fill.
type FillSpec struct {
	Color string
}

// BorderSpec describes border edges to apply.
type BorderSpec struct {
	Style  string
	Color  string
	Top    bool
	Bottom bool
	Left   bool
	Right  bool
}

// AlignmentSpec describes cell alignment.
type AlignmentSpec struct {
	Horizontal string
	HasH       bool
	Vertical   string
	HasV       bool
	WrapText   *bool
}

// CellStyleSpec is a composite style request. Nil sub-specs are left unchanged.
type CellStyleSpec struct {
	Font      *FontSpec
	Fill      *FillSpec
	Border    *BorderSpec
	Alignment *AlignmentSpec
}

// SetRangeStyleRequest applies a visual style across a worksheet range.
type SetRangeStyleRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	StylesURI   string
	SheetRef    model.SheetRef
	Range       address.RangeRef
	Style       CellStyleSpec
}

// SetRangeStyleResult reports a style mutation outcome.
type SetRangeStyleResult struct {
	Range         string `json:"range"`
	Updated       int    `json:"updated"`
	Created       int    `json:"created"`
	StyleIndexes  []int  `json:"styleIndexes,omitempty"`
	CreatedStyles int    `json:"createdStyles"`
	StylesURI     string `json:"stylesPartUri"`
}

var validBorderStyles = map[string]bool{
	"thin": true, "medium": true, "thick": true, "double": true,
	"dotted": true, "dashed": true, "hair": true, "dashDot": true,
	"dashDotDot": true, "mediumDashed": true, "none": true,
}
var validHAlign = map[string]bool{"left": true, "center": true, "right": true, "fill": true, "justify": true, "centerContinuous": true, "distributed": true, "general": true}
var validVAlign = map[string]bool{"top": true, "center": true, "bottom": true, "justify": true, "distributed": true}

// NormalizeColor converts #RGB/RRGGBB/AARRGGBB to an 8-digit ARGB hex string.
func NormalizeColor(value string) (string, error) {
	v := strings.TrimSpace(value)
	v = strings.TrimPrefix(v, "#")
	v = strings.ToUpper(v)
	for _, r := range v {
		if !((r >= '0' && r <= '9') || (r >= 'A' && r <= 'F')) {
			return "", fmt.Errorf("invalid color %q (use hex like #1A2B3C)", value)
		}
	}
	switch len(v) {
	case 6:
		return "FF" + v, nil
	case 8:
		return v, nil
	default:
		return "", fmt.Errorf("invalid color %q (expected 6 or 8 hex digits)", value)
	}
}

// ValidateCellStyleSpec checks enumerations and colors up front.
func ValidateCellStyleSpec(spec *CellStyleSpec) error {
	if spec.Font != nil {
		if spec.Font.HasSize && (spec.Font.Size <= 0 || spec.Font.Size > 409) {
			return fmt.Errorf("font size %g out of range (1-409)", spec.Font.Size)
		}
		if spec.Font.HasColor {
			if _, err := NormalizeColor(spec.Font.Color); err != nil {
				return err
			}
		}
	}
	if spec.Fill != nil {
		if _, err := NormalizeColor(spec.Fill.Color); err != nil {
			return err
		}
	}
	if spec.Border != nil {
		if spec.Border.Style != "" && !validBorderStyles[spec.Border.Style] {
			return fmt.Errorf("invalid border style %q", spec.Border.Style)
		}
		if spec.Border.Color != "" {
			if _, err := NormalizeColor(spec.Border.Color); err != nil {
				return err
			}
		}
	}
	if spec.Alignment != nil {
		if spec.Alignment.HasH && !validHAlign[spec.Alignment.Horizontal] {
			return fmt.Errorf("invalid horizontal alignment %q", spec.Alignment.Horizontal)
		}
		if spec.Alignment.HasV && !validVAlign[spec.Alignment.Vertical] {
			return fmt.Errorf("invalid vertical alignment %q", spec.Alignment.Vertical)
		}
	}
	return nil
}

// SetRangeStyle applies the composite style to all cells in the range.
func SetRangeStyle(req *SetRangeStyleRequest) (*SetRangeStyleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set range style request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SheetRef.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", req.SheetRef.Name)
	}
	if err := ValidateCellStyleSpec(&req.Style); err != nil {
		return nil, err
	}

	stylesURI, stylesDoc, err := loadOrCreateStylesDocument(req.Package, req.WorkbookURI, req.StylesURI)
	if err != nil {
		return nil, err
	}
	editor, err := newNumberStyleEditor(stylesDoc) // ensures default fonts/fills/borders/cellXfs
	if err != nil {
		return nil, err
	}
	se := &styleApplier{root: editor.root, prefix: editor.root.Space}

	worksheetDoc, err := req.Package.ReadXMLPart(req.SheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	root := worksheetDoc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", req.SheetRef.PartURI)
	}

	minCol, minRow, maxCol, maxRow := req.Range.Bounds()
	prefix := root.Space
	sheetData := ensureSheetData(root, prefix)
	result := &SetRangeStyleResult{Range: req.Range.String(), StylesURI: stylesURI}
	styleByBase := map[int]int{}
	styleSeen := map[int]struct{}{}
	for rowNum := minRow; rowNum <= maxRow; rowNum++ {
		row := ensureRow(sheetData, prefix, rowNum)
		for colNum := minCol; colNum <= maxCol; colNum++ {
			refText := cellRef(colNum, rowNum)
			cell, created := ensureCell(row, prefix, refText, colNum)
			baseStyle := cellStyleIndex(cell)
			styleIndex, ok := styleByBase[baseStyle]
			if !ok {
				styleIndex, err = se.applyStyle(baseStyle, &req.Style)
				if err != nil {
					return nil, err
				}
				styleByBase[baseStyle] = styleIndex
			}
			cell.CreateAttr("s", strconv.Itoa(styleIndex))
			row.RemoveAttr("spans")
			result.Updated++
			if created {
				result.Created++
			}
			styleSeen[styleIndex] = struct{}{}
		}
	}
	updateDimension(root, prefix)

	editor.refreshCounts()
	if err := writeStylesDocument(req.Package, stylesURI, stylesDoc); err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, worksheetDoc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	result.CreatedStyles = se.createdStyles
	result.StyleIndexes = sortedStyleIndexes(styleSeen)
	return result, nil
}

// styleApplier mutates a styles.xml document's shared collections.
type styleApplier struct {
	root          *etree.Element
	prefix        string
	createdStyles int
}

func (s *styleApplier) collection(name string) *etree.Element {
	return ensureStylesheetChild(s.root, s.prefix, name)
}

func (s *styleApplier) cellXfs() *etree.Element { return s.collection("cellXfs") }

// applyStyle derives a new cell xf from baseStyleIndex with the spec applied,
// dedups it, and returns its index.
func (s *styleApplier) applyStyle(baseStyleIndex int, spec *CellStyleSpec) (int, error) {
	xfs := namespaces.FindChildren(s.cellXfs(), namespaces.NsSpreadsheetML, "xf")
	if baseStyleIndex < 0 || baseStyleIndex >= len(xfs) {
		baseStyleIndex = 0
	}
	var base *etree.Element
	if baseStyleIndex < len(xfs) {
		base = xfs[baseStyleIndex]
	}
	candidate := s.cloneXf(base)

	if spec.Font != nil {
		baseFontID := attrInt(candidate, "fontId", 0)
		fontID, err := s.ensureFont(baseFontID, spec.Font)
		if err != nil {
			return 0, err
		}
		candidate.CreateAttr("fontId", strconv.Itoa(fontID))
		candidate.CreateAttr("applyFont", "1")
	}
	if spec.Fill != nil {
		fillID, err := s.ensureSolidFill(spec.Fill.Color)
		if err != nil {
			return 0, err
		}
		candidate.CreateAttr("fillId", strconv.Itoa(fillID))
		candidate.CreateAttr("applyFill", "1")
	}
	if spec.Border != nil {
		baseBorderID := attrInt(candidate, "borderId", 0)
		borderID, err := s.ensureBorder(baseBorderID, spec.Border)
		if err != nil {
			return 0, err
		}
		candidate.CreateAttr("borderId", strconv.Itoa(borderID))
		candidate.CreateAttr("applyBorder", "1")
	}
	if spec.Alignment != nil {
		s.applyAlignment(candidate, spec.Alignment)
		candidate.CreateAttr("applyAlignment", "1")
	}

	// Dedup against existing cellXfs.
	candidateSig := elementSignature(candidate)
	for idx, xf := range xfs {
		if elementSignature(xf) == candidateSig {
			return idx, nil
		}
	}
	s.cellXfs().AddChild(candidate)
	s.createdStyles++
	return len(xfs), nil
}

func (s *styleApplier) cloneXf(base *etree.Element) *etree.Element {
	var clone *etree.Element
	if base == nil {
		clone = defaultCellXF(s.prefix)
	} else {
		clone = base.Copy()
	}
	if clone.Space == "" {
		clone.Space = s.prefix
	}
	ensureAttr(clone, "numFmtId", "0")
	ensureAttr(clone, "fontId", "0")
	ensureAttr(clone, "fillId", "0")
	ensureAttr(clone, "borderId", "0")
	ensureAttr(clone, "xfId", "0")
	return clone
}

// ensureFont clones the base font, applies overrides, dedups, returns index.
func (s *styleApplier) ensureFont(baseFontID int, spec *FontSpec) (int, error) {
	fonts := s.collection("fonts")
	fontEls := namespaces.FindChildren(fonts, namespaces.NsSpreadsheetML, "font")
	var base *etree.Element
	if baseFontID >= 0 && baseFontID < len(fontEls) {
		base = fontEls[baseFontID]
	}
	var candidate *etree.Element
	if base != nil {
		candidate = base.Copy()
	} else {
		candidate = newElement(s.prefix, "font")
	}
	if spec.Bold != nil {
		s.setBoolChild(candidate, "b", *spec.Bold)
	}
	if spec.Italic != nil {
		s.setBoolChild(candidate, "i", *spec.Italic)
	}
	if spec.Underline != nil {
		s.setBoolChild(candidate, "u", *spec.Underline)
	}
	if spec.HasSize {
		s.setValChild(candidate, "sz", strconv.FormatFloat(spec.Size, 'f', -1, 64))
	}
	if spec.HasColor {
		color, err := NormalizeColor(spec.Color)
		if err != nil {
			return 0, err
		}
		s.setColorChild(candidate, color)
	}
	if spec.HasName {
		s.setValChild(candidate, "name", spec.Name)
	}
	candidateSig := elementSignature(candidate)
	for idx, f := range fontEls {
		if elementSignature(f) == candidateSig {
			return idx, nil
		}
	}
	fonts.AddChild(candidate)
	return len(fontEls), nil
}

func (s *styleApplier) ensureSolidFill(color string) (int, error) {
	rgb, err := NormalizeColor(color)
	if err != nil {
		return 0, err
	}
	fills := s.collection("fills")
	fillEls := namespaces.FindChildren(fills, namespaces.NsSpreadsheetML, "fill")
	candidate := newElement(s.prefix, "fill")
	pattern := newElement(s.prefix, "patternFill")
	pattern.CreateAttr("patternType", "solid")
	fg := newElement(s.prefix, "fgColor")
	fg.CreateAttr("rgb", rgb)
	pattern.AddChild(fg)
	bg := newElement(s.prefix, "bgColor")
	bg.CreateAttr("indexed", "64")
	pattern.AddChild(bg)
	candidate.AddChild(pattern)
	candidateSig := elementSignature(candidate)
	for idx, f := range fillEls {
		if elementSignature(f) == candidateSig {
			return idx, nil
		}
	}
	fills.AddChild(candidate)
	return len(fillEls), nil
}

func (s *styleApplier) ensureBorder(baseBorderID int, spec *BorderSpec) (int, error) {
	borders := s.collection("borders")
	borderEls := namespaces.FindChildren(borders, namespaces.NsSpreadsheetML, "border")
	var base *etree.Element
	if baseBorderID >= 0 && baseBorderID < len(borderEls) {
		base = borderEls[baseBorderID]
	}
	var candidate *etree.Element
	if base != nil {
		candidate = base.Copy()
	} else {
		candidate = newElement(s.prefix, "border")
	}
	var color string
	if spec.Color != "" {
		c, err := NormalizeColor(spec.Color)
		if err != nil {
			return 0, err
		}
		color = c
	}
	edges := []struct {
		name string
		set  bool
	}{
		{"left", spec.Left}, {"right", spec.Right}, {"top", spec.Top}, {"bottom", spec.Bottom},
	}
	for _, e := range edges {
		if !e.set {
			continue
		}
		s.setBorderEdge(candidate, e.name, spec.Style, color)
	}
	candidateSig := elementSignature(candidate)
	for idx, b := range borderEls {
		if elementSignature(b) == candidateSig {
			return idx, nil
		}
	}
	borders.AddChild(candidate)
	return len(borderEls), nil
}

func (s *styleApplier) applyAlignment(xf *etree.Element, spec *AlignmentSpec) {
	align := namespaces.FindChild(xf, namespaces.NsSpreadsheetML, "alignment")
	if align == nil {
		align = newElement(s.prefix, "alignment")
		// alignment precedes extLst within CT_Xf; append is fine for our xfs.
		xf.AddChild(align)
	}
	if spec.HasH {
		align.CreateAttr("horizontal", spec.Horizontal)
	}
	if spec.HasV {
		align.CreateAttr("vertical", spec.Vertical)
	}
	if spec.WrapText != nil {
		if *spec.WrapText {
			align.CreateAttr("wrapText", "1")
		} else {
			align.RemoveAttr("wrapText")
		}
	}
}

// ---- font/border child helpers (schema-ordered) ----

func fontChildOrder(name string) int {
	switch name {
	case "b":
		return 10
	case "i":
		return 20
	case "u":
		return 30
	case "strike":
		return 40
	case "sz":
		return 50
	case "color":
		return 60
	case "name":
		return 70
	case "family":
		return 80
	case "scheme":
		return 90
	default:
		return 1000
	}
}

func (s *styleApplier) insertFontChild(font, child *etree.Element, name string) {
	order := fontChildOrder(name)
	for _, existing := range font.ChildElements() {
		if fontChildOrder(existing.Tag) > order {
			font.InsertChildAt(existing.Index(), child)
			return
		}
	}
	font.AddChild(child)
}

func (s *styleApplier) setBoolChild(font *etree.Element, name string, on bool) {
	existing := namespaces.FindChild(font, namespaces.NsSpreadsheetML, name)
	if !on {
		if existing != nil {
			font.RemoveChild(existing)
		}
		return
	}
	if existing == nil {
		s.insertFontChild(font, newElement(s.prefix, name), name)
	}
}

func (s *styleApplier) setValChild(font *etree.Element, name, val string) {
	child := namespaces.FindChild(font, namespaces.NsSpreadsheetML, name)
	if child == nil {
		child = newElement(s.prefix, name)
		s.insertFontChild(font, child, name)
	}
	child.CreateAttr("val", val)
}

func (s *styleApplier) setColorChild(font *etree.Element, rgb string) {
	child := namespaces.FindChild(font, namespaces.NsSpreadsheetML, "color")
	if child == nil {
		child = newElement(s.prefix, "color")
		s.insertFontChild(font, child, "color")
	}
	child.RemoveAttr("theme")
	child.RemoveAttr("indexed")
	child.RemoveAttr("auto")
	child.CreateAttr("rgb", rgb)
}

func borderEdgeOrder(name string) int {
	switch name {
	case "start", "left":
		return 10
	case "end", "right":
		return 20
	case "top":
		return 30
	case "bottom":
		return 40
	case "diagonal":
		return 50
	default:
		return 1000
	}
}

func (s *styleApplier) setBorderEdge(border *etree.Element, name, style, color string) {
	edge := namespaces.FindChild(border, namespaces.NsSpreadsheetML, name)
	if edge == nil {
		edge = newElement(s.prefix, name)
		order := borderEdgeOrder(name)
		inserted := false
		for _, existing := range border.ChildElements() {
			if borderEdgeOrder(existing.Tag) > order {
				border.InsertChildAt(existing.Index(), edge)
				inserted = true
				break
			}
		}
		if !inserted {
			border.AddChild(edge)
		}
	}
	// reset edge content
	for _, c := range edge.ChildElements() {
		edge.RemoveChild(c)
	}
	if style == "" || style == "none" {
		edge.RemoveAttr("style")
		return
	}
	edge.CreateAttr("style", style)
	if color != "" {
		c := newElement(s.prefix, "color")
		c.CreateAttr("rgb", color)
		edge.AddChild(c)
	}
}

func attrInt(elem *etree.Element, key string, def int) int {
	if elem == nil {
		return def
	}
	v, err := strconv.Atoi(elem.SelectAttrValue(key, ""))
	if err != nil {
		return def
	}
	return v
}
