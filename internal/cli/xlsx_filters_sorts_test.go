package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXSetAutoFilterOnRangeAndShow(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "af.xlsx")

	out, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "filters-sorts", "set-autofilter", workbookPath,
		"--sheet", "1", "--range", "A1:C1", "--out", outPath,
	)
	if err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	var setResult XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(out), &setResult); err != nil {
		t.Fatalf("unmarshal set result: %v\n%s", err, out)
	}
	if setResult.AutoFilter == nil || setResult.AutoFilter.Ref != "A1:C1" {
		t.Fatalf("unexpected autoFilter ref: %+v", setResult.AutoFilter)
	}
	if setResult.Note == "" || !strings.Contains(setResult.Note, "does NOT") {
		t.Fatalf("expected honesty note, got %q", setResult.Note)
	}
	if setResult.ShowCommand == "" || !strings.Contains(setResult.ShowCommand, "filters-sorts show") {
		t.Fatalf("missing show command: %+v", setResult)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, setResult.ShowCommand)
	var showResult XLSXFiltersSortsShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal show result: %v\n%s", err, showOut)
	}
	if showResult.AutoFilter == nil || showResult.AutoFilter.Ref != "A1:C1" {
		t.Fatalf("show: expected autoFilter A1:C1, got %+v", showResult.AutoFilter)
	}
	if showResult.Note == "" {
		t.Fatalf("show must include honesty note")
	}
}

func TestXLSXSetAutoFilterOnTable(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")
	outPath := filepath.Join(t.TempDir(), "af-table.xlsx")

	out, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "filters-sorts", "set-autofilter", workbookPath,
		"--table", "Sales", "--out", outPath,
	)
	if err != nil {
		t.Fatalf("set-autofilter on table failed: %v", err)
	}
	var setResult XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(out), &setResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if setResult.Table != "Sales" || setResult.AutoFilter == nil || setResult.AutoFilter.Ref != "A1:B3" {
		t.Fatalf("unexpected table autoFilter result: %+v", setResult)
	}

	showOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "show", outPath, "--table", "Sales")
	if err != nil {
		t.Fatalf("show table failed: %v", err)
	}
	var showResult XLSXFiltersSortsShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal show: %v\n%s", err, showOut)
	}
	if showResult.AutoFilter == nil || showResult.AutoFilter.Ref != "A1:B3" {
		t.Fatalf("show table: expected A1:B3, got %+v", showResult.AutoFilter)
	}
}

func TestXLSXClearAutoFilter(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	cleared := filepath.Join(t.TempDir(), "cleared.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "clear-autofilter", withAF, "--sheet", "1", "--out", cleared); err != nil {
		t.Fatalf("clear-autofilter failed: %v", err)
	}
	showOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "show", cleared, "--sheet", "1")
	if err != nil {
		t.Fatalf("show failed: %v", err)
	}
	var showResult XLSXFiltersSortsShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, showOut)
	}
	if showResult.AutoFilter != nil {
		t.Fatalf("expected autoFilter removed, got %+v", showResult.AutoFilter)
	}
}

func TestXLSXAddColumnFilterValues(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	out := filepath.Join(t.TempDir(), "filtered.xlsx")
	res, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "add-column-filter", withAF, "--sheet", "1", "--column", "0", "--values", "Apple,Banana,Apple", "--out", out)
	if err != nil {
		t.Fatalf("add-column-filter failed: %v", err)
	}
	var addResult XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(res), &addResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, res)
	}
	if addResult.AutoFilter == nil || len(addResult.AutoFilter.Columns) != 1 {
		t.Fatalf("expected 1 filter column, got %+v", addResult.AutoFilter)
	}
	col := addResult.AutoFilter.Columns[0]
	if col.ColID != 0 {
		t.Fatalf("expected colId 0, got %d", col.ColID)
	}
	// Apple should be deduplicated.
	if len(col.Values) != 2 {
		t.Fatalf("expected 2 deduped values, got %v", col.Values)
	}
}

func TestXLSXAddColumnFilterCustom(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	out := filepath.Join(t.TempDir(), "custom.xlsx")
	res, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "add-column-filter", withAF, "--sheet", "1", "--column", "1", "--custom-op", "greaterThan", "--custom-val1", "50", "--out", out)
	if err != nil {
		t.Fatalf("add-column-filter custom failed: %v", err)
	}
	var addResult XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(res), &addResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, res)
	}
	if addResult.AutoFilter == nil || len(addResult.AutoFilter.Columns) != 1 {
		t.Fatalf("expected 1 filter column, got %+v", addResult.AutoFilter)
	}
	cf := addResult.AutoFilter.Columns[0].CustomFilter
	if cf == nil || len(cf.Criteria) != 1 || cf.Criteria[0].Operator != "greaterThan" || cf.Criteria[0].Val != "50" {
		t.Fatalf("unexpected custom filter: %+v", cf)
	}
}

