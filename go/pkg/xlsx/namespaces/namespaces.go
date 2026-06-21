// Package namespaces provides XLSX-specific XML namespace constants and traversal helpers.
package namespaces

import (
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
)

// XLSX namespace URI constants.
const (
	// NsSpreadsheetML is the SpreadsheetML namespace for workbook and worksheet XML.
	NsSpreadsheetML = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
	NsSpreadsheet   = NsSpreadsheetML

	// NsR is the Office Open XML relationships namespace used by r:id attributes.
	NsR = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
)

// XLSX relationship type constants.
const (
	RelOfficeDocument = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
	RelWorksheet      = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet"
	RelSharedStrings  = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings"
	RelStyles         = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles"
	RelCalcChain      = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain"
	RelTable          = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table"
	RelDrawing        = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing"
	RelChart          = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart"
	RelPivotTable     = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable"
	RelPivotCache     = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition"
	RelPivotRecords   = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords"
	RelHyperlink      = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"
	RelComments       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments"
	RelVmlDrawing     = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing"

	// DrawingML namespaces used when authoring embedded charts.
	NsChart              = "http://schemas.openxmlformats.org/drawingml/2006/chart"
	NsDrawingMain        = "http://schemas.openxmlformats.org/drawingml/2006/main"
	NsSpreadsheetDrawing = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"
)

// XLSX content type constants.
const (
	ContentTypeWorkbook         = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"
	ContentTypeWorkbookMacro    = "application/vnd.ms-excel.sheet.macroEnabled.main+xml"
	ContentTypeWorkbookAddin    = "application/vnd.ms-excel.addin.macroEnabled.main+xml"
	ContentTypeWorkbookTemplate = "application/vnd.openxmlformats-officedocument.spreadsheetml.template.main+xml"
	ContentTypeSharedStrings    = "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"
	ContentTypeStyles           = "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"
	ContentTypeWorksheet        = "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"
	ContentTypeTable            = "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"
	ContentTypePivotTable       = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"
	ContentTypePivotCache       = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"
	ContentTypePivotRecords     = "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml"
	ContentTypeChart            = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml"
	ContentTypeDrawing          = "application/vnd.openxmlformats-officedocument.drawing+xml"
	ContentTypeCalcChain        = "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"
	ContentTypeComments         = "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml"
	// ContentTypeVml is the legacy VML drawing content type. It has no +xml
	// suffix, so the VML part must be written via AddPart with raw bytes; passing
	// it through ReplaceXMLPart would silently coerce it to application/xml.
	ContentTypeVml = "application/vnd.openxmlformats-officedocument.vmlDrawing"
)

// FindChild finds a direct child element with the given namespace and local name.
func FindChild(elem *etree.Element, ns, localName string) *etree.Element {
	return xmlx.FindChild(elem, ns, localName)
}

// FindChildren finds all direct child elements with the given namespace and local name.
func FindChildren(elem *etree.Element, ns, localName string) []*etree.Element {
	return xmlx.FindChildren(elem, ns, localName)
}

// FindDescendants recursively finds all descendant elements with the given namespace and local name.
func FindDescendants(elem *etree.Element, ns, localName string) []*etree.Element {
	return xmlx.FindDescendants(elem, ns, localName)
}

// Attr gets the value of an attribute by namespace and local name.
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

// IsElement reports whether elem matches the namespace and local name.
func IsElement(elem *etree.Element, ns, localName string) bool {
	return xmlx.ElementMatches(elem, ns, localName)
}

// RelationshipKind returns the final path segment from a relationship type URI.
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
	case NsR:
		return "r"
	default:
		return ""
	}
}
