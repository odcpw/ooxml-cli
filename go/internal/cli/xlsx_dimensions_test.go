package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXColWidthsSetAndShowReadback(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "colwidths.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "colwidths", "set", workbookPath,
		"--sheet", "1",
		"--range", "B:C",
		"--width", "30",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("colwidths set failed: %v", err)
	}
	var setResult XLSXColWidthsSetResult
	if err := json.Unmarshal([]byte(output), &setResult); err != nil {
		t.Fatalf("failed to unmarshal colwidths set JSON: %v\n%s", err, output)
	}
	if setResult.Range != "B:C" || setResult.Columns != 2 || setResult.Width != 30 {
		t.Fatalf("unexpected colwidths set result: %+v", setResult)
	}
	if setResult.Output != outPath || setResult.DryRun {
		t.Fatalf("unexpected colwidths mutation metadata: %+v", setResult)
	}
	if setResult.ColWidthsShowCommand == "" || !strings.Contains(setResult.ColWidthsShowCommand, "colwidths show") {
		t.Fatalf("missing colwidths show command: %+v", setResult)
	}
	if !strings.Contains(setResult.ColWidthsShowCommand, "--json") {
		t.Fatalf("colwidths show command should request --json: %s", setResult.ColWidthsShowCommand)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, setResult.ColWidthsShowCommand)
	var showResult XLSXColWidthsShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal colwidths show JSON: %v\n%s", err, showOut)
	}
	if showResult.Columns["B"].Width == nil || *showResult.Columns["B"].Width != 30 {
		t.Fatalf("expected B width 30: %+v", showResult.Columns["B"])
	}
	if !showResult.Columns["B"].Explicit {
		t.Fatalf("expected B width to be explicit: %+v", showResult.Columns["B"])
	}
	if showResult.Columns["C"].Width == nil || *showResult.Columns["C"].Width != 30 {
		t.Fatalf("expected C width 30: %+v", showResult.Columns["C"])
	}
}

func TestXLSXColWidthsSetSplitsExistingSpan(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	wide := filepath.Join(t.TempDir(), "wide.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "colwidths", "set", workbookPath, "--sheet", "1", "--range", "B:D", "--width", "30", "--out", wide); err != nil {
		t.Fatalf("initial colwidths set failed: %v", err)
	}
	narrow := filepath.Join(t.TempDir(), "narrow.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "colwidths", "set", wide, "--sheet", "1", "--range", "C:C", "--width", "12", "--out", narrow); err != nil {
		t.Fatalf("split colwidths set failed: %v", err)
	}
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "colwidths", "show", narrow, "--sheet", "1", "--range", "B:D")
	if err != nil {
		t.Fatalf("colwidths show failed: %v", err)
	}
	var show XLSXColWidthsShowResult
	if err := json.Unmarshal([]byte(out), &show); err != nil {
		t.Fatalf("failed to unmarshal show: %v\n%s", err, out)
	}
	got := map[string]float64{}
	for k, v := range show.Columns {
		if v.Width != nil {
			got[k] = *v.Width
		}
	}
	if got["B"] != 30 || got["C"] != 12 || got["D"] != 30 {
		t.Fatalf("unexpected widths after split: %+v", got)
	}
}

func TestXLSXColWidthsExpectGuardMismatch(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "guard.xlsx")
	_, err := executeRootForXLSXTest(t,
		"xlsx", "colwidths", "set", workbookPath,
		"--sheet", "1", "--range", "B:B", "--width", "40",
		"--expect-width", "99",
		"--out", outPath,
	)
	if err == nil {
		t.Fatalf("expected guard mismatch error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXRowHeightsSetAndShowReadback(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "rowheights.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rowheights", "set", workbookPath,
		"--sheet", "1",
		"--range", "2:4",
		"--height", "25",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("rowheights set failed: %v", err)
	}
	var setResult XLSXRowHeightsSetResult
	if err := json.Unmarshal([]byte(output), &setResult); err != nil {
		t.Fatalf("failed to unmarshal rowheights set JSON: %v\n%s", err, output)
	}
	if setResult.Range != "2:4" || setResult.Rows != 3 || setResult.Height != 25 {
		t.Fatalf("unexpected rowheights set result: %+v", setResult)
	}
	if setResult.RowHeightsShowCommand == "" || !strings.Contains(setResult.RowHeightsShowCommand, "--json") {
		t.Fatalf("missing or non-json rowheights show command: %+v", setResult)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, setResult.RowHeightsShowCommand)
	var showResult XLSXRowHeightsShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal rowheights show JSON: %v\n%s", err, showOut)
	}
	if showResult.Rows["2"].Height == nil || *showResult.Rows["2"].Height != 25 {
		t.Fatalf("expected row 2 height 25: %+v", showResult.Rows["2"])
	}
	if !showResult.Rows["2"].Explicit {
		t.Fatalf("expected row 2 height to be explicit: %+v", showResult.Rows["2"])
	}
}

func TestXLSXColWidthsShowDefaults(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "colwidths", "show", workbookPath, "--sheet", "1", "--range", "A:B")
	if err != nil {
		t.Fatalf("colwidths show failed: %v", err)
	}
	var show XLSXColWidthsShowResult
	if err := json.Unmarshal([]byte(out), &show); err != nil {
		t.Fatalf("failed to unmarshal show: %v\n%s", err, out)
	}
	if !show.Uniform {
		t.Fatalf("expected uniform default widths: %+v", show)
	}
	if show.Columns["A"].Explicit {
		t.Fatalf("expected default (non-explicit) width for A: %+v", show.Columns["A"])
	}
}

func TestXLSXRowHeightsRequiresHeightFlag(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	_, err := executeRootForXLSXTest(t, "xlsx", "rowheights", "set", workbookPath, "--sheet", "1", "--range", "2:3", "--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected error when --height missing")
	}
}
