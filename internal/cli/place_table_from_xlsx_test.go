package cli

import (
	"encoding/json"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func TestPPTXPlaceTableFromXLSXCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	place := findSubcommand(pptx, "place")
	if place == nil {
		t.Fatal("pptx place command is not registered")
	}
	if command := findSubcommand(place, "table-from-xlsx"); command == nil {
		t.Fatal("pptx place table-from-xlsx command is not registered")
	}
}

func TestPPTXPlaceTableFromXLSXRangeJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("minimal-title")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "table-from-xlsx.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "place", "table-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B2",
		"--expect-source-range", "A1:B2",
		"--slide", "1",
		"--x", "0",
		"--y", "0",
		"--cx", "3000000",
		"--header",
		"--name", "Revenue Table",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx place table-from-xlsx failed: %v", err)
	}

	var result PlaceTableFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, output)
	}
	if result.Source.Workbook != workbookPath || result.Source.Sheet != "Sheet1" || result.Source.Range != "A1:B2" || result.Source.Rows != 2 || result.Source.Cols != 2 {
		t.Fatalf("unexpected source metadata: %+v", result.Source)
	}
	if result.Output != outPath || result.Destination.File != outPath || result.Destination.Slide != 1 || result.Destination.ShapeName != "Revenue Table" {
		t.Fatalf("unexpected destination metadata: %+v", result.Destination)
	}
	if result.Destination.PrimarySelector == "" || len(result.Destination.Selectors) == 0 {
		t.Fatalf("destination missing selectors: %+v", result.Destination)
	}
	if got := result.Destination.Cells[1][1]; got != "42" {
		t.Fatalf("destination JSON cell B2 = %q, want 42", got)
	}

	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if len(showResult.Tables) != 1 {
		t.Fatalf("readback table count = %d, want 1", len(showResult.Tables))
	}
	if got := showResult.Tables[0].Cells[1][1]; got != "42" {
		t.Fatalf("table cell B2 = %q, want 42", got)
	}
	if !containsString(showResult.Tables[0].Selectors, result.Destination.PrimarySelector) {
		t.Fatalf("readback selectors missing primary selector: %+v", showResult.Tables[0].Selectors)
	}
}

func TestPPTXPlaceTableFromXLSXNamedTable(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("minimal-title")
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")
	outPath := filepath.Join(t.TempDir(), "table-from-xlsx-table.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "place", "table-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--table", "Sales",
		"--expect-source-range", "A1:B3",
		"--slide", "1",
		"--x", "0",
		"--y", "0",
		"--cx", "3000000",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx place table-from-xlsx table failed: %v", err)
	}
	var result PlaceTableFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, output)
	}
	if result.Source.Table != "Sales" || result.Source.Sheet != "Data" || result.Source.Range != "A1:B3" || result.Source.Rows != 3 || result.Source.Cols != 2 {
		t.Fatalf("unexpected table source: %+v", result.Source)
	}
	if len(result.Destination.Cells) != 3 || len(result.Destination.Cells[0]) != 2 {
		t.Fatalf("unexpected destination cell readback: %+v", result.Destination.Cells)
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")
}

func TestPPTXPlaceTableFromXLSXDryRunTemplates(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("minimal-title")
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "place", "table-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--table", "Sales",
		"--expect-source-range", "A1:B3",
		"--slide", "1",
		"--x", "0",
		"--y", "0",
		"--cx", "3000000",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx place table-from-xlsx dry-run failed: %v", err)
	}
	var result PlaceTableFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Destination.File != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}
	if result.Destination.PrimarySelector == "" || len(result.Destination.Cells) != 3 {
		t.Fatalf("unexpected dry-run destination: %+v", result.Destination)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx tables show")
}

