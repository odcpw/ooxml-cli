package chart

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// emuPerPoint converts DrawingML line widths (stored in EMU) to/from points.
const emuPerPoint = 12700

// FontStyle captures the practical run-level font fields of chart text.
type FontStyle struct {
	Family string  `json:"family,omitempty"`
	SizePt float64 `json:"sizePt,omitempty"`
	Bold   *bool   `json:"bold,omitempty"`
	Italic *bool   `json:"italic,omitempty"`
	Color  string  `json:"color,omitempty"` // hex RRGGBB or scheme:<name>
}

// TitleStyle reports the chart title text and basic font.
type TitleStyle struct {
	Present bool       `json:"present"`
	Linked  bool       `json:"linked,omitempty"` // title text comes from a cell reference
	Text    string     `json:"text,omitempty"`
	Overlay *bool      `json:"overlay,omitempty"`
	Font    *FontStyle `json:"font,omitempty"`
}

// LegendStyle reports legend visibility, position, and overlay.
type LegendStyle struct {
	Present  bool   `json:"present"`
	Position string `json:"position,omitempty"` // r, l, t, b, tr
	Overlay  *bool  `json:"overlay,omitempty"`
}

// AxisStyle reports practical fields of a category/value/date axis.
type AxisStyle struct {
	Element        string     `json:"element"` // catAx, valAx, dateAx, serAx
	Kind           string     `json:"kind"`    // category or value
	AxisID         string     `json:"axisId,omitempty"`
	Hidden         *bool      `json:"hidden,omitempty"`
	Title          string     `json:"title,omitempty"`
	TitleFont      *FontStyle `json:"titleFont,omitempty"`
	NumberFormat   string     `json:"numberFormat,omitempty"`
	Min            *float64   `json:"min,omitempty"`
	Max            *float64   `json:"max,omitempty"`
	MajorUnit      *float64   `json:"majorUnit,omitempty"`
	MajorGridlines bool       `json:"majorGridlines"`
	MinorGridlines bool       `json:"minorGridlines"`
	TickLabelFont  *FontStyle `json:"tickLabelFont,omitempty"`
}

// MarkerStyle reports series marker basics.
type MarkerStyle struct {
	Symbol string `json:"symbol,omitempty"` // circle, square, none, ...
	Size   int    `json:"size,omitempty"`   // marker size 2..72 (not EMU)
}

// SeriesStyle reports practical visual style of one chart series.
type SeriesStyle struct {
	Number      int          `json:"number"`
	Name        string       `json:"name,omitempty"`
	FillColor   string       `json:"fillColor,omitempty"`
	NoFill      bool         `json:"noFill,omitempty"`
	LineColor   string       `json:"lineColor,omitempty"`
	LineWidthPt *float64     `json:"lineWidthPt,omitempty"`
	NoLine      bool         `json:"noLine,omitempty"`
	Marker      *MarkerStyle `json:"marker,omitempty"`
}

// ChartStyle is a practical, family-neutral snapshot of a DrawingML chart's
// current styling. It is produced by InspectStyle and read back by the chart
// style mutation helpers.
type ChartStyle struct {
	PartURI        string        `json:"partUri,omitempty"`
	Types          []string      `json:"types,omitempty"`
	Title          TitleStyle    `json:"title"`
	Legend         LegendStyle   `json:"legend"`
	Axes           []AxisStyle   `json:"axes,omitempty"`
	Series         []SeriesStyle `json:"series,omitempty"`
	PlotAreaFill   string        `json:"plotAreaFill,omitempty"`
	ChartSpaceFill string        `json:"chartSpaceFill,omitempty"`
}

// InspectStyle reads a DrawingML chart part and returns its practical style. The
// chart XML is shared by XLSX and PPTX chart parts, so both families reuse this.
func InspectStyle(session opc.PackageSession, chartURI string) (*ChartStyle, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if strings.TrimSpace(chartURI) == "" {
		return nil, fmt.Errorf("chart URI is required")
	}
	doc, err := session.ReadXMLPart(chartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read chart part %s: %w", chartURI, err)
	}
	root := doc.Root()
	if root == nil || !isLocal(root, "chartSpace") {
		return nil, fmt.Errorf("chart part %s root element not found", chartURI)
	}

	style := &ChartStyle{PartURI: chartURI, Types: chartTypes(root)}
	chartElem := firstDirectChild(root, "chart")
	if chartElem != nil {
		style.Title = inspectTitle(firstDirectChild(chartElem, "title"))
		style.Legend = inspectLegend(firstDirectChild(chartElem, "legend"))
	}
	plotArea := firstDescendant(root, "plotArea")
	if plotArea != nil {
		style.Axes = inspectAxes(plotArea)
		style.PlotAreaFill = inspectFill(firstDirectChild(plotArea, "spPr"))
	}
	style.ChartSpaceFill = inspectFill(firstDirectChild(root, "spPr"))
	style.Series = inspectSeries(root)
	return style, nil
}

func inspectTitle(title *etree.Element) TitleStyle {
	if title == nil {
		return TitleStyle{Present: false}
	}
	result := TitleStyle{Present: true}
	if overlay := firstDirectChild(title, "overlay"); overlay != nil {
		result.Overlay = boolPtr(parseOOXMLBool(overlay.SelectAttrValue("val", "")))
	}
	tx := firstDirectChild(title, "tx")
	if tx != nil {
		if firstDirectChild(tx, "strRef") != nil {
			result.Linked = true
		}
	}
	result.Text = titleText(title)
	if font := inspectTitleFont(title); font != nil {
		result.Font = font
	}
	return result
}

