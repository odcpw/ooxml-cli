package inspect

import (
	"strconv"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// Field type discriminators.
const (
	// FieldTypeSimple is a self-contained w:fldSimple field.
	FieldTypeSimple = "simple"
	// FieldTypeComplex is a w:fldChar-delimited complex field (begin/separate/end).
	FieldTypeComplex = "complex"
)

// Field describes a single document field (simple or complex) and its location.
//
// CachedResult is the result text Word last computed for the field; it is a cache
// that only refreshes when Word recalculates fields, so IsStale is always true for
// fields read by this tool.
type Field struct {
	Index        int    `json:"index"`
	PartURI      string `json:"partUri"`
	BlockIndex   int    `json:"blockIndex"`
	BlockKind    string `json:"blockKind"`
	FieldType    string `json:"fieldType"`
	Instruction  string `json:"instruction"`
	CachedResult string `json:"cachedResult"`
	Location     string `json:"location"`
	IsStale      bool   `json:"isStale"`
	// Editable reports whether `docx fields set-result` / `docx fields insert` can
	// address this field with the current selector grammar (part:block:field). Fields
	// nested inside body tables are listed for visibility but are NOT addressable, so
	// they are reported with Editable=false to prevent silent mis-targeting.
	Editable bool `json:"editable"`
}

// DocumentFields is the top-level listing returned by ListFields.
type DocumentFields struct {
	DocumentPartURI string  `json:"documentPartUri"`
	Fields          []Field `json:"fields"`
}

// ListFields enumerates every w:fldSimple and complex (w:fldChar) field in the
// document body and in every referenced header/footer part. Fields are returned in
// document order: body first, then header/footer parts in section order.
func ListFields(session opc.PackageSession, documentURI string) (*DocumentFields, error) {
	result := &DocumentFields{
		DocumentPartURI: documentURI,
		Fields:          make([]Field, 0),
	}

	// Body fields.
	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, err
	}
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		return nil, err
	}
	for _, block := range docxbody.Blocks(body) {
		switch string(block.Kind) {
		case "table":
			// Table-nested fields are listed for visibility but are NOT addressable by
			// the part:block:field selector, so they are flagged non-editable.
			for _, p := range namespaces.FindDescendants(block.Element, namespaces.NsW, "p") {
				appendParagraphFields(result, p, documentURI, block.Index, string(block.Kind), false)
			}
		default:
			appendParagraphFields(result, block.Element, documentURI, block.Index, string(block.Kind), true)
		}
	}

	// Header/footer fields.
	for _, partURI := range headerFooterPartURIs(session, documentURI) {
		hfDoc, err := session.ReadXMLPart(partURI)
		if err != nil {
			continue
		}
		root := hfDoc.Root()
		if root == nil {
			continue
		}
		for i, p := range namespaces.FindChildren(root, namespaces.NsW, "p") {
			appendParagraphFields(result, p, partURI, i+1, "paragraph", true)
		}
	}

	// Assign stable indices and locations.
	for i := range result.Fields {
		result.Fields[i].Index = i
	}
	return result, nil
}

// appendParagraphFields walks one paragraph's direct children in document order and
// appends each simple (w:fldSimple) and complex (w:fldChar begin/separate/end) field
// at its true position. This mirrors the mutate layer's locateFieldsInParagraph walk
// (it cannot be reused directly: inspect cannot import mutate without an import cycle),
// so the i-th field listed here is the same field the set-result selector resolves at
// field index i within this paragraph. The editable flag is propagated to each field.
func appendParagraphFields(result *DocumentFields, paragraph *etree.Element, partURI string, blockIndex int, blockKind string, editable bool) {
	loc := fieldLocation(partURI, blockIndex)
	emit := func(fieldType, instruction, cached string) {
		result.Fields = append(result.Fields, Field{
			PartURI:      partURI,
			BlockIndex:   blockIndex,
			BlockKind:    blockKind,
			FieldType:    fieldType,
			Instruction:  strings.TrimSpace(instruction),
			CachedResult: cached,
			Location:     loc,
			IsStale:      true,
			Editable:     editable,
		})
	}

	var (
		inField   bool
		afterSep  bool
		depth     int
		curInstr  strings.Builder
		curResult strings.Builder
	)
	for _, child := range paragraph.ChildElements() {
		name := docxbody.LocalName(child.Tag)
		if name == "fldSimple" && depth == 0 {
			instr, _ := namespaces.Attr(child, namespaces.NsW, "instr")
			emit(FieldTypeSimple, instr, fldSimpleResultText(child))
			continue
		}
		if name != "r" {
			continue
		}
		for _, runChild := range child.ChildElements() {
			switch docxbody.LocalName(runChild.Tag) {
			case "fldChar":
				ct, _ := namespaces.Attr(runChild, namespaces.NsW, "fldCharType")
				switch ct {
				case "begin":
					if !inField {
						inField = true
						afterSep = false
						depth = 1
						curInstr.Reset()
						curResult.Reset()
					} else {
						depth++
					}
				case "separate":
					if inField && depth == 1 {
						afterSep = true
					}
				case "end":
					if inField {
						depth--
						if depth == 0 {
							emit(FieldTypeComplex, curInstr.String(), curResult.String())
							inField = false
							afterSep = false
						}
					}
				}
			case "instrText":
				if inField && depth == 1 && !afterSep {
					curInstr.WriteString(runChild.Text())
				}
			case "t":
				if inField && depth == 1 && afterSep {
					curResult.WriteString(runChild.Text())
				}
			}
		}
	}
}

// fldSimpleResultText collects the run text of a w:fldSimple element. Unlike
// body.ParagraphText this stays scoped to the field's own runs.
func fldSimpleResultText(fld *etree.Element) string {
	var b strings.Builder
	for _, t := range namespaces.FindDescendants(fld, namespaces.NsW, "t") {
		b.WriteString(t.Text())
	}
	return b.String()
}

// headerFooterPartURIs returns the part URIs of every referenced header/footer,
// de-duplicated and in section order.
func headerFooterPartURIs(session opc.PackageSession, documentURI string) []string {
	listing, err := ListHeadersFooters(session, documentURI)
	if err != nil {
		return nil
	}
	var uris []string
	seen := make(map[string]bool)
	add := func(ref *HeaderFooterRef) {
		if ref == nil || ref.PartURI == "" || seen[ref.PartURI] {
			return
		}
		seen[ref.PartURI] = true
		uris = append(uris, ref.PartURI)
	}
	for _, section := range listing.Sections {
		for _, set := range []*HeaderFooterSet{section.Headers, section.Footers} {
			if set == nil {
				continue
			}
			add(set.Default)
			add(set.First)
			add(set.Even)
		}
	}
	return uris
}

// fieldLocation renders a "body:<n>" or "<part>:<n>" location string for a field.
func fieldLocation(partURI string, blockIndex int) string {
	prefix := "body"
	if partURI != "" && !strings.HasSuffix(partURI, "/document.xml") {
		prefix = partLocationLabel(partURI)
	}
	return prefix + ":" + strconv.Itoa(blockIndex)
}

// partLocationLabel derives a short label (e.g. "header1") from a part URI.
func partLocationLabel(partURI string) string {
	name := partURI
	if idx := strings.LastIndex(name, "/"); idx >= 0 {
		name = name[idx+1:]
	}
	name = strings.TrimSuffix(name, ".xml")
	return name
}
