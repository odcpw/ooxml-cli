package chart

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// ChartType is a friendly chart-type name exposed by the convert-type command.
// It is distinct from the DrawingML plot element name because bar and column
// charts share the c:barChart element (differentiated by c:barDir).
type ChartType string

const (
	ChartTypeBar     ChartType = "bar"
	ChartTypeColumn  ChartType = "column"
	ChartTypeLine    ChartType = "line"
	ChartTypeArea    ChartType = "area"
	ChartTypePie     ChartType = "pie"
	ChartTypeScatter ChartType = "scatter"
)

// chartTypeAliases maps user-supplied --to / --expect-type values to a canonical
// ChartType. The plot element name alone is not authoritative for bar vs column.
var chartTypeAliases = map[string]ChartType{
	"bar":          ChartTypeBar,
	"barchart":     ChartTypeBar,
	"column":       ChartTypeColumn,
	"col":          ChartTypeColumn,
	"columnchart":  ChartTypeColumn,
	"line":         ChartTypeLine,
	"linechart":    ChartTypeLine,
	"area":         ChartTypeArea,
	"areachart":    ChartTypeArea,
	"pie":          ChartTypePie,
	"piechart":     ChartTypePie,
	"scatter":      ChartTypeScatter,
	"scatterchart": ChartTypeScatter,
	"xy":           ChartTypeScatter,
}

// ParseChartType resolves a friendly chart-type value into a canonical ChartType.
func ParseChartType(value string) (ChartType, error) {
	t, ok := chartTypeAliases[strings.ToLower(strings.TrimSpace(value))]
	if !ok {
		return "", fmt.Errorf("invalid chart type %q (use bar, column, line, area, pie, or scatter)", value)
	}
	return t, nil
}

// elementForChartType returns the DrawingML plot element local name for a type.
func elementForChartType(t ChartType) string {
	switch t {
	case ChartTypeBar, ChartTypeColumn:
		return "barChart"
	case ChartTypeLine:
		return "lineChart"
	case ChartTypeArea:
		return "areaChart"
	case ChartTypePie:
		return "pieChart"
	case ChartTypeScatter:
		return "scatterChart"
	default:
		return ""
	}
}

// canonicalChartType maps a plot element to its friendly ChartType, reading
// c:barDir to distinguish bar (barDir=bar) from column (barDir=col, the default).
func canonicalChartType(plot *etree.Element) (ChartType, error) {
	if plot == nil {
		return "", fmt.Errorf("chart has no plot element")
	}
	switch localName(plot.Tag) {
	case "barChart", "bar3DChart":
		if dir := firstDirectChild(plot, "barDir"); dir != nil {
			if strings.EqualFold(strings.TrimSpace(dir.SelectAttrValue("val", "")), "bar") {
				return ChartTypeBar, nil
			}
		}
		return ChartTypeColumn, nil
	case "lineChart", "line3DChart":
		return ChartTypeLine, nil
	case "areaChart", "area3DChart":
		return ChartTypeArea, nil
	case "pieChart", "pie3DChart", "doughnutChart", "ofPieChart":
		return ChartTypePie, nil
	case "scatterChart":
		return ChartTypeScatter, nil
	default:
		return "", fmt.Errorf("chart type %q is not supported for conversion", localName(plot.Tag))
	}
}

// ConvertChartTypeRequest converts the single plot of a DrawingML chart to a
// different chart type, preserving each series' source references and caches.
type ConvertChartTypeRequest struct {
	Package    opc.PackageSession
	ChartURI   string
	TargetType ChartType
	ExpectType *ChartType // guard: current type must equal this when set
}

// ConvertChartTypeResult reports the previous and new types plus non-fatal
// warnings raised during the conversion.
type ConvertChartTypeResult struct {
	PreviousType ChartType
	NewType      ChartType
	Warnings     []string
}

