package validate

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxns "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func validateChartParts(session opc.PackageSession) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, part := range session.ListParts() {
		if part.ContentType != xlsxns.ContentTypeChart {
			continue
		}
		doc, err := session.ReadXMLPart(part.URI)
		if err != nil || doc == nil || doc.Root() == nil {
			continue
		}
		for _, plotArea := range xmlx.FindDescendants(doc.Root(), xlsxns.NsChart, "plotArea") {
			diags = append(diags, validateChartAxes(part.URI, plotArea)...)
		}
	}
	return diags
}

func validateChartAxes(partURI string, plotArea *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, axis := range plotArea.ChildElements() {
		axisName := chartLocalName(axis)
		if !isChartAxis(axisName) {
			continue
		}
		label := chartAxisDiagnosticLabel(axis)
		for _, childName := range []string{"axId", "scaling", "axPos", "crossAx"} {
			if firstChartChild(axis, childName) == nil {
				diags = append(diags, diag.Error(
					"OOXML_CHART_AXIS_REQUIRED",
					fmt.Sprintf("%s %s is missing required <c:%s>", partURI, label, childName),
				))
			}
		}
		if axPos := firstChartChild(axis, "axPos"); axPos != nil {
			value := strings.TrimSpace(axPos.SelectAttrValue("val", ""))
			if !isValidChartAxisPosition(value) {
				diags = append(diags, diag.Error(
					"OOXML_CHART_AXIS_POSITION",
					fmt.Sprintf("%s %s has invalid <c:axPos val=%q>", partURI, label, value),
				))
			}
		}
		diags = append(diags, validateChartAxisChildOrder(partURI, axis, label)...)
	}
	return diags
}

func validateChartAxisChildOrder(partURI string, axis *etree.Element, label string) []result.Diagnostic {
	var diags []result.Diagnostic
	lastOrder := 0
	lastName := ""
	for _, child := range axis.ChildElements() {
		name := chartLocalName(child)
		order := chartAxisChildOrder(name)
		if order == 0 {
			continue
		}
		if lastOrder > order {
			diags = append(diags, diag.Error(
				"OOXML_CHART_AXIS_ORDER",
				fmt.Sprintf("%s %s child <c:%s> appears after <c:%s>", partURI, label, name, lastName),
			))
			continue
		}
		lastOrder = order
		lastName = name
	}
	return diags
}

func isChartAxis(name string) bool {
	switch name {
	case "catAx", "dateAx", "valAx", "serAx":
		return true
	default:
		return false
	}
}

func isValidChartAxisPosition(value string) bool {
	switch value {
	case "b", "l", "r", "t":
		return true
	default:
		return false
	}
}

func chartAxisChildOrder(name string) int {
	switch name {
	case "axId":
		return 1
	case "scaling":
		return 2
	case "delete":
		return 3
	case "axPos":
		return 4
	case "majorGridlines":
		return 5
	case "minorGridlines":
		return 6
	case "title":
		return 7
	case "numFmt":
		return 8
	case "majorTickMark":
		return 9
	case "minorTickMark":
		return 10
	case "tickLblPos":
		return 11
	case "spPr":
		return 12
	case "txPr":
		return 13
	case "crossAx":
		return 14
	case "crosses", "crossesAt":
		return 15
	case "crossBetween":
		return 16
	case "majorUnit":
		return 17
	case "minorUnit":
		return 18
	case "dispUnits":
		return 19
	case "auto":
		return 20
	case "lblAlgn":
		return 21
	case "lblOffset":
		return 22
	case "tickLblSkip":
		return 23
	case "tickMarkSkip":
		return 24
	case "noMultiLvlLbl":
		return 25
	case "extLst":
		return 26
	default:
		return 0
	}
}

func firstChartChild(parent *etree.Element, localName string) *etree.Element {
	return xmlx.FindChild(parent, xlsxns.NsChart, localName)
}

func chartAxisDiagnosticLabel(axis *etree.Element) string {
	name := chartLocalName(axis)
	if axID := chartChildVal(axis, "axId"); axID != "" {
		return fmt.Sprintf("<c:%s axId=%q>", name, axID)
	}
	return fmt.Sprintf("<c:%s>", name)
}

func chartChildVal(parent *etree.Element, childName string) string {
	child := firstChartChild(parent, childName)
	if child == nil {
		return ""
	}
	return strings.TrimSpace(child.SelectAttrValue("val", ""))
}

func chartLocalName(elem *etree.Element) string {
	if elem == nil {
		return ""
	}
	if idx := strings.LastIndex(elem.Tag, "}"); idx >= 0 && idx+1 < len(elem.Tag) {
		return elem.Tag[idx+1:]
	}
	if idx := strings.LastIndex(elem.Tag, ":"); idx >= 0 && idx+1 < len(elem.Tag) {
		return elem.Tag[idx+1:]
	}
	return elem.Tag
}
