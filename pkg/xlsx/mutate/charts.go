package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
)

// chart axis ids (arbitrary but stable within a chart part).
const (
	catAxID = 111111111
	valAxID = 222222222
)

var validChartTypes = map[string]bool{
	"bar": true, "line": true, "area": true, "pie": true, "scatter": true,
}

// CreateChartRequest authors a new embedded chart from a worksheet range.
type CreateChartRequest struct {
	Package     opc.PackageSession
	WorkbookURI string
	SheetRef    model.SheetRef
	ChartType   string
	SourceSheet string // sheet name used in c:f formula refs
	SourceRange address.RangeRef
	SourceCells [][]rangeio.Cell
	Title       string
	AnchorFrom  address.CellRef
	AnchorTo    address.CellRef
}

// CreateChartResult reports the authored chart.
type CreateChartResult struct {
	ChartURI    string   `json:"chartPartUri"`
	DrawingURI  string   `json:"drawingPartUri"`
	ChartType   string   `json:"chartType"`
	Title       string   `json:"title,omitempty"`
	SeriesCount int      `json:"seriesCount"`
	Categories  int      `json:"categories"`
	Anchor      string   `json:"anchor"`
	Warnings    []string `json:"warnings,omitempty"`
}

type chartSeries struct {
	name    string
	nameRef string
	cats    []string
	catRef  string
	values  []string
	valRef  string
}

// CreateChart builds a chart part, wires it through the worksheet drawing, and
// creates the drawing part when the worksheet does not already have one.
func CreateChart(req *CreateChartRequest) (*CreateChartResult, error) {
	if req == nil {
		return nil, fmt.Errorf("create chart request is nil")
	}
	if !validChartTypes[req.ChartType] {
		return nil, fmt.Errorf("invalid chart type %q (bar, line, area, pie, scatter)", req.ChartType)
	}
	if req.SheetRef.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", req.SheetRef.Name)
	}
	chartXML, seriesCount, cats, warnings, err := BuildChartPart(req.ChartType, req.Title, req.SourceSheet, req.SourceRange, req.SourceCells)
	if err != nil {
		return nil, err
	}

	chartURI := allocateNumberedPart(req.Package, "/xl/charts/chart", ".xml")

	if err := req.Package.AddPart(chartURI, chartXML, namespaces.ContentTypeChart, nil); err != nil {
		return nil, fmt.Errorf("failed to add chart part: %w", err)
	}

	drawingURI, _, found, err := worksheetDrawing(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if found {
		if _, err := appendChartToDrawing(req.Package, drawingURI, chartURI, req.AnchorFrom, req.AnchorTo); err != nil {
			return nil, err
		}
	} else {
		drawingURI = allocateNumberedPart(req.Package, "/xl/drawings/drawing", ".xml")
		drawingChartRID := opc.AllocateRelationshipID(nil)
		drawingRels := []opc.RelationshipInfo{{
			SourceURI: drawingURI, ID: drawingChartRID, Type: namespaces.RelChart,
			Target: opc.RelationshipTarget(drawingURI, chartURI),
		}}
		drawingXML, err := buildDrawingPartXML(req.AnchorFrom, req.AnchorTo, drawingChartRID)
		if err != nil {
			return nil, err
		}
		if err := req.Package.AddPart(drawingURI, drawingXML, namespaces.ContentTypeDrawing, nil); err != nil {
			return nil, fmt.Errorf("failed to add drawing part: %w", err)
		}
		if err := opc.WriteRelationships(req.Package, drawingURI, drawingRels); err != nil {
			return nil, fmt.Errorf("failed to write drawing relationships: %w", err)
		}

		// worksheet -> drawing relationship + <drawing r:id>
		wsRels := req.Package.ListRelationships(req.SheetRef.PartURI)
		drawingRID := opc.AllocateRelationshipID(wsRels)
		wsRels = append(wsRels, opc.RelationshipInfo{
			SourceURI: req.SheetRef.PartURI, ID: drawingRID, Type: namespaces.RelDrawing,
			Target: opc.RelationshipTarget(req.SheetRef.PartURI, drawingURI),
		})
		if err := opc.WriteRelationships(req.Package, req.SheetRef.PartURI, wsRels); err != nil {
			return nil, fmt.Errorf("failed to write worksheet relationships: %w", err)
		}
		if err := addWorksheetDrawingRef(req.Package, req.SheetRef, drawingRID); err != nil {
			return nil, err
		}
	}

	anchor := cellRef(req.AnchorFrom.Column, req.AnchorFrom.Row) + ":" + cellRef(req.AnchorTo.Column, req.AnchorTo.Row)
	return &CreateChartResult{
		ChartURI: chartURI, DrawingURI: drawingURI, ChartType: req.ChartType,
		Title: req.Title, SeriesCount: seriesCount, Categories: cats, Anchor: anchor, Warnings: warnings,
	}, nil
}

