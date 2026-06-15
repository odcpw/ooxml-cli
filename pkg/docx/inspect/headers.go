package inspect

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// Header/footer kinds.
const (
	KindHeader = "header"
	KindFooter = "footer"
)

// Header/footer reference types as carried by the w:type attribute.
const (
	TypeDefault = "default"
	TypeFirst   = "first"
	TypeEven    = "even"
)

// HeaderFooterRef describes a single resolved header/footer reference within a section.
type HeaderFooterRef struct {
	Kind            string   `json:"kind"`
	ID              string   `json:"id"`
	Type            string   `json:"type"`
	Section         int      `json:"section,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	PartURI         string   `json:"partUri"`
	ContentType     string   `json:"contentType"`
}

// SectionHeaderFooters holds the header and footer references for one document section.
type SectionHeaderFooters struct {
	SectionIndex int              `json:"sectionIndex"`
	Headers      *HeaderFooterSet `json:"headers"`
	Footers      *HeaderFooterSet `json:"footers"`
}

// HeaderFooterSet groups the default/first/even references of a single kind.
type HeaderFooterSet struct {
	Default *HeaderFooterRef `json:"default"`
	First   *HeaderFooterRef `json:"first"`
	Even    *HeaderFooterRef `json:"even"`
}

// DocumentHeaderFooters is the top-level listing of every section's header/footer references.
type DocumentHeaderFooters struct {
	DocumentPartURI string                 `json:"documentPartUri"`
	Sections        []SectionHeaderFooters `json:"sections"`
}

// HeaderFooterParagraph is a single paragraph extracted from a header/footer part.
type HeaderFooterParagraph struct {
	Index           int      `json:"index"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Style           string   `json:"style"`
	Text            string   `json:"text"`
}

// ListHeadersFooters walks every w:sectPr in the body and resolves each
// headerReference/footerReference to its part URI and content type.
func ListHeadersFooters(session opc.PackageSession, documentURI string) (*DocumentHeaderFooters, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read document part %s: %w", documentURI, err)
	}
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		return nil, err
	}

	relTargets := relationshipTargets(session, documentURI)

	result := &DocumentHeaderFooters{DocumentPartURI: documentURI}
	for i, sectPr := range sectionProperties(body) {
		section := SectionHeaderFooters{
			SectionIndex: i + 1,
			Headers:      &HeaderFooterSet{},
			Footers:      &HeaderFooterSet{},
		}
		for _, ref := range namespaces.FindChildren(sectPr, namespaces.NsW, "headerReference") {
			assignRef(section.Headers, buildRef(session, KindHeader, ref, relTargets, section.SectionIndex))
		}
		for _, ref := range namespaces.FindChildren(sectPr, namespaces.NsW, "footerReference") {
			assignRef(section.Footers, buildRef(session, KindFooter, ref, relTargets, section.SectionIndex))
		}
		result.Sections = append(result.Sections, section)
	}
	return result, nil
}

// ResolveHeaderFooter selects a single header/footer reference by kind, type, and
// 1-based section index. If id is non-empty it is used as a direct relationship
// resolver instead of scanning sectPr.
func ResolveHeaderFooter(session opc.PackageSession, documentURI, kind, refType, id string, section int) (*HeaderFooterRef, error) {
	listing, err := ListHeadersFooters(session, documentURI)
	if err != nil {
		return nil, err
	}
	if id != "" {
		for _, ref := range HeaderFooterRefs(listing, kind) {
			if ref.ID == id {
				return ref, nil
			}
		}
		relTargets := relationshipTargets(session, documentURI)
		target, ok := relTargets[id]
		if !ok {
			return nil, fmt.Errorf("relationship %q not found in %s", id, documentURI)
		}
		resolvedKind := kind
		if resolvedKind == "" {
			resolvedKind = kindFromContentType(session.GetContentType(target))
		}
		return annotateHeaderFooterRef(&HeaderFooterRef{
			Kind:        resolvedKind,
			ID:          id,
			Type:        refType,
			PartURI:     target,
			ContentType: session.GetContentType(target),
		}, 0), nil
	}
	if len(listing.Sections) == 0 {
		return nil, fmt.Errorf("document has no sections")
	}
	if section <= 0 {
		// 0 (the default) means the last section, matching the set-text path.
		section = len(listing.Sections)
	}
	if section > len(listing.Sections) {
		return nil, fmt.Errorf("section %d out of range (document has %d sections)", section, len(listing.Sections))
	}
	set := listing.Sections[section-1].Headers
	if kind == KindFooter {
		set = listing.Sections[section-1].Footers
	}
	ref := selectByType(set, refType)
	if ref == nil {
		return nil, fmt.Errorf("no %s of type %q in section %d", kind, refType, section)
	}
	return ref, nil
}

