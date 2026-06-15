// Package chart discovers existing PPTX chart parts and their embedded workbooks.
package chart

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxmodel "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

const (
	RelChart   = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart"
	RelPackage = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package"
)

type ChartRef struct {
	Number                         int         `json:"number"`
	Slide                          int         `json:"slide"`
	SlidePartURI                   string      `json:"slidePartUri"`
	ShapeID                        string      `json:"shapeId,omitempty"`
	ShapeName                      string      `json:"shapeName,omitempty"`
	RelationshipID                 string      `json:"relationshipId,omitempty"`
	PartURI                        string      `json:"partUri"`
	Title                          string      `json:"title,omitempty"`
	Types                          []string    `json:"types,omitempty"`
	Series                         []SeriesRef `json:"series,omitempty"`
	EmbeddedWorkbookPartURI        string      `json:"embeddedWorkbookPartUri,omitempty"`
	EmbeddedWorkbookRelationshipID string      `json:"embeddedWorkbookRelationshipId,omitempty"`
	PrimarySelector                string      `json:"primarySelector,omitempty"`
	Selectors                      []string    `json:"selectors,omitempty"`
}

type SeriesRef struct {
	Number          int                           `json:"number"`
	Index           int                           `json:"index,omitempty"`
	Order           int                           `json:"order,omitempty"`
	Name            *xlsxmodel.ChartDataSourceRef `json:"name,omitempty"`
	Categories      *xlsxmodel.ChartDataSourceRef `json:"categories,omitempty"`
	Values          *xlsxmodel.ChartDataSourceRef `json:"values,omitempty"`
	XValues         *xlsxmodel.ChartDataSourceRef `json:"xValues,omitempty"`
	YValues         *xlsxmodel.ChartDataSourceRef `json:"yValues,omitempty"`
	BubbleSize      *xlsxmodel.ChartDataSourceRef `json:"bubbleSize,omitempty"`
	PrimarySelector string                        `json:"primarySelector,omitempty"`
	Selectors       []string                      `json:"selectors,omitempty"`
}

func List(session opc.PackageSession, slideNumber int) ([]ChartRef, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	graph, err := pptxinspect.ParsePresentation(session)
	if err != nil {
		return nil, err
	}
	var charts []ChartRef
	for _, slide := range graph.Slides {
		if slideNumber > 0 && slide.SlideNumber != slideNumber {
			continue
		}
		slideCharts, err := listForSlide(session, slide, len(charts)+1)
		if err != nil {
			return nil, err
		}
		charts = append(charts, slideCharts...)
	}
	return charts, nil
}

func WithSelectors(chart ChartRef) ChartRef {
	builder := selectorBuilder{}
	if chart.Number > 0 {
		chart.PrimarySelector = fmt.Sprintf("chart:%d", chart.Number)
	}
	builder.add(chart.PrimarySelector)
	if chart.Number > 0 {
		builder.add(fmt.Sprintf("chart:%d", chart.Number))
		builder.add(fmt.Sprintf("#%d", chart.Number))
	}
	if chart.Slide > 0 {
		builder.add(fmt.Sprintf("slide:%d/chart:%d", chart.Slide, chart.Number))
	}
	if chart.ShapeID != "" {
		builder.add("shape:" + chart.ShapeID)
		builder.add("id:" + chart.ShapeID)
	}
	if chart.ShapeName != "" {
		builder.add("shape:" + chart.ShapeName)
		builder.add("name:" + chart.ShapeName)
		builder.add("~" + chart.ShapeName)
		builder.add(chart.ShapeName)
	}
	if chart.RelationshipID != "" {
		builder.add("rid:" + chart.RelationshipID)
		builder.add("rId:" + chart.RelationshipID)
	}
	if chart.PartURI != "" {
		builder.add("part:" + chart.PartURI)
	}
	chart.Selectors = builder.values
	for idx := range chart.Series {
		chart.Series[idx] = withSeriesSelectors(chart.Series[idx])
	}
	return chart
}

