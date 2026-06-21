package mutate

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func ensureSheetData(root *etree.Element, prefix string) *etree.Element {
	if sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData"); sheetData != nil {
		return sheetData
	}
	sheetData := newElement(prefix, "sheetData")
	insertWorksheetChild(root, sheetData, "sheetData")
	return sheetData
}

func ensureRow(sheetData *etree.Element, prefix string, rowNum int) *etree.Element {
	if row := findRow(sheetData, rowNum); row != nil {
		return row
	}
	row := newElement(prefix, "row")
	row.CreateAttr("r", strconv.Itoa(rowNum))

	for _, existing := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		existingNum, ok := rowNumber(existing)
		if ok && existingNum > rowNum {
			sheetData.InsertChildAt(existing.Index(), row)
			return row
		}
	}
	sheetData.AddChild(row)
	return row
}

func ensureCell(row *etree.Element, prefix, ref string, column int) (*etree.Element, bool) {
	if cell := findCell(row, ref); cell != nil {
		return cell, false
	}
	cell := newElement(prefix, "c")
	cell.CreateAttr("r", ref)

	for _, existing := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
		existingRef, ok := cellReference(existing)
		if ok && existingRef.Column > column {
			row.InsertChildAt(existing.Index(), cell)
			return cell, true
		}
	}
	addCellToRow(row, cell)
	return cell, true
}

func findRow(sheetData *etree.Element, rowNum int) *etree.Element {
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		existing, ok := rowNumber(row)
		if ok && existing == rowNum {
			return row
		}
	}
	return nil
}

func findCell(row *etree.Element, ref string) *etree.Element {
	for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
		if cell.SelectAttrValue("r", "") == ref {
			return cell
		}
	}
	return nil
}

func rowNumber(row *etree.Element) (int, bool) {
	value := row.SelectAttrValue("r", "")
	if value == "" {
		return 0, false
	}
	rowNum, err := strconv.Atoi(value)
	if err != nil || rowNum < 1 {
		return 0, false
	}
	return rowNum, true
}

func cellReference(cell *etree.Element) (address.CellRef, bool) {
	refText := cell.SelectAttrValue("r", "")
	if refText == "" {
		return address.CellRef{}, false
	}
	ref, err := address.ParseCell(refText)
	if err != nil {
		return address.CellRef{}, false
	}
	return ref, true
}

func clearCellContent(cell *etree.Element) {
	cell.RemoveAttr("t")
	for _, child := range cell.ChildElements() {
		if isCellContentElement(child) {
			cell.RemoveChild(child)
		}
	}
}

func isCellContentElement(elem *etree.Element) bool {
	return namespaces.IsElement(elem, namespaces.NsSpreadsheetML, "v") ||
		namespaces.IsElement(elem, namespaces.NsSpreadsheetML, "f") ||
		namespaces.IsElement(elem, namespaces.NsSpreadsheetML, "is")
}

func summarizeCell(cell *etree.Element) (string, string) {
	if cell == nil {
		return "", ""
	}
	if formula := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "f"); formula != nil {
		return "formula", formula.Text()
	}
	switch cell.SelectAttrValue("t", "") {
	case "inlineStr":
		inline := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "is")
		if inline == nil {
			return "string", ""
		}
		var value string
		for _, text := range namespaces.FindDescendants(inline, namespaces.NsSpreadsheetML, "t") {
			value += text.Text()
		}
		return "string", value
	case "s":
		return "sharedString", childText(cell, "v")
	case "b":
		return "bool", childText(cell, "v")
	case "e":
		return "error", childText(cell, "v")
	case "str":
		return "string", childText(cell, "v")
	case "d":
		return "date", childText(cell, "v")
	case "":
		if value := childText(cell, "v"); value != "" {
			return "number", value
		}
		return "", ""
	default:
		return cell.SelectAttrValue("t", ""), childText(cell, "v")
	}
}

func childText(elem *etree.Element, localName string) string {
	child := namespaces.FindChild(elem, namespaces.NsSpreadsheetML, localName)
	if child == nil {
		return ""
	}
	return child.Text()
}

