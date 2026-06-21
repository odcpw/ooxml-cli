package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPPTXSlidesMoveJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "moved.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "move", fixturePath, "1", "2",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result SlidesMoveResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, 1, result.FromPosition)
	assert.Equal(t, 2, result.ToPosition)
	assert.False(t, result.IsNoOp)
	assert.NotEmpty(t, result.SlideURI)
	require.NotNil(t, result.Destination)
	assert.Equal(t, outPath, result.Destination.File)
	assert.Equal(t, 2, result.Destination.Number)
	assert.Equal(t, result.SlideURI, result.Destination.PartURI)
	assert.Contains(t, result.ReadbackCommand, "pptx slides show")
	assert.Contains(t, result.SlidesListCommand, "pptx slides list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForXLSXTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForXLSXTest(t, result.ReadbackCommand)
	var show SlidesShowResult
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	require.Len(t, show.Slides, 1)
	assert.Equal(t, result.SlideURI, show.Slides[0].PartURI)

	list := readPPTXSlidesListFromGeneratedCommandForTest(t, result.SlidesListCommand)
	require.Len(t, list.Slides, 2)
	assert.Equal(t, result.SlideURI, list.Slides[1].PartURI)
}

func TestPPTXSlidesMoveDryRunReadbackTemplates(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "move", fixturePath, "1", "2",
		"--dry-run",
	)
	require.NoError(t, err)

	var result SlidesMoveResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	require.NotNil(t, result.Destination)
	assert.Empty(t, result.Destination.File)
	assert.Equal(t, 2, result.Destination.Number)
	assert.Empty(t, result.ReadbackCommand)
	assert.Empty(t, result.SlidesListCommand)
	assert.Empty(t, result.ValidateCommand)
	assert.Empty(t, result.RenderCommand)
	assert.Contains(t, result.ReadbackCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.SlidesListCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.ValidateCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.RenderCommandTemplate, "<out.pptx>")

	pkg, err := opc.Open(fixturePath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)
	assert.NotEqual(t, result.SlideURI, graph.Slides[1].PartURI)
}

func TestPPTXSlidesReorderJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	original := readPPTXSlidesListForTest(t, fixturePath)
	require.Len(t, original.Slides, 2)
	outPath := filepath.Join(t.TempDir(), "reordered.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "reorder", fixturePath, "2,1",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result SlidesReorderResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, []int{2, 1}, result.NewOrder)
	assert.Equal(t, 2, result.SlideCount)
	assert.Contains(t, result.SlidesListCommand, "pptx slides list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForXLSXTest(t, result.ValidateCommand)
	list := readPPTXSlidesListFromGeneratedCommandForTest(t, result.SlidesListCommand)
	require.Len(t, list.Slides, 2)
	assert.Equal(t, original.Slides[1].PartURI, list.Slides[0].PartURI)
	assert.Equal(t, original.Slides[0].PartURI, list.Slides[1].PartURI)
}

func TestPPTXSlidesDeleteJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "deleted.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "delete", fixturePath, "2",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result SlidesDeleteResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, 2, result.DeletedSlide)
	assert.NotEmpty(t, result.RemovedURI)
	assert.Equal(t, 1, result.RemainingSlides)
	assert.Contains(t, result.SlidesListCommand, "pptx slides list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForXLSXTest(t, result.ValidateCommand)
	list := readPPTXSlidesListFromGeneratedCommandForTest(t, result.SlidesListCommand)
	require.Len(t, list.Slides, 1)
	assert.NotEqual(t, result.RemovedURI, list.Slides[0].PartURI)
}

func TestPPTXSlidesReorderAndDeleteDryRunTemplates(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	reorderOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "reorder", fixturePath, "2,1",
		"--dry-run",
	)
	require.NoError(t, err)
	var reorder SlidesReorderResult
	require.NoError(t, json.Unmarshal([]byte(reorderOutput), &reorder))
	assert.Equal(t, fixturePath, reorder.File)
	assert.Empty(t, reorder.Output)
	assert.True(t, reorder.DryRun)
	assert.Equal(t, []int{2, 1}, reorder.NewOrder)
	assert.Empty(t, reorder.SlidesListCommand)
	assert.Empty(t, reorder.ValidateCommand)
	assert.Empty(t, reorder.RenderCommand)
	assert.Contains(t, reorder.SlidesListCommandTemplate, "<out.pptx>")
	assert.Contains(t, reorder.ValidateCommandTemplate, "<out.pptx>")
	assert.Contains(t, reorder.RenderCommandTemplate, "<out.pptx>")

	deleteOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "delete", fixturePath, "2",
		"--dry-run",
	)
	require.NoError(t, err)
	var deleted SlidesDeleteResult
	require.NoError(t, json.Unmarshal([]byte(deleteOutput), &deleted))
	assert.Equal(t, fixturePath, deleted.File)
	assert.Empty(t, deleted.Output)
	assert.True(t, deleted.DryRun)
	assert.Equal(t, 1, deleted.RemainingSlides)
	assert.Empty(t, deleted.SlidesListCommand)
	assert.Empty(t, deleted.ValidateCommand)
	assert.Empty(t, deleted.RenderCommand)
	assert.Contains(t, deleted.SlidesListCommandTemplate, "<out.pptx>")
	assert.Contains(t, deleted.ValidateCommandTemplate, "<out.pptx>")
	assert.Contains(t, deleted.RenderCommandTemplate, "<out.pptx>")

	original := readPPTXSlidesListForTest(t, fixturePath)
	require.Len(t, original.Slides, 2)
}

func readPPTXSlidesListForTest(t *testing.T, deckPath string) SlidesListResult {
	t.Helper()
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "slides", "list", deckPath)
	require.NoError(t, err)
	var result SlidesListResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	return result
}

func readPPTXSlidesListFromGeneratedCommandForTest(t *testing.T, command string) SlidesListResult {
	t.Helper()
	output := executeGeneratedOOXMLCommandForXLSXTest(t, command)
	var result SlidesListResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	return result
}

func TestPPTXSlidesDeleteJSONOutputFileUsesGlobalOutput(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "deleted.pptx")
	jsonPath := filepath.Join(tmpDir, "delete.json")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "slides", "delete", fixturePath, "2",
		"--out", outPath,
		"--format", "json",
		"-o", jsonPath,
	})
	require.NoError(t, cmd.Execute())

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var result SlidesDeleteResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, outPath, result.Output)
	assert.Equal(t, 1, result.RemainingSlides)
}
