package mutate

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// dateFieldType maps the practical --date-format choices to an a:fld@type value.
// "auto" uses PowerPoint's locale-resolved date field; "datetime" and "date-only"
// pick concrete ST_TextFieldType date formats.
var dateFieldType = map[string]string{
	"auto":      "datetimeFigureOut",
	"datetime":  "datetime",
	"date-only": "datetime1",
}

// ValidDateFormats returns the accepted --date-format values, for CLI validation
// and help text.
func ValidDateFormats() []string {
	return []string{"auto", "datetime", "date-only"}
}

// SetFieldsRequest holds the presentation-wide header/footer field changes to
// apply. Pointer fields distinguish "leave unchanged" (nil) from an explicit set.
//
// Visibility toggles (ShowSlideNumber/ShowFooter/ShowDate) are written to every
// slide master's p:hf element, matching how PowerPoint's Header & Footer dialog
// stores presentation-wide visibility. FooterText and DateFormat update the
// matching placeholder shapes on every slide that already carries them.
type SetFieldsRequest struct {
	Package opc.PackageSession

	FooterText      *string
	ShowSlideNumber *bool
	ShowFooter      *bool
	ShowDate        *bool
	// DateFormat is one of the keys in dateFieldType ("auto", "datetime",
	// "date-only"). Empty means leave the date field type unchanged.
	DateFormat string
}

// SetFieldsResult describes the outcome of a SetFields mutation.
type SetFieldsResult struct {
	FooterText      *string `json:"footerText,omitempty"`
	ShowSlideNumber *bool   `json:"showSlideNumber,omitempty"`
	ShowFooter      *bool   `json:"showFooter,omitempty"`
	ShowDate        *bool   `json:"showDate,omitempty"`
	DateFormat      string  `json:"dateFormat,omitempty"`

	// MastersUpdated lists master part URIs whose p:hf visibility was changed or
	// created.
	MastersUpdated []string `json:"mastersUpdated"`
	// CreatedHeaderFooter is true when at least one master's p:hf element was
	// created (it was absent before).
	CreatedHeaderFooter bool `json:"createdHeaderFooter"`
	// FooterPlaceholdersUpdated is the number of slide footer placeholders whose
	// text was changed.
	FooterPlaceholdersUpdated int `json:"footerPlaceholdersUpdated"`
	// DatePlaceholdersUpdated is the number of slide date placeholders whose field
	// type was changed.
	DatePlaceholdersUpdated int `json:"datePlaceholdersUpdated"`
	// SlidesWithoutFooterPlaceholder lists slides that had no footer placeholder
	// to receive --footer text. Informational, not an error.
	SlidesWithoutFooterPlaceholder []int `json:"slidesWithoutFooterPlaceholder,omitempty"`
	// SlidesWithoutDatePlaceholder lists slides that had no date placeholder to
	// receive --date-format. Informational, not an error.
	SlidesWithoutDatePlaceholder []int `json:"slidesWithoutDatePlaceholder,omitempty"`
	// SlidesWithDatePlaceholderButNoField lists slides that carry a date
	// placeholder which holds only plain run text (no a:fld). --date-format can
	// only retype an existing date field, so on these slides it is a no-op. This is
	// surfaced so the caller can tell present-but-unchanged from genuinely applied;
	// it is informational, not an error.
	SlidesWithDatePlaceholderButNoField []int `json:"slidesWithDatePlaceholderButNoField,omitempty"`
}

// hfChildOrder is the CT_SlideMaster child element sequence (ECMA-376 Part 1).
// p:hf must be written after p:sldLayoutIdLst/p:transition/p:timing and before
// p:txStyles for strict schema validation.
var slideMasterChildOrder = []string{
	"cSld", "clrMap", "sldLayoutIdLst", "transition", "timing", "hf", "txStyles", "extLst",
}

func slideMasterChildRank(local string) int {
	for i, name := range slideMasterChildOrder {
		if name == local {
			return i
		}
	}
	return len(slideMasterChildOrder)
}

