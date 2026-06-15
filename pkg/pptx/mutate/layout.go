package mutate

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// PlaceholderType represents the type of placeholder being created
type PlaceholderType string

const (
	PlaceholderTypeTitle    PlaceholderType = "title"
	PlaceholderTypeSubtitle PlaceholderType = "subTitle"
	PlaceholderTypeBody     PlaceholderType = "body"
	PlaceholderTypePicture  PlaceholderType = "pic"
)

// AddTextPlaceholderRequest holds parameters for adding a text placeholder to a layout
type AddTextPlaceholderRequest struct {
	// Package session
	Package opc.PackageSession

	// Layout part URI
	LayoutPartURI string

	// Placeholder type (e.g., "body", "title")
	PlaceholderType PlaceholderType

	// Position and size in EMUs
	X  int64 // Position X
	Y  int64 // Position Y
	CX int64 // Width
	CY int64 // Height

	// Optional: placeholder size enum (e.g., "full", "half")
	Size string

	// Optional: placeholder orientation
	Orient string

	// Optional: explicit idx value.
	Idx int

	// ExplicitIdx distinguishes an intentional idx=0 from the zero-value default.
	// When false, the placeholder index is auto-allocated.
	ExplicitIdx bool
}

// AddTextPlaceholderResult holds the result of adding a text placeholder
type AddTextPlaceholderResult struct {
	ShapeID   int
	ShapeName string
	Idx       int
}

// AddPicturePlaceholderRequest holds parameters for adding a picture placeholder to a layout
type AddPicturePlaceholderRequest struct {
	// Package session
	Package opc.PackageSession

	// Layout part URI
	LayoutPartURI string

	// Position and size in EMUs
	X  int64 // Position X
	Y  int64 // Position Y
	CX int64 // Width
	CY int64 // Height

	// Optional: placeholder size enum
	Size string

	// Optional: placeholder orientation
	Orient string

	// Optional: explicit idx value.
	Idx int

	// ExplicitIdx distinguishes an intentional idx=0 from the zero-value default.
	// When false, the placeholder index is auto-allocated.
	ExplicitIdx bool
}

// AddPicturePlaceholderResult holds the result of adding a picture placeholder
type AddPicturePlaceholderResult struct {
	ShapeID   int
	ShapeName string
	Idx       int
}

// AddTextPlaceholder adds a text placeholder to a layout.
// It creates a p:sp (shape) element with:
// - valid p:ph semantics (type, idx)
// - geometry (xfrm with position and size)
// - normalized naming
// - default text body with proper structure
func AddTextPlaceholder(req *AddTextPlaceholderRequest) (*AddTextPlaceholderResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("placeholder dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	// Read the layout XML
	layoutDoc, err := req.Package.ReadXMLPart(req.LayoutPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read layout: %w", err)
	}

	// Get the shape tree
	spTree := layoutDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in layout")
	}

	// Allocate new shape ID across every existing cNvPr in the tree.
	newShapeID := nextSpTreeShapeID(spTree)

	// Allocate placeholder idx if not specified.
	phIdx := req.Idx
	if !req.ExplicitIdx || phIdx < 0 {
		phIdx = allocateNextPlaceholderIndex(spTree)
	}

	// Create shape name based on placeholder type
	shapeName := generatePlaceholderName(req.PlaceholderType, phIdx)

	// Create the shape element
	shape := createTextPlaceholderElement(newShapeID, shapeName, req.PlaceholderType, phIdx, req.Size, req.Orient, req.X, req.Y, req.CX, req.CY)

	// Add to shape tree
	appendSpTreeChild(spTree, shape)

	// Write the layout back
	if err := req.Package.ReplaceXMLPart(req.LayoutPartURI, layoutDoc); err != nil {
		return nil, fmt.Errorf("failed to write layout: %w", err)
	}

	return &AddTextPlaceholderResult{
		ShapeID:   newShapeID,
		ShapeName: shapeName,
		Idx:       phIdx,
	}, nil
}

