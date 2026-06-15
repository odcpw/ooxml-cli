package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXTablesSetColumnFormatJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")
	outPath := filepath.Join(t.TempDir(), "table-format.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "set-column-format", workbookPath,
		"--table", "Sales",
		"--column", "Amount",
		"--preset", "currency",
		"--decimals", "2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx tables set-column-format failed: %v", err)
	}
	var result XLSXTablesSetColumnFormatResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-column-format JSON: %v\n%s", err, output)
	}
	if result.Table != "Sales" || result.Column != "Amount" || result.ColumnIndex != 1 {
		t.Fatalf("unexpected table/column metadata: %+v", result)
	}
	if result.Range != "B2:B3" || result.TableRange != "A1:B3" {
		t.Fatalf("unexpected ranges: range=%q tableRange=%q", result.Range, result.TableRange)
	}
	if result.Rows != 2 || result.Cols != 1 || result.Updated != 2 {
		t.Fatalf("unexpected dimensions/updated: %+v", result)
	}
	if result.Preset != "currency" || !strings.Contains(result.FormatCode, "#,##0.00") {
		t.Fatalf("unexpected format: preset=%q code=%q", result.Preset, result.FormatCode)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected mutation metadata: %+v", result)
	}
	if result.Destination == nil || result.Destination.Range != "B2:B3" {
		t.Fatalf("unexpected destination: %+v", result.Destination)
	}

	assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "B2:B3")
	showOutput, _ := assertXLSXTableAppendSavedCommandsForTest(t, result.XLSXTableAppendReadbackCommands, outPath)
	var showResult XLSXTablesResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal table show readback: %v\n%s", err, showOutput)
	}
	if len(showResult.Tables) != 1 || showResult.Tables[0].Range != "A1:B3" {
		t.Fatalf("unexpected table readback: %+v", showResult.Tables)
	}

	stylesXML := readZipEntryForTest(t, outPath, "xl/styles.xml")
	if !strings.Contains(stylesXML, "applyNumberFormat=\"1\"") {
		t.Fatalf("styles.xml missing applied number format:\n%s", stylesXML)
	}
}

func TestXLSXTablesSetColumnFormatDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "set-column-format", workbookPath,
		"--table", "Sales",
		"--column", "Amount",
		"--preset", "integer",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx tables set-column-format dry-run failed: %v", err)
	}
	var result XLSXTablesSetColumnFormatResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "B2:B3")
	assertXLSXTableAppendDryRunTemplatesForTest(t, result.XLSXTableAppendReadbackCommands)
}

func TestXLSXTablesSetColumnFormatExcludesTotalsRow(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B4", true, "")
	outPath := filepath.Join(t.TempDir(), "table-format-totals.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "set-column-format", workbookPath,
		"--table", "Sales",
		"--column", "Amount",
		"--preset", "number",
		"--decimals", "0",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx tables set-column-format with totals failed: %v", err)
	}
	var result XLSXTablesSetColumnFormatResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal totals JSON: %v\n%s", err, output)
	}
	if result.Range != "B2:B3" || result.Rows != 2 {
		t.Fatalf("totals row not excluded: range=%q rows=%d", result.Range, result.Rows)
	}
}

func TestXLSXTablesSetColumnFormatGuardMismatch(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	_, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "set-column-format", workbookPath,
		"--table", "Sales",
		"--column", "Amount",
		"--expect-column", "Region",
		"--preset", "currency",
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "set-column-format"}, err, ExitInvalidArgs)
}

func TestXLSXTablesSetColumnFormatUnknownColumn(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	_, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "set-column-format", workbookPath,
		"--table", "Sales",
		"--column", "Missing",
		"--preset", "currency",
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "set-column-format"}, err, ExitTargetNotFound)
}

func TestXLSXTablesSetColumnFormatRequiresColumn(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	_, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "set-column-format", workbookPath,
		"--table", "Sales",
		"--preset", "currency",
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "set-column-format"}, err, ExitInvalidArgs)
}
