package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func stagePivotData(t *testing.T) string {
	t.Helper()
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "pdata.xlsx")
	values := `[["Region","Product","Sales"],["North","A",42],["South","A",58],["North","B",30],["South","B",33]]`
	if _, err := executeRootForXLSXTest(t, "xlsx", "ranges", "set", wb, "--sheet", "1", "--anchor", "A1", "--values", values, "--out", out); err != nil {
		t.Fatalf("stage pivot data failed: %v", err)
	}
	return out
}

func TestXLSXPivotsCreateAndReadback(t *testing.T) {
	data := stagePivotData(t)
	out := filepath.Join(t.TempDir(), "pivot.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "create", data,
		"--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--values", "Sales:sum", "--anchor", "F1", "--out", out)
	if err != nil {
		t.Fatalf("pivots create failed: %v\n%s", err, output)
	}
	var res XLSXPivotsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.CacheDefURI == "" || res.CacheRecordURI == "" || res.PivotTableURI == "" {
		t.Fatalf("missing pivot part URIs: %+v", res)
	}
	if len(res.RowFields) != 1 || res.RowFields[0] != "Region" || len(res.ValueFields) != 1 {
		t.Fatalf("unexpected pivot fields: %+v", res)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	// The authored pivot must be discoverable via the inspector.
	if res.PivotsListCommand == "" {
		t.Fatalf("missing pivots list readback command")
	}
	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, res.PivotsListCommand)
	if !strings.Contains(listOut, res.Name) {
		t.Fatalf("authored pivot not found in list readback: %s", listOut)
	}
}

func TestXLSXPivotsCreateMultiField(t *testing.T) {
	data := stagePivotData(t)
	out := filepath.Join(t.TempDir(), "pivot.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "create", data,
		"--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--cols", "Product",
		"--values", "Sales:count", "--anchor", "F1", "--out", out)
	if err != nil {
		t.Fatalf("multi-field pivots create failed: %v", err)
	}
	var res XLSXPivotsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if len(res.ColFields) != 1 || res.ColFields[0] != "Product" {
		t.Fatalf("expected Product col field: %+v", res)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
}

func TestXLSXPivotsCreateUnknownFieldErrors(t *testing.T) {
	data := stagePivotData(t)
	_, err := executeRootForXLSXTest(t, "xlsx", "pivots", "create", data, "--sheet", "1", "--range", "A1:C5",
		"--rows", "Nonexistent", "--values", "Sales", "--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected error for unknown row field")
	}
}

func TestXLSXPivotsCreateRequiresRowsOrColsAndValues(t *testing.T) {
	data := stagePivotData(t)
	if _, err := executeRootForXLSXTest(t, "xlsx", "pivots", "create", data, "--sheet", "1", "--range", "A1:C5", "--values", "Sales", "--out", filepath.Join(t.TempDir(), "a.xlsx")); err == nil {
		t.Fatalf("expected error when no rows/cols given")
	}
	if _, err := executeRootForXLSXTest(t, "xlsx", "pivots", "create", data, "--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--out", filepath.Join(t.TempDir(), "b.xlsx")); err == nil {
		t.Fatalf("expected error when no values given")
	}
}
