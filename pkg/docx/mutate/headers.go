package mutate

import (
	"errors"
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	// ErrHeaderFooterPartNotFound is returned when a header/footer part URI cannot be read.
	ErrHeaderFooterPartNotFound = errors.New("header/footer part not found")
	// ErrHeaderFooterParaOutOfRange is returned when a paragraph index does not exist.
	ErrHeaderFooterParaOutOfRange = errors.New("header/footer paragraph index out of range")
)

// SetHeaderFooterTextRequest sets the text of a paragraph inside a header/footer part.
type SetHeaderFooterTextRequest struct {
	Package        opc.PackageSession
	PartURI        string
	ParagraphIndex int
	Text           string
}

// SetHeaderFooterTextResult reports the outcome of SetHeaderFooterText.
type SetHeaderFooterTextResult struct {
	PartURI        string `json:"partUri"`
	ParagraphIndex int    `json:"paragraphIndex"`
	PreviousText   string `json:"previousText"`
	Text           string `json:"text"`
}

// SetHeaderFooterText replaces a header/footer paragraph's text by 1-based index,
// preserving its w:pPr and the first run's properties.
func SetHeaderFooterText(req *SetHeaderFooterTextRequest) (*SetHeaderFooterTextResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set header/footer text request is nil")
	}
	if req.ParagraphIndex < 1 {
		return nil, fmt.Errorf("paragraph index must be >= 1")
	}
	doc, err := req.Package.ReadXMLPart(req.PartURI)
	if err != nil {
		return nil, fmt.Errorf("%w: %s", ErrHeaderFooterPartNotFound, req.PartURI)
	}
	root := doc.Root()
	if root == nil || (!namespaces.IsElement(root, namespaces.NsW, "hdr") && !namespaces.IsElement(root, namespaces.NsW, "ftr")) {
		return nil, fmt.Errorf("part %s is not a header or footer", req.PartURI)
	}

	paragraphs := namespaces.FindChildren(root, namespaces.NsW, "p")
	if req.ParagraphIndex > len(paragraphs) {
		return nil, fmt.Errorf("%w: %d", ErrHeaderFooterParaOutOfRange, req.ParagraphIndex)
	}
	paragraph := paragraphs[req.ParagraphIndex-1]

	previousText := docxbody.ParagraphText(paragraph)
	rPrCopy := firstDirectRunProperties(paragraph)
	clearParagraphChildren(paragraph)

	run := newElement(root.Space, "r")
	if rPrCopy != nil {
		run.AddChild(rPrCopy)
	}
	appendTextChildren(run, root.Space, req.Text)
	paragraph.AddChild(run)

	if err := req.Package.ReplaceXMLPart(req.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace header/footer part %s: %w", req.PartURI, err)
	}
	return &SetHeaderFooterTextResult{
		PartURI:        req.PartURI,
		ParagraphIndex: req.ParagraphIndex,
		PreviousText:   previousText,
		Text:           req.Text,
	}, nil
}

// EnsureHeaderFooterRequest ensures a header/footer of the given kind/type exists
// for a section, creating the part, relationship, and sectPr reference as needed.
type EnsureHeaderFooterRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	Kind         string // "header" or "footer"
	Type         string // "default", "first", or "even"
	SectionIndex int    // 1-based; 0 selects the trailing body section
}

// EnsureHeaderFooterResult reports the resolved part/relationship and whether they were created.
type EnsureHeaderFooterResult struct {
	PartURI     string `json:"partUri"`
	ID          string `json:"id"`
	Type        string `json:"type"`
	Kind        string `json:"kind"`
	CreatedPart bool   `json:"createdPart"`
	CreatedRef  bool   `json:"createdRef"`
}

// EnsureHeaderFooter guarantees a header/footer reference of the requested kind/type
// exists in the target section. It handles three cases idempotently:
//   - reference already present -> reuse it;
//   - part+relationship exist but sectPr lacks the reference -> add only the reference;
//   - nothing exists -> create the part, the relationship, and the reference.
func EnsureHeaderFooter(req *EnsureHeaderFooterRequest) (*EnsureHeaderFooterResult, error) {
	if req == nil {
		return nil, fmt.Errorf("ensure header/footer request is nil")
	}
	kind := req.Kind
	if kind != "header" && kind != "footer" {
		return nil, fmt.Errorf("kind must be header or footer")
	}
	refType := req.Type
	if refType == "" {
		refType = "default"
	}

	doc, err := req.Package.ReadXMLPart(req.DocumentURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read document part %s: %w", req.DocumentURI, err)
	}
	root := doc.Root()
	body, err := docxbody.FindBody(root)
	if err != nil {
		return nil, err
	}
	prefix := root.Space

	sectPr, err := selectSectPr(body, prefix, req.SectionIndex)
	if err != nil {
		return nil, err
	}

	refTag := kind + "Reference"

	// Case (a): the reference already exists -> reuse.
	if existing := findReferenceByType(sectPr, refTag, refType); existing != nil {
		id, _ := namespaces.Attr(existing, namespaces.NsR, "id")
		partURI := relationshipTarget(req.Package, req.DocumentURI, id)
		return &EnsureHeaderFooterResult{PartURI: partURI, ID: id, Type: refType, Kind: kind}, nil
	}

	result := &EnsureHeaderFooterResult{Type: refType, Kind: kind}

	// Reuse an existing unreferenced part+relationship if one is available (case b).
	relID, partURI := unreferencedPart(req.Package, req.DocumentURI, body, kind)
	if relID == "" {
		// Case (c): create the part and the relationship.
		partURI = allocatePartURI(req.Package, kind)
		if err := req.Package.AddPart(partURI, headerFooterTemplate(kind), contentTypeForKind(kind), nil); err != nil {
			return nil, fmt.Errorf("failed to add %s part %s: %w", kind, partURI, err)
		}
		result.CreatedPart = true

		rels := req.Package.ListRelationships(req.DocumentURI)
		relID = opc.AllocateRelationshipID(rels)
		rels = append(rels, opc.RelationshipInfo{
			SourceURI: req.DocumentURI,
			ID:        relID,
			Type:      relTypeForKind(kind),
			Target:    opc.RelationshipTarget(req.DocumentURI, partURI),
		})
		if err := opc.WriteRelationships(req.Package, req.DocumentURI, rels); err != nil {
			return nil, fmt.Errorf("failed to write relationships for %s: %w", req.DocumentURI, err)
		}
	}

	// Inject the reference into sectPr (cases b and c).
	ref := newElement(prefix, refTag)
	ref.CreateAttr(qualifiedWordAttrName(root, prefix, "type"), refType)
	ensureNamespacePrefix(root, "r", namespaces.NsR)
	ref.CreateAttr(qualifiedRelAttrName(root, "id"), relID)
	insertReference(sectPr, ref)

	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}

	result.PartURI = partURI
	result.ID = relID
	result.CreatedRef = true
	return result, nil
}