// ConvertChartType rewrites a chart's plot element to the target type. Series
// source refs and caches (c:tx/c:cat/c:val or c:xVal/c:yVal) are preserved.
// bar<->column flips c:barDir without rebuilding the plot. Converting to pie
// drops the axes and rejects multi-series charts; scatter renames the category
// axis to a value axis and the series cat/val sources to xVal/yVal.
func ConvertChartType(req *ConvertChartTypeRequest) (*ConvertChartTypeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("convert chart type request is nil")
	}
	if req.TargetType == "" {
		return nil, fmt.Errorf("--to target chart type is required")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	plotArea := firstDescendant(root, "plotArea")
	if plotArea == nil {
		return nil, fmt.Errorf("chart part %s has no plotArea", req.ChartURI)
	}
	oldPlot := firstPlotElement(plotArea)
	if oldPlot == nil {
		return nil, fmt.Errorf("chart part %s has no chart-type plot element", req.ChartURI)
	}

	previous, err := canonicalChartType(oldPlot)
	if err != nil {
		return nil, err
	}
	if req.ExpectType != nil && *req.ExpectType != previous {
		return nil, fmt.Errorf("chart type mismatch: expected %s but found %s; use --dry-run to inspect", *req.ExpectType, previous)
	}
	if previous == req.TargetType {
		return nil, fmt.Errorf("chart is already a %s chart", req.TargetType)
	}
	if err := checkConvertible(previous, req.TargetType); err != nil {
		return nil, err
	}

	series := directChildren(oldPlot, "ser")
	if len(series) == 0 {
		return nil, fmt.Errorf("chart has no series to convert")
	}
	if req.TargetType == ChartTypePie && len(series) > 1 {
		return nil, fmt.Errorf("cannot convert to pie: a pie chart supports a single series but this chart has %d; remove series before converting", len(series))
	}

	result := &ConvertChartTypeResult{PreviousType: previous, NewType: req.TargetType}
	b := newStyleBuilder(root)

	// bar<->column share the c:barChart element: flip barDir only.
	if elementForChartType(previous) == elementForChartType(req.TargetType) {
		setBarDir(b, oldPlot, req.TargetType)
		if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
		}
		return result, nil
	}

	// Collect the old plot's axId children (in order) so the new plot keeps the
	// real axis IDs rather than synthesizing fresh ones.
	axIDs := plotAxisIDs(oldPlot)

	// Transform each series in place for the target structure, accumulating
	// warnings, then move it into the freshly built plot wrapper.
	for i, ser := range series {
		warns := transformSeries(b, ser, previous, req.TargetType, i+1)
		result.Warnings = append(result.Warnings, warns...)
	}

	newPlot := buildPlotWrapper(b, req.TargetType, series, axIDs)
	idx := oldPlot.Index()
	plotArea.RemoveChild(oldPlot)
	plotArea.InsertChildAt(idx, newPlot)

	// Axis surgery: pie has no axes; scatter needs the category-position axis to
	// become a value axis; converting away from scatter renames it back.
	if axisWarn := transformAxes(plotArea, previous, req.TargetType, axIDs); axisWarn != "" {
		result.Warnings = append(result.Warnings, axisWarn)
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	return result, nil
}

// firstPlotElement returns the first chart-type plot element in a plotArea.
func firstPlotElement(plotArea *etree.Element) *etree.Element {
	for _, child := range plotArea.ChildElements() {
		if strings.HasSuffix(localName(child.Tag), "Chart") && child.NamespaceURI() == nsChart {
			return child
		}
	}
	return nil
}

// plotAxisIDs returns the val of each c:axId child of a plot element, in order.
func plotAxisIDs(plot *etree.Element) []string {
	var ids []string
	for _, child := range directChildren(plot, "axId") {
		ids = append(ids, strings.TrimSpace(child.SelectAttrValue("val", "")))
	}
	return ids
}

// setBarDir sets c:barDir on a c:barChart to col (column) or bar.
func setBarDir(b styleBuilder, plot *etree.Element, t ChartType) {
	dir := "col"
	if t == ChartTypeBar {
		dir = "bar"
	}
	if existing := firstDirectChild(plot, "barDir"); existing != nil {
		existing.CreateAttr("val", dir)
		return
	}
	plot.InsertChildAt(0, b.cVal("barDir", dir))
}

