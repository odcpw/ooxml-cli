package extract

import (
	"path/filepath"
	"strings"
	"testing"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestExtractBlocksReportsIDsHashesAndParagraphRuns(t *testing.T) {
	doc := extractBlockFixture(t, "mixed-blocks", 2, true)

	if len(doc.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(doc.Blocks))
	}
	block := doc.Blocks[0]
	if block.ID != "body.b2" || block.Index != 2 || block.Kind != model.BlockKindParagraph || block.Text != "Bold heading" {
		t.Fatalf("unexpected block: %+v", block)
	}
	if !strings.HasPrefix(block.ContentHash, "sha256:") || len(block.ContentHash) != len("sha256:")+64 {
		t.Fatalf("content hash = %q, want sha256 hex", block.ContentHash)
	}
	if block.Paragraph == nil || block.Paragraph.Style != "Heading1" {
		t.Fatalf("paragraph info = %+v, want Heading1", block.Paragraph)
	}
	if len(block.Paragraph.Runs) != 1 || block.Paragraph.Runs[0].Text != "Bold heading" || !block.Paragraph.Runs[0].Bold {
		t.Fatalf("runs = %+v, want one bold run", block.Paragraph.Runs)
	}
}

func TestExtractBlocksReportsTables(t *testing.T) {
	doc := extractBlockFixture(t, "table", 0, false)

	if len(doc.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(doc.Blocks))
	}
	block := doc.Blocks[0]
	if block.ID != "body.b1" || block.Kind != model.BlockKindTable || block.Text != "A1\tB1\nA2\tB2" {
		t.Fatalf("unexpected table block: %+v", block)
	}
	if block.Table == nil || len(block.Table.Rows) != 2 || len(block.Table.Rows[0].Cells) != 2 {
		t.Fatalf("table info = %+v, want 2x2", block.Table)
	}
	if block.Table.Rows[1].Cells[0].Text != "A2" {
		t.Fatalf("table cell text = %q, want A2", block.Table.Rows[1].Cells[0].Text)
	}
}

func TestExtractBlocksMissingBlockReturnsError(t *testing.T) {
	path := filepath.Join("..", "..", "..", "testdata", "docx", "minimal", "document.docx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer pkg.Close()
	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		t.Fatalf("FindMainDocumentPart returned error: %v", err)
	}
	if _, err := ExtractBlocks(&ExtractBlocksRequest{Session: pkg, DocumentURI: documentURI, Block: 99}); err == nil {
		t.Fatal("ExtractBlocks expected missing block error")
	}
}

func extractBlockFixture(t *testing.T, name string, block int, includeRuns bool) *ExtractedBlocks {
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
	doc, err := ExtractBlocks(&ExtractBlocksRequest{
		Session:     pkg,
		DocumentURI: documentURI,
		Block:       block,
		IncludeRuns: includeRuns,
	})
	if err != nil {
		t.Fatalf("ExtractBlocks returned error: %v", err)
	}
	return doc
}
