package mutate

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/styles"
)

const customNumberFormatStartID = 164

// NumberFormatOptions describes a practical cell/range number format request.
type NumberFormatOptions struct {
	Preset         string
	FormatCode     string
	Decimals       int
	CurrencySymbol string
}

// NumberFormatSpec is the normalized format applied to cellXfs.
type NumberFormatSpec struct {
	Preset     string `json:"preset,omitempty"`
	FormatCode string `json:"formatCode"`
	NumFmtID   int    `json:"numberFormatId,omitempty"`
	Builtin    bool   `json:"builtin"`
}

type SetRangeFormatRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	StylesURI   string
	SheetRef    model.SheetRef
	Range       address.RangeRef
	Format      NumberFormatSpec
}

type SetRangeFormatResult struct {
	Range         string `json:"range"`
	Updated       int    `json:"updated"`
	Created       int    `json:"created"`
	FormatCode    string `json:"formatCode"`
	NumFmtID      int    `json:"numberFormatId"`
	Builtin       bool   `json:"builtin"`
	StylesURI     string `json:"stylesPartUri"`
	StyleIndexes  []int  `json:"styleIndexes,omitempty"`
	CreatedStyles int    `json:"createdStyles"`
}

// ResolveNumberFormat normalizes agent-friendly presets into SpreadsheetML codes.
func ResolveNumberFormat(opts NumberFormatOptions) (NumberFormatSpec, error) {
	preset := strings.ToLower(strings.TrimSpace(opts.Preset))
	formatCode := strings.TrimSpace(opts.FormatCode)
	if (preset == "") == (formatCode == "") {
		return NumberFormatSpec{}, fmt.Errorf("specify exactly one of preset or format code")
	}
	if opts.Decimals < 0 || opts.Decimals > 10 {
		return NumberFormatSpec{}, fmt.Errorf("decimals must be between 0 and 10")
	}
	if formatCode != "" {
		return NumberFormatSpec{
			Preset:     "custom",
			FormatCode: formatCode,
			Builtin:    false,
		}, nil
	}

	switch preset {
	case "general":
		return builtinNumberFormatSpec(preset, 0)
	case "integer":
		return builtinNumberFormatSpec(preset, 3)
	case "number":
		code := fixedDecimalFormat("#,##0", opts.Decimals)
		switch opts.Decimals {
		case 0:
			return builtinNumberFormatSpec(preset, 3)
		case 2:
			return builtinNumberFormatSpec(preset, 4)
		default:
			return customNumberFormatSpec(preset, code), nil
		}
	case "percent":
		code := fixedDecimalFormat("0", opts.Decimals) + "%"
		switch opts.Decimals {
		case 0:
			return builtinNumberFormatSpec(preset, 9)
		case 2:
			return builtinNumberFormatSpec(preset, 10)
		default:
			return customNumberFormatSpec(preset, code), nil
		}
	case "currency":
		symbol := opts.CurrencySymbol
		if symbol == "" {
			symbol = "$"
		}
		code := formatLiteral(symbol) + fixedDecimalFormat("#,##0", opts.Decimals)
		return customNumberFormatSpec(preset, code), nil
	case "date":
		return customNumberFormatSpec(preset, "yyyy-mm-dd"), nil
	case "datetime":
		return customNumberFormatSpec(preset, "yyyy-mm-dd h:mm"), nil
	case "text":
		return builtinNumberFormatSpec(preset, 49)
	default:
		return NumberFormatSpec{}, fmt.Errorf("invalid preset %q (must be integer, number, currency, percent, date, datetime, text, or general)", opts.Preset)
	}
}

func builtinNumberFormatSpec(preset string, id int) (NumberFormatSpec, error) {
	code, ok := styles.BuiltinNumberFormatCode(id)
	if !ok {
		return NumberFormatSpec{}, fmt.Errorf("unknown built-in number format id %d", id)
	}
	return NumberFormatSpec{
		Preset:     preset,
		FormatCode: code,
		NumFmtID:   id,
		Builtin:    true,
	}, nil
}

func customNumberFormatSpec(preset, code string) NumberFormatSpec {
	return NumberFormatSpec{
		Preset:     preset,
		FormatCode: code,
		Builtin:    false,
	}
}

func fixedDecimalFormat(base string, decimals int) string {
	if decimals == 0 {
		return base
	}
	return base + "." + strings.Repeat("0", decimals)
}

func formatLiteral(value string) string {
	return `"` + strings.ReplaceAll(value, `"`, `""`) + `"`
}

