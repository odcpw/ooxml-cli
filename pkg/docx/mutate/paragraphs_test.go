package mutate

import (
	"errors"
	"path/filepath"
	"testing"

	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestSetParagraphTextPreservesStyleAndRunProperties(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()

	result, err := SetParagraphText(&SetParagraphTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Index:       2,
		Text:        "Updated heading",
	})
	if err != nil {
		t.Fatalf("SetParagraphText returned error: %v", err)
	}
	if result.PreviousText != "Bold heading" || result.Style != "Heading1" {
		t.Fatalf("unexpected result metadata: %+v", result)
	}

	extracted := extractText(t, pkg, documentURI)
	block := extracted.Blocks[1]
	if block.Text != "Updated heading" || block.Style != "Heading1" {
		t.Fatalf("block readback = %+v", block)
	}

	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	rPr := namespaces.FindDescendants(doc.Root(), namespaces.NsW, "rPr")
	if len(rPr) == 0 || namespaces.FindChild(rPr[0], namespaces.NsW, "b") == nil {
		t.Fatalf("expected copied bold run properties")
	}
}

func TestSetParagraphTextEncodesTabsNewlinesAndSpaces(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	want := " lead\tmid\nnext "
	_, err := SetParagraphText(&SetParagraphTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Index:       1,
		Text:        want,
	})
	if err != nil {
		t.Fatalf("SetParagraphText returned error: %v", err)
	}

	extracted := extractText(t, pkg, documentURI)
	if got := extracted.Blocks[0].Text; got != want {
		t.Fatalf("text readback = %q, want %q", got, want)
	}
}

func TestSetParagraphTextDefaultNamespaceRoundTrips(t *testing.T) {
	pkg, documentURI := openFixture(t, "default-ns")
	defer pkg.Close()

	_, err := SetParagraphText(&SetParagraphTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Index:       1,
		Text:        "default namespace updated",
	})
	if err != nil {
		t.Fatalf("SetParagraphText returned error: %v", err)
	}
	extracted := extractText(t, pkg, documentURI)
	if got := extracted.Blocks[0].Text; got != "default namespace updated" {
		t.Fatalf("text readback = %q", got)
	}

	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	runs := namespaces.FindDescendants(doc.Root(), namespaces.NsW, "r")
	if len(runs) == 0 || runs[0].Space != "" {
		t.Fatalf("run namespace prefix = %q, want empty", runs[0].Space)
	}
}

func TestClearParagraphTextKeepsStyleAndEmptiesText(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	result, err := ClearParagraphText(&ClearParagraphTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Index:       1,
	})
	if err != nil {
		t.Fatalf("ClearParagraphText returned error: %v", err)
	}
	if result.PreviousText != "Heading Text" || result.Style != "Heading1" {
		t.Fatalf("unexpected clear result: %+v", result)
	}
	extracted := extractText(t, pkg, documentURI)
	if extracted.Blocks[0].Text != "" || extracted.Blocks[0].Style != "Heading1" {
		t.Fatalf("clear readback = %+v", extracted.Blocks[0])
	}
}

func TestParagraphMutationErrors(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()

	_, err := SetParagraphText(&SetParagraphTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Index:       1,
		Text:        "not allowed",
	})
	if !errors.Is(err, ErrBlockNotParagraph) {
		t.Fatalf("table index error = %v, want ErrBlockNotParagraph", err)
	}

	_, err = SetParagraphText(&SetParagraphTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Index:       99,
		Text:        "missing",
	})
	if !errors.Is(err, ErrBlockIndexOutOfRange) {
		t.Fatalf("out-of-range error = %v, want ErrBlockIndexOutOfRange", err)
	}
}

func TestAppendParagraphPreservesSectPrAndStyle(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	result, err := AppendParagraph(&AppendParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Text:        "Tail paragraph",
		Style:       "Heading1",
	})
	if err != nil {
		t.Fatalf("AppendParagraph returned error: %v", err)
	}
	if result.Index != 3 || result.Style != "Heading1" || result.Text != "Tail paragraph" {
		t.Fatalf("append result = %+v", result)
	}

	extracted := extractText(t, pkg, documentURI)
	if len(extracted.Blocks) != 3 {
		t.Fatalf("block count = %d, want 3", len(extracted.Blocks))
	}
	tail := extracted.Blocks[2]
	if tail.Text != "Tail paragraph" || tail.Style != "Heading1" {
		t.Fatalf("tail block = %+v", tail)
	}
	assertSectPrLast(t, pkg, documentURI)
}