// HeaderFooterRefs flattens a listing to the concrete refs of one kind. If kind
// is empty, both headers and footers are returned in document order.
func HeaderFooterRefs(listing *DocumentHeaderFooters, kind string) []*HeaderFooterRef {
	if listing == nil {
		return nil
	}
	var out []*HeaderFooterRef
	for i := range listing.Sections {
		section := &listing.Sections[i]
		if kind == "" || kind == KindHeader {
			appendHeaderFooterSetRefs(&out, section.Headers)
		}
		if kind == "" || kind == KindFooter {
			appendHeaderFooterSetRefs(&out, section.Footers)
		}
	}
	return out
}

func appendHeaderFooterSetRefs(out *[]*HeaderFooterRef, set *HeaderFooterSet) {
	if set == nil {
		return
	}
	for _, ref := range []*HeaderFooterRef{set.Default, set.First, set.Even} {
		if ref != nil {
			*out = append(*out, ref)
		}
	}
}

// ReadHeaderFooterParagraphs reads a header/footer part and extracts its paragraphs.
func ReadHeaderFooterParagraphs(session opc.PackageSession, partURI string) ([]HeaderFooterParagraph, error) {
	doc, err := session.ReadXMLPart(partURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read header/footer part %s: %w", partURI, err)
	}
	root := doc.Root()
	if root == nil || (!namespaces.IsElement(root, namespaces.NsW, "hdr") && !namespaces.IsElement(root, namespaces.NsW, "ftr")) {
		return nil, fmt.Errorf("part %s is not a header or footer", partURI)
	}
	return ExtractHeaderFooterParagraphs(root), nil
}

// AnnotateHeaderFooterParagraphs adds stable paragraph selectors scoped beneath
// a resolved header/footer ref. The returned slice is a copy.
func AnnotateHeaderFooterParagraphs(ref *HeaderFooterRef, paragraphs []HeaderFooterParagraph) []HeaderFooterParagraph {
	out := make([]HeaderFooterParagraph, len(paragraphs))
	copy(out, paragraphs)
	if ref == nil || ref.PrimarySelector == "" {
		return out
	}
	for i := range out {
		out[i].PrimarySelector = HeaderFooterParagraphPrimarySelector(ref.PrimarySelector, out[i].Index)
		out[i].Selectors = HeaderFooterParagraphSelectors(ref, out[i].Index)
	}
	return out
}

// HeaderFooterPrimarySelector returns the canonical paste-able selector for a
// section-scoped header/footer reference.
func HeaderFooterPrimarySelector(kind string, section int, refType string) string {
	if kind == "" || section < 1 {
		return ""
	}
	return fmt.Sprintf("%s:%d:%s", kind, section, normalizeHeaderFooterRefType(refType))
}

// HeaderFooterSelectors returns all practical aliases an agent may paste back
// into show/set-text --selector.
func HeaderFooterSelectors(kind string, section int, refType, id, partURI string) []string {
	refType = normalizeHeaderFooterRefType(refType)
	var out []string
	if primary := HeaderFooterPrimarySelector(kind, section, refType); primary != "" {
		out = appendUniqueString(out, primary)
	}
	if id != "" {
		out = appendUniqueString(out, "id:"+id)
		out = appendUniqueString(out, id)
	}
	if partURI != "" {
		out = appendUniqueString(out, "part:"+partURI)
		out = appendUniqueString(out, partURI)
	}
	return out
}

// HeaderFooterParagraphPrimarySelector returns the canonical selector for one
// paragraph inside a resolved header/footer part.
func HeaderFooterParagraphPrimarySelector(refSelector string, index int) string {
	if refSelector == "" || index < 1 {
		return ""
	}
	return refSelector + "/p:" + strconv.Itoa(index)
}

