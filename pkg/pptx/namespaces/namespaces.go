// Package namespaces provides PPTX-specific XML namespace constants and traversal helpers.
package namespaces

import (
	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
)

// PPTX namespace URI constants
const (
	// NsP is the PresentationML namespace for PPTX main schema
	NsP = "http://schemas.openxmlformats.org/presentationml/2006/main"

	// NsA is the DrawingML namespace for shapes, text, and drawing elements
	NsA = "http://schemas.openxmlformats.org/drawingml/2006/main"

	// NsR is the Office Open XML relationships namespace
	NsR = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"

	// NsMC is the markup compatibility namespace for office documents
	NsMC = "http://schemas.openxmlformats.org/markup-compatibility/2006"

	// NsC is the DrawingML chart namespace
	NsC = "http://schemas.openxmlformats.org/drawingml/2006/chart"

	// NsDgm is the DrawingML diagram (SmartArt) namespace
	NsDgm = "http://schemas.openxmlformats.org/drawingml/2006/diagram"

	// Np14 is the Microsoft PowerPoint 2010 extension namespace (p14), used for
	// the p14:media embedded-media reference inside a p:ext extension element.
	Np14 = "http://schemas.microsoft.com/office/powerpoint/2010/main"
)

// FindChild finds a direct child element with the given namespace and local name.
// Returns nil if not found. Wraps xmlx.FindChild for consistency.
func FindChild(elem *etree.Element, ns, localName string) *etree.Element {
	return xmlx.FindChild(elem, ns, localName)
}

// FindChildren finds all direct child elements with the given namespace and local name.
// Wraps xmlx.FindChildren for consistency.
func FindChildren(elem *etree.Element, ns, localName string) []*etree.Element {
	return xmlx.FindChildren(elem, ns, localName)
}

// FindDescendants recursively finds all descendant elements with the given namespace and local name.
// Wraps xmlx.FindDescendants for consistency.
func FindDescendants(elem *etree.Element, ns, localName string) []*etree.Element {
	return xmlx.FindDescendants(elem, ns, localName)
}

// Attr gets the value of an attribute by namespace and local name.
// Returns the attribute value and true if found, empty string and false otherwise.
// Wraps xmlx.GetAttrNS for convenience.
func Attr(elem *etree.Element, ns, localName string) (string, bool) {
	return xmlx.GetAttrNS(elem, ns, localName)
}

// IsElement reports whether elem is in the given namespace with the given local name.
// Wraps xmlx.ElementMatches for consistency.
func IsElement(elem *etree.Element, ns, localName string) bool {
	return xmlx.ElementMatches(elem, ns, localName)
}

// HasChild checks if an element has a direct child with the given namespace and local name.
// Returns true if such a child exists, false otherwise.
func HasChild(elem *etree.Element, ns, localName string) bool {
	return xmlx.FindChild(elem, ns, localName) != nil
}
