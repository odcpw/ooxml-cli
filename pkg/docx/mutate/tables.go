package mutate

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	ErrTableIndexOutOfRange = errors.New("table index out of range")
	ErrTableCellOutOfRange  = errors.New("table cell out of range")
	ErrTableHasMergedCells  = errors.New("table has merged cells")
	ErrDeleteLastTableRow   = errors.New("cannot delete the last table row")
)

type SetTableCellTextRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	TableIndex   int
	ExpectedHash string
	RowIndex     int
	ColumnIndex  int
	Text         string
}

type SetTableCellTextResult struct {
	TableIndex   int    `json:"tableIndex"`
	BlockIndex   int    `json:"blockIndex"`
	RowIndex     int    `json:"rowIndex"`
	ColumnIndex  int    `json:"columnIndex"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
	Text         string `json:"text"`
	PreviousText string `json:"previousText"`
	Flattened    bool   `json:"flattened"`
}

type ClearTableCellTextRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	TableIndex   int
	ExpectedHash string
	RowIndex     int
	ColumnIndex  int
}

type ClearTableCellTextResult struct {
	TableIndex   int    `json:"tableIndex"`
	BlockIndex   int    `json:"blockIndex"`
	RowIndex     int    `json:"rowIndex"`
	ColumnIndex  int    `json:"columnIndex"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
	PreviousText string `json:"previousText"`
	Flattened    bool   `json:"flattened"`
}

type InsertTableRowRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	TableIndex   int
	ExpectedHash string
	At           int
}

type InsertTableRowResult struct {
	TableIndex   int    `json:"tableIndex"`
	BlockIndex   int    `json:"blockIndex"`
	RowIndex     int    `json:"rowIndex"`
	Rows         int    `json:"rows"`
	Cols         int    `json:"cols"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
}

type DeleteTableRowRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	TableIndex   int
	ExpectedHash string
	RowIndex     int
}

type DeleteTableRowResult struct {
	TableIndex   int    `json:"tableIndex"`
	BlockIndex   int    `json:"blockIndex"`
	RowIndex     int    `json:"rowIndex"`
	Rows         int    `json:"rows"`
	Cols         int    `json:"cols"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
}

func SetTableCellText(req *SetTableCellTextRequest) (*SetTableCellTextResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set table cell text request is nil")
	}
	doc, block, prefix, err := locateTable(req.Package, req.DocumentURI, req.TableIndex)
	if err != nil {
		return nil, err
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	table := block.Element
	ensureTableScaffold(doc.Root(), prefix, table)
	cell, err := locateTableCell(table, req.RowIndex, req.ColumnIndex)
	if err != nil {
		return nil, err
	}
	previous, flattened := setTableCellText(doc.Root(), prefix, cell, req.Text)
	newReport := extract.ReportBlock(docxbody.BodyBlock{Index: block.Index, Kind: block.Kind, Element: table}, false)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &SetTableCellTextResult{
		TableIndex:   req.TableIndex,
		BlockIndex:   block.Index,
		RowIndex:     req.RowIndex,
		ColumnIndex:  req.ColumnIndex,
		ContentHash:  newReport.ContentHash,
		PreviousHash: report.ContentHash,
		Text:         req.Text,
		PreviousText: previous,
		Flattened:    flattened,
	}, nil
}

func ClearTableCellText(req *ClearTableCellTextRequest) (*ClearTableCellTextResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear table cell text request is nil")
	}
	result, err := SetTableCellText(&SetTableCellTextRequest{
		Package:      req.Package,
		DocumentURI:  req.DocumentURI,
		TableIndex:   req.TableIndex,
		ExpectedHash: req.ExpectedHash,
		RowIndex:     req.RowIndex,
		ColumnIndex:  req.ColumnIndex,
		Text:         "",
	})
	if err != nil {
		return nil, err
	}
	return &ClearTableCellTextResult{
		TableIndex:   result.TableIndex,
		BlockIndex:   result.BlockIndex,
		RowIndex:     result.RowIndex,
		ColumnIndex:  result.ColumnIndex,
		ContentHash:  result.ContentHash,
		PreviousHash: result.PreviousHash,
		PreviousText: result.PreviousText,
		Flattened:    result.Flattened,
	}, nil
}

