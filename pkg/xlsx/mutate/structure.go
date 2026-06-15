package mutate

import (
	"errors"
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

var (
	ErrWorksheetHasFormulas              = errors.New("worksheet has formulas")
	ErrWorkbookHasDefinedNames           = errors.New("workbook has defined names")
	ErrWorkbookHasCalcChain              = errors.New("workbook has calc chain")
	ErrWorksheetHasMergedCells           = errors.New("worksheet has merged cells")
	ErrWorksheetHasTables                = errors.New("worksheet has tables")
	ErrWorksheetHasAutofilter            = errors.New("worksheet has autofilter or sort state")
	ErrWorksheetHasDrawings              = errors.New("worksheet has drawings or comments")
	ErrWorksheetHasHyperlinks            = errors.New("worksheet has hyperlinks")
	ErrWorksheetHasConditionalFormatting = errors.New("worksheet has conditional formatting")
	ErrWorksheetHasDataValidations       = errors.New("worksheet has data validations")
	ErrWorksheetHasColumnMetadata        = errors.New("worksheet has column metadata")
	ErrWorksheetHasInvalidReferences     = errors.New("worksheet has missing or invalid row/cell references")
	ErrWorksheetHasUnsupportedStructure  = errors.New("worksheet has unsupported structural references")
	ErrWorksheetStructureOutOfBounds     = errors.New("worksheet structural edit out of bounds")
)

type StructureMutationResult struct {
	Sheet        string `json:"sheet"`
	SheetNumber  int    `json:"sheetNumber"`
	Axis         string `json:"axis"`
	Operation    string `json:"operation"`
	Start        int    `json:"start"`
	StartColumn  string `json:"startColumn,omitempty"`
	Count        int    `json:"count"`
	ShiftedRows  int    `json:"shiftedRows,omitempty"`
	ShiftedCells int    `json:"shiftedCells"`
	RemovedRows  int    `json:"removedRows,omitempty"`
	RemovedCells int    `json:"removedCells,omitempty"`
	OldUsedRange string `json:"oldUsedRange,omitempty"`
	NewUsedRange string `json:"newUsedRange,omitempty"`
}

type InsertRowsRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	At          int
	Count       int
}

type DeleteRowsRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	Row         int
	Count       int
}

type InsertColumnsRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	At          int
	Count       int
}

type DeleteColumnsRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	Column      int
	Count       int
}

func InsertRows(req *InsertRowsRequest) (*StructureMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert rows request is nil")
	}
	doc, root, prefix, err := openStructureWorksheet(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if err := validateStructureRequest(req.Package, req.WorkbookURI, root, req.At, req.Count, false); err != nil {
		return nil, err
	}
	oldRange := currentUsedRange(root)
	result, err := insertRows(root, prefix, req.SheetRef, req.At, req.Count)
	if err != nil {
		return nil, err
	}
	result.OldUsedRange = oldRange
	result.NewUsedRange = currentUsedRange(root)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return result, nil
}

func DeleteRows(req *DeleteRowsRequest) (*StructureMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete rows request is nil")
	}
	doc, root, prefix, err := openStructureWorksheet(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if err := validateStructureRequest(req.Package, req.WorkbookURI, root, req.Row, req.Count, false); err != nil {
		return nil, err
	}
	oldRange := currentUsedRange(root)
	result, err := deleteRows(root, prefix, req.SheetRef, req.Row, req.Count)
	if err != nil {
		return nil, err
	}
	result.OldUsedRange = oldRange
	result.NewUsedRange = currentUsedRange(root)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return result, nil
}

func InsertColumns(req *InsertColumnsRequest) (*StructureMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert columns request is nil")
	}
	doc, root, prefix, err := openStructureWorksheet(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if err := validateStructureRequest(req.Package, req.WorkbookURI, root, req.At, req.Count, true); err != nil {
		return nil, err
	}
	oldRange := currentUsedRange(root)
	result, err := insertColumns(root, prefix, req.SheetRef, req.At, req.Count)
	if err != nil {
		return nil, err
	}
	result.OldUsedRange = oldRange
	result.NewUsedRange = currentUsedRange(root)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return result, nil
}

func DeleteColumns(req *DeleteColumnsRequest) (*StructureMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete columns request is nil")
	}
	doc, root, prefix, err := openStructureWorksheet(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if err := validateStructureRequest(req.Package, req.WorkbookURI, root, req.Column, req.Count, true); err != nil {
		return nil, err
	}
	oldRange := currentUsedRange(root)
	result, err := deleteColumns(root, prefix, req.SheetRef, req.Column, req.Count)
	if err != nil {
		return nil, err
	}
	result.OldUsedRange = oldRange
	result.NewUsedRange = currentUsedRange(root)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return result, nil
}