// checkConvertible enforces the compatibility matrix. The only conversions the
// command rejects outright are those out of pie (no cat/val structure remains
// usable) and into an identical type (handled by the caller).
func checkConvertible(from, to ChartType) error {
	if from == ChartTypePie {
		return fmt.Errorf("cannot convert from pie to %s: pie charts have no category/value axis structure to carry over; recreate the chart with `charts create --type %s`", to, to)
	}
	return nil
}

// transformSeries rewrites one c:ser for the target type, returning warnings.
// cat<->xVal and val<->yVal are clean renames (the underlying CT_*DataSource
// content is identical); markers are dropped on types that cannot carry them.
func transformSeries(b styleBuilder, ser *etree.Element, from, to ChartType, number int) []string {
	var warnings []string
	toScatter := to == ChartTypeScatter
	fromScatter := from == ChartTypeScatter

	if toScatter && !fromScatter {
		renameChild(ser, "cat", "xVal")
		renameChild(ser, "val", "yVal")
		if xVal := firstDirectChild(ser, "xVal"); xVal != nil {
			if firstDirectChild(xVal, "strRef") != nil || firstDirectChild(xVal, "multiLvlStrRef") != nil {
				warnings = append(warnings, fmt.Sprintf("series %d x-values are a text reference; scatter charts expect numeric x-values, so the chart may misrender until the source is re-pointed at numeric data", number))
			}
		}
	} else if fromScatter && !toScatter {
		renameChild(ser, "xVal", "cat")
		renameChild(ser, "yVal", "val")
	}

	// Markers only exist on line/scatter series; drop them elsewhere.
	if !markerChartTypes[elementForChartType(to)] {
		if marker := firstDirectChild(ser, "marker"); marker != nil {
			ser.RemoveChild(marker)
			warnings = append(warnings, fmt.Sprintf("series %d had a marker style; %s charts do not support markers, so it was removed", number, to))
		}
	}

	reorderSeriesChildren(b, ser)
	return warnings
}

// renameChild renames a single direct child element (keeping its namespace and
// content) from oldLocal to newLocal.
func renameChild(parent *etree.Element, oldLocal, newLocal string) {
	child := firstDirectChild(parent, oldLocal)
	if child == nil {
		return
	}
	child.Tag = newLocal
}

// reorderSeriesChildren re-inserts a series' children in schema (seriesChildOrder)
// order so renamed cat/val/xVal/yVal land at valid positions.
func reorderSeriesChildren(_ styleBuilder, ser *etree.Element) {
	children := ser.ChildElements()
	for _, child := range children {
		ser.RemoveChild(child)
	}
	for _, child := range children {
		insertInOrder(ser, child, seriesChildOrder)
	}
}

// buildPlotWrapper builds a new plot element for the target type, moving the
// already-transformed series into it and re-using the supplied axis IDs. The
// child order mirrors the chart producer so output strict-validates.
func buildPlotWrapper(b styleBuilder, t ChartType, series []*etree.Element, axIDs []string) *etree.Element {
	plot := b.c(elementForChartType(t))
	switch t {
	case ChartTypeBar, ChartTypeColumn:
		dir := "col"
		if t == ChartTypeBar {
			dir = "bar"
		}
		plot.AddChild(b.cVal("barDir", dir))
		plot.AddChild(b.cVal("grouping", "clustered"))
		plot.AddChild(b.cVal("varyColors", "0"))
		appendSeries(plot, series)
		appendAxisIDs(b, plot, axIDs, 2)
	case ChartTypeLine:
		plot.AddChild(b.cVal("grouping", "standard"))
		plot.AddChild(b.cVal("varyColors", "0"))
		appendSeries(plot, series)
		plot.AddChild(b.cVal("marker", "1"))
		appendAxisIDs(b, plot, axIDs, 2)
	case ChartTypeArea:
		plot.AddChild(b.cVal("grouping", "standard"))
		plot.AddChild(b.cVal("varyColors", "0"))
		appendSeries(plot, series)
		appendAxisIDs(b, plot, axIDs, 2)
	case ChartTypePie:
		plot.AddChild(b.cVal("varyColors", "1"))
		appendSeries(plot, series)
		plot.AddChild(b.cVal("firstSliceAng", "0"))
	case ChartTypeScatter:
		plot.AddChild(b.cVal("scatterStyle", "lineMarker"))
		plot.AddChild(b.cVal("varyColors", "0"))
		appendSeries(plot, series)
		appendAxisIDs(b, plot, axIDs, 2)
	}
	return plot
}

