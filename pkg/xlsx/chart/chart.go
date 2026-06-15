// Package chart reads existing XLSX worksheet chart definitions.
package chart

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

const (
	nsChart = "http://schemas.openxmlformats.org/drawingml/2006/chart"
	nsA     = "http://schemas.openxmlformats.org/drawingml/2006/main"
)

// List returns existing worksheet charts in workbook and worksheet order.
func List(session opc.PackageSession, workbook *model.Workbook, sheets []model.SheetRef) ([]model.ChartRef, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if workbook == nil {
		return nil, fmt.Errorf("workbook is nil")
	}
	if sheets == nil {
		sheets = workbook.Sheets
	}

	var charts []model.ChartRef
	for _, sheetRef := range sheets {
		if sheetRef.PartURI == "" || sheetRef.RelationshipType != namespaces.RelWorksheet {
			continue
		}
		sheetCharts, err := listForSheet(session, sheetRef, len(charts)+1)
		if err != nil {
			return nil, err
		}
		charts = append(charts, sheetCharts...)
	}
	return charts, nil
}

func listForSheet(session opc.PackageSession, sheetRef model.SheetRef, startNumber int) ([]model.ChartRef, error) {
	doc, err := session.ReadXMLPart(sheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", sheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", sheetRef.PartURI)
	}

	relMap := mapRelationships(session.ListRelationships(sheetRef.PartURI))
	var charts []model.ChartRef
	for _, drawingElem := range namespaces.FindChildren(root, namespaces.NsSpreadsheetML, "drawing") {
		rid, ok := namespaces.Attr(drawingElem, namespaces.NsR, "id")
		if !ok || rid == "" {
			return nil, fmt.Errorf("worksheet %s drawing is missing r:id", sheetRef.PartURI)
		}
		rel, ok := relMap[rid]
		if !ok {
			return nil, fmt.Errorf("worksheet %s drawing relationship %s not found", sheetRef.PartURI, rid)
		}
		if rel.TargetMode == "External" {
			return nil, fmt.Errorf("worksheet %s drawing relationship %s is external", sheetRef.PartURI, rid)
		}
		if rel.Type != namespaces.RelDrawing {
			return nil, fmt.Errorf("worksheet %s relationship %s is %s, expected drawing", sheetRef.PartURI, rid, rel.Type)
		}
		drawingURI := resolveTargetURI(sheetRef.PartURI, rel.Target)
		drawingCharts, err := listForDrawing(session, sheetRef, rid, drawingURI, startNumber+len(charts))
		if err != nil {
			return nil, err
		}
		charts = append(charts, drawingCharts...)
	}
	return charts, nil
}

func listForDrawing(session opc.PackageSession, sheetRef model.SheetRef, drawingRid, drawingURI string, startNumber int) ([]model.ChartRef, error) {
	doc, err := session.ReadXMLPart(drawingURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read drawing part %s: %w", drawingURI, err)
	}
	root := doc.Root()
	if root == nil || !isLocal(root, "wsDr") {
		return nil, fmt.Errorf("drawing part %s root element not found", drawingURI)
	}

	relMap := mapRelationships(session.ListRelationships(drawingURI))
	var charts []model.ChartRef
	for _, anchor := range chartAnchors(root) {
		chartElem := firstDescendant(anchor, "chart")
		if chartElem == nil || chartElem.NamespaceURI() != nsChart {
			continue
		}
		chartRid, ok := attrNS(chartElem, namespaces.NsR, "id")
		if !ok || chartRid == "" {
			return nil, fmt.Errorf("drawing %s chart is missing r:id", drawingURI)
		}
		rel, ok := relMap[chartRid]
		if !ok {
			return nil, fmt.Errorf("drawing %s chart relationship %s not found", drawingURI, chartRid)
		}
		if rel.TargetMode == "External" {
			return nil, fmt.Errorf("drawing %s chart relationship %s is external", drawingURI, chartRid)
		}
		if rel.Type != namespaces.RelChart {
			return nil, fmt.Errorf("drawing %s relationship %s is %s, expected chart", drawingURI, chartRid, rel.Type)
		}
		chartURI := resolveTargetURI(drawingURI, rel.Target)
		chartRef, err := readChartPart(session, chartURI)
		if err != nil {
			return nil, err
		}
		chartRef.Number = startNumber + len(charts)
		chartRef.Sheet = sheetRef.Name
		chartRef.SheetNumber = sheetRef.Number
		chartRef.SheetPartURI = sheetRef.PartURI
		chartRef.DrawingRelationshipID = drawingRid
		chartRef.DrawingPartURI = drawingURI
		chartRef.RelationshipID = chartRid
		chartRef.PartURI = chartURI
		chartRef.Name = chartName(anchor)
		chartRef.Anchor = parseAnchor(anchor)
		chartRef = model.WithChartSelectors(chartRef)
		charts = append(charts, chartRef)
	}
	return charts, nil
}

