package mutate

import (
	"errors"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestReplaceBlockWithParagraphGuardsHashPreservesStyleAndReturnsNewHash(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()
	hash := blockHashForTest(t, pkg, documentURI, 2)

	result, err := ReplaceBlockWithParagraph(&ReplaceBlockWithParagraphRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		Index:        2,
		ExpectedHash: hash,
		Text:         "Updated block",
	})
	if err != nil {
		t.Fatalf("ReplaceBlockWithParagraph returned error: %v", err)
	}
	if result.PreviousKind != model.BlockKindParagraph || result.PreviousHash != hash || result.PreviousText != "Bold heading" || result.Style != "Heading1" {
		t.Fatalf("unexpected replace result: %+v", result)
	}
	if result.ContentHash == "" || result.ContentHash == hash {
		t.Fatalf("new content hash = %q, old hash = %q", result.ContentHash, hash)
	}

	extracted := extractBlocksForTest(t, pkg, documentURI)
	if extracted.Blocks[1].Text != "Updated block" || extracted.Blocks[1].Paragraph == nil || extracted.Blocks[1].Paragraph.Style != "Heading1" {
		t.Fatalf("unexpected replace readback: %+v", extracted.Blocks[1])
	}

	_, err = ReplaceBlockWithParagraph(&ReplaceBlockWithParagraphRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		Index:        2,
		ExpectedHash: hash,
		Text:         "stale",
	})
	if !errors.Is(err, ErrBlockHashMismatch) {
		t.Fatalf("stale hash error = %v, want ErrBlockHashMismatch", err)
	}
}

func TestDeleteBlockRejectsSectionPropertiesAndLastBlock(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()

	_, err := DeleteBlock(&DeleteBlockRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		Index:        3,
		ExpectedHash: blockHashForTest(t, pkg, documentURI, 3),
	})
	if !errors.Is(err, ErrBlockHasSectionPr) {
		t.Fatalf("section-property delete error = %v, want ErrBlockHasSectionPr", err)
	}

	minimalPkg, minimalDocumentURI := openFixture(t, "minimal")
	defer minimalPkg.Close()
	_, err = DeleteBlock(&DeleteBlockRequest{
		Package:      minimalPkg,
		DocumentURI:  minimalDocumentURI,
		Index:        1,
		ExpectedHash: blockHashForTest(t, minimalPkg, minimalDocumentURI, 1),
	})
	if !errors.Is(err, ErrDeleteLastBlock) {
		t.Fatalf("last-block delete error = %v, want ErrDeleteLastBlock", err)
	}
}

func TestInsertParagraphAfterBlockReturnsHashAndPrepends(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()
	hash := blockHashForTest(t, pkg, documentURI, 1)

	result, err := InsertParagraphAfterBlock(&InsertParagraphAfterBlockRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		AfterIndex:   1,
		ExpectedHash: hash,
		Text:         "After table",
		Style:        "Heading1",
	})
	if err != nil {
		t.Fatalf("InsertParagraphAfterBlock returned error: %v", err)
	}
	if result.Index != 2 || result.AnchorHash != hash || result.ContentHash == "" || result.Style != "Heading1" {
		t.Fatalf("unexpected insert result: %+v", result)
	}
	extracted := extractBlocksForTest(t, pkg, documentURI)
	if extracted.Blocks[1].Text != "After table" || extracted.Blocks[1].Paragraph.Style != "Heading1" {
		t.Fatalf("unexpected insert readback: %+v", extracted.Blocks[1])
	}

	result, err = InsertParagraphAfterBlock(&InsertParagraphAfterBlockRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  0,
		Text:        "Lead",
	})
	if err != nil {
		t.Fatalf("prepend InsertParagraphAfterBlock returned error: %v", err)
	}
	if result.Index != 1 || result.InsertAfter != 0 || result.ContentHash == "" {
		t.Fatalf("unexpected prepend result: %+v", result)
	}
}

func blockHashForTest(t *testing.T, pkg opc.PackageSession, documentURI string, block int) string {
	t.Helper()
	extracted := extractBlocksForTest(t, pkg, documentURI)
	for _, report := range extracted.Blocks {
		if report.Index == block {
			return report.ContentHash
		}
	}
	t.Fatalf("block %d not found", block)
	return ""
}

func extractBlocksForTest(t *testing.T, pkg opc.PackageSession, documentURI string) *extract.ExtractedBlocks {
	t.Helper()
	result, err := extract.ExtractBlocks(&extract.ExtractBlocksRequest{
		Session:     pkg,
		DocumentURI: documentURI,
	})
	if err != nil {
		t.Fatalf("ExtractBlocks returned error: %v", err)
	}
	return result
}
