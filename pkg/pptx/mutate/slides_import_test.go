package mutate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestImportSlide_BasicImport(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/title-content/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "imported.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 1,
		InsertAfter:       0,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.NoError(t, err)
	assert.NotEmpty(t, res.NewSlideURI)
	assert.NotZero(t, res.NewSlideID)
	assert.Equal(t, 2, res.NewSlideNumber)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify result
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)
	requirePackageValid(t, result)
}

func TestImportSlide_PreservesSourcePackage(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/title-content/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"

	// Perform import
	sourcePkg := openMutatePackage(t, sourceFixture)
	sourceGraph, err := inspect.ParsePresentation(sourcePkg)
	require.NoError(t, err)
	sourceSlideCount := len(sourceGraph.Slides)
	sourcePkg.Close()

	sourcePkg = openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()
	targetPkg := openMutatePackage(t, targetFixture)
	defer targetPkg.Close()

	_, err = ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 1,
		InsertAfter:       0,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.NoError(t, err)

	// Verify source is unchanged
	verifySourceGraph, err := inspect.ParsePresentation(sourcePkg)
	require.NoError(t, err)
	assert.Equal(t, sourceSlideCount, len(verifySourceGraph.Slides))
}

func TestImportSlide_CopiesMediaWhenPresent(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/picture-placeholder/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "imported-media.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 2,
		InsertAfter:       0,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.NoError(t, err)
	require.Equal(t, 2, res.NewSlideNumber)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)
	requirePackageValid(t, result)
}

func TestImportSlide_WithNotesDrop(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/notes-slide/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "imported-notes-drop.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	res, err := ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 1,
		InsertAfter:       0,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.NoError(t, err)
	// Notes should not be imported in drop mode
	assert.Empty(t, res.NotesURI)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)
	requirePackageValid(t, result)
}

func TestImportSlide_InvalidSourceSlideNumber(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/title-content/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	defer targetPkg.Close()

	_, err := ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 999,
		InsertAfter:       0,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "not found")
}

func TestImportSlide_InvalidInsertAfter(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/title-content/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"

	sourcePkg := openMutatePackage(t, sourceFixture)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	defer targetPkg.Close()

	_, err := ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 1,
		InsertAfter:       999,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "out of range")
}

func TestImportSlide_ImportLayoutAndMaster(t *testing.T) {
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "imported-layout.pptx")

	sourcePkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	_, err := RenameLayout(&RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, targetFixture)
	_, err = ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 1,
		InsertAfter:       0,
		LayoutPolicy:      "import",
		ThemePolicy:       "import",
		NotesPolicy:       NotesDrop,
	})
	require.NoError(t, err)

	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	// Verify the imported layout is registered through the imported master chain.
	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)
	assert.GreaterOrEqual(t, len(graph.Masters), 2)
	var registered bool
	for _, layout := range graph.Layouts {
		if layout.PartURI == graph.Slides[1].LayoutPartURI {
			registered = true
			break
		}
	}
	assert.True(t, registered, "imported slide layout should be discoverable via presentation layouts")
	requirePackageValid(t, result)
}

func TestImportSlide_ReuseRequiresExactCompatibleLayout(t *testing.T) {
	sourceFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	renamedSourcePath := filepath.Join(t.TempDir(), "renamed-source.pptx")

	sourcePkg := openMutatePackage(t, sourceFixture)
	_, err := RenameLayout(&RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Renamed Source Title Slide",
	})
	require.NoError(t, err)
	require.NoError(t, sourcePkg.SaveAs(renamedSourcePath))
	sourcePkg.Close()

	renamedSource := openMutatePackage(t, renamedSourcePath)
	defer renamedSource.Close()
	targetPkg := openMutatePackage(t, targetFixture)
	defer targetPkg.Close()

	_, err = ImportSlide(&ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     renamedSource,
		SourceSlideNumber: 1,
		InsertAfter:       0,
		LayoutPolicy:      "reuse",
		ThemePolicy:       "reuse",
		NotesPolicy:       NotesDrop,
	})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "layout-policy reuse requires an explicit compatible target layout")
}
