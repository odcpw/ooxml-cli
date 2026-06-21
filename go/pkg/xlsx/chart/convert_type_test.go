package chart

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func convertType(t *testing.T, pkg opc.PackageSession, uri string, to ChartType, expect *ChartType) (*ConvertChartTypeResult, error) {
	t.Helper()
	return ConvertChartType(&ConvertChartTypeRequest{
		Package:    pkg,
		ChartURI:   uri,
		TargetType: to,
		ExpectType: expect,
	})
}

func plotElementName(t *testing.T, pkg opc.PackageSession, uri string) string {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read chart part: %v", err)
	}
	plot := firstPlotElement(firstDescendant(doc.Root(), "plotArea"))
	if plot == nil {
		t.Fatalf("no plot element in %s", uri)
	}
	return localName(plot.Tag)
}

func axisElements(t *testing.T, pkg opc.PackageSession, uri string) []string {
	t.Helper()
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		t.Fatalf("read chart part: %v", err)
	}
	plotArea := firstDescendant(doc.Root(), "plotArea")
	var names []string
	for _, child := range plotArea.ChildElements() {
		switch localName(child.Tag) {
		case "catAx", "valAx", "dateAx", "serAx":
			names = append(names, localName(child.Tag))
		}
	}
	return names
}

func TestParseChartType(t *testing.T) {
	cases := map[string]ChartType{
		"bar": ChartTypeBar, "BAR": ChartTypeBar, "column": ChartTypeColumn,
		"col": ChartTypeColumn, "line": ChartTypeLine, "area": ChartTypeArea,
		"pie": ChartTypePie, "scatter": ChartTypeScatter, "xy": ChartTypeScatter,
	}
	for in, want := range cases {
		got, err := ParseChartType(in)
		if err != nil || got != want {
			t.Fatalf("ParseChartType(%q) = %q, %v; want %q", in, got, err, want)
		}
	}
	if _, err := ParseChartType("bubble"); err == nil {
		t.Fatalf("expected error for unknown chart type")
	}
}

// TestConvertColumnToBarFlipsBarDir verifies bar<->column reuses c:barChart and
// only flips barDir, preserving the element and axis IDs.
func TestConvertColumnToBarFlipsBarDir(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	res, err := convertType(t, pkg, uri, ChartTypeBar, nil)
	if err != nil {
		t.Fatalf("convert column->bar failed: %v", err)
	}
	if res.PreviousType != ChartTypeColumn || res.NewType != ChartTypeBar {
		t.Fatalf("unexpected result: %+v", res)
	}
	if plotElementName(t, pkg, uri) != "barChart" {
		t.Fatalf("expected barChart element after bar<->column flip")
	}
	doc, _ := pkg.ReadXMLPart(uri)
	plot := firstPlotElement(firstDescendant(doc.Root(), "plotArea"))
	if dir := firstDirectChild(plot, "barDir"); dir == nil || dir.SelectAttrValue("val", "") != "bar" {
		t.Fatalf("expected barDir=bar, got %+v", dir)
	}
	// axId values must be preserved from the source plot.
	if ids := plotAxisIDs(plot); len(ids) != 2 || ids[0] != "123456" || ids[1] != "654321" {
		t.Fatalf("axId not preserved: %v", ids)
	}
}

// TestConvertColumnToLinePreservesSeries verifies a real element change keeps
// the series source refs and carries the axis IDs into the new plot.
func TestConvertColumnToLinePreservesSeries(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	res, err := convertType(t, pkg, uri, ChartTypeLine, nil)
	if err != nil {
		t.Fatalf("convert column->line failed: %v", err)
	}
	if res.NewType != ChartTypeLine {
		t.Fatalf("unexpected new type: %+v", res)
	}
	if plotElementName(t, pkg, uri) != "lineChart" {
		t.Fatalf("expected lineChart element")
	}
	style, err := InspectStyle(pkg, uri)
	if err != nil {
		t.Fatalf("inspect: %v", err)
	}
	if len(style.Series) != 1 || style.Series[0].Name == "" {
		t.Fatalf("series source not preserved: %+v", style.Series)
	}
	if got := axisElements(t, pkg, uri); len(got) != 2 || got[0] != "catAx" || got[1] != "valAx" {
		t.Fatalf("unexpected axes after line conversion: %v", got)
	}
	doc, _ := pkg.ReadXMLPart(uri)
	plot := firstPlotElement(firstDescendant(doc.Root(), "plotArea"))
	if ids := plotAxisIDs(plot); len(ids) != 2 || ids[0] != "123456" {
		t.Fatalf("axId not carried into line plot: %v", ids)
	}
}