// BuildChartPart builds the c:chartSpace XML for a chart of the given type from
// a source matrix (first column = categories, remaining columns = series, first
// row = series names). It returns the serialized chart part, the series and
// category counts, and any coercion warnings. The sourceSheet/srcRange describe
// the c:f formula refs embedded in the chart caches. This is the shared chart
// authoring core reused by both xlsx and pptx chart creation.
func BuildChartPart(chartType, title, sourceSheet string, srcRange address.RangeRef, cells [][]rangeio.Cell) (xml []byte, seriesCount, categories int, warnings []string, err error) {
	if !validChartTypes[chartType] {
		return nil, 0, 0, nil, fmt.Errorf("invalid chart type %q (bar, line, area, pie, scatter)", chartType)
	}
	series, cats, warnings, err := buildChartSeries(&CreateChartRequest{
		SourceSheet: sourceSheet,
		SourceRange: srcRange,
		SourceCells: cells,
	})
	if err != nil {
		return nil, 0, 0, nil, err
	}
	if len(series) == 0 {
		return nil, 0, 0, nil, fmt.Errorf("source range produced no chart series")
	}
	if chartType == "pie" && len(series) > 1 {
		series = series[:1]
		warnings = append(warnings, "pie chart uses only the first series")
	}
	xml, err = buildChartPartXML(chartType, title, series)
	if err != nil {
		return nil, 0, 0, nil, err
	}
	return xml, len(series), cats, warnings, nil
}

// buildChartSeries interprets the source matrix as categories (first column) +
// one series per remaining column, with the first row as series names.
func buildChartSeries(req *CreateChartRequest) ([]chartSeries, int, []string, error) {
	matrix := req.SourceCells
	if len(matrix) == 0 {
		return nil, 0, nil, fmt.Errorf("source range is empty")
	}
	minCol, minRow, maxCol, maxRow := req.SourceRange.Bounds()
	rows := maxRow - minRow + 1
	cols := maxCol - minCol + 1
	var warnings []string

	hasHeader := rows > 1
	dataStartRow := minRow
	if hasHeader {
		dataStartRow = minRow + 1
	}
	dataRowCount := maxRow - dataStartRow + 1
	if dataRowCount <= 0 {
		return nil, 0, nil, fmt.Errorf("source range has no data rows")
	}

	cellAt := func(r, c int) rangeio.Cell {
		ri := r - minRow
		ci := c - minCol
		if ri < 0 || ri >= len(matrix) || ci < 0 || ci >= len(matrix[ri]) {
			return rangeio.Cell{Null: true}
		}
		return matrix[ri][ci]
	}
	text := func(cell rangeio.Cell) string {
		if cell.Null {
			return ""
		}
		return cell.Value
	}

	// Categories from the first column (unless only a single column exists).
	hasCategories := cols > 1
	catCol := minCol
	var cats []string
	if hasCategories {
		for r := dataStartRow; r <= maxRow; r++ {
			cats = append(cats, text(cellAt(r, catCol)))
		}
	}
	catRef := absRef(req.SourceSheet, catCol, dataStartRow, catCol, maxRow)

	firstSeriesCol := minCol
	if hasCategories {
		firstSeriesCol = minCol + 1
	}
	coerced := 0
	var series []chartSeries
	for c := firstSeriesCol; c <= maxCol; c++ {
		s := chartSeries{catRef: catRef, cats: cats}
		if hasHeader {
			s.name = text(cellAt(minRow, c))
			s.nameRef = absRef(req.SourceSheet, c, minRow, c, minRow)
		}
		for r := dataStartRow; r <= maxRow; r++ {
			cell := cellAt(r, c)
			v, wasCoerced := numericTextCoerced(cell)
			if wasCoerced {
				coerced++
			}
			s.values = append(s.values, v)
		}
		s.valRef = absRef(req.SourceSheet, c, dataStartRow, c, maxRow)
		series = append(series, s)
	}
	if !hasCategories {
		warnings = append(warnings, "single-column source: no categories axis")
	}
	if coerced > 0 {
		warnings = append(warnings, fmt.Sprintf("%d non-numeric value(s) treated as 0", coerced))
	}
	return series, len(cats), warnings, nil
}