func openStructureWorksheet(session opc.PackageSession, sheetRef model.SheetRef) (*etree.Document, *etree.Element, string, error) {
	if session == nil {
		return nil, nil, "", fmt.Errorf("package session is nil")
	}
	if sheetRef.PartURI == "" {
		return nil, nil, "", fmt.Errorf("sheet %q has no worksheet part URI", sheetRef.Name)
	}
	doc, err := session.ReadXMLPart(sheetRef.PartURI)
	if err != nil {
		return nil, nil, "", fmt.Errorf("failed to read worksheet %s: %w", sheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, nil, "", fmt.Errorf("worksheet part %s root element not found", sheetRef.PartURI)
	}
	return doc, root, root.Space, nil
}

func validateStructureRequest(session opc.PackageSession, workbookURI string, root *etree.Element, start, count int, columnEdit bool) error {
	if start < 1 || count < 1 {
		return fmt.Errorf("%w: start and count must be positive", ErrWorksheetStructureOutOfBounds)
	}
	if columnEdit {
		if count > address.MaxColumn {
			return fmt.Errorf("%w: column count %d exceeds XLSX column limit %d", ErrWorksheetStructureOutOfBounds, count, address.MaxColumn)
		}
		if _, err := address.OffsetColumn(start, count-1); err != nil {
			return fmt.Errorf("%w: %v", ErrWorksheetStructureOutOfBounds, err)
		}
	} else {
		if count > address.MaxRow {
			return fmt.Errorf("%w: row count %d exceeds XLSX row limit %d", ErrWorksheetStructureOutOfBounds, count, address.MaxRow)
		}
		if _, err := address.OffsetRow(start, count-1); err != nil {
			return fmt.Errorf("%w: %v", ErrWorksheetStructureOutOfBounds, err)
		}
	}
	if err := validateStructureReferences(root); err != nil {
		return err
	}
	if err := scanStructureHazards(session, workbookURI, root, columnEdit); err != nil {
		return err
	}
	return nil
}

func scanStructureHazards(session opc.PackageSession, workbookURI string, root *etree.Element, columnEdit bool) error {
	if workbookURI != "" {
		if workbookHasDefinedNames(session, workbookURI) {
			return ErrWorkbookHasDefinedNames
		}
	}
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == "/xl/calcChain.xml" || part.ContentType == namespaces.ContentTypeCalcChain {
			return ErrWorkbookHasCalcChain
		}
	}
	if len(namespaces.FindDescendants(root, namespaces.NsSpreadsheetML, "f")) > 0 {
		return ErrWorksheetHasFormulas
	}
	for _, name := range []string{"mergeCells", "mergeCell"} {
		if namespaces.FindChild(root, namespaces.NsSpreadsheetML, name) != nil || len(namespaces.FindDescendants(root, namespaces.NsSpreadsheetML, name)) > 0 {
			return ErrWorksheetHasMergedCells
		}
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "tableParts") != nil {
		return ErrWorksheetHasTables
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter") != nil ||
		namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sortState") != nil {
		return ErrWorksheetHasAutofilter
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "drawing") != nil ||
		namespaces.FindChild(root, namespaces.NsSpreadsheetML, "legacyDrawing") != nil ||
		namespaces.FindChild(root, namespaces.NsSpreadsheetML, "legacyDrawingHF") != nil {
		return ErrWorksheetHasDrawings
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "hyperlinks") != nil {
		return ErrWorksheetHasHyperlinks
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "conditionalFormatting") != nil {
		return ErrWorksheetHasConditionalFormatting
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dataValidations") != nil {
		return ErrWorksheetHasDataValidations
	}
	if columnEdit && namespaces.FindChild(root, namespaces.NsSpreadsheetML, "cols") != nil {
		return ErrWorksheetHasColumnMetadata
	}
	for _, name := range []string{
		"protectedRanges",
		"scenarios",
		"rowBreaks",
		"colBreaks",
		"dataConsolidate",
		"customSheetViews",
		"cellWatches",
		"ignoredErrors",
		"smartTags",
		"picture",
		"oleObjects",
		"controls",
		"webPublishItems",
		"extLst",
	} {
		if namespaces.FindChild(root, namespaces.NsSpreadsheetML, name) != nil {
			return fmt.Errorf("%w: %s", ErrWorksheetHasUnsupportedStructure, name)
		}
	}
	return nil
}

