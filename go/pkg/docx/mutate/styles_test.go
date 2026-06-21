package mutate

import (
	"errors"
	"strconv"
	"testing"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

func TestApplyParagraphStyleUpdatesExistingPStyle(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, err := docxinspect.FindStylesPart(pkg)
	if err != nil {
		t.Fatalf("FindStylesPart returned error: %v", err)
	}

	result, err := ApplyParagraphStyle(&ApplyParagraphStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       1,
		StyleID:     "Heading2",
		Validate:    true,
	})
	if err != nil {
		t.Fatalf("ApplyParagraphStyle returned error: %v", err)
	}
	if result.PreviousStyle != "Heading1" || result.Style != "Heading2" || result.Target != "paragraph" {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.ContentHash == result.PreviousHash {
		t.Fatalf("paragraph style change should alter content hash: %+v", result)
	}

	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	body, _ := docxbody.FindBody(doc.Root())
	para := docxbody.Blocks(body)[0].Element
	pStyles := namespaces.FindChildren(namespaces.FindChild(para, namespaces.NsW, "pPr"), namespaces.NsW, "pStyle")
	if len(pStyles) != 1 {
		t.Fatalf("expected exactly one pStyle, got %d", len(pStyles))
	}
	if got := docxbody.ParagraphStyle(para); got != "Heading2" {
		t.Fatalf("paragraph style = %q, want Heading2", got)
	}
	assertFirstBodyTableScaffold(t, doc)
	paraID := readParaIDForBlock(t, pkg, documentURI, 1)
	value, err := strconv.ParseUint(paraID, 16, 32)
	if err != nil {
		t.Fatalf("paraId = %q is not valid hex: %v", paraID, err)
	}
	if value >= 0x80000000 {
		t.Fatalf("paraId = %q (%#x), want < 0x80000000", paraID, value)
	}
}

func TestApplyRunStyleCreatesRPrAndKeepsHash(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	result, err := ApplyRunStyle(&ApplyRunStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       2,
		StyleID:     "Emphasis",
		Validate:    true,
	})
	if err != nil {
		t.Fatalf("ApplyRunStyle returned error: %v", err)
	}
	if result.Style != "Emphasis" || result.Target != "run" {
		t.Fatalf("unexpected result: %+v", result)
	}
	// rStyle is not folded into the block content hash, so it stays stable.
	if result.ContentHash != result.PreviousHash {
		t.Fatalf("run style change should not alter content hash: %+v", result)
	}

	doc, _ := pkg.ReadXMLPart(documentURI)
	body, _ := docxbody.FindBody(doc.Root())
	para := docxbody.Blocks(body)[1].Element
	if got := docxbody.RunStyle(para); got != "Emphasis" {
		t.Fatalf("run style = %q, want Emphasis", got)
	}
	// rStyle must be the first child of rPr.
	run := namespaces.FindChildren(para, namespaces.NsW, "r")[0]
	rPr := namespaces.FindChild(run, namespaces.NsW, "rPr")
	if rPr == nil || rPr.ChildElements()[0].Tag != "rStyle" {
		t.Fatalf("rStyle is not the first rPr child: %+v", rPr)
	}
}

func TestApplyRunStyleIntoExistingRPrPrependsRStyle(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	// Block 1's run already carries an rPr with <w:b/>; rStyle must be inserted
	// as the first child to preserve WordprocessingML ordering.
	if _, err := ApplyRunStyle(&ApplyRunStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       1,
		StyleID:     "Emphasis",
		Validate:    true,
	}); err != nil {
		t.Fatalf("ApplyRunStyle returned error: %v", err)
	}

	doc, _ := pkg.ReadXMLPart(documentURI)
	body, _ := docxbody.FindBody(doc.Root())
	run := namespaces.FindChildren(docxbody.Blocks(body)[0].Element, namespaces.NsW, "r")[0]
	rPr := namespaces.FindChild(run, namespaces.NsW, "rPr")
	children := rPr.ChildElements()
	if len(children) != 2 || children[0].Tag != "rStyle" || children[1].Tag != "b" {
		var tags []string
		for _, c := range children {
			tags = append(tags, c.Tag)
		}
		t.Fatalf("rPr children = %v, want [rStyle b]", tags)
	}
}

