package handle_test

import (
	"path/filepath"
	"testing"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	. "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func openDoc(t *testing.T, fixture string) (*opc.Package, *etree.Element) {
	t.Helper()
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "docx", fixture, "document.docx"))
	if err != nil {
		t.Fatalf("opc.Open: %v", err)
	}
	uri, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("FindMainDocumentPart: %v", err)
	}
	doc, err := pkg.ReadXMLPart(uri)
	if err != nil {
		pkg.Close()
		t.Fatalf("ReadXMLPart: %v", err)
	}
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		pkg.Close()
		t.Fatalf("FindBody: %v", err)
	}
	return pkg, body
}

// TestResolveParagraphReadsExistingMarker proves a paragraph carrying a real
// w14:paraId is resolvable by a marker handle without any injection (the pure
// read path against an authored fixture).
func TestResolveParagraphReadsExistingMarker(t *testing.T) {
	pkg, body := openDoc(t, "paraid")
	defer pkg.Close()

	h := Handle{Format: FormatDOCX, Kind: KindParagraph, ParaID: "1A2B3C4D"}
	idx, elem, err := ResolveParagraphBlock(body, h)
	if err != nil {
		t.Fatalf("ResolveParagraphBlock: %v", err)
	}
	if idx != 1 {
		t.Fatalf("block index = %d, want 1", idx)
	}
	if got := docxbody.ParagraphText(elem); got != "First marked paragraph" {
		t.Fatalf("resolved paragraph text = %q", got)
	}
}

// TestResolveParagraphCaseInsensitive proves paraId matching is case-insensitive
// (hex values are case-insensitive per the OOXML type).
func TestResolveParagraphCaseInsensitive(t *testing.T) {
	pkg, body := openDoc(t, "paraid")
	defer pkg.Close()

	h := Handle{Format: FormatDOCX, Kind: KindParagraph, ParaID: "1a2b3c4d"}
	if _, _, err := ResolveParagraphBlock(body, h); err != nil {
		t.Fatalf("case-insensitive resolve failed: %v", err)
	}
}

// TestResolveParagraphStale proves a marker that is not present yields a clean
// CodeStale error rather than a wrong-target positional fallback.
func TestResolveParagraphStale(t *testing.T) {
	pkg, body := openDoc(t, "paraid")
	defer pkg.Close()

	h := Handle{Format: FormatDOCX, Kind: KindParagraph, ParaID: "DEADBEEF"}
	_, _, err := ResolveParagraphBlock(body, h)
	if !IsCode(err, CodeStale) {
		t.Fatalf("err = %v, want %s", err, CodeStale)
	}
}

// TestResolveParagraphAmbiguous proves a duplicate marker (e.g. host-app
// copy/paste) refuses to resolve positionally and surfaces CodeAmbiguous.
func TestResolveParagraphAmbiguous(t *testing.T) {
	body := etree.NewElement("body")
	body.Space = "w"
	body.CreateAttr("xmlns:w", namespaces.NsW)
	for i := 0; i < 2; i++ {
		p := etree.NewElement("p")
		p.Space = "w"
		p.CreateAttr("w14:paraId", "AAAA0001")
		body.AddChild(p)
	}
	h := Handle{Format: FormatDOCX, Kind: KindParagraph, ParaID: "AAAA0001"}
	_, _, err := ResolveParagraphBlock(body, h)
	if !IsCode(err, CodeAmbiguous) {
		t.Fatalf("err = %v, want %s", err, CodeAmbiguous)
	}
}

// TestResolveCommentNativeAndStale exercises the comment resolver.
func TestResolveCommentNativeAndStale(t *testing.T) {
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "docx", "with-comments", "document.docx"))
	if err != nil {
		t.Fatalf("opc.Open: %v", err)
	}
	defer pkg.Close()
	uri, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		t.Fatalf("FindMainDocumentPart: %v", err)
	}
	commentsURI, ok := docxinspect.FindCommentsPart(pkg, uri)
	if !ok {
		t.Fatal("expected a comments part")
	}
	commentsDoc, err := pkg.ReadXMLPart(commentsURI)
	if err != nil {
		t.Fatalf("ReadXMLPart comments: %v", err)
	}
	root := commentsDoc.Root()

	if _, err := ResolveComment(root, Handle{Kind: KindComment, CommentID: 0}); err != nil {
		t.Fatalf("ResolveComment(0): %v", err)
	}
	if _, err := ResolveComment(root, Handle{Kind: KindComment, CommentID: 999}); !IsCode(err, CodeStale) {
		t.Fatalf("ResolveComment(999) = %v, want %s", err, CodeStale)
	}
}

// TestResolveCommentAmbiguous proves a duplicate comment w:id refuses to resolve.
func TestResolveCommentAmbiguous(t *testing.T) {
	root := etree.NewElement("comments")
	root.Space = "w"
	root.CreateAttr("xmlns:w", namespaces.NsW)
	for i := 0; i < 2; i++ {
		c := etree.NewElement("comment")
		c.Space = "w"
		c.CreateAttr("w:id", "5")
		root.AddChild(c)
	}
	if _, err := ResolveComment(root, Handle{Kind: KindComment, CommentID: 5}); !IsCode(err, CodeAmbiguous) {
		t.Fatalf("err = %v, want %s", err, CodeAmbiguous)
	}
}

// TestResolveStyleNativeAndStale exercises the style resolver.
func TestResolveStyleNativeAndStale(t *testing.T) {
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "docx", "styles-catalog", "document.docx"))
	if err != nil {
		t.Fatalf("opc.Open: %v", err)
	}
	defer pkg.Close()
	stylesURI, err := docxinspect.FindStylesPart(pkg)
	if err != nil || stylesURI == "" {
		t.Fatalf("FindStylesPart: %v (uri %q)", err, stylesURI)
	}
	stylesDoc, err := pkg.ReadXMLPart(stylesURI)
	if err != nil {
		t.Fatalf("ReadXMLPart styles: %v", err)
	}
	root := stylesDoc.Root()

	if _, err := ResolveStyle(root, Handle{Kind: KindStyle, StyleID: "Heading1"}); err != nil {
		t.Fatalf("ResolveStyle(Heading1): %v", err)
	}
	if _, err := ResolveStyle(root, Handle{Kind: KindStyle, StyleID: "NoSuchStyle"}); !IsCode(err, CodeStale) {
		t.Fatalf("ResolveStyle(NoSuchStyle) = %v, want %s", err, CodeStale)
	}
}