func listForSlide(session opc.PackageSession, slide pptxinspect.SlideRef, startNumber int) ([]ChartRef, error) {
	doc, err := session.ReadXMLPart(slide.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide %s: %w", slide.PartURI, err)
	}
	root := doc.Root()
	if root == nil {
		return nil, fmt.Errorf("slide part %s root element not found", slide.PartURI)
	}
	relMap := relationshipMap(session.ListRelationships(slide.PartURI))
	var charts []ChartRef
	for _, frame := range graphicFrames(root) {
		chartElem := chartElement(frame)
		if chartElem == nil {
			continue
		}
		rid := relationshipID(chartElem)
		if rid == "" {
			return nil, fmt.Errorf("slide %s chart graphicFrame is missing r:id", slide.PartURI)
		}
		rel, ok := relMap[rid]
		if !ok {
			return nil, fmt.Errorf("slide %s chart relationship %s not found", slide.PartURI, rid)
		}
		if rel.TargetMode == "External" {
			return nil, fmt.Errorf("slide %s chart relationship %s is external", slide.PartURI, rid)
		}
		if rel.Type != RelChart {
			return nil, fmt.Errorf("slide %s relationship %s is %s, expected chart", slide.PartURI, rid, rel.Type)
		}
		chartURI := resolveTargetURI(slide.PartURI, rel.Target)
		chartPart, err := xlsxchart.ReadChartPart(session, chartURI)
		if err != nil {
			return nil, err
		}
		embeddedURI, embeddedRID := embeddedWorkbook(session, chartURI)
		shapeID, shapeName := graphicFrameIDName(frame)
		chart := ChartRef{
			Number:                         startNumber + len(charts),
			Slide:                          slide.SlideNumber,
			SlidePartURI:                   slide.PartURI,
			ShapeID:                        shapeID,
			ShapeName:                      shapeName,
			RelationshipID:                 rid,
			PartURI:                        chartURI,
			Title:                          chartPart.Title,
			Types:                          chartPart.Types,
			Series:                         convertSeries(chartPart.Series),
			EmbeddedWorkbookPartURI:        embeddedURI,
			EmbeddedWorkbookRelationshipID: embeddedRID,
		}
		charts = append(charts, WithSelectors(chart))
	}
	return charts, nil
}

func convertSeries(series []xlsxmodel.ChartSeriesRef) []SeriesRef {
	result := make([]SeriesRef, 0, len(series))
	for _, item := range series {
		result = append(result, withSeriesSelectors(SeriesRef{
			Number:     item.Number,
			Index:      item.Index,
			Order:      item.Order,
			Name:       item.Name,
			Categories: item.Categories,
			Values:     item.Values,
			XValues:    item.XValues,
			YValues:    item.YValues,
			BubbleSize: item.BubbleSize,
		}))
	}
	return result
}

func withSeriesSelectors(series SeriesRef) SeriesRef {
	builder := selectorBuilder{}
	if series.Number > 0 {
		series.PrimarySelector = fmt.Sprintf("series:%d", series.Number)
	}
	builder.add(series.PrimarySelector)
	if series.Number > 0 {
		builder.add(fmt.Sprintf("series:%d", series.Number))
		builder.add(fmt.Sprintf("#%d", series.Number))
	}
	if name := seriesDisplayName(series); name != "" {
		builder.add("name:" + name)
		builder.add("~" + name)
		builder.add(name)
	}
	series.Selectors = builder.values
	return series
}

func seriesDisplayName(series SeriesRef) string {
	if series.Name == nil {
		return ""
	}
	if len(series.Name.CachePreview) > 0 {
		return strings.TrimSpace(series.Name.CachePreview[0])
	}
	return strings.TrimSpace(series.Name.Formula)
}

func embeddedWorkbook(session opc.PackageSession, chartURI string) (string, string) {
	doc, err := session.ReadXMLPart(chartURI)
	if err != nil || doc.Root() == nil {
		return "", ""
	}
	externalRID := ""
	if externalData := firstDescendant(doc.Root(), "externalData"); externalData != nil {
		externalRID = relationshipID(externalData)
	}
	for _, rel := range session.ListRelationships(chartURI) {
		if rel.TargetMode == "External" || rel.Type != RelPackage {
			continue
		}
		if externalRID != "" && rel.ID != externalRID {
			continue
		}
		return resolveTargetURI(chartURI, rel.Target), rel.ID
	}
	if externalRID == "" {
		for _, rel := range session.ListRelationships(chartURI) {
			if rel.TargetMode == "External" || rel.Type != RelPackage {
				continue
			}
			return resolveTargetURI(chartURI, rel.Target), rel.ID
		}
	}
	return "", ""
}

