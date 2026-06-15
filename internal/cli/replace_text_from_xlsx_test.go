package cli

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"unicode/utf8"
)

func TestPPTXReplaceTextFromXLSXCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	replace := findSubcommand(pptx, "replace")
	if replace == nil {
		t.Fatal("pptx replace command is not registered")
	}
	if command := findSubcommand(replace, "text-from-xlsx"); command == nil {
		t.Fatal("pptx replace text-from-xlsx command is not registered")
	}
	if command := findSubcommand(replace, "text-map-from-xlsx"); command == nil {
		t.Fatal("pptx replace text-map-from-xlsx command is not registered")
	}
}

func TestPPTXReplaceTextFromXLSXRangeJSONReadbackAndValidate(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>Nörd</t></is></c><c r="B2"><v>42</v></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "text-from-xlsx.pptx")

	output, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "replace", "text-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B2",
		"--slide", "1",
		"--target", "title",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx replace text-from-xlsx failed: %v", err)
	}

	var result ReplaceTextFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, output)
	}
	wantText := "Region\tAmount\nNörd\t42"
	if result.Source.Workbook != workbookPath || result.Source.Sheet != "Sheet1" || result.Source.Range != "A1:B2" || result.Source.Rows != 2 || result.Source.Cols != 2 {
		t.Fatalf("unexpected source metadata: %+v", result.Source)
	}
	if result.Text.Value != wantText || result.Text.Chars != utf8.RuneCountInString(wantText) || result.Text.FormulaMode != "value" {
		t.Fatalf("unexpected text result: %+v", result.Text)
	}
	if result.Output != outPath || result.Destination.File != outPath || result.Destination.PrimarySelector != "title" || !containsString(result.Destination.Selectors, "shape:2") {
		t.Fatalf("unexpected destination metadata: %+v", result.Destination)
	}

	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal shapes readback: %v\n%s", err, readback)
	}
	if len(shapes.Shapes) != 1 || !strings.Contains(shapes.Shapes[0].TextPreview, "Nörd") {
		t.Fatalf("unexpected shape readback: %+v", shapes.Shapes)
	}
}

func TestPPTXReplaceTextFromXLSXFormulaModeAndSeparators(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(C1:D1)</f><v>7</v></c><c r="B1" t="inlineStr"><is><t>done</t></is></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "text-from-xlsx-formula.pptx")

	output, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "replace", "text-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B1",
		"--formula-mode", "formula",
		"--col-sep", " | ",
		"--slide", "1",
		"--target", "title",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx replace text-from-xlsx formula mode failed: %v", err)
	}
	var result ReplaceTextFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, output)
	}
	if result.Source.FormulaCount != 1 || result.Text.Value != "=SUM(C1:D1) | done" {
		t.Fatalf("unexpected formula result: source=%+v text=%+v", result.Source, result.Text)
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
}