func InsertTableRow(req *InsertTableRowRequest) (*InsertTableRowResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert table row request is nil")
	}
	doc, block, prefix, err := locateTable(req.Package, req.DocumentURI, req.TableIndex)
	if err != nil {
		return nil, err
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	table := block.Element
	ensureTableScaffold(doc.Root(), prefix, table)
	if tableHasMergedCells(table) {
		return nil, ErrTableHasMergedCells
	}
	rows := tableRows(table)
	if req.At < 1 || req.At > len(rows)+1 {
		return nil, fmt.Errorf("%w: row %d", ErrTableCellOutOfRange, req.At)
	}
	if len(rows) == 0 {
		return nil, fmt.Errorf("%w: table has no rows", ErrTableCellOutOfRange)
	}

	template := rows[len(rows)-1]
	if req.At <= len(rows) {
		template = rows[req.At-1]
	}
	inserted := template.Copy()
	for _, cell := range rowCells(inserted) {
		setTableCellText(doc.Root(), prefix, cell, "")
	}
	if req.At <= len(rows) {
		table.InsertChildAt(rows[req.At-1].Index(), inserted)
	} else {
		table.AddChild(inserted)
	}

	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	rowsAfter, colsAfter := tableDimensions(table)
	newReport := extract.ReportBlock(docxbody.BodyBlock{Index: block.Index, Kind: block.Kind, Element: table}, false)
	return &InsertTableRowResult{
		TableIndex:   req.TableIndex,
		BlockIndex:   block.Index,
		RowIndex:     req.At,
		Rows:         rowsAfter,
		Cols:         colsAfter,
		ContentHash:  newReport.ContentHash,
		PreviousHash: report.ContentHash,
	}, nil
}

func DeleteTableRow(req *DeleteTableRowRequest) (*DeleteTableRowResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete table row request is nil")
	}
	doc, block, prefix, err := locateTable(req.Package, req.DocumentURI, req.TableIndex)
	if err != nil {
		return nil, err
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	table := block.Element
	ensureTableScaffold(doc.Root(), prefix, table)
	if tableHasMergedCells(table) {
		return nil, ErrTableHasMergedCells
	}
	rows := tableRows(table)
	if req.RowIndex < 1 || req.RowIndex > len(rows) {
		return nil, fmt.Errorf("%w: row %d", ErrTableCellOutOfRange, req.RowIndex)
	}
	if len(rows) == 1 {
		return nil, ErrDeleteLastTableRow
	}
	table.RemoveChild(rows[req.RowIndex-1])

	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	rowsAfter, colsAfter := tableDimensions(table)
	newReport := extract.ReportBlock(docxbody.BodyBlock{Index: block.Index, Kind: block.Kind, Element: table}, false)
	return &DeleteTableRowResult{
		TableIndex:   req.TableIndex,
		BlockIndex:   block.Index,
		RowIndex:     req.RowIndex,
		Rows:         rowsAfter,
		Cols:         colsAfter,
		ContentHash:  newReport.ContentHash,
		PreviousHash: report.ContentHash,
	}, nil
}

func locateTable(session opc.PackageSession, documentURI string, tableIndex int) (*etree.Document, docxbody.BodyBlock, string, error) {
	if tableIndex < 1 {
		return nil, docxbody.BodyBlock{}, "", fmt.Errorf("table index must be >= 1")
	}
	doc, bodyElem, prefix, err := locateBody(session, documentURI)
	if err != nil {
		return nil, docxbody.BodyBlock{}, "", err
	}
	seen := 0
	for _, block := range docxbody.Blocks(bodyElem) {
		if block.Kind != "table" {
			continue
		}
		seen++
		if seen == tableIndex {
			return doc, block, prefix, nil
		}
	}
	return nil, docxbody.BodyBlock{}, "", fmt.Errorf("%w: %d", ErrTableIndexOutOfRange, tableIndex)
}

func locateTableCell(table *etree.Element, rowIndex, colIndex int) (*etree.Element, error) {
	rows := tableRows(table)
	if rowIndex < 1 || rowIndex > len(rows) {
		return nil, fmt.Errorf("%w: row %d", ErrTableCellOutOfRange, rowIndex)
	}
	cells := rowCells(rows[rowIndex-1])
	if colIndex < 1 || colIndex > len(cells) {
		return nil, fmt.Errorf("%w: row %d column %d", ErrTableCellOutOfRange, rowIndex, colIndex)
	}
	return cells[colIndex-1], nil
}

func tableRows(table *etree.Element) []*etree.Element {
	return childElementsByLocalName(table, "tr")
}

func rowCells(row *etree.Element) []*etree.Element {
	return childElementsByLocalName(row, "tc")
}

func tableDimensions(table *etree.Element) (int, int) {
	rows := tableRows(table)
	cols := 0
	for _, row := range rows {
		if count := tableRowGridWidth(row); count > cols {
			cols = count
		}
	}
	return len(rows), cols
}

func tableHasMergedCells(table *etree.Element) bool {
	return hasDescendantByLocalName(table, "gridSpan") || hasDescendantByLocalName(table, "vMerge")
}

func ensureTableScaffold(root *etree.Element, prefix string, table *etree.Element) {
	if table == nil {
		return
	}
	if prefix == "" {
		prefix = "w"
	}
	ensureNamespacePrefix(root, prefix, namespaces.NsW)

	firstRowIndex := len(table.Child)
	for _, child := range table.ChildElements() {
		if docxbody.LocalName(child.Tag) == "tr" {
			firstRowIndex = child.Index()
			break
		}
	}

	tblPr := firstChildByLocalName(table, "tblPr")
	if tblPr == nil {
		tblPr = newElement(prefix, "tblPr")
		table.InsertChildAt(firstRowIndex, tblPr)
		firstRowIndex++
	}

	rowCount, cols := tableDimensions(table)
	tblGrid := firstChildByLocalName(table, "tblGrid")
	if tblGrid == nil {
		tblGrid = newElement(prefix, "tblGrid")
		reconcileTableGrid(root, prefix, tblGrid, cols)
		table.InsertChildAt(firstRowIndex, tblGrid)
	} else if rowCount > 0 {
		reconcileTableGrid(root, prefix, tblGrid, cols)
	}
}

