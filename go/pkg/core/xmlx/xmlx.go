// Package xmlx provides XML namespace-aware helpers using beevik/etree.
package xmlx

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
)

// FindChild finds a direct child element with the given namespace and local name.
// Returns nil if not found.
func FindChild(elem *etree.Element, ns, local string) *etree.Element {
	if elem == nil {
		return nil
	}

	for _, child := range elem.Child {
		if e, ok := child.(*etree.Element); ok {
			if ElementMatches(e, ns, local) {
				return e
			}
		}
	}
	return nil
}

// FindChildren finds all direct child elements with the given namespace and local name.
func FindChildren(elem *etree.Element, ns, local string) []*etree.Element {
	if elem == nil {
		return nil
	}

	var results []*etree.Element
	for _, child := range elem.Child {
		if e, ok := child.(*etree.Element); ok {
			if ElementMatches(e, ns, local) {
				results = append(results, e)
			}
		}
	}
	return results
}

// FindDescendants recursively finds all descendant elements with the given namespace and local name.
func FindDescendants(elem *etree.Element, ns, local string) []*etree.Element {
	if elem == nil {
		return nil
	}

	var results []*etree.Element

	for _, child := range elem.Child {
		if e, ok := child.(*etree.Element); ok {
			if ElementMatches(e, ns, local) {
				results = append(results, e)
			}
			// Recursively search children
			results = append(results, FindDescendants(e, ns, local)...)
		}
	}
	return results
}

// ElementMatches checks if an element matches the given namespace and local name.
func ElementMatches(elem *etree.Element, ns, local string) bool {
	if elem == nil {
		return false
	}

	// Check local name
	if elem.Tag != local {
		// Also check if the tag includes a namespace prefix
		parts := strings.Split(elem.Tag, "}")
		if len(parts) == 2 {
			// Tag is in the form "{namespace}local"
			actualNS := parts[0][1:] // Remove leading '{'
			actualLocal := parts[1]
			return actualNS == ns && actualLocal == local
		}
		return false
	}

	// If no prefix, check that the element's namespace matches
	elemNS := elem.NamespaceURI()
	return elemNS == ns
}

// GetAttr gets the value of an attribute by name.
// Returns the attribute value and true if found, empty string and false otherwise.
func GetAttr(elem *etree.Element, name string) (string, bool) {
	if elem == nil {
		return "", false
	}

	attr := elem.SelectAttr(name)
	if attr != nil {
		return attr.Value, true
	}
	return "", false
}

// GetAttrNS gets the value of an attribute by namespace and local name.
func GetAttrNS(elem *etree.Element, ns, local string) (string, bool) {
	if elem == nil {
		return "", false
	}

	// In etree, attributes with namespaces are typically stored with the prefix:local format
	// or we need to check both the Space and Key
	for _, attr := range elem.Attr {
		// Check if this attribute matches the namespace and local name
		if attr.Key == local && attr.Space == ns {
			return attr.Value, true
		}
	}
	return "", false
}

// SetAttr sets the value of an attribute.
func SetAttr(elem *etree.Element, name string, value string) {
	if elem == nil {
		return
	}
	elem.CreateAttr(name, value)
}

// SetAttrNS sets the value of an attribute with namespace.
func SetAttrNS(elem *etree.Element, ns, local string, value string) {
	if elem == nil {
		return
	}

	// In etree, namespaced attributes use Space and Key
	// First remove existing attribute with this namespace and key if it exists
	for i := 0; i < len(elem.Attr); i++ {
		if elem.Attr[i].Key == local && elem.Attr[i].Space == ns {
			elem.Attr = append(elem.Attr[:i], elem.Attr[i+1:]...)
			i--
		}
	}

	// Add new attribute - create it using the low-level approach
	attr := &etree.Attr{
		Space: ns,
		Key:   local,
		Value: value,
	}
	elem.Attr = append(elem.Attr, *attr)
}

// Attr is a convenience function to create an attribute for use with etree.
func Attr(name, value string) *etree.Attr {
	return &etree.Attr{
		Key:   name,
		Value: value,
	}
}

// AttrNS is a convenience function to create a namespaced attribute for use with etree.
func AttrNS(ns, local, value string) *etree.Attr {
	return &etree.Attr{
		Space: ns,
		Key:   local,
		Value: value,
	}
}

// GetText returns the text content of an element.
func GetText(elem *etree.Element) string {
	if elem == nil {
		return ""
	}
	return elem.Text()
}

// SetText sets the text content of an element.
func SetText(elem *etree.Element, text string) {
	if elem == nil {
		return
	}
	// Clear existing children and set text
	elem.Child = nil
	elem.SetText(text)
}

// AppendChild appends a child element.
func AppendChild(parent, child *etree.Element) *etree.Element {
	if parent == nil || child == nil {
		return child
	}
	parent.AddChild(child)
	return child
}

// CreateChild creates a new child element with the given local name and appends it.
func CreateChild(parent *etree.Element, local string) *etree.Element {
	if parent == nil {
		return nil
	}
	return parent.CreateElement(local)
}

// CreateChildNS creates a new child element with namespace and appends it.
func CreateChildNS(parent *etree.Element, ns, local string) *etree.Element {
	if parent == nil {
		return nil
	}

	// Create element with full tag including namespace
	tag := fmt.Sprintf("{%s}%s", ns, local)
	return parent.CreateElement(tag)
}

// RemoveChild removes a child element.
func RemoveChild(parent, child *etree.Element) {
	if parent == nil || child == nil {
		return
	}
	parent.RemoveChild(child)
}

// GetParent returns the parent of an element.
func GetParent(elem *etree.Element) *etree.Element {
	if elem == nil {
		return nil
	}
	return elem.Parent()
}

// Path returns the path to an element as a slice of element names.
func Path(elem *etree.Element) []string {
	if elem == nil {
		return nil
	}

	var path []string
	current := elem
	for current != nil {
		path = append([]string{current.Tag}, path...)
		current = current.Parent()
	}
	return path
}
