package chart

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

func openChartWorkbookForStyleTest(t *testing.T) (*opc.Package, string) {
	t.Helper()
	workbookPath := filepath.Join("..", "..", "..", "testdata", "xlsx", "chart-workbook", "workbook.xlsx")
	pkg, err := opc.Open(workbookPath)
	if err != nil {
		t.Fatalf("failed to open chart workbook: %v", err)
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("failed to parse workbook: %v", err)
	}
	charts, err := List(pkg, workbook, nil)
	if err != nil {
		pkg.Close()
		t.Fatalf("List failed: %v", err)
	}
	if len(charts) != 1 {
		pkg.Close()
		t.Fatalf("charts len = %d, want 1", len(charts))
	}
	return pkg, charts[0].PartURI
}

func TestChartStyleSettersRoundTrip(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	size := 16.0
	titleRes, err := SetTitle(&SetTitleRequest{
		Package: pkg, ChartURI: uri, Text: "Updated",
		ExpectText: strPtr("Revenue by Region"),
		FontSizePt: &size, FontColor: "#0a0b0c", FontBold: boolPtr(true),
	})
	if err != nil {
		t.Fatalf("SetTitle failed: %v", err)
	}
	if titleRes.PreviousText != "Revenue by Region" || titleRes.Title.Text != "Updated" {
		t.Fatalf("unexpected title result: %+v", titleRes)
	}
	if titleRes.Title.Font == nil || titleRes.Title.Font.SizePt != 16 || titleRes.Title.Font.Bold == nil || !*titleRes.Title.Font.Bold || titleRes.Title.Font.Color != "0A0B0C" {
		t.Fatalf("unexpected title font: %+v", titleRes.Title.Font)
	}

	legendRes, err := SetLegend(&SetLegendRequest{Package: pkg, ChartURI: uri, SetPosition: true, Position: "l", Overlay: boolPtr(true)})
	if err != nil {
		t.Fatalf("SetLegend failed: %v", err)
	}
	if !legendRes.Legend.Present || legendRes.Legend.Position != "l" || legendRes.Legend.Overlay == nil || !*legendRes.Legend.Overlay {
		t.Fatalf("unexpected legend result: %+v", legendRes.Legend)
	}

	count := 1
	seriesRes, err := SetSeriesStyle(&SetSeriesStyleRequest{Package: pkg, ChartURI: uri, SeriesNumber: 1, FillColor: "#FF0000", ExpectSeriesCount: &count})
	if err != nil {
		t.Fatalf("SetSeriesStyle failed: %v", err)
	}
	if seriesRes.SeriesCount != 1 || seriesRes.Series.FillColor != "FF0000" {
		t.Fatalf("unexpected series result: %+v", seriesRes)
	}

	// Markers are not valid on a bar chart series.
	if _, err := SetSeriesStyle(&SetSeriesStyleRequest{Package: pkg, ChartURI: uri, SeriesNumber: 1, MarkerSymbol: "circle"}); err == nil || !strings.Contains(err.Error(), "does not support markers") {
		t.Fatalf("expected marker rejection on bar series, got %v", err)
	}

	// All three edits must persist together.
	style, err := InspectStyle(pkg, uri)
	if err != nil {
		t.Fatalf("InspectStyle failed: %v", err)
	}
	if style.Title.Text != "Updated" || style.Legend.Position != "l" || len(style.Series) != 1 || style.Series[0].FillColor != "FF0000" {
		t.Fatalf("style did not round-trip: %+v", style)
	}
}

func TestSetTitleRejectsStaleGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	if _, err := SetTitle(&SetTitleRequest{Package: pkg, ChartURI: uri, Text: "X", ExpectText: strPtr("WRONG")}); err == nil || !strings.Contains(err.Error(), "title mismatch") {
		t.Fatalf("expected stale title guard error, got %v", err)
	}
}

func TestSetSeriesStyleRejectsCountGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	count := 5
	if _, err := SetSeriesStyle(&SetSeriesStyleRequest{Package: pkg, ChartURI: uri, SeriesNumber: 1, FillColor: "#112233", ExpectSeriesCount: &count}); err == nil || !strings.Contains(err.Error(), "series count mismatch") {
		t.Fatalf("expected series-count guard error, got %v", err)
	}
}

func TestNormalizeHexColor(t *testing.T) {
	valid := map[string]string{
		"#1f77b4":   "1F77B4",
		"1F77B4":    "1F77B4",
		" #abcdef ": "ABCDEF",
	}
	for in, want := range valid {
		got, err := NormalizeHexColor(in)
		if err != nil || got != want {
			t.Fatalf("NormalizeHexColor(%q) = (%q, %v), want %q", in, got, err, want)
		}
	}
	for _, bad := range []string{"", "12345", "1234567", "nothex", "#GG0000"} {
		if _, err := NormalizeHexColor(bad); err == nil {
			t.Fatalf("NormalizeHexColor(%q) should have failed", bad)
		}
	}
}

func float64Ptr(v float64) *float64 { return &v }

func TestSetAxisRoundTrip(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	// Category axis: title + tick-label font.
	catRes, err := SetAxis(&SetAxisRequest{
		Package: pkg, ChartURI: uri, AxisKind: "category",
		SetTitle: true, Title: "Region",
		TickLabelFontSizePt: float64Ptr(9), TickLabelFontColor: "#333333", TickLabelFontBold: boolPtr(true),
		ExpectAxisCount: func() *int { c := 2; return &c }(),
	})
	if err != nil {
		t.Fatalf("SetAxis category failed: %v", err)
	}
	if catRes.AxisCount != 2 || catRes.Axis.Title != "Region" {
		t.Fatalf("unexpected category axis result: %+v", catRes.Axis)
	}
	if catRes.Axis.TickLabelFont == nil || catRes.Axis.TickLabelFont.SizePt != 9 || catRes.Axis.TickLabelFont.Color != "333333" {
		t.Fatalf("unexpected tick label font: %+v", catRes.Axis.TickLabelFont)
	}

	// Value axis: hide flag, scale, major unit, number format, gridlines, title font.
	valRes, err := SetAxis(&SetAxisRequest{
		Package: pkg, ChartURI: uri, AxisKind: "value",
		SetTitle: true, Title: "Sales", TitleFontSizePt: float64Ptr(12), TitleFontBold: boolPtr(true),
		SetHidden: true, Hidden: false,
		Min: float64Ptr(0), Max: float64Ptr(100), MajorUnit: float64Ptr(25),
		NumberFormat:      "#,##0",
		SetMajorGridlines: true, MajorGridlines: true,
		SetMinorGridlines: true, MinorGridlines: true,
	})
	if err != nil {
		t.Fatalf("SetAxis value failed: %v", err)
	}
	v := valRes.Axis
	if v.Title != "Sales" || v.NumberFormat != "#,##0" {
		t.Fatalf("unexpected value axis title/format: %+v", v)
	}
	if v.Min == nil || *v.Min != 0 || v.Max == nil || *v.Max != 100 || v.MajorUnit == nil || *v.MajorUnit != 25 {
		t.Fatalf("unexpected value axis scale: %+v", v)
	}
	if !v.MajorGridlines || !v.MinorGridlines {
		t.Fatalf("expected both gridlines on: %+v", v)
	}
	if v.Hidden == nil || *v.Hidden {
		t.Fatalf("expected value axis visible (delete=0): %+v", v.Hidden)
	}
	if v.TitleFont == nil || v.TitleFont.SizePt != 12 || v.TitleFont.Bold == nil || !*v.TitleFont.Bold {
		t.Fatalf("unexpected value axis title font: %+v", v.TitleFont)
	}

	// All edits must persist together when the whole part is re-read.
	style, err := InspectStyle(pkg, uri)
	if err != nil {
		t.Fatalf("InspectStyle failed: %v", err)
	}
	if len(style.Axes) != 2 {
		t.Fatalf("axes len = %d, want 2", len(style.Axes))
	}
	for _, ax := range style.Axes {
		switch ax.Kind {
		case "category":
			if ax.Title != "Region" || ax.TickLabelFont == nil {
				t.Fatalf("category axis did not round-trip: %+v", ax)
			}
		case "value":
			if ax.Title != "Sales" || ax.NumberFormat != "#,##0" || ax.MajorUnit == nil || *ax.MajorUnit != 25 || !ax.MajorGridlines || !ax.MinorGridlines {
				t.Fatalf("value axis did not round-trip: %+v", ax)
			}
		}
	}
}

