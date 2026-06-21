package mutate

import (
	"testing"

	"github.com/beevik/etree"
)

func TestAppendSpTreeChildInsertsBeforeExtLst(t *testing.T) {
	spTree := mustParseSpTree(t)
	appendSpTreeChild(spTree, mustParseElement(t, `<p:graphicFrame xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:nvGraphicFramePr><p:cNvPr id="3" name="Chart 3"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr></p:graphicFrame>`))

	children := spTree.ChildElements()
	if got := pptxMutateLocalName(children[len(children)-2].Tag); got != "graphicFrame" {
		t.Fatalf("expected inserted child before extLst, got %s", got)
	}
	if got := pptxMutateLocalName(children[len(children)-1].Tag); got != "extLst" {
		t.Fatalf("expected extLst to remain last, got %s", got)
	}
}

func TestInsertSpTreeChildAfterShapeIDDoesNotCrossExtLst(t *testing.T) {
	spTree := mustParseSpTree(t)
	insertSpTreeChildAfterShapeID(spTree, mustParseElement(t, `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:nvSpPr><p:cNvPr id="3" name="Inserted"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr></p:sp>`), 2)

	children := spTree.ChildElements()
	if got := pptxMutateLocalName(children[3].Tag); got != "sp" {
		t.Fatalf("expected shape inserted directly before extLst, got %s", got)
	}
	if got := pptxMutateLocalName(children[4].Tag); got != "extLst" {
		t.Fatalf("expected extLst to remain final, got %s", got)
	}
}

func mustParseSpTree(t *testing.T) *etree.Element {
	t.Helper()
	xml := `<p:spTree xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
  <p:grpSpPr/>
  <p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr></p:sp>
  <p:extLst><p:ext uri="{11111111-1111-1111-1111-111111111111}"/></p:extLst>
</p:spTree>`
	return mustParseElement(t, xml)
}

func mustParseElement(t *testing.T, xml string) *etree.Element {
	t.Helper()
	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("parse XML fixture: %v", err)
	}
	return doc.Root()
}
