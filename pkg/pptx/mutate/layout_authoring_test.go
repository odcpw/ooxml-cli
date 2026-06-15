package mutate

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCloneLayout_AddsNewLayoutToMaster(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	graphBefore, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graphBefore.Layouts, 11)

	result, err := CloneLayout(&CloneLayoutRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout2.xml",
		NewName:       "Image Grid Clone",
	})
	require.NoError(t, err)
	require.NotNil(t, result)
	assert.Equal(t, "Title and Content", result.OldName)
	assert.Equal(t, "Image Grid Clone", result.NewName)
	assert.NotEqual(t, result.SourceLayoutURI, result.NewLayoutURI)
	assert.Equal(t, "/ppt/slideMasters/slideMaster1.xml", result.MasterPartURI)

	graphAfter, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graphAfter.Layouts, 12)

	var found bool
	for _, layout := range graphAfter.Layouts {
		if layout.PartURI == result.NewLayoutURI {
			found = true
			assert.Equal(t, "Image Grid Clone", layout.Name)
			assert.Equal(t, "/ppt/slideMasters/slideMaster1.xml", layout.MasterPartURI)
			break
		}
	}
	assert.True(t, found, "cloned layout should appear in presentation graph")

	rels := pkg.ListRelationships(result.NewLayoutURI)
	var foundMasterRel bool
	for _, rel := range rels {
		if rel.Type == slideMasterRelationshipType {
			foundMasterRel = true
			break
		}
	}
	assert.True(t, foundMasterRel, "cloned layout should keep slideMaster relationship")
}

func TestRenameLayout_UpdatesName(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	result, err := RenameLayout(&RenameLayoutRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout2.xml",
		NewName:       "Renamed Layout",
	})
	require.NoError(t, err)
	require.NotNil(t, result)
	assert.Equal(t, "Title and Content", result.OldName)
	assert.Equal(t, "Renamed Layout", result.NewName)

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	var found bool
	for _, layout := range graph.Layouts {
		if layout.PartURI == "/ppt/slideLayouts/slideLayout2.xml" {
			found = true
			assert.Equal(t, "Renamed Layout", layout.Name)
		}
	}
	assert.True(t, found)
}

func TestDeleteLayoutShape_RemovesPlaceholder(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	result, err := DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout2.xml",
		Target:        "title",
	})
	require.NoError(t, err)
	require.NotNil(t, result)
	assert.NotZero(t, result.ShapeID)

	layoutDoc, err := pkg.ReadXMLPart("/ppt/slideLayouts/slideLayout2.xml")
	require.NoError(t, err)
	shape, err := findTargetShape(layoutDoc.Root(), nil, nil, "title")
	require.NoError(t, err)
	assert.Nil(t, shape, "title placeholder should be removed")
}

func TestDeleteLayoutShape_ResolvesLiteralPlaceholderKeys(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	result, err := DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout2.xml",
		Target:        "dt:10",
	})
	require.NoError(t, err)
	require.NotNil(t, result)

	layoutDoc, err := pkg.ReadXMLPart("/ppt/slideLayouts/slideLayout2.xml")
	require.NoError(t, err)
	shape, err := findTargetShape(layoutDoc.Root(), layoutDoc.Root(), nil, "dt:10")
	require.NoError(t, err)
	assert.Nil(t, shape, "date placeholder should be removed by literal key")
}

func TestSetLayoutShapeBounds_UpdatesGeometry(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	result, err := SetLayoutShapeBounds(&SetLayoutShapeBoundsRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout2.xml",
		Target:        "shape:3",
		X:             111111,
		Y:             222222,
		CX:            333333,
		CY:            444444,
	})
	require.NoError(t, err)
	require.NotNil(t, result)
	assert.Equal(t, int64(111111), result.NewX)
	assert.Equal(t, int64(444444), result.NewCY)

	layoutDoc, err := pkg.ReadXMLPart("/ppt/slideLayouts/slideLayout2.xml")
	require.NoError(t, err)
	shape, err := findTargetShape(layoutDoc.Root(), nil, nil, "shape:3")
	require.NoError(t, err)
	require.NotNil(t, shape)

	spPr := shape.FindElement("spPr")
	require.NotNil(t, spPr)
	xfrm := spPr.FindElement("xfrm")
	require.NotNil(t, xfrm)
	off := xfrm.FindElement("off")
	require.NotNil(t, off)
	ext := xfrm.FindElement("ext")
	require.NotNil(t, ext)
	assert.Equal(t, "111111", off.SelectAttrValue("x", ""))
	assert.Equal(t, "222222", off.SelectAttrValue("y", ""))
	assert.Equal(t, "333333", ext.SelectAttrValue("cx", ""))
	assert.Equal(t, "444444", ext.SelectAttrValue("cy", ""))
}