func TestSetAxisGridlinesToggleOff(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "value", SetMajorGridlines: true, MajorGridlines: true}); err != nil {
		t.Fatalf("turn gridlines on failed: %v", err)
	}
	res, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "value", SetMajorGridlines: true, MajorGridlines: false})
	if err != nil {
		t.Fatalf("turn gridlines off failed: %v", err)
	}
	if res.Axis.MajorGridlines {
		t.Fatalf("expected major gridlines off, got on: %+v", res.Axis)
	}
}

func TestSetAxisExpectTitleGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "value", SetTitle: true, Title: "X", ExpectAxisTitle: strPtr("Nope")}); err == nil || !strings.Contains(err.Error(), "axis title mismatch") {
		t.Fatalf("expected axis title guard error, got %v", err)
	}
}

func TestSetAxisExpectCountGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	count := 9
	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "value", SetTitle: true, Title: "X", ExpectAxisCount: &count}); err == nil || !strings.Contains(err.Error(), "axis count mismatch") {
		t.Fatalf("expected axis count guard error, got %v", err)
	}
}

func TestSetAxisRejectsMajorUnitOnCategoryAxis(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	mu := 5.0
	// majorUnit has no slot in the catAx schema; setting it there would append
	// out of order and Excel would reject the file, so it must be refused.
	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "category", MajorUnit: &mu}); err == nil || !strings.Contains(err.Error(), "value axis") {
		t.Fatalf("expected major-unit-on-category rejection, got %v", err)
	}
}

// TestSetAxisRejectsAmbiguousValueAxes proves the scatter-style case where a
// chart carries two value axes: --axis value must be rejected as ambiguous. The
// fixtures are bar charts (one catAx + one valAx), so a second valAx is injected
// directly into the chart part to exercise the rejection branch.
func TestSetAxisRejectsAmbiguousValueAxes(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("ReadXMLPart failed: %v", err)
	}
	plotArea := firstDescendant(doc.Root(), "plotArea")
	if plotArea == nil {
		t.Fatalf("no plotArea in fixture")
	}
	valAx := firstDirectChild(plotArea, "valAx")
	if valAx == nil {
		t.Fatalf("no valAx in fixture")
	}
	dup := valAx.Copy()
	// Give the clone a distinct axId so the two are clearly separate axes.
	if id := firstDirectChild(dup, "axId"); id != nil {
		id.CreateAttr("val", "999999")
	}
	plotArea.AddChild(dup)
	if err := pkg.ReplaceXMLPart(uri, doc); err != nil {
		t.Fatalf("ReplaceXMLPart failed: %v", err)
	}

	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "value", SetTitle: true, Title: "X"}); err == nil || !strings.Contains(err.Error(), "ambiguous") {
		t.Fatalf("expected ambiguous value-axis rejection, got %v", err)
	}
	// The single category axis must still be selectable unambiguously.
	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "category", SetTitle: true, Title: "Region"}); err != nil {
		t.Fatalf("category axis should remain selectable: %v", err)
	}
}

func TestSetAxisRejectsInvalidKind(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	if _, err := SetAxis(&SetAxisRequest{Package: pkg, ChartURI: uri, AxisKind: "foo", SetTitle: true, Title: "X"}); err == nil || !strings.Contains(err.Error(), "category or value") {
		t.Fatalf("expected invalid-kind error, got %v", err)
	}
}