func TestInsertParagraphAtStart(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	result, err := InsertParagraph(&InsertParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  0,
		Text:        "Lead paragraph",
	})
	if err != nil {
		t.Fatalf("InsertParagraph returned error: %v", err)
	}
	if result.Index != 1 || result.Text != "Lead paragraph" {
		t.Fatalf("insert result = %+v", result)
	}
	extracted := extractText(t, pkg, documentURI)
	if len(extracted.Blocks) != 3 {
		t.Fatalf("block count = %d, want 3", len(extracted.Blocks))
	}
	if extracted.Blocks[0].Text != "Lead paragraph" || extracted.Blocks[1].Text != "Heading Text" {
		t.Fatalf("blocks after prepend = %+v", extracted.Blocks)
	}
}

func TestInsertParagraphAfterTableBlock(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()

	result, err := InsertParagraph(&InsertParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  1,
		Text:        "Paragraph after table",
	})
	if err != nil {
		t.Fatalf("InsertParagraph returned error: %v", err)
	}
	if result.Index != 2 {
		t.Fatalf("insert result index = %d, want 2", result.Index)
	}

	extracted := extractText(t, pkg, documentURI)
	if len(extracted.Blocks) != 5 {
		t.Fatalf("block count = %d, want 5", len(extracted.Blocks))
	}
	if extracted.Blocks[0].Kind != model.BlockKindTable || extracted.Blocks[1].Kind != model.BlockKindParagraph {
		t.Fatalf("unexpected block kinds after table insert: %+v", extracted.Blocks[:2])
	}
	if extracted.Blocks[1].Text != "Paragraph after table" {
		t.Fatalf("inserted block text = %q", extracted.Blocks[1].Text)
	}
}

func TestAppendParagraphAllowsEmptyText(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	result, err := AppendParagraph(&AppendParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
	})
	if err != nil {
		t.Fatalf("AppendParagraph returned error: %v", err)
	}
	if result.Index != 2 || result.Text != "" {
		t.Fatalf("append result = %+v", result)
	}
	extracted := extractText(t, pkg, documentURI)
	if len(extracted.Blocks) != 2 || extracted.Blocks[1].Text != "" {
		t.Fatalf("blocks after blank append = %+v", extracted.Blocks)
	}
}

func TestInsertParagraphDefaultNamespaceStyleRoundTrips(t *testing.T) {
	pkg, documentURI := openFixture(t, "default-ns")
	defer pkg.Close()

	_, err := InsertParagraph(&InsertParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  1,
		Text:        "default namespace styled",
		Style:       "Heading1",
	})
	if err != nil {
		t.Fatalf("InsertParagraph returned error: %v", err)
	}
	extracted := extractText(t, pkg, documentURI)
	if len(extracted.Blocks) != 2 {
		t.Fatalf("block count = %d, want 2", len(extracted.Blocks))
	}
	if extracted.Blocks[1].Text != "default namespace styled" || extracted.Blocks[1].Style != "Heading1" {
		t.Fatalf("inserted block = %+v", extracted.Blocks[1])
	}

	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	if attr := doc.Root().SelectAttr("xmlns:w"); attr == nil || attr.Value != namespaces.NsW {
		t.Fatalf("expected xmlns:w declaration for styled default-namespace document")
	}
}

func TestInsertParagraphErrors(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	_, err := InsertParagraph(&InsertParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  99,
		Text:        "missing",
	})
	if !errors.Is(err, ErrBlockIndexOutOfRange) {
		t.Fatalf("out-of-range error = %v, want ErrBlockIndexOutOfRange", err)
	}

	_, err = InsertParagraph(&InsertParagraphRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  -1,
		Text:        "invalid",
	})
	if err == nil {
		t.Fatal("expected negative index error")
	}
}

func openFixture(t *testing.T, fixtureDir string) (*opc.Package, string) {
	t.Helper()
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "docx", fixtureDir, "document.docx"))
	if err != nil {
		t.Fatalf("opc.Open returned error: %v", err)
	}
	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("FindMainDocumentPart returned error: %v", err)
	}
	return pkg, documentURI
}

func extractText(t *testing.T, pkg opc.PackageSession, documentURI string) *extract.ExtractedDocument {
	t.Helper()
	result, err := extract.ExtractText(&extract.ExtractTextRequest{
		Session:     pkg,
		DocumentURI: documentURI,
	})
	if err != nil {
		t.Fatalf("ExtractText returned error: %v", err)
	}
	return result
}

func assertSectPrLast(t *testing.T, pkg opc.PackageSession, documentURI string) {
	t.Helper()
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	bodyElem, err := docxbody.FindBody(doc.Root())
	if err != nil {
		t.Fatalf("FindBody returned error: %v", err)
	}
	elements := bodyElem.ChildElements()
	if len(elements) == 0 {
		t.Fatal("document body has no child elements")
	}
	if got := docxbody.LocalName(elements[len(elements)-1].Tag); got != "sectPr" {
		t.Fatalf("last body element = %s, want sectPr", got)
	}
}
