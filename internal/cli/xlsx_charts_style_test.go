package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
)

// stageLineChart authors a single-series line chart and returns its path.
func stageLineChart(t *testing.T) string {
	t.Helper()
	data := stageChartData(t, `[["Region","Sales"],["North",42],["South",58],["East",30]]`)
	out := filepath.Join(t.TempDir(), "linechart.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data,
		"--type", "line", "--sheet", "1", "--range", "A1:B4", "--title", "Original", "--anchor", "D1", "--out", out); err != nil {
		t.Fatalf("stage line chart failed: %v", err)
	}
	return out
}

func TestXLSXChartsSetTitleLegendSeriesStyle(t *testing.T) {
	dir := t.TempDir()
	chartPath := stageLineChart(t)

	// set-title with font.
	titlePath := filepath.Join(dir, "title.xlsx")
	titleOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-title", chartPath,
		"--title", "Quarterly Sales", "--expect-title", "Original",
		"--font-size", "18", "--font-color", "#1f77b4", "--font-bold", "--out", titlePath)
	if err != nil {
		t.Fatalf("set-title failed: %v\n%s", err, titleOut)
	}
	var titleRes XLSXChartStyleResult
	if err := json.Unmarshal([]byte(titleOut), &titleRes); err != nil {
		t.Fatalf("unmarshal set-title: %v\n%s", err, titleOut)
	}
	if titleRes.Action != "xlsx.chart.set-title" || titleRes.Output != titlePath || titleRes.DryRun || titleRes.PreviousTitle != "Original" {
		t.Fatalf("unexpected set-title envelope: %+v", titleRes)
	}
	if titleRes.Chart == nil || titleRes.Chart.Style == nil {
		t.Fatalf("set-title missing chart style readback: %+v", titleRes)
	}
	gotTitle := titleRes.Chart.Style.Title
	if !gotTitle.Present || gotTitle.Text != "Quarterly Sales" {
		t.Fatalf("unexpected title readback: %+v", gotTitle)
	}
	if gotTitle.Font == nil || gotTitle.Font.SizePt != 18 || gotTitle.Font.Bold == nil || !*gotTitle.Font.Bold || gotTitle.Font.Color != "1F77B4" {
		t.Fatalf("unexpected title font readback: %+v", gotTitle.Font)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", titlePath); err != nil {
		t.Fatalf("validate after set-title failed: %v", err)
	}
	// The generated chart show command must read the new title back.
	showStyle := xlsxChartStyleFromShowCommand(t, titleRes.ChartShowCommand)
	if showStyle.Title.Text != "Quarterly Sales" {
		t.Fatalf("show readback title = %q, want Quarterly Sales", showStyle.Title.Text)
	}

	// set-legend.
	legendPath := filepath.Join(dir, "legend.xlsx")
	legendOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-legend", titlePath,
		"--position", "right", "--overlay", "--out", legendPath)
	if err != nil {
		t.Fatalf("set-legend failed: %v\n%s", err, legendOut)
	}
	var legendRes XLSXChartStyleResult
	if err := json.Unmarshal([]byte(legendOut), &legendRes); err != nil {
		t.Fatalf("unmarshal set-legend: %v\n%s", err, legendOut)
	}
	if legendRes.Action != "xlsx.chart.set-legend" || legendRes.LegendRemoved {
		t.Fatalf("unexpected set-legend envelope: %+v", legendRes)
	}
	gotLegend := legendRes.Chart.Style.Legend
	if !gotLegend.Present || gotLegend.Position != "r" || gotLegend.Overlay == nil || !*gotLegend.Overlay {
		t.Fatalf("unexpected legend readback: %+v", gotLegend)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", legendPath); err != nil {
		t.Fatalf("validate after set-legend failed: %v", err)
	}

	// set-series-style on a line series (markers supported).
	stylePath := filepath.Join(dir, "styled.xlsx")
	styleOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-series-style", legendPath,
		"--series", "1", "--line-color", "#FF0000", "--line-width-pt", "2.5",
		"--marker-symbol", "circle", "--marker-size", "7", "--expect-series-count", "1", "--out", stylePath)
	if err != nil {
		t.Fatalf("set-series-style failed: %v\n%s", err, styleOut)
	}
	var styleRes XLSXChartStyleResult
	if err := json.Unmarshal([]byte(styleOut), &styleRes); err != nil {
		t.Fatalf("unmarshal set-series-style: %v\n%s", err, styleOut)
	}
	if styleRes.Action != "xlsx.chart.set-series-style" || styleRes.Series != 1 {
		t.Fatalf("unexpected set-series-style envelope: %+v", styleRes)
	}
	if len(styleRes.Chart.Style.Series) != 1 {
		t.Fatalf("series readback len = %d, want 1: %+v", len(styleRes.Chart.Style.Series), styleRes.Chart.Style.Series)
	}
	series := styleRes.Chart.Style.Series[0]
	if series.LineColor != "FF0000" || series.LineWidthPt == nil || *series.LineWidthPt != 2.5 {
		t.Fatalf("unexpected series line readback: %+v", series)
	}
	if series.Marker == nil || series.Marker.Symbol != "circle" || series.Marker.Size != 7 {
		t.Fatalf("unexpected series marker readback: %+v", series.Marker)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", stylePath); err != nil {
		t.Fatalf("validate after set-series-style failed: %v", err)
	}
	// The generated show command must read the series style back, and the legend
	// and title set earlier must survive.
	finalStyle := xlsxChartStyleFromShowCommand(t, styleRes.ChartShowCommand)
	if finalStyle.Series[0].LineColor != "FF0000" || finalStyle.Series[0].Marker == nil || finalStyle.Series[0].Marker.Symbol != "circle" {
		t.Fatalf("show readback series style mismatch: %+v", finalStyle.Series[0])
	}
	if finalStyle.Legend.Position != "r" || finalStyle.Title.Text != "Quarterly Sales" {
		t.Fatalf("show readback lost earlier title/legend: %+v / %+v", finalStyle.Title, finalStyle.Legend)
	}
}