func TestSetTitleFontFamilyRoundTrip(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	size := 13.0
	res, err := SetTitle(&SetTitleRequest{Package: pkg, ChartURI: uri, Text: "Fonts", FontFamily: "Verdana", FontSizePt: &size})
	if err != nil {
		t.Fatalf("SetTitle failed: %v", err)
	}
	if res.Title.Font == nil || res.Title.Font.Family != "Verdana" || res.Title.Font.SizePt != 13 {
		t.Fatalf("title font did not round-trip: %+v", res.Title.Font)
	}
	style, err := InspectStyle(pkg, uri)
	if err != nil {
		t.Fatalf("InspectStyle failed: %v", err)
	}
	if style.Title.Font == nil || style.Title.Font.Family != "Verdana" {
		t.Fatalf("title font family not persisted: %+v", style.Title.Font)
	}
}

func TestSetAxisTitleFontFamily(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	res, err := SetAxis(&SetAxisRequest{
		Package: pkg, ChartURI: uri, AxisKind: "category",
		SetTitle: true, Title: "Cats", TitleFontFamily: "Tahoma",
	})
	if err != nil {
		t.Fatalf("SetAxis failed: %v", err)
	}
	if res.Axis.Title != "Cats" || res.Axis.TitleFont == nil || res.Axis.TitleFont.Family != "Tahoma" {
		t.Fatalf("axis title font family not applied: %+v", res.Axis)
	}
}

func TestSetPlotAreaFillRoundTrip(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	res, err := SetPlotAreaFill(&SetFillRequest{Package: pkg, ChartURI: uri, FillColor: "#F5F5F5"})
	if err != nil {
		t.Fatalf("SetPlotAreaFill failed: %v", err)
	}
	if res.NewFill != "F5F5F5" {
		t.Fatalf("unexpected new fill: %q", res.NewFill)
	}
	if res.Style == nil || res.Style.PlotAreaFill != "F5F5F5" {
		t.Fatalf("plot-area fill not persisted: %+v", res.Style)
	}
	// Clearing it writes a noFill and reports the previous color.
	cleared, err := SetPlotAreaFill(&SetFillRequest{Package: pkg, ChartURI: uri, NoFill: true, ExpectFill: strPtr("F5F5F5")})
	if err != nil {
		t.Fatalf("SetPlotAreaFill clear failed: %v", err)
	}
	if cleared.PreviousFill != "F5F5F5" || cleared.NewFill != "" {
		t.Fatalf("unexpected clear result: %+v", cleared)
	}
	if cleared.Style == nil || cleared.Style.PlotAreaFill != "" {
		t.Fatalf("plot-area fill not cleared: %+v", cleared.Style)
	}
}

func TestSetChartAreaFillRoundTrip(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	res, err := SetChartAreaFill(&SetFillRequest{Package: pkg, ChartURI: uri, FillColor: "FFFFFF"})
	if err != nil {
		t.Fatalf("SetChartAreaFill failed: %v", err)
	}
	if res.Style == nil || res.Style.ChartSpaceFill != "FFFFFF" {
		t.Fatalf("chart-area fill not persisted: %+v", res.Style)
	}
}

func TestSetFillRejectsStaleGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	if _, err := SetPlotAreaFill(&SetFillRequest{Package: pkg, ChartURI: uri, FillColor: "#112233", ExpectFill: strPtr("AABBCC")}); err == nil || !strings.Contains(err.Error(), "fill mismatch") {
		t.Fatalf("expected fill guard error, got %v", err)
	}
}

