package inspect

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestSummarizeMinimalDocument(t *testing.T) {
	pkg := openDOCXFixture(t, "minimal")
	defer pkg.Close()

	summary, err := SummarizeDocument(pkg)
	if err != nil {
		t.Fatalf("SummarizeDocument returned error: %v", err)
	}

	if summary.Type != string(opc.PackageTypeDOCX) {
		t.Fatalf("type = %q, want docx", summary.Type)
	}
	if summary.DocumentPartURI != "/word/document.xml" {
		t.Fatalf("document URI = %q, want /word/document.xml", summary.DocumentPartURI)
	}
	if summary.Paragraphs != 1 || summary.Tables != 0 || summary.Sections != 1 {
		t.Fatalf("summary counts = %+v, want 1 paragraph, 0 tables, 1 section", summary)
	}
}

func TestSummarizeDocumentCountsRelationshipsAndParts(t *testing.T) {
	pkg := openDOCXFixture(t, "with-media")
	defer pkg.Close()

	summary, err := SummarizeDocument(pkg)
	if err != nil {
		t.Fatalf("SummarizeDocument returned error: %v", err)
	}

	if summary.Headers != 1 || summary.Footers != 1 || summary.MediaAssets != 1 {
		t.Fatalf("summary = %+v, want one header, footer, and media asset", summary)
	}
}

func TestFindMainDocumentPartErrorsWhenMissing(t *testing.T) {
	pkg := openDOCXFixture(t, "corrupted-missing-document")
	defer pkg.Close()

	uri, err := FindMainDocumentPart(pkg)
	if err != nil {
		t.Fatalf("FindMainDocumentPart returned error: %v", err)
	}
	if uri != "/word/document.xml" {
		t.Fatalf("document URI = %q, want /word/document.xml", uri)
	}

	if _, err := ParseDocument(pkg); err == nil {
		t.Fatal("ParseDocument returned nil error for missing document part")
	}
}

func openDOCXFixture(t *testing.T, name string) *opc.Package {
	t.Helper()
	path := filepath.Join("..", "..", "..", "testdata", "docx", name, "document.docx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture %s: %v", name, err)
	}
	return pkg
}
