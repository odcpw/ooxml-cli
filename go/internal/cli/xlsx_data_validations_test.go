package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func createDVForTest(t *testing.T, args ...string) (string, XLSXDataValidationMutationResult) {
	t.Helper()
	output, err := executeRootForXLSXTest(t, args...)
	if err != nil {
		t.Fatalf("data-validations create failed: %v\nargs: %v", err, args)
	}
	var res XLSXDataValidationMutationResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("failed to unmarshal create JSON: %v\n%s", err, output)
	}
	return output, res
}

func TestXLSXDataValidationsCreateListInlineReadback(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "dv.xlsx")

	_, res := createDVForTest(t,
		"--format", "json",
		"xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10",
		"--type", "list", "--list-values", "Red,Green,Blue",
		"--show-input-message", "--input-title", "Pick", "--input-message", "Choose a color",
		"--out", outPath,
	)
	if res.Range != "A1:A10" || res.Action != "create" {
		t.Fatalf("unexpected create result: %+v", res)
	}
	if res.DataValidation == nil || res.DataValidation.Type != "list" || res.DataValidation.Formula1 != `"Red,Green,Blue"` {
		t.Fatalf("unexpected list formula1: %+v", res.DataValidation)
	}
	if res.DataValidation.PrimarySelector != "A1:A10" || !containsString(res.DataValidation.Selectors, "A1:A10") {
		t.Fatalf("missing create data validation selectors: %+v", res.DataValidation)
	}
	if !res.DataValidation.ShowInputMessage || res.DataValidation.PromptTitle != "Pick" || res.DataValidation.Prompt != "Choose a color" {
		t.Fatalf("unexpected input message: %+v", res.DataValidation)
	}
	if res.CellsAffected != 10 {
		t.Fatalf("expected 10 cells affected, got %d", res.CellsAffected)
	}
	if res.DataValidationsListCommand == "" || !strings.Contains(res.DataValidationsListCommand, "--json") {
		t.Fatalf("missing list readback command: %+v", res)
	}
	if res.DataValidationsShowCommand == "" {
		t.Fatalf("missing show readback command: %+v", res)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, res.DataValidationsListCommand)
	var listResult XLSXDataValidationsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || listResult.DataValidations[0].Sqref != "A1:A10" {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	if listResult.DataValidations[0].PrimarySelector != "A1:A10" || !containsString(listResult.DataValidations[0].Selectors, "A1:A10") {
		t.Fatalf("missing list data validation selectors: %+v", listResult.DataValidations[0])
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, res.DataValidationsShowCommand)
	var showResult XLSXDataValidationJSON
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOut)
	}
	if showResult.Type != "list" || showResult.Formula1 != `"Red,Green,Blue"` {
		t.Fatalf("unexpected show result: %+v", showResult)
	}
	if showResult.PrimarySelector != "A1:A10" || !containsString(showResult.Selectors, "A1:A10") {
		t.Fatalf("missing show data validation selectors: %+v", showResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXDataValidationsCreateWholeBetween(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "dv.xlsx")
	_, res := createDVForTest(t,
		"--format", "json",
		"xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "B1:B20",
		"--type", "whole", "--operator", "between",
		"--formula1", "1", "--formula2", "100",
		"--allow-blank", "--show-error-message", "--error-style", "stop",
		"--error-title", "Bad", "--error-message", "1-100 only",
		"--out", outPath,
	)
	dv := res.DataValidation
	if dv == nil || dv.Type != "whole" || dv.Operator != "between" || dv.Formula1 != "1" || dv.Formula2 != "100" {
		t.Fatalf("unexpected whole result: %+v", dv)
	}
	if !dv.AllowBlank || !dv.ShowErrorMessage || dv.ErrorStyle != "stop" || dv.Error != "1-100 only" {
		t.Fatalf("unexpected error attrs: %+v", dv)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXDataValidationsCreateListRange(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "dv.xlsx")
	_, res := createDVForTest(t,
		"--format", "json",
		"xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "C1:C5",
		"--type", "list", "--list-range", "$E$1:$E$3",
		"--out", outPath,
	)
	if res.DataValidation == nil || res.DataValidation.Formula1 != "$E$1:$E$3" {
		t.Fatalf("unexpected list-range formula1: %+v", res.DataValidation)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXDataValidationsCreateTextLengthAndDate(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "dv.xlsx")
	_, res := createDVForTest(t,
		"--format", "json",
		"xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "D1:D10",
		"--type", "text-length", "--operator", "lessThanOrEqual", "--formula1", "5",
		"--out", outPath,
	)
	if res.DataValidation == nil || res.DataValidation.Type != "textLength" {
		t.Fatalf("expected textLength type, got: %+v", res.DataValidation)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXDataValidationsUpdate(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	created := filepath.Join(t.TempDir(), "dv.xlsx")
	createDVForTest(t, "--format", "json", "xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10", "--type", "list", "--list-values", "a,b,c", "--out", created)

	updated := filepath.Join(t.TempDir(), "dv2.xlsx")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "data-validations", "update", created,
		"--sheet", "1", "--range", "A1:A10", "--list-values", "a,b,c,d", "--expect-type", "list", "--out", updated)
	if err != nil {
		t.Fatalf("update failed: %v", err)
	}
	var res XLSXDataValidationMutationResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("failed to unmarshal update JSON: %v\n%s", err, out)
	}
	if res.DataValidation == nil || res.DataValidation.Formula1 != `"a,b,c,d"` {
		t.Fatalf("unexpected updated formula1: %+v", res.DataValidation)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", updated); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXDataValidationsUpdateGuardMismatch(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	created := filepath.Join(t.TempDir(), "dv.xlsx")
	createDVForTest(t, "--format", "json", "xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10", "--type", "list", "--list-values", "a,b", "--out", created)

	_, err := executeRootForXLSXTest(t, "xlsx", "data-validations", "update", created,
		"--sheet", "1", "--range", "A1:A10", "--list-values", "x,y", "--expect-type", "whole",
		"--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected guard mismatch error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXDataValidationsDelete(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	created := filepath.Join(t.TempDir(), "dv.xlsx")
	createDVForTest(t, "--format", "json", "xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10", "--type", "list", "--list-values", "a,b", "--out", created)

	deleted := filepath.Join(t.TempDir(), "del.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "data-validations", "delete", created,
		"--sheet", "1", "--range", "A1:A10", "--expect-type", "list", "--out", deleted); err != nil {
		t.Fatalf("delete failed: %v", err)
	}
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "data-validations", "list", deleted, "--sheet", "1")
	if err != nil {
		t.Fatalf("list failed: %v", err)
	}
	var listResult XLSXDataValidationsListResult
	if err := json.Unmarshal([]byte(out), &listResult); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, out)
	}
	if listResult.Count != 0 {
		t.Fatalf("expected no data validations after delete: %+v", listResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", deleted); err != nil {
		t.Fatalf("validate after delete failed: %v", err)
	}
}

func TestXLSXDataValidationsShowNotFound(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	created := filepath.Join(t.TempDir(), "dv.xlsx")
	createDVForTest(t, "--format", "json", "xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10", "--type", "list", "--list-values", "a,b", "--out", created)

	_, err := executeRootForXLSXTest(t, "xlsx", "data-validations", "show", created, "--sheet", "1", "--range", "Z9")
	if err == nil {
		t.Fatalf("expected not-found error")
	}
	if !strings.Contains(err.Error(), "A1:A10") {
		t.Fatalf("expected available ranges in error, got: %v", err)
	}
	if !strings.Contains(err.Error(), "did you mean: A1:A10") || !strings.Contains(err.Error(), "ooxml --json xlsx data-validations list <file> --sheet sheetId:1") {
		t.Fatalf("expected selector candidates and discovery command, got: %v", err)
	}
}

func TestXLSXDataValidationsMultiRangeSqref(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "dv.xlsx")
	_, res := createDVForTest(t,
		"--format", "json",
		"xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A5 C1:C5",
		"--type", "whole", "--operator", "greaterThan", "--formula1", "0",
		"--out", outPath,
	)
	if res.Range != "A1:A5 C1:C5" {
		t.Fatalf("unexpected multi-range sqref: %q", res.Range)
	}
	if res.CellsAffected != 10 {
		t.Fatalf("expected 10 cells affected, got %d", res.CellsAffected)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXDataValidationsCreateListMissingSource(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	_, err := executeRootForXLSXTest(t, "xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10", "--type", "list",
		"--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected error when list type has no values or range")
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXDataValidationsDryRun(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "data-validations", "create", workbookPath,
		"--sheet", "1", "--range", "A1:A10", "--type", "list", "--list-values", "a,b", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run failed: %v", err)
	}
	var res XLSXDataValidationMutationResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, out)
	}
	if !res.DryRun || res.Output != "" {
		t.Fatalf("expected dry-run with no output: %+v", res)
	}
}
