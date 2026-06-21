package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPPTXLayoutsImportJSONReadbackCommands(t *testing.T) {
	targetPath := getTestFilePath("minimal-title", "presentation.pptx")
	sourcePath := createImportSourceDeckForReadbackTest(t)
	outPath := filepath.Join(t.TempDir(), "layout-imported.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "import", targetPath,
		"--source", sourcePath,
		"--layout", "1",
		"--theme-policy", "import",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result importLayoutResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, targetPath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.True(t, result.Imported)
	assert.True(t, result.MasterImported)
	assert.Equal(t, "ImportedTitleSlide", result.Name)
	assert.NotEmpty(t, result.TargetLayoutURI)
	assert.NotEmpty(t, result.TargetMasterURI)
	assert.Contains(t, result.ReadbackCommand, "pptx layouts show")
	assert.Contains(t, result.LayoutsListCommand, "pptx layouts list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForLayoutsTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForLayoutsTest(t, result.ReadbackCommand)
	var show LayoutShowOutput
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	assert.Equal(t, result.Name, show.Name)
	assert.Equal(t, result.TargetLayoutURI, show.PartURI)

	listOutput := executeGeneratedOOXMLCommandForLayoutsTest(t, result.LayoutsListCommand)
	var list LayoutListOutput
	require.NoError(t, json.Unmarshal([]byte(listOutput), &list))
	assert.True(t, layoutListContainsNameForTest(list, result.Name))
}

func TestPPTXMastersImportJSONReadbackCommands(t *testing.T) {
	targetPath := getTestFilePath("minimal-title", "presentation.pptx")
	sourcePath := createImportSourceDeckForReadbackTest(t)
	outPath := filepath.Join(t.TempDir(), "master-imported.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "masters", "import", targetPath,
		"--source", sourcePath,
		"--master", "1",
		"--theme-policy", "import",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result importMasterResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, targetPath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.True(t, result.Imported)
	assert.Equal(t, 2, result.TargetMaster)
	assert.NotEmpty(t, result.TargetMasterURI)
	assert.Greater(t, result.LayoutCount, 0)
	assert.Contains(t, result.ReadbackCommand, "pptx masters show")
	assert.Contains(t, result.MastersListCommand, "pptx masters list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForLayoutsTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForLayoutsTest(t, result.ReadbackCommand)
	var show MasterDetail
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	assert.Equal(t, result.TargetMasterURI, show.URI)
	assert.Equal(t, result.TargetMaster, show.Index)

	listOutput := executeGeneratedOOXMLCommandForLayoutsTest(t, result.MastersListCommand)
	var list MasterListResult
	require.NoError(t, json.Unmarshal([]byte(listOutput), &list))
	assert.True(t, masterListContainsURIForTest(list, result.TargetMasterURI))
}

func TestPPTXMastersAddPlaceholderJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "master-placeholder.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "masters", "add-placeholder", fixturePath,
		"--master", "1",
		"--type", "pic",
		"--idx", "0",
		"--bounds", "1000,2000,3000,4000",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result addMasterPlaceholderResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, 1, result.Master)
	assert.Equal(t, "pic", result.Type)
	assert.Equal(t, 0, result.Idx)
	assert.NotEmpty(t, result.MasterURI)
	assert.Contains(t, result.ReadbackCommand, "pptx masters show")
	assert.Contains(t, result.MastersListCommand, "pptx masters list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForLayoutsTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForLayoutsTest(t, result.ReadbackCommand)
	var show MasterDetail
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	assert.Equal(t, result.MasterURI, show.URI)
	assert.True(t, masterShowContainsPlaceholderForTest(show, "pic:0"))
}

func TestPPTXMastersAddPlaceholderMissingMasterListsDiscovery(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	_, err = executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "masters", "add-placeholder", fixturePath,
		"--master", "999",
		"--type", "text",
		"--bounds", "1000,2000,3000,4000",
		"--dry-run",
	)
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok, "error should be CLIError: %v", err)
	assert.Equal(t, ExitTargetNotFound, cliErr.ExitCode)
	assert.Contains(t, err.Error(), "master not found: 999")
	assert.Contains(t, err.Error(), "did you mean:")
	assert.Contains(t, err.Error(), "ooxml --json pptx masters list <file>")
}

func TestPPTXMasterMutationDryRunTemplates(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "masters", "add-placeholder", fixturePath,
		"--master", "1",
		"--type", "text",
		"--bounds", "1000,2000,3000,4000",
		"--dry-run",
	)
	require.NoError(t, err)

	var result addMasterPlaceholderResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	assert.Empty(t, result.ReadbackCommand)
	assert.Empty(t, result.MastersListCommand)
	assert.Empty(t, result.ValidateCommand)
	assert.Empty(t, result.RenderCommand)
	assert.Contains(t, result.ReadbackCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.MastersListCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.ValidateCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.RenderCommandTemplate, "<out.pptx>")
}

func createImportSourceDeckForReadbackTest(t *testing.T) string {
	t.Helper()
	sourceFixture := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(sourceFixture); err != nil {
		t.Skipf("source fixture not found: %v", err)
	}
	sourcePath := filepath.Join(t.TempDir(), "source-import-readback.pptx")
	sourcePkg, err := opc.Open(sourceFixture)
	require.NoError(t, err)
	_, err = mutate.RenameLayout(&mutate.RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "ImportedTitleSlide",
	})
	require.NoError(t, err)
	require.NoError(t, sourcePkg.SaveAs(sourcePath))
	require.NoError(t, sourcePkg.Close())
	return sourcePath
}

func masterShowContainsPlaceholderForTest(result MasterDetail, key string) bool {
	for _, placeholder := range result.Placeholders {
		if placeholder.Key == key {
			return true
		}
	}
	return false
}

func masterListContainsURIForTest(result MasterListResult, uri string) bool {
	for _, master := range result.Masters {
		if master.URI == uri {
			return true
		}
	}
	return false
}