// numericTextCoerced returns the cell's numeric text and whether a non-empty,
// non-numeric value was coerced to "0".
func numericTextCoerced(cell rangeio.Cell) (string, bool) {
	if cell.Null || cell.Value == "" {
		return "0", false
	}
	if _, err := strconv.ParseFloat(cell.Value, 64); err == nil {
		return cell.Value, false
	}
	return "0", true
}

func absRef(sheet string, col1, row1, col2, row2 int) string {
	a, _ := address.ColumnIndexToLetters(col1)
	b, _ := address.ColumnIndexToLetters(col2)
	q := quoteChartSheet(sheet)
	if col1 == col2 && row1 == row2 {
		return fmt.Sprintf("%s!$%s$%d", q, a, row1)
	}
	return fmt.Sprintf("%s!$%s$%d:$%s$%d", q, a, row1, b, row2)
}

// quoteChartSheet single-quotes a sheet name for a formula, doubling any
// embedded apostrophes per the ECMA-376 formula grammar.
func quoteChartSheet(sheet string) string {
	return "'" + strings.ReplaceAll(sheet, "'", "''") + "'"
}

// ---- chart part XML ----

func cel(local string) *etree.Element { return newElement("c", local) }

func celVal(local, val string) *etree.Element {
	e := cel(local)
	e.CreateAttr("val", val)
	return e
}

func buildChartPartXML(chartType, title string, series []chartSeries) ([]byte, error) {
	doc := etree.NewDocument()
	root := cel("chartSpace")
	root.CreateAttr("xmlns:c", namespaces.NsChart)
	root.CreateAttr("xmlns:a", namespaces.NsDrawingMain)
	root.CreateAttr("xmlns:r", namespaces.NsR)
	doc.SetRoot(root)

	chart := cel("chart")
	root.AddChild(chart)
	if strings.TrimSpace(title) != "" {
		chart.AddChild(buildChartTitle(title))
		chart.AddChild(celVal("autoTitleDeleted", "0"))
	} else {
		chart.AddChild(celVal("autoTitleDeleted", "1"))
	}
	plotArea := cel("plotArea")
	chart.AddChild(plotArea)
	plotArea.AddChild(cel("layout"))

	plot := buildPlot(chartType, series)
	plotArea.AddChild(plot)

	if chartType != "pie" {
		plotArea.AddChild(buildCatAx(chartType))
		plotArea.AddChild(buildValAx())
	}
	chart.AddChild(celVal("plotVisOnly", "1"))
	chart.AddChild(celVal("dispBlanksAs", "gap"))

	doc.IndentTabs()
	return doc.WriteToBytes()
}

func buildChartTitle(title string) *etree.Element {
	t := cel("title")
	tx := cel("tx")
	rich := cel("rich")
	rich.AddChild(newElement("a", "bodyPr"))
	rich.AddChild(newElement("a", "lstStyle"))
	p := newElement("a", "p")
	run := newElement("a", "r")
	tEl := newElement("a", "t")
	tEl.SetText(title)
	run.AddChild(tEl)
	p.AddChild(run)
	rich.AddChild(p)
	tx.AddChild(rich)
	t.AddChild(tx)
	t.AddChild(celVal("overlay", "0"))
	return t
}