// AddPicturePlaceholder adds a picture placeholder to a layout.
// It creates a p:sp (shape) element with:
// - valid p:ph semantics (type="pic", idx)
// - geometry (xfrm with position and size)
// - normalized naming
// - default picture body structure
func AddPicturePlaceholder(req *AddPicturePlaceholderRequest) (*AddPicturePlaceholderResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("placeholder dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	// Read the layout XML
	layoutDoc, err := req.Package.ReadXMLPart(req.LayoutPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read layout: %w", err)
	}

	// Get the shape tree
	spTree := layoutDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in layout")
	}

	// Allocate new shape ID across every existing cNvPr in the tree.
	newShapeID := nextSpTreeShapeID(spTree)

	// Allocate placeholder idx if not specified.
	phIdx := req.Idx
	if !req.ExplicitIdx || phIdx < 0 {
		phIdx = allocateNextPlaceholderIndex(spTree)
	}

	// Create shape name
	shapeName := fmt.Sprintf("Picture Placeholder %d", phIdx)

	// Create the shape element
	shape := createPicturePlaceholderElement(newShapeID, shapeName, phIdx, req.Size, req.Orient, req.X, req.Y, req.CX, req.CY)

	// Add to shape tree
	appendSpTreeChild(spTree, shape)

	// Write the layout back
	if err := req.Package.ReplaceXMLPart(req.LayoutPartURI, layoutDoc); err != nil {
		return nil, fmt.Errorf("failed to write layout: %w", err)
	}

	return &AddPicturePlaceholderResult{
		ShapeID:   newShapeID,
		ShapeName: shapeName,
		Idx:       phIdx,
	}, nil
}

// allocateNextPlaceholderIndex finds the next available placeholder index
func allocateNextPlaceholderIndex(spTree *etree.Element) int {
	maxIdx := -1

	// Check all shapes for placeholder elements
	for _, sp := range spTree.FindElements("//sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr == nil {
			continue
		}
		nvPr := nvSpPr.FindElement("nvPr")
		if nvPr == nil {
			continue
		}
		phElem := nvPr.FindElement("ph")
		if phElem == nil {
			continue
		}

		idxStr := phElem.SelectAttrValue("idx", "")
		if idxStr != "" {
			var idx int
			if _, err := fmt.Sscanf(idxStr, "%d", &idx); err == nil && idx > maxIdx {
				maxIdx = idx
			}
		}
	}

	return maxIdx + 1
}

// generatePlaceholderName generates a normalized shape name for a placeholder
func generatePlaceholderName(phType PlaceholderType, idx int) string {
	switch phType {
	case PlaceholderTypeTitle:
		return "Title 1"
	case PlaceholderTypeSubtitle:
		return fmt.Sprintf("Subtitle %d", idx)
	case PlaceholderTypeBody:
		return fmt.Sprintf("Content Placeholder %d", idx)
	case PlaceholderTypePicture:
		return fmt.Sprintf("Picture Placeholder %d", idx)
	default:
		return fmt.Sprintf("Placeholder %d", idx)
	}
}

