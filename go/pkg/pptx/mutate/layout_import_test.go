package mutate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestImportLayout_RegistersSingleImportedLayout(t *testing.T) {
	sourcePkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	_, err := RenameLayout(&RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	defer targetPkg.Close()

	result, err := ImportLayout(&ImportLayoutRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceLayoutURI: "/ppt/slideLayouts/slideLayout1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	assert.True(t, result.Imported)
	assert.True(t, result.MasterImported)
	assert.NotEmpty(t, result.TargetLayoutURI)
	assert.NotEmpty(t, result.TargetMasterURI)

	graph, err := inspect.ParsePresentation(targetPkg)
	require.NoError(t, err)
	assert.Len(t, graph.Masters, 2)
	assert.Len(t, graph.Layouts, 12)

	var foundMaster bool
	for _, master := range graph.Masters {
		if master.PartURI == result.TargetMasterURI {
			foundMaster = true
			assert.Len(t, master.LinkedLayoutURIs, 1)
			assert.Equal(t, result.TargetLayoutURI, master.LinkedLayoutURIs[0])
		}
	}
	assert.True(t, foundMaster, "imported layout master should be registered")
	requirePackageValid(t, targetPkg)
}

func TestImportLayout_ReusesExactImportedLayout(t *testing.T) {
	sourcePkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	_, err := RenameLayout(&RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	defer sourcePkg.Close()

	targetPkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	defer targetPkg.Close()

	first, err := ImportLayout(&ImportLayoutRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceLayoutURI: "/ppt/slideLayouts/slideLayout1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	second, err := ImportLayout(&ImportLayoutRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceLayoutURI: "/ppt/slideLayouts/slideLayout1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)

	assert.True(t, first.Imported)
	assert.False(t, second.Imported)
	assert.Equal(t, first.TargetLayoutURI, second.TargetLayoutURI)

	graph, err := inspect.ParsePresentation(targetPkg)
	require.NoError(t, err)
	assert.Len(t, graph.Masters, 2)
	assert.Len(t, graph.Layouts, 12)
	requirePackageValid(t, targetPkg)
}

func TestImportLayout_CommandRoundTrip(t *testing.T) {
	sourcePkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	_, err := RenameLayout(&RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	sourcePath := filepath.Join(t.TempDir(), "source-layout-import.pptx")
	require.NoError(t, sourcePkg.SaveAs(sourcePath))
	require.NoError(t, sourcePkg.Close())

	targetPkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	importSourcePkg := openMutatePackage(t, sourcePath)
	defer importSourcePkg.Close()
	result, err := ImportLayout(&ImportLayoutRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   importSourcePkg,
		SourceLayoutURI: "/ppt/slideLayouts/slideLayout1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-imported.pptx")
	require.NoError(t, targetPkg.SaveAs(outPath))
	require.NoError(t, targetPkg.Close())

	resultPkg := openMutatePackage(t, outPath)
	defer resultPkg.Close()
	graph, err := inspect.ParsePresentation(resultPkg)
	require.NoError(t, err)
	assert.Len(t, graph.Masters, 2)
	assert.Len(t, graph.Layouts, 12)
	assert.NotEmpty(t, result.TargetLayoutURI)
	requirePackageValid(t, resultPkg)
}
