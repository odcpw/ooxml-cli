package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func xlsxWorkbookMetadataUpdate(t *testing.T, args ...string) XLSXWorkbookMetadataUpdateResult {
	t.Helper()
	output, err := executeRootForXLSXTest(t, append([]string{"--format", "json", "xlsx", "workbook", "metadata", "update"}, args...)...)
	if err != nil {
		t.Fatalf("update failed: %v\n%s", err, output)
	}
	var res XLSXWorkbookMetadataUpdateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("failed to unmarshal update JSON: %v\n%s", err, output)
	}
	return res
}

func TestXLSXWorkbookMetadataUpdateAndReadback(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "meta.xlsx")

	res := xlsxWorkbookMetadataUpdate(t, wb,
		"--title", "Project Budget 2026",
		"--category", "Finance",
		"--company", "Acme Corp",
		"--full-calc-on-load",
		"--out", out,
	)
	if res.Updated != 4 {
		t.Fatalf("expected 4 updated fields, got %d (%v)", res.Updated, res.UpdatedFields)
	}
	if res.Metadata.Title != "Project Budget 2026" || res.Metadata.Category != "Finance" || res.Metadata.Company != "Acme Corp" {
		t.Fatalf("unexpected metadata readback: %+v", res.Metadata)
	}
	if !res.CalcSettings.FullCalcOnLoad || !res.CalcSettings.ForceFullCalc {
		t.Fatalf("expected fullCalcOnLoad+forceFullCalc set: %+v", res.CalcSettings)
	}
	if res.InspectCommand == "" || !strings.HasPrefix(res.InspectCommand, "ooxml --json ") {
		t.Fatalf("missing/invalid inspect readback command: %q", res.InspectCommand)
	}

	// Exercise the generated readback command as-is.
	inspectOut := executeGeneratedOOXMLCommandForXLSXTest(t, res.InspectCommand)
	var inspectRes XLSXWorkbookMetadataInspectResult
	if err := json.Unmarshal([]byte(inspectOut), &inspectRes); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, inspectOut)
	}
	if inspectRes.Metadata.Title != "Project Budget 2026" || inspectRes.Metadata.Company != "Acme Corp" {
		t.Fatalf("inspect readback mismatch: %+v", inspectRes.Metadata)
	}
	if !inspectRes.CalcSettings.FullCalcOnLoad {
		t.Fatalf("inspect readback missing fullCalcOnLoad: %+v", inspectRes.CalcSettings)
	}
}

func TestXLSXWorkbookMetadataEmptyValueClearsField(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	withTitle := filepath.Join(t.TempDir(), "with.xlsx")
	xlsxWorkbookMetadataUpdate(t, wb, "--title", "Temp Title", "--out", withTitle)

	cleared := filepath.Join(t.TempDir(), "cleared.xlsx")
	xlsxWorkbookMetadataUpdate(t, withTitle, "--title", "", "--out", cleared)

	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "workbook", "metadata", "inspect", cleared)
	if err != nil {
		t.Fatalf("inspect failed: %v", err)
	}
	var res XLSXWorkbookMetadataInspectResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, output)
	}
	if res.Metadata.Title != "" {
		t.Fatalf("expected title cleared, got %q", res.Metadata.Title)
	}
	// The cleared file must remain valid (no empty dc:title element corruption).
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", cleared); err != nil {
		t.Fatalf("validate after clear failed: %v", err)
	}
}

func TestXLSXWorkbookMetadataRejectsInvalidCalcMode(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	_, err := executeRootForXLSXTest(t, "xlsx", "workbook", "metadata", "update", wb, "--calc-mode", "bogus", "--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected error for invalid calc-mode")
	}
	if !strings.Contains(err.Error(), "calcMode") {
		t.Fatalf("expected calcMode validation error, got: %v", err)
	}
}

