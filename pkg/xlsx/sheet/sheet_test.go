package sheet

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

func TestReadMinimalWorkbookCells(t *testing.T) {
	report := readFixtureSheet(t, "minimal-workbook", "")

	if report.UsedRange.Ref != "A1:C1" {
		t.Fatalf("used range = %q, want A1:C1", report.UsedRange.Ref)
	}
	if report.RowCount != 1 || report.CellCount != 3 {
		t.Fatalf("counts = rows %d cells %d, want 1/3", report.RowCount, report.CellCount)
	}
	if len(report.Rows) != 1 || len(report.Rows[0].Cells) != 3 {
		t.Fatalf("rows = %+v, want one row with three cells", report.Rows)
	}

	assertCell(t, report.Rows[0].Cells[0], "A1", model.CellTypeString, "Hello")
	assertCell(t, report.Rows[0].Cells[1], "B1", model.CellTypeNumber, "42")
	assertCell(t, report.Rows[0].Cells[2], "C1", model.CellTypeBoolean, "true")
}

func TestReadSharedStringsWorkbookCells(t *testing.T) {
	report := readFixtureSheet(t, "shared-strings", "Data")

	if report.Name != "Data" {
		t.Fatalf("sheet name = %q, want Data", report.Name)
	}
	if len(report.Rows) != 1 || len(report.Rows[0].Cells) != 1 {
		t.Fatalf("rows = %+v, want one cell", report.Rows)
	}
	assertCell(t, report.Rows[0].Cells[0], "A1", model.CellTypeString, "North")
}

func TestReadTypesAndFormulasWorkbookCells(t *testing.T) {
	report := readFixtureSheet(t, "types-and-formulas", "")

	if report.DimensionDeclared != "A1:H4" {
		t.Fatalf("declared dimension = %q, want A1:H4", report.DimensionDeclared)
	}
	if report.UsedRange.Ref != "A1:H4" {
		t.Fatalf("used range = %q, want A1:H4", report.UsedRange.Ref)
	}
	if report.RowCount != 3 || report.CellCount != 17 {
		t.Fatalf("counts = rows %d cells %d, want 3/17", report.RowCount, report.CellCount)
	}

	row2 := report.Rows[1].Cells
	assertCell(t, row2[0], "A2", model.CellTypeString, "North")
	assertCell(t, row2[1], "B2", model.CellTypeNumber, "1234.5")
	assertCell(t, row2[2], "C2", model.CellTypeBoolean, "true")
	assertCell(t, row2[3], "D2", model.CellTypeError, "#DIV/0!")
	assertCell(t, row2[4], "E2", model.CellTypeNumber, "2469")
	if row2[4].Formula != "B2*2" {
		t.Fatalf("E2 formula = %q, want B2*2", row2[4].Formula)
	}
	assertCell(t, row2[5], "F2", model.CellTypeString, "North region")
	if row2[5].Formula != `CONCAT(A2," region")` {
		t.Fatalf("F2 formula = %q, want CONCAT(A2,\" region\")", row2[5].Formula)
	}
	assertCell(t, row2[6], "G2", model.CellTypeString, "Inline value")
	assertCell(t, row2[7], "H2", model.CellTypeDate, "45292")
	if !row2[7].DateStyle || row2[7].StyleIndex != 1 || row2[7].NumberFormatID != 14 || row2[7].NumberFormatCode != "m/d/yy" {
		t.Fatalf("H2 date style fields = %+v, want DateStyle=true StyleIndex=1 NumberFormatID=14 NumberFormatCode=m/d/yy", row2[7])
	}
}

func TestReadSharedStringRunsWorkbookCells(t *testing.T) {
	report := readFixtureSheet(t, "shared-string-runs", "")

	assertCell(t, report.Rows[0].Cells[0], "A1", model.CellTypeString, "Rich Text")
	assertCell(t, report.Rows[1].Cells[0], "A2", model.CellTypeString, " padded ")
}

func TestReadUsedRangeIgnoresStaleDeclaredDimension(t *testing.T) {
	report := readFixtureSheet(t, "used-range", "")

	if report.DimensionDeclared != "A1:Z100" {
		t.Fatalf("declared dimension = %q, want A1:Z100", report.DimensionDeclared)
	}
	if report.UsedRange.Ref != "B2:D5" {
		t.Fatalf("used range = %q, want B2:D5", report.UsedRange.Ref)
	}
	if report.UsedRange.Rows != 4 || report.UsedRange.Cols != 3 {
		t.Fatalf("used dimensions = %d x %d, want 4 x 3", report.UsedRange.Rows, report.UsedRange.Cols)
	}
}

func TestReadSharedFormulaWorkbookCells(t *testing.T) {
	report := readFixtureSheet(t, "shared-formula", "")

	firstFormula := report.Rows[0].Cells[1]
	secondFormula := report.Rows[1].Cells[1]
	assertCell(t, firstFormula, "B1", model.CellTypeNumber, "2")
	if firstFormula.Formula != "A1*2" {
		t.Fatalf("B1 formula = %q, want A1*2", firstFormula.Formula)
	}
	assertCell(t, secondFormula, "B2", model.CellTypeNumber, "4")
	if secondFormula.Formula != "" {
		t.Fatalf("B2 formula = %q, want empty shared follower formula text", secondFormula.Formula)
	}
}

func TestReadDenseRowsUsesDefaultCellLimit(t *testing.T) {
	rangeRef, err := address.ParseRange("A1:Z1000")
	if err != nil {
		t.Fatalf("ParseRange returned error: %v", err)
	}
	report := readFixtureSheetWithOptions(t, "used-range", "", ReadOptions{
		Range:        &rangeRef,
		IncludeData:  true,
		IncludeEmpty: true,
	})

	emitted := 0
	for _, row := range report.Rows {
		emitted += len(row.Cells)
	}
	if emitted != DefaultDenseCellLimit {
		t.Fatalf("emitted dense cells = %d, want default cap %d", emitted, DefaultDenseCellLimit)
	}
	if !report.Truncated {
		t.Fatal("truncated = false, want true")
	}
}

func readFixtureSheet(t *testing.T, fixtureDir, sheetName string) *model.SheetReport {
	t.Helper()
	return readFixtureSheetWithOptions(t, fixtureDir, sheetName, ReadOptions{IncludeData: true})
}

func readFixtureSheetWithOptions(t *testing.T, fixtureDir, sheetName string, opts ReadOptions) *model.SheetReport {
	t.Helper()

	path := filepath.Join("..", "..", "..", "testdata", "xlsx", fixtureDir, "workbook.xlsx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	ctx, err := LoadContext(pkg, workbook)
	if err != nil {
		t.Fatalf("LoadContext returned error: %v", err)
	}

	ref := workbook.Sheets[0]
	if sheetName != "" {
		for _, sheet := range workbook.Sheets {
			if sheet.Name == sheetName {
				ref = sheet
				break
			}
		}
	}
	report, err := Read(pkg, ref, ctx, opts)
	if err != nil {
		t.Fatalf("Read returned error: %v", err)
	}
	return report
}

func assertCell(t *testing.T, cell model.Cell, ref string, typ model.CellDataType, value string) {
	t.Helper()
	if cell.Ref != ref || cell.Type != typ || cell.Value != value {
		t.Fatalf("cell = %+v, want ref=%s type=%s value=%q", cell, ref, typ, value)
	}
}