func buildPlot(chartType string, series []chartSeries) *etree.Element {
	switch chartType {
	case "bar":
		plot := cel("barChart")
		plot.AddChild(celVal("barDir", "col"))
		plot.AddChild(celVal("grouping", "clustered"))
		plot.AddChild(celVal("varyColors", "0"))
		for i, s := range series {
			plot.AddChild(buildCategorySeries(i, s, false))
		}
		plot.AddChild(celVal("axId", strconv.Itoa(catAxID)))
		plot.AddChild(celVal("axId", strconv.Itoa(valAxID)))
		return plot
	case "line":
		plot := cel("lineChart")
		plot.AddChild(celVal("grouping", "standard"))
		plot.AddChild(celVal("varyColors", "0"))
		for i, s := range series {
			plot.AddChild(buildCategorySeries(i, s, false))
		}
		plot.AddChild(celVal("marker", "1"))
		plot.AddChild(celVal("axId", strconv.Itoa(catAxID)))
		plot.AddChild(celVal("axId", strconv.Itoa(valAxID)))
		return plot
	case "area":
		plot := cel("areaChart")
		plot.AddChild(celVal("grouping", "standard"))
		plot.AddChild(celVal("varyColors", "0"))
		for i, s := range series {
			plot.AddChild(buildCategorySeries(i, s, false))
		}
		plot.AddChild(celVal("axId", strconv.Itoa(catAxID)))
		plot.AddChild(celVal("axId", strconv.Itoa(valAxID)))
		return plot
	case "pie":
		plot := cel("pieChart")
		plot.AddChild(celVal("varyColors", "1"))
		for i, s := range series {
			plot.AddChild(buildCategorySeries(i, s, false))
		}
		plot.AddChild(celVal("firstSliceAng", "0"))
		return plot
	case "scatter":
		plot := cel("scatterChart")
		plot.AddChild(celVal("scatterStyle", "lineMarker"))
		plot.AddChild(celVal("varyColors", "0"))
		for i, s := range series {
			plot.AddChild(buildScatterSeries(i, s))
		}
		plot.AddChild(celVal("axId", strconv.Itoa(catAxID)))
		plot.AddChild(celVal("axId", strconv.Itoa(valAxID)))
		return plot
	}
	return cel("barChart")
}

func buildSeriesHeader(idx int, s chartSeries) *etree.Element {
	ser := cel("ser")
	ser.AddChild(celVal("idx", strconv.Itoa(idx)))
	ser.AddChild(celVal("order", strconv.Itoa(idx)))
	if s.nameRef != "" {
		tx := cel("tx")
		tx.AddChild(buildStrRef(s.nameRef, []string{s.name}))
		ser.AddChild(tx)
	}
	return ser
}

func buildCategorySeries(idx int, s chartSeries, _ bool) *etree.Element {
	ser := buildSeriesHeader(idx, s)
	if s.catRef != "" && len(s.cats) > 0 {
		cat := cel("cat")
		cat.AddChild(buildStrRef(s.catRef, s.cats))
		ser.AddChild(cat)
	}
	val := cel("val")
	val.AddChild(buildNumRef(s.valRef, s.values))
	ser.AddChild(val)
	return ser
}

func buildScatterSeries(idx int, s chartSeries) *etree.Element {
	ser := buildSeriesHeader(idx, s)
	// Scatter x-values must be numeric (c:numRef). Non-numeric categories
	// (e.g. labels) are coerced to their 1-based position so the numCache stays
	// schema-valid rather than putting strings into a numeric cache.
	xVal := cel("xVal")
	if s.catRef != "" && len(s.cats) > 0 {
		xVal.AddChild(buildNumRef(s.catRef, numericAxis(s.cats)))
	} else {
		xVal.AddChild(buildNumRef(s.valRef, s.values))
	}
	ser.AddChild(xVal)
	yVal := cel("yVal")
	yVal.AddChild(buildNumRef(s.valRef, s.values))
	ser.AddChild(yVal)
	return ser
}

