package cli

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/normalize"
)

// LayoutInfo represents metadata about a slide layout
type LayoutInfo struct {
	ID           string // e.g., "layout-1"
	Name         string
	PartURI      string
	MasterID     string
	ThemeURI     string // Theme URI from the master
	Preserve     bool
	UserDrawn    bool
	Placeholders []model.PlaceholderInfo
}

// ParsePresentationLayouts extracts all slide layouts from a presentation
func ParsePresentationLayouts(pkg opc.PackageSession) ([]*LayoutInfo, error) {
	var layouts []*LayoutInfo

	// Use the inspect package's ParsePresentation function to get the layout graph
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation graph: %w", err)
	}

	// Build master URI to ID mapping and master URI to theme URI mapping
	masterURIToID := make(map[string]string)
	masterURIToTheme := make(map[string]string)
	for i, master := range graph.Masters {
		masterURIToID[master.PartURI] = fmt.Sprintf("master-%d", i+1)
		masterURIToTheme[master.PartURI] = master.ThemeURI
	}

	// Process each layout
	for i, layoutRef := range graph.Layouts {
		// Read layout XML to extract properties
		layoutXML, err := pkg.ReadXMLPart(layoutRef.PartURI)
		if err != nil {
			// Skip layouts that can't be parsed
			continue
		}

		root := layoutXML.Root()

		// Extract preserve and userDrawn flags from root element
		preserve := root.SelectAttrValue("preserve", "")
		userDrawn := root.SelectAttrValue("userDrawn", "")

		// Parse placeholders
		placeholders, err := parseLayoutPlaceholders(pkg, layoutXML, layoutRef.PartURI)
		if err != nil {
			placeholders = []model.PlaceholderInfo{}
		}

		layout := &LayoutInfo{
			ID:           fmt.Sprintf("layout-%d", i+1),
			Name:         layoutRef.Name, // Use the name from the inspect package
			PartURI:      layoutRef.PartURI,
			Preserve:     preserve == "1" || preserve == "true",
			UserDrawn:    userDrawn == "1" || userDrawn == "true",
			Placeholders: placeholders,
		}

		// Set master ID and theme URI from the mapping
		if masterID, ok := masterURIToID[layoutRef.MasterPartURI]; ok {
			layout.MasterID = masterID
		}
		if themeURI, ok := masterURIToTheme[layoutRef.MasterPartURI]; ok {
			layout.ThemeURI = themeURI
		}

		layouts = append(layouts, layout)
	}

	return layouts, nil
}

// parseLayoutPlaceholders extracts placeholder information from a layout XML
// Handles missing optional elements and non-placeholder text boxes by falling back to shape:<id> selectors
func parseLayoutPlaceholders(pkg opc.PackageSession, layoutXML *etree.Document, layoutURI string) ([]model.PlaceholderInfo, error) {
	var placeholders []model.PlaceholderInfo

	root := layoutXML.Root()
	cSld := namespaces.FindChild(root, namespaces.NsP, "cSld")
	if cSld == nil {
		return placeholders, nil
	}

	spTree := namespaces.FindChild(cSld, namespaces.NsP, "spTree")
	if spTree == nil {
		return placeholders, nil
	}

	// Process all shapes in the shape tree
	shapes := namespaces.FindChildren(spTree, namespaces.NsP, "sp")
	for _, shape := range shapes {
		ph := extractPlaceholderInfo(shape)
		if ph != nil {
			placeholders = append(placeholders, *ph)
		}
	}

	// Process group shapes
	grpShapes := namespaces.FindChildren(spTree, namespaces.NsP, "grpSp")
	for _, grpShape := range grpShapes {
		grpChildren := namespaces.FindChildren(grpShape, namespaces.NsP, "sp")
		for _, shape := range grpChildren {
			ph := extractPlaceholderInfo(shape)
			if ph != nil {
				placeholders = append(placeholders, *ph)
			}
		}
	}

	return placeholders, nil
}

// extractPlaceholderInfo extracts placeholder info from a shape element
// Falls back to shape:<id> selector if placeholder metadata is absent
func extractPlaceholderInfo(shape *etree.Element) *model.PlaceholderInfo {
	nvSpPr := namespaces.FindChild(shape, namespaces.NsP, "nvSpPr")
	if nvSpPr == nil {
		return nil
	}

	cNvPr := namespaces.FindChild(nvSpPr, namespaces.NsP, "cNvPr")
	if cNvPr == nil {
		return nil
	}

	shapeID, _ := namespaces.Attr(cNvPr, "", "id")
	shapeName, _ := namespaces.Attr(cNvPr, "", "name")

	// Check for placeholder element
	nvPr := namespaces.FindChild(nvSpPr, namespaces.NsP, "nvPr")
	if nvPr == nil {
		// Not a placeholder shape - return nil to skip it
		return nil
	}

	ph := namespaces.FindChild(nvPr, namespaces.NsP, "ph")
	if ph == nil {
		// Shape has nvPr but no ph element - skip it
		return nil
	}

	// Extract placeholder attributes
	phType, _ := namespaces.Attr(ph, "", "type")
	phIdx, _ := namespaces.Attr(ph, "", "idx")

	// Generate placeholder key
	key := generatePlaceholderKey(phType, phIdx, shapeID)

	// Extract geometry from shape properties
	var geometry *model.Geometry
	spPr := namespaces.FindChild(shape, namespaces.NsP, "spPr")
	if spPr != nil {
		xfrm := namespaces.FindChild(spPr, namespaces.NsA, "xfrm")
		if xfrm != nil {
			geometry = extractPlaceholderGeometry(xfrm, spPr)
		}
	}

	return &model.PlaceholderInfo{
		Key:       key,
		Role:      canonicalRole(phType),
		Index:     parseStringIndex(phIdx),
		ShapeName: shapeName,
		Geometry:  geometry,
	}
}

