package cli

import (
	"encoding/json"
	"path/filepath"
	"testing"
)

func TestPPTXChartsSetTitle(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "title.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-title", deckPath,
		"--chart", "chart:1", "--title", "New Title", "--expect-title", "Revenue by Region",
		"--font-size", "20", "--font-italic", "--out", out)
	if err != nil {
		t.Fatalf("pptx set-title failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal pptx set-title: %v\n%s", err, output)
	}
	if res.Action != "pptx.chart.set-title" || res.PreviousTitle != "Revenue by Region" || res.Output != out {
		t.Fatalf("unexpected pptx set-title envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("missing chart style readback: %+v", res)
	}
	title := res.Chart.Style.Title
	if title.Text != "New Title" || title.Font == nil || title.Font.SizePt != 20 || title.Font.Italic == nil || !*title.Font.Italic {
		t.Fatalf("unexpected pptx title readback: %+v / %+v", title, title.Font)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after pptx set-title failed: %v", err)
	}
}

func TestPPTXChartsSetLegendAndSeriesStyle(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	dir := t.TempDir()

	// The fixture chart has no legend; set-legend must create one.
	legendPath := filepath.Join(dir, "legend.pptx")
	legendOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-legend", deckPath,
		"--chart", "chart:1", "--position", "top", "--overlay=false", "--out", legendPath)
	if err != nil {
		t.Fatalf("pptx set-legend failed: %v\n%s", err, legendOut)
	}
	var legendRes PPTXChartStyleResult
	if err := json.Unmarshal([]byte(legendOut), &legendRes); err != nil {
		t.Fatalf("unmarshal pptx set-legend: %v\n%s", err, legendOut)
	}
	if legendRes.Action != "pptx.chart.set-legend" || legendRes.Output != legendPath {
		t.Fatalf("unexpected pptx set-legend envelope: %+v", legendRes)
	}
	gotLegend := legendRes.Chart.Style.Legend
	if !gotLegend.Present || gotLegend.Position != "t" || gotLegend.Overlay == nil || *gotLegend.Overlay {
		t.Fatalf("unexpected pptx legend readback: %+v", gotLegend)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", legendPath); err != nil {
		t.Fatalf("validate after pptx set-legend failed: %v", err)
	}

	// set-series-style fill on the bar series, with a series-count guard.
	stylePath := filepath.Join(dir, "styled.pptx")
	styleOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-series-style", legendPath,
		"--chart", "chart:1", "--series", "1", "--fill-color", "#2ca02c", "--expect-series-count", "1", "--out", stylePath)
	if err != nil {
		t.Fatalf("pptx set-series-style failed: %v\n%s", err, styleOut)
	}
	var styleRes PPTXChartStyleResult
	if err := json.Unmarshal([]byte(styleOut), &styleRes); err != nil {
		t.Fatalf("unmarshal pptx set-series-style: %v\n%s", err, styleOut)
	}
	if styleRes.Action != "pptx.chart.set-series-style" || styleRes.Series != 1 {
		t.Fatalf("unexpected pptx set-series-style envelope: %+v", styleRes)
	}
	if len(styleRes.Chart.Style.Series) != 1 || styleRes.Chart.Style.Series[0].FillColor != "2CA02C" {
		t.Fatalf("unexpected pptx series readback: %+v", styleRes.Chart.Style.Series)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", stylePath); err != nil {
		t.Fatalf("validate after pptx set-series-style failed: %v", err)
	}

	// The generated chart show command must read both edits back.
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, styleRes.ChartShowCommand)
	var showRes PPTXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showRes); err != nil {
		t.Fatalf("unmarshal pptx chart show: %v\n%s", err, showOutput)
	}
	if len(showRes.Charts) != 1 || showRes.Charts[0].Style == nil {
		t.Fatalf("pptx chart show missing style: %s", showOutput)
	}
	finalStyle := showRes.Charts[0].Style
	if len(finalStyle.Series) != 1 || finalStyle.Series[0].FillColor != "2CA02C" {
		t.Fatalf("pptx show readback series fill mismatch: %+v", finalStyle.Series)
	}
	if finalStyle.Legend.Position != "t" {
		t.Fatalf("pptx show readback lost legend: %+v", finalStyle.Legend)
	}
}

