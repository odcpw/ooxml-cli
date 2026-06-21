package inspect

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// FieldsReport summarizes the presentation-wide header/footer/slide-number/date
// field configuration: the visibility defaults declared by each slide master's
// p:hf element plus, per slide, which footer/date/slide-number placeholders exist
// and the practical values they carry (footer text, date a:fld type).
type FieldsReport struct {
	// Masters lists the p:hf visibility defaults for each slide master. These are
	// the presentation-wide toggles PowerPoint's Header & Footer dialog writes.
	Masters []FieldsMasterDefaults `json:"masters"`
	// Slides lists per-slide field placeholder presence and values.
	Slides []FieldsSlideInfo `json:"slides"`
}

// FieldsMasterDefaults captures the p:hf visibility attributes for a slide master.
// CT_HeaderFooter omits absent attributes, which the schema treats as true; the
// booleans here report the effective (default-true) values.
type FieldsMasterDefaults struct {
	PartURI         string `json:"partUri"`
	HasHeaderFooter bool   `json:"hasHeaderFooter"`
	ShowSlideNumber bool   `json:"showSlideNumber"`
	ShowFooter      bool   `json:"showFooter"`
	ShowDate        bool   `json:"showDate"`
	ShowHeader      bool   `json:"showHeader"`
}

// FieldsSlideInfo captures the field-placeholder state of a single slide.
type FieldsSlideInfo struct {
	Slide   int    `json:"slide"`
	PartURI string `json:"partUri"`
	// FooterPlaceholder is non-nil when the slide carries an a:ph type="ftr" shape.
	FooterPlaceholder *FieldPlaceholderInfo `json:"footerPlaceholder,omitempty"`
	// DatePlaceholder is non-nil when the slide carries an a:ph type="dt" shape.
	DatePlaceholder *FieldPlaceholderInfo `json:"datePlaceholder,omitempty"`
	// SlideNumberPlaceholder is non-nil when the slide carries an a:ph type="sldNum" shape.
	SlideNumberPlaceholder *FieldPlaceholderInfo `json:"slideNumberPlaceholder,omitempty"`
}

// FieldPlaceholderInfo describes a single field placeholder shape on a slide.
type FieldPlaceholderInfo struct {
	ShapeID   int    `json:"shapeId"`
	ShapeName string `json:"shapeName"`
	// Text is the literal text content (footer text, or the cached a:fld text).
	Text string `json:"text,omitempty"`
	// FieldType is the a:fld@type when the placeholder uses a field (e.g.
	// "datetimeFigureOut", "slidenum"). Empty when no a:fld is present.
	FieldType string `json:"fieldType,omitempty"`
}

// ReadFields builds a FieldsReport from a presentation by reading each slide
// master's p:hf defaults and each slide's footer/date/slide-number placeholders.
func ReadFields(session opc.PackageSession) (*FieldsReport, error) {
	graph, err := ParsePresentation(session)
	if err != nil {
		return nil, err
	}
	report := &FieldsReport{
		Masters: make([]FieldsMasterDefaults, 0, len(graph.Masters)),
		Slides:  make([]FieldsSlideInfo, 0, len(graph.Slides)),
	}

	for _, master := range graph.Masters {
		doc, err := session.ReadXMLPart(master.PartURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read master %s: %w", master.PartURI, err)
		}
		report.Masters = append(report.Masters, readMasterHeaderFooter(master.PartURI, doc.Root()))
	}

	for _, slide := range graph.Slides {
		doc, err := session.ReadXMLPart(slide.PartURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read slide %s: %w", slide.PartURI, err)
		}
		info := FieldsSlideInfo{Slide: slide.SlideNumber, PartURI: slide.PartURI}
		spTree := findSpTree(doc.Root())
		if spTree != nil {
			info.FooterPlaceholder = readFieldPlaceholder(spTree, "ftr")
			info.DatePlaceholder = readFieldPlaceholder(spTree, "dt")
			info.SlideNumberPlaceholder = readFieldPlaceholder(spTree, "sldNum")
		}
		report.Slides = append(report.Slides, info)
	}

	return report, nil
}

