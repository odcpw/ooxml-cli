package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

type SetTableCellTextRequest struct {
	Package     opc.PackageSession
	SlideRef    *inspect.SlideRef
	TableID     int
	RowIndex    int
	ColumnIndex int
	Text        string
}

type SetTableCellTextResult struct {
	TableID      int    `json:"tableId"`
	RowIndex     int    `json:"rowIndex"`
	ColumnIndex  int    `json:"columnIndex"`
	Text         string `json:"text"`
	PreviousText string `json:"previousText"`
}

type SetTableTextMatrixRequest struct {
	Package  opc.PackageSession
	SlideRef *inspect.SlideRef
	TableID  int
	Data     [][]string
}

type SetTableTextMatrixResult struct {
	TableID      int `json:"tableId"`
	Rows         int `json:"rows"`
	Cols         int `json:"cols"`
	UpdatedCells int `json:"updatedCells"`
	ChangedCells int `json:"changedCells"`
}

func SetTableCellText(req *SetTableCellTextRequest) (*SetTableCellTextResult, error) {
	if req == nil || req.Package == nil || req.SlideRef == nil || req.TableID <= 0 {
		return nil, fmt.Errorf("invalid request")
	}
	if req.RowIndex < 0 {
		return nil, fmt.Errorf("row index cannot be negative")
	}
	if req.ColumnIndex < 0 {
		return nil, fmt.Errorf("column index cannot be negative")
	}

	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	tblElem, err := findTableInSlide(slideDoc.Root(), req.TableID)
	if err != nil {
		return nil, err
	}
	if tblElem == nil {
		return nil, fmt.Errorf("table with ID %d not found", req.TableID)
	}

	rows := tableRows(tblElem)
	if req.RowIndex >= len(rows) {
		return nil, fmt.Errorf("row index out of range")
	}
	cells := tableRowCells(rows[req.RowIndex])
	if req.ColumnIndex >= len(cells) {
		return nil, fmt.Errorf("column index out of range")
	}

	cell := cells[req.ColumnIndex]
	previousText := tableCellText(cell)
	if err := replaceTableCellText(cell, drawingPrefix(tblElem), req.Text); err != nil {
		return nil, err
	}

	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &SetTableCellTextResult{
		TableID:      req.TableID,
		RowIndex:     req.RowIndex,
		ColumnIndex:  req.ColumnIndex,
		Text:         req.Text,
		PreviousText: previousText,
	}, nil
}

func SetTableTextMatrix(req *SetTableTextMatrixRequest) (*SetTableTextMatrixResult, error) {
	if req == nil || req.Package == nil || req.SlideRef == nil || req.TableID <= 0 {
		return nil, fmt.Errorf("invalid request")
	}
	rowsCount, colsCount, err := validateTextMatrix(req.Data)
	if err != nil {
		return nil, err
	}

	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	tblElem, err := findTableInSlide(slideDoc.Root(), req.TableID)
	if err != nil {
		return nil, err
	}
	if tblElem == nil {
		return nil, fmt.Errorf("table with ID %d not found", req.TableID)
	}
	if tableHasMergedCells(tblElem) {
		return nil, fmt.Errorf("cannot update table with merged cells")
	}

	rows := tableRows(tblElem)
	destCols := 0
	if len(rows) > 0 {
		destCols = len(tableRowCells(rows[0]))
	}
	if len(rows) != rowsCount {
		return nil, fmt.Errorf("source matrix dimension mismatch: destination table is %dx%d, source is %dx%d", len(rows), destCols, rowsCount, colsCount)
	}

	prefix := drawingPrefix(tblElem)
	var changedCells int
	for rowIndex, row := range rows {
		cells := tableRowCells(row)
		if len(cells) != colsCount {
			return nil, fmt.Errorf("source matrix dimension mismatch: destination table row %d has %d cells, source has %d columns", rowIndex+1, len(cells), colsCount)
		}
		for colIndex, cell := range cells {
			nextText := req.Data[rowIndex][colIndex]
			if tableCellText(cell) != nextText {
				changedCells++
			}
			if err := replaceTableCellText(cell, prefix, nextText); err != nil {
				return nil, err
			}
		}
	}

	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &SetTableTextMatrixResult{
		TableID:      req.TableID,
		Rows:         rowsCount,
		Cols:         colsCount,
		UpdatedCells: rowsCount * colsCount,
		ChangedCells: changedCells,
	}, nil
}

func validateTextMatrix(data [][]string) (rows, cols int, err error) {
	if len(data) == 0 {
		return 0, 0, fmt.Errorf("source matrix is empty")
	}
	cols = len(data[0])
	if cols == 0 {
		return 0, 0, fmt.Errorf("source matrix is empty")
	}
	for rowIndex, row := range data {
		if len(row) != cols {
			return 0, 0, fmt.Errorf("source matrix must be rectangular: row %d has %d cells, row 1 has %d", rowIndex+1, len(row), cols)
		}
	}
	return len(data), cols, nil
}