func TestPPTXPlaceTableFromXLSXFormulaMode(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("minimal-title")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:A1"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(B1:C1)</f><v>7</v></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "formula-value.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "place", "table-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:A1",
		"--slide", "1",
		"--x", "0",
		"--y", "0",
		"--cx", "1000000",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx place table-from-xlsx value formula mode failed: %v", err)
	}
	var result PlaceTableFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, output)
	}
	if result.Source.FormulaCount != 1 {
		t.Fatalf("formulaCount = %d, want 1", result.Source.FormulaCount)
	}
	assertPPTXTableCellForXLSXTest(t, outPath, result.Destination.ShapeID, "7")

	formulaOutPath := filepath.Join(t.TempDir(), "formula-text.pptx")
	formulaOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "place", "table-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:A1",
		"--formula-mode", "formula",
		"--slide", "1",
		"--x", "0",
		"--y", "0",
		"--cx", "1000000",
		"--out", formulaOutPath,
	)
	if err != nil {
		t.Fatalf("pptx place table-from-xlsx formula mode failed: %v", err)
	}
	var formulaResult PlaceTableFromXLSXResult
	if err := json.Unmarshal([]byte(formulaOutput), &formulaResult); err != nil {
		t.Fatalf("failed to unmarshal formula JSON: %v\n%s", err, formulaOutput)
	}
	assertPPTXTableCellForXLSXTest(t, formulaOutPath, formulaResult.Destination.ShapeID, "=SUM(B1:C1)")
}

func TestPPTXPlaceTableFromXLSXRejectsBadArgs(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("minimal-title")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row></sheetData>
</worksheet>`)
	tableWorkbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")
	outPath := filepath.Join(t.TempDir(), "bad.pptx")

	tests := []struct {
		args []string
		code int
		want string
	}{
		{[]string{"pptx", "place", "table-from-xlsx", presentationPath, "--workbook", workbookPath, "--range", "A1", "--slide", "1", "--cx", "1000", "--dry-run"}, ExitInvalidArgs, "--sheet is required"},
		{[]string{"pptx", "place", "table-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--table", "Sales", "--slide", "1", "--cx", "1000", "--dry-run"}, ExitInvalidArgs, "only one"},
		{[]string{"pptx", "place", "table-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--slide", "1", "--cx", "1000", "--max-cells", "1", "--dry-run"}, ExitInvalidArgs, "above --max-cells"},
		{[]string{"pptx", "place", "table-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--expect-source-range", "A1:C2", "--slide", "1", "--cx", "1000", "--dry-run"}, ExitInvalidArgs, "expect-source-range mismatch"},
		{[]string{"pptx", "place", "table-from-xlsx", presentationPath, "--workbook", tableWorkbookPath, "--table", "Sales", "--expect-source-range", "A1:B4", "--slide", "1", "--cx", "1000", "--dry-run"}, ExitInvalidArgs, "expect-source-range mismatch"},
		{[]string{"pptx", "place", "table-from-xlsx", workbookPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--slide", "1", "--cx", "1000", "--out", outPath}, ExitUnsupportedType, ""},
	}

	for _, tt := range tests {
		_, err := executeRootForXLSXTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
		if tt.want != "" && !strings.Contains(err.Error(), tt.want) {
			t.Fatalf("%v: error = %v, want containing %q", tt.args, err, tt.want)
		}
	}
}

func assertPPTXTableCellForXLSXTest(t *testing.T, path string, shapeID int, want string) {
	t.Helper()

	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", path,
		"--slide", "1",
		"--table-id", strconv.Itoa(shapeID),
	)
	if err != nil {
		t.Fatalf("pptx tables show readback failed: %v", err)
	}
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if len(showResult.Tables) != 1 || len(showResult.Tables[0].Cells) != 1 || len(showResult.Tables[0].Cells[0]) != 1 {
		t.Fatalf("unexpected readback shape: %+v", showResult.Tables)
	}
	if got := showResult.Tables[0].Cells[0][0]; got != want {
		t.Fatalf("table cell A1 = %q, want %q", got, want)
	}
}
