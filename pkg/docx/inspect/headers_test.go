package inspect

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

func TestExtractHeaderFooterParagraphs(t *testing.T) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<w:hdr xmlns:w="` + namespaces.NsW + `">` +
		`<w:p><w:pPr><w:pStyle w:val="Header"/></w:pPr><w:r><w:t>Hello</w:t></w:r></w:p>` +
		`<w:p><w:r><w:t>World</w:t></w:r></w:p></w:hdr>`); err != nil {
		t.Fatalf("read hdr: %v", err)
	}
	paras := ExtractHeaderFooterParagraphs(doc.Root())
	if len(paras) != 2 {
		t.Fatalf("paragraph count = %d, want 2", len(paras))
	}
	if paras[0].Index != 1 || paras[0].Text != "Hello" || paras[0].Style != "Header" {
		t.Fatalf("paragraph 0 = %+v", paras[0])
	}
	if paras[1].Index != 2 || paras[1].Text != "World" || paras[1].Style != "" {
		t.Fatalf("paragraph 1 = %+v", paras[1])
	}
}

func TestAssignAndSelectByType(t *testing.T) {
	set := &HeaderFooterSet{}
	assignRef(set, &HeaderFooterRef{Type: TypeDefault, ID: "d"})
	assignRef(set, &HeaderFooterRef{Type: TypeFirst, ID: "f"})
	assignRef(set, &HeaderFooterRef{Type: TypeEven, ID: "e"})

	if set.Default == nil || set.Default.ID != "d" {
		t.Fatalf("default = %+v", set.Default)
	}
	if set.First == nil || set.First.ID != "f" {
		t.Fatalf("first = %+v", set.First)
	}
	if set.Even == nil || set.Even.ID != "e" {
		t.Fatalf("even = %+v", set.Even)
	}
	if got := selectByType(set, TypeFirst); got == nil || got.ID != "f" {
		t.Fatalf("selectByType first = %+v", got)
	}
	if got := selectByType(set, TypeEven); got == nil || got.ID != "e" {
		t.Fatalf("selectByType even = %+v", got)
	}
	if got := selectByType(set, TypeDefault); got == nil || got.ID != "d" {
		t.Fatalf("selectByType default = %+v", got)
	}
}

func TestHeaderFooterSelectors(t *testing.T) {
	ref := &HeaderFooterRef{
		Kind:            KindHeader,
		ID:              "rId10",
		Type:            TypeDefault,
		Section:         1,
		PrimarySelector: HeaderFooterPrimarySelector(KindHeader, 1, TypeDefault),
		Selectors:       HeaderFooterSelectors(KindHeader, 1, TypeDefault, "rId10", "/word/header1.xml"),
		PartURI:         "/word/header1.xml",
	}
	if ref.PrimarySelector != "header:1:default" {
		t.Fatalf("primary selector = %q", ref.PrimarySelector)
	}
	for _, want := range []string{"header:1:default", "id:rId10", "rId10", "part:/word/header1.xml", "/word/header1.xml"} {
		if !containsHeaderFooterSelector(ref.Selectors, want) {
			t.Fatalf("selectors missing %q: %+v", want, ref.Selectors)
		}
	}
	if containsHeaderFooterSelector(ref.Selectors, "section:1:type:default") {
		t.Fatalf("ambiguous section-only selector should not be emitted: %+v", ref.Selectors)
	}

	paras := AnnotateHeaderFooterParagraphs(ref, []HeaderFooterParagraph{{Index: 1, Text: "Page Header"}})
	if len(paras) != 1 || paras[0].PrimarySelector != "header:1:default/p:1" {
		t.Fatalf("paragraph selector not annotated: %+v", paras)
	}
	if !containsHeaderFooterSelector(paras[0].Selectors, "header:1:default/paragraph:1") {
		t.Fatalf("paragraph selectors missing alias: %+v", paras[0].Selectors)
	}
}

func containsHeaderFooterSelector(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func TestSectionPropertiesInlineAndTrailing(t *testing.T) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<w:document xmlns:w="` + namespaces.NsW + `"><w:body>` +
		`<w:p><w:pPr><w:sectPr/></w:pPr></w:p>` +
		`<w:p/>` +
		`<w:sectPr/>` +
		`</w:body></w:document>`); err != nil {
		t.Fatalf("read document: %v", err)
	}
	body := doc.Root().SelectElement("w:body")
	sections := sectionProperties(body)
	if len(sections) != 2 {
		t.Fatalf("section count = %d, want 2", len(sections))
	}
}