func SetRangeNumberFormat(req *SetRangeFormatRequest) (*SetRangeFormatResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set range format request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.WorkbookURI == "" {
		return nil, fmt.Errorf("workbook URI cannot be empty")
	}
	if req.SheetRef.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", req.SheetRef.Name)
	}
	if strings.TrimSpace(req.Format.FormatCode) == "" {
		return nil, fmt.Errorf("format code cannot be empty")
	}

	stylesURI, stylesDoc, err := loadOrCreateStylesDocument(req.Package, req.WorkbookURI, req.StylesURI)
	if err != nil {
		return nil, err
	}
	editor, err := newNumberStyleEditor(stylesDoc)
	if err != nil {
		return nil, err
	}
	numFmtID, err := editor.ensureNumberFormat(req.Format)
	if err != nil {
		return nil, err
	}

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
	result := &SetRangeFormatResult{
		Range:      req.Range.String(),
		FormatCode: req.Format.FormatCode,
		NumFmtID:   numFmtID,
		Builtin:    req.Format.Builtin,
		StylesURI:  stylesURI,
	}
	styleSeen := map[int]struct{}{}
	styleByBase := map[int]int{}
	for rowNum := minRow; rowNum <= maxRow; rowNum++ {
		row := ensureRow(sheetData, prefix, rowNum)
		for colNum := minCol; colNum <= maxCol; colNum++ {
			refText := cellRef(colNum, rowNum)
			cell, created := ensureCell(row, prefix, refText, colNum)
			baseStyle := cellStyleIndex(cell)
			styleIndex, ok := styleByBase[baseStyle]
			if !ok {
				styleIndex, err = editor.ensureCellStyle(baseStyle, numFmtID)
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

	if err := writeStylesDocument(req.Package, stylesURI, stylesDoc); err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, worksheetDoc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	result.CreatedStyles = editor.createdCellStyles
	result.StyleIndexes = sortedStyleIndexes(styleSeen)
	return result, nil
}

func loadOrCreateStylesDocument(session opc.PackageSession, workbookURI, stylesURI string) (string, *etree.Document, error) {
	if stylesURI == "" {
		stylesURI = "/xl/styles.xml"
	}
	stylesURI = opc.NormalizeURI(stylesURI)
	if err := ensureWorkbookStylesRelationship(session, workbookURI, stylesURI); err != nil {
		return "", nil, err
	}
	if packagePartExists(session, stylesURI) {
		doc, err := session.ReadXMLPart(stylesURI)
		if err != nil {
			return "", nil, fmt.Errorf("failed to read styles %s: %w", stylesURI, err)
		}
		return stylesURI, doc, nil
	}
	return stylesURI, defaultStylesDocument(), nil
}

func ensureWorkbookStylesRelationship(session opc.PackageSession, workbookURI, stylesURI string) error {
	rels := session.ListRelationships(workbookURI)
	for _, rel := range rels {
		if rel.TargetMode == "External" || rel.Type != namespaces.RelStyles {
			continue
		}
		if relationshipTargetURI(workbookURI, rel.Target) == stylesURI {
			return nil
		}
	}
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: workbookURI,
		ID:        opc.AllocateRelationshipID(rels),
		Type:      namespaces.RelStyles,
		Target:    opc.RelationshipTarget(workbookURI, stylesURI),
	})
	if err := opc.WriteRelationships(session, workbookURI, rels); err != nil {
		return fmt.Errorf("failed to write workbook relationships: %w", err)
	}
	return nil
}

func relationshipTargetURI(sourceURI, target string) string {
	if strings.HasPrefix(target, "/") {
		return opc.NormalizeURI(target)
	}
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, target))
}

func packagePartExists(session opc.PackageSession, uri string) bool {
	uri = opc.NormalizeURI(uri)
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return true
		}
	}
	return false
}

func defaultStylesDocument() *etree.Document {
	doc := etree.NewDocument()
	root := etree.NewElement("styleSheet")
	root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	doc.SetRoot(root)
	editor, _ := newNumberStyleEditor(doc)
	editor.refreshCounts()
	return doc
}

func writeStylesDocument(session opc.PackageSession, stylesURI string, doc *etree.Document) error {
	data, err := doc.WriteToBytes()
	if err != nil {
		return fmt.Errorf("failed to serialize styles: %w", err)
	}
	if err := session.ReplaceRawPart(stylesURI, data, namespaces.ContentTypeStyles); err != nil {
		return fmt.Errorf("failed to replace styles %s: %w", stylesURI, err)
	}
	return nil
}

type numberStyleEditor struct {
	doc               *etree.Document
	root              *etree.Element
	prefix            string
	numFmts           *etree.Element
	cellXfs           *etree.Element
	createdCellStyles int
}

func newNumberStyleEditor(doc *etree.Document) (*numberStyleEditor, error) {
	if doc == nil {
		return nil, fmt.Errorf("styles document is nil")
	}
	root := doc.Root()
	if root == nil || root.Tag != "styleSheet" {
		return nil, fmt.Errorf("styles root element not found")
	}
	editor := &numberStyleEditor{
		doc:    doc,
		root:   root,
		prefix: root.Space,
	}
	editor.ensureDefaults()
	editor.cellXfs = ensureStylesheetChild(root, editor.prefix, "cellXfs")
	editor.numFmts = namespaces.FindChild(root, namespaces.NsSpreadsheetML, "numFmts")
	editor.refreshCounts()
	return editor, nil
}

