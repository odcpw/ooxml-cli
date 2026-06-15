package chart

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// Schema child orders for the containers that carry a c:spPr shape-properties
// element. Both place c:spPr late in the sequence; the only child guaranteed to
// follow it is c:extLst, so insertInOrder with these (extLst-only) orders inserts
// spPr before any extension list and otherwise appends, which is schema-valid.
var (
	// plotAreaChildOrder: CT_PlotArea. The chart-type children are open-ended, so
	// only the tail (spPr before extLst) is enumerated for positioning.
	plotAreaChildOrder = []string{"spPr", "extLst"}
	// chartSpaceChildOrder: CT_ChartSpace. c:spPr follows c:chart and precedes
	// c:txPr/c:externalData/c:printSettings/c:userShapes/c:extLst.
	chartSpaceChildOrder = []string{"spPr", "txPr", "externalData", "printSettings", "userShapes", "extLst"}
)

// SetFillRequest sets (or clears) the solid fill of the plot area or the chart
// area (chart-space background). It is shared by the XLSX and PPTX commands.
type SetFillRequest struct {
	Package  opc.PackageSession
	ChartURI string
	// FillColor is a hex RRGGBB color (with or without leading #). When empty and
	// NoFill is false the request is rejected.
	FillColor string
	// NoFill clears any fill by writing an explicit a:noFill.
	NoFill bool
	// ExpectFill guards the current fill: a hex RRGGBB color, "scheme:<name>", or
	// "none" (no solid fill). Nil skips the guard.
	ExpectFill *string
}

// SetFillResult reports the previous fill and the post-mutation chart readback.
type SetFillResult struct {
	PreviousFill string // hex, "scheme:<name>", or "" when no solid fill
	NewFill      string // hex, "" when cleared
	Style        *ChartStyle
}

// SetPlotAreaFill sets or clears the solid fill of the chart's plot area.
func SetPlotAreaFill(req *SetFillRequest) (*SetFillResult, error) {
	return setAreaFill(req, "plotArea")
}

// SetChartAreaFill sets or clears the solid fill of the chart-space background.
func SetChartAreaFill(req *SetFillRequest) (*SetFillResult, error) {
	return setAreaFill(req, "chartSpace")
}

