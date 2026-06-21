package body

import (
	"path/filepath"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestBlocksIndexesParagraphsAndTablesOnly(t *testing.T) {
	bodyElem := fixtureBody(t, "mixed-blocks")
	blocks := Blocks(bodyElem)

	if len(blocks) != 4 {
		t.Fatalf("block count = %d, want 4", len(blocks))
	}
	wantKinds := []model.BlockKind{
		model.BlockKindTable,
		model.BlockKindParagraph,
		model.BlockKindParagraph,
		model.BlockKindParagraph,
	}
	for i, block := range blocks {
		if block.Index != i+1 || block.Kind != wantKinds[i] {
			t.Fatalf("block %d = index %d kind %s, want index %d kind %s", i, block.Index, block.Kind, i+1, wantKinds[i])
		}
	}
}

func TestParagraphTextHandlesHyperlinksAndWhitespace(t *testing.T) {
	hyperlinkBody := fixtureBody(t, "hyperlink")
	hyperlinkBlocks := Blocks(hyperlinkBody)
	if got := ParagraphText(hyperlinkBlocks[0].Element); got != "Before link text after" {
		t.Fatalf("hyperlink text = %q", got)
	}

	spaceBody := fixtureBody(t, "space-preserve")
	spaceBlocks := Blocks(spaceBody)
	if got := ParagraphText(spaceBlocks[0].Element); got != " pad \ttabbed\nline" {
		t.Fatalf("space text = %q", got)
	}
}

func fixtureBody(t *testing.T, fixtureDir string) *etree.Element {
	t.Helper()
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "docx", fixtureDir, "document.docx"))
	if err != nil {
		t.Fatalf("opc.Open returned error: %v", err)
	}
	defer pkg.Close()
	doc, err := pkg.ReadXMLPart("/word/document.xml")
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	bodyElem, err := FindBody(doc.Root())
	if err != nil {
		t.Fatalf("FindBody returned error: %v", err)
	}
	return bodyElem
}