func (e *numberStyleEditor) ensureDefaults() {
	fonts := ensureStylesheetChild(e.root, e.prefix, "fonts")
	if len(namespaces.FindChildren(fonts, namespaces.NsSpreadsheetML, "font")) == 0 {
		fonts.AddChild(newElement(e.prefix, "font"))
	}

	fills := ensureStylesheetChild(e.root, e.prefix, "fills")
	for len(namespaces.FindChildren(fills, namespaces.NsSpreadsheetML, "fill")) < 2 {
		fill := newElement(e.prefix, "fill")
		patternFill := newElement(e.prefix, "patternFill")
		if len(namespaces.FindChildren(fills, namespaces.NsSpreadsheetML, "fill")) == 0 {
			patternFill.CreateAttr("patternType", "none")
		} else {
			patternFill.CreateAttr("patternType", "gray125")
		}
		fill.AddChild(patternFill)
		fills.AddChild(fill)
	}

	borders := ensureStylesheetChild(e.root, e.prefix, "borders")
	if len(namespaces.FindChildren(borders, namespaces.NsSpreadsheetML, "border")) == 0 {
		borders.AddChild(newElement(e.prefix, "border"))
	}

	cellStyleXfs := ensureStylesheetChild(e.root, e.prefix, "cellStyleXfs")
	if len(namespaces.FindChildren(cellStyleXfs, namespaces.NsSpreadsheetML, "xf")) == 0 {
		cellStyleXfs.AddChild(defaultStyleXF(e.prefix))
	}

	cellXfs := ensureStylesheetChild(e.root, e.prefix, "cellXfs")
	if len(namespaces.FindChildren(cellXfs, namespaces.NsSpreadsheetML, "xf")) == 0 {
		cellXfs.AddChild(defaultCellXF(e.prefix))
	}

	cellStyles := ensureStylesheetChild(e.root, e.prefix, "cellStyles")
	if len(namespaces.FindChildren(cellStyles, namespaces.NsSpreadsheetML, "cellStyle")) == 0 {
		cellStyle := newElement(e.prefix, "cellStyle")
		cellStyle.CreateAttr("name", "Normal")
		cellStyle.CreateAttr("xfId", "0")
		cellStyle.CreateAttr("builtinId", "0")
		cellStyles.AddChild(cellStyle)
	}
}

func (e *numberStyleEditor) ensureNumberFormat(spec NumberFormatSpec) (int, error) {
	if spec.Builtin {
		return spec.NumFmtID, nil
	}
	formatCode := strings.TrimSpace(spec.FormatCode)
	if formatCode == "" {
		return 0, fmt.Errorf("custom format code cannot be empty")
	}
	numFmts := ensureStylesheetChild(e.root, e.prefix, "numFmts")
	e.numFmts = numFmts
	for _, numFmt := range namespaces.FindChildren(numFmts, namespaces.NsSpreadsheetML, "numFmt") {
		if numFmt.SelectAttrValue("formatCode", "") == formatCode {
			id, err := strconv.Atoi(numFmt.SelectAttrValue("numFmtId", ""))
			if err != nil {
				return 0, fmt.Errorf("invalid numFmtId %q", numFmt.SelectAttrValue("numFmtId", ""))
			}
			e.refreshCounts()
			return id, nil
		}
	}

	nextID := customNumberFormatStartID
	used := map[int]struct{}{}
	for _, numFmt := range namespaces.FindChildren(numFmts, namespaces.NsSpreadsheetML, "numFmt") {
		id, err := strconv.Atoi(numFmt.SelectAttrValue("numFmtId", ""))
		if err != nil {
			continue
		}
		used[id] = struct{}{}
		if id >= nextID {
			nextID = id + 1
		}
	}
	for {
		if _, exists := used[nextID]; !exists {
			break
		}
		nextID++
	}

	numFmt := newElement(e.prefix, "numFmt")
	numFmt.CreateAttr("numFmtId", strconv.Itoa(nextID))
	numFmt.CreateAttr("formatCode", formatCode)
	numFmts.AddChild(numFmt)
	e.refreshCounts()
	return nextID, nil
}

