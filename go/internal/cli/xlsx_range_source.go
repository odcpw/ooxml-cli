package cli

import (
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
)

type XLSXRangeSource struct {
	Workbook     string `json:"workbook"`
	Sheet        string `json:"sheet"`
	SheetNumber  int    `json:"sheetNumber"`
	Range        string `json:"range"`
	Table        string `json:"table,omitempty"`
	Rows         int    `json:"rows"`
	Cols         int    `json:"cols"`
	FormulaCount int    `json:"formulaCount"`
}

func readXLSXRangeSourceForCLI(workbookPath, sheetSelector string, rangeRef address.RangeRef) (*XLSXRangeSource, [][]rangeio.Cell, error) {
	pkg, err := openPackageExpectType(workbookPath, opc.PackageTypeXLSX)
	if err != nil {
		return nil, nil, err
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
	if err != nil {
		return nil, nil, err
	}
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		return nil, nil, err
	}
	ctx, err := xlsxsheet.LoadContext(pkg, workbook)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to load workbook context: %v", err)
	}

	totalCells := xlsxRangeCellCount(rangeRef)
	if totalCells > int64(^uint(0)>>1) {
		return nil, nil, NewCLIErrorf(ExitInvalidArgs, "range %s is too large to read on this platform", rangeRef.String())
	}
	report, err := xlsxsheet.Read(pkg, sheetRef, ctx, xlsxsheet.ReadOptions{
		Range:        &rangeRef,
		MaxCells:     int(totalCells),
		IncludeEmpty: true,
		IncludeData:  true,
	})
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to read sheet %q: %v", sheetRef.Name, err)
	}

	rows, cols := xlsxRangeDimensions(rangeRef)
	matrix := xlsxRangeCellsFromRows(report.Rows)
	source := &XLSXRangeSource{
		Workbook:     workbookPath,
		Sheet:        sheetRef.Name,
		SheetNumber:  sheetRef.Number,
		Range:        rangeRef.String(),
		Rows:         rows,
		Cols:         cols,
		FormulaCount: rangeio.FormulaCount(matrix),
	}
	return source, matrix, nil
}

func loadXLSXRangeOrTableSourceForCLI(workbookPath, sheetSelector, rangeSelector, tableSelector string, maxCells int) (*XLSXRangeSource, [][]rangeio.Cell, error) {
	sourceSheet := strings.TrimSpace(sheetSelector)
	sourceRange := strings.TrimSpace(rangeSelector)
	sourceTable := strings.TrimSpace(tableSelector)
	if sourceRange != "" && sourceTable != "" {
		return nil, nil, InvalidArgsError("specify only one of --range or --table")
	}
	if sourceRange == "" && sourceTable == "" {
		return nil, nil, InvalidArgsError("must specify --range or --table")
	}
	if sourceTable != "" {
		tableRef, err := resolveXLSXTableForCLI(workbookPath, sourceSheet, sourceTable)
		if err != nil {
			return nil, nil, err
		}
		sourceSheet = tableRef.Sheet
		sourceRange = tableRef.Range
		sourceTable = tableRef.DisplayName
	}
	if sourceSheet == "" {
		return nil, nil, InvalidArgsError("--sheet is required when using --range")
	}
	rangeRef, err := address.ParseRange(sourceRange)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
	}
	if err := checkXLSXRangeMaxCells(rangeRef, maxCells); err != nil {
		return nil, nil, err
	}

	source, matrix, err := readXLSXRangeSourceForCLI(workbookPath, sourceSheet, rangeRef)
	if err != nil {
		return nil, nil, err
	}
	source.Table = sourceTable
	return source, matrix, nil
}

func xlsxRangeStringsFromMatrix(matrix [][]rangeio.Cell, formulaMode string) [][]string {
	out := make([][]string, len(matrix))
	for rowIdx, row := range matrix {
		out[rowIdx] = make([]string, len(row))
		for colIdx, cell := range row {
			out[rowIdx][colIdx] = xlsxRangeStringFromCell(cell, formulaMode)
		}
	}
	return out
}

func xlsxRangeStringFromCell(cell rangeio.Cell, formulaMode string) string {
	if cell.Null {
		return ""
	}
	if formulaMode == "formula" && cell.Formula != "" {
		if strings.HasPrefix(cell.Formula, "=") {
			return cell.Formula
		}
		return "=" + cell.Formula
	}
	return cell.Value
}

func normalizeXLSXFormulaMode(value string, flagName string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", "value":
		return "value", nil
	case "formula":
		return "formula", nil
	default:
		return "", InvalidArgsError(flagName + " must be value or formula")
	}
}
