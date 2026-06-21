package extract

import (
	"path/filepath"
	"testing"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestExtractTextCapturesParagraphsAndStyles(t *testing.T) {
	doc := extractFixture(t, "styled-headings")

	if len(doc.Blocks) != 2 {
		t.Fatalf("block count = %d, want 2", len(doc.Blocks))
	}
	if doc.Blocks[0].Kind != model.BlockKindParagraph ||
		doc.Blocks[0].Style != "Heading1" ||
		doc.Blocks[0].Text != "Heading Text" {
		t.Fatalf("first block = %+v, want Heading1 paragraph", doc.Blocks[0])
	}
	if doc.Blocks[1].Text != "Body text" {
		t.Fatalf("second block text = %q, want Body text", doc.Blocks[1].Text)
	}
}

func TestExtractTextCapturesHyperlinkNestedText(t *testing.T) {
	doc := extractFixture(t, "hyperlink")

	if len(doc.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(doc.Blocks))
	}
	if got, want := doc.Blocks[0].Text, "Before link text after"; got != want {
		t.Fatalf("text = %q, want %q", got, want)
	}
}

func TestExtractTextPreservesWhitespaceTabsAndBreaks(t *testing.T) {
	doc := extractFixture(t, "space-preserve")

	if len(doc.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(doc.Blocks))
	}
	if got, want := doc.Blocks[0].Text, " pad \ttabbed\nline"; got != want {
		t.Fatalf("text = %q, want %q", got, want)
	}
}

func TestExtractTextFlattensTables(t *testing.T) {
	doc := extractFixture(t, "table")

	if len(doc.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(doc.Blocks))
	}
	block := doc.Blocks[0]
	if block.Kind != model.BlockKindTable || block.Text != "A1\tB1\nA2\tB2" {
		t.Fatalf("table block = %+v, want flattened table", block)
	}
	if block.Table == nil || len(block.Table.Rows) != 2 || len(block.Table.Rows[0].Cells) != 2 {
		t.Fatalf("table structure = %+v, want 2x2", block.Table)
	}
}

func extractFixture(t *testing.T, name string) *ExtractedDocument {
	t.Helper()
	path := filepath.Join("..", "..", "..", "testdata", "docx", name, "document.docx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture %s: %v", name, err)
	}
	defer pkg.Close()

	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		t.Fatalf("FindMainDocumentPart returned error: %v", err)
	}
	doc, err := ExtractText(&ExtractTextRequest{Session: pkg, DocumentURI: documentURI})
	if err != nil {
		t.Fatalf("ExtractText returned error: %v", err)
	}
	return doc
}