func TestXLSXChartsSetTitleCreatesMissingTitle(t *testing.T) {
	// A chart authored without --title has autoTitleDeleted=1 and no title element;
	// set-title must create the title element and flip autoTitleDeleted, and the
	// font must survive a re-read of the written file (locks element ordering and
	// font serialization, which the in-memory readback alone would not catch).
	data := stageChartData(t, `[["Region","Sales"],["North",42],["South",58]]`)
	dir := t.TempDir()
	bare := filepath.Join(dir, "bare.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data, "--type", "bar", "--sheet", "1", "--range", "A1:B3", "--anchor", "D1", "--out", bare); err != nil {
		t.Fatalf("create chart without title failed: %v", err)
	}
	titled := filepath.Join(dir, "titled.xlsx")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-title", bare,
		"--title", "Fresh Title", "--font-size", "14", "--font-bold", "--out", titled)
	if err != nil {
		t.Fatalf("set-title on untitled chart failed: %v\n%s", err, out)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if res.PreviousTitle != "" {
		t.Fatalf("previous title should be empty for an untitled chart: %q", res.PreviousTitle)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", titled); err != nil {
		t.Fatalf("validate after creating title failed: %v", err)
	}
	// Re-read the written file via the generated show command: title text and font
	// must both round-trip through serialization.
	style := xlsxChartStyleFromShowCommand(t, res.ChartShowCommand)
	if !style.Title.Present || style.Title.Text != "Fresh Title" {
		t.Fatalf("title not created/read back: %+v", style.Title)
	}
	if style.Title.Font == nil || style.Title.Font.SizePt != 14 || style.Title.Font.Bold == nil || !*style.Title.Font.Bold {
		t.Fatalf("title font did not survive written-file re-read: %+v", style.Title.Font)
	}
}