// extractPlaceholderGeometry extracts geometry information from a placeholder's transform element
func extractPlaceholderGeometry(xfrm *etree.Element, spPr *etree.Element) *model.Geometry {
	if xfrm == nil {
		return nil
	}

	geom := &model.Geometry{}
	hasGeometry := false

	// Parse bounds from a:off and a:ext
	off := namespaces.FindChild(xfrm, namespaces.NsA, "off")
	ext := namespaces.FindChild(xfrm, namespaces.NsA, "ext")

	if off != nil || ext != nil {
		bounds := &model.Bounds{}

		if off != nil {
			if xStr, ok := namespaces.Attr(off, "", "x"); ok {
				if x, err := strconv.ParseInt(xStr, 10, 64); err == nil {
					bounds.X = x
					hasGeometry = true
				}
			}
			if yStr, ok := namespaces.Attr(off, "", "y"); ok {
				if y, err := strconv.ParseInt(yStr, 10, 64); err == nil {
					bounds.Y = y
					hasGeometry = true
				}
			}
		}

		if ext != nil {
			if cxStr, ok := namespaces.Attr(ext, "", "cx"); ok {
				if cx, err := strconv.ParseInt(cxStr, 10, 64); err == nil {
					bounds.CX = cx
					hasGeometry = true
				}
			}
			if cyStr, ok := namespaces.Attr(ext, "", "cy"); ok {
				if cy, err := strconv.ParseInt(cyStr, 10, 64); err == nil {
					bounds.CY = cy
					hasGeometry = true
				}
			}
		}

		if hasGeometry {
			geom.Bounds = bounds
		}
	}

	// Parse rotation from a:xfrm@rot attribute (in 1/60000 of a degree)
	if rotStr, ok := namespaces.Attr(xfrm, "", "rot"); ok {
		if rot, err := strconv.Atoi(rotStr); err == nil && rot != 0 {
			geom.Rotation = rot
			hasGeometry = true
		}
	}

	// Parse flip attributes from a:xfrm
	if flipHStr, ok := namespaces.Attr(xfrm, "", "flipH"); ok {
		if flipHStr == "1" {
			geom.FlipH = true
			hasGeometry = true
		}
	}
	if flipVStr, ok := namespaces.Attr(xfrm, "", "flipV"); ok {
		if flipVStr == "1" {
			geom.FlipV = true
			hasGeometry = true
		}
	}

	// Only return geometry if it has actual values
	if !hasGeometry {
		return nil
	}

	return geom
}

// canonicalRole maps PPTX placeholder types to canonical role names
func canonicalRole(phType string) string {
	switch phType {
	case "title", "ctrTitle":
		return "title"
	case "subTitle":
		return "subtitle"
	case "body":
		return "body"
	case "pic":
		return "pic"
	case "tbl":
		return "table"
	case "chart":
		return "chart"
	case "obj":
		return "object"
	case "dt":
		return "date"
	case "ftr":
		return "footer"
	case "sldNum":
		return "slideNumber"
	default:
		return phType // Preserve unknown types
	}
}

// generatePlaceholderKey generates a stable key for a placeholder
func generatePlaceholderKey(role, idx, shapeID string) string {
	// If no role, fall back to shape ID
	if role == "" {
		if shapeID != "" {
			return "shape:" + shapeID
		}
		return "unknown"
	}

	// If has index, include it for uniqueness
	if idx != "" {
		return role + ":" + idx
	}

	// Just the role
	return role
}

// parseStringIndex converts a string index to int, returning 0 if invalid
func parseStringIndex(idxStr string) int {
	if idxStr == "" {
		return 0
	}
	idx, err := strconv.Atoi(idxStr)
	if err != nil {
		return 0
	}
	return idx
}

// buildLayoutContextFromShapes builds a layout context by counting roles in shapes
func buildLayoutContextFromShapes(shapes []*etree.Element) normalize.LayoutContext {
	roleCounts := make(map[string]int)

	for _, shape := range shapes {
		ph := normalize.ParsePlaceholder(shape)
		if ph != nil && ph.Type != "" {
			role := normalize.CanonicalRole(ph.Type)
			roleCounts[role]++
		}
	}

	return normalize.NewSimpleLayoutContext(roleCounts)
}

// GetLayoutByNumber finds a layout by its 1-based index
func GetLayoutByNumber(layouts []*LayoutInfo, num int) *LayoutInfo {
	if num > 0 && num <= len(layouts) {
		return layouts[num-1]
	}
	return nil
}

// GetLayoutByName finds a layout by its name (case-sensitive)
func GetLayoutByName(layouts []*LayoutInfo, name string) *LayoutInfo {
	for _, layout := range layouts {
		if layout.Name == name {
			return layout
		}
	}
	return nil
}

// FilterLayoutsByMaster filters layouts to those belonging to a specific master
func FilterLayoutsByMaster(layouts []*LayoutInfo, masterID string) []*LayoutInfo {
	var filtered []*LayoutInfo
	for _, layout := range layouts {
		if layout.MasterID == masterID {
			filtered = append(filtered, layout)
		}
	}
	return filtered
}

// FormatPlaceholderKeyList returns a comma-separated list of placeholder keys
func FormatPlaceholderKeyList(placeholders []model.PlaceholderInfo) string {
	if len(placeholders) == 0 {
		return "-"
	}

	keys := make([]string, len(placeholders))
	for i, ph := range placeholders {
		keys[i] = ph.Key
	}
	return strings.Join(keys, ", ")
}