// createTextPlaceholderElement creates a p:sp element for a text placeholder
func createTextPlaceholderElement(
	shapeID int,
	shapeName string,
	phType PlaceholderType,
	phIdx int,
	size string,
	orient string,
	x, y, cx, cy int64,
) *etree.Element {
	sp := etree.NewElement("p:sp")

	// Non-visual properties
	nvSpPr := etree.NewElement("p:nvSpPr")

	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", fmt.Sprintf("%d", shapeID))
	cNvPr.CreateAttr("name", shapeName)
	nvSpPr.AddChild(cNvPr)

	cNvSpPr := etree.NewElement("p:cNvSpPr")
	nvSpPr.AddChild(cNvSpPr)

	nvPr := etree.NewElement("p:nvPr")
	ph := etree.NewElement("p:ph")
	ph.CreateAttr("type", string(phType))
	ph.CreateAttr("idx", fmt.Sprintf("%d", phIdx))
	if size != "" {
		ph.CreateAttr("sz", size)
	}
	if orient != "" {
		ph.CreateAttr("orient", orient)
	}
	nvPr.AddChild(ph)
	nvSpPr.AddChild(nvPr)

	sp.AddChild(nvSpPr)

	// Shape properties (geometry)
	spPr := etree.NewElement("p:spPr")

	xfrm := etree.NewElement("a:xfrm")
	off := etree.NewElement("a:off")
	off.CreateAttr("x", fmt.Sprintf("%d", x))
	off.CreateAttr("y", fmt.Sprintf("%d", y))
	xfrm.AddChild(off)

	ext := etree.NewElement("a:ext")
	ext.CreateAttr("cx", fmt.Sprintf("%d", cx))
	ext.CreateAttr("cy", fmt.Sprintf("%d", cy))
	xfrm.AddChild(ext)

	spPr.AddChild(xfrm)

	// Add preset geometry
	prstGeom := etree.NewElement("a:prstGeom")
	prstGeom.CreateAttr("prst", "rect")
	prstGeom.AddChild(etree.NewElement("a:avLst"))
	spPr.AddChild(prstGeom)

	sp.AddChild(spPr)

	// Text body with default empty paragraph
	txBody := etree.NewElement("p:txBody")

	// Body properties
	bodyPr := etree.NewElement("a:bodyPr")
	bodyPr.CreateAttr("rtlCol", "0")
	txBody.AddChild(bodyPr)

	// List style
	lstStyle := etree.NewElement("a:lstStyle")
	txBody.AddChild(lstStyle)

	// Add default empty paragraph
	p := etree.NewElement("a:p")
	pPr := etree.NewElement("a:pPr")
	pPr.CreateAttr("lvl", "0")
	p.AddChild(pPr)

	endParaRPr := etree.NewElement("a:endParaRPr")
	endParaRPr.CreateAttr("lang", "en-US")
	endParaRPr.CreateAttr("sz", "1800")
	endParaRPr.CreateAttr("dirty", "0")
	p.AddChild(endParaRPr)

	txBody.AddChild(p)

	sp.AddChild(txBody)

	return sp
}

// createPicturePlaceholderElement creates a p:sp element for a picture placeholder
func createPicturePlaceholderElement(
	shapeID int,
	shapeName string,
	phIdx int,
	size string,
	orient string,
	x, y, cx, cy int64,
) *etree.Element {
	sp := etree.NewElement("p:sp")

	// Non-visual properties
	nvSpPr := etree.NewElement("p:nvSpPr")

	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", fmt.Sprintf("%d", shapeID))
	cNvPr.CreateAttr("name", shapeName)
	nvSpPr.AddChild(cNvPr)

	cNvSpPr := etree.NewElement("p:cNvSpPr")
	nvSpPr.AddChild(cNvSpPr)

	nvPr := etree.NewElement("p:nvPr")
	ph := etree.NewElement("p:ph")
	ph.CreateAttr("type", "pic")
	ph.CreateAttr("idx", fmt.Sprintf("%d", phIdx))
	if size != "" {
		ph.CreateAttr("sz", size)
	}
	if orient != "" {
		ph.CreateAttr("orient", orient)
	}
	nvPr.AddChild(ph)
	nvSpPr.AddChild(nvPr)

	sp.AddChild(nvSpPr)

	// Shape properties (geometry)
	spPr := etree.NewElement("p:spPr")

	xfrm := etree.NewElement("a:xfrm")
	off := etree.NewElement("a:off")
	off.CreateAttr("x", fmt.Sprintf("%d", x))
	off.CreateAttr("y", fmt.Sprintf("%d", y))
	xfrm.AddChild(off)

	ext := etree.NewElement("a:ext")
	ext.CreateAttr("cx", fmt.Sprintf("%d", cx))
	ext.CreateAttr("cy", fmt.Sprintf("%d", cy))
	xfrm.AddChild(ext)

	spPr.AddChild(xfrm)

	// Add preset geometry
	prstGeom := etree.NewElement("a:prstGeom")
	prstGeom.CreateAttr("prst", "rect")
	prstGeom.AddChild(etree.NewElement("a:avLst"))
	spPr.AddChild(prstGeom)

	sp.AddChild(spPr)

	return sp
}