// titleText extracts the visible text of a CT_Title (rich text runs, or the
// cached values of a cell-linked title).
func titleText(title *etree.Element) string {
	if title == nil {
		return ""
	}
	var parts []string
	for _, text := range descendants(title, "t") {
		if text.NamespaceURI() == nsA {
			parts = append(parts, text.Text())
		}
	}
	if len(parts) == 0 {
		for _, value := range descendants(title, "v") {
			parts = append(parts, value.Text())
		}
	}
	return strings.TrimSpace(strings.Join(parts, ""))
}

// inspectTitleFont reads the title font from the first rich run, then falls back
// to the rich paragraph default, then the title-wide txPr default.
func inspectTitleFont(title *etree.Element) *FontStyle {
	candidates := []*etree.Element{}
	if rich := firstDescendant(title, "rich"); rich != nil {
		if run := firstDescendant(rich, "r"); run != nil {
			candidates = append(candidates, firstDirectChild(run, "rPr"))
		}
		if pPr := firstDescendant(rich, "pPr"); pPr != nil {
			candidates = append(candidates, firstDirectChild(pPr, "defRPr"))
		}
	}
	if txPr := firstDirectChild(title, "txPr"); txPr != nil {
		if pPr := firstDescendant(txPr, "pPr"); pPr != nil {
			candidates = append(candidates, firstDirectChild(pPr, "defRPr"))
		}
	}
	for _, candidate := range candidates {
		if font := inspectFont(candidate); font != nil {
			return font
		}
	}
	return nil
}

// inspectAxisTickLabelFont reads the tick-label font from an axis-wide c:txPr,
// preferring the paragraph default (a:defRPr) and falling back to the first run.
func inspectAxisTickLabelFont(axis *etree.Element) *FontStyle {
	txPr := firstDirectChild(axis, "txPr")
	if txPr == nil {
		return nil
	}
	candidates := []*etree.Element{}
	if pPr := firstDescendant(txPr, "pPr"); pPr != nil {
		candidates = append(candidates, firstDirectChild(pPr, "defRPr"))
	}
	if run := firstDescendant(txPr, "r"); run != nil {
		candidates = append(candidates, firstDirectChild(run, "rPr"))
	}
	for _, candidate := range candidates {
		if font := inspectFont(candidate); font != nil {
			return font
		}
	}
	return nil
}

// inspectFont reads font fields from an a:rPr or a:defRPr element.
func inspectFont(rPr *etree.Element) *FontStyle {
	if rPr == nil {
		return nil
	}
	font := &FontStyle{}
	found := false
	if sz := strings.TrimSpace(rPr.SelectAttrValue("sz", "")); sz != "" {
		if hundredths, err := strconv.Atoi(sz); err == nil {
			font.SizePt = float64(hundredths) / 100
			found = true
		}
	}
	if b := strings.TrimSpace(rPr.SelectAttrValue("b", "")); b != "" {
		font.Bold = boolPtr(parseOOXMLBool(b))
		found = true
	}
	if i := strings.TrimSpace(rPr.SelectAttrValue("i", "")); i != "" {
		font.Italic = boolPtr(parseOOXMLBool(i))
		found = true
	}
	if latin := firstDirectChild(rPr, "latin"); latin != nil {
		if typeface := strings.TrimSpace(latin.SelectAttrValue("typeface", "")); typeface != "" {
			font.Family = typeface
			found = true
		}
	}
	if color := inspectFill(rPr); color != "" {
		font.Color = color
		found = true
	}
	if !found {
		return nil
	}
	return font
}

func inspectLegend(legend *etree.Element) LegendStyle {
	if legend == nil {
		return LegendStyle{Present: false}
	}
	result := LegendStyle{Present: true}
	if pos := firstDirectChild(legend, "legendPos"); pos != nil {
		result.Position = strings.TrimSpace(pos.SelectAttrValue("val", ""))
	}
	if overlay := firstDirectChild(legend, "overlay"); overlay != nil {
		result.Overlay = boolPtr(parseOOXMLBool(overlay.SelectAttrValue("val", "")))
	}
	return result
}

func inspectAxes(plotArea *etree.Element) []AxisStyle {
	var axes []AxisStyle
	for _, child := range plotArea.ChildElements() {
		name := localName(child.Tag)
		switch name {
		case "catAx", "valAx", "dateAx", "serAx":
		default:
			continue
		}
		axis := AxisStyle{Element: name, Kind: axisKind(name)}
		if id := firstDirectChild(child, "axId"); id != nil {
			axis.AxisID = strings.TrimSpace(id.SelectAttrValue("val", ""))
		}
		if del := firstDirectChild(child, "delete"); del != nil {
			axis.Hidden = boolPtr(parseOOXMLBool(del.SelectAttrValue("val", "")))
		}
		if titleElem := firstDirectChild(child, "title"); titleElem != nil {
			axis.Title = titleText(titleElem)
			axis.TitleFont = inspectTitleFont(titleElem)
		}
		if numFmt := firstDirectChild(child, "numFmt"); numFmt != nil {
			axis.NumberFormat = strings.TrimSpace(numFmt.SelectAttrValue("formatCode", ""))
		}
		if scaling := firstDirectChild(child, "scaling"); scaling != nil {
			axis.Min = parseFloatAttr(firstDirectChild(scaling, "min"), "val")
			axis.Max = parseFloatAttr(firstDirectChild(scaling, "max"), "val")
		}
		axis.MajorUnit = parseFloatAttr(firstDirectChild(child, "majorUnit"), "val")
		axis.MajorGridlines = firstDirectChild(child, "majorGridlines") != nil
		axis.MinorGridlines = firstDirectChild(child, "minorGridlines") != nil
		axis.TickLabelFont = inspectAxisTickLabelFont(child)
		axes = append(axes, axis)
	}
	return axes
}

func inspectSeries(root *etree.Element) []SeriesStyle {
	var result []SeriesStyle
	for i, ser := range walkSeries(root) {
		result = append(result, inspectSeriesStyle(ser, i+1))
	}
	return result
}

