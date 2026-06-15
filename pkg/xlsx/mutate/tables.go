package mutate

import (
	"errors"
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

var (
	ErrTableHasTotals              = errors.New("table has totals rows")
	ErrTableHasCalculatedColumns   = errors.New("table has calculated columns")
	ErrTableHasUnsupportedFeatures = errors.New("table has unsupported features")
	ErrTableColumnCountMismatch    = errors.New("table column count mismatch")
	ErrTableAppendWouldOverwrite   = errors.New("table append would overwrite existing cells")
	ErrTableColumnNotFound         = errors.New("table column not found")
	ErrTableColumnHasNoDataRows    = errors.New("table column has no data rows")
)

// ResolveTableColumnDataRange resolves a table column name to its data-body
// range, excluding the header rows and any totals rows. The column is matched
// case-sensitively against tableRef.Columns; its position in that slice yields
// the column offset from the table's leftmost column.
func ResolveTableColumnDataRange(tableRef model.TableRef, columnName string) (address.RangeRef, error) {
	tableRange, err := address.ParseRange(tableRef.Range)
	if err != nil {
		return address.RangeRef{}, fmt.Errorf("invalid table ref %q: %w", tableRef.Range, err)
	}
	colIndex := -1
	for idx, col := range tableRef.Columns {
		if col.Name == columnName {
			colIndex = idx
			break
		}
	}
	if colIndex < 0 {
		return address.RangeRef{}, fmt.Errorf("%w: %q in table %q", ErrTableColumnNotFound, columnName, tableRef.DisplayName)
	}

	minCol, minRow, maxCol, maxRow := tableRange.Bounds()
	absCol := minCol + colIndex
	if absCol > maxCol {
		return address.RangeRef{}, fmt.Errorf("%w: %q resolves outside table range %s", ErrTableColumnNotFound, columnName, tableRef.Range)
	}

	headerRows := tableRef.HeaderRowCount
	if headerRows < 0 {
		headerRows = 0
	}
	totalsRows := tableRef.TotalsRowCount
	if totalsRows < 0 {
		totalsRows = 0
	}
	startRow := minRow + headerRows
	endRow := maxRow - totalsRows
	if endRow < startRow {
		return address.RangeRef{}, fmt.Errorf("%w: column %q in table %q", ErrTableColumnHasNoDataRows, columnName, tableRef.DisplayName)
	}

	return address.RangeRef{
		Start: address.CellRef{Column: absCol, Row: startRow},
		End:   address.CellRef{Column: absCol, Row: endRow},
	}, nil
}

type AppendTableRowsRequest struct {
	Package           opc.PackageSession
	WorkbookURI       string
	Table             model.TableRef
	Rows              [][]RangeCell
	NullPolicy        RangeNullPolicy
	OverwriteFormulas bool
}

type AppendTableRowsResult struct {
	Table         string `json:"table"`
	Sheet         string `json:"sheet"`
	SheetNumber   int    `json:"sheetNumber"`
	PreviousRange string `json:"previousRange"`
	Range         string `json:"range"`
	AppendRange   string `json:"appendRange"`
	RowsAppended  int    `json:"rowsAppended"`
	Updated       int    `json:"updated"`
	Created       int    `json:"created"`
	Cleared       int    `json:"cleared"`
	Skipped       int    `json:"skipped"`
	FormulaCount  int    `json:"formulaCount"`
}

func AppendTableRows(req *AppendTableRowsRequest) (*AppendTableRowsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("append table rows request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.Table.PartURI == "" {
		return nil, fmt.Errorf("table part URI is empty")
	}
	if req.Table.SheetPartURI == "" {
		return nil, fmt.Errorf("table %q has no worksheet part URI", req.Table.DisplayName)
	}
	if len(req.Rows) == 0 {
		return nil, fmt.Errorf("rows matrix cannot be empty")
	}

	tableDoc, err := req.Package.ReadXMLPart(req.Table.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read table part %s: %w", req.Table.PartURI, err)
	}
	tableRoot := tableDoc.Root()
	if tableRoot == nil || !namespaces.IsElement(tableRoot, namespaces.NsSpreadsheetML, "table") {
		return nil, fmt.Errorf("table part %s root element not found", req.Table.PartURI)
	}
	if err := rejectUnsupportedTableAppendFeatures(tableRoot); err != nil {
		return nil, err
	}

	tableRange, err := address.ParseRange(tableRoot.SelectAttrValue("ref", ""))
	if err != nil {
		return nil, fmt.Errorf("invalid table ref %q: %w", tableRoot.SelectAttrValue("ref", ""), err)
	}
	minCol, minRow, maxCol, maxRow := tableRange.Bounds()
	cols := maxCol - minCol + 1
	if err := validateAppendTableRows(req.Rows, cols); err != nil {
		return nil, err
	}
	if maxRow+len(req.Rows) > address.MaxRow {
		return nil, fmt.Errorf("table append exceeds XLSX max row %d", address.MaxRow)
	}

	appendRange := address.RangeRef{
		Start: address.CellRef{Column: minCol, Row: maxRow + 1},
		End:   address.CellRef{Column: maxCol, Row: maxRow + len(req.Rows)},
	}
	if err := rejectAppendOverwrite(req.Package, req.Table.SheetPartURI, appendRange); err != nil {
		return nil, err
	}

	setResult, err := SetRange(&SetRangeRequest{
		Package:           req.Package,
		WorkbookURI:       req.WorkbookURI,
		SheetRef:          model.SheetRef{Name: req.Table.Sheet, Number: req.Table.SheetNumber, PartURI: req.Table.SheetPartURI, RelationshipType: namespaces.RelWorksheet},
		Range:             appendRange,
		Rows:              req.Rows,
		NullPolicy:        req.NullPolicy,
		OverwriteFormulas: req.OverwriteFormulas,
	})
	if err != nil {
		return nil, err
	}

	newRange := address.RangeRef{
		Start: address.CellRef{Column: minCol, Row: minRow},
		End:   address.CellRef{Column: maxCol, Row: maxRow + len(req.Rows)},
	}
	newRangeText := newRange.String()
	tableRoot.CreateAttr("ref", newRangeText)
	if autoFilter := namespaces.FindChild(tableRoot, namespaces.NsSpreadsheetML, "autoFilter"); autoFilter != nil {
		autoFilter.CreateAttr("ref", newRangeText)
	}
	if err := req.Package.ReplaceXMLPart(req.Table.PartURI, tableDoc); err != nil {
		return nil, fmt.Errorf("failed to replace table part %s: %w", req.Table.PartURI, err)
	}

	return &AppendTableRowsResult{
		Table:         req.Table.DisplayName,
		Sheet:         req.Table.Sheet,
		SheetNumber:   req.Table.SheetNumber,
		PreviousRange: tableRange.String(),
		Range:         newRangeText,
		AppendRange:   appendRange.String(),
		RowsAppended:  len(req.Rows),
		Updated:       setResult.Updated,
		Created:       setResult.Created,
		Cleared:       setResult.Cleared,
		Skipped:       setResult.Skipped,
		FormulaCount:  setResult.FormulaCount,
	}, nil
}

func rejectUnsupportedTableAppendFeatures(root *etree.Element) error {
	if parseBoolish(root.SelectAttrValue("totalsRowShown", "")) || parsePositiveInt(root.SelectAttrValue("totalsRowCount", "")) {
		return ErrTableHasTotals
	}
	tableType := root.SelectAttrValue("tableType", "")
	if tableType != "" && tableType != "worksheet" {
		return fmt.Errorf("%w: tableType=%s", ErrTableHasUnsupportedFeatures, tableType)
	}
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "extLst") != nil {
		return fmt.Errorf("%w: extLst", ErrTableHasUnsupportedFeatures)
	}
	if autoFilter := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter"); autoFilter != nil {
		if namespaces.FindChild(autoFilter, namespaces.NsSpreadsheetML, "sortState") != nil {
			return fmt.Errorf("%w: sortState", ErrTableHasUnsupportedFeatures)
		}
	}
	tableColumns := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "tableColumns")
	if tableColumns == nil {
		return fmt.Errorf("%w: missing tableColumns", ErrTableHasUnsupportedFeatures)
	}
	for _, col := range namespaces.FindChildren(tableColumns, namespaces.NsSpreadsheetML, "tableColumn") {
		if namespaces.FindChild(col, namespaces.NsSpreadsheetML, "calculatedColumnFormula") != nil {
			return ErrTableHasCalculatedColumns
		}
	}
	return nil
}