func TestPPTXChartsStyleMarkerRejectedOnBar(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "x.pptx")
	args := []string{"pptx", "charts", "set-series-style", deckPath, "--chart", "chart:1", "--series", "1", "--marker-symbol", "circle", "--out", out}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestPPTXChartSetPlotAreaFill(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "fill.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-plot-area-fill", deckPath,
		"--chart", "chart:1", "--fill-color", "#F5F5F5", "--out", out)
	if err != nil {
		t.Fatalf("pptx set-plot-area-fill failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.Action != "pptx.chart.set-plot-area-fill" || res.NewFill != "F5F5F5" {
		t.Fatalf("unexpected envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil || res.Chart.Style.PlotAreaFill != "F5F5F5" {
		t.Fatalf("plot-area fill not in readback: %+v", res.Chart)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	openPPTXWithLibreOfficeIfAvailable(t, out)
}

func TestPPTXChartSetChartAreaFillFontFamily(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	dir := t.TempDir()
	// set title font family
	titled := filepath.Join(dir, "titled.pptx")
	if _, err := executeRootForXLSXTest(t, "pptx", "charts", "set-title", deckPath,
		"--chart", "chart:1", "--title", "Fonts", "--font-family", "Verdana", "--out", titled); err != nil {
		t.Fatalf("set-title --font-family failed: %v", err)
	}
	out := filepath.Join(dir, "area.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-chart-area-fill", titled,
		"--chart", "chart:1", "--fill-color", "#FFFFFF", "--out", out)
	if err != nil {
		t.Fatalf("pptx set-chart-area-fill failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.Chart == nil || res.Chart.Style == nil || res.Chart.Style.ChartSpaceFill != "FFFFFF" {
		t.Fatalf("chart-area fill not in readback: %+v", res.Chart)
	}
	if res.Chart.Style.Title.Font == nil || res.Chart.Style.Title.Font.Family != "Verdana" {
		t.Fatalf("title font family lost: %+v", res.Chart.Style.Title.Font)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
}

func TestPPTXChartCopyStyleRoundTrip(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	dir := t.TempDir()

	// Build a styled template deck (chart:1).
	tmpl1 := filepath.Join(dir, "tmpl1.pptx")
	if _, err := executeRootForXLSXTest(t, "pptx", "charts", "set-legend", deckPath,
		"--chart", "chart:1", "--position", "bottom", "--out", tmpl1); err != nil {
		t.Fatalf("template set-legend failed: %v", err)
	}
	tmpl := filepath.Join(dir, "tmpl.pptx")
	if _, err := executeRootForXLSXTest(t, "pptx", "charts", "set-plot-area-fill", tmpl1,
		"--chart", "chart:1", "--fill-color", "#EEEEEE", "--out", tmpl); err != nil {
		t.Fatalf("template set-plot-area-fill failed: %v", err)
	}

	// Apply onto the target deck's chart:1, using the --to-chart alias.
	copied := filepath.Join(dir, "copied.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "copy-style", deckPath,
		"--to-chart", "chart:1", "--from", tmpl, "--from-chart", "chart:1", "--out", copied)
	if err != nil {
		t.Fatalf("pptx copy-style failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal copy-style: %v\n%s", err, output)
	}
	if res.Action != "pptx.chart.copy-style" || len(res.AppliedStyle) == 0 {
		t.Fatalf("unexpected copy-style envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("copy-style missing chart style readback")
	}
	got := res.Chart.Style
	if got.Legend.Position != "b" {
		t.Errorf("legend not copied: %q", got.Legend.Position)
	}
	if got.PlotAreaFill != "EEEEEE" {
		t.Errorf("plot-area fill not copied: %q", got.PlotAreaFill)
	}
	// Target keeps its own title text (content not copied).
	if got.Title.Text != "Revenue by Region" {
		t.Errorf("target title text was overwritten: %q", got.Title.Text)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", copied); err != nil {
		t.Fatalf("validate after copy-style failed: %v", err)
	}
	openPPTXWithLibreOfficeIfAvailable(t, copied)
}