func validateStructureReferences(root *etree.Element) error {
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	if sheetData == nil {
		return nil
	}
	seenRows := make(map[int]struct{})
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		rowText := strings.TrimSpace(row.SelectAttrValue("r", ""))
		if rowText == "" {
			return fmt.Errorf("%w: row is missing r attribute", ErrWorksheetHasInvalidReferences)
		}
		rowNum, err := strconv.Atoi(rowText)
		if err != nil || rowNum < 1 || rowNum > address.MaxRow {
			return fmt.Errorf("%w: invalid row reference %q", ErrWorksheetHasInvalidReferences, rowText)
		}
		if _, exists := seenRows[rowNum]; exists {
			return fmt.Errorf("%w: duplicate row reference %d", ErrWorksheetHasInvalidReferences, rowNum)
		}
		seenRows[rowNum] = struct{}{}

		seenCells := make(map[string]struct{})
		for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
			refText := strings.TrimSpace(cell.SelectAttrValue("r", ""))
			if refText == "" {
				return fmt.Errorf("%w: cell in row %d is missing r attribute", ErrWorksheetHasInvalidReferences, rowNum)
			}
			ref, err := address.ParseCell(refText)
			if err != nil {
				return fmt.Errorf("%w: invalid cell reference %q", ErrWorksheetHasInvalidReferences, refText)
			}
			if ref.Row != rowNum {
				return fmt.Errorf("%w: cell %s is stored in row %d", ErrWorksheetHasInvalidReferences, ref.String(), rowNum)
			}
			refKey := cellRef(ref.Column, ref.Row)
			if _, exists := seenCells[refKey]; exists {
				return fmt.Errorf("%w: duplicate cell reference %s", ErrWorksheetHasInvalidReferences, refKey)
			}
			seenCells[refKey] = struct{}{}
		}
	}
	return nil
}

func workbookHasDefinedNames(session opc.PackageSession, workbookURI string) bool {
	doc, err := session.ReadXMLPart(workbookURI)
	if err != nil || doc.Root() == nil {
		return false
	}
	return namespaces.FindChild(doc.Root(), namespaces.NsSpreadsheetML, "definedNames") != nil
}

func insertRows(root *etree.Element, prefix string, sheetRef model.SheetRef, at, count int) (*StructureMutationResult, error) {
	sheetData := ensureSheetData(root, prefix)
	result := baseStructureResult(sheetRef, "rows", "insert", at, count)
	for _, row := range rowsDescending(sheetData) {
		rowNum, ok := rowNumber(row)
		if !ok || rowNum < at {
			continue
		}
		newRow, err := address.OffsetRow(rowNum, count)
		if err != nil {
			return nil, fmt.Errorf("%w: %v", ErrWorksheetStructureOutOfBounds, err)
		}
		setRowNumber(row, newRow)
		result.ShiftedRows++
		for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
			ref, ok := cellReference(cell)
			if !ok {
				continue
			}
			ref.Row = newRow
			if err := setCellReference(cell, ref); err != nil {
				return nil, err
			}
			result.ShiftedCells++
		}
		row.RemoveAttr("spans")
	}
	sortRows(sheetData)
	updateDimension(root, prefix)
	return result, nil
}

func deleteRows(root *etree.Element, prefix string, sheetRef model.SheetRef, rowStart, count int) (*StructureMutationResult, error) {
	sheetData := ensureSheetData(root, prefix)
	result := baseStructureResult(sheetRef, "rows", "delete", rowStart, count)
	rowEnd := rowStart + count - 1
	for _, row := range append([]*etree.Element(nil), namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row")...) {
		rowNum, ok := rowNumber(row)
		if !ok {
			continue
		}
		switch {
		case rowNum >= rowStart && rowNum <= rowEnd:
			result.RemovedRows++
			result.RemovedCells += len(namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c"))
			sheetData.RemoveChild(row)
		case rowNum > rowEnd:
			newRow, err := address.OffsetRow(rowNum, -count)
			if err != nil {
				return nil, fmt.Errorf("%w: %v", ErrWorksheetStructureOutOfBounds, err)
			}
			setRowNumber(row, newRow)
			result.ShiftedRows++
			for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
				ref, ok := cellReference(cell)
				if !ok {
					continue
				}
				ref.Row = newRow
				if err := setCellReference(cell, ref); err != nil {
					return nil, err
				}
				result.ShiftedCells++
			}
			row.RemoveAttr("spans")
		}
	}
	removeEmptyRows(sheetData)
	sortRows(sheetData)
	updateDimension(root, prefix)
	return result, nil
}

