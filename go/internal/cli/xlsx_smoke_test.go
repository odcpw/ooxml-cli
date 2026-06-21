package cli

import (
	"encoding/json"
	"net/url"
	"os/exec"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/conformance"
)

// TestXLSXWorkbenchSmoke exercises the practical XLSX mutation surface end to
// end: it chains range/formula writes, number formats, visual styles, column
// widths, row heights, data validation, hyperlinks, defined names, and chart
// authoring through the CLI, validating the package at each step. When a
// LibreOffice binary is available it also confirms the final workbook opens.
func TestXLSXWorkbenchSmoke(t *testing.T) {
	dir := t.TempDir()
	step := 0
	next := func() string {
		step++
		return filepath.Join(dir, "wb"+string(rune('a'+step))+".xlsx")
	}

	run := func(t *testing.T, args ...string) {
		t.Helper()
		if _, err := executeRootForXLSXTest(t, args...); err != nil {
			t.Fatalf("step failed: %v\nargs=%v", err, args)
		}
	}
	validate := func(t *testing.T, path string) {
		t.Helper()
		if _, err := executeRootForXLSXTest(t, "validate", "--strict", path); err != nil {
			t.Fatalf("validate --strict failed for %s: %v", path, err)
		}
		out, err := executeRootForXLSXTest(t, "--json", "conformance", "check", path)
		if err != nil {
			t.Fatalf("conformance check failed for %s: %v", path, err)
		}
		var report conformance.Report
		if err := json.Unmarshal([]byte(out), &report); err != nil {
			t.Fatalf("failed to parse conformance report for %s: %v\n%s", path, err, out)
		}
		if report.Status != "passed" {
			t.Fatalf("conformance check status for %s = %s, want passed\n%s", path, report.Status, out)
		}
	}

	cur := next()
	// 1. Seed a data table (header + categories + two numeric series).
	run(t, "xlsx", "ranges", "set", getXLSXTestFilePath("minimal-workbook"),
		"--sheet", "1", "--anchor", "A1",
		"--values", `[["Region","Q1","Q2"],["North",42,50],["South",58,61],["East",30,33]]`,
		"--out", cur)
	validate(t, cur)

	// 2. A formula cell.
	prev := cur
	cur = next()
	run(t, "xlsx", "cells", "set", prev, "--sheet", "1", "--cell", "D1", "--formula", "SUM(B2:C4)", "--out", cur)
	validate(t, cur)

	// 3. Currency number format on the value columns.
	prev = cur
	cur = next()
	run(t, "xlsx", "ranges", "set-format", prev, "--sheet", "1", "--range", "B2:C4", "--preset", "currency", "--out", cur)
	validate(t, cur)

	// 4. Visual styles on the header row.
	prev = cur
	cur = next()
	run(t, "xlsx", "ranges", "set-style", prev, "--sheet", "1", "--range", "A1:D1",
		"--font-bold", "--fill-color", "#FFF2CC", "--border-style", "thin",
		"--alignment-horizontal", "center", "--out", cur)
	validate(t, cur)

	// 5. Column widths and row heights.
	prev = cur
	cur = next()
	run(t, "xlsx", "colwidths", "set", prev, "--sheet", "1", "--range", "A:D", "--width", "16", "--out", cur)
	validate(t, cur)
	prev = cur
	cur = next()
	run(t, "xlsx", "rowheights", "set", prev, "--sheet", "1", "--range", "1:1", "--height", "22", "--out", cur)
	validate(t, cur)

	// 6. Data validation on the category column.
	prev = cur
	cur = next()
	run(t, "xlsx", "data-validations", "create", prev, "--sheet", "1", "--range", "A2:A4",
		"--type", "list", "--list-values", "North,South,East,West", "--out", cur)
	validate(t, cur)

	// 7. A hyperlink.
	prev = cur
	cur = next()
	run(t, "xlsx", "hyperlinks", "add", prev, "--sheet", "1", "--cell", "A1", "--url", "https://example.com", "--out", cur)
	validate(t, cur)

	// 8. A defined name over the data.
	prev = cur
	cur = next()
	run(t, "xlsx", "names", "add", prev, "--name", "SalesData", "--sheet", "1", "--range", "A1:D4", "--out", cur)
	validate(t, cur)

	// 9. A chart authored from the range.
	prev = cur
	cur = next()
	run(t, "xlsx", "charts", "create", prev, "--type", "bar", "--sheet", "1", "--range", "A1:C4",
		"--title", "Quarterly Sales", "--anchor", "F1", "--out", cur)
	validate(t, cur)

	// Final: confirm a real spreadsheet engine opens the fully-built workbook.
	openWithLibreOfficeIfAvailable(t, cur)
}

func openWithLibreOfficeIfAvailable(t *testing.T, path string) {
	t.Helper()
	bin := ""
	for _, name := range []string{"libreoffice", "soffice"} {
		if p, err := exec.LookPath(name); err == nil {
			bin = p
			break
		}
	}
	if bin == "" {
		t.Log("libreoffice not available; skipping headless-open check")
		return
	}
	baselineOut := t.TempDir()
	baselineCmd := exec.Command(bin, libreOfficeUserInstallationArg(t), "--headless", "--convert-to", "csv", "--outdir", baselineOut, getXLSXTestFilePath("minimal-workbook"))
	if out, err := baselineCmd.CombinedOutput(); err != nil {
		t.Logf("libreoffice cannot open known-good XLSX fixture; skipping headless-open check: %v\n%s", err, out)
		return
	}
	outDir := t.TempDir()
	cmd := exec.Command(bin, libreOfficeUserInstallationArg(t), "--headless", "--convert-to", "csv", "--outdir", outDir, path)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("libreoffice failed to open %s: %v\n%s", path, err, out)
	}
}

func libreOfficeUserInstallationArg(t *testing.T) string {
	t.Helper()
	profileDir := t.TempDir()
	return "-env:UserInstallation=" + (&url.URL{Scheme: "file", Path: profileDir}).String()
}