// TestConvertToScatterRenamesSources verifies cat->xVal, val->yVal, and that the
// category axis becomes a value axis, with a non-numeric-x warning.
func TestConvertToScatterRenamesSources(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	res, err := convertType(t, pkg, uri, ChartTypeScatter, nil)
	if err != nil {
		t.Fatalf("convert column->scatter failed: %v", err)
	}
	if plotElementName(t, pkg, uri) != "scatterChart" {
		t.Fatalf("expected scatterChart element")
	}
	doc, _ := pkg.ReadXMLPart(uri)
	ser := walkSeries(doc.Root())[0]
	if firstDirectChild(ser, "xVal") == nil || firstDirectChild(ser, "yVal") == nil {
		t.Fatalf("series not renamed to xVal/yVal")
	}
	if firstDirectChild(ser, "cat") != nil || firstDirectChild(ser, "val") != nil {
		t.Fatalf("cat/val should have been renamed away")
	}
	// Both axes are value axes for a scatter chart.
	if got := axisElements(t, pkg, uri); len(got) != 2 || got[0] != "valAx" || got[1] != "valAx" {
		t.Fatalf("expected two value axes for scatter, got %v", got)
	}
	if !containsSubstr(res.Warnings, "numeric x-values") {
		t.Fatalf("expected non-numeric x warning, got %v", res.Warnings)
	}

	// Converting back to a cat-family type renames xVal/yVal to cat/val and the
	// value-position axis back to a category axis (pruning CT_ValAx-only tail).
	if _, err := convertType(t, pkg, uri, ChartTypeLine, nil); err != nil {
		t.Fatalf("scatter->line failed: %v", err)
	}
	doc2, _ := pkg.ReadXMLPart(uri)
	ser2 := walkSeries(doc2.Root())[0]
	if firstDirectChild(ser2, "cat") == nil || firstDirectChild(ser2, "val") == nil {
		t.Fatalf("scatter->line should restore cat/val sources")
	}
	if got := axisElements(t, pkg, uri); len(got) != 2 || got[0] != "catAx" {
		t.Fatalf("scatter->line should restore a category axis, got %v", got)
	}
}

// TestConvertToPieDropsAxes verifies single-series pie conversion drops axes.
func TestConvertToPieDropsAxes(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	if _, err := convertType(t, pkg, uri, ChartTypePie, nil); err != nil {
		t.Fatalf("convert to pie failed: %v", err)
	}
	if plotElementName(t, pkg, uri) != "pieChart" {
		t.Fatalf("expected pieChart element")
	}
	if got := axisElements(t, pkg, uri); len(got) != 0 {
		t.Fatalf("pie chart should have no axes, got %v", got)
	}
	// pie keeps both cat and val sources.
	doc, _ := pkg.ReadXMLPart(uri)
	ser := walkSeries(doc.Root())[0]
	if firstDirectChild(ser, "cat") == nil || firstDirectChild(ser, "val") == nil {
		t.Fatalf("pie series should keep cat and val sources")
	}
}

func TestConvertFromPieRejected(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	if _, err := convertType(t, pkg, uri, ChartTypePie, nil); err != nil {
		t.Fatalf("setup convert to pie failed: %v", err)
	}
	_, err := convertType(t, pkg, uri, ChartTypeBar, nil)
	if err == nil || !strings.Contains(err.Error(), "cannot convert from pie") {
		t.Fatalf("expected pie->bar rejection, got %v", err)
	}
}

func TestConvertExpectTypeGuard(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	wrong := ChartTypePie
	if _, err := convertType(t, pkg, uri, ChartTypeLine, &wrong); err == nil || !strings.Contains(err.Error(), "type mismatch") {
		t.Fatalf("expected expect-type mismatch, got %v", err)
	}
	// Matching guard passes.
	right := ChartTypeColumn
	if _, err := convertType(t, pkg, uri, ChartTypeLine, &right); err != nil {
		t.Fatalf("matching guard should pass: %v", err)
	}
}

func TestConvertIdenticalTypeRejected(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	if _, err := convertType(t, pkg, uri, ChartTypeColumn, nil); err == nil || !strings.Contains(err.Error(), "already a column") {
		t.Fatalf("expected identical-type rejection, got %v", err)
	}
}

// TestConvertRoundTrip column->line->column keeps the series source.
func TestConvertRoundTrip(t *testing.T) {
	pkg, uri := openChartWorkbookForStyleTest(t)
	defer pkg.Close()

	if _, err := convertType(t, pkg, uri, ChartTypeLine, nil); err != nil {
		t.Fatalf("to line: %v", err)
	}
	if _, err := convertType(t, pkg, uri, ChartTypeColumn, nil); err != nil {
		t.Fatalf("back to column: %v", err)
	}
	if plotElementName(t, pkg, uri) != "barChart" {
		t.Fatalf("expected barChart after round trip")
	}
	style, err := InspectStyle(pkg, uri)
	if err != nil {
		t.Fatalf("inspect: %v", err)
	}
	if len(style.Series) != 1 || style.Series[0].Name == "" {
		t.Fatalf("round-trip lost series source: %+v", style.Series)
	}
}

func containsSubstr(list []string, sub string) bool {
	for _, s := range list {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}