// inspectSeriesStyle reads the practical visual style of one series element. It
// is shared by InspectStyle readback and by SetSeriesStyle's after-mutation
// readback so the round-trip stays self-consistent.
func inspectSeriesStyle(ser *etree.Element, number int) SeriesStyle {
	style := SeriesStyle{Number: number}
	if tx := firstDirectChild(ser, "tx"); tx != nil {
		style.Name = seriesNameText(tx)
	}
	if spPr := firstDirectChild(ser, "spPr"); spPr != nil {
		fill := inspectFill(spPr)
		if firstDirectChild(spPr, "noFill") != nil {
			style.NoFill = true
		} else if fill != "" {
			style.FillColor = fill
		}
		if ln := firstDirectChild(spPr, "ln"); ln != nil {
			if firstDirectChild(ln, "noFill") != nil {
				style.NoLine = true
			} else if color := inspectFill(ln); color != "" {
				style.LineColor = color
			}
			if w := strings.TrimSpace(ln.SelectAttrValue("w", "")); w != "" {
				if emu, err := strconv.Atoi(w); err == nil {
					pt := float64(emu) / emuPerPoint
					style.LineWidthPt = &pt
				}
			}
		}
	}
	if marker := firstDirectChild(ser, "marker"); marker != nil {
		style.Marker = inspectMarker(marker)
	}
	return style
}

func inspectMarker(marker *etree.Element) *MarkerStyle {
	result := &MarkerStyle{}
	found := false
	if symbol := firstDirectChild(marker, "symbol"); symbol != nil {
		result.Symbol = strings.TrimSpace(symbol.SelectAttrValue("val", ""))
		found = true
	}
	if size := firstDirectChild(marker, "size"); size != nil {
		if n, err := strconv.Atoi(strings.TrimSpace(size.SelectAttrValue("val", ""))); err == nil {
			result.Size = n
			found = true
		}
	}
	if !found {
		return nil
	}
	return result
}

// inspectFill returns the solid fill color of a shape-properties-like element as
// a hex string (srgbClr) or "scheme:<name>" (schemeClr); "" when no solid fill.
func inspectFill(holder *etree.Element) string {
	if holder == nil {
		return ""
	}
	solid := firstDirectChild(holder, "solidFill")
	if solid == nil {
		return ""
	}
	if srgb := firstDirectChild(solid, "srgbClr"); srgb != nil {
		return strings.ToUpper(strings.TrimSpace(srgb.SelectAttrValue("val", "")))
	}
	if scheme := firstDirectChild(solid, "schemeClr"); scheme != nil {
		return "scheme:" + strings.TrimSpace(scheme.SelectAttrValue("val", ""))
	}
	return ""
}

func seriesNameText(tx *etree.Element) string {
	var parts []string
	for _, value := range descendants(tx, "v") {
		parts = append(parts, value.Text())
	}
	if len(parts) == 0 {
		for _, text := range descendants(tx, "t") {
			parts = append(parts, text.Text())
		}
	}
	return strings.TrimSpace(strings.Join(parts, ""))
}

func axisKind(element string) string {
	switch element {
	case "valAx":
		return "value"
	case "dateAx":
		return "date"
	case "serAx":
		return "series"
	default:
		return "category"
	}
}

func parseOOXMLBool(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "1", "true", "on":
		return true
	default:
		return false
	}
}

func parseFloatAttr(elem *etree.Element, attr string) *float64 {
	if elem == nil {
		return nil
	}
	value := strings.TrimSpace(elem.SelectAttrValue(attr, ""))
	if value == "" {
		return nil
	}
	parsed, err := strconv.ParseFloat(value, 64)
	if err != nil {
		return nil
	}
	return &parsed
}

func boolPtr(value bool) *bool { return &value }

// ---- chart style mutation (shared by XLSX and PPTX chart parts) ----

// Canonical child orderings from the DrawingML chart schema (ECMA-376). The
// setters insert and update children at schema-correct positions so the output
// is valid for Office, not merely readable by InspectStyle.
var (
	chartChildOrder      = []string{"title", "autoTitleDeleted", "pivotFmts", "view3D", "floor", "sideWall", "backWall", "plotArea", "legend", "plotVisOnly", "dispBlanksAs", "showDLblsOverMax", "extLst"}
	titleChildOrder      = []string{"tx", "layout", "overlay", "spPr", "txPr", "extLst"}
	legendChildOrder     = []string{"legendPos", "legendEntry", "layout", "overlay", "spPr", "txPr", "extLst"}
	seriesChildOrder     = []string{"idx", "order", "tx", "spPr", "invertIfNegative", "pictureOptions", "explosion", "marker", "dPt", "dLbls", "trendline", "errBars", "cat", "cPt", "val", "xVal", "yVal", "bubbleSize", "bubble3D", "shape", "smooth", "extLst"}
	shapePropsChildOrder = []string{"xfrm", "custGeom", "prstGeom", "noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill", "ln", "effectLst", "effectDag", "scene3d", "sp3d", "extLst"}
	lineChildOrder       = []string{"noFill", "solidFill", "gradFill", "pattFill", "prstDash", "custDash", "round", "bevel", "miter", "headEnd", "tailEnd", "extLst"}
	markerChildOrder     = []string{"symbol", "size", "spPr", "extLst"}
	paragraphChildOrder  = []string{"pPr", "r", "br", "fld", "endParaRPr"}
	runChildOrder        = []string{"rPr", "t"}
	rPrChildOrder        = []string{"ln", "noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill", "effectLst", "effectDag", "highlight", "uLnTx", "uLn", "uFillTx", "uFill", "latin", "ea", "cs", "sym", "hlinkClick", "hlinkMouseOver", "rtl", "extLst"}
	// scalingChildOrder: CT_Scaling. Note c:max precedes c:min in the schema.
	scalingChildOrder = []string{"logBase", "orientation", "max", "min", "extLst"}
	// catAxisChildOrder / valAxisChildOrder: shared CT_AxBase prefix plus the
	// type-specific tail. Used by insertInOrder so axis mutations stay valid.
	catAxisChildOrder = []string{"axId", "scaling", "delete", "axPos", "majorGridlines", "minorGridlines", "title", "numFmt", "majorTickMark", "minorTickMark", "tickLblPos", "spPr", "txPr", "crossAx", "crosses", "crossesAt", "auto", "lblAlgn", "lblOffset", "tickLblSkip", "tickMarkSkip", "noMultiLvlLbl", "extLst"}
	valAxisChildOrder = []string{"axId", "scaling", "delete", "axPos", "majorGridlines", "minorGridlines", "title", "numFmt", "majorTickMark", "minorTickMark", "tickLblPos", "spPr", "txPr", "crossAx", "crosses", "crossesAt", "crossBetween", "majorUnit", "minorUnit", "dispUnits", "extLst"}
	// axisTitleChildOrder: CT_Title (same as the chart title).
	axisTitleChildOrder = titleChildOrder
	txPrChildOrder      = []string{"bodyPr", "lstStyle", "p"}
)