// ReadChartPart reads a DrawingML chart part. The chart XML is shared by XLSX
// and PPTX chart parts; package-family-specific discovery lives elsewhere.
func ReadChartPart(session opc.PackageSession, chartURI string) (model.ChartRef, error) {
	return readChartPart(session, chartURI)
}

func readChartPart(session opc.PackageSession, chartURI string) (model.ChartRef, error) {
	doc, err := session.ReadXMLPart(chartURI)
	if err != nil {
		return model.ChartRef{}, fmt.Errorf("failed to read chart part %s: %w", chartURI, err)
	}
	root := doc.Root()
	if root == nil || !isLocal(root, "chartSpace") {
		return model.ChartRef{}, fmt.Errorf("chart part %s root element not found", chartURI)
	}

	chart := model.ChartRef{
		Title:  chartTitle(root),
		Types:  chartTypes(root),
		Series: chartSeries(root),
	}
	return chart, nil
}

func chartAnchors(root *etree.Element) []*etree.Element {
	if root == nil {
		return nil
	}
	var anchors []*etree.Element
	for _, child := range root.ChildElements() {
		switch localName(child.Tag) {
		case "twoCellAnchor", "oneCellAnchor", "absoluteAnchor":
			anchors = append(anchors, child)
		}
	}
	return anchors
}

func chartName(anchor *etree.Element) string {
	graphicFrame := firstDescendant(anchor, "graphicFrame")
	if graphicFrame == nil {
		return ""
	}
	for _, cNvPr := range descendants(graphicFrame, "cNvPr") {
		if name := strings.TrimSpace(cNvPr.SelectAttrValue("name", "")); name != "" {
			return name
		}
	}
	return ""
}

func parseAnchor(anchor *etree.Element) *model.ChartAnchorRef {
	if anchor == nil {
		return nil
	}
	result := &model.ChartAnchorRef{Type: localName(anchor.Tag)}
	for _, child := range anchor.ChildElements() {
		switch localName(child.Tag) {
		case "from":
			result.From = parseMarker(child)
		case "to":
			result.To = parseMarker(child)
		}
	}
	return result
}

func parseMarker(marker *etree.Element) *model.ChartMarkerRef {
	if marker == nil {
		return nil
	}
	return &model.ChartMarkerRef{
		Column:       parseChildInt(marker, "col"),
		ColumnOffset: parseChildInt(marker, "colOff"),
		Row:          parseChildInt(marker, "row"),
		RowOffset:    parseChildInt(marker, "rowOff"),
	}
}

func chartTitle(root *etree.Element) string {
	title := firstDescendant(root, "title")
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
		if value := firstDescendant(title, "v"); value != nil {
			return strings.TrimSpace(value.Text())
		}
	}
	return strings.TrimSpace(strings.Join(parts, ""))
}

func chartTypes(root *etree.Element) []string {
	plotArea := firstDescendant(root, "plotArea")
	if plotArea == nil {
		return nil
	}
	var result []string
	seen := map[string]struct{}{}
	for _, child := range plotArea.ChildElements() {
		name := localName(child.Tag)
		if !strings.HasSuffix(name, "Chart") || child.NamespaceURI() != nsChart {
			continue
		}
		if _, ok := seen[name]; ok {
			continue
		}
		seen[name] = struct{}{}
		result = append(result, name)
	}
	return result
}

func chartSeries(root *etree.Element) []model.ChartSeriesRef {
	var series []model.ChartSeriesRef
	for _, ser := range walkSeries(root) {
		item := model.ChartSeriesRef{
			Number: len(series) + 1,
			Index:  parseIdxVal(firstDirectChild(ser, "idx")),
			Order:  parseIdxVal(firstDirectChild(ser, "order")),
		}
		item.Name = chartDataSource(firstDirectChild(ser, "tx"))
		item.Categories = chartDataSource(firstDirectChild(ser, "cat"))
		item.Values = chartDataSource(firstDirectChild(ser, "val"))
		item.XValues = chartDataSource(firstDirectChild(ser, "xVal"))
		item.YValues = chartDataSource(firstDirectChild(ser, "yVal"))
		item.BubbleSize = chartDataSource(firstDirectChild(ser, "bubbleSize"))
		series = append(series, item)
	}
	return series
}

func walkSeries(root *etree.Element) []*etree.Element {
	plotArea := firstDescendant(root, "plotArea")
	if plotArea == nil {
		return nil
	}
	var series []*etree.Element
	for _, chartType := range plotArea.ChildElements() {
		if !strings.HasSuffix(localName(chartType.Tag), "Chart") || chartType.NamespaceURI() != nsChart {
			continue
		}
		series = append(series, directChildren(chartType, "ser")...)
	}
	return series
}

