package normalize

import (
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// LayoutContext is defined in keys.go
// It provides context-dependent information about a layout.

// ResolvePlaceholder resolves a slide-level placeholder by applying inheritance rules.
//
// Inheritance chain:
// 1. Use slide placeholder type if present
// 2. If slide type is missing, look for layout placeholder with same idx and use its type
// 3. If still missing, look for master placeholder with same idx and use its type
// 4. Return resolved placeholder with final type (may still be empty if no match found)
func ResolvePlaceholder(
	slideRaw *model.RawPlaceholder,
	layoutPlaceholders []*model.RawPlaceholder,
	masterPlaceholders []*model.RawPlaceholder,
) *model.ResolvedPlaceholder {
	if slideRaw == nil {
		return nil
	}

	resolved := &model.ResolvedPlaceholder{
		Raw: *slideRaw,
	}

	// Start with the slide-level type
	resolvedType := slideRaw.Type

	// If slide type is missing and we have an idx, try to inherit from layout/master
	if resolvedType == "" && slideRaw.Idx >= 0 {
		// Look for layout placeholder with matching idx
		for _, layoutPh := range layoutPlaceholders {
			if layoutPh.Idx == slideRaw.Idx && layoutPh.Type != "" {
				resolvedType = layoutPh.Type
				break
			}
		}

		// If still missing, look for master placeholder with matching idx
		if resolvedType == "" {
			for _, masterPh := range masterPlaceholders {
				if masterPh.Idx == slideRaw.Idx && masterPh.Type != "" {
					resolvedType = masterPh.Type
					break
				}
			}
		}
	}

	resolved.Raw.Type = resolvedType
	return resolved
}

// NormalizePlaceholdersRequest holds the input data for the full normalization pipeline.
type NormalizePlaceholdersRequest struct {
	// SlideShapes are the shape elements from the slide
	SlideShapes []*etree.Element
	// LayoutShapes are the shape elements from the layout
	LayoutShapes []*etree.Element
	// MasterShapes are the shape elements from the master
	MasterShapes []*etree.Element
	// LayoutContext provides information about the layout
	LayoutContext LayoutContext
}

// NormalizePlaceholders implements the full normalization pipeline:
// 1. Parse placeholders from slide/layout/master XML
// 2. Resolve types through inheritance
// 3. Generate keys using resolved types
// 4. Return PlaceholderInfo for each slide placeholder
//
// The pipeline uses roles.go:CanonicalRole() and keys.go:GenerateKey()
// from task-4 to map types to roles and generate keys.
func NormalizePlaceholders(req *NormalizePlaceholdersRequest) []model.PlaceholderInfo {
	if req == nil {
		return []model.PlaceholderInfo{}
	}

	// Parse placeholders from each level
	layoutPlaceholders := parseShapesForPlaceholders(req.LayoutShapes)
	masterPlaceholders := parseShapesForPlaceholders(req.MasterShapes)

	var results []model.PlaceholderInfo

	// For each slide placeholder, resolve and generate key
	for i, slideShape := range req.SlideShapes {
		slidePh := ParsePlaceholder(slideShape)
		if slidePh == nil {
			continue
		}

		// Resolve the placeholder through inheritance chain
		resolved := ResolvePlaceholder(slidePh, layoutPlaceholders, masterPlaceholders)
		if resolved == nil {
			continue
		}

		// Extract shape ID and name for metadata
		shapeID, shapeName := extractShapeMetadata(slideShape)
		resolved.ShapeID = shapeID
		resolved.ShapeName = shapeName

		// Map to canonical role
		role := CanonicalRole(resolved.Raw.Type)
		resolved.Role = role

		// Generate key using the resolved placeholder and layout context
		key := GenerateKey(*resolved, req.LayoutContext)

		info := model.PlaceholderInfo{
			Key:          key,
			Role:         role,
			Index:        slidePh.Idx,
			ShapeName:    shapeName,
			LiteralType:  slidePh.Type,      // Original slide-level type
			ResolvedType: resolved.Raw.Type, // Final resolved type
		}

		results = append(results, info)

		_ = i // unused for now
	}

	return results
}

// parseShapesForPlaceholders extracts RawPlaceholder from each shape element.
func parseShapesForPlaceholders(shapes []*etree.Element) []*model.RawPlaceholder {
	var result []*model.RawPlaceholder

	for _, shape := range shapes {
		ph := ParsePlaceholder(shape)
		if ph != nil {
			result = append(result, ph)
		}
	}

	return result
}

// extractShapeMetadata extracts ID and name from a shape element.
func extractShapeMetadata(shape *etree.Element) (int, string) {
	if shape == nil {
		return 0, ""
	}

	id := 0
	name := ""

	// Navigate to p:nvSpPr/p:cNvPr using namespaces helper
	nvSpPr := shape.FindElement("nvSpPr")
	if nvSpPr == nil {
		nvSpPr = shape.FindElement("{http://schemas.openxmlformats.org/presentationml/2006/main}nvSpPr")
	}
	if nvSpPr != nil {
		cNvPr := nvSpPr.FindElement("cNvPr")
		if cNvPr == nil {
			cNvPr = nvSpPr.FindElement("{http://schemas.openxmlformats.org/presentationml/2006/main}cNvPr")
		}
		if cNvPr != nil {
			// Extract id attribute
			if idStr := cNvPr.SelectAttrValue("id", ""); idStr != "" {
				if parsedID, err := strconv.Atoi(idStr); err == nil {
					id = parsedID
				}
			}
			// Extract name attribute
			name = cNvPr.SelectAttrValue("name", "")
		}
	}

	return id, name
}