// HeaderFooterParagraphSelectors returns practical aliases for a paragraph
// beneath a header/footer selector.
func HeaderFooterParagraphSelectors(ref *HeaderFooterRef, index int) []string {
	if ref == nil || index < 1 {
		return nil
	}
	var out []string
	for _, selector := range ref.Selectors {
		if selector == "" {
			continue
		}
		out = appendUniqueString(out, selector+"/p:"+strconv.Itoa(index))
		out = appendUniqueString(out, selector+"/paragraph:"+strconv.Itoa(index))
	}
	return out
}

// ExtractHeaderFooterParagraphs enumerates w:p children of a w:hdr/w:ftr root.
func ExtractHeaderFooterParagraphs(root *etree.Element) []HeaderFooterParagraph {
	var paragraphs []HeaderFooterParagraph
	for _, p := range namespaces.FindChildren(root, namespaces.NsW, "p") {
		paragraphs = append(paragraphs, HeaderFooterParagraph{
			Index: len(paragraphs) + 1,
			Style: docxbody.ParagraphStyle(p),
			Text:  docxbody.ParagraphText(p),
		})
	}
	return paragraphs
}

// sectionProperties returns every w:sectPr that defines a section, in document order:
// each w:p/w:pPr/w:sectPr (an inline section break) followed by the trailing body w:sectPr.
func sectionProperties(body *etree.Element) []*etree.Element {
	var sections []*etree.Element
	for _, child := range body.ChildElements() {
		switch docxbody.LocalName(child.Tag) {
		case "p":
			if pPr := namespaces.FindChild(child, namespaces.NsW, "pPr"); pPr != nil {
				if sectPr := namespaces.FindChild(pPr, namespaces.NsW, "sectPr"); sectPr != nil {
					sections = append(sections, sectPr)
				}
			}
		case "sectPr":
			sections = append(sections, child)
		}
	}
	return sections
}

func buildRef(session opc.PackageSession, kind string, ref *etree.Element, relTargets map[string]string, section int) *HeaderFooterRef {
	id, _ := namespaces.Attr(ref, namespaces.NsR, "id")
	refType, _ := namespaces.Attr(ref, namespaces.NsW, "type")
	if refType == "" {
		refType = TypeDefault
	}
	partURI := relTargets[id]
	return annotateHeaderFooterRef(&HeaderFooterRef{
		Kind:        kind,
		ID:          id,
		Type:        refType,
		PartURI:     partURI,
		ContentType: session.GetContentType(partURI),
	}, section)
}

func annotateHeaderFooterRef(ref *HeaderFooterRef, section int) *HeaderFooterRef {
	if ref == nil {
		return nil
	}
	ref.Type = normalizeHeaderFooterRefType(ref.Type)
	ref.Section = section
	ref.PrimarySelector = HeaderFooterPrimarySelector(ref.Kind, section, ref.Type)
	ref.Selectors = HeaderFooterSelectors(ref.Kind, section, ref.Type, ref.ID, ref.PartURI)
	if ref.PrimarySelector == "" && len(ref.Selectors) > 0 {
		ref.PrimarySelector = ref.Selectors[0]
	}
	return ref
}

func normalizeHeaderFooterRefType(refType string) string {
	switch refType {
	case TypeFirst, TypeEven:
		return refType
	default:
		return TypeDefault
	}
}

func appendUniqueString(out []string, value string) []string {
	if value == "" {
		return out
	}
	for _, existing := range out {
		if existing == value {
			return out
		}
	}
	return append(out, value)
}

func assignRef(set *HeaderFooterSet, ref *HeaderFooterRef) {
	switch ref.Type {
	case TypeFirst:
		set.First = ref
	case TypeEven:
		set.Even = ref
	default:
		set.Default = ref
	}
}

func selectByType(set *HeaderFooterSet, refType string) *HeaderFooterRef {
	switch refType {
	case TypeFirst:
		return set.First
	case TypeEven:
		return set.Even
	default:
		return set.Default
	}
}

func relationshipTargets(session opc.PackageSession, documentURI string) map[string]string {
	targets := make(map[string]string)
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.TargetMode == "External" {
			continue
		}
		targets[rel.ID] = resolveTargetURI(documentURI, rel.Target)
	}
	return targets
}

func kindFromContentType(contentType string) string {
	switch contentType {
	case namespaces.ContentTypeFooter:
		return KindFooter
	case namespaces.ContentTypeHeader:
		return KindHeader
	default:
		return ""
	}
}