func reconcileTableGrid(root *etree.Element, prefix string, tblGrid *etree.Element, cols int) {
	if tblGrid == nil || cols < 0 {
		return
	}
	gridCols := childElementsByLocalName(tblGrid, "gridCol")
	for len(gridCols) > cols {
		last := gridCols[len(gridCols)-1]
		tblGrid.RemoveChild(last)
		gridCols = gridCols[:len(gridCols)-1]
	}
	for len(gridCols) < cols {
		gridCol := newElement(prefix, "gridCol")
		gridCol.CreateAttr(qualifiedWordAttrName(root, prefix, "w"), "0")
		tblGrid.AddChild(gridCol)
		gridCols = append(gridCols, gridCol)
	}
}

func tableRowGridWidth(row *etree.Element) int {
	width := 0
	if trPr := firstChildByLocalName(row, "trPr"); trPr != nil {
		width += nonNegativeWordIntAttr(firstChildByLocalName(trPr, "gridBefore"), "val", 0)
		width += nonNegativeWordIntAttr(firstChildByLocalName(trPr, "gridAfter"), "val", 0)
	}
	for _, cell := range rowCells(row) {
		width += tableCellGridSpan(cell)
	}
	return width
}

func tableCellGridSpan(cell *etree.Element) int {
	tcPr := firstChildByLocalName(cell, "tcPr")
	if tcPr == nil {
		return 1
	}
	return positiveWordIntAttr(firstChildByLocalName(tcPr, "gridSpan"), "val", 1)
}

func positiveWordIntAttr(elem *etree.Element, localName string, fallback int) int {
	value, ok := namespaces.Attr(elem, namespaces.NsW, localName)
	if !ok {
		return fallback
	}
	parsed, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || parsed < 1 {
		return fallback
	}
	return parsed
}

func nonNegativeWordIntAttr(elem *etree.Element, localName string, fallback int) int {
	value, ok := namespaces.Attr(elem, namespaces.NsW, localName)
	if !ok {
		return fallback
	}
	parsed, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || parsed < 0 {
		return fallback
	}
	return parsed
}

func setTableCellText(root *etree.Element, prefix string, cell *etree.Element, text string) (string, bool) {
	previous := tableCellText(cell)
	paragraphs := childElementsByLocalName(cell, "p")
	flattened := len(paragraphs) > 1

	var pPrCopy, rPrCopy *etree.Element
	if len(paragraphs) > 0 {
		if pPr := firstChildByLocalName(paragraphs[0], "pPr"); pPr != nil {
			pPrCopy = pPr.Copy()
		}
		rPrCopy = firstDirectRunProperties(paragraphs[0])
	}

	for _, child := range cell.ChildElements() {
		if docxbody.LocalName(child.Tag) == "tcPr" {
			continue
		}
		if docxbody.LocalName(child.Tag) != "p" || len(paragraphs) > 1 {
			flattened = true
		}
		cell.RemoveChild(child)
	}

	paragraph := newElement(prefix, "p")
	if pPrCopy != nil {
		paragraph.AddChild(pPrCopy)
	}
	if text != "" {
		run := newElement(prefix, "r")
		if rPrCopy != nil {
			run.AddChild(rPrCopy)
		}
		appendTextChildren(run, prefix, text)
		paragraph.AddChild(run)
	}
	cell.AddChild(paragraph)
	if pPrCopy != nil {
		ensureNamespacePrefix(root, "w", namespaces.NsW)
	}
	return previous, flattened
}

func tableCellText(cell *etree.Element) string {
	var text []string
	for _, paragraph := range childElementsByLocalName(cell, "p") {
		text = append(text, docxbody.ParagraphText(paragraph))
	}
	return joinParagraphTexts(text)
}

func joinParagraphTexts(values []string) string {
	if len(values) == 0 {
		return ""
	}
	out := values[0]
	for _, value := range values[1:] {
		out += "\n" + value
	}
	return out
}

func childElementsByLocalName(elem *etree.Element, local string) []*etree.Element {
	if elem == nil {
		return nil
	}
	var out []*etree.Element
	for _, child := range elem.ChildElements() {
		if docxbody.LocalName(child.Tag) == local {
			out = append(out, child)
		}
	}
	return out
}

func firstChildByLocalName(elem *etree.Element, local string) *etree.Element {
	for _, child := range childElementsByLocalName(elem, local) {
		return child
	}
	return nil
}

func hasDescendantByLocalName(elem *etree.Element, local string) bool {
	if elem == nil {
		return false
	}
	for _, child := range elem.ChildElements() {
		if docxbody.LocalName(child.Tag) == local || hasDescendantByLocalName(child, local) {
			return true
		}
	}
	return false
}
