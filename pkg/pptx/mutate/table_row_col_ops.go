package mutate

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// InsertTableRowRequest holds parameters for inserting a row into a table
type InsertTableRowRequest struct {
	Package          opc.PackageSession
	SlideRef         *inspect.SlideRef
	TableID          int
	InsertAtRowIndex int
}

// InsertTableRowResult holds the result of a successful row insertion
type InsertTableRowResult struct {
	InsertedRowIndex int
	CellCount        int
}

// InsertTableRow inserts a new row at the specified index
func InsertTableRow(req *InsertTableRowRequest) (*InsertTableRowResult, error) {
	if req == nil || req.Package == nil || req.SlideRef == nil || req.TableID <= 0 {
		return nil, fmt.Errorf("invalid request")
	}
	if req.InsertAtRowIndex < 0 {
		return nil, fmt.Errorf("insert row index cannot be negative")
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

	if req.InsertAtRowIndex > len(rows) {
		return nil, fmt.Errorf("insert row index out of range")
	}
	if req.InsertAtRowIndex < len(rows) {
		for _, cell := range tableRowCells(rows[req.InsertAtRowIndex]) {
			if cell.SelectAttrValue("vMerge", "") == "1" {
				return nil, fmt.Errorf("cannot insert row at %d: would split a vertical merge", req.InsertAtRowIndex)
			}
		}
	}

	// Determine cell count
	var cellCount int
	if len(rows) > 0 {
		cellCount = len(tableRowCells(rows[0]))
	} else if tblGrid := firstDirectChildByLocalName(tblElem, "tblGrid"); tblGrid != nil {
		cellCount = len(directChildrenByLocalName(tblGrid, "gridCol"))
	}

	prefix := drawingPrefix(tblElem)
	newRow := newDrawingElement(prefix, "tr")
	if height := adjacentTableRowHeight(rows, req.InsertAtRowIndex); height != "" {
		newRow.CreateAttr("h", height)
	}
	for i := 0; i < cellCount; i++ {
		cell := createEmptyTableCellElement(prefix)
		newRow.AddChild(cell)
	}

	if req.InsertAtRowIndex >= len(rows) {
		tblElem.AddChild(newRow)
	} else {
		targetRow := rows[req.InsertAtRowIndex]
		children := tblElem.ChildElements()
		for i, child := range children {
			if child == targetRow {
				tblElem.InsertChildAt(i, newRow)
				break
			}
		}
	}

	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &InsertTableRowResult{
		InsertedRowIndex: req.InsertAtRowIndex,
		CellCount:        cellCount,
	}, nil
}

// DeleteTableRowRequest holds parameters for deleting a row from a table
type DeleteTableRowRequest struct {
	Package  opc.PackageSession
	SlideRef *inspect.SlideRef
	TableID  int
	RowIndex int
}

// DeleteTableRowResult holds the result of a successful row deletion
type DeleteTableRowResult struct {
	DeletedRowIndex int
	CellCount       int
}

// DeleteTableRow deletes a row from the table
func DeleteTableRow(req *DeleteTableRowRequest) (*DeleteTableRowResult, error) {
	if req == nil || req.Package == nil || req.SlideRef == nil || req.TableID <= 0 {
		return nil, fmt.Errorf("invalid request")
	}
	if req.RowIndex < 0 {
		return nil, fmt.Errorf("row index cannot be negative")
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
	if len(rows) <= 1 {
		return nil, fmt.Errorf("cannot delete last row")
	}

	rowToDelete := rows[req.RowIndex]
	cells := tableRowCells(rowToDelete)
	cellCount := len(cells)

	// Check for unsafe merged cells
	for _, cell := range cells {
		if rowSpanStr := cell.SelectAttrValue("rowSpan", ""); rowSpanStr != "" {
			if rowSpan, _ := strconv.Atoi(rowSpanStr); rowSpan > 1 {
				return nil, fmt.Errorf("cannot delete row %d: cell contains vertical merge extending into row(s) below", req.RowIndex)
			}
		}
		if cell.SelectAttrValue("vMerge", "") == "1" {
			return nil, fmt.Errorf("cannot delete row %d: cell is part of a vertical merge extending from above", req.RowIndex)
		}
	}

	tblElem.RemoveChild(rowToDelete)

	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &DeleteTableRowResult{
		DeletedRowIndex: req.RowIndex,
		CellCount:       cellCount,
	}, nil
}

// InsertTableColumnRequest holds parameters for inserting a column into a table
type InsertTableColumnRequest struct {
	Package             opc.PackageSession
	SlideRef            *inspect.SlideRef
	TableID             int
	InsertAtColumnIndex int
	Width               int64
}

// InsertTableColumnResult holds the result of a successful column insertion
type InsertTableColumnResult struct {
	InsertedColumnIndex int
	RowCount            int
	Width               int64
}

// InsertTableColumn inserts a new column at the specified index
func InsertTableColumn(req *InsertTableColumnRequest) (*InsertTableColumnResult, error) {
	if req == nil || req.Package == nil || req.SlideRef == nil || req.TableID <= 0 {
		return nil, fmt.Errorf("invalid request")
	}
	if req.InsertAtColumnIndex < 0 {
		return nil, fmt.Errorf("insert column index cannot be negative")
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

	tblGrid := firstDirectChildByLocalName(tblElem, "tblGrid")
	if tblGrid == nil {
		return nil, fmt.Errorf("table has no tblGrid element")
	}

	gridCols := directChildrenByLocalName(tblGrid, "gridCol")
	if req.InsertAtColumnIndex > len(gridCols) {
		return nil, fmt.Errorf("insert column index out of range")
	}
	rows := tableRows(tblElem)
	if req.InsertAtColumnIndex < len(gridCols) {
		for _, row := range rows {
			cells := tableRowCells(row)
			if req.InsertAtColumnIndex < len(cells) && cells[req.InsertAtColumnIndex].SelectAttrValue("hMerge", "") == "1" {
				return nil, fmt.Errorf("cannot insert column at %d: would split a horizontal merge", req.InsertAtColumnIndex)
			}
		}
	}

	width := req.Width
	if width <= 0 {
		var totalWidth int64
		for _, col := range gridCols {
			if wStr := col.SelectAttrValue("w", ""); wStr != "" {
				if w, _ := strconv.ParseInt(wStr, 10, 64); w > 0 {
					totalWidth += w
				}
			}
		}
		if len(gridCols) > 0 {
			width = totalWidth / int64(len(gridCols))
		} else {
			width = 1828800
		}
	}

	prefix := drawingPrefix(tblElem)
	newGridCol := newDrawingElement(prefix, "gridCol")
	newGridCol.CreateAttr("w", strconv.FormatInt(width, 10))

	if req.InsertAtColumnIndex >= len(gridCols) {
		tblGrid.AddChild(newGridCol)
	} else {
		children := tblGrid.ChildElements()
		for i, child := range children {
			if child == gridCols[req.InsertAtColumnIndex] {
				tblGrid.InsertChildAt(i, newGridCol)
				break
			}
		}
	}

	for _, row := range rows {
		newCell := createEmptyTableCellElement(prefix)
		cells := tableRowCells(row)
		if req.InsertAtColumnIndex >= len(cells) {
			row.AddChild(newCell)
		} else {
			children := row.ChildElements()
			for i, child := range children {
				if child == cells[req.InsertAtColumnIndex] {
					row.InsertChildAt(i, newCell)
					break
				}
			}
		}
	}

	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &InsertTableColumnResult{
		InsertedColumnIndex: req.InsertAtColumnIndex,
		RowCount:            len(rows),
		Width:               width,
	}, nil
}

// DeleteTableColumnRequest holds parameters for deleting a column from a table
type DeleteTableColumnRequest struct {
	Package     opc.PackageSession
	SlideRef    *inspect.SlideRef
	TableID     int
	ColumnIndex int
}

// DeleteTableColumnResult holds the result of a successful column deletion
type DeleteTableColumnResult struct {
	DeletedColumnIndex int
	RowCount           int
}

// DeleteTableColumn deletes a column from the table
func DeleteTableColumn(req *DeleteTableColumnRequest) (*DeleteTableColumnResult, error) {
	if req == nil || req.Package == nil || req.SlideRef == nil || req.TableID <= 0 {
		return nil, fmt.Errorf("invalid request")
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

	tblGrid := firstDirectChildByLocalName(tblElem, "tblGrid")
	if tblGrid == nil {
		return nil, fmt.Errorf("table has no tblGrid element")
	}

	gridCols := directChildrenByLocalName(tblGrid, "gridCol")
	if req.ColumnIndex >= len(gridCols) {
		return nil, fmt.Errorf("column index out of range")
	}
	if len(gridCols) <= 1 {
		return nil, fmt.Errorf("cannot delete last column")
	}

	rows := tableRows(tblElem)
	for _, row := range rows {
		cells := tableRowCells(row)
		if req.ColumnIndex >= len(cells) {
			continue
		}
		cell := cells[req.ColumnIndex]

		// Check for unsafe merges
		if gridSpanStr := cell.SelectAttrValue("gridSpan", ""); gridSpanStr != "" {
			if gridSpan, _ := strconv.Atoi(gridSpanStr); gridSpan > 1 {
				return nil, fmt.Errorf("cannot delete column %d: cell contains horizontal merge extending right", req.ColumnIndex)
			}
		}
		if cell.SelectAttrValue("hMerge", "") == "1" {
			return nil, fmt.Errorf("cannot delete column %d: cell is part of a merge extending from left", req.ColumnIndex)
		}
	}

	// Delete the grid column
	gridColToDelete := gridCols[req.ColumnIndex]
	tblGrid.RemoveChild(gridColToDelete)

	// Delete cells from rows
	for _, row := range rows {
		cells := tableRowCells(row)
		if req.ColumnIndex < len(cells) {
			row.RemoveChild(cells[req.ColumnIndex])
		}
	}

	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &DeleteTableColumnResult{
		DeletedColumnIndex: req.ColumnIndex,
		RowCount:           len(rows),
	}, nil
}

// findTableInSlide finds a table (a:tbl) within a graphic frame with the given table ID
func findTableInSlide(slideRoot *etree.Element, tableID int) (*etree.Element, error) {
	spTree := slideRoot.FindElement("//p:spTree")
	if spTree == nil {
		spTree = slideRoot.FindElement(".//p:spTree")
		if spTree == nil {
			return nil, fmt.Errorf("shape tree not found in slide")
		}
	}

	tableIDStr := strconv.Itoa(tableID)

	// Search for a graphic frame with this ID containing a table
	for _, gf := range spTree.FindElements(".//p:graphicFrame") {
		nvGraphicFramePr := gf.FindElement(".//p:nvGraphicFramePr")
		if nvGraphicFramePr == nil {
			nvGraphicFramePr = gf.FindElement("p:nvGraphicFramePr")
		}

		if nvGraphicFramePr != nil {
			cNvPr := nvGraphicFramePr.FindElement(".//p:cNvPr")
			if cNvPr == nil {
				cNvPr = nvGraphicFramePr.FindElement("p:cNvPr")
			}

			if cNvPr != nil && cNvPr.SelectAttrValue("id", "") == tableIDStr {
				// Found the graphic frame with the matching ID
				graphicData := gf.FindElement(".//a:graphicData")
				if graphicData == nil {
					graphicData = gf.FindElement("a:graphicData")
				}

				if graphicData != nil {
					tbl := graphicData.FindElement(".//a:tbl")
					if tbl == nil {
						tbl = graphicData.FindElement("a:tbl")
					}

					if tbl != nil {
						return tbl, nil
					}
				}
			}
		}
	}

	return nil, nil
}

func adjacentTableRowHeight(rows []*etree.Element, insertAt int) string {
	if len(rows) == 0 {
		return ""
	}
	if insertAt > 0 && insertAt-1 < len(rows) {
		return rows[insertAt-1].SelectAttrValue("h", "")
	}
	return rows[0].SelectAttrValue("h", "")
}

// createEmptyTableCellElement creates a new empty table cell with default structure.
func createEmptyTableCellElement(prefix string) *etree.Element {
	cell := newDrawingElement(prefix, "tc")

	// Create text body with empty paragraph
	txBody := newDrawingElement(prefix, "txBody")
	bodyPr := newDrawingElement(prefix, "bodyPr")
	txBody.AddChild(bodyPr)

	lstStyle := newDrawingElement(prefix, "lstStyle")
	txBody.AddChild(lstStyle)

	p := newDrawingElement(prefix, "p")
	txBody.AddChild(p)

	cell.AddChild(txBody)

	// Create cell properties
	tcPr := newDrawingElement(prefix, "tcPr")
	cell.AddChild(tcPr)

	return cell
}