// axisChildOrder returns the schema child order for the given axis element name.
func axisChildOrder(element string) []string {
	if element == "valAx" {
		return valAxisChildOrder
	}
	return catAxisChildOrder
}

// markerChartTypes are the chart-type elements whose series carry a CT_Marker.
var markerChartTypes = map[string]bool{"lineChart": true, "scatterChart": true, "radarChart": true}

// SetTitleRequest replaces the literal text (and optional font) of a chart title.
type SetTitleRequest struct {
	Package    opc.PackageSession
	ChartURI   string
	Text       string
	ExpectText *string  // guard: current title text must equal this when set
	FontFamily string   // a:latin typeface ("" leaves unchanged)
	FontSizePt *float64 // points
	FontColor  string   // hex RRGGBB (with or without leading #)
	FontBold   *bool
	FontItalic *bool
}

// SetTitleResult reports the previous text and the new title readback.
type SetTitleResult struct {
	PreviousText string
	Title        TitleStyle
}

// SetTitle sets the literal title text of a DrawingML chart, creating the title
// when absent. Cell-linked (strRef) titles are rejected rather than silently
// converted to literal text.
func SetTitle(req *SetTitleRequest) (*SetTitleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set chart title request is nil")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	b := newStyleBuilder(root)
	chartElem := firstDirectChild(root, "chart")
	if chartElem == nil {
		return nil, fmt.Errorf("chart part %s has no chart element", req.ChartURI)
	}

	title := firstDirectChild(chartElem, "title")
	previous := titleText(title)
	if req.ExpectText != nil && strings.TrimSpace(previous) != strings.TrimSpace(*req.ExpectText) {
		return nil, fmt.Errorf("chart title mismatch: expected %q but found %q", *req.ExpectText, previous)
	}
	if title == nil {
		title = b.c("title")
		insertInOrder(chartElem, title, chartChildOrder)
	}

	run, err := setTitleRichText(b, title, req.Text)
	if err != nil {
		return nil, err
	}
	applyRunFont(b, run, fontFields{
		family: req.FontFamily,
		sizePt: req.FontSizePt,
		color:  req.FontColor,
		bold:   req.FontBold,
		italic: req.FontItalic,
	})

	// An explicit title must not be suppressed by autoTitleDeleted.
	if atd := firstDirectChild(chartElem, "autoTitleDeleted"); atd != nil {
		atd.CreateAttr("val", "0")
	} else {
		insertInOrder(chartElem, b.cVal("autoTitleDeleted", "0"), chartChildOrder)
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	return &SetTitleResult{PreviousText: previous, Title: inspectTitle(firstDirectChild(chartElem, "title"))}, nil
}

// fontFields carries the optional run-level font overrides shared by the title
// and axis title/tick-label setters.
type fontFields struct {
	family string
	sizePt *float64
	color  string
	bold   *bool
	italic *bool
}

func (f fontFields) empty() bool {
	return f.family == "" && f.sizePt == nil && f.color == "" && f.bold == nil && f.italic == nil
}

// setTitleRichText creates or reuses the c:tx/c:rich/a:p/a:r/a:t chain of a
// CT_Title, rejecting cell-linked (strRef) titles, and returns the text run so
// the caller can apply font styling. Shared by the chart and axis title setters.
func setTitleRichText(b styleBuilder, title *etree.Element, text string) (*etree.Element, error) {
	tx := firstDirectChild(title, "tx")
	if tx != nil && firstDirectChild(tx, "strRef") != nil {
		return nil, fmt.Errorf("title is linked to a cell; setting literal title text is not supported")
	}
	if tx == nil {
		tx = b.c("tx")
		insertInOrder(title, tx, titleChildOrder)
	}
	rich := firstDirectChild(tx, "rich")
	if rich == nil {
		rich = b.c("rich")
		rich.AddChild(b.a("bodyPr"))
		rich.AddChild(b.a("lstStyle"))
		tx.AddChild(rich)
	}
	p := firstDirectChild(rich, "p")
	if p == nil {
		p = b.a("p")
		rich.AddChild(p)
	}
	run := firstDirectChild(p, "r")
	if run == nil {
		run = b.a("r")
		insertInOrder(p, run, paragraphChildOrder)
	}
	textElem := firstDirectChild(run, "t")
	if textElem == nil {
		textElem = b.a("t")
		insertInOrder(run, textElem, runChildOrder)
	}
	textElem.SetText(text)
	return run, nil
}

func applyRunFont(b styleBuilder, run *etree.Element, font fontFields) {
	if font.empty() {
		return
	}
	rPr := firstDirectChild(run, "rPr")
	if rPr == nil {
		rPr = b.a("rPr")
		insertInOrder(run, rPr, runChildOrder)
	}
	applyFontToRPr(b, rPr, font)
}

// applyFontToRPr writes the practical font fields (size, bold, italic, color,
// family) onto an a:rPr/a:defRPr element. The a:latin typeface is inserted at
// its schema position so the output stays Office-valid. Shared by the run-level
// (applyRunFont) and paragraph-default (applyAxisTickLabelFont) setters.
func applyFontToRPr(b styleBuilder, rPr *etree.Element, font fontFields) {
	if font.sizePt != nil {
		rPr.CreateAttr("sz", strconv.Itoa(hundredthsFromPoints(*font.sizePt)))
	}
	if font.bold != nil {
		rPr.CreateAttr("b", boolAttr(*font.bold))
	}
	if font.italic != nil {
		rPr.CreateAttr("i", boolAttr(*font.italic))
	}
	if font.color != "" {
		setSolidFill(b, rPr, font.color, rPrChildOrder)
	}
	if font.family != "" {
		latin := firstDirectChild(rPr, "latin")
		if latin == nil {
			latin = b.a("latin")
			insertInOrder(rPr, latin, rPrChildOrder)
		}
		latin.CreateAttr("typeface", font.family)
	}
}

// SetLegendRequest sets the legend position/overlay or removes the legend.
type SetLegendRequest struct {
	Package        opc.PackageSession
	ChartURI       string
	SetPosition    bool   // whether to apply Position
	Position       string // r, l, t, b, tr (ST_LegendPos)
	Remove         bool   // remove the legend entirely (--position none)
	Overlay        *bool
	ExpectPosition *string // guard: current legendPos must equal this when set
}

// SetLegendResult reports the previous position and the new legend readback.
type SetLegendResult struct {
	PreviousPosition string
	Removed          bool
	Legend           LegendStyle
}

// SetLegend sets legend position and/or overlay on a chart, creating the legend
// when absent, or removes the legend when Remove is set.
func SetLegend(req *SetLegendRequest) (*SetLegendResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set chart legend request is nil")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	b := newStyleBuilder(root)
	chartElem := firstDirectChild(root, "chart")
	if chartElem == nil {
		return nil, fmt.Errorf("chart part %s has no chart element", req.ChartURI)
	}

	legend := firstDirectChild(chartElem, "legend")
	previous := ""
	if legend != nil {
		if pos := firstDirectChild(legend, "legendPos"); pos != nil {
			previous = strings.TrimSpace(pos.SelectAttrValue("val", ""))
		}
	}
	if req.ExpectPosition != nil && !strings.EqualFold(previous, strings.TrimSpace(*req.ExpectPosition)) {
		return nil, fmt.Errorf("legend position mismatch: expected %q but found %q", *req.ExpectPosition, previous)
	}

	if req.Remove {
		if legend != nil {
			chartElem.RemoveChild(legend)
		}
		if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
		}
		return &SetLegendResult{PreviousPosition: previous, Removed: true, Legend: LegendStyle{Present: false}}, nil
	}

	if legend == nil {
		legend = b.c("legend")
		insertInOrder(chartElem, legend, chartChildOrder)
	}
	if req.SetPosition {
		if pos := firstDirectChild(legend, "legendPos"); pos != nil {
			pos.CreateAttr("val", req.Position)
		} else {
			insertInOrder(legend, b.cVal("legendPos", req.Position), legendChildOrder)
		}
	} else if firstDirectChild(legend, "legendPos") == nil {
		// A legend with no position is valid (defaults to right); make it explicit.
		insertInOrder(legend, b.cVal("legendPos", "r"), legendChildOrder)
	}
	if req.Overlay != nil {
		if ov := firstDirectChild(legend, "overlay"); ov != nil {
			ov.CreateAttr("val", boolAttr(*req.Overlay))
		} else {
			insertInOrder(legend, b.cVal("overlay", boolAttr(*req.Overlay)), legendChildOrder)
		}
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	return &SetLegendResult{PreviousPosition: previous, Legend: inspectLegend(firstDirectChild(chartElem, "legend"))}, nil
}

// SetSeriesStyleRequest sets fill/line/marker visual style on one chart series.
type SetSeriesStyleRequest struct {
	Package           opc.PackageSession
	ChartURI          string
	SeriesNumber      int
	FillColor         string // hex RRGGBB; "" leaves unchanged
	LineColor         string // hex RRGGBB; "" leaves unchanged
	LineWidthPt       *float64
	MarkerSymbol      string // circle, square, diamond, triangle, none; "" unchanged
	MarkerSize        *int
	ExpectSeriesCount *int // guard: chart must have this many series
}

// SetSeriesStyleResult reports the chart series count and the new series readback.
type SetSeriesStyleResult struct {
	SeriesCount int
	Series      SeriesStyle
}

// SetSeriesStyle applies fill, line, and marker style to one series. Marker
// styling is rejected on chart types whose series have no CT_Marker child.
func SetSeriesStyle(req *SetSeriesStyleRequest) (*SetSeriesStyleResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set chart series style request is nil")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	b := newStyleBuilder(root)
	series := walkSeries(root)
	if req.ExpectSeriesCount != nil && *req.ExpectSeriesCount != len(series) {
		return nil, fmt.Errorf("series count mismatch: expected %d but found %d", *req.ExpectSeriesCount, len(series))
	}
	if req.SeriesNumber < 1 || req.SeriesNumber > len(series) {
		return nil, fmt.Errorf("series %d is out of range (1-%d)", req.SeriesNumber, len(series))
	}
	ser := series[req.SeriesNumber-1]

	if req.FillColor != "" || req.LineColor != "" || req.LineWidthPt != nil {
		spPr := firstDirectChild(ser, "spPr")
		if spPr == nil {
			spPr = b.c("spPr")
			insertInOrder(ser, spPr, seriesChildOrder)
		}
		if req.FillColor != "" {
			setSolidFill(b, spPr, req.FillColor, shapePropsChildOrder)
		}
		if req.LineColor != "" || req.LineWidthPt != nil {
			ln := firstDirectChild(spPr, "ln")
			if ln == nil {
				ln = b.a("ln")
				insertInOrder(spPr, ln, shapePropsChildOrder)
			}
			if req.LineWidthPt != nil {
				ln.CreateAttr("w", strconv.Itoa(emuFromPoints(*req.LineWidthPt)))
			}
			if req.LineColor != "" {
				setSolidFill(b, ln, req.LineColor, lineChildOrder)
			}
		}
	}

	if req.MarkerSymbol != "" || req.MarkerSize != nil {
		parentType := seriesChartType(ser)
		if !markerChartTypes[parentType] {
			return nil, fmt.Errorf("series %d belongs to a %s, which does not support markers", req.SeriesNumber, displayChartType(parentType))
		}
		marker := firstDirectChild(ser, "marker")
		if marker == nil {
			marker = b.c("marker")
			insertInOrder(ser, marker, seriesChildOrder)
		}
		if req.MarkerSymbol != "" {
			if sym := firstDirectChild(marker, "symbol"); sym != nil {
				sym.CreateAttr("val", req.MarkerSymbol)
			} else {
				insertInOrder(marker, b.cVal("symbol", req.MarkerSymbol), markerChildOrder)
			}
		}
		if req.MarkerSize != nil {
			if sz := firstDirectChild(marker, "size"); sz != nil {
				sz.CreateAttr("val", strconv.Itoa(*req.MarkerSize))
			} else {
				insertInOrder(marker, b.cVal("size", strconv.Itoa(*req.MarkerSize)), markerChildOrder)
			}
		}
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	return &SetSeriesStyleResult{SeriesCount: len(series), Series: inspectSeriesStyle(ser, req.SeriesNumber)}, nil
}

// SetAxisRequest sets practical properties of one category or value axis.
type SetAxisRequest struct {
	Package  opc.PackageSession
	ChartURI string
	// AxisKind selects the axis: "category" (catAx) or "value" (valAx). The
	// selector rejects ambiguity (e.g. scatter charts with two value axes).
	AxisKind string

	SetTitle bool   // whether to apply Title
	Title    string // literal axis title text ("" clears the title)

	SetHidden bool
	Hidden    bool // c:delete val (true hides the axis)

	Min          *float64 // c:scaling/c:min
	Max          *float64 // c:scaling/c:max
	MajorUnit    *float64 // c:majorUnit (value axes)
	NumberFormat string   // c:numFmt formatCode ("" leaves unchanged)

	SetMajorGridlines bool
	MajorGridlines    bool
	SetMinorGridlines bool
	MinorGridlines    bool

	// Tick-label font basics, written to the axis-wide c:txPr default run props.
	TickLabelFontFamily string
	TickLabelFontSizePt *float64
	TickLabelFontColor  string
	TickLabelFontBold   *bool
	TickLabelFontItalic *bool

	// TitleFont basics applied to the axis title run when SetTitle is set.
	TitleFontFamily string
	TitleFontSizePt *float64
	TitleFontColor  string
	TitleFontBold   *bool
	TitleFontItalic *bool

	ExpectAxisTitle *string // guard: current axis title must equal this
	ExpectAxisCount *int    // guard: plotArea must have this many axes
}

// SetAxisResult reports the previous axis title and the post-mutation readback.
type SetAxisResult struct {
	PreviousTitle string
	AxisCount     int
	Axis          AxisStyle
}

// SetAxis applies title, visibility, scaling, number format, gridline, and
// tick-label font changes to a single category or value axis of a DrawingML
// chart. The catAx/valAx child order is respected so output is Office-valid.
func SetAxis(req *SetAxisRequest) (*SetAxisResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set chart axis request is nil")
	}
	kind := strings.ToLower(strings.TrimSpace(req.AxisKind))
	if kind != "category" && kind != "value" {
		return nil, fmt.Errorf("--axis is required; use category or value")
	}
	// majorUnit is a value-axis property; the catAx schema has no majorUnit slot,
	// so writing it on a category axis would append out of schema order and Excel
	// would reject the file. Reject up front with an actionable message.
	if req.MajorUnit != nil && kind != "value" {
		return nil, fmt.Errorf("major unit applies only to the value axis (use --axis value)")
	}
	root, doc, err := openChartRoot(req.Package, req.ChartURI)
	if err != nil {
		return nil, err
	}
	plotArea := firstDescendant(root, "plotArea")
	if plotArea == nil {
		return nil, fmt.Errorf("chart part %s has no plotArea", req.ChartURI)
	}

	var allAxes []*etree.Element
	for _, child := range plotArea.ChildElements() {
		switch localName(child.Tag) {
		case "catAx", "valAx", "dateAx", "serAx":
			allAxes = append(allAxes, child)
		}
	}
	if req.ExpectAxisCount != nil && *req.ExpectAxisCount != len(allAxes) {
		return nil, fmt.Errorf("axis count mismatch: expected %d but found %d", *req.ExpectAxisCount, len(allAxes))
	}

	wantElem := "catAx"
	if kind == "value" {
		wantElem = "valAx"
	}
	var matches []*etree.Element
	for _, ax := range allAxes {
		if localName(ax.Tag) == wantElem {
			matches = append(matches, ax)
		}
	}
	if len(matches) == 0 {
		return nil, fmt.Errorf("chart has no %s axis", kind)
	}
	if len(matches) > 1 {
		return nil, fmt.Errorf("chart has %d %s axes (e.g. a scatter chart's x and y axes); axis selection is ambiguous, narrow the chart with --chart or guard with --expect-axis-count", len(matches), kind)
	}
	axis := matches[0]
	order := axisChildOrder(localName(axis.Tag))

	previousTitle := titleText(firstDirectChild(axis, "title"))
	if req.ExpectAxisTitle != nil && strings.TrimSpace(previousTitle) != strings.TrimSpace(*req.ExpectAxisTitle) {
		return nil, fmt.Errorf("axis title mismatch: expected %q but found %q", *req.ExpectAxisTitle, previousTitle)
	}

	b := newStyleBuilder(root)

	if req.SetTitle {
		if strings.TrimSpace(req.Title) == "" {
			if title := firstDirectChild(axis, "title"); title != nil {
				axis.RemoveChild(title)
			}
		} else {
			title := firstDirectChild(axis, "title")
			if title == nil {
				title = b.c("title")
				insertInOrder(axis, title, order)
			}
			run, err := setTitleRichText(b, title, req.Title)
			if err != nil {
				return nil, err
			}
			applyRunFont(b, run, fontFields{
				family: req.TitleFontFamily,
				sizePt: req.TitleFontSizePt,
				color:  req.TitleFontColor,
				bold:   req.TitleFontBold,
				italic: req.TitleFontItalic,
			})
		}
	}

	if req.SetHidden {
		if del := firstDirectChild(axis, "delete"); del != nil {
			del.CreateAttr("val", boolAttr(req.Hidden))
		} else {
			insertInOrder(axis, b.cVal("delete", boolAttr(req.Hidden)), order)
		}
	}

	if req.Min != nil || req.Max != nil {
		scaling := firstDirectChild(axis, "scaling")
		if scaling == nil {
			scaling = b.c("scaling")
			insertInOrder(axis, scaling, order)
		}
		if req.Max != nil {
			setOrCreateValChild(b, scaling, "max", formatFloat(*req.Max), scalingChildOrder)
		}
		if req.Min != nil {
			setOrCreateValChild(b, scaling, "min", formatFloat(*req.Min), scalingChildOrder)
		}
	}

	if req.MajorUnit != nil {
		setOrCreateValChild(b, axis, "majorUnit", formatFloat(*req.MajorUnit), order)
	}

	if req.NumberFormat != "" {
		numFmt := firstDirectChild(axis, "numFmt")
		if numFmt == nil {
			numFmt = b.c("numFmt")
			insertInOrder(axis, numFmt, order)
		}
		numFmt.CreateAttr("formatCode", req.NumberFormat)
		numFmt.CreateAttr("sourceLinked", "0")
	}

	if req.SetMajorGridlines {
		applyGridlines(b, axis, "majorGridlines", req.MajorGridlines, order)
	}
	if req.SetMinorGridlines {
		applyGridlines(b, axis, "minorGridlines", req.MinorGridlines, order)
	}

	tickFont := fontFields{
		family: req.TickLabelFontFamily,
		sizePt: req.TickLabelFontSizePt,
		color:  req.TickLabelFontColor,
		bold:   req.TickLabelFontBold,
		italic: req.TickLabelFontItalic,
	}
	if !tickFont.empty() {
		applyAxisTickLabelFont(b, axis, tickFont, order)
	}

	if err := req.Package.ReplaceXMLPart(req.ChartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace chart part %s: %w", req.ChartURI, err)
	}
	return &SetAxisResult{
		PreviousTitle: previousTitle,
		AxisCount:     len(allAxes),
		Axis:          inspectSingleAxis(axis),
	}, nil
}

// inspectSingleAxis reads one axis element back into an AxisStyle, mirroring
// inspectAxes for a single child.
func inspectSingleAxis(axis *etree.Element) AxisStyle {
	for _, a := range inspectAxes(axis.Parent()) {
		if a.Element == localName(axis.Tag) {
			id := ""
			if axId := firstDirectChild(axis, "axId"); axId != nil {
				id = strings.TrimSpace(axId.SelectAttrValue("val", ""))
			}
			if a.AxisID == id {
				return a
			}
		}
	}
	return AxisStyle{Element: localName(axis.Tag), Kind: axisKind(localName(axis.Tag))}
}

// applyGridlines adds (on) or removes (off) a gridlines element. Presence of the
// element means gridlines are shown.
func applyGridlines(b styleBuilder, axis *etree.Element, name string, on bool, order []string) {
	existing := firstDirectChild(axis, name)
	if on {
		if existing == nil {
			insertInOrder(axis, b.c(name), order)
		}
	} else if existing != nil {
		axis.RemoveChild(existing)
	}
}

// applyAxisTickLabelFont writes tick-label font basics into the axis-wide c:txPr
// (a:p/a:pPr/a:defRPr), creating the structure when absent.
func applyAxisTickLabelFont(b styleBuilder, axis *etree.Element, font fontFields, order []string) {
	txPr := firstDirectChild(axis, "txPr")
	if txPr == nil {
		txPr = b.c("txPr")
		txPr.AddChild(b.a("bodyPr"))
		txPr.AddChild(b.a("lstStyle"))
		insertInOrder(axis, txPr, order)
	}
	p := firstDirectChild(txPr, "p")
	if p == nil {
		p = b.a("p")
		insertInOrder(txPr, p, txPrChildOrder)
	}
	pPr := firstDirectChild(p, "pPr")
	if pPr == nil {
		pPr = b.a("pPr")
		insertInOrder(p, pPr, paragraphChildOrder)
	}
	defRPr := firstDirectChild(pPr, "defRPr")
	if defRPr == nil {
		defRPr = b.a("defRPr")
		pPr.AddChild(defRPr)
	}
	applyFontToRPr(b, defRPr, font)
	// A paragraph with only pPr must still carry an a:endParaRPr to be valid.
	if firstDirectChild(p, "endParaRPr") == nil && firstDirectChild(p, "r") == nil {
		insertInOrder(p, b.a("endParaRPr"), paragraphChildOrder)
	}
}

// setOrCreateValChild sets the val attribute of a child element, creating it at
// its schema position when absent.
func setOrCreateValChild(b styleBuilder, parent *etree.Element, name, val string, order []string) {
	if child := firstDirectChild(parent, name); child != nil {
		child.CreateAttr("val", val)
		return
	}
	insertInOrder(parent, b.cVal(name, val), order)
}

func formatFloat(v float64) string {
	return strconv.FormatFloat(v, 'f', -1, 64)
}

// NormalizeHexColor validates an RRGGBB color (optionally #-prefixed) and
// returns the uppercase 6-digit form used in srgbClr values.
func NormalizeHexColor(value string) (string, error) {
	v := strings.ToUpper(strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(value), "#")))
	if len(v) != 6 {
		return "", fmt.Errorf("color %q must be a 6-digit hex like #1F77B4", value)
	}
	for _, r := range v {
		if !((r >= '0' && r <= '9') || (r >= 'A' && r <= 'F')) {
			return "", fmt.Errorf("color %q must be a 6-digit hex like #1F77B4", value)
		}
	}
	return v, nil
}

