package mutate

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestReplaceText_PlaceholderKey(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	err = ReplaceText(&ReplaceTextRequest{
		Package:     pkg,
		SlideNumber: 1,
		Target:      "title",
		NewText:     "Updated Title",
	})
	require.NoError(t, err)

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)

	slide1, err := pkg.ReadXMLPart(graph.Slides[0].PartURI)
	require.NoError(t, err)
	slide1Text, err := slide1.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slide1Text, "Updated Title")
	assert.Contains(t, slide1Text, "Subtitle goes here")
	assert.NotContains(t, slide1Text, "Title Content Presentation")

	slide2, err := pkg.ReadXMLPart(graph.Slides[1].PartURI)
	require.NoError(t, err)
	slide2Text, err := slide2.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slide2Text, "Content Slide")
	assert.Contains(t, slide2Text, "This is the main content area")
}

func TestReplaceText_ShapeID(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	err = ReplaceText(&ReplaceTextRequest{
		Package:     pkg,
		SlideNumber: 2,
		Target:      "shape:3",
		NewText:     "Updated body",
	})
	require.NoError(t, err)

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	slide2, err := pkg.ReadXMLPart(graph.Slides[1].PartURI)
	require.NoError(t, err)
	slide2Text, err := slide2.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slide2Text, "Updated body")
	assert.NotContains(t, slide2Text, "This is the main content area")
}

func TestReplaceText_ImportedSlideBodyIndexAlias(t *testing.T) {
	sourcePkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer sourcePkg.Close()

	targetPkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	defer targetPkg.Close()

	_, err = ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 2,
		InsertAfter:       1,
		LayoutPolicy:      "import",
		ThemePolicy:       "import",
		NotesPolicy:       NotesDrop,
	})
	require.NoError(t, err)

	err = ReplaceText(&ReplaceTextRequest{
		Package:     targetPkg,
		SlideNumber: 2,
		Target:      "body:1",
		NewText:     "Imported slide body",
	})
	require.NoError(t, err)

	graph, err := inspect.ParsePresentation(targetPkg)
	require.NoError(t, err)
	slide2, err := targetPkg.ReadXMLPart(graph.Slides[1].PartURI)
	require.NoError(t, err)
	slide2Text, err := slide2.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slide2Text, "Imported slide body")
}

func TestReplaceText_AmbiguousRoleFails(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	created, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout5.xml",
		InsertAfter:   1,
	})
	require.NoError(t, err)

	err = ReplaceText(&ReplaceTextRequest{
		Package:     pkg,
		SlideNumber: created.NewSlideNumber,
		Target:      "body",
		NewText:     "won't work",
	})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "ambiguous target")
}

func TestReplaceText_TargetNotFound(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	err = ReplaceText(&ReplaceTextRequest{
		Package:     pkg,
		SlideNumber: 1,
		Target:      "shape:9999",
		NewText:     "won't work",
	})
	require.Error(t, err)
	assert.True(t, strings.Contains(err.Error(), "target not found") || strings.Contains(err.Error(), "not found"))
}
