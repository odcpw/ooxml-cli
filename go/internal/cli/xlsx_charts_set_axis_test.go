package cli

import (
	"encoding/json"
	"path/filepath"
	"testing"
)

func TestXLSXChartsSetAxis(t *testing.T) {
	dir := t.TempDir()
	chartPath := stageLineChart(t)

	out := filepath.Join(dir, "axis.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-axis", chartPath,
		"--sheet", "1", "--chart", "1", "--axis", "value",
		"--title", "Sales", "--min", "0", "--max", "100", "--major-unit", "25",
		"--number-format", "#,##0", "--major-gridlines=true", "--minor-gridlines=true",
		"--tick-label-font-size", "9", "--tick-label-font-color", "#333333", "--tick-label-font-bold",
		"--out", out)
	if err != nil {
		t.Fatalf("set-axis failed: %v\n%s", err, output)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal set-axis: %v\n%s", err, output)
	}
	if res.Action != "xlsx.chart.set-axis" || res.Output != out || res.DryRun {
		t.Fatalf("unexpected set-axis envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("set-axis missing chart style readback: %+v", res)
	}
	valAx := findAxisByKind(t, res.Chart.Style.Axes, "value")
	if valAx.Title != "Sales" || valAx.NumberFormat != "#,##0" {
		t.Fatalf("unexpected value axis readback: %+v", valAx)
	}
	if valAx.Min == nil || *valAx.Min != 0 || valAx.Max == nil || *valAx.Max != 100 || valAx.MajorUnit == nil || *valAx.MajorUnit != 25 {
		t.Fatalf("unexpected value axis scale readback: %+v", valAx)
	}
	if !valAx.MajorGridlines || !valAx.MinorGridlines {
		t.Fatalf("expected both gridlines on: %+v", valAx)
	}
	if valAx.TickLabelFont == nil || valAx.TickLabelFont.SizePt != 9 || valAx.TickLabelFont.Color != "333333" || valAx.TickLabelFont.Bold == nil || !*valAx.TickLabelFont.Bold {
		t.Fatalf("unexpected tick label font readback: %+v", valAx.TickLabelFont)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after set-axis failed: %v", err)
	}
	// The generated chart show command must read the axis edits back.
	showStyle := xlsxChartStyleFromShowCommand(t, res.ChartShowCommand)
	showVal := findAxisByKind(t, showStyle.Axes, "value")
	if showVal.Title != "Sales" || showVal.NumberFormat != "#,##0" || !showVal.MajorGridlines {
		t.Fatalf("show readback axis mismatch: %+v", showVal)
	}

	// A second mutation on the category axis must leave the value axis intact.
	catOut := filepath.Join(dir, "axis-cat.xlsx")
	catOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-axis", out,
		"--sheet", "1", "--chart", "1", "--axis", "category", "--title", "Region", "--hidden", "--out", catOut)
	if err != nil {
		t.Fatalf("set-axis category failed: %v\n%s", err, catOutput)
	}
	var catRes XLSXChartStyleResult
	if err := json.Unmarshal([]byte(catOutput), &catRes); err != nil {
		t.Fatalf("unmarshal set-axis category: %v\n%s", err, catOutput)
	}
	catAx := findAxisByKind(t, catRes.Chart.Style.Axes, "category")
	if catAx.Title != "Region" || catAx.Hidden == nil || !*catAx.Hidden {
		t.Fatalf("unexpected category axis readback: %+v", catAx)
	}
	stillVal := findAxisByKind(t, catRes.Chart.Style.Axes, "value")
	if stillVal.Title != "Sales" {
		t.Fatalf("category mutation clobbered value axis: %+v", stillVal)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", catOut); err != nil {
		t.Fatalf("validate after category set-axis failed: %v", err)
	}
}

func TestXLSXChartsSetAxisDryRun(t *testing.T) {
	chartPath := stageLineChart(t)
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-axis", chartPath,
		"--sheet", "1", "--chart", "1", "--axis", "value", "--title", "Draft", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run set-axis failed: %v\n%s", err, out)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("unmarshal dry-run: %v\n%s", err, out)
	}
	if !res.DryRun || res.Output != "" {
		t.Fatalf("unexpected dry-run envelope: %+v", res)
	}
	valAx := findAxisByKind(t, res.Chart.Style.Axes, "value")
	if valAx.Title != "Draft" {
		t.Fatalf("dry-run should preview the new axis title: %+v", valAx)
	}
}

func TestXLSXChartsSetAxisGuards(t *testing.T) {
	chartPath := stageLineChart(t)
	out := filepath.Join(t.TempDir(), "guard.xlsx")
	cases := [][]string{
		// missing --axis
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--title", "X", "--out", out},
		// invalid --axis
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--axis", "foo", "--title", "X", "--out", out},
		// no mutation flags
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--axis", "value", "--out", out},
		// expect-axis-title mismatch
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--axis", "value", "--title", "X", "--expect-axis-title", "WRONG", "--out", out},
		// expect-axis-count mismatch
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--axis", "value", "--title", "X", "--expect-axis-count", "9", "--out", out},
		// min >= max
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--axis", "value", "--min", "100", "--max", "0", "--out", out},
		// bad tick font color
		{"xlsx", "charts", "set-axis", chartPath, "--sheet", "1", "--chart", "1", "--axis", "value", "--tick-label-font-color", "nothex", "--out", out},
	}
	for _, args := range cases {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}