func TestXLSXAddColumnFilterRequiresAutoFilter(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "noaf.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "add-column-filter", workbookPath, "--sheet", "1", "--column", "0", "--values", "A", "--out", out)
	if err == nil {
		t.Fatalf("expected error adding column filter without autoFilter")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXAddColumnFilterOutOfBounds(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	out := filepath.Join(t.TempDir(), "oob.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "add-column-filter", withAF, "--sheet", "1", "--column", "9", "--values", "A", "--out", out)
	if err == nil {
		t.Fatalf("expected out-of-bounds error")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXClearColumnFilter(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	filtered := filepath.Join(t.TempDir(), "filtered.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "add-column-filter", withAF, "--sheet", "1", "--column", "0", "--values", "A", "--out", filtered); err != nil {
		t.Fatalf("add-column-filter failed: %v", err)
	}
	cleared := filepath.Join(t.TempDir(), "cleared.xlsx")
	res, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "clear-column-filter", filtered, "--sheet", "1", "--column", "0", "--out", cleared)
	if err != nil {
		t.Fatalf("clear-column-filter failed: %v", err)
	}
	var clearResult XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(res), &clearResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, res)
	}
	if clearResult.AutoFilter == nil || len(clearResult.AutoFilter.Columns) != 0 {
		t.Fatalf("expected filter column removed, got %+v", clearResult.AutoFilter)
	}
}

func TestXLSXAddColumnFilterExpectFilterGuard(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	first := filepath.Join(t.TempDir(), "first.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "add-column-filter", withAF, "--sheet", "1", "--column", "0", "--values", "Apple,Banana", "--out", first); err != nil {
		t.Fatalf("first add-column-filter failed: %v", err)
	}

	// Matching guard ("values:Apple,Banana") should succeed and replace the filter.
	ok := filepath.Join(t.TempDir(), "ok.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "add-column-filter", first, "--sheet", "1", "--column", "0", "--values", "Cherry", "--expect-filter", "values:Apple,Banana", "--out", ok); err != nil {
		t.Fatalf("matching expect-filter should succeed: %v", err)
	}

	// Mismatched guard should fail with ExitInvalidArgs.
	bad := filepath.Join(t.TempDir(), "bad.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "add-column-filter", first, "--sheet", "1", "--column", "0", "--values", "Cherry", "--expect-filter", "none", "--out", bad)
	if err == nil {
		t.Fatalf("mismatched expect-filter should error")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXSetSortSingleAndMulti(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	sort1 := filepath.Join(t.TempDir(), "sort1.xlsx")
	res, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "set-sort", workbookPath, "--sheet", "1", "--ref", "A1:C3", "--column", "A", "--out", sort1)
	if err != nil {
		t.Fatalf("set-sort failed: %v", err)
	}
	var sortResult XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(res), &sortResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, res)
	}
	if sortResult.SortState == nil || sortResult.SortState.Ref != "A1:C3" {
		t.Fatalf("unexpected sortState: %+v", sortResult.SortState)
	}
	if len(sortResult.SortState.Conditions) != 1 {
		t.Fatalf("expected 1 sort condition, got %+v", sortResult.SortState.Conditions)
	}
	// Must be a single-column ST_Ref range, not a bare column letter.
	if sortResult.SortState.Conditions[0].Ref != "A1:A3" {
		t.Fatalf("expected condition ref A1:A3, got %q", sortResult.SortState.Conditions[0].Ref)
	}

	sort2 := filepath.Join(t.TempDir(), "sort2.xlsx")
	res2, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "set-sort", sort1, "--sheet", "1", "--ref", "A1:C3", "--column", "B", "--descending", "--out", sort2)
	if err != nil {
		t.Fatalf("second set-sort failed: %v", err)
	}
	var sortResult2 XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(res2), &sortResult2); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, res2)
	}
	if len(sortResult2.SortState.Conditions) != 2 {
		t.Fatalf("expected 2 sort conditions, got %+v", sortResult2.SortState.Conditions)
	}
	if !sortResult2.SortState.Conditions[1].Descending || sortResult2.SortState.Conditions[1].Ref != "B1:B3" {
		t.Fatalf("expected second condition B1:B3 desc, got %+v", sortResult2.SortState.Conditions[1])
	}
}

func TestXLSXClearSort(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	sorted := filepath.Join(t.TempDir(), "sorted.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-sort", workbookPath, "--sheet", "1", "--ref", "A1:C3", "--column", "A", "--out", sorted); err != nil {
		t.Fatalf("set-sort failed: %v", err)
	}
	cleared := filepath.Join(t.TempDir(), "cleared.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "clear-sort", sorted, "--sheet", "1", "--out", cleared); err != nil {
		t.Fatalf("clear-sort failed: %v", err)
	}
	showOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "show", cleared, "--sheet", "1")
	if err != nil {
		t.Fatalf("show failed: %v", err)
	}
	var showResult XLSXFiltersSortsShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, showOut)
	}
	if showResult.SortState != nil {
		t.Fatalf("expected sortState removed, got %+v", showResult.SortState)
	}
}

func TestXLSXSetAutoFilterDryRun(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run failed: %v", err)
	}
	var result XLSXFiltersSortsMutationResult
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if !result.DryRun {
		t.Fatalf("expected dryRun true")
	}
	if result.Output != "" {
		t.Fatalf("expected no output path on dry-run, got %q", result.Output)
	}
}

func TestXLSXSetAutoFilterInvalidRange(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "bad.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "not-a-range", "--out", out)
	if err == nil {
		t.Fatalf("expected invalid range error")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXSetAutoFilterExpectRangeMismatch(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	withAF := filepath.Join(t.TempDir(), "with.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", workbookPath, "--sheet", "1", "--range", "A1:C1", "--out", withAF); err != nil {
		t.Fatalf("set-autofilter failed: %v", err)
	}
	out := filepath.Join(t.TempDir(), "guard.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "filters-sorts", "set-autofilter", withAF, "--sheet", "1", "--range", "A1:B1", "--expect-range", "Z1:Z9", "--out", out)
	if err == nil {
		t.Fatalf("expected expect-range mismatch error")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}