func TestPPTXReplaceTextFromXLSXDryRunTemplates(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:A1"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Dry run title</t></is></c></row>
  </sheetData>
</worksheet>`)

	output, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "replace", "text-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1",
		"--slide", "1",
		"--target", "title",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx replace text-from-xlsx dry-run failed: %v", err)
	}
	var result ReplaceTextFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Destination.File != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}
	if result.Destination.PrimarySelector != "title" || !strings.Contains(result.Destination.TextPreview, "Dry run title") {
		t.Fatalf("unexpected dry-run destination: %+v", result.Destination)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx shapes get")

	readback, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "shapes", "get", presentationPath,
		"--slide", "1",
		"--target", "title",
		"--include-text",
	)
	if err != nil {
		t.Fatalf("pptx shapes get source readback failed: %v", err)
	}
	if strings.Contains(readback, "Dry run title") {
		t.Fatalf("dry-run wrote to source presentation:\n%s", readback)
	}
}

func TestPPTXReplaceTextFromXLSXRejectsBadArgs(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row></sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "bad.pptx")

	tests := []struct {
		args []string
		code int
		want string
	}{
		{[]string{"pptx", "replace", "text-from-xlsx", presentationPath, "--workbook", workbookPath, "--range", "A1", "--slide", "1", "--target", "title", "--dry-run"}, ExitInvalidArgs, "--sheet is required"},
		{[]string{"pptx", "replace", "text-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--slide", "1", "--target", "title", "--max-cells", "1", "--dry-run"}, ExitInvalidArgs, "above --max-cells"},
		{[]string{"pptx", "replace", "text-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--slide", "1", "--target", "title", "--formula-mode", "bad", "--dry-run"}, ExitInvalidArgs, "formula-mode"},
		{[]string{"pptx", "replace", "text-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--slide", "1", "--target", "shape:9999", "--out", outPath}, ExitTargetNotFound, "shape:9999"},
		{[]string{"pptx", "replace", "text-from-xlsx", workbookPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--slide", "1", "--target", "title", "--out", outPath}, ExitUnsupportedType, ""},
	}

	for _, tt := range tests {
		_, err := executeReplaceTextFromXLSXForTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
		if tt.want != "" && !strings.Contains(err.Error(), tt.want) {
			t.Fatalf("%v: error = %v, want containing %q", tt.args, err, tt.want)
		}
	}
}

func TestPPTXReplaceTextMapFromXLSXRangeJSONReadbackAndValidate(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Mapped title</t></is></c></row>
    <row r="3"><c r="A3"><v>2</v></c><c r="B3" t="inlineStr"><is><t>body</t></is></c><c r="C3"><f>CONCAT(&quot;Body&quot;,&quot; mapped&quot;)</f><v>Body mapped</v></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "text-map-from-xlsx.pptx")

	output, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "replace", "text-map-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:C3",
		"--expect-source-range", "A1:C3",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx replace text-map-from-xlsx failed: %v", err)
	}

	var result ReplaceTextMapFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal text-map JSON: %v\n%s", err, output)
	}
	if result.File != presentationPath || result.Output != outPath || result.Source.Range != "A1:C3" || result.Source.FormulaCount != 1 {
		t.Fatalf("unexpected result metadata: %+v", result)
	}
	if result.Map.Mode != "plain-text" || result.Map.FormulaMode != "value" || result.Map.Rows != 2 || result.Map.Applied != 2 {
		t.Fatalf("unexpected map metadata: %+v", result.Map)
	}
	if len(result.Replacements) != 2 {
		t.Fatalf("replacement count = %d, want 2", len(result.Replacements))
	}
	if result.Replacements[0].SourceRow != 2 || result.Replacements[0].Slide != 1 || result.Replacements[0].Destination.PrimarySelector != "title" {
		t.Fatalf("unexpected first replacement: %+v", result.Replacements[0])
	}
	if result.Replacements[1].SourceRow != 3 || result.Replacements[1].Slide != 2 || result.Replacements[1].Text != "Body mapped" {
		t.Fatalf("unexpected second replacement: %+v", result.Replacements[1])
	}

	assertPPTXBridgeOutputVerificationCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath)
	firstReadback := assertPPTXBridgeSavedCommandsForTest(t, result.Replacements[0].PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	assertShapeReadbackContainsForTextMapTest(t, firstReadback, "Mapped title")
	secondReadback := assertPPTXBridgeSavedCommandsForTest(t, result.Replacements[1].PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	assertShapeReadbackContainsForTextMapTest(t, secondReadback, "Body mapped")
}

func TestPPTXReplaceTextMapFromXLSXNamedTableFormulaModeAndDryRun(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithTextMapTable(t)

	output, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "replace", "text-map-from-xlsx", presentationPath,
		"--workbook", workbookPath,
		"--table", "TextMap",
		"--formula-mode", "formula",
		"--mode", "preserve-format",
		"--slide-col", "1",
		"--target-col", "2",
		"--text-col", "3",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx replace text-map-from-xlsx table dry-run failed: %v", err)
	}

	var result ReplaceTextMapFromXLSXResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal text-map dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected dry-run output metadata: %+v", result)
	}
	if result.Source.Table != "TextMap" || result.Source.Sheet != "Data" || result.Source.Range != "A1:C2" {
		t.Fatalf("unexpected table source: %+v", result.Source)
	}
	if result.Map.Mode != "preserve-format" || result.Map.FormulaMode != "formula" || result.Replacements[0].Text != "=CONCAT(\"Dry\",\" run\")" {
		t.Fatalf("unexpected dry-run map result: map=%+v replacements=%+v", result.Map, result.Replacements)
	}
	assertPPTXBridgeOutputVerificationTemplatesForTest(t, result.PPTXBridgeReadbackCommands)
	if len(result.Replacements) != 1 {
		t.Fatalf("replacement count = %d, want 1", len(result.Replacements))
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.Replacements[0].PPTXBridgeReadbackCommands, "pptx shapes get")
	assertShapeTextContainsForTextMapTest(t, presentationPath, 1, "title", "Title Content Presentation")
}

func TestPPTXReplaceTextMapFromXLSXRejectsBadArgs(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>x</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>bad</t></is></c><c r="B3" t="inlineStr"><is><t>title</t></is></c><c r="C3" t="inlineStr"><is><t>x</t></is></c></row>
  </sheetData>
</worksheet>`)
	outOfRangeWorkbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>99</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>x</t></is></c></row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "bad-text-map.pptx")

	tests := []struct {
		args []string
		code int
		want string
	}{
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--range", "A1:C2", "--slide-col", "slide", "--target-col", "target", "--text-col", "text", "--dry-run"}, ExitInvalidArgs, "--sheet is required"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--table", "TextMap", "--dry-run"}, ExitInvalidArgs, "only one"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--max-cells", "1", "--dry-run"}, ExitInvalidArgs, "above --max-cells"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--expect-source-range", "A1:C3", "--dry-run"}, ExitInvalidArgs, "expect-source-range mismatch"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--slide-col", "missing", "--dry-run"}, ExitInvalidArgs, "not found"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--slide-col", "1", "--target-col", "1", "--text-col", "3", "--dry-run"}, ExitInvalidArgs, "distinct"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C3", "--dry-run"}, ExitInvalidArgs, "slide must be"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", outOfRangeWorkbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--dry-run"}, ExitInvalidArgs, "slide 99 out of range"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--target-col", "target", "--text-col", "text", "--slide-col", "slide", "--mode", "bad", "--dry-run"}, ExitInvalidArgs, "mode"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", presentationPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--target-col", "target", "--text-col", "text", "--slide-col", "slide", "--out", outPath}, ExitTargetNotFound, "row 2"},
		{[]string{"pptx", "replace", "text-map-from-xlsx", workbookPath, "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1:C2", "--dry-run"}, ExitUnsupportedType, ""},
	}
	for testIndex, tt := range tests {
		if testIndex == 9 {
			badTargetWorkbook := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>shape:9999</t></is></c><c r="C2" t="inlineStr"><is><t>x</t></is></c></row>
  </sheetData>
</worksheet>`)
			tt.args[5] = badTargetWorkbook
		}
		_, err := executeReplaceTextFromXLSXForTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
		if tt.want != "" && !strings.Contains(err.Error(), tt.want) {
			t.Fatalf("%v: error = %v, want containing %q", tt.args, err, tt.want)
		}
	}
}

func writeTestXLSXWithTextMapTable(t *testing.T) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "text-map-table.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	worksheet := `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2"><f>CONCAT(&quot;Dry&quot;,&quot; run&quot;)</f><v>Dry run</v></c></row>
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>`
	tableXML := `<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="TextMap" displayName="TextMap" ref="A1:C2" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C2"/>
  <tableColumns count="3">
    <tableColumn id="1" name="slide"/>
    <tableColumn id="2" name="target"/>
    <tableColumn id="3" name="text"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>`

	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>`)
	addZipFile(t, zw, "_rels/.rels", rootRelsXML())
	addZipFile(t, zw, "xl/workbook.xml", workbookXML([]testSheet{{Name: "Data", SheetID: "1"}}))
	addZipFile(t, zw, "xl/_rels/workbook.xml.rels", workbookRelsXML([]testSheet{{Name: "Data", SheetID: "1"}}))
	addZipFile(t, zw, "xl/worksheets/sheet1.xml", worksheet)
	addZipFile(t, zw, "xl/worksheets/_rels/sheet1.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>`)
	addZipFile(t, zw, "xl/tables/table1.xml", tableXML)
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}
	return path
}