func chartDataSource(elem *etree.Element) *model.ChartDataSourceRef {
	if elem == nil {
		return nil
	}
	var source *etree.Element
	for _, local := range []string{"strRef", "numRef", "multiLvlStrRef"} {
		if found := firstDescendant(elem, local); found != nil {
			source = found
			break
		}
	}
	if source == nil {
		if value := firstDirectChild(elem, "v"); value != nil {
			return &model.ChartDataSourceRef{CacheType: "literal", CachePreview: []string{value.Text()}}
		}
		return nil
	}

	result := &model.ChartDataSourceRef{RefKind: localName(source.Tag)}
	if formula := firstDirectChild(source, "f"); formula != nil {
		result.Formula = strings.TrimSpace(formula.Text())
		result.Sheet, result.Range = splitSheetRangeFormula(result.Formula)
	}
	if cache := firstCacheChild(source); cache != nil {
		result.CacheType = localName(cache.Tag)
		if ptCount := firstDirectChild(cache, "ptCount"); ptCount != nil {
			result.PointCount = parseAttrInt(ptCount, "val")
		}
		for _, pt := range descendants(cache, "pt") {
			if len(result.CachePreview) >= 5 {
				break
			}
			if value := firstDirectChild(pt, "v"); value != nil {
				result.CachePreview = append(result.CachePreview, value.Text())
			}
		}
	}
	if result.Formula == "" && result.PointCount == 0 && len(result.CachePreview) == 0 {
		return nil
	}
	return result
}

func splitSheetRangeFormula(formula string) (string, string) {
	formula = strings.TrimSpace(strings.TrimPrefix(formula, "="))
	if formula == "" {
		return "", ""
	}
	bang := -1
	inQuote := false
	for i := 0; i < len(formula); i++ {
		switch formula[i] {
		case '\'':
			if inQuote && i+1 < len(formula) && formula[i+1] == '\'' {
				i++
				continue
			}
			inQuote = !inQuote
		case '!':
			if !inQuote {
				bang = i
			}
		}
	}
	if bang < 0 {
		return "", ""
	}
	sheet := formula[:bang]
	ref := formula[bang+1:]
	if strings.HasPrefix(sheet, "'") && strings.HasSuffix(sheet, "'") && len(sheet) >= 2 {
		sheet = strings.ReplaceAll(sheet[1:len(sheet)-1], "''", "'")
	}
	if strings.ContainsAny(sheet, "[]") || strings.ContainsAny(ref, "[],") {
		return "", ""
	}
	normalized, err := address.NormalizeRange(ref)
	if err != nil {
		return "", ""
	}
	return sheet, normalized
}

func firstCacheChild(elem *etree.Element) *etree.Element {
	for _, child := range elem.ChildElements() {
		switch localName(child.Tag) {
		case "strCache", "numCache", "multiLvlStrCache":
			return child
		}
	}
	return nil
}

func mapRelationships(rels []opc.RelationshipInfo) map[string]opc.RelationshipInfo {
	result := make(map[string]opc.RelationshipInfo, len(rels))
	for _, rel := range rels {
		if rel.ID == "" {
			continue
		}
		result[rel.ID] = rel
	}
	return result
}

func resolveTargetURI(sourceURI, target string) string {
	if strings.HasPrefix(target, "/") {
		return opc.NormalizeURI(target)
	}
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, target))
}

func firstDirectChild(elem *etree.Element, local string) *etree.Element {
	if elem == nil {
		return nil
	}
	for _, child := range elem.ChildElements() {
		if localName(child.Tag) == local {
			return child
		}
	}
	return nil
}

func directChildren(elem *etree.Element, local string) []*etree.Element {
	if elem == nil {
		return nil
	}
	var result []*etree.Element
	for _, child := range elem.ChildElements() {
		if localName(child.Tag) == local {
			result = append(result, child)
		}
	}
	return result
}

func firstDescendant(elem *etree.Element, local string) *etree.Element {
	if elem == nil {
		return nil
	}
	if localName(elem.Tag) == local {
		return elem
	}
	for _, child := range elem.ChildElements() {
		if found := firstDescendant(child, local); found != nil {
			return found
		}
	}
	return nil
}

func descendants(elem *etree.Element, local string) []*etree.Element {
	if elem == nil {
		return nil
	}
	var result []*etree.Element
	var walk func(*etree.Element)
	walk = func(current *etree.Element) {
		if localName(current.Tag) == local {
			result = append(result, current)
		}
		for _, child := range current.ChildElements() {
			walk(child)
		}
	}
	walk(elem)
	return result
}

func isLocal(elem *etree.Element, local string) bool {
	return elem != nil && localName(elem.Tag) == local
}

func attrNS(elem *etree.Element, ns, local string) (string, bool) {
	if elem == nil {
		return "", false
	}
	if value, ok := namespaces.Attr(elem, ns, local); ok {
		return value, true
	}
	return "", false
}

func parseChildInt(elem *etree.Element, local string) int {
	child := firstDirectChild(elem, local)
	if child == nil {
		return 0
	}
	value, _ := strconv.Atoi(strings.TrimSpace(child.Text()))
	return value
}

func parseAttrInt(elem *etree.Element, attr string) int {
	if elem == nil {
		return 0
	}
	value, _ := strconv.Atoi(strings.TrimSpace(elem.SelectAttrValue(attr, "")))
	return value
}

func parseIdxVal(elem *etree.Element) int {
	return parseAttrInt(elem, "val")
}

func localName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}
