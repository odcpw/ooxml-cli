package mutate

import (
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

func TestMergeDeck_SimpleMinimalMerge(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "merged-simple.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := MergeDeck(&MergeDeckRequest{
		TargetPackage: targetPkg,
		SourcePackage: sourcePkg,
		LayoutPolicy:  "reuse",
		ThemePolicy:   "reuse",
		NotesPolicy:   NotesDrop,
	})
	require.NoError(t, err)
	assert.Equal(t, 1, res.MergedSlideCount)
	require.Len(t, res.ImportedSlides, 1)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify result has 2 slides (1 original + 1 merged)
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)
	requirePackageValid(t, result)
}

func TestMergeDeck_MultiSlideSource(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/title-content/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "merged-multi.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	sourceGraph, err := inspect.ParsePresentation(sourcePkg)
	require.NoError(t, err)
	sourceSlideCount := len(sourceGraph.Slides)
	sourcePkg.Close()

	sourcePkg = openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := MergeDeck(&MergeDeckRequest{
		TargetPackage: targetPkg,
		SourcePackage: sourcePkg,
		LayoutPolicy:  "reuse",
		ThemePolicy:   "reuse",
		NotesPolicy:   NotesDrop,
	})
	require.NoError(t, err)
	assert.Equal(t, sourceSlideCount, res.MergedSlideCount)
	require.Len(t, res.ImportedSlides, sourceSlideCount)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify result has correct total
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 1+sourceSlideCount)
	requirePackageValid(t, result)
}

func TestMergeDeck_WithMedia(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/picture-placeholder/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "merged-media.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := MergeDeck(&MergeDeckRequest{
		TargetPackage: targetPkg,
		SourcePackage: sourcePkg,
		LayoutPolicy:  "reuse",
		ThemePolicy:   "reuse",
		NotesPolicy:   NotesDrop,
	})
	require.NoError(t, err)
	assert.Greater(t, res.MergedSlideCount, 0)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify result
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Greater(t, len(graph.Slides), 1)
	requirePackageValid(t, result)
}

func TestMergeDeck_WithNotesMediaCopiesNotesDependencies(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/slide-assembly-notes-media/presentation.pptx"
	targetFixture := "../../../testdata/pptx/slide-assembly-multi/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "merged-notes-media.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := MergeDeck(&MergeDeckRequest{
		TargetPackage: targetPkg,
		SourcePackage: sourcePkg,
		LayoutPolicy:  "import",
		ThemePolicy:   "import",
		NotesPolicy:   NotesClone,
	})
	require.NoError(t, err)
	assert.Equal(t, 5, res.MergedSlideCount)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 10)
	for i := 5; i < len(graph.Slides); i++ {
		assert.NotEmpty(t, graph.Slides[i].NotesPartURI, "imported slide %d should keep notes", i+1)
	}
	requirePackageValid(t, result)
}