// setAreaFill applies a solid fill (or noFill) to the c:spPr of the plot area
// (area == "plotArea") or chart-space root (area == "chartSpace").
func setAreaFill(req *SetFillRequest, area string) (*SetFillResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set chart fill request is nil")
	}
	if !req.NoFill && strings.TrimSpace(req.FillColor) == "" {
		return nil, fmt.Errorf("a fill color is required (or set --fill-color none to clear)")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	b := newStyleBuilder(root)

	var holder *etree.Element
	var order []string
	switch area {
	case "plotArea":
		holder = firstDescendant(root, "plotArea")
		if holder == nil {
			return nil, fmt.Errorf("chart part %s has no plotArea", req.ChartURI)
		}
		order = plotAreaChildOrder
	default:
		holder = root
		order = chartSpaceChildOrder
	}

	spPr := firstDirectChild(holder, "spPr")
	previous := inspectFill(spPr)
	if req.ExpectFill != nil && !fillMatches(previous, *req.ExpectFill) {
		want := strings.TrimSpace(*req.ExpectFill)
		have := previous
		if have == "" {
			have = "none"
		}
		return nil, fmt.Errorf("fill mismatch: expected %q but found %q", want, have)
	}

	if spPr == nil {
		spPr = b.c("spPr")
		insertInOrder(holder, spPr, order)
	}

	newFill := ""
	if req.NoFill {
		for _, name := range fillGroupNames {
			for _, child := range directChildren(spPr, name) {
				spPr.RemoveChild(child)
			}
		}
		insertInOrder(spPr, b.a("noFill"), shapePropsChildOrder)
	} else {
		setSolidFill(b, spPr, req.FillColor, shapePropsChildOrder)
		newFill = normalizeHexColorLoose(req.FillColor)
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	style, _ := InspectStyle(req.Package, req.ChartURI)
	return &SetFillResult{PreviousFill: previous, NewFill: newFill, Style: style}, nil
}

// fillGroupNames are the mutually-exclusive fill children of a shape-properties
// element. setSolidFill removes all of these before inserting a solid fill.
var fillGroupNames = []string{"noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill"}

// fillMatches reports whether the current fill equals the guard value. The guard
// accepts "none"/"" for an absent solid fill, a "scheme:<name>" reference, or a
// hex RRGGBB color compared case-insensitively.
func fillMatches(current, expect string) bool {
	want := strings.TrimSpace(expect)
	if strings.EqualFold(want, "none") || want == "" {
		return current == ""
	}
	return strings.EqualFold(current, want)
}

// ApplyStyleRequest copies a practical style subset from a template chart's
// ChartStyle onto a target chart. It copies STYLE (fonts, fills, colors, legend
// position, gridlines, number formats, marker/line defaults), never CONTENT
// (title text, axis title text, series names/data).
type ApplyStyleRequest struct {
	Package  opc.PackageSession
	ChartURI string
	// Source is the template chart's style, typically obtained via InspectStyle
	// on a different (read-only) package.
	Source *ChartStyle
	// ExpectSeriesCount guards the target's series count when set.
	ExpectSeriesCount *int
}

// ApplyStyleResult reports the applied fields and the post-mutation readback.
type ApplyStyleResult struct {
	Applied []string // human-readable list of applied style facets
	Style   *ChartStyle
}

// ApplyStyle applies the practical style fields of a template ChartStyle onto a
// target chart in one pass. Decorative or rare effects are skipped.
func ApplyStyle(req *ApplyStyleRequest) (*ApplyStyleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("apply chart style request is nil")
	}
	if req.Source == nil {
		return nil, fmt.Errorf("template style is nil")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	src := req.Source
	b := newStyleBuilder(root)
	chartElem := firstDirectChild(root, "chart")
	if chartElem == nil {
		return nil, fmt.Errorf("chart part %s has no chart element", req.ChartURI)
	}
	plotArea := firstDescendant(root, "plotArea")

	series := walkSeries(root)
	if req.ExpectSeriesCount != nil && *req.ExpectSeriesCount != len(series) {
		return nil, fmt.Errorf("series count mismatch: expected %d but found %d", *req.ExpectSeriesCount, len(series))
	}

	var applied []string

	// Title font (only when the target already has a title; copy STYLE not TEXT).
	if src.Title.Font != nil {
		if title := firstDirectChild(chartElem, "title"); title != nil {
			if run := firstDescendant(firstDescendant(title, "rich"), "r"); run != nil {
				applyRunFont(b, run, fontFieldsFromFont(src.Title.Font))
				applied = append(applied, "title-font")
			}
		}
	}

	// Legend position + overlay.
	if src.Legend.Present && src.Legend.Position != "" {
		legend := firstDirectChild(chartElem, "legend")
		if legend == nil {
			legend = b.c("legend")
			insertInOrder(chartElem, legend, chartChildOrder)
		}
		setOrCreateValChild(b, legend, "legendPos", src.Legend.Position, legendChildOrder)
		if src.Legend.Overlay != nil {
			setOrCreateValChild(b, legend, "overlay", boolAttr(*src.Legend.Overlay), legendChildOrder)
		}
		applied = append(applied, "legend")
	}

	// Axis fonts, gridlines, and number formats, matched by axis kind/element.
	if plotArea != nil {
		for _, srcAxis := range src.Axes {
			target := findTargetAxis(plotArea, srcAxis)
			if target == nil {
				continue
			}
			order := axisChildOrder(localName(target.Tag))
			changed := false
			if srcAxis.TitleFont != nil {
				if title := firstDirectChild(target, "title"); title != nil {
					if run := firstDescendant(firstDescendant(title, "rich"), "r"); run != nil {
						applyRunFont(b, run, fontFieldsFromFont(srcAxis.TitleFont))
						changed = true
					}
				}
			}
			if srcAxis.TickLabelFont != nil {
				applyAxisTickLabelFont(b, target, fontFieldsFromFont(srcAxis.TickLabelFont), order)
				changed = true
			}
			if srcAxis.NumberFormat != "" {
				numFmt := firstDirectChild(target, "numFmt")
				if numFmt == nil {
					numFmt = b.c("numFmt")
					insertInOrder(target, numFmt, order)
				}
				numFmt.CreateAttr("formatCode", srcAxis.NumberFormat)
				numFmt.CreateAttr("sourceLinked", "0")
				changed = true
			}
			applyGridlines(b, target, "majorGridlines", srcAxis.MajorGridlines, order)
			applyGridlines(b, target, "minorGridlines", srcAxis.MinorGridlines, order)
			if changed {
				applied = append(applied, "axis:"+srcAxis.Element)
			} else {
				applied = append(applied, "axis-gridlines:"+srcAxis.Element)
			}
		}
	}

	// Series fill/line/marker defaults, matched by series number.
	for i, srcSeries := range src.Series {
		if i >= len(series) {
			break
		}
		ser := series[i]
		applySeriesStyleFromSource(b, ser, srcSeries)
		applied = append(applied, fmt.Sprintf("series:%d", i+1))
	}

	// Plot-area fill.
	if src.PlotAreaFill != "" && plotArea != nil {
		spPr := firstDirectChild(plotArea, "spPr")
		if spPr == nil {
			spPr = b.c("spPr")
			insertInOrder(plotArea, spPr, plotAreaChildOrder)
		}
		applyFillFromSource(b, spPr, src.PlotAreaFill)
		applied = append(applied, "plot-area-fill")
	}

	// Chart-area (background) fill.
	if src.ChartSpaceFill != "" {
		spPr := firstDirectChild(root, "spPr")
		if spPr == nil {
			spPr = b.c("spPr")
			insertInOrder(root, spPr, chartSpaceChildOrder)
		}
		applyFillFromSource(b, spPr, src.ChartSpaceFill)
		applied = append(applied, "chart-area-fill")
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	style, _ := InspectStyle(req.Package, req.ChartURI)
	return &ApplyStyleResult{Applied: applied, Style: style}, nil
}

// fontFieldsFromFont converts an inspected FontStyle into the writable
// fontFields used by the run/paragraph font setters.
func fontFieldsFromFont(f *FontStyle) fontFields {
	if f == nil {
		return fontFields{}
	}
	ff := fontFields{family: f.Family, color: f.Color, bold: f.Bold, italic: f.Italic}
	if f.SizePt > 0 {
		v := f.SizePt
		ff.sizePt = &v
	}
	return ff
}

// findTargetAxis locates the target axis matching a source axis, preferring an
// exact axId match and falling back to the first axis of the same element name.
func findTargetAxis(plotArea *etree.Element, src AxisStyle) *etree.Element {
	var fallback *etree.Element
	for _, child := range plotArea.ChildElements() {
		if localName(child.Tag) != src.Element {
			continue
		}
		if fallback == nil {
			fallback = child
		}
		if src.AxisID != "" {
			if id := firstDirectChild(child, "axId"); id != nil {
				if strings.TrimSpace(id.SelectAttrValue("val", "")) == src.AxisID {
					return child
				}
			}
		}
	}
	return fallback
}

// applySeriesStyleFromSource copies fill/line/marker defaults from a source
// series style onto a target series element. Only solid fills and basic line and
// marker properties are copied; data and names are left untouched.
func applySeriesStyleFromSource(b styleBuilder, ser *etree.Element, src SeriesStyle) {
	if src.FillColor != "" || src.LineColor != "" || src.LineWidthPt != nil {
		spPr := firstDirectChild(ser, "spPr")
		if spPr == nil {
			spPr = b.c("spPr")
			insertInOrder(ser, spPr, seriesChildOrder)
		}
		if src.FillColor != "" {
			applyColorFill(b, spPr, src.FillColor, shapePropsChildOrder)
		}
		if src.LineColor != "" || src.LineWidthPt != nil {
			ln := firstDirectChild(spPr, "ln")
			if ln == nil {
				ln = b.a("ln")
				insertInOrder(spPr, ln, shapePropsChildOrder)
			}
			if src.LineWidthPt != nil {
				ln.CreateAttr("w", fmt.Sprintf("%d", emuFromPoints(*src.LineWidthPt)))
			}
			if src.LineColor != "" {
				applyColorFill(b, ln, src.LineColor, lineChildOrder)
			}
		}
	}
	if src.Marker != nil && markerChartTypes[seriesChartType(ser)] {
		marker := firstDirectChild(ser, "marker")
		if marker == nil {
			marker = b.c("marker")
			insertInOrder(ser, marker, seriesChildOrder)
		}
		if src.Marker.Symbol != "" {
			setOrCreateValChild(b, marker, "symbol", src.Marker.Symbol, markerChildOrder)
		}
		if src.Marker.Size > 0 {
			setOrCreateValChild(b, marker, "size", fmt.Sprintf("%d", src.Marker.Size), markerChildOrder)
		}
	}
}

// applyFillFromSource writes a solid fill from an inspected fill value (hex or
// "scheme:<name>") onto a shape-properties holder. Scheme references are written
// as a:schemeClr; hex colors as a:srgbClr.
func applyFillFromSource(b styleBuilder, spPr *etree.Element, fill string) {
	applyColorFill(b, spPr, fill, shapePropsChildOrder)
}

// applyColorFill writes a solid fill on holder, placed per order, honoring both
// hex (srgbClr via setSolidFill) and theme ("scheme:NAME") colors. It exists so
// series fill/line copy theme colors instead of silently dropping them.
func applyColorFill(b styleBuilder, holder *etree.Element, color string, order []string) {
	if strings.HasPrefix(color, "scheme:") {
		for _, name := range fillGroupNames {
			for _, child := range directChildren(holder, name) {
				holder.RemoveChild(child)
			}
		}
		solid := b.a("solidFill")
		clr := b.a("schemeClr")
		clr.CreateAttr("val", strings.TrimPrefix(color, "scheme:"))
		solid.AddChild(clr)
		insertInOrder(holder, solid, order)
		return
	}
	setSolidFill(b, holder, color, order)
}