func assertShapeTextContainsForTextMapTest(t *testing.T, filePath string, slide int, target string, want string) {
	t.Helper()
	readback, err := executeReplaceTextFromXLSXForTest(t,
		"--format", "json",
		"pptx", "shapes", "get", filePath,
		"--slide", strconv.Itoa(slide),
		"--target", target,
		"--include-text",
	)
	if err != nil {
		t.Fatalf("pptx shapes get readback failed: %v", err)
	}
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal shape readback: %v\n%s", err, readback)
	}
	if len(shapes.Shapes) != 1 || !strings.Contains(shapes.Shapes[0].TextPreview, want) {
		t.Fatalf("shape text = %+v, want containing %q", shapes.Shapes, want)
	}
}

func assertShapeReadbackContainsForTextMapTest(t *testing.T, readback string, want string) {
	t.Helper()
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal generated shape readback: %v\n%s", err, readback)
	}
	if len(shapes.Shapes) != 1 || !strings.Contains(shapes.Shapes[0].TextPreview, want) {
		t.Fatalf("generated shape readback = %+v, want containing %q", shapes.Shapes, want)
	}
}

func executeReplaceTextFromXLSXForTest(t *testing.T, args ...string) (string, error) {
	t.Helper()
	cmd := newTestRootCmd(t)
	cmd.SetArgs(args)
	var output bytes.Buffer
	cmd.SetOut(&output)
	cmd.SetErr(&bytes.Buffer{})
	err := cmd.Execute()
	return output.String(), err
}
