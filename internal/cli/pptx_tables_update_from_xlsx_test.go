package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestPPTXTablesUpdateFromXLSXRangeJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(B1:C1)</f><v>7</v></c><c r="B1" t="inlineStr"><is><t>Header B</t></is></c><c r="C1" t="inlineStr"><is><t>Header C</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>55</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "update-from-xlsx.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "update-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:C3",
		"--formula-mode", "formula",
		"--expect-source-range", "A1:C3",
		"--slide", "2",
		"--target", "table:1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables update-from-xlsx failed: %v", err)
	}

	var result PPTXTablesUpdateFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal update-from-xlsx JSON: %v\n%s", err, output)
	}
	if result.File != presentationPath || result.Output != outPath {
		t.Fatalf("unexpected file metadata: %+v", result)
	}
	if result.Source.Workbook != workbookPath || result.Source.Sheet != "Sheet1" || result.Source.Range != "A1:C3" || result.Source.Rows != 3 || result.Source.Cols != 3 || result.Source.FormulaCount != 1 {
		t.Fatalf("unexpected source metadata: %+v", result.Source)
	}
	if result.Update.FormulaMode != "formula" || result.Update.UpdatedCells != 9 || result.Update.ChangedCells != 9 {
		t.Fatalf("unexpected update metadata: %+v", result.Update)
	}
	if result.Destination.ShapeID != 2 || result.Destination.PrimarySelector != "table:1" || result.Destination.Rows != 3 || result.Destination.Cols != 3 {
		t.Fatalf("unexpected destination metadata: %+v", result.Destination)
	}
	if result.Destination.File != outPath {
		t.Fatalf("destination file = %q, want %q", result.Destination.File, outPath)
	}
	if got := result.Destination.Cells[0][0]; got != "=SUM(B1:C1)" {
		t.Fatalf("destination cell A1 = %q, want formula text", got)
	}
	if got := result.Destination.Cells[1][1]; got != "42" {
		t.Fatalf("destination cell B2 = %q, want 42", got)
	}

	readbackOutput := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readbackOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal generated table readback: %v\n%s", err, readbackOutput)
	}
	if len(showResult.Tables) != 1 {
		t.Fatalf("generated readback table count = %d, want 1", len(showResult.Tables))
	}
	readback := showResult.Tables[0]
	if got := readback.Cells[0][0]; got != "=SUM(B1:C1)" {
		t.Fatalf("readback cell A1 = %q, want formula text", got)
	}
	if !containsString(readback.Selectors, "table:1") {
		t.Fatalf("readback selectors missing table:1: %+v", readback.Selectors)
	}
}

func TestPPTXTablesUpdateFromXLSXNamedTableDryRun(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writeTestXLSXWithTableColumns(t, "A1:C3", false, "", []string{"Region", "Amount", "Note"})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "update-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--table", "Sales",
		"--slide", "2",
		"--target", "table:1",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx tables update-from-xlsx named table dry-run failed: %v", err)
	}

	var result PPTXTablesUpdateFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run update JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected dry-run output metadata: %+v", result)
	}
	if result.Source.Table != "Sales" || result.Source.Sheet != "Data" || result.Source.Range != "A1:C3" || result.Source.Rows != 3 || result.Source.Cols != 3 {
		t.Fatalf("unexpected table source: %+v", result.Source)
	}
	if got := result.Destination.Cells[1][0]; got != "East" {
		t.Fatalf("dry-run destination cell A2 = %q, want East", got)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx tables show")

	original := readPPTXTableSummaryForTest(t, presentationPath)
	if got := original.Cells[1][0]; got != "R1C0" {
		t.Fatalf("dry-run wrote to source presentation: %q", got)
	}
}

func TestPPTXTablesUpdateFromXLSXRejectsBadArgs(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	mergedPath := getPPTXTableTestFilePath("table-merged")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D4"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>A</t></is></c><c r="B1" t="inlineStr"><is><t>B</t></is></c><c r="C1" t="inlineStr"><is><t>C</t></is></c><c r="D1" t="inlineStr"><is><t>D</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>E</t></is></c><c r="B2" t="inlineStr"><is><t>F</t></is></c><c r="C2" t="inlineStr"><is><t>G</t></is></c><c r="D2" t="inlineStr"><is><t>H</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>I</t></is></c><c r="B3" t="inlineStr"><is><t>J</t></is></c><c r="C3" t="inlineStr"><is><t>K</t></is></c><c r="D3" t="inlineStr"><is><t>L</t></is></c></row>
    <row r="4"><c r="A4" t="inlineStr"><is><t>M</t></is></c><c r="B4" t="inlineStr"><is><t>N</t></is></c><c r="C4" t="inlineStr"><is><t>O</t></is></c><c r="D4" t="inlineStr"><is><t>P</t></is></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "bad-update.pptx")

	tests := []struct {
		args []string
		code int
		want string
	}{
		{[]string{"pptx", "tables", "update-from-xlsx", presentationPath, "--workbook", workbookPath, "--range", "A1:C3", "--table", "Sales", "--slide", "2", "--target", "table:1", "--dry-run"}, ExitInvalidArgs, "only one"},
		{[]string{"pptx", "tables", "update-from-xlsx", presentationPath, "--workbook", workbookPath, "--range", "A1:C3", "--slide", "2", "--target", "table:1", "--dry-run"}, ExitInvalidArgs, "--sheet is required"},
		{[]string{"pptx", "tables", "update-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C3", "--max-cells", "1", "--slide", "2", "--target", "table:1", "--dry-run"}, ExitInvalidArgs, "above --max-cells"},
		{[]string{"pptx", "tables", "update-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C3", "--expect-source-range", "A1:B3", "--slide", "2", "--target", "table:1", "--dry-run"}, ExitInvalidArgs, "expect-source-range mismatch"},
		{[]string{"pptx", "tables", "update-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--slide", "2", "--target", "table:1", "--dry-run"}, ExitInvalidArgs, "dimension mismatch"},
		{[]string{"pptx", "tables", "update-from-xlsx", mergedPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:D4", "--slide", "2", "--target", "table:1", "--dry-run"}, ExitInvalidArgs, "merged"},
		{[]string{"pptx", "tables", "update-from-xlsx", filepath.Join(getTestdataPath(), "pptx", "title-content", "presentation.pptx"), "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--slide", "2", "--target", "body", "--out", outPath}, ExitInvalidArgs, "not a table"},
	}

	for _, tt := range tests {
		_, err := executeRootForXLSXTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
		if tt.want != "" && !strings.Contains(err.Error(), tt.want) {
			t.Fatalf("%v: error = %v, want containing %q", tt.args, err, tt.want)
		}
	}
}