// SetFields applies presentation-wide header/footer field changes.
func SetFields(req *SetFieldsRequest) (*SetFieldsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set fields request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.DateFormat != "" {
		if _, ok := dateFieldType[req.DateFormat]; !ok {
			return nil, fmt.Errorf("invalid date format %q (expected auto, datetime, or date-only)", req.DateFormat)
		}
	}
	if req.FooterText == nil && req.ShowSlideNumber == nil && req.ShowFooter == nil &&
		req.ShowDate == nil && req.DateFormat == "" {
		return nil, fmt.Errorf("no field changes requested")
	}

	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, err
	}

	result := &SetFieldsResult{
		FooterText:      req.FooterText,
		ShowSlideNumber: req.ShowSlideNumber,
		ShowFooter:      req.ShowFooter,
		ShowDate:        req.ShowDate,
		DateFormat:      req.DateFormat,
		MastersUpdated:  []string{},
	}

	// Visibility toggles live on each master's p:hf element.
	if req.ShowSlideNumber != nil || req.ShowFooter != nil || req.ShowDate != nil {
		for _, master := range graph.Masters {
			changed, created, err := applyMasterVisibility(req.Package, master.PartURI, req)
			if err != nil {
				return nil, err
			}
			if created {
				result.CreatedHeaderFooter = true
			}
			if changed {
				result.MastersUpdated = append(result.MastersUpdated, master.PartURI)
			}
		}
	}

	// Footer text and date format update slide placeholder shapes where present.
	if req.FooterText != nil || req.DateFormat != "" {
		for _, slide := range graph.Slides {
			footerUpdated, dateUpdated, hadFooter, hadDate, dateFieldPresent, err := applySlidePlaceholders(req.Package, slide.PartURI, req)
			if err != nil {
				return nil, err
			}
			if footerUpdated {
				result.FooterPlaceholdersUpdated++
			}
			if dateUpdated {
				result.DatePlaceholdersUpdated++
			}
			if req.FooterText != nil && !hadFooter {
				result.SlidesWithoutFooterPlaceholder = append(result.SlidesWithoutFooterPlaceholder, slide.SlideNumber)
			}
			if req.DateFormat != "" {
				switch {
				case !hadDate:
					result.SlidesWithoutDatePlaceholder = append(result.SlidesWithoutDatePlaceholder, slide.SlideNumber)
				case !dateFieldPresent:
					// Placeholder exists but has no a:fld to retype, so --date-format
					// was a no-op here. Report it so the caller is not misled by an
					// otherwise successful readback.
					result.SlidesWithDatePlaceholderButNoField = append(result.SlidesWithDatePlaceholderButNoField, slide.SlideNumber)
				}
			}
		}
	}

	return result, nil
}

// applyMasterVisibility ensures a p:hf element exists on the master and applies
// the requested visibility attributes. Returns whether the master XML changed and
// whether the p:hf element was created.
func applyMasterVisibility(pkg opc.PackageSession, masterURI string, req *SetFieldsRequest) (changed bool, created bool, err error) {
	doc, err := pkg.ReadXMLPart(masterURI)
	if err != nil {
		return false, false, fmt.Errorf("failed to read master %s: %w", masterURI, err)
	}
	root := doc.Root()
	if root == nil {
		return false, false, fmt.Errorf("master %s has no root element", masterURI)
	}

	hf := xmlx.FindChild(root, ns.NsP, "hf")
	if hf == nil {
		hf = insertSlideMasterChild(root, "hf")
		created = true
		changed = true
	}

	if applyVisibilityAttr(hf, "sldNum", req.ShowSlideNumber) {
		changed = true
	}
	if applyVisibilityAttr(hf, "ftr", req.ShowFooter) {
		changed = true
	}
	if applyVisibilityAttr(hf, "dt", req.ShowDate) {
		changed = true
	}

	if !changed {
		return false, false, nil
	}
	if err := pkg.ReplaceXMLPart(masterURI, doc); err != nil {
		return false, false, fmt.Errorf("failed to replace master %s: %w", masterURI, err)
	}
	return changed, created, nil
}

// applyVisibilityAttr writes a CT_HeaderFooter boolean attribute when value is
// non-nil. The schema default is true, so an explicit "true" is written as "1"
// rather than dropping the attribute, to make the intent durable and inspectable.
// Returns whether the attribute value changed.
func applyVisibilityAttr(hf *etree.Element, name string, value *bool) bool {
	if value == nil {
		return false
	}
	want := boolAttr(*value)
	if cur, ok := xmlx.GetAttr(hf, name); ok && cur == want {
		return false
	}
	hf.CreateAttr(name, want)
	return true
}

// insertSlideMasterChild creates a new schema-ordered child element on a slide
// master root and returns it.
func insertSlideMasterChild(root *etree.Element, local string) *etree.Element {
	newChild := etree.NewElement("p:" + local)
	rank := slideMasterChildRank(local)
	for _, existing := range root.ChildElements() {
		if slideMasterChildRank(localTag(existing.Tag)) > rank {
			root.InsertChildAt(existing.Index(), newChild)
			return newChild
		}
	}
	root.AddChild(newChild)
	return newChild
}

