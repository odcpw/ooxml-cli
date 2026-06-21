package mutate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const imageRelationshipType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"

func TestImportMaster_RegistersImportedMasterAndLayouts(t *testing.T) {
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

	res, err := ImportMaster(&ImportMasterRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceMasterURI: "/ppt/slideMasters/slideMaster1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	assert.True(t, res.Imported)
	assert.NotEmpty(t, res.TargetMasterURI)
	assert.NotEmpty(t, res.Layouts)

	graph, err := inspect.ParsePresentation(targetPkg)
	require.NoError(t, err)
	assert.Len(t, graph.Masters, 2)

	var foundMaster bool
	for _, master := range graph.Masters {
		if master.PartURI == res.TargetMasterURI {
			foundMaster = true
			assert.Len(t, master.LinkedLayoutURIs, len(res.Layouts))
			break
		}
	}
	assert.True(t, foundMaster, "imported master should be registered in presentation.xml")

	for _, layout := range res.Layouts {
		var foundLayout bool
		for _, targetLayout := range graph.Layouts {
			if targetLayout.PartURI == layout.TargetLayoutURI {
				foundLayout = true
				assert.Equal(t, res.TargetMasterURI, targetLayout.MasterPartURI)
				break
			}
		}
		assert.True(t, foundLayout, "imported layout %s should be discoverable", layout.TargetLayoutURI)
	}
	requirePackageValid(t, targetPkg)
}

func TestImportMaster_ReusesExactImportedMaster(t *testing.T) {
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

	first, err := ImportMaster(&ImportMasterRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceMasterURI: "/ppt/slideMasters/slideMaster1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	second, err := ImportMaster(&ImportMasterRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceMasterURI: "/ppt/slideMasters/slideMaster1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	assert.True(t, first.Imported)
	assert.False(t, second.Imported)
	assert.Equal(t, first.TargetMasterURI, second.TargetMasterURI)

	graph, err := inspect.ParsePresentation(targetPkg)
	require.NoError(t, err)
	assert.Len(t, graph.Masters, 2)
	requirePackageValid(t, targetPkg)
}

func TestImportMaster_CopiesDependentMasterMedia(t *testing.T) {
	sourcePkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	defer sourcePkg.Close()
	masterURI := "/ppt/slideMasters/slideMaster1.xml"
	sourceMediaURI := "/ppt/media/image99.png"
	sourceMedia := []byte{0x89, 0x50, 0x4e, 0x47, 0x00, 0x01, 0x02, 0x03}
	require.NoError(t, sourcePkg.AddPart(sourceMediaURI, sourceMedia, "image/png", nil))
	masterRels := sourcePkg.ListRelationships(masterURI)
	masterRels = append(masterRels, opc.RelationshipInfo{
		SourceURI: masterURI,
		ID:        AllocateRelationshipID(masterRels),
		Type:      imageRelationshipType,
		Target:    "../media/image99.png",
	})
	masterRelsXML, err := BuildRelationshipsXML(masterRels)
	require.NoError(t, err)
	require.NoError(t, sourcePkg.ReplaceRawPart(relsURIForPart(masterURI), masterRelsXML, relationshipsContentType))

	targetPkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	defer targetPkg.Close()
	res, err := ImportMaster(&ImportMasterRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceMasterURI: masterURI,
		ThemePolicy:     "import",
	})
	require.NoError(t, err)

	var copiedMediaURI string
	for _, rel := range targetPkg.ListRelationships(res.TargetMasterURI) {
		if rel.Type != imageRelationshipType {
			continue
		}
		copiedMediaURI = opc.ResolveRelationshipTarget(res.TargetMasterURI, rel.Target)
		break
	}
	require.NotEmpty(t, copiedMediaURI, "expected imported master to retain image relationship")
	copiedMedia, err := targetPkg.ReadRawPart(copiedMediaURI)
	require.NoError(t, err)
	assert.Equal(t, sourceMedia, copiedMedia)
	requirePackageValid(t, targetPkg)
}

func TestImportMaster_CommandRoundTrip(t *testing.T) {
	targetFixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "imported-master.pptx")

	sourcePkg := openMutatePackage(t, "../../../testdata/pptx/minimal-title/presentation.pptx")
	_, err := RenameLayout(&RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	defer sourcePkg.Close()
	targetPkg := openMutatePackage(t, targetFixture)
	res, err := ImportMaster(&ImportMasterRequest{
		TargetPackage:   targetPkg,
		SourcePackage:   sourcePkg,
		SourceMasterURI: "/ppt/slideMasters/slideMaster1.xml",
		ThemePolicy:     "import",
	})
	require.NoError(t, err)
	require.NoError(t, targetPkg.SaveAs(outPath))
	targetPkg.Close()

	result := openMutatePackage(t, outPath)
	defer result.Close()
	graph, err := inspect.ParsePresentation(result)
	require.NoError(t, err)
	assert.Len(t, graph.Masters, 2)
	assert.NotEmpty(t, res.TargetMasterURI)
	requirePackageValid(t, result)
}