func relationshipID(elem *etree.Element) string {
	if elem == nil {
		return ""
	}
	if rid, ok := namespaces.Attr(elem, namespaces.NsR, "id"); ok && strings.TrimSpace(rid) != "" {
		return strings.TrimSpace(rid)
	}
	for _, attr := range elem.Attr {
		if attr.Key == "id" && (attr.Space == "r" || attr.Space == namespaces.NsR) {
			return strings.TrimSpace(attr.Value)
		}
	}
	return strings.TrimSpace(elem.SelectAttrValue("r:id", ""))
}

func relationshipMap(rels []opc.RelationshipInfo) map[string]opc.RelationshipInfo {
	result := map[string]opc.RelationshipInfo{}
	for _, rel := range rels {
		if rel.ID != "" {
			result[rel.ID] = rel
		}
	}
	return result
}

func resolveTargetURI(sourceURI, target string) string {
	if strings.HasPrefix(target, "/") {
		return opc.NormalizeURI(target)
	}
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, target))
}

func graphicFrames(root *etree.Element) []*etree.Element {
	var result []*etree.Element
	var walk func(*etree.Element)
	walk = func(elem *etree.Element) {
		if elem == nil {
			return
		}
		if localName(elem.Tag) == "graphicFrame" {
			result = append(result, elem)
		}
		for _, child := range elem.ChildElements() {
			walk(child)
		}
	}
	walk(root)
	return result
}

func chartElement(frame *etree.Element) *etree.Element {
	for _, elem := range descendants(frame, "chart") {
		if elem.NamespaceURI() == namespaces.NsC {
			return elem
		}
	}
	return nil
}

func graphicFrameIDName(frame *etree.Element) (string, string) {
	for _, elem := range descendants(frame, "cNvPr") {
		id := strings.TrimSpace(elem.SelectAttrValue("id", ""))
		name := strings.TrimSpace(elem.SelectAttrValue("name", ""))
		if id != "" || name != "" {
			return id, name
		}
	}
	return "", ""
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

func localName(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}

type selectorBuilder struct {
	values []string
	seen   map[string]bool
}

func (b *selectorBuilder) add(value string) {
	value = strings.TrimSpace(value)
	if value == "" {
		return
	}
	if b.seen == nil {
		b.seen = map[string]bool{}
	}
	key := strings.ToLower(value)
	if b.seen[key] {
		return
	}
	b.seen[key] = true
	b.values = append(b.values, value)
}

func Select(charts []ChartRef, selector string) (ChartRef, error) {
	if len(charts) == 0 {
		return ChartRef{}, fmt.Errorf("presentation has no charts")
	}
	selector = strings.TrimSpace(selector)
	if selector == "" {
		if len(charts) == 1 {
			return WithSelectors(charts[0]), nil
		}
		return ChartRef{}, fmt.Errorf("chart selector is required when presentation has multiple charts")
	}
	var matches []ChartRef
	for _, chart := range charts {
		withSelectors := WithSelectors(chart)
		for _, candidate := range withSelectors.Selectors {
			if strings.EqualFold(candidate, selector) {
				matches = append(matches, withSelectors)
				break
			}
		}
	}
	if len(matches) == 1 {
		return matches[0], nil
	}
	if len(matches) > 1 {
		var selectors []string
		for _, match := range matches {
			selectors = append(selectors, match.PrimarySelector)
		}
		sort.Strings(selectors)
		return ChartRef{}, fmt.Errorf("chart selector %q matched multiple charts (%s); use a more specific selector", selector, strings.Join(selectors, ", "))
	}
	if number, err := strconv.Atoi(selector); err == nil {
		if number >= 1 && number <= len(charts) {
			return WithSelectors(charts[number-1]), nil
		}
		return ChartRef{}, fmt.Errorf("chart %d is out of range (1-%d)", number, len(charts))
	}
	return ChartRef{}, fmt.Errorf("chart not found: %s", selector)
}