// numericAxis returns numeric strings for a scatter x-axis: numeric inputs pass
// through, anything else becomes its 1-based position.
func numericAxis(vals []string) []string {
	out := make([]string, len(vals))
	for i, v := range vals {
		if _, err := strconv.ParseFloat(strings.TrimSpace(v), 64); err == nil && strings.TrimSpace(v) != "" {
			out[i] = v
		} else {
			out[i] = strconv.Itoa(i + 1)
		}
	}
	return out
}

func buildStrRef(ref string, vals []string) *etree.Element {
	strRef := cel("strRef")
	f := cel("f")
	f.SetText(ref)
	strRef.AddChild(f)
	cache := cel("strCache")
	cache.AddChild(celVal("ptCount", strconv.Itoa(len(vals))))
	for i, v := range vals {
		pt := cel("pt")
		pt.CreateAttr("idx", strconv.Itoa(i))
		vv := cel("v")
		vv.SetText(v)
		pt.AddChild(vv)
		cache.AddChild(pt)
	}
	strRef.AddChild(cache)
	return strRef
}

func buildNumRef(ref string, vals []string) *etree.Element {
	numRef := cel("numRef")
	f := cel("f")
	f.SetText(ref)
	numRef.AddChild(f)
	cache := cel("numCache")
	fc := cel("formatCode")
	fc.SetText("General")
	cache.AddChild(fc)
	cache.AddChild(celVal("ptCount", strconv.Itoa(len(vals))))
	for i, v := range vals {
		pt := cel("pt")
		pt.CreateAttr("idx", strconv.Itoa(i))
		vv := cel("v")
		vv.SetText(v)
		pt.AddChild(vv)
		cache.AddChild(pt)
	}
	numRef.AddChild(cache)
	return numRef
}

func buildCatAx(chartType string) *etree.Element {
	ax := cel("catAx")
	if chartType == "scatter" {
		ax = cel("valAx")
	}
	ax.AddChild(celVal("axId", strconv.Itoa(catAxID)))
	scaling := cel("scaling")
	scaling.AddChild(celVal("orientation", "minMax"))
	ax.AddChild(scaling)
	ax.AddChild(celVal("delete", "0"))
	ax.AddChild(celVal("axPos", "b"))
	ax.AddChild(celVal("crossAx", strconv.Itoa(valAxID)))
	return ax
}

func buildValAx() *etree.Element {
	ax := cel("valAx")
	ax.AddChild(celVal("axId", strconv.Itoa(valAxID)))
	scaling := cel("scaling")
	scaling.AddChild(celVal("orientation", "minMax"))
	ax.AddChild(scaling)
	ax.AddChild(celVal("delete", "0"))
	ax.AddChild(celVal("axPos", "l"))
	ax.AddChild(celVal("crossAx", strconv.Itoa(catAxID)))
	return ax
}

// ---- drawing part XML ----

func xel(local string) *etree.Element { return newElement("xdr", local) }

func buildDrawingPartXML(from, to address.CellRef, chartRID string) ([]byte, error) {
	doc := etree.NewDocument()
	root := xel("wsDr")
	root.CreateAttr("xmlns:xdr", namespaces.NsSpreadsheetDrawing)
	root.CreateAttr("xmlns:a", namespaces.NsDrawingMain)
	root.CreateAttr("xmlns:r", namespaces.NsR)
	root.CreateAttr("xmlns:c", namespaces.NsChart)
	doc.SetRoot(root)

	root.AddChild(buildChartAnchor(root.Space, from, to, chartRID, 2, 1))

	doc.IndentTabs()
	return doc.WriteToBytes()
}