// readMasterHeaderFooter reads the p:hf visibility defaults from a slide master
// root element. A missing p:hf (or missing attribute) is reported as visible,
// matching the CT_HeaderFooter schema default of true.
func readMasterHeaderFooter(partURI string, root *etree.Element) FieldsMasterDefaults {
	defaults := FieldsMasterDefaults{
		PartURI:         partURI,
		ShowSlideNumber: true,
		ShowFooter:      true,
		ShowDate:        true,
		ShowHeader:      true,
	}
	cSld := xmlx.FindChild(root, ns.NsP, "cSld")
	if cSld == nil {
		return defaults
	}
	hf := xmlx.FindChild(root, ns.NsP, "hf")
	if hf == nil {
		return defaults
	}
	defaults.HasHeaderFooter = true
	defaults.ShowSlideNumber = boolAttrDefaultTrue(hf, "sldNum")
	defaults.ShowFooter = boolAttrDefaultTrue(hf, "ftr")
	defaults.ShowDate = boolAttrDefaultTrue(hf, "dt")
	defaults.ShowHeader = boolAttrDefaultTrue(hf, "hdr")
	return defaults
}

// readFieldPlaceholder finds the first shape on a slide whose p:ph@type matches
// phType and returns its field info, or nil when no such placeholder exists.
func readFieldPlaceholder(spTree *etree.Element, phType string) *FieldPlaceholderInfo {
	for _, sp := range xmlx.FindChildren(spTree, ns.NsP, "sp") {
		nvSpPr := xmlx.FindChild(sp, ns.NsP, "nvSpPr")
		if nvSpPr == nil {
			continue
		}
		nvPr := xmlx.FindChild(nvSpPr, ns.NsP, "nvPr")
		if nvPr == nil {
			continue
		}
		ph := xmlx.FindChild(nvPr, ns.NsP, "ph")
		if ph == nil {
			continue
		}
		if t, _ := xmlx.GetAttr(ph, "type"); t != phType {
			continue
		}
		info := &FieldPlaceholderInfo{}
		if cNvPr := xmlx.FindChild(nvSpPr, ns.NsP, "cNvPr"); cNvPr != nil {
			if id, ok := xmlx.GetAttr(cNvPr, "id"); ok {
				fmt.Sscanf(id, "%d", &info.ShapeID)
			}
			info.ShapeName, _ = xmlx.GetAttr(cNvPr, "name")
		}
		info.Text, info.FieldType = readFieldPlaceholderText(sp)
		return info
	}
	return nil
}

// readFieldPlaceholderText extracts the literal text and field type from a
// placeholder shape's first text-body paragraph. A footer placeholder typically
// holds plain a:r runs; date/slide-number placeholders hold an a:fld with a
// cached a:t value.
func readFieldPlaceholderText(sp *etree.Element) (text string, fieldType string) {
	txBody := xmlx.FindChild(sp, ns.NsP, "txBody")
	if txBody == nil {
		return "", ""
	}
	for _, p := range xmlx.FindChildren(txBody, ns.NsA, "p") {
		if fld := xmlx.FindChild(p, ns.NsA, "fld"); fld != nil {
			fieldType, _ = xmlx.GetAttr(fld, "type")
			if t := xmlx.FindChild(fld, ns.NsA, "t"); t != nil {
				text = t.Text()
			}
			return text, fieldType
		}
		for _, r := range xmlx.FindChildren(p, ns.NsA, "r") {
			if t := xmlx.FindChild(r, ns.NsA, "t"); t != nil {
				text += t.Text()
			}
		}
		if text != "" {
			return text, ""
		}
	}
	return text, ""
}

// findSpTree returns the p:cSld/p:spTree element of a slide root, or nil.
func findSpTree(root *etree.Element) *etree.Element {
	cSld := xmlx.FindChild(root, ns.NsP, "cSld")
	if cSld == nil {
		return nil
	}
	return xmlx.FindChild(cSld, ns.NsP, "spTree")
}

// boolAttrDefaultTrue reads a boolean OOXML attribute that defaults to true when
// absent. Recognizes "0"/"false" as false and everything else as true.
func boolAttrDefaultTrue(elem *etree.Element, name string) bool {
	v, ok := xmlx.GetAttr(elem, name)
	if !ok {
		return true
	}
	return v != "0" && v != "false"
}
