package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxnamespaces "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	xlsxnamespaces "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
)

// relChart is the slide -> chart relationship type.
const relChart = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart"

// relPackage is the chart -> embedded workbook relationship type.
const relPackage = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package"

// embeddedWorkbookContentType is the content type for an embedded .xlsx workbook.
const embeddedWorkbookContentType = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"

// CreateSlideChartRequest authors a new chart on a slide from a source matrix.
//
// The chart part XML (categories/series/caches) is built by the shared
// pkg/xlsx/mutate.BuildChartPart authoring core; this wrapper adds the slide
// graphicFrame, the slide->chart relationship, and (optionally) an embedded
// workbook so the chart's data is editable later.
type CreateSlideChartRequest struct {
	Package   opc.PackageSession
	SlideRef  *inspect.SlideRef
	ChartType string
	Title     string

	// SourceSheet/SourceRange describe the c:f formula refs embedded in the
	// chart caches. SourceCells is the source matrix.
	SourceSheet string
	SourceRange address.RangeRef
	SourceCells [][]rangeio.Cell

	// Geometry in EMUs.
	X  int64
	Y  int64
	CX int64
	CY int64

	// EmbeddedWorkbook, when non-empty, is added as an embedded .xlsx part and
	// linked from the chart so the data stays editable.
	EmbeddedWorkbook []byte
}

// CreateSlideChartResult reports the authored slide chart.
type CreateSlideChartResult struct {
	ChartURI                string
	ChartRelationshipID     string
	ShapeID                 int
	ShapeName               string
	ChartType               string
	Title                   string
	SeriesCount             int
	Categories              int
	EmbeddedWorkbookPartURI string
	Warnings                []string
}

// CreateSlideChart builds a chart part, wires it to the slide through a
// graphicFrame + relationship, and optionally embeds a source workbook.
func CreateSlideChart(req *CreateSlideChartRequest) (*CreateSlideChartResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference cannot be nil")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("chart dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	chartXML, seriesCount, categories, warnings, err := mutate.BuildChartPart(req.ChartType, req.Title, req.SourceSheet, req.SourceRange, req.SourceCells)
	if err != nil {
		return nil, err
	}

	// Read the slide and locate its shape tree before mutating the package, so a
	// malformed slide fails before any parts are added.
	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}
	spTree := slideDoc.FindElement(".//p:spTree")
	if spTree == nil {
		spTree = slideDoc.FindElement(".//spTree")
	}
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in slide")
	}

	chartURI := allocatePPTXNumberedPart(req.Package, "/ppt/charts/chart", ".xml")
	if err := req.Package.AddPart(chartURI, chartXML, xlsxnamespaces.ContentTypeChart, nil); err != nil {
		return nil, fmt.Errorf("failed to add chart part: %w", err)
	}

	// Optional embedded workbook + chart->package relationship + externalData.
	embeddedURI := ""
	if len(req.EmbeddedWorkbook) > 0 {
		embeddedURI = allocatePPTXNumberedPart(req.Package, "/ppt/embeddings/Microsoft_Excel_Sheet", ".xlsx")
		if err := req.Package.AddPart(embeddedURI, req.EmbeddedWorkbook, embeddedWorkbookContentType, nil); err != nil {
			return nil, fmt.Errorf("failed to add embedded workbook part: %w", err)
		}
		chartRels := req.Package.ListRelationships(chartURI)
		pkgRID := AllocateRelationshipID(chartRels)
		target, err := relationshipTarget(chartURI, embeddedURI)
		if err != nil {
			return nil, fmt.Errorf("failed to relativize embedded workbook target: %w", err)
		}
		chartRels = append(chartRels, opc.RelationshipInfo{
			SourceURI: chartURI, ID: pkgRID, Type: relPackage, Target: target,
		})
		if err := writeRelationships(req.Package, chartURI, chartRels); err != nil {
			return nil, err
		}
		if err := addChartExternalData(req.Package, chartURI, pkgRID); err != nil {
			return nil, err
		}
	}

	// slide -> chart relationship.
	slideRels := req.Package.ListRelationships(req.SlideRef.PartURI)
	chartRID := AllocateRelationshipID(slideRels)
	chartTarget, err := relationshipTarget(req.SlideRef.PartURI, chartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to relativize chart target: %w", err)
	}
	slideRels = append(slideRels, opc.RelationshipInfo{
		SourceURI: req.SlideRef.PartURI, ID: chartRID, Type: relChart, Target: chartTarget,
	})
	if err := writeRelationships(req.Package, req.SlideRef.PartURI, slideRels); err != nil {
		return nil, err
	}

	// graphicFrame in spTree.
	shapeID := nextSpTreeShapeID(spTree)
	shapeName := fmt.Sprintf("Chart %d", shapeID)
	frame := buildChartGraphicFrame(shapeID, shapeName, chartRID, req.X, req.Y, req.CX, req.CY)
	appendSpTreeChild(spTree, frame)
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &CreateSlideChartResult{
		ChartURI:                chartURI,
		ChartRelationshipID:     chartRID,
		ShapeID:                 shapeID,
		ShapeName:               shapeName,
		ChartType:               req.ChartType,
		Title:                   req.Title,
		SeriesCount:             seriesCount,
		Categories:              categories,
		EmbeddedWorkbookPartURI: embeddedURI,
		Warnings:                warnings,
	}, nil
}