func appendSeries(plot *etree.Element, series []*etree.Element) {
	for _, ser := range series {
		if ser.Parent() != nil {
			ser.Parent().RemoveChild(ser)
		}
		plot.AddChild(ser)
	}
}

// appendAxisIDs appends up to want c:axId children, re-using existing IDs and
// synthesizing stable fallbacks when the source plot lacked enough.
func appendAxisIDs(b styleBuilder, plot *etree.Element, axIDs []string, want int) {
	fallback := []string{"111111111", "222222222"}
	for i := 0; i < want; i++ {
		id := ""
		if i < len(axIDs) && strings.TrimSpace(axIDs[i]) != "" {
			id = axIDs[i]
		} else if i < len(fallback) {
			id = fallback[i]
		}
		plot.AddChild(b.cVal("axId", id))
	}
}

// transformAxes adjusts the plotArea axes for the target type and returns a
// warning when an axis role changes.
func transformAxes(plotArea *etree.Element, from, to ChartType, axIDs []string) string {
	if to == ChartTypePie {
		for _, name := range []string{"catAx", "valAx", "dateAx", "serAx"} {
			for _, ax := range directChildren(plotArea, name) {
				plotArea.RemoveChild(ax)
			}
		}
		return ""
	}

	categoryAxisID := ""
	if len(axIDs) > 0 {
		categoryAxisID = axIDs[0]
	}

	if to == ChartTypeScatter && from != ChartTypeScatter {
		if ax := axisByID(plotArea, categoryAxisID, "catAx"); ax != nil {
			renameAxisElement(ax, "valAx")
			return "category axis converted to a value axis for the scatter chart; review its scale and number format"
		}
		return ""
	}
	if from == ChartTypeScatter && to != ChartTypeScatter {
		if ax := axisByID(plotArea, categoryAxisID, "valAx"); ax != nil {
			renameAxisElement(ax, "catAx")
			return "scatter x value axis converted to a category axis; review its labels and number format"
		}
		return ""
	}
	return ""
}

// axisByID returns the axis whose c:axId equals id; when id is empty it falls
// back to the first axis with the given element local name.
func axisByID(plotArea *etree.Element, id, elementFallback string) *etree.Element {
	if strings.TrimSpace(id) != "" {
		for _, child := range plotArea.ChildElements() {
			switch localName(child.Tag) {
			case "catAx", "valAx", "dateAx", "serAx":
				if axID := firstDirectChild(child, "axId"); axID != nil &&
					strings.TrimSpace(axID.SelectAttrValue("val", "")) == strings.TrimSpace(id) {
					return child
				}
			}
		}
	}
	return firstDirectChild(plotArea, elementFallback)
}

// renameAxisElement renames an axis element (catAx<->valAx) in place and prunes
// children that are not valid on the target axis type. catAx and valAx share the
// CT_AxBase prefix, but each has a type-specific tail (e.g. catAx carries
// c:auto/c:lblAlgn/c:lblOffset/c:noMultiLvlLbl, valAx carries
// c:crossBetween/c:majorUnit/c:minorUnit/c:dispUnits); leaving the wrong tail in
// place produces XML that strict validators accept but Office rejects.
func renameAxisElement(ax *etree.Element, newLocal string) {
	ax.Tag = newLocal
	allowed := make(map[string]bool, len(catAxisChildOrder))
	for _, name := range axisChildOrder(newLocal) {
		allowed[name] = true
	}
	for _, child := range ax.ChildElements() {
		if !allowed[localName(child.Tag)] {
			ax.RemoveChild(child)
		}
	}
}
