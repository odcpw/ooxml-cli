package normalize

import (
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// ParsePlaceholder extracts placeholder attributes from a p:sp element.
// Returns nil if the element is not a placeholder shape.
func ParsePlaceholder(shape *etree.Element) *model.RawPlaceholder {
	if shape == nil {
		return nil
	}

	// Navigate to p:nvSpPr/p:nvPr/p:ph
	nvSpPr := namespaces.FindChild(shape, namespaces.NsP, "nvSpPr")
	if nvSpPr == nil {
		return nil
	}

	nvPr := namespaces.FindChild(nvSpPr, namespaces.NsP, "nvPr")
	if nvPr == nil {
		return nil
	}

	phElem := namespaces.FindChild(nvPr, namespaces.NsP, "ph")
	if phElem == nil {
		return nil
	}

	// Extract attributes from p:ph
	phType := phElem.SelectAttrValue("type", "")
	idxStr := phElem.SelectAttrValue("idx", "")
	sz := phElem.SelectAttrValue("sz", "")
	orient := phElem.SelectAttrValue("orient", "")

	// Parse idx as integer (0-based index)
	idx := -1
	if idxStr != "" {
		if parsedIdx, err := strconv.Atoi(idxStr); err == nil {
			idx = parsedIdx
		}
	}

	return &model.RawPlaceholder{
		Type:   phType,
		Idx:    idx,
		Sz:     sz,
		Orient: orient,
	}
}

// ExtractPlaceholdersFromShapes extracts all placeholders from a list of shape elements.
// Returns a list of shape elements that are placeholders.
func ExtractPlaceholdersFromShapes(shapes []*etree.Element) []*etree.Element {
	var placeholderShapes []*etree.Element

	for _, shape := range shapes {
		if ParsePlaceholder(shape) != nil {
			placeholderShapes = append(placeholderShapes, shape)
		}
	}

	return placeholderShapes
}
