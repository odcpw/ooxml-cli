package cli

import (
	"encoding/json"
	"path/filepath"
	"testing"

	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
)

// findAxisByKind returns the axis with the given kind, failing the test if absent.
func findAxisByKind(t *testing.T, axes []xlsxchart.AxisStyle, kind string) xlsxchart.AxisStyle {
	t.Helper()
	for _, ax := range axes {
		if ax.Kind == kind {
			return ax
		}
	}
	t.Fatalf("no %s axis in readback: %+v", kind, axes)
	return xlsxchart.AxisStyle{}
}

func TestPPTXChartsSetAxis(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "axis.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-axis", deckPath,
		"--chart", "chart:1", "--axis", "value",
		"--title", "Revenue", "--min", "0", "--max", "200", "--number-format", "#,##0",
		"--tick-label-font-size", "10", "--tick-label-font-italic",
		"--out", out)
	if err != nil {
		t.Fatalf("pptx set-axis failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal pptx set-axis: %v\n%s", err, output)
	}
	if res.Action != "pptx.chart.set-axis" || res.Output != out {
		t.Fatalf("unexpected pptx set-axis envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("missing chart style readback: %+v", res)
	}
	valAx := findAxisByKind(t, res.Chart.Style.Axes, "value")
	if valAx.Title != "Revenue" || valAx.NumberFormat != "#,##0" {
		t.Fatalf("unexpected pptx value axis readback: %+v", valAx)
	}
	if valAx.Min == nil || *valAx.Min != 0 || valAx.Max == nil || *valAx.Max != 200 {
		t.Fatalf("unexpected pptx value axis scale: %+v", valAx)
	}
	if valAx.TickLabelFont == nil || valAx.TickLabelFont.SizePt != 10 || valAx.TickLabelFont.Italic == nil || !*valAx.TickLabelFont.Italic {
		t.Fatalf("unexpected pptx tick label font: %+v", valAx.TickLabelFont)
	}
	if res.RenderCommand == "" {
		t.Fatalf("pptx set-axis should generate a render command: %+v", res)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after pptx set-axis failed: %v", err)
	}

	// The generated chart show command must read the axis edits back.
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, res.ChartShowCommand)
	var showRes PPTXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showRes); err != nil {
		t.Fatalf("unmarshal pptx chart show: %v\n%s", err, showOutput)
	}
	if len(showRes.Charts) != 1 || showRes.Charts[0].Style == nil {
		t.Fatalf("pptx chart show missing style: %s", showOutput)
	}
	showVal := findAxisByKind(t, showRes.Charts[0].Style.Axes, "value")
	if showVal.Title != "Revenue" || showVal.NumberFormat != "#,##0" {
		t.Fatalf("pptx show readback axis mismatch: %+v", showVal)
	}
}

func TestPPTXChartsSetAxisGuards(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "x.pptx")
	cases := [][]string{
		{"pptx", "charts", "set-axis", deckPath, "--chart", "chart:1", "--title", "X", "--out", out},
		{"pptx", "charts", "set-axis", deckPath, "--chart", "chart:1", "--axis", "value", "--out", out},
		{"pptx", "charts", "set-axis", deckPath, "--chart", "chart:1", "--axis", "value", "--title", "X", "--expect-axis-count", "9", "--out", out},
	}
	for _, args := range cases {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}
