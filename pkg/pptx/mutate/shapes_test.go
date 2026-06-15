package mutate

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

func TestSetSlideShapeBounds(t *testing.T) {
	pkg := openShapeMutationFixture(t)
	defer pkg.Close()

	result, err := SetSlideShapeBounds(&SetSlideShapeBoundsRequest{
		Package:     pkg,
		SlideNumber: 2,
		Target:      "body",
		X:           111111,
		Y:           222222,
		CX:          333333,
		CY:          444444,
	})
	if err != nil {
		t.Fatalf("SetSlideShapeBounds returned error: %v", err)
	}
	if result.ShapeID != 3 || result.ShapeName != "Content Placeholder 2" || result.NewX != 111111 || result.NewCY != 444444 {
		t.Fatalf("unexpected set-bounds result: %+v", result)
	}

	catalog, err := selectors.BuildSlideCatalog(pkg, 2)
	if err != nil {
		t.Fatalf("failed to rebuild catalog: %v", err)
	}
	_, elem, err := catalog.ResolveTargetElement("body")
	if err != nil {
		t.Fatalf("failed to resolve body: %v", err)
	}
	xfrm := elem.FindElement("spPr/xfrm")
	if xfrm == nil {
		t.Fatal("body shape missing transform")
	}
	off := xfrm.FindElement("off")
	ext := xfrm.FindElement("ext")
	if off == nil || ext == nil {
		t.Fatalf("transform missing off/ext: %+v", xfrm)
	}
	if off.SelectAttrValue("x", "") != "111111" || off.SelectAttrValue("y", "") != "222222" ||
		ext.SelectAttrValue("cx", "") != "333333" || ext.SelectAttrValue("cy", "") != "444444" {
		t.Fatalf("unexpected transform off/ext: off=%v ext=%v", off.Attr, ext.Attr)
	}
}

func TestDeleteSlideShape(t *testing.T) {
	pkg := openShapeMutationFixture(t)
	defer pkg.Close()

	result, err := DeleteSlideShape(&DeleteSlideShapeRequest{
		Package:     pkg,
		SlideNumber: 2,
		Target:      "title",
	})
	if err != nil {
		t.Fatalf("DeleteSlideShape returned error: %v", err)
	}
	if result.ShapeID != 2 || result.Target != "title" {
		t.Fatalf("unexpected delete result: %+v", result)
	}
	catalog, err := selectors.BuildSlideCatalog(pkg, 2)
	if err != nil {
		t.Fatalf("failed to rebuild catalog: %v", err)
	}
	if _, _, err := catalog.ResolveTargetElement("title"); err == nil {
		t.Fatal("expected title target to be gone")
	}
	if _, _, err := catalog.ResolveTargetElement("body"); err != nil {
		t.Fatalf("body target should remain: %v", err)
	}
}

func TestSetSlideShapeBoundsRejectsGroup(t *testing.T) {
	pkg := openPackageWithSyntheticGroupShape(t)
	defer pkg.Close()

	_, err := SetSlideShapeBounds(&SetSlideShapeBoundsRequest{
		Package:     pkg,
		SlideNumber: 2,
		Target:      "shape:99",
		X:           1,
		Y:           2,
		CX:          3,
		CY:          4,
	})
	if err == nil || !strings.Contains(err.Error(), "group shape") {
		t.Fatalf("group set-bounds error = %v", err)
	}
}

func openShapeMutationFixture(t *testing.T) *opc.Package {
	t.Helper()
	path := filepath.Join("..", "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	return pkg
}

func openPackageWithSyntheticGroupShape(t *testing.T) *opc.Package {
	t.Helper()
	pkg := openShapeMutationFixture(t)

	slideDoc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide XML: %v", err)
	}
	spTree := findTestChildByLocalName(findTestChildByLocalName(slideDoc.Root(), "cSld"), "spTree")
	if spTree == nil {
		t.Fatal("slide missing shape tree")
	}

	groupDoc := etree.NewDocument()
	if err := groupDoc.ReadFromString(`<p:grpSp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:nvGrpSpPr><p:cNvPr id="99" name="Synthetic Group"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:grpSp>`); err != nil {
		t.Fatalf("failed to parse synthetic group XML: %v", err)
	}
	spTree.AddChild(groupDoc.Root().Copy())
	if err := pkg.ReplaceXMLPart("/ppt/slides/slide2.xml", slideDoc); err != nil {
		t.Fatalf("failed to write synthetic group: %v", err)
	}
	return pkg
}

func findTestChildByLocalName(elem *etree.Element, localName string) *etree.Element {
	if elem == nil {
		return nil
	}
	for _, child := range elem.ChildElements() {
		if child.Tag == localName || strings.HasSuffix(child.Tag, ":"+localName) {
			return child
		}
	}
	return nil
}
