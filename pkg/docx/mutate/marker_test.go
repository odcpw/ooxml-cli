package mutate

import (
	"testing"

	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// readParaIDForBlock returns the w14:paraId on the 1-based body block index.
func readParaIDForBlock(t *testing.T, pkg opc.PackageSession, documentURI string, index int) string {
	t.Helper()
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart: %v", err)
	}
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		t.Fatalf("FindBody: %v", err)
	}
	for _, b := range docxbody.Blocks(body) {
		if b.Index == index {
			return docxhandle.ReadParaID(b.Element)
		}
	}
	return ""
}

func countMarkedParagraphs(t *testing.T, pkg opc.PackageSession, documentURI string) int {
	t.Helper()
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart: %v", err)
	}
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		t.Fatalf("FindBody: %v", err)
	}
	return len(docxhandle.CollectParaIDs(body))
}

// TestSetParagraphInjectsExactlyOneMarkerAndIsIdempotent proves the INJECTION
// contract: mutating a marker-less paragraph injects exactly one w14:paraId,
// returns it, the w14 namespace is declared, and re-running does NOT add a
// second marker or churn the id.
func TestSetParagraphInjectsExactlyOneMarkerAndIsIdempotent(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	first, err := SetParagraphText(&SetParagraphTextRequest{Package: pkg, DocumentURI: documentURI, Index: 1, Text: "first"})
	if err != nil {
		t.Fatalf("SetParagraphText: %v", err)
	}
	if first.ParaID == "" {
		t.Fatal("expected an injected paraId on first mutate")
	}
	if got := countMarkedParagraphs(t, pkg, documentURI); got != 1 {
		t.Fatalf("marked paragraphs after first mutate = %d, want 1", got)
	}

	// w14 namespace must be declared on the document root.
	doc, _ := pkg.ReadXMLPart(documentURI)
	if attr := doc.Root().SelectAttr("xmlns:w14"); attr == nil || attr.Value != namespaces.NsW14 {
		t.Fatalf("xmlns:w14 not declared on root: %v", attr)
	}

	// Re-run the same mutate: the id must not churn and no second marker appears.
	second, err := SetParagraphText(&SetParagraphTextRequest{Package: pkg, DocumentURI: documentURI, Index: 1, Text: "second"})
	if err != nil {
		t.Fatalf("SetParagraphText (re-run): %v", err)
	}
	if second.ParaID != first.ParaID {
		t.Fatalf("paraId churned: first=%q second=%q", first.ParaID, second.ParaID)
	}
	if got := countMarkedParagraphs(t, pkg, documentURI); got != 1 {
		t.Fatalf("marked paragraphs after re-run = %d, want 1 (no second marker)", got)
	}
}

// TestParagraphHandleSurvivesTranslation proves the TRANSLATION case: after a
// paragraph is stamped, editing ITS OWN TEXT leaves the marker intact, so the
// SAME handle still resolves to that paragraph (the address is the marker attr,
// not the text).
func TestParagraphHandleSurvivesTranslation(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	stamp, err := SetParagraphText(&SetParagraphTextRequest{Package: pkg, DocumentURI: documentURI, Index: 1, Text: "before translation"})
	if err != nil {
		t.Fatalf("stamp: %v", err)
	}
	paraID := stamp.ParaID

	// Translate (edit text) by index again; the marker must be unchanged.
	if _, err := SetParagraphText(&SetParagraphTextRequest{Package: pkg, DocumentURI: documentURI, Index: 1, Text: "después de la traducción"}); err != nil {
		t.Fatalf("translate: %v", err)
	}

	doc, _ := pkg.ReadXMLPart(documentURI)
	body, _ := docxbody.FindBody(doc.Root())
	idx, elem, rerr := docxhandle.ResolveParagraphBlock(body, docxhandle.Handle{Kind: docxhandle.KindParagraph, ParaID: paraID})
	if rerr != nil {
		t.Fatalf("handle did not survive translation: %v", rerr)
	}
	if idx != 1 || docxbody.ParagraphText(elem) != "después de la traducción" {
		t.Fatalf("resolved wrong paragraph: idx=%d text=%q", idx, docxbody.ParagraphText(elem))
	}
}

// TestParagraphHandleSurvivesStructuralInsert proves the STRUCTURAL case: after
// stamping a paragraph and inserting another block ABOVE it, the same marker
// handle resolves to the SAME paragraph at its new (shifted) index — resolution
// is search-by-marker, not by index.
func TestParagraphHandleSurvivesStructuralInsert(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	stamp, err := SetParagraphText(&SetParagraphTextRequest{Package: pkg, DocumentURI: documentURI, Index: 1, Text: "target paragraph"})
	if err != nil {
		t.Fatalf("stamp: %v", err)
	}
	paraID := stamp.ParaID

	// Prepend a new paragraph (unrelated structural edit).
	if _, err := InsertParagraph(&InsertParagraphRequest{Package: pkg, DocumentURI: documentURI, AfterIndex: 0, Text: "new top"}); err != nil {
		t.Fatalf("InsertParagraph: %v", err)
	}

	doc, _ := pkg.ReadXMLPart(documentURI)
	body, _ := docxbody.FindBody(doc.Root())
	idx, elem, rerr := docxhandle.ResolveParagraphBlock(body, docxhandle.Handle{Kind: docxhandle.KindParagraph, ParaID: paraID})
	if rerr != nil {
		t.Fatalf("handle did not survive structural insert: %v", rerr)
	}
	if idx != 2 {
		t.Fatalf("resolved index = %d, want 2 (shifted by prepend)", idx)
	}
	if got := docxbody.ParagraphText(elem); got != "target paragraph" {
		t.Fatalf("resolved wrong paragraph text = %q", got)
	}
}

// TestInspectDoesNotInject proves the read path NEVER injects: collecting paraIds
// on a marker-less fixture finds zero, and reading a block report does not write
// a marker into the file.
func TestInspectDoesNotInject(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	if got := countMarkedParagraphs(t, pkg, documentURI); got != 0 {
		t.Fatalf("minimal fixture should have no markers, got %d", got)
	}
	// The pure-read ReadParaID must return "" without mutating.
	if id := readParaIDForBlock(t, pkg, documentURI, 1); id != "" {
		t.Fatalf("read path returned a marker %q on a marker-less paragraph", id)
	}
	if got := countMarkedParagraphs(t, pkg, documentURI); got != 0 {
		t.Fatalf("read path injected a marker (count=%d)", got)
	}
}
