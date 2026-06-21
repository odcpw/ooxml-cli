package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

// stageChartData writes a small header+category+series dataset and returns the path.
func stageChartData(t *testing.T, values string) string {
	t.Helper()
	wb := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "data.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "ranges", "set", wb, "--sheet", "1", "--anchor", "A1", "--values", values, "--out", out); err != nil {
		t.Fatalf("stage data failed: %v", err)
	}
	return out
}

func TestXLSXChartsCreateBarAndReadback(t *testing.T) {
	data := stageChartData(t, `[["Region","Sales"],["North",42],["South",58],["East",30]]`)
	out := filepath.Join(t.TempDir(), "chart.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", data,
		"--type", "bar", "--sheet", "1", "--range", "A1:B4", "--title", "Sales", "--anchor", "D1", "--out", out)
	if err != nil {
		t.Fatalf("charts create failed: %v\n%s", err, output)
	}
	var res XLSXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal failed: %v\n%s", err, output)
	}
	if res.ChartType != "bar" || res.SeriesCount != 1 || res.Categories != 3 {
		t.Fatalf("unexpected create result: %+v", res)
	}
	if res.ChartPartURI == "" || res.DrawingURI == "" {
		t.Fatalf("missing chart/drawing part URIs: %+v", res)
	}
	if res.ChartsListCommand == "" || !strings.Contains(res.ChartsListCommand, "charts list") {
		t.Fatalf("missing charts list readback command: %+v", res)
	}
	// Output must validate, and the chart must be discoverable via list.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, res.ChartsListCommand)
	if !strings.Contains(listOut, "chart1.xml") {
		t.Fatalf("expected chart in list readback: %s", listOut)
	}
}

func TestXLSXChartsCreateMultiSeries(t *testing.T) {
	data := stageChartData(t, `[["Region","Q1","Q2"],["North",42,50],["South",58,61]]`)
	out := filepath.Join(t.TempDir(), "chart.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", data,
		"--type", "line", "--sheet", "1", "--range", "A1:C3", "--out", out)
	if err != nil {
		t.Fatalf("charts create failed: %v", err)
	}
	var res XLSXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.SeriesCount != 2 || res.Categories != 2 {
		t.Fatalf("expected 2 series, 2 categories: %+v", res)
	}
}

func TestXLSXChartsCreateMultipleChartsOnSameSheet(t *testing.T) {
	data := stageChartData(t, `[["Region","Q1","Q2"],["North",42,50],["South",58,61],["East",30,33]]`)
	dir := t.TempDir()
	firstPath := filepath.Join(dir, "chart1.xlsx")
	firstOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", data,
		"--type", "bar", "--sheet", "1", "--range", "A1:C4", "--title", "First", "--anchor", "F1", "--out", firstPath)
	if err != nil {
		t.Fatalf("first charts create failed: %v\n%s", err, firstOut)
	}
	var first XLSXChartsCreateResult
	if err := json.Unmarshal([]byte(firstOut), &first); err != nil {
		t.Fatalf("unmarshal first: %v\n%s", err, firstOut)
	}

	secondPath := filepath.Join(dir, "chart2.xlsx")
	secondOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", firstPath,
		"--type", "line", "--sheet", "1", "--range", "A1:C4", "--title", "Second", "--anchor", "F18", "--out", secondPath)
	if err != nil {
		t.Fatalf("second charts create failed: %v\n%s", err, secondOut)
	}
	var second XLSXChartsCreateResult
	if err := json.Unmarshal([]byte(secondOut), &second); err != nil {
		t.Fatalf("unmarshal second: %v\n%s", err, secondOut)
	}
	if second.DrawingURI != first.DrawingURI {
		t.Fatalf("second chart should reuse worksheet drawing: first=%s second=%s", first.DrawingURI, second.DrawingURI)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", secondPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}

	listOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "list", secondPath)
	if err != nil {
		t.Fatalf("charts list failed: %v", err)
	}
	var list XLSXChartsResult
	if err := json.Unmarshal([]byte(listOut), &list); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, listOut)
	}
	if len(list.Charts) != 2 {
		t.Fatalf("chart count = %d, want 2: %+v", len(list.Charts), list.Charts)
	}
	if list.Charts[0].DrawingPartURI != list.Charts[1].DrawingPartURI {
		t.Fatalf("charts should share drawing part: %+v", list.Charts)
	}
	if list.Charts[0].PartURI == list.Charts[1].PartURI {
		t.Fatalf("charts should have distinct chart parts: %+v", list.Charts)
	}
}

func TestXLSXChartsCreatePieLimitsToOneSeries(t *testing.T) {
	data := stageChartData(t, `[["Region","Q1","Q2"],["North",42,50],["South",58,61]]`)
	out := filepath.Join(t.TempDir(), "pie.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", data,
		"--type", "pie", "--sheet", "1", "--range", "A1:C3", "--out", out)
	if err != nil {
		t.Fatalf("charts create failed: %v", err)
	}
	var res XLSXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if res.SeriesCount != 1 {
		t.Fatalf("pie should limit to 1 series: %+v", res)
	}
	if len(res.Warnings) == 0 {
		t.Fatalf("expected a warning about pie series limit")
	}
}

func TestXLSXChartsCreateScatterCoercesNonNumericX(t *testing.T) {
	// String categories must not land in the scatter xVal numCache; they are
	// coerced to 1-based positions so the chart stays schema-valid.
	data := stageChartData(t, `[["Region","Sales"],["North",42],["South",58]]`)
	out := filepath.Join(t.TempDir(), "scatter.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", data,
		"--type", "scatter", "--sheet", "1", "--range", "A1:B3", "--out", out)
	if err != nil {
		t.Fatalf("scatter create failed: %v\n%s", err, output)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
}

func TestXLSXChartsCreateInvalidType(t *testing.T) {
	data := stageChartData(t, `[["A","B"],["x",1]]`)
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data, "--type", "bogus", "--sheet", "1", "--range", "A1:B2", "--out", filepath.Join(t.TempDir(), "x.xlsx")); err == nil {
		t.Fatalf("expected error for invalid chart type")
	}
}

func TestXLSXChartsCreateExpectSourceRangeGuard(t *testing.T) {
	data := stageChartData(t, `[["A","B"],["x",1],["y",2]]`)
	_, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data, "--type", "bar", "--sheet", "1", "--range", "A1:B3", "--expect-source-range", "Z1:Z9", "--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected source-range guard mismatch")
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}
