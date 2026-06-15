package inspect

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

func TestFindStylesPartReturnsURI(t *testing.T) {
	pkg := openDOCXFixture(t, "styles-catalog")
	defer pkg.Close()

	uri, err := FindStylesPart(pkg)
	if err != nil {
		t.Fatalf("FindStylesPart returned error: %v", err)
	}
	if uri != "/word/styles.xml" {
		t.Fatalf("styles URI = %q, want /word/styles.xml", uri)
	}
}

func TestFindStylesPartNotFound(t *testing.T) {
	pkg := openDOCXFixture(t, "minimal")
	defer pkg.Close()

	uri, err := FindStylesPart(pkg)
	if err != nil {
		t.Fatalf("FindStylesPart returned error: %v", err)
	}
	if uri != "" {
		t.Fatalf("styles URI = %q, want empty string for missing styles part", uri)
	}
}

func TestParseStylesReturnsElements(t *testing.T) {
	pkg := openDOCXFixture(t, "styles-catalog")
	defer pkg.Close()

	styles, err := ParseStyles(pkg, "/word/styles.xml")
	if err != nil {
		t.Fatalf("ParseStyles returned error: %v", err)
	}
	if len(styles) != 9 {
		t.Fatalf("style count = %d, want 9", len(styles))
	}
}

func TestReportStyleExtractsAll(t *testing.T) {
	pkg := openDOCXFixture(t, "styles-catalog")
	defer pkg.Close()

	styles, err := ParseStyles(pkg, "/word/styles.xml")
	if err != nil {
		t.Fatalf("ParseStyles returned error: %v", err)
	}

	heading, ok := FindStyle(styles, "Heading1")
	if !ok {
		t.Fatal("Heading1 not found")
	}
	if heading.Name != "heading 1" || heading.Type != "paragraph" {
		t.Fatalf("Heading1 name/type = %q/%q", heading.Name, heading.Type)
	}
	if heading.Handle == "" {
		t.Fatal("Heading1 handle is empty, want a unique style handle")
	}
	if heading.BasedOn != "Normal" || heading.Next != "BodyText" {
		t.Fatalf("Heading1 basedOn/next = %q/%q", heading.BasedOn, heading.Next)
	}
	if heading.Default || !heading.Builtin {
		t.Fatalf("Heading1 default=%t builtin=%t, want false/true", heading.Default, heading.Builtin)
	}

	normal, ok := FindStyle(styles, "Normal")
	if !ok {
		t.Fatal("Normal not found")
	}
	if !normal.Default {
		t.Fatal("Normal default = false, want true")
	}

	custom, ok := FindStyle(styles, "MyCustomPara")
	if !ok {
		t.Fatal("MyCustomPara not found")
	}
	if custom.Builtin {
		t.Fatal("MyCustomPara builtin = true, want false (customStyle)")
	}

	numbering, ok := FindStyle(styles, "NoList")
	if !ok {
		t.Fatal("NoList not found")
	}
	if numbering.Type != "numbering" {
		t.Fatalf("NoList type = %q, want numbering", numbering.Type)
	}
}

func TestReportStyleOmitsHandleForDuplicateStyleID(t *testing.T) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:style w:type="paragraph" w:styleId="DupStyle"><w:name w:val="First"/></w:style>
		<w:style w:type="paragraph" w:styleId="DupStyle"><w:name w:val="Second"/></w:style>
		<w:style w:type="paragraph" w:styleId="UniqueStyle"><w:name w:val="Third"/></w:style>
	</w:styles>`); err != nil {
		t.Fatalf("parse styles XML: %v", err)
	}

	elems := namespaces.FindChildren(doc.Root(), namespaces.NsW, "style")
	counts := countStyleIDs(elems)
	first := reportStyle(elems[0], counts)
	second := reportStyle(elems[1], counts)
	unique := reportStyle(elems[2], counts)

	if first.Handle != "" || second.Handle != "" {
		t.Fatalf("duplicate style handles = %q/%q, want both empty", first.Handle, second.Handle)
	}
	if unique.Handle == "" {
		t.Fatal("unique style handle is empty")
	}
}

func TestFindStyleNotFound(t *testing.T) {
	pkg := openDOCXFixture(t, "styles-catalog")
	defer pkg.Close()

	styles, err := ParseStyles(pkg, "/word/styles.xml")
	if err != nil {
		t.Fatalf("ParseStyles returned error: %v", err)
	}
	if _, ok := FindStyle(styles, "DoesNotExist"); ok {
		t.Fatal("FindStyle returned ok for nonexistent style")
	}
}
