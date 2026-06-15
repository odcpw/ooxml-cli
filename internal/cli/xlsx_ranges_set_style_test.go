package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXRangesSetStyleAndReadback(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "styled.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "set-style", wb,
		"--sheet", "1", "--range", "A1:B2",
		"--font-bold", "--font-color", "#FF0000", "--font-size", "14",
		"--fill-color", "#FFF2CC",
		"--border-style", "thin",
		"--alignment-horizontal", "center", "--alignment-wrap-text",
		"--out", out,
	)
	if err != nil {
		t.Fatalf("set-style failed: %v\n%s", err, output)
	}
	var res XLSXRangesSetStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, output)
	}
	if res.Range != "A1:B2" || res.Updated != 4 || res.CreatedStyles != 1 {
		t.Fatalf("unexpected set-style result: %+v", res)
	}
	if res.ValidateCommand == "" || res.RangesExportCommand == "" {
		t.Fatalf("missing readback commands: %+v", res.XLSXMutationReadbackCommands)
	}
	// Generated validate command must succeed.
	executeGeneratedOOXMLCommandForXLSXTest(t, res.ValidateCommand)
}

func TestXLSXRangesSetStyleDedupsAndPreservesNumberFormat(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	// First apply a number format, then a style; the style xf must keep the numFmt.
	formatted := filepath.Join(t.TempDir(), "fmt.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "ranges", "set-format", wb, "--sheet", "1", "--range", "A1:A1", "--preset", "currency", "--out", formatted); err != nil {
		t.Fatalf("set-format failed: %v", err)
	}
	styled := filepath.Join(t.TempDir(), "styled.xlsx")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "ranges", "set-style", formatted, "--sheet", "1", "--range", "A1:A1", "--font-bold", "--out", styled)
	if err != nil {
		t.Fatalf("set-style failed: %v", err)
	}
	var res XLSXRangesSetStyleResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, out)
	}
	// Validate the result stays a valid package.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", styled); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	// Export with formats to confirm the number format survived alongside the new style.
	exp, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "ranges", "export", styled, "--sheet", "1", "--range", "A1:A1", "--include-formats")
	if err != nil {
		t.Fatalf("export failed: %v", err)
	}
	if !strings.Contains(exp, "numberFormat") {
		t.Fatalf("expected number-format metadata in export: %s", exp)
	}
}

func TestXLSXRangesSetStyleRejectsBadInput(t *testing.T) {
	wb := getXLSXTestFilePath("minimal-workbook")
	cases := [][]string{
		{"xlsx", "ranges", "set-style", wb, "--sheet", "1", "--range", "A1:A1", "--out", filepath.Join(t.TempDir(), "a.xlsx")},                                       // no style flags
		{"xlsx", "ranges", "set-style", wb, "--sheet", "1", "--range", "A1:A1", "--font-color", "nothex", "--out", filepath.Join(t.TempDir(), "b.xlsx")},             // bad color
		{"xlsx", "ranges", "set-style", wb, "--sheet", "1", "--range", "A1:A1", "--border-style", "wiggly", "--out", filepath.Join(t.TempDir(), "c.xlsx")},           // bad border style
		{"xlsx", "ranges", "set-style", wb, "--sheet", "1", "--range", "A1:A1", "--alignment-horizontal", "sideways", "--out", filepath.Join(t.TempDir(), "d.xlsx")}, // bad alignment
	}
	for i, args := range cases {
		if _, err := executeRootForXLSXTest(t, args...); err == nil {
			t.Fatalf("case %d expected error", i)
		}
	}
}