func TestXLSXChartsSetLegendRemove(t *testing.T) {
	dir := t.TempDir()
	chartPath := stageLineChart(t)
	withLegend := filepath.Join(dir, "with-legend.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "set-legend", chartPath, "--position", "bottom", "--out", withLegend); err != nil {
		t.Fatalf("set-legend bottom failed: %v", err)
	}
	removed := filepath.Join(dir, "no-legend.xlsx")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-legend", withLegend, "--position", "none", "--out", removed)
	if err != nil {
		t.Fatalf("set-legend none failed: %v\n%s", err, out)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if !res.LegendRemoved || res.Chart.Style.Legend.Present {
		t.Fatalf("legend should be removed: %+v", res)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", removed); err != nil {
		t.Fatalf("validate after legend remove failed: %v", err)
	}
}

func TestXLSXChartsSetTitleDryRun(t *testing.T) {
	chartPath := stageLineChart(t)
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-title", chartPath,
		"--title", "Draft", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run set-title failed: %v\n%s", err, out)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("unmarshal dry-run: %v\n%s", err, out)
	}
	if !res.DryRun || res.Output != "" {
		t.Fatalf("unexpected dry-run envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil || res.Chart.Style.Title.Text != "Draft" {
		t.Fatalf("dry-run should preview the new title: %+v", res.Chart)
	}
	if !strings.Contains(res.ValidateCommandTemplate, "<out.xlsx>") || !strings.Contains(res.ChartShowCommandTemplate, "<out.xlsx>") {
		t.Fatalf("dry-run templates missing placeholder: %+v", res)
	}
}

func TestXLSXChartsStyleGuards(t *testing.T) {
	data := stageChartData(t, `[["Region","Sales"],["North",42],["South",58]]`)
	barPath := filepath.Join(t.TempDir(), "bar.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data, "--type", "bar", "--sheet", "1", "--range", "A1:B3", "--title", "Bar", "--anchor", "D1", "--out", barPath); err != nil {
		t.Fatalf("create bar chart failed: %v", err)
	}
	out := filepath.Join(t.TempDir(), "guard.xlsx")
	cases := [][]string{
		{"xlsx", "charts", "set-title", barPath, "--title", "X", "--expect-title", "WRONG", "--out", out},
		{"xlsx", "charts", "set-series-style", barPath, "--series", "1", "--marker-symbol", "circle", "--out", out},
		{"xlsx", "charts", "set-series-style", barPath, "--series", "1", "--fill-color", "#112233", "--expect-series-count", "5", "--out", out},
		{"xlsx", "charts", "set-series-style", barPath, "--series", "1", "--out", out},
		{"xlsx", "charts", "set-series-style", barPath, "--series", "1", "--fill-color", "nothex", "--out", out},
		{"xlsx", "charts", "set-legend", barPath, "--out", out},
	}
	for _, args := range cases {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}

func TestXLSXChartSetTitleFontFamily(t *testing.T) {
	chartPath := stageLineChart(t)
	out := filepath.Join(t.TempDir(), "fam.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-title", chartPath,
		"--title", "Fonts", "--font-family", "Verdana", "--font-size", "12", "--out", out)
	if err != nil {
		t.Fatalf("set-title --font-family failed: %v\n%s", err, output)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.Chart == nil || res.Chart.Style == nil || res.Chart.Style.Title.Font == nil || res.Chart.Style.Title.Font.Family != "Verdana" {
		t.Fatalf("title font family not in readback: %+v", res.Chart)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	openWithLibreOfficeIfAvailable(t, out)
}

func TestXLSXChartSetPlotAndChartAreaFill(t *testing.T) {
	dir := t.TempDir()
	chartPath := stageLineChart(t)

	plot := filepath.Join(dir, "plot.xlsx")
	plotOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-plot-area-fill", chartPath,
		"--fill-color", "#F5F5F5", "--out", plot)
	if err != nil {
		t.Fatalf("set-plot-area-fill failed: %v\n%s", err, plotOut)
	}
	var plotRes XLSXChartStyleResult
	if err := json.Unmarshal([]byte(plotOut), &plotRes); err != nil {
		t.Fatalf("unmarshal plot: %v\n%s", err, plotOut)
	}
	if plotRes.Action != "xlsx.chart.set-plot-area-fill" || plotRes.NewFill != "F5F5F5" {
		t.Fatalf("unexpected plot envelope: %+v", plotRes)
	}
	if plotRes.Chart == nil || plotRes.Chart.Style == nil || plotRes.Chart.Style.PlotAreaFill != "F5F5F5" {
		t.Fatalf("plot-area fill not in readback: %+v", plotRes.Chart)
	}

	area := filepath.Join(dir, "area.xlsx")
	areaOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-chart-area-fill", plot,
		"--fill-color", "#FFFFFF", "--expect-fill", "none", "--out", area)
	if err != nil {
		t.Fatalf("set-chart-area-fill failed: %v\n%s", err, areaOut)
	}
	var areaRes XLSXChartStyleResult
	if err := json.Unmarshal([]byte(areaOut), &areaRes); err != nil {
		t.Fatalf("unmarshal area: %v\n%s", err, areaOut)
	}
	if areaRes.Chart == nil || areaRes.Chart.Style == nil || areaRes.Chart.Style.ChartSpaceFill != "FFFFFF" {
		t.Fatalf("chart-area fill not in readback: %+v", areaRes.Chart)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", area); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	openWithLibreOfficeIfAvailable(t, area)

	// Stale guard must be rejected.
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "set-plot-area-fill", area,
		"--fill-color", "#000000", "--expect-fill", "#AABBCC", "--out", filepath.Join(dir, "no.xlsx")); err == nil {
		t.Fatalf("expected fill guard rejection")
	}
}

func TestXLSXChartCopyStyleRoundTrip(t *testing.T) {
	dir := t.TempDir()

	// Build a styled template chart.
	base := stageLineChart(t)
	tmpl1 := filepath.Join(dir, "t1.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "set-title", base, "--title", "Template", "--font-family", "Verdana", "--font-size", "16", "--out", tmpl1); err != nil {
		t.Fatalf("template set-title failed: %v", err)
	}
	tmpl2 := filepath.Join(dir, "t2.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "set-legend", tmpl1, "--position", "bottom", "--out", tmpl2); err != nil {
		t.Fatalf("template set-legend failed: %v", err)
	}
	tmpl3 := filepath.Join(dir, "t3.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "set-series-style", tmpl2, "--series", "1", "--fill-color", "#4472C4", "--out", tmpl3); err != nil {
		t.Fatalf("template set-series-style failed: %v", err)
	}
	tmpl := filepath.Join(dir, "tmpl.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "set-plot-area-fill", tmpl3, "--fill-color", "#EEEEEE", "--out", tmpl); err != nil {
		t.Fatalf("template set-plot-area-fill failed: %v", err)
	}

	// Fresh target with its own distinct title text.
	target := stageLineChart(t)

	copied := filepath.Join(dir, "copied.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "copy-style", target,
		"--from", tmpl, "--out", copied)
	if err != nil {
		t.Fatalf("copy-style failed: %v\n%s", err, output)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal copy-style: %v\n%s", err, output)
	}
	if res.Action != "xlsx.chart.copy-style" || len(res.AppliedStyle) == 0 {
		t.Fatalf("unexpected copy-style envelope: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("copy-style missing chart style readback")
	}
	got := res.Chart.Style

	// Target adopts the template's STYLE fields.
	if got.Legend.Position != "b" {
		t.Errorf("legend not copied: %q", got.Legend.Position)
	}
	if got.PlotAreaFill != "EEEEEE" {
		t.Errorf("plot-area fill not copied: %q", got.PlotAreaFill)
	}
	if len(got.Series) == 0 || got.Series[0].FillColor != "4472C4" {
		t.Errorf("series fill not copied: %+v", got.Series)
	}
	if got.Title.Font == nil || got.Title.Font.Family != "Verdana" {
		t.Errorf("title font not copied: %+v", got.Title.Font)
	}
	// Target keeps its own title text (content not copied).
	if got.Title.Text != "Original" {
		t.Errorf("target title text was overwritten: %q", got.Title.Text)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", copied); err != nil {
		t.Fatalf("validate after copy-style failed: %v", err)
	}
	openWithLibreOfficeIfAvailable(t, copied)
}

func xlsxChartStyleFromShowCommand(t *testing.T, command string) *xlsxchart.ChartStyle {
	t.Helper()
	if command == "" {
		t.Fatalf("empty chart show command")
	}
	output := executeGeneratedOOXMLCommandForXLSXTest(t, command)
	var res XLSXChartsResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal show output: %v\n%s", err, output)
	}
	if len(res.Charts) != 1 || res.Charts[0].Style == nil {
		t.Fatalf("show output missing chart style: %s", output)
	}
	return res.Charts[0].Style
}