func TestApplyTableStyleCreatesTblPr(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	result, err := ApplyTableStyle(&ApplyTableStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       1,
		StyleID:     "TableGrid",
		Validate:    true,
	})
	if err != nil {
		t.Fatalf("ApplyTableStyle returned error: %v", err)
	}
	if result.Style != "TableGrid" || result.Target != "table" || result.BlockKind != "table" {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.BlockIndex != 3 {
		t.Fatalf("table block index = %d, want 3", result.BlockIndex)
	}

	doc, _ := pkg.ReadXMLPart(documentURI)
	body, _ := docxbody.FindBody(doc.Root())
	table := docxbody.Blocks(body)[2].Element
	if got := docxbody.TableStyle(table); got != "TableGrid" {
		t.Fatalf("table style = %q, want TableGrid", got)
	}
	// tblPr must be the first child of the table.
	if table.ChildElements()[0].Tag != "tblPr" {
		t.Fatalf("tblPr is not the first table child")
	}
	assertFirstBodyTableScaffold(t, doc)
}

func assertFirstBodyTableScaffold(t *testing.T, doc interface{ Root() *etree.Element }) {
	t.Helper()
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		t.Fatalf("FindBody returned error: %v", err)
	}
	for _, child := range body.ChildElements() {
		if docxbody.LocalName(child.Tag) != "tbl" {
			continue
		}
		children := child.ChildElements()
		if len(children) < 3 {
			t.Fatalf("table has %d children, want at least tblPr, tblGrid, tr", len(children))
		}
		if docxbody.LocalName(children[0].Tag) != "tblPr" || docxbody.LocalName(children[1].Tag) != "tblGrid" || docxbody.LocalName(children[2].Tag) != "tr" {
			t.Fatalf("table children begin with %s, %s, %s; want tblPr, tblGrid, tr", children[0].Tag, children[1].Tag, children[2].Tag)
		}
		return
	}
	t.Fatal("no body table found")
}

func TestApplyStyleHashGuardMismatch(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	_, err := ApplyParagraphStyle(&ApplyParagraphStyleRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		StylesURI:    stylesURI,
		Index:        1,
		StyleID:      "Heading2",
		ExpectedHash: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		Validate:     true,
	})
	if !errors.Is(err, ErrBlockHashMismatch) {
		t.Fatalf("error = %v, want ErrBlockHashMismatch", err)
	}
}

func TestApplyStyleNotFoundListsCandidates(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	_, err := ApplyParagraphStyle(&ApplyParagraphStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       1,
		StyleID:     "DoesNotExist",
		Validate:    true,
	})
	var notFound *StyleNotFoundError
	if !errors.As(err, &notFound) {
		t.Fatalf("error = %v, want *StyleNotFoundError", err)
	}
	if len(notFound.Candidates) == 0 {
		t.Fatalf("expected candidate paragraph styles, got none")
	}
	for _, c := range notFound.Candidates {
		if c == "Emphasis" {
			t.Fatalf("character style leaked into paragraph candidates: %v", notFound.Candidates)
		}
	}
}

func TestApplyStyleTypeMismatch(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	_, err := ApplyParagraphStyle(&ApplyParagraphStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       1,
		StyleID:     "Emphasis",
		Validate:    true,
	})
	var mismatch *StyleTypeMismatchError
	if !errors.As(err, &mismatch) {
		t.Fatalf("error = %v, want *StyleTypeMismatchError", err)
	}
	if mismatch.ActualType != "character" || mismatch.WantType != "paragraph" {
		t.Fatalf("unexpected mismatch detail: %+v", mismatch)
	}
}

func TestApplyStyleSkipsValidationWhenDisabled(t *testing.T) {
	pkg, documentURI := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)

	// A wrong-type style would normally be rejected; with Validate=false the
	// mutation goes through.
	result, err := ApplyParagraphStyle(&ApplyParagraphStyleRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		StylesURI:   stylesURI,
		Index:       1,
		StyleID:     "Emphasis",
		Validate:    false,
	})
	if err != nil {
		t.Fatalf("ApplyParagraphStyle with Validate=false returned error: %v", err)
	}
	if result.Style != "Emphasis" {
		t.Fatalf("unexpected result: %+v", result)
	}
}

func TestSuggestStylesFiltersByType(t *testing.T) {
	pkg, _ := openFixture(t, "apply-styles")
	defer pkg.Close()
	stylesURI, _ := docxinspect.FindStylesPart(pkg)
	styles, err := docxinspect.ParseStyles(pkg, stylesURI)
	if err != nil {
		t.Fatalf("ParseStyles returned error: %v", err)
	}
	tables := suggestStyles(styles, "table")
	if len(tables) == 0 {
		t.Fatalf("expected table styles")
	}
	for _, id := range tables {
		if id == "Heading1" {
			t.Fatalf("paragraph style leaked into table candidates: %v", tables)
		}
	}
	// Sorted ascending.
	for i := 1; i < len(tables); i++ {
		if tables[i-1] > tables[i] {
			t.Fatalf("candidates not sorted: %v", tables)
		}
	}
}