func updateDimension(root *etree.Element, prefix string) {
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	if sheetData == nil {
		removeDimension(root)
		return
	}

	minCol, minRow, maxCol, maxRow := 0, 0, 0, 0
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
			ref, ok := cellReference(cell)
			if !ok {
				continue
			}
			if minRow == 0 || ref.Row < minRow {
				minRow = ref.Row
			}
			if ref.Row > maxRow {
				maxRow = ref.Row
			}
			if minCol == 0 || ref.Column < minCol {
				minCol = ref.Column
			}
			if ref.Column > maxCol {
				maxCol = ref.Column
			}
		}
	}

	if minRow == 0 {
		removeDimension(root)
		return
	}
	dimension := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dimension")
	if dimension == nil {
		dimension = newElement(prefix, "dimension")
		insertWorksheetChild(root, dimension, "dimension")
	}
	dimension.CreateAttr("ref", rangeRef(minCol, minRow, maxCol, maxRow))
}

func removeDimension(root *etree.Element) {
	if dimension := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dimension"); dimension != nil {
		root.RemoveChild(dimension)
	}
}

func rangeRef(minCol, minRow, maxCol, maxRow int) string {
	start := cellRef(minCol, minRow)
	end := cellRef(maxCol, maxRow)
	if start == end {
		return start
	}
	return fmt.Sprintf("%s:%s", start, end)
}

func cellRef(col, row int) string {
	column, _ := address.ColumnIndexToLetters(col)
	return fmt.Sprintf("%s%d", column, row)
}

func newElement(prefix, tag string) *etree.Element {
	elem := etree.NewElement(tag)
	elem.Space = prefix
	return elem
}

func addCellChild(cell *etree.Element, child *etree.Element) {
	for _, existing := range cell.ChildElements() {
		if namespaces.IsElement(existing, namespaces.NsSpreadsheetML, "extLst") {
			cell.InsertChildAt(existing.Index(), child)
			return
		}
	}
	cell.AddChild(child)
}

func addCellToRow(row *etree.Element, cell *etree.Element) {
	if extLst := namespaces.FindChild(row, namespaces.NsSpreadsheetML, "extLst"); extLst != nil {
		row.InsertChildAt(extLst.Index(), cell)
		return
	}
	row.AddChild(cell)
}

func insertWorksheetChild(root *etree.Element, child *etree.Element, localName string) {
	targetOrder := worksheetChildOrder(localName)
	for _, existing := range root.ChildElements() {
		if !namespaces.IsElement(existing, namespaces.NsSpreadsheetML, existing.Tag) {
			continue
		}
		if existingOrder := worksheetChildOrder(existing.Tag); existingOrder > targetOrder {
			root.InsertChildAt(existing.Index(), child)
			return
		}
	}
	root.AddChild(child)
}

func worksheetChildOrder(localName string) int {
	switch localName {
	case "sheetPr":
		return 10
	case "dimension":
		return 20
	case "sheetViews":
		return 30
	case "sheetFormatPr":
		return 40
	case "cols":
		return 50
	case "sheetData":
		return 60
	case "sheetCalcPr":
		return 70
	case "sheetProtection":
		return 80
	case "protectedRanges":
		return 90
	case "scenarios":
		return 100
	case "autoFilter":
		return 110
	case "sortState":
		return 120
	case "dataConsolidate":
		return 130
	case "customSheetViews":
		return 140
	case "mergeCells":
		return 150
	case "phoneticPr":
		return 160
	case "conditionalFormatting":
		return 170
	case "dataValidations":
		return 180
	case "hyperlinks":
		return 190
	case "printOptions":
		return 200
	case "pageMargins":
		return 210
	case "pageSetup":
		return 220
	case "headerFooter":
		return 230
	case "rowBreaks":
		return 240
	case "colBreaks":
		return 250
	case "customProperties":
		return 260
	case "cellWatches":
		return 270
	case "ignoredErrors":
		return 280
	case "smartTags":
		return 290
	case "drawing":
		return 300
	case "legacyDrawing":
		return 310
	case "legacyDrawingHF":
		return 320
	case "picture":
		return 330
	case "oleObjects":
		return 340
	case "controls":
		return 350
	case "webPublishItems":
		return 360
	case "tableParts":
		return 370
	case "extLst":
		return 380
	default:
		return 1000
	}
}
