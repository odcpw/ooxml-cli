// Package namespaces provides DOCX-specific XML namespace constants and traversal helpers.
package namespaces

import (
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
)

const (
	NsWordprocessingML = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
	NsW                = NsWordprocessingML
	NsR                = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
	NsXML              = "http://www.w3.org/XML/1998/namespace"
	// NsW14 is the Microsoft Word 2010 wordml extension namespace. It carries
	// w14:paraId, the durable per-paragraph id that PR-HANDLES-1 uses (and, when
	// absent, injects) as a stable paragraph handle marker.
	NsW14 = "http://schemas.microsoft.com/office/word/2010/wordml"
)

const (
	RelOfficeDocument = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
	RelStyles         = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
	RelNumbering      = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering"
	RelHeader         = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header"
	RelFooter         = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer"
	RelFootnotes      = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes"
	RelEndnotes       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes"
	RelComments       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
	RelImage          = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
	RelHyperlink      = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"
)

const (
	ContentTypeDocument  = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
	ContentTypeStyles    = "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"
	ContentTypeNumbering = "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"
	ContentTypeHeader    = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"
	ContentTypeFooter    = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"
	ContentTypeFootnotes = "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"
	ContentTypeEndnotes  = "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"
	ContentTypeComments  = "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"
)

func FindChild(elem *etree.Element, ns, localName string) *etree.Element {
	return xmlx.FindChild(elem, ns, localName)
}

func FindChildren(elem *etree.Element, ns, localName string) []*etree.Element {
	return xmlx.FindChildren(elem, ns, localName)
}

func FindDescendants(elem *etree.Element, ns, localName string) []*etree.Element {
	return xmlx.FindDescendants(elem, ns, localName)
}

func Attr(elem *etree.Element, ns, localName string) (string, bool) {
	if elem == nil {
		return "", false
	}

	if value, ok := xmlx.GetAttrNS(elem, ns, localName); ok {
		return value, true
	}

	prefix := namespacePrefix(ns)
	if prefix != "" {
		if attr := elem.SelectAttr(prefix + ":" + localName); attr != nil {
			return attr.Value, true
		}
	}

	for _, attr := range elem.Attr {
		if attr.Key != localName {
			continue
		}
		if attr.Space == ns || attr.Space == prefix {
			return attr.Value, true
		}
	}

	return "", false
}

func IsElement(elem *etree.Element, ns, localName string) bool {
	return xmlx.ElementMatches(elem, ns, localName)
}

func RelationshipKind(relType string) string {
	if relType == "" {
		return ""
	}
	if idx := strings.LastIndex(relType, "/"); idx >= 0 && idx+1 < len(relType) {
		return relType[idx+1:]
	}
	return relType
}

func namespacePrefix(ns string) string {
	switch ns {
	case NsW:
		return "w"
	case NsR:
		return "r"
	case NsXML:
		return "xml"
	default:
		return ""
	}
}
