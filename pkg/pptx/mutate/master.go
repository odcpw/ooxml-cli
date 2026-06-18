package mutate

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// AddTextPlaceholderToMasterRequest holds parameters for adding a text placeholder to a master
type AddTextPlaceholderToMasterRequest struct {
	// Package session
	Package opc.PackageSession

	// Master part URI
	MasterPartURI string

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

// AddTextPlaceholderToMasterResult holds the result of adding a text placeholder to a master
type AddTextPlaceholderToMasterResult struct {
	ShapeID   int
	ShapeName string
	Idx       int
}

// AddPicturePlaceholderToMasterRequest holds parameters for adding a picture placeholder to a master
type AddPicturePlaceholderToMasterRequest struct {
	// Package session
	Package opc.PackageSession

	// Master part URI
	MasterPartURI string

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

// AddPicturePlaceholderToMasterResult holds the result of adding a picture placeholder to a master
type AddPicturePlaceholderToMasterResult struct {
	ShapeID   int
	ShapeName string
	Idx       int
}

// DefaultTextStyleInfo holds default text style information for a master
type DefaultTextStyleInfo struct {
	// Style type (e.g., "title", "body", "other")
	StyleType string

	// Font properties
	FontSize  int    // in hundredths of a point (e.g., 4400 = 44pt)
	FontName  string // e.g., "+mj-lt" (Latin), "+mn-lt" (Minor)
	Alignment string // e.g., "l", "ctr", "r"
	Bold      bool
	Italic    bool
	Underline bool
	Color     string // hex color code or scheme color

	// Paragraph properties
	LeftMargin  int64  // in EMUs
	IndentLevel int32  // 0-8
	BulletChar  string // e.g., "•", "-"
	SpaceBefore int64  // in EMUs
	SpaceAfter  int64  // in EMUs
	LineSpacing int64  // in EMUs
}

// AddTextPlaceholderToMaster adds a text placeholder to a master.
// It creates a p:sp (shape) element with:
// - valid p:ph semantics (type, idx)
// - geometry (xfrm with position and size)
// - normalized naming
// - default text body with proper structure
func AddTextPlaceholderToMaster(req *AddTextPlaceholderToMasterRequest) (*AddTextPlaceholderToMasterResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.MasterPartURI == "" {
		return nil, fmt.Errorf("master part URI cannot be empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("placeholder dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	// Read the master XML
	masterDoc, err := req.Package.ReadXMLPart(req.MasterPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read master: %w", err)
	}

	// Get the shape tree
	spTree := masterDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in master")
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

	// Write the master back
	if err := req.Package.ReplaceXMLPart(req.MasterPartURI, masterDoc); err != nil {
		return nil, fmt.Errorf("failed to write master: %w", err)
	}

	return &AddTextPlaceholderToMasterResult{
		ShapeID:   newShapeID,
		ShapeName: shapeName,
		Idx:       phIdx,
	}, nil
}

// AddPicturePlaceholderToMaster adds a picture placeholder to a master.
// It creates a p:sp (shape) element with:
// - valid p:ph semantics (type="pic", idx)
// - geometry (xfrm with position and size)
// - normalized naming
// - default picture body structure
func AddPicturePlaceholderToMaster(req *AddPicturePlaceholderToMasterRequest) (*AddPicturePlaceholderToMasterResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.MasterPartURI == "" {
		return nil, fmt.Errorf("master part URI cannot be empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("placeholder dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	// Read the master XML
	masterDoc, err := req.Package.ReadXMLPart(req.MasterPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read master: %w", err)
	}

	// Get the shape tree
	spTree := masterDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in master")
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

	// Write the master back
	if err := req.Package.ReplaceXMLPart(req.MasterPartURI, masterDoc); err != nil {
		return nil, fmt.Errorf("failed to write master: %w", err)
	}

	return &AddPicturePlaceholderToMasterResult{
		ShapeID:   newShapeID,
		ShapeName: shapeName,
		Idx:       phIdx,
	}, nil
}

// UpdateMasterDefaultTextStyleRequest holds parameters for updating default text styles on a master
type UpdateMasterDefaultTextStyleRequest struct {
	// Package session
	Package opc.PackageSession

	// Master part URI
	MasterPartURI string

	// Style type (e.g., "title", "body")
	StyleType string

	// Text style info with desired updates
	Style *DefaultTextStyleInfo
}

// UpdateMasterDefaultTextStyle updates the default text style for a specific type on a master
// (e.g., title, body, other). This allows layouts and slides to inherit these default styles.
func UpdateMasterDefaultTextStyle(req *UpdateMasterDefaultTextStyleRequest) error {
	if req == nil {
		return fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return fmt.Errorf("package session cannot be nil")
	}
	if req.MasterPartURI == "" {
		return fmt.Errorf("master part URI cannot be empty")
	}
	if req.StyleType == "" {
		return fmt.Errorf("style type cannot be empty")
	}
	if req.Style == nil {
		return fmt.Errorf("style cannot be nil")
	}

	// Read the master XML
	masterDoc, err := req.Package.ReadXMLPart(req.MasterPartURI)
	if err != nil {
		return fmt.Errorf("failed to read master: %w", err)
	}

	// Find or create txStyles element
	txStyles := masterDoc.FindElement(".//txStyles")
	if txStyles == nil {
		root := masterDoc.Root()
		if root == nil {
			return fmt.Errorf("master has no root element")
		}
		txStyles = insertSlideMasterChild(root, "txStyles")
	}

	// Find or create the appropriate style element
	var styleElem *etree.Element
	switch req.StyleType {
	case "title":
		styleElem = txStyles.FindElement("titleStyle")
		if styleElem == nil {
			styleElem = etree.NewElement("p:titleStyle")
			txStyles.AddChild(styleElem)
		}
	case "body":
		styleElem = txStyles.FindElement("bodyStyle")
		if styleElem == nil {
			styleElem = etree.NewElement("p:bodyStyle")
			txStyles.AddChild(styleElem)
		}
	case "other":
		styleElem = txStyles.FindElement("otherStyle")
		if styleElem == nil {
			styleElem = etree.NewElement("p:otherStyle")
			txStyles.AddChild(styleElem)
		}
	default:
		return fmt.Errorf("unsupported style type: %s", req.StyleType)
	}

	lvlPPr := findLevelParagraphProperties(styleElem, 1)
	if lvlPPr == nil {
		lvlPPr := createLevelParagraphProperties(1, req.Style)
		styleElem.AddChild(lvlPPr)
	} else {
		patchLevelParagraphProperties(lvlPPr, req.Style)
	}

	// Write the master back
	if err := req.Package.ReplaceXMLPart(req.MasterPartURI, masterDoc); err != nil {
		return fmt.Errorf("failed to write master: %w", err)
	}

	return nil
}

func findLevelParagraphProperties(styleElem *etree.Element, level int32) *etree.Element {
	if styleElem == nil {
		return nil
	}
	want := fmt.Sprintf("lvl%dpPr", level)
	for _, child := range styleElem.ChildElements() {
		if localTag(child.Tag) == want {
			return child
		}
	}
	return nil
}

func patchLevelParagraphProperties(lvlPPr *etree.Element, style *DefaultTextStyleInfo) {
	if lvlPPr == nil || style == nil {
		return
	}
	if style.Alignment != "" {
		lvlPPr.CreateAttr("algn", style.Alignment)
	}
	if style.SpaceBefore > 0 {
		replaceSpacingPct(lvlPPr, "spcBef", style.SpaceBefore)
	}
	if style.SpaceAfter > 0 {
		replaceSpacingPct(lvlPPr, "spcAft", style.SpaceAfter)
	}
	defRPr := findDirectChildByLocal(lvlPPr, "defRPr")
	if defRPr == nil {
		defRPr = etree.NewElement("a:defRPr")
		lvlPPr.AddChild(defRPr)
	}
	if style.FontSize > 0 {
		defRPr.CreateAttr("sz", fmt.Sprintf("%d", style.FontSize))
	}
	if style.Color != "" {
		setDefaultRunSolidFill(defRPr, style.Color)
	}
	if style.FontName != "" {
		setTypeface(defRPr, "latin", style.FontName)
	}
}

func replaceSpacingPct(lvlPPr *etree.Element, name string, emu int64) {
	if lvlPPr == nil {
		return
	}
	for _, child := range lvlPPr.ChildElements() {
		if localTag(child.Tag) == name {
			lvlPPr.RemoveChild(child)
		}
	}
	spacing := etree.NewElement("a:" + name)
	spcPct := etree.NewElement("a:spcPct")
	pct := (emu * 100000) / 914400
	if pct > 100000 {
		pct = 100000
	}
	spcPct.CreateAttr("val", fmt.Sprintf("%d", pct))
	spacing.AddChild(spcPct)
	lvlPPr.AddChild(spacing)
}

func setDefaultRunSolidFill(defRPr *etree.Element, color string) {
	if defRPr == nil {
		return
	}
	for _, child := range defRPr.ChildElements() {
		switch localTag(child.Tag) {
		case "noFill", "solidFill", "gradFill", "blipFill", "pattFill", "grpFill":
			defRPr.RemoveChild(child)
		}
	}
	solidFill := etree.NewElement("a:solidFill")
	if isValidHexColor(color) {
		srgbClr := etree.NewElement("a:srgbClr")
		srgbClr.CreateAttr("val", color)
		solidFill.AddChild(srgbClr)
	} else {
		schemeClr := etree.NewElement("a:schemeClr")
		schemeClr.CreateAttr("val", color)
		solidFill.AddChild(schemeClr)
	}
	defRPr.AddChild(solidFill)
}

func setTypeface(defRPr *etree.Element, local, typeface string) {
	if defRPr == nil || local == "" {
		return
	}
	font := findDirectChildByLocal(defRPr, local)
	if font == nil {
		font = etree.NewElement("a:" + local)
		defRPr.AddChild(font)
	}
	font.CreateAttr("typeface", typeface)
}

func findDirectChildByLocal(elem *etree.Element, local string) *etree.Element {
	if elem == nil {
		return nil
	}
	for _, child := range elem.ChildElements() {
		if localTag(child.Tag) == local {
			return child
		}
	}
	return nil
}

// createLevelParagraphProperties creates a level paragraph properties element for a given level
func createLevelParagraphProperties(level int32, style *DefaultTextStyleInfo) *etree.Element {
	lvlPPr := etree.NewElement("a:lvl" + fmt.Sprintf("%dpPr", level))

	// Add alignment if specified
	if style.Alignment != "" {
		lvlPPr.CreateAttr("algn", style.Alignment)
	}

	// Add other attributes
	lvlPPr.CreateAttr("defTabSz", "457200")
	lvlPPr.CreateAttr("rtl", "0")
	lvlPPr.CreateAttr("eaLnBrk", "1")
	lvlPPr.CreateAttr("latinLnBrk", "0")
	lvlPPr.CreateAttr("hangingPunct", "1")

	// Add spacing before if specified
	if style.SpaceBefore > 0 {
		spcBef := etree.NewElement("a:spcBef")
		spcPct := etree.NewElement("a:spcPct")
		// Convert EMU to percentage (20000 is 20%)
		pct := (style.SpaceBefore * 100000) / 914400
		if pct > 100000 {
			pct = 100000 // Cap at 100%
		}
		spcPct.CreateAttr("val", fmt.Sprintf("%d", pct))
		spcBef.AddChild(spcPct)
		lvlPPr.AddChild(spcBef)
	}

	// Add spacing after if specified
	if style.SpaceAfter > 0 {
		spcAft := etree.NewElement("a:spcAft")
		spcPct := etree.NewElement("a:spcPct")
		// Convert EMU to percentage
		pct := (style.SpaceAfter * 100000) / 914400
		if pct > 100000 {
			pct = 100000
		}
		spcPct.CreateAttr("val", fmt.Sprintf("%d", pct))
		spcAft.AddChild(spcPct)
		lvlPPr.AddChild(spcAft)
	}

	// Add default run properties
	defRPr := etree.NewElement("a:defRPr")
	if style.FontSize > 0 {
		defRPr.CreateAttr("sz", fmt.Sprintf("%d", style.FontSize))
	}
	defRPr.CreateAttr("kern", "1200")

	// Add color if specified
	if style.Color != "" {
		solidFill := etree.NewElement("a:solidFill")
		if isValidHexColor(style.Color) {
			srgbClr := etree.NewElement("a:srgbClr")
			srgbClr.CreateAttr("val", style.Color)
			solidFill.AddChild(srgbClr)
		} else {
			schemeClr := etree.NewElement("a:schemeClr")
			schemeClr.CreateAttr("val", style.Color)
			solidFill.AddChild(schemeClr)
		}
		defRPr.AddChild(solidFill)
	}

	// Add font if specified
	if style.FontName != "" {
		latin := etree.NewElement("a:latin")
		latin.CreateAttr("typeface", style.FontName)
		defRPr.AddChild(latin)

		ea := etree.NewElement("a:ea")
		ea.CreateAttr("typeface", style.FontName)
		defRPr.AddChild(ea)

		cs := etree.NewElement("a:cs")
		cs.CreateAttr("typeface", style.FontName)
		defRPr.AddChild(cs)
	}

	lvlPPr.AddChild(defRPr)

	return lvlPPr
}
