package mutate

import (
	"testing"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestSetHeaderFooterTextExisting(t *testing.T) {
	pkg, _ := openFixture(t, "headers")
	defer pkg.Close()

	result, err := SetHeaderFooterText(&SetHeaderFooterTextRequest{
		Package:        pkg,
		PartURI:        "/word/header1.xml",
		ParagraphIndex: 1,
		Text:           "Updated Header",
	})
	if err != nil {
		t.Fatalf("SetHeaderFooterText: %v", err)
	}
	if result.PreviousText != "Page Header" || result.Text != "Updated Header" {
		t.Fatalf("result = %+v", result)
	}
	paras, err := docxinspect.ReadHeaderFooterParagraphs(pkg, "/word/header1.xml")
	if err != nil {
		t.Fatalf("readback: %v", err)
	}
	if len(paras) != 1 || paras[0].Text != "Updated Header" {
		t.Fatalf("readback paras = %+v", paras)
	}
}

func TestSetHeaderFooterTextOutOfRange(t *testing.T) {
	pkg, _ := openFixture(t, "headers")
	defer pkg.Close()

	_, err := SetHeaderFooterText(&SetHeaderFooterTextRequest{
		Package:        pkg,
		PartURI:        "/word/header1.xml",
		ParagraphIndex: 9,
		Text:           "x",
	})
	if err == nil {
		t.Fatalf("expected out-of-range error")
	}
}

func TestEnsureHeaderFooterCreatesPart(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	result, err := EnsureHeaderFooter(&EnsureHeaderFooterRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Kind:        "header",
		Type:        "default",
	})
	if err != nil {
		t.Fatalf("EnsureHeaderFooter: %v", err)
	}
	if !result.CreatedPart || !result.CreatedRef {
		t.Fatalf("expected created part and ref, got %+v", result)
	}
	if result.PartURI != "/word/header1.xml" {
		t.Fatalf("partURI = %q", result.PartURI)
	}

	// Part registered with correct content type.
	if ct := pkg.GetContentType(result.PartURI); ct != namespaces.ContentTypeHeader {
		t.Fatalf("content type = %q", ct)
	}
	// Relationship written on document part.
	if rel := findRel(pkg, documentURI, result.ID); rel == nil || rel.Type != namespaces.RelHeader {
		t.Fatalf("relationship not written for %s", result.ID)
	}
	// sectPr reference present.
	listing, err := docxinspect.ListHeadersFooters(pkg, documentURI)
	if err != nil {
		t.Fatalf("list: %v", err)
	}
	if listing.Sections[0].Headers.Default == nil {
		t.Fatalf("default header reference not injected")
	}
}

func TestEnsureHeaderFooterAddsRefToExistingPart(t *testing.T) {
	// with-media has header1.xml + relationship but an empty sectPr (no reference).
	pkg, documentURI := openFixture(t, "with-media")
	defer pkg.Close()

	result, err := EnsureHeaderFooter(&EnsureHeaderFooterRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Kind:        "header",
		Type:        "default",
	})
	if err != nil {
		t.Fatalf("EnsureHeaderFooter: %v", err)
	}
	if result.CreatedPart {
		t.Fatalf("should reuse existing part, got created: %+v", result)
	}
	if !result.CreatedRef {
		t.Fatalf("expected a new reference, got %+v", result)
	}
	if result.PartURI != "/word/header1.xml" || result.ID != "rHeader" {
		t.Fatalf("unexpected result %+v", result)
	}
	// No duplicate relationship created.
	count := 0
	for _, rel := range pkg.ListRelationships(documentURI) {
		if rel.Type == namespaces.RelHeader {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("header relationship count = %d, want 1", count)
	}
}

func TestEnsureHeaderFooterIdempotent(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	first, err := EnsureHeaderFooter(&EnsureHeaderFooterRequest{
		Package: pkg, DocumentURI: documentURI, Kind: "header", Type: "default",
	})
	if err != nil {
		t.Fatalf("first ensure: %v", err)
	}
	second, err := EnsureHeaderFooter(&EnsureHeaderFooterRequest{
		Package: pkg, DocumentURI: documentURI, Kind: "header", Type: "default",
	})
	if err != nil {
		t.Fatalf("second ensure: %v", err)
	}
	if second.CreatedPart || second.CreatedRef {
		t.Fatalf("second ensure should be a no-op, got %+v", second)
	}
	if second.ID != first.ID || second.PartURI != first.PartURI {
		t.Fatalf("ids diverged: %+v vs %+v", first, second)
	}
}

func TestEnsureHeaderFooterIndependentNumbering(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	h, err := EnsureHeaderFooter(&EnsureHeaderFooterRequest{
		Package: pkg, DocumentURI: documentURI, Kind: "header", Type: "default",
	})
	if err != nil {
		t.Fatalf("header ensure: %v", err)
	}
	f, err := EnsureHeaderFooter(&EnsureHeaderFooterRequest{
		Package: pkg, DocumentURI: documentURI, Kind: "footer", Type: "default",
	})
	if err != nil {
		t.Fatalf("footer ensure: %v", err)
	}
	if h.PartURI != "/word/header1.xml" || f.PartURI != "/word/footer1.xml" {
		t.Fatalf("unexpected part URIs: header=%q footer=%q", h.PartURI, f.PartURI)
	}
}

func findRel(pkg opc.PackageSession, sourceURI, id string) *opc.RelationshipInfo {
	for _, rel := range pkg.ListRelationships(sourceURI) {
		if rel.ID == id {
			r := rel
			return &r
		}
	}
	return nil
}