func buildChartAnchor(prefix string, from, to address.CellRef, chartRID string, objectID, chartNumber int) *etree.Element {
	if prefix == "" {
		prefix = "xdr"
	}
	if objectID < 1 {
		objectID = 1
	}
	if chartNumber < 1 {
		chartNumber = 1
	}
	anchor := newElement(prefix, "twoCellAnchor")
	anchor.CreateAttr("editAs", "oneCell")
	anchor.AddChild(buildAnchorMarker("from", from.Column-1, from.Row-1))
	anchor.AddChild(buildAnchorMarker("to", to.Column-1, to.Row-1))

	frame := newElement(prefix, "graphicFrame")
	frame.CreateAttr("macro", "")
	anchor.AddChild(frame)
	nv := newElement(prefix, "nvGraphicFramePr")
	cNvPr := newElement(prefix, "cNvPr")
	cNvPr.CreateAttr("id", strconv.Itoa(objectID))
	cNvPr.CreateAttr("name", fmt.Sprintf("Chart %d", chartNumber))
	nv.AddChild(cNvPr)
	nv.AddChild(newElement(prefix, "cNvGraphicFramePr"))
	frame.AddChild(nv)

	xfrm := newElement(prefix, "xfrm")
	off := newElement("a", "off")
	off.CreateAttr("x", "0")
	off.CreateAttr("y", "0")
	ext := newElement("a", "ext")
	ext.CreateAttr("cx", "0")
	ext.CreateAttr("cy", "0")
	xfrm.AddChild(off)
	xfrm.AddChild(ext)
	frame.AddChild(xfrm)

	graphic := newElement("a", "graphic")
	graphicData := newElement("a", "graphicData")
	graphicData.CreateAttr("uri", namespaces.NsChart)
	chartRef := cel("chart")
	chartRef.CreateAttr("r:id", chartRID)
	graphicData.AddChild(chartRef)
	graphic.AddChild(graphicData)
	frame.AddChild(graphic)

	anchor.AddChild(newElement(prefix, "clientData"))

	return anchor
}

func buildAnchorMarker(name string, col0, row0 int) *etree.Element {
	if col0 < 0 {
		col0 = 0
	}
	if row0 < 0 {
		row0 = 0
	}
	m := xel(name)
	c := xel("col")
	c.SetText(strconv.Itoa(col0))
	m.AddChild(c)
	co := xel("colOff")
	co.SetText("0")
	m.AddChild(co)
	r := xel("row")
	r.SetText(strconv.Itoa(row0))
	m.AddChild(r)
	ro := xel("rowOff")
	ro.SetText("0")
	m.AddChild(ro)
	return m
}

func appendChartToDrawing(session opc.PackageSession, drawingURI, chartURI string, from, to address.CellRef) (string, error) {
	doc, err := session.ReadXMLPart(drawingURI)
	if err != nil {
		return "", fmt.Errorf("failed to read drawing part %s: %w", drawingURI, err)
	}
	root := doc.Root()
	if root == nil || localName(root.Tag) != "wsDr" {
		return "", fmt.Errorf("drawing part %s root element not found", drawingURI)
	}
	ensureDrawingNamespaces(root)

	rels := session.ListRelationships(drawingURI)
	chartRID := opc.AllocateRelationshipID(rels)
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: drawingURI, ID: chartRID, Type: namespaces.RelChart,
		Target: opc.RelationshipTarget(drawingURI, chartURI),
	})

	anchor := buildChartAnchor(root.Space, from, to, chartRID, nextDrawingObjectID(root), nextChartNumber(root))
	addDrawingAnchor(root, anchor)
	doc.IndentTabs()
	if err := session.ReplaceXMLPart(drawingURI, doc); err != nil {
		return "", fmt.Errorf("failed to replace drawing part %s: %w", drawingURI, err)
	}
	if err := opc.WriteRelationships(session, drawingURI, rels); err != nil {
		return "", fmt.Errorf("failed to write drawing relationships: %w", err)
	}
	return chartRID, nil
}

func ensureDrawingNamespaces(root *etree.Element) {
	if root.SelectAttr("xmlns:xdr") == nil {
		root.CreateAttr("xmlns:xdr", namespaces.NsSpreadsheetDrawing)
	}
	if root.SelectAttr("xmlns:a") == nil {
		root.CreateAttr("xmlns:a", namespaces.NsDrawingMain)
	}
	if root.SelectAttr("xmlns:r") == nil {
		root.CreateAttr("xmlns:r", namespaces.NsR)
	}
	if root.SelectAttr("xmlns:c") == nil {
		root.CreateAttr("xmlns:c", namespaces.NsChart)
	}
}