func selectSectPr(body *etree.Element, prefix string, sectionIndex int) (*etree.Element, error) {
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
	if len(sections) == 0 {
		// No section properties at all: create a trailing body sectPr.
		sectPr := newElement(prefix, "sectPr")
		body.AddChild(sectPr)
		return sectPr, nil
	}
	if sectionIndex <= 0 {
		return sections[len(sections)-1], nil
	}
	if sectionIndex > len(sections) {
		return nil, fmt.Errorf("section %d out of range (document has %d sections)", sectionIndex, len(sections))
	}
	return sections[sectionIndex-1], nil
}

func findReferenceByType(sectPr *etree.Element, refTag, refType string) *etree.Element {
	for _, ref := range namespaces.FindChildren(sectPr, namespaces.NsW, refTag) {
		t, _ := namespaces.Attr(ref, namespaces.NsW, "type")
		if t == "" {
			t = "default"
		}
		if t == refType {
			return ref
		}
	}
	return nil
}

// unreferencedPart returns a (relID, partURI) for an existing header/footer relationship
// of the given kind whose part is not referenced by any sectPr, or empty strings.
func unreferencedPart(session opc.PackageSession, documentURI string, body *etree.Element, kind string) (string, string) {
	referenced := referencedRelIDs(body)
	wantType := relTypeForKind(kind)
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.Type != wantType || rel.TargetMode == "External" {
			continue
		}
		if referenced[rel.ID] {
			continue
		}
		return rel.ID, opc.ResolveRelationshipTarget(documentURI, rel.Target)
	}
	return "", ""
}

func referencedRelIDs(body *etree.Element) map[string]bool {
	ids := make(map[string]bool)
	for _, sectPr := range namespaces.FindDescendants(body, namespaces.NsW, "sectPr") {
		for _, tag := range []string{"headerReference", "footerReference"} {
			for _, ref := range namespaces.FindChildren(sectPr, namespaces.NsW, tag) {
				if id, ok := namespaces.Attr(ref, namespaces.NsR, "id"); ok {
					ids[id] = true
				}
			}
		}
	}
	return ids
}

func relationshipTarget(session opc.PackageSession, documentURI, id string) string {
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.ID == id {
			return opc.ResolveRelationshipTarget(documentURI, rel.Target)
		}
	}
	return ""
}

// insertReference places a header/footer reference at the front of sectPr, where the
// schema sequences w:headerReference/w:footerReference before page-geometry elements.
func insertReference(sectPr, ref *etree.Element) {
	children := sectPr.ChildElements()
	for _, child := range children {
		switch docxbody.LocalName(child.Tag) {
		case "headerReference", "footerReference":
			continue
		default:
			sectPr.InsertChildAt(child.Index(), ref)
			return
		}
	}
	sectPr.AddChild(ref)
}

// allocatePartURI returns the next free /word/headerN.xml or /word/footerN.xml URI.
func allocatePartURI(session opc.PackageSession, kind string) string {
	prefix := "/word/" + kind
	used := make(map[int]bool)
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		if !strings.HasPrefix(uri, prefix) || !strings.HasSuffix(uri, ".xml") {
			continue
		}
		numStr := strings.TrimSuffix(strings.TrimPrefix(uri, prefix), ".xml")
		if n, err := strconv.Atoi(numStr); err == nil {
			used[n] = true
		}
	}
	nums := make([]int, 0, len(used))
	for n := range used {
		nums = append(nums, n)
	}
	sort.Ints(nums)
	next := 1
	for _, n := range nums {
		if n == next {
			next++
		}
	}
	return fmt.Sprintf("%s%d.xml", prefix, next)
}

func headerFooterTemplate(kind string) []byte {
	tag := "w:hdr"
	if kind == "footer" {
		tag = "w:ftr"
	}
	return []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
		`<` + tag + ` xmlns:w="` + namespaces.NsW + `" xmlns:r="` + namespaces.NsR + `"><w:p/></` + tag + `>`)
}

func contentTypeForKind(kind string) string {
	if kind == "footer" {
		return namespaces.ContentTypeFooter
	}
	return namespaces.ContentTypeHeader
}

func relTypeForKind(kind string) string {
	if kind == "footer" {
		return namespaces.RelFooter
	}
	return namespaces.RelHeader
}

func qualifiedRelAttrName(root *etree.Element, local string) string {
	ensureNamespacePrefix(root, "r", namespaces.NsR)
	return "r:" + local
}