// applySlidePlaceholders updates footer text and/or date field type on the
// matching placeholders of a single slide. Returns which placeholders changed and
// whether each placeholder type was present.
func applySlidePlaceholders(pkg opc.PackageSession, slideURI string, req *SetFieldsRequest) (footerUpdated, dateUpdated, hadFooter, hadDate, dateFieldPresent bool, err error) {
	doc, err := pkg.ReadXMLPart(slideURI)
	if err != nil {
		return false, false, false, false, false, fmt.Errorf("failed to read slide %s: %w", slideURI, err)
	}
	root := doc.Root()
	spTree := findSlideSpTree(root)
	if spTree == nil {
		return false, false, false, false, false, nil
	}

	if req.FooterText != nil {
		if sp := findPlaceholderShape(spTree, "ftr"); sp != nil {
			hadFooter = true
			if setFooterText(sp, *req.FooterText) {
				footerUpdated = true
			}
		}
	}
	if req.DateFormat != "" {
		if sp := findPlaceholderShape(spTree, "dt"); sp != nil {
			hadDate = true
			var changed bool
			changed, dateFieldPresent = setDateFieldType(sp, dateFieldType[req.DateFormat])
			if changed {
				dateUpdated = true
			}
		}
	}

	if !footerUpdated && !dateUpdated {
		return footerUpdated, dateUpdated, hadFooter, hadDate, dateFieldPresent, nil
	}
	if err := pkg.ReplaceXMLPart(slideURI, doc); err != nil {
		return false, false, false, false, false, fmt.Errorf("failed to replace slide %s: %w", slideURI, err)
	}
	return footerUpdated, dateUpdated, hadFooter, hadDate, dateFieldPresent, nil
}

// findSlideSpTree returns the p:cSld/p:spTree element of a slide root, or nil.
func findSlideSpTree(root *etree.Element) *etree.Element {
	cSld := xmlx.FindChild(root, ns.NsP, "cSld")
	if cSld == nil {
		return nil
	}
	return xmlx.FindChild(cSld, ns.NsP, "spTree")
}

// findPlaceholderShape returns the first p:sp whose p:ph@type matches phType.
func findPlaceholderShape(spTree *etree.Element, phType string) *etree.Element {
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
		if t, _ := xmlx.GetAttr(ph, "type"); t == phType {
			return sp
		}
	}
	return nil
}

// setFooterText sets the literal footer text in a footer placeholder's first
// paragraph, replacing any existing runs with a single a:r/a:t. An empty text
// clears the footer to a single empty paragraph. Returns whether anything changed.
func setFooterText(sp *etree.Element, text string) bool {
	txBody := xmlx.FindChild(sp, ns.NsP, "txBody")
	if txBody == nil {
		return false
	}
	paras := xmlx.FindChildren(txBody, ns.NsA, "p")
	var p *etree.Element
	if len(paras) > 0 {
		p = paras[0]
	} else {
		p = etree.NewElement("a:p")
		txBody.AddChild(p)
	}

	if footerParagraphText(p) == text {
		return false
	}

	// Preserve a:pPr (must stay first) and a:endParaRPr (must stay last); drop the
	// run/field content in between and rewrite it.
	pPr := xmlx.FindChild(p, ns.NsA, "pPr")
	endParaRPr := xmlx.FindChild(p, ns.NsA, "endParaRPr")
	for _, child := range p.ChildElements() {
		if child == pPr || child == endParaRPr {
			continue
		}
		p.RemoveChild(child)
	}

	if text != "" {
		r := etree.NewElement("a:r")
		t := etree.NewElement("a:t")
		t.SetText(text)
		r.AddChild(t)
		insertFooterRun(p, r, endParaRPr)
	}
	return true
}

// insertFooterRun inserts a run before endParaRPr (when present) so paragraph
// child order (pPr, runs..., endParaRPr) is preserved.
func insertFooterRun(p, run, endParaRPr *etree.Element) {
	if endParaRPr != nil {
		p.InsertChildAt(endParaRPr.Index(), run)
		return
	}
	p.AddChild(run)
}

// footerParagraphText returns the concatenated a:r/a:t text of a paragraph.
func footerParagraphText(p *etree.Element) string {
	text := ""
	for _, r := range xmlx.FindChildren(p, ns.NsA, "r") {
		if t := xmlx.FindChild(r, ns.NsA, "t"); t != nil {
			text += t.Text()
		}
	}
	return text
}

// setDateFieldType updates the a:fld@type of the date placeholder's field. It
// reports whether the type changed and whether an a:fld was present at all. When
// no a:fld exists (the placeholder holds only plain run text) it cannot retype
// anything: changed is false and fieldPresent is false, letting the caller flag
// the no-op rather than report a silent success.
func setDateFieldType(sp *etree.Element, fieldType string) (changed, fieldPresent bool) {
	txBody := xmlx.FindChild(sp, ns.NsP, "txBody")
	if txBody == nil {
		return false, false
	}
	for _, p := range xmlx.FindChildren(txBody, ns.NsA, "p") {
		if fld := xmlx.FindChild(p, ns.NsA, "fld"); fld != nil {
			if cur, _ := xmlx.GetAttr(fld, "type"); cur == fieldType {
				return false, true
			}
			fld.CreateAttr("type", fieldType)
			return true, true
		}
	}
	return false, false
}