// styleBuilder creates chart (c:) and DrawingML main (a:) elements using the
// prefixes actually bound on a chart part's root, matching the producer.
type styleBuilder struct {
	cp string
	ap string
}

func newStyleBuilder(root *etree.Element) styleBuilder {
	b := styleBuilder{
		cp: nsPrefixOrDefault(root, nsChart, "c"),
		ap: nsPrefixOrDefault(root, nsA, "a"),
	}
	ensureNamespace(root, b.cp, nsChart)
	ensureNamespace(root, b.ap, nsA)
	return b
}

func (b styleBuilder) c(local string) *etree.Element { return etree.NewElement(b.cp + ":" + local) }
func (b styleBuilder) a(local string) *etree.Element { return etree.NewElement(b.ap + ":" + local) }

func (b styleBuilder) cVal(local, val string) *etree.Element {
	e := b.c(local)
	e.CreateAttr("val", val)
	return e
}

func nsPrefixOrDefault(root *etree.Element, uri, fallback string) string {
	if root != nil {
		for _, attr := range root.Attr {
			if attr.Space == "xmlns" && attr.Value == uri {
				return attr.Key
			}
		}
	}
	return fallback
}

func ensureNamespace(root *etree.Element, prefix, uri string) {
	if root == nil || prefix == "" {
		return
	}
	for _, attr := range root.Attr {
		if attr.Space == "xmlns" && attr.Value == uri {
			return
		}
	}
	root.CreateAttr("xmlns:"+prefix, uri)
}