func TestApplyStyleCopiesStyleNotContent(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	// Record the target's original content (title text + series name) so we can
	// assert copy-style leaves it untouched.
	before, err := InspectStyle(pkg, uri)
	if err != nil {
		t.Fatalf("InspectStyle (before) failed: %v", err)
	}
	originalTitle := before.Title.Text
	originalSeriesName := ""
	if len(before.Series) > 0 {
		originalSeriesName = before.Series[0].Name
	}

	// Build a template style with distinct style fields and bogus content.
	bold := true
	source := &ChartStyle{
		Title: TitleStyle{
			Present: true,
			Text:    "TEMPLATE TITLE TEXT (must not be copied)",
			Font:    &FontStyle{Family: "Verdana", SizePt: 20, Bold: &bold, Color: "112233"},
		},
		Legend: LegendStyle{Present: true, Position: "b"},
		Axes: []AxisStyle{
			{Element: "catAx", Kind: "category", MajorGridlines: true, NumberFormat: "0.00"},
		},
		Series: []SeriesStyle{
			{Number: 1, Name: "TEMPLATE SERIES (must not be copied)", FillColor: "4472C4"},
		},
		PlotAreaFill:   "F5F5F5",
		ChartSpaceFill: "FFFFFF",
	}

	res, err := ApplyStyle(&ApplyStyleRequest{Package: pkg, ChartURI: uri, Source: source})
	if err != nil {
		t.Fatalf("ApplyStyle failed: %v", err)
	}
	if len(res.Applied) == 0 {
		t.Fatalf("ApplyStyle applied nothing")
	}
	got := res.Style
	if got == nil {
		t.Fatalf("ApplyStyle returned no style readback")
	}

	// Style fields adopted from the template.
	if got.Legend.Position != "b" {
		t.Errorf("legend position not copied: %q", got.Legend.Position)
	}
	if got.PlotAreaFill != "F5F5F5" {
		t.Errorf("plot-area fill not copied: %q", got.PlotAreaFill)
	}
	if got.ChartSpaceFill != "FFFFFF" {
		t.Errorf("chart-area fill not copied: %q", got.ChartSpaceFill)
	}
	if len(got.Series) == 0 || got.Series[0].FillColor != "4472C4" {
		t.Errorf("series fill not copied: %+v", got.Series)
	}
	if got.Title.Font == nil || got.Title.Font.Family != "Verdana" || got.Title.Font.SizePt != 20 {
		t.Errorf("title font not copied: %+v", got.Title.Font)
	}

	// Content preserved: title text and series name unchanged.
	if got.Title.Text != originalTitle {
		t.Errorf("title text was overwritten: got %q want %q", got.Title.Text, originalTitle)
	}
	if len(got.Series) > 0 && got.Series[0].Name != originalSeriesName {
		t.Errorf("series name was overwritten: got %q want %q", got.Series[0].Name, originalSeriesName)
	}
}

func TestApplyStyleCopiesSchemeColoredSeries(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	// A template series styled with theme (scheme) colors must copy through, not
	// be silently dropped the way hex-only handling would.
	source := &ChartStyle{
		Series: []SeriesStyle{{Number: 1, FillColor: "scheme:accent1", LineColor: "scheme:accent2"}},
	}
	res, err := ApplyStyle(&ApplyStyleRequest{Package: pkg, ChartURI: uri, Source: source})
	if err != nil {
		t.Fatalf("ApplyStyle failed: %v", err)
	}
	if res.Style == nil || len(res.Style.Series) == 0 {
		t.Fatalf("no series readback: %+v", res.Style)
	}
	got := res.Style.Series[0]
	if got.FillColor != "scheme:accent1" {
		t.Errorf("scheme series fill not copied: %q", got.FillColor)
	}
	if got.LineColor != "scheme:accent2" {
		t.Errorf("scheme series line not copied: %q", got.LineColor)
	}
}

func TestApplyStyleRejectsSeriesCountGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()
	count := 9
	if _, err := ApplyStyle(&ApplyStyleRequest{Package: pkg, ChartURI: uri, Source: &ChartStyle{}, ExpectSeriesCount: &count}); err == nil || !strings.Contains(err.Error(), "series count mismatch") {
		t.Fatalf("expected series-count guard error, got %v", err)
	}
}

func strPtr(s string) *string { return &s }