func addDrawingAnchor(root, anchor *etree.Element) {
	for _, child := range root.ChildElements() {
		if localName(child.Tag) == "extLst" {
			root.InsertChildAt(child.Index(), anchor)
			return
		}
	}
	root.AddChild(anchor)
}

func nextDrawingObjectID(root *etree.Element) int {
	maxID := 1
	for _, elem := range localDescendants(root, "cNvPr") {
		if id, err := strconv.Atoi(strings.TrimSpace(elem.SelectAttrValue("id", ""))); err == nil && id > maxID {
			maxID = id
		}
	}
	return maxID + 1
}

func nextChartNumber(root *etree.Element) int {
	count := 0
	for _, anchor := range root.ChildElements() {
		switch localName(anchor.Tag) {
		case "twoCellAnchor", "oneCellAnchor", "absoluteAnchor":
			for _, elem := range localDescendants(anchor, "chart") {
				if elem.NamespaceURI() == namespaces.NsChart || elem.Space == "c" {
					count++
					break
				}
			}
		}
	}
	return count + 1
}

func localDescendants(elem *etree.Element, local string) []*etree.Element {
	if elem == nil {
		return nil
	}
	var out []*etree.Element
	for _, child := range elem.ChildElements() {
		if localName(child.Tag) == local {
			out = append(out, child)
		}
		out = append(out, localDescendants(child, local)...)
	}
	return out
}

// ---- worksheet drawing reference ----

func worksheetDrawing(session opc.PackageSession, sheet model.SheetRef) (string, string, bool, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return "", "", false, err
	}
	drawing := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "drawing")
	if drawing == nil {
		return "", "", false, nil
	}
	rid, ok := namespaces.Attr(drawing, namespaces.NsR, "id")
	if !ok || strings.TrimSpace(rid) == "" {
		return "", "", false, fmt.Errorf("worksheet %s drawing is missing r:id", sheet.PartURI)
	}
	for _, rel := range session.ListRelationships(sheet.PartURI) {
		if rel.ID != rid {
			continue
		}
		if rel.TargetMode == "External" {
			return "", "", false, fmt.Errorf("worksheet %s drawing relationship %s is external", sheet.PartURI, rid)
		}
		if rel.Type != namespaces.RelDrawing {
			return "", "", false, fmt.Errorf("worksheet %s relationship %s is %s, expected drawing", sheet.PartURI, rid, rel.Type)
		}
		return opc.NormalizeURI(opc.ResolveRelationshipTarget(sheet.PartURI, rel.Target)), rid, true, nil
	}
	return "", "", false, fmt.Errorf("worksheet %s drawing relationship %s not found", sheet.PartURI, rid)
}

func addWorksheetDrawingRef(session opc.PackageSession, sheet model.SheetRef, rid string) error {
	doc, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return err
	}
	if existing := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "drawing"); existing != nil {
		existingRID, _ := namespaces.Attr(existing, namespaces.NsR, "id")
		if existingRID == rid {
			return nil
		}
		return fmt.Errorf("worksheet already has drawing relationship %s", existingRID)
	}
	prefix := root.Space
	drawing := newElement(prefix, "drawing")
	drawing.CreateAttr("r:id", rid)
	ensureRelationshipsNamespace(root)
	insertWorksheetChild(root, drawing, "drawing")
	if err := session.ReplaceXMLPart(sheet.PartURI, doc); err != nil {
		return fmt.Errorf("failed to replace worksheet %s: %w", sheet.PartURI, err)
	}
	return nil
}

// allocateNumberedPart returns the next free /prefixN.suffix part URI.
func allocateNumberedPart(session opc.PackageSession, prefix, suffix string) string {
	maxN := 0
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		if !strings.HasPrefix(uri, prefix) || !strings.HasSuffix(uri, suffix) {
			continue
		}
		mid := strings.TrimSuffix(strings.TrimPrefix(uri, prefix), suffix)
		if n, err := strconv.Atoi(mid); err == nil && n > maxN {
			maxN = n
		}
	}
	return fmt.Sprintf("%s%d%s", prefix, maxN+1, suffix)
}
