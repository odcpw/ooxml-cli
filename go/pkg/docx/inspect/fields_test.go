package inspect

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

func parseParagraph(t *testing.T, inner string) *etree.Element {
	t.Helper()
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<w:p xmlns:w="` + namespaces.NsW + `">` + inner + `</w:p>`); err != nil {
		t.Fatalf("read paragraph: %v", err)
	}
	return doc.Root()
}

func TestAppendFieldsSimple(t *testing.T) {
	p := parseParagraph(t, `<w:r><w:t>Page </w:t></w:r>`+
		`<w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>`)
	result := &DocumentFields{}
	appendParagraphFields(result, p, "/word/document.xml", 1, "paragraph", true)
	if len(result.Fields) != 1 {
		t.Fatalf("field count = %d, want 1", len(result.Fields))
	}
	f := result.Fields[0]
	if f.FieldType != FieldTypeSimple {
		t.Fatalf("fieldType = %q, want simple", f.FieldType)
	}
	if !f.Editable {
		t.Fatalf("body paragraph field should be editable")
	}
	if f.Instruction != "PAGE" {
		t.Fatalf("instruction = %q, want PAGE", f.Instruction)
	}
	if f.CachedResult != "1" {
		t.Fatalf("cachedResult = %q, want 1", f.CachedResult)
	}
	if f.Location != "body:1" {
		t.Fatalf("location = %q, want body:1", f.Location)
	}
	if !f.IsStale {
		t.Fatalf("expected IsStale true")
	}
}

func TestAppendFieldsComplex(t *testing.T) {
	p := parseParagraph(t, `<w:r><w:t>Total: </w:t></w:r>`+
		`<w:r><w:fldChar w:fldCharType="begin"/></w:r>`+
		`<w:r><w:instrText xml:space="preserve"> NUMPAGES </w:instrText></w:r>`+
		`<w:r><w:fldChar w:fldCharType="separate"/></w:r>`+
		`<w:r><w:t>3</w:t></w:r>`+
		`<w:r><w:fldChar w:fldCharType="end"/></w:r>`)
	result := &DocumentFields{}
	appendParagraphFields(result, p, "/word/header1.xml", 2, "paragraph", true)
	if len(result.Fields) != 1 {
		t.Fatalf("field count = %d, want 1", len(result.Fields))
	}
	f := result.Fields[0]
	if f.FieldType != FieldTypeComplex {
		t.Fatalf("fieldType = %q, want complex", f.FieldType)
	}
	// Instruction must come from instrText (collectText skips instrText, which would
	// be the silent-empty trap), and result from the separate..end run text.
	if f.Instruction != "NUMPAGES" {
		t.Fatalf("instruction = %q, want NUMPAGES", f.Instruction)
	}
	if f.CachedResult != "3" {
		t.Fatalf("cachedResult = %q, want 3", f.CachedResult)
	}
	if f.Instruction == f.CachedResult {
		t.Fatalf("instruction and result should differ")
	}
	if f.Location != "header1:2" {
		t.Fatalf("location = %q, want header1:2", f.Location)
	}
}

func TestAppendFieldsComplexNested(t *testing.T) {
	// A field with a nested field in its instruction must report only the outer pair.
	p := parseParagraph(t, `<w:r><w:fldChar w:fldCharType="begin"/></w:r>`+
		`<w:r><w:instrText> IF </w:instrText></w:r>`+
		`<w:r><w:fldChar w:fldCharType="begin"/></w:r>`+
		`<w:r><w:instrText> PAGE </w:instrText></w:r>`+
		`<w:r><w:fldChar w:fldCharType="end"/></w:r>`+
		`<w:r><w:fldChar w:fldCharType="separate"/></w:r>`+
		`<w:r><w:t>yes</w:t></w:r>`+
		`<w:r><w:fldChar w:fldCharType="end"/></w:r>`)
	result := &DocumentFields{}
	appendParagraphFields(result, p, "/word/document.xml", 1, "paragraph", true)
	if len(result.Fields) != 1 {
		t.Fatalf("complex field count = %d, want 1", len(result.Fields))
	}
	if result.Fields[0].CachedResult != "yes" {
		t.Fatalf("result = %q, want yes", result.Fields[0].CachedResult)
	}
}

// TestAppendFieldsDocumentOrder asserts simple and complex fields are emitted in true
// document order (complex-then-simple here), so the per-paragraph index lines up with
// the mutate layer's locateFieldsInParagraph selector order. This is the core of
// Finding A: the old "all simple, then all complex" emission silently mis-targeted.
func TestAppendFieldsDocumentOrder(t *testing.T) {
	p := parseParagraph(t,
		// Complex field first.
		`<w:r><w:fldChar w:fldCharType="begin"/></w:r>`+
			`<w:r><w:instrText> NUMPAGES </w:instrText></w:r>`+
			`<w:r><w:fldChar w:fldCharType="separate"/></w:r>`+
			`<w:r><w:t>3</w:t></w:r>`+
			`<w:r><w:fldChar w:fldCharType="end"/></w:r>`+
			// Simple field second.
			`<w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>`)
	result := &DocumentFields{}
	appendParagraphFields(result, p, "/word/document.xml", 1, "paragraph", true)
	if len(result.Fields) != 2 {
		t.Fatalf("field count = %d, want 2", len(result.Fields))
	}
	if result.Fields[0].FieldType != FieldTypeComplex || result.Fields[0].Instruction != "NUMPAGES" {
		t.Fatalf("field[0] = %+v, want complex NUMPAGES first", result.Fields[0])
	}
	if result.Fields[1].FieldType != FieldTypeSimple || result.Fields[1].Instruction != "PAGE" {
		t.Fatalf("field[1] = %+v, want simple PAGE second", result.Fields[1])
	}
}

func TestAppendFieldsNoFields(t *testing.T) {
	p := parseParagraph(t, `<w:r><w:t>just text</w:t></w:r>`)
	result := &DocumentFields{}
	appendParagraphFields(result, p, "/word/document.xml", 1, "paragraph", true)
	if len(result.Fields) != 0 {
		t.Fatalf("field count = %d, want 0", len(result.Fields))
	}
}

func TestPartLocationLabel(t *testing.T) {
	cases := map[string]string{
		"/word/header1.xml": "header1",
		"/word/footer2.xml": "footer2",
		"header1.xml":       "header1",
	}
	for in, want := range cases {
		if got := partLocationLabel(in); got != want {
			t.Fatalf("partLocationLabel(%q) = %q, want %q", in, got, want)
		}
	}
}