func tableHasMergedCells(tbl *etree.Element) bool {
	for _, row := range tableRows(tbl) {
		for _, cell := range tableRowCells(row) {
			if tableCellHasMerge(cell) {
				return true
			}
		}
	}
	return false
}

func tableCellHasMerge(cell *etree.Element) bool {
	for _, name := range []string{"hMerge", "vMerge"} {
		if attrValueIsTrue(cell.SelectAttrValue(name, "")) {
			return true
		}
	}
	for _, name := range []string{"gridSpan", "rowSpan"} {
		if span, err := strconv.Atoi(cell.SelectAttrValue(name, "")); err == nil && span > 1 {
			return true
		}
	}
	return false
}

func attrValueIsTrue(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "1", "true":
		return true
	default:
		return false
	}
}

func replaceTableCellText(cell *etree.Element, prefix, text string) error {
	txBody := tableCellTextBody(cell, prefix)
	if txBody == nil {
		return fmt.Errorf("failed to create table cell text body")
	}
	rPrCopy := firstTableCellRunProperties(txBody)

	for _, paragraph := range directChildrenByLocalName(txBody, "p") {
		txBody.RemoveChild(paragraph)
	}

	paragraph := newDrawingElement(prefix, "p")
	if text != "" {
		appendDrawingTextRuns(paragraph, prefix, text, rPrCopy)
	}
	txBody.AddChild(paragraph)
	return nil
}

func tableCellTextBody(cell *etree.Element, prefix string) *etree.Element {
	if txBody := firstDirectChildByLocalName(cell, "txBody"); txBody != nil {
		return txBody
	}
	txBody := newDrawingElement(prefix, "txBody")
	txBody.AddChild(newDrawingElement(prefix, "bodyPr"))
	txBody.AddChild(newDrawingElement(prefix, "lstStyle"))
	if tcPr := firstDirectChildByLocalName(cell, "tcPr"); tcPr != nil {
		cell.InsertChildAt(tcPr.Index(), txBody)
	} else {
		cell.AddChild(txBody)
	}
	return txBody
}

func firstTableCellRunProperties(txBody *etree.Element) *etree.Element {
	for _, paragraph := range directChildrenByLocalName(txBody, "p") {
		for _, run := range directChildrenByLocalName(paragraph, "r") {
			if rPr := firstDirectChildByLocalName(run, "rPr"); rPr != nil {
				return rPr.Copy()
			}
		}
	}
	return nil
}

func appendDrawingTextRuns(paragraph *etree.Element, prefix, text string, rPrTemplate *etree.Element) {
	lines := strings.Split(text, "\n")
	for lineIndex, line := range lines {
		if line != "" {
			run := newDrawingElement(prefix, "r")
			if rPrTemplate != nil {
				run.AddChild(rPrTemplate.Copy())
			}
			t := newDrawingElement(prefix, "t")
			if textNeedsSpacePreserve(line) {
				t.CreateAttr("xml:space", "preserve")
			}
			t.SetText(line)
			run.AddChild(t)
			paragraph.AddChild(run)
		}
		if lineIndex < len(lines)-1 {
			breakRun := newDrawingElement(prefix, "r")
			breakRun.AddChild(newDrawingElement(prefix, "br"))
			paragraph.AddChild(breakRun)
		}
	}
}

func tableCellText(cell *etree.Element) string {
	txBody := firstDirectChildByLocalName(cell, "txBody")
	if txBody == nil {
		return ""
	}
	var builder strings.Builder
	for paraIndex, paragraph := range directChildrenByLocalName(txBody, "p") {
		if paraIndex > 0 {
			builder.WriteString("\n")
		}
		collectDrawingText(paragraph, &builder)
	}
	return builder.String()
}

func collectDrawingText(elem *etree.Element, builder *strings.Builder) {
	for _, child := range elem.ChildElements() {
		switch localName(child.Tag) {
		case "t":
			builder.WriteString(child.Text())
		case "br":
			builder.WriteString("\n")
		default:
			collectDrawingText(child, builder)
		}
	}
}

func tableRows(tbl *etree.Element) []*etree.Element {
	return directChildrenByLocalName(tbl, "tr")
}

func tableRowCells(row *etree.Element) []*etree.Element {
	return directChildrenByLocalName(row, "tc")
}

func directChildrenByLocalName(elem *etree.Element, name string) []*etree.Element {
	if elem == nil {
		return nil
	}
	var matches []*etree.Element
	for _, child := range elem.ChildElements() {
		if localName(child.Tag) == name {
			matches = append(matches, child)
		}
	}
	return matches
}

func firstDirectChildByLocalName(elem *etree.Element, name string) *etree.Element {
	for _, child := range directChildrenByLocalName(elem, name) {
		return child
	}
	return nil
}

func drawingPrefix(elem *etree.Element) string {
	if elem != nil && elem.Space != "" {
		return elem.Space
	}
	return "a"
}

func newDrawingElement(prefix, tag string) *etree.Element {
	elem := etree.NewElement(tag)
	elem.Space = prefix
	return elem
}

func textNeedsSpacePreserve(value string) bool {
	return value != strings.Trim(value, " \t\r\n")
}

func localName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}