// nextSpTreeShapeID returns the smallest shape id greater than every existing
// cNvPr id anywhere in the shape tree (sp, pic, graphicFrame, grpSp, etc.), so
// the new graphicFrame's id cannot collide with an existing shape.
func nextSpTreeShapeID(spTree *etree.Element) int {
	maxID := 0
	seen := map[*etree.Element]bool{}
	for _, cNvPr := range append(spTree.FindElements(".//p:cNvPr"), spTree.FindElements(".//cNvPr")...) {
		if seen[cNvPr] {
			continue
		}
		seen[cNvPr] = true
		if id, err := strconv.Atoi(strings.TrimSpace(cNvPr.SelectAttrValue("id", ""))); err == nil && id > maxID {
			maxID = id
		}
	}
	return maxID + 1
}

// buildChartGraphicFrame constructs the p:graphicFrame element wiring a chart
// into a slide. Child order follows the PresentationML schema:
// nvGraphicFramePr (cNvPr, cNvGraphicFramePr, nvPr) -> p:xfrm -> a:graphic.
func buildChartGraphicFrame(shapeID int, shapeName, chartRID string, x, y, cx, cy int64) *etree.Element {
	frame := etree.NewElement("p:graphicFrame")

	nv := etree.NewElement("p:nvGraphicFramePr")
	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", strconv.Itoa(shapeID))
	cNvPr.CreateAttr("name", shapeName)
	nv.AddChild(cNvPr)
	nv.AddChild(etree.NewElement("p:cNvGraphicFramePr"))
	nv.AddChild(etree.NewElement("p:nvPr"))
	frame.AddChild(nv)

	xfrm := etree.NewElement("p:xfrm")
	off := etree.NewElement("a:off")
	off.CreateAttr("x", strconv.FormatInt(x, 10))
	off.CreateAttr("y", strconv.FormatInt(y, 10))
	xfrm.AddChild(off)
	ext := etree.NewElement("a:ext")
	ext.CreateAttr("cx", strconv.FormatInt(cx, 10))
	ext.CreateAttr("cy", strconv.FormatInt(cy, 10))
	xfrm.AddChild(ext)
	frame.AddChild(xfrm)

	graphic := etree.NewElement("a:graphic")
	graphicData := etree.NewElement("a:graphicData")
	graphicData.CreateAttr("uri", xlsxnamespaces.NsChart)
	chart := etree.NewElement("c:chart")
	// Slides do not declare the chart namespace, so it must be declared here.
	chart.CreateAttr("xmlns:c", xlsxnamespaces.NsChart)
	chart.CreateAttr("xmlns:r", pptxnamespaces.NsR)
	chart.CreateAttr("r:id", chartRID)
	graphicData.AddChild(chart)
	graphic.AddChild(graphicData)
	frame.AddChild(graphic)

	return frame
}

// addChartExternalData inserts <c:externalData r:id=".."> as the last child of
// c:chartSpace (after c:chart), declaring xmlns:r locally if necessary.
func addChartExternalData(session opc.PackageSession, chartURI, rid string) error {
	doc, err := session.ReadXMLPart(chartURI)
	if err != nil {
		return fmt.Errorf("failed to read chart part %s: %w", chartURI, err)
	}
	root := doc.Root()
	if root == nil {
		return fmt.Errorf("chart part %s root element not found", chartURI)
	}
	if root.SelectAttr("xmlns:r") == nil {
		root.CreateAttr("xmlns:r", pptxnamespaces.NsR)
	}
	external := etree.NewElement("c:externalData")
	external.CreateAttr("r:id", rid)
	autoUpdate := etree.NewElement("c:autoUpdate")
	autoUpdate.CreateAttr("val", "0")
	external.AddChild(autoUpdate)
	root.AddChild(external)
	doc.IndentTabs()
	if err := session.ReplaceXMLPart(chartURI, doc); err != nil {
		return fmt.Errorf("failed to replace chart part %s: %w", chartURI, err)
	}
	return nil
}

// writeRelationships persists the relationships for a source part.
func writeRelationships(session opc.PackageSession, sourceURI string, rels []opc.RelationshipInfo) error {
	relsXML, err := BuildRelationshipsXML(rels)
	if err != nil {
		return fmt.Errorf("failed to build relationships XML for %s: %w", sourceURI, err)
	}
	relsURI := opc.GetDirectory(sourceURI) + "/_rels/" + opc.GetFileName(sourceURI) + ".rels"
	contentType := session.GetContentType(relsURI)
	if contentType == "" {
		contentType = "application/vnd.openxmlformats-package.relationships+xml"
	}
	if err := session.ReplaceRawPart(relsURI, relsXML, contentType); err != nil {
		return fmt.Errorf("failed to write relationships for %s: %w", sourceURI, err)
	}
	return nil
}

// allocatePPTXNumberedPart returns the next free /prefixN.suffix part URI.
func allocatePPTXNumberedPart(session opc.PackageSession, prefix, suffix string) string {
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
