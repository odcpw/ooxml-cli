package cli

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// MasterInfo represents metadata about a slide master
type MasterInfo struct {
	Index        int                     // 1-based index
	PartURI      string                  // e.g., "/ppt/slideMasters/slideMaster1.xml"
	LayoutCount  int                     // Number of layouts linked to this master
	Layouts      []string                // URIs of linked layouts
	ThemeURI     string                  // e.g., "/ppt/theme/theme1.xml"
	Shapes       int                     // Total number of shapes in the master
	Placeholders []model.PlaceholderInfo // Placeholders defined in the master
}

// ParsePresentationMasters extracts all slide masters from a presentation
func ParsePresentationMasters(pkg opc.PackageSession) ([]*MasterInfo, error) {
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	masters := make([]*MasterInfo, len(graph.Masters))
	for idx, masterRef := range graph.Masters {
		// Read master XML to extract placeholders
		placeholders, err := parseMasterPlaceholders(pkg, masterRef.PartURI)
		if err != nil {
			placeholders = []model.PlaceholderInfo{}
		}

		masters[idx] = &MasterInfo{
			Index:        idx + 1,
			PartURI:      masterRef.PartURI,
			LayoutCount:  len(masterRef.LinkedLayoutURIs),
			Layouts:      masterRef.LinkedLayoutURIs,
			ThemeURI:     masterRef.ThemeURI,
			Placeholders: placeholders,
		}
	}

	return masters, nil
}

// GetMasterByIndex finds a master by its 1-based index
func GetMasterByIndex(masters []*MasterInfo, index int) *MasterInfo {
	if index > 0 && index <= len(masters) {
		return masters[index-1]
	}
	return nil
}

// CountShapesInMaster counts the total number of shapes in a master XML
func CountShapesInMaster(pkg opc.PackageSession, masterPartURI string) (int, error) {
	masterXML, err := pkg.ReadXMLPart(masterPartURI)
	if err != nil {
		return 0, fmt.Errorf("failed to read master %s: %w", masterPartURI, err)
	}

	// Count different shape element types
	root := masterXML.Root()
	if root == nil {
		return 0, fmt.Errorf("master XML root element not found")
	}

	// Use simple counting for shape elements (we don't have access to the shape tree parsing here)
	xmlStr, _ := masterXML.WriteToString()
	count := 0
	count += countOccurrences(xmlStr, "<p:sp")
	count += countOccurrences(xmlStr, "<p:pic")
	count += countOccurrences(xmlStr, "<p:graphicFrame")
	count += countOccurrences(xmlStr, "<p:grpSp")

	return count, nil
}

// countOccurrences counts the number of occurrences of a substring
func countOccurrences(str, substr string) int {
	count := 0
	pos := 0
	for {
		idx := find(str, substr, pos)
		if idx == -1 {
			break
		}
		count++
		pos = idx + len(substr)
	}
	return count
}

// find finds a substring starting from a position (simple replacement for strings.Index)
func find(str, substr string, start int) int {
	if start >= len(str) {
		return -1
	}
	idx := indexOf(str[start:], substr)
	if idx == -1 {
		return -1
	}
	return start + idx
}

// indexOf returns the index of the first occurrence of substr in str, or -1 if not found
func indexOf(str, substr string) int {
	if len(substr) == 0 {
		return 0
	}
	if len(substr) > len(str) {
		return -1
	}

	for i := 0; i <= len(str)-len(substr); i++ {
		match := true
		for j := 0; j < len(substr); j++ {
			if str[i+j] != substr[j] {
				match = false
				break
			}
		}
		if match {
			return i
		}
	}
	return -1
}

// parseMasterPlaceholders extracts placeholder information from a master XML
func parseMasterPlaceholders(pkg opc.PackageSession, masterURI string) ([]model.PlaceholderInfo, error) {
	var placeholders []model.PlaceholderInfo

	masterXML, err := pkg.ReadXMLPart(masterURI)
	if err != nil {
		return placeholders, fmt.Errorf("failed to read master %s: %w", masterURI, err)
	}

	root := masterXML.Root()
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
		ph := extractMasterPlaceholderInfo(shape)
		if ph != nil {
			placeholders = append(placeholders, *ph)
		}
	}

	// Process group shapes
	grpShapes := namespaces.FindChildren(spTree, namespaces.NsP, "grpSp")
	for _, grpShape := range grpShapes {
		grpChildren := namespaces.FindChildren(grpShape, namespaces.NsP, "sp")
		for _, shape := range grpChildren {
			ph := extractMasterPlaceholderInfo(shape)
			if ph != nil {
				placeholders = append(placeholders, *ph)
			}
		}
	}

	return placeholders, nil
}

// extractMasterPlaceholderInfo extracts placeholder info from a master shape element
func extractMasterPlaceholderInfo(shape *etree.Element) *model.PlaceholderInfo {
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
	key := generateMasterPlaceholderKey(phType, phIdx, shapeID)

	// Extract geometry from shape properties
	var geometry *model.Geometry
	spPr := namespaces.FindChild(shape, namespaces.NsP, "spPr")
	if spPr != nil {
		xfrm := namespaces.FindChild(spPr, namespaces.NsA, "xfrm")
		if xfrm != nil {
			geometry = extractMasterPlaceholderGeometry(xfrm, spPr)
		}
	}

	return &model.PlaceholderInfo{
		Key:       key,
		Role:      canonicalMasterRole(phType),
		Index:     parseMasterStringIndex(phIdx),
		ShapeName: shapeName,
		Geometry:  geometry,
	}
}

// extractMasterPlaceholderGeometry extracts geometry information from a placeholder's transform element
func extractMasterPlaceholderGeometry(xfrm *etree.Element, spPr *etree.Element) *model.Geometry {
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

// canonicalMasterRole maps PPTX placeholder types to canonical role names for masters
func canonicalMasterRole(phType string) string {
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

// generateMasterPlaceholderKey generates a stable key for a master placeholder
func generateMasterPlaceholderKey(role, idx, shapeID string) string {
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

// parseMasterStringIndex converts a string index to int, returning 0 if invalid
func parseMasterStringIndex(idxStr string) int {
	if idxStr == "" {
		return 0
	}
	idx, err := strconv.Atoi(idxStr)
	if err != nil {
		return 0
	}
	return idx
}