func TestXLSXWorkbookMetadataInspectEmpty(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "workbook", "metadata", "inspect", wb)
	if err != nil {
		t.Fatalf("inspect failed: %v\n%s", err, output)
	}
	var res XLSXWorkbookMetadataInspectResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, output)
	}
	if res.Metadata.Title != "" || res.Metadata.Company != "" {
		t.Fatalf("expected empty metadata for minimal workbook: %+v", res.Metadata)
	}
	// Missing calcPr -> defaults.
	if res.CalcSettings.CalcMode != "auto" || res.CalcSettings.FullCalcOnLoad || res.CalcSettings.IterateCount != 100 {
		t.Fatalf("unexpected default calc settings: %+v", res.CalcSettings)
	}
}

func TestXLSXWorkbookMetadataUpdateAppProps(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "meta.xlsx")
	res := xlsxWorkbookMetadataUpdate(t, wb, "--manager", "Carol White", "--out", out)
	if res.Metadata.Manager != "Carol White" {
		t.Fatalf("expected manager set: %+v", res.Metadata)
	}
	inspectOut := executeGeneratedOOXMLCommandForXLSXTest(t, res.InspectCommand)
	var inspectRes XLSXWorkbookMetadataInspectResult
	if err := json.Unmarshal([]byte(inspectOut), &inspectRes); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, inspectOut)
	}
	if inspectRes.Metadata.Manager != "Carol White" {
		t.Fatalf("manager readback mismatch: %+v", inspectRes.Metadata)
	}
}

func TestXLSXWorkbookMetadataUpdateGuardSuccess(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	step1 := filepath.Join(t.TempDir(), "step1.xlsx")
	xlsxWorkbookMetadataUpdate(t, wb, "--title", "Old Title", "--out", step1)

	step2 := filepath.Join(t.TempDir(), "step2.xlsx")
	res := xlsxWorkbookMetadataUpdate(t, step1, "--title", "New Title", "--expect-title", "Old Title", "--out", step2)
	if res.Metadata.Title != "New Title" {
		t.Fatalf("expected title updated: %+v", res.Metadata)
	}
}

func TestXLSXWorkbookMetadataUpdateGuardMismatch(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "meta.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json",
		"xlsx", "workbook", "metadata", "update", wb,
		"--title", "New", "--expect-title", "Wrong", "--out", out)
	if err == nil {
		t.Fatalf("expected guard mismatch error, got success: %s", output)
	}
	if !strings.Contains(err.Error(), "expected title to be") {
		t.Fatalf("unexpected error: %v", err)
	}
	if _, statErr := os.Stat(out); statErr == nil {
		t.Fatalf("output file should not be written on guard mismatch")
	}
}

func TestXLSXWorkbookMetadataUpdateDryRun(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	res := xlsxWorkbookMetadataUpdate(t, wb, "--title", "Dry", "--dry-run")
	if !res.DryRun {
		t.Fatalf("expected dryRun true: %+v", res)
	}
	if res.Output != "" {
		t.Fatalf("dry-run should not report an output file: %q", res.Output)
	}
}

func TestXLSXWorkbookMetadataUpdateValidate(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "meta.xlsx")
	// Set both app properties so app.xml element ordering (xsd:sequence) is exercised.
	res := xlsxWorkbookMetadataUpdate(t, wb, "--title", "Validatable", "--company", "Acme", "--manager", "Carol", "--full-calc-on-load", "--out", out)
	if res.Metadata.Manager != "Carol" || res.Metadata.Company != "Acme" {
		t.Fatalf("expected both app props set: %+v", res.Metadata)
	}
	if res.ValidateCommand == "" {
		t.Fatalf("missing validate command")
	}
	validateOut, err := executeRootForXLSXTest(t, "--format", "json", "validate", "--strict", out)
	if err != nil {
		t.Fatalf("validate failed: %v\n%s", err, validateOut)
	}
	if !strings.Contains(validateOut, "\"valid\":true") && !strings.Contains(validateOut, "\"status\":\"valid\"") {
		t.Fatalf("expected valid output, got: %s", validateOut)
	}
}

func TestXLSXWorkbookMetadataUpdateNoFields(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "meta.xlsx")
	_, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "workbook", "metadata", "update", wb, "--out", out)
	if err == nil {
		t.Fatalf("expected error when no metadata fields specified")
	}
}