func insertColumns(root *etree.Element, prefix string, sheetRef model.SheetRef, at, count int) (*StructureMutationResult, error) {
	sheetData := ensureSheetData(root, prefix)
	result := baseStructureResult(sheetRef, "cols", "insert", at, count)
	result.StartColumn = columnName(at)
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		for _, cell := range cellsDescending(row) {
			ref, ok := cellReference(cell)
			if !ok || ref.Column < at {
				continue
			}
			shifted, err := address.OffsetCell(ref, 0, count)
			if err != nil {
				return nil, fmt.Errorf("%w: %v", ErrWorksheetStructureOutOfBounds, err)
			}
			if err := setCellReference(cell, shifted); err != nil {
				return nil, err
			}
			result.ShiftedCells++
		}
		row.RemoveAttr("spans")
		sortCells(row)
	}
	updateDimension(root, prefix)
	return result, nil
}

func deleteColumns(root *etree.Element, prefix string, sheetRef model.SheetRef, colStart, count int) (*StructureMutationResult, error) {
	sheetData := ensureSheetData(root, prefix)
	result := baseStructureResult(sheetRef, "cols", "delete", colStart, count)
	result.StartColumn = columnName(colStart)
	colEnd := colStart + count - 1
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		for _, cell := range append([]*etree.Element(nil), namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c")...) {
			ref, ok := cellReference(cell)
			if !ok {
				continue
			}
			switch {
			case ref.Column >= colStart && ref.Column <= colEnd:
				result.RemovedCells++
				row.RemoveChild(cell)
			case ref.Column > colEnd:
				shifted, err := address.OffsetCell(ref, 0, -count)
				if err != nil {
					return nil, fmt.Errorf("%w: %v", ErrWorksheetStructureOutOfBounds, err)
				}
				if err := setCellReference(cell, shifted); err != nil {
					return nil, err
				}
				result.ShiftedCells++
			}
		}
		row.RemoveAttr("spans")
		sortCells(row)
	}
	removeEmptyRows(sheetData)
	updateDimension(root, prefix)
	return result, nil
}

func baseStructureResult(sheetRef model.SheetRef, axis, operation string, start, count int) *StructureMutationResult {
	return &StructureMutationResult{
		Sheet:       sheetRef.Name,
		SheetNumber: sheetRef.Number,
		Axis:        axis,
		Operation:   operation,
		Start:       start,
		Count:       count,
	}
}

func currentUsedRange(root *etree.Element) string {
	if dimension := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dimension"); dimension != nil {
		return dimension.SelectAttrValue("ref", "")
	}
	return ""
}

func setRowNumber(row *etree.Element, rowNum int) {
	row.CreateAttr("r", fmt.Sprintf("%d", rowNum))
}

func setCellReference(cell *etree.Element, ref address.CellRef) error {
	value := cellRef(ref.Column, ref.Row)
	if value == "" {
		return fmt.Errorf("%w: invalid cell reference", ErrWorksheetStructureOutOfBounds)
	}
	cell.CreateAttr("r", value)
	return nil
}

func rowsDescending(sheetData *etree.Element) []*etree.Element {
	rows := append([]*etree.Element(nil), namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row")...)
	sort.Slice(rows, func(i, j int) bool {
		ri, _ := rowNumber(rows[i])
		rj, _ := rowNumber(rows[j])
		return ri > rj
	})
	return rows
}

func cellsDescending(row *etree.Element) []*etree.Element {
	cells := append([]*etree.Element(nil), namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c")...)
	sort.Slice(cells, func(i, j int) bool {
		ri, _ := cellReference(cells[i])
		rj, _ := cellReference(cells[j])
		return ri.Column > rj.Column
	})
	return cells
}

func sortRows(sheetData *etree.Element) {
	rows := append([]*etree.Element(nil), namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row")...)
	sort.SliceStable(rows, func(i, j int) bool {
		ri, _ := rowNumber(rows[i])
		rj, _ := rowNumber(rows[j])
		return ri < rj
	})
	for _, row := range rows {
		sheetData.RemoveChild(row)
	}
	for _, row := range rows {
		sheetData.AddChild(row)
	}
}

func sortCells(row *etree.Element) {
	cells := append([]*etree.Element(nil), namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c")...)
	sort.SliceStable(cells, func(i, j int) bool {
		ri, _ := cellReference(cells[i])
		rj, _ := cellReference(cells[j])
		if ri.Row != rj.Row {
			return ri.Row < rj.Row
		}
		return ri.Column < rj.Column
	})
	for _, cell := range cells {
		row.RemoveChild(cell)
	}
	for _, cell := range cells {
		addCellToRow(row, cell)
	}
}

func removeEmptyRows(sheetData *etree.Element) {
	for _, row := range append([]*etree.Element(nil), namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row")...) {
		if len(namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c")) == 0 {
			sheetData.RemoveChild(row)
		}
	}
}

func columnName(index int) string {
	value, _ := address.ColumnIndexToLetters(index)
	return value
}