func validateAppendTableRows(rows [][]RangeCell, cols int) error {
	if cols <= 0 {
		return fmt.Errorf("%w: table must have at least one column", ErrTableColumnCountMismatch)
	}
	for idx, row := range rows {
		if len(row) != cols {
			return fmt.Errorf("%w: row %d has %d columns, want %d", ErrTableColumnCountMismatch, idx+1, len(row), cols)
		}
	}
	return nil
}

func rejectAppendOverwrite(session opc.PackageSession, sheetPartURI string, target address.RangeRef) error {
	doc, err := session.ReadXMLPart(sheetPartURI)
	if err != nil {
		return fmt.Errorf("failed to read worksheet %s: %w", sheetPartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return fmt.Errorf("worksheet part %s root element not found", sheetPartURI)
	}
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	if sheetData == nil {
		return nil
	}
	minCol, minRow, maxCol, maxRow := target.Bounds()
	for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		rowNum, ok := rowNumber(row)
		if !ok || rowNum < minRow || rowNum > maxRow {
			continue
		}
		for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
			ref, ok := cellReference(cell)
			if !ok || ref.Column < minCol || ref.Column > maxCol {
				continue
			}
			if cellHasContent(cell) {
				return fmt.Errorf("%w: %s", ErrTableAppendWouldOverwrite, ref.String())
			}
		}
	}
	return nil
}

func cellHasContent(cell *etree.Element) bool {
	return namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "v") != nil ||
		namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "f") != nil ||
		namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "is") != nil
}

func parseBoolish(value string) bool {
	return value == "1" || value == "true"
}

func parsePositiveInt(value string) bool {
	if value == "" {
		return false
	}
	for _, ch := range value {
		if ch < '0' || ch > '9' {
			return false
		}
	}
	return value != "0"
}