func openChartRoot(session opc.PackageSession, chartURI string) (*etree.Element, *etree.Document, error) {
	if session == nil {
		return nil, nil, fmt.Errorf("package session is nil")
	}
	if strings.TrimSpace(chartURI) == "" {
		return nil, nil, fmt.Errorf("chart URI is required")
	}
	doc, err := session.ReadXMLPart(chartURI)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read chart part %s: %w", chartURI, err)
	}
	root := doc.Root()
	if root == nil || !isLocal(root, "chartSpace") {
		return nil, nil, fmt.Errorf("chart part %s root element not found", chartURI)
	}
	return root, doc, nil
}

// insertInOrder inserts child into parent before the first existing child whose
// schema rank is greater, keeping the element sequence schema-valid.
func insertInOrder(parent, child *etree.Element, order []string) {
	rank := make(map[string]int, len(order))
	for i, name := range order {
		rank[name] = i
	}
	childRank, known := rank[localName(child.Tag)]
	if known {
		for _, existing := range parent.ChildElements() {
			if existing == child {
				continue
			}
			if r, ok := rank[localName(existing.Tag)]; ok && r > childRank {
				parent.InsertChildAt(existing.Index(), child)
				return
			}
		}
	}
	parent.AddChild(child)
}

// setSolidFill replaces any existing fill-group child of holder with a single
// solid srgbClr fill inserted at its schema position.
func setSolidFill(b styleBuilder, holder *etree.Element, hexColor string, order []string) {
	for _, name := range []string{"noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill"} {
		for _, child := range directChildren(holder, name) {
			holder.RemoveChild(child)
		}
	}
	fill := b.a("solidFill")
	clr := b.a("srgbClr")
	clr.CreateAttr("val", normalizeHexColorLoose(hexColor))
	fill.AddChild(clr)
	insertInOrder(holder, fill, order)
}

// seriesChartType returns the local name of the chart-type element that owns ser
// (e.g. barChart, lineChart, scatterChart).
func seriesChartType(ser *etree.Element) string {
	if ser == nil || ser.Parent() == nil {
		return ""
	}
	return localName(ser.Parent().Tag)
}

func displayChartType(local string) string {
	if local == "" {
		return "chart of this type"
	}
	return local
}

func normalizeHexColorLoose(value string) string {
	if v, err := NormalizeHexColor(value); err == nil {
		return v
	}
	return strings.ToUpper(strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(value), "#")))
}

func boolAttr(value bool) string {
	if value {
		return "1"
	}
	return "0"
}

func hundredthsFromPoints(pt float64) int { return int(pt*100 + 0.5) }

func emuFromPoints(pt float64) int { return int(pt*emuPerPoint + 0.5) }