func (e *numberStyleEditor) ensureCellStyle(baseStyleIndex, numFmtID int) (int, error) {
	xfs := namespaces.FindChildren(e.cellXfs, namespaces.NsSpreadsheetML, "xf")
	if baseStyleIndex < 0 || baseStyleIndex >= len(xfs) {
		baseStyleIndex = 0
	}
	base := xfs[baseStyleIndex]
	if cellXFNumFmtID(base) == numFmtID {
		return baseStyleIndex, nil
	}

	candidate := cloneCellXFWithNumFmt(base, e.prefix, numFmtID)
	candidateSignature := elementSignature(candidate)
	for idx, xf := range xfs {
		if elementSignature(xf) == candidateSignature {
			return idx, nil
		}
	}

	e.cellXfs.AddChild(candidate)
	e.createdCellStyles++
	e.refreshCounts()
	return len(xfs), nil
}

func (e *numberStyleEditor) refreshCounts() {
	for _, localName := range []string{"numFmts", "fonts", "fills", "borders", "cellStyleXfs", "cellXfs", "cellStyles"} {
		elem := namespaces.FindChild(e.root, namespaces.NsSpreadsheetML, localName)
		if elem == nil {
			continue
		}
		childCount := 0
		for _, child := range elem.ChildElements() {
			if child.NamespaceURI() == namespaces.NsSpreadsheetML || child.NamespaceURI() == "" {
				childCount++
			}
		}
		elem.CreateAttr("count", strconv.Itoa(childCount))
	}
}

func ensureStylesheetChild(root *etree.Element, prefix, localName string) *etree.Element {
	if child := namespaces.FindChild(root, namespaces.NsSpreadsheetML, localName); child != nil {
		return child
	}
	child := newElement(prefix, localName)
	insertStylesheetChild(root, child, localName)
	return child
}

func insertStylesheetChild(root, child *etree.Element, localName string) {
	targetOrder := stylesheetChildOrder(localName)
	for _, existing := range root.ChildElements() {
		if stylesheetChildOrder(existing.Tag) > targetOrder {
			root.InsertChildAt(existing.Index(), child)
			return
		}
	}
	root.AddChild(child)
}

func stylesheetChildOrder(localName string) int {
	switch localName {
	case "numFmts":
		return 10
	case "fonts":
		return 20
	case "fills":
		return 30
	case "borders":
		return 40
	case "cellStyleXfs":
		return 50
	case "cellXfs":
		return 60
	case "cellStyles":
		return 70
	case "dxfs":
		return 80
	case "tableStyles":
		return 90
	case "colors":
		return 100
	case "extLst":
		return 110
	default:
		return 1000
	}
}

func defaultStyleXF(prefix string) *etree.Element {
	xf := newElement(prefix, "xf")
	xf.CreateAttr("numFmtId", "0")
	xf.CreateAttr("fontId", "0")
	xf.CreateAttr("fillId", "0")
	xf.CreateAttr("borderId", "0")
	return xf
}

func defaultCellXF(prefix string) *etree.Element {
	xf := defaultStyleXF(prefix)
	xf.CreateAttr("xfId", "0")
	return xf
}

func cloneCellXFWithNumFmt(base *etree.Element, prefix string, numFmtID int) *etree.Element {
	var clone *etree.Element
	if base == nil {
		clone = defaultCellXF(prefix)
	} else {
		clone = base.Copy()
	}
	if clone.Space == "" {
		clone.Space = prefix
	}
	ensureAttr(clone, "fontId", "0")
	ensureAttr(clone, "fillId", "0")
	ensureAttr(clone, "borderId", "0")
	ensureAttr(clone, "xfId", "0")
	clone.CreateAttr("numFmtId", strconv.Itoa(numFmtID))
	clone.CreateAttr("applyNumberFormat", "1")
	return clone
}

func ensureAttr(elem *etree.Element, key, value string) {
	if elem.SelectAttr(key) == nil {
		elem.CreateAttr(key, value)
	}
}

func cellXFNumFmtID(xf *etree.Element) int {
	if xf == nil {
		return 0
	}
	id, err := strconv.Atoi(xf.SelectAttrValue("numFmtId", "0"))
	if err != nil || id < 0 {
		return 0
	}
	return id
}

func cellStyleIndex(cell *etree.Element) int {
	if cell == nil {
		return 0
	}
	styleIndex, err := strconv.Atoi(cell.SelectAttrValue("s", "0"))
	if err != nil || styleIndex < 0 {
		return 0
	}
	return styleIndex
}

func elementSignature(elem *etree.Element) string {
	clone := elem.Copy()
	sortAttrsRecursive(clone)
	doc := etree.NewDocumentWithRoot(clone)
	data, err := doc.WriteToBytes()
	if err != nil {
		return ""
	}
	return string(data)
}

func sortAttrsRecursive(elem *etree.Element) {
	elem.SortAttrs()
	for _, child := range elem.ChildElements() {
		sortAttrsRecursive(child)
	}
}

func sortedStyleIndexes(seen map[int]struct{}) []int {
	indexes := make([]int, 0, len(seen))
	for index := range seen {
		indexes = append(indexes, index)
	}
	sort.Ints(indexes)
	return indexes
}
