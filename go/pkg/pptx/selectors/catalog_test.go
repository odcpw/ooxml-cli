package selectors_test

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestBuildSlideCatalog_BodyAliasesResolveOnImportedSlide(t *testing.T) {
	sourcePkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer sourcePkg.Close()

	targetPkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	defer targetPkg.Close()

	_, err = mutate.ImportSlide(&mutate.ImportSlideRequest{
		TargetPackage:     targetPkg,
		SourcePackage:     sourcePkg,
		SourceSlideNumber: 2,
		InsertAfter:       1,
		LayoutPolicy:      "import",
		ThemePolicy:       "import",
		NotesPolicy:       mutate.NotesDrop,
	})
	require.NoError(t, err)

	catalog, err := selectors.BuildSlideCatalog(targetPkg, 2)
	require.NoError(t, err)

	bodyTarget, err := catalog.ResolveTarget("body")
	require.NoError(t, err)
	bodyIdxTarget, err := catalog.ResolveTarget("body:1")
	require.NoError(t, err)
	shapeTarget, err := catalog.ResolveTarget("shape:3")
	require.NoError(t, err)

	assert.Equal(t, bodyTarget.ShapeID, bodyIdxTarget.ShapeID)
	assert.Equal(t, bodyTarget.ShapeID, shapeTarget.ShapeID)
	assert.Contains(t, bodyTarget.Selectors, "body")
	assert.Contains(t, bodyTarget.Selectors, "body:1")
}

func TestBuildSlideCatalog_AmbiguousRoleFailsLoudly(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	created, err := mutate.NewSlideFromLayout(&mutate.NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout5.xml",
		InsertAfter:   1,
	})
	require.NoError(t, err)

	catalog, err := selectors.BuildSlideCatalog(pkg, created.NewSlideNumber)
	require.NoError(t, err)

	_, err = catalog.ResolveTarget("body")
	require.Error(t, err)
	assert.Contains(t, err.Error(), "ambiguous target")
	assert.Contains(t, err.Error(), "body:1")
	assert.Contains(t, err.Error(), "body:3")

	firstBody, err := catalog.ResolveTarget("body:1")
	require.NoError(t, err)
	secondBody, err := catalog.ResolveTarget("body:3")
	require.NoError(t, err)
	assert.NotEqual(t, firstBody.ShapeID, secondBody.ShapeID)
}

func TestBuildSlideCatalog_NotFoundListsAvailableSelectors(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	catalog, err := selectors.BuildSlideCatalog(pkg, 2)
	require.NoError(t, err)

	_, err = catalog.ResolveTarget("body:99")
	require.Error(t, err)
	assert.Contains(t, err.Error(), "target not found")
	assert.True(t, strings.Contains(err.Error(), "body") || strings.Contains(err.Error(), "body:1"))
}

func TestBuildSlideCatalog_TableSelectors(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-slide/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	catalog, err := selectors.BuildSlideCatalog(pkg, 2)
	require.NoError(t, err)

	tableTarget, err := catalog.ResolveTarget("table:1")
	require.NoError(t, err)
	assert.Equal(t, 2, tableTarget.ShapeID)
	assert.Equal(t, "table", tableTarget.TargetKind)
	assert.Equal(t, "table:1", tableTarget.PrimarySelector)
	assert.Contains(t, tableTarget.Selectors, "table:1")
	assert.Contains(t, tableTarget.Selectors, "shape:2")
	assert.Contains(t, tableTarget.Selectors, "~Table 1")

	nameTarget, err := catalog.ResolveTarget("~Table 1")
	require.NoError(t, err)
	assert.Equal(t, tableTarget.ShapeID, nameTarget.ShapeID)
}
