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

func TestCloneSlide_WithOut(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "cloned.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "clone-slide", fixturePath, "--slide", "1", "--out", outPath})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	assert.Len(t, graph.Slides, 3)
}

func TestCloneSlide_JSONOutput(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "cloned.pptx")
	jsonPath := filepath.Join(tmpDir, "clone.json")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "clone-slide", fixturePath, "--slide", "1", "--out", outPath, "--format", "json", "-o", jsonPath})
	require.NoError(t, cmd.Execute())

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var result cloneSlideResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, 1, result.SourceSlide)
	assert.Equal(t, 1, result.InsertAfter)
	assert.Equal(t, 2, result.SlideCountBefore)
	assert.Equal(t, 3, result.SlideCountAfter)
	assert.Equal(t, 2, result.NewSlideNumber)
	assert.NotZero(t, result.NewSlideID)
	assert.NotEmpty(t, result.NewSlideURI)
	require.NotNil(t, result.Source)
	assert.Equal(t, fixturePath, result.Source.File)
	assert.Equal(t, 1, result.Source.Number)
	assert.NotEmpty(t, result.Source.PartURI)
	assert.NotEmpty(t, result.Source.LayoutPartURI)
	require.NotNil(t, result.Destination)
	assert.Equal(t, outPath, result.Destination.File)
	assert.Equal(t, result.NewSlideNumber, result.Destination.Number)
	assert.Equal(t, result.NewSlideURI, result.Destination.PartURI)
	assert.NotEmpty(t, result.Destination.LayoutPartURI)
	assert.NotEmpty(t, result.ReadbackCommand)
	assert.Contains(t, result.ReadbackCommand, "pptx slides show")
	assert.Contains(t, result.ReadbackCommand, outPath)
	assert.Contains(t, result.SlidesListCommand, "pptx slides list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")
}

func TestCloneSlide_InsertAfterAndQuotedReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "clone output with spaces.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "clone-slide", fixturePath,
		"--slide", "1",
		"--insert-after", "2",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result cloneSlideResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, 2, result.InsertAfter)
	assert.Equal(t, 3, result.NewSlideNumber)
	assert.Equal(t, 3, result.Destination.Number)
	assert.Contains(t, result.ReadbackCommand, "'"+outPath+"'")
	assert.Contains(t, result.SlidesListCommand, "'"+outPath+"'")
	assert.Contains(t, result.ValidateCommand, "'"+outPath+"'")
	assert.Contains(t, result.RenderCommand, "'"+outPath+"'")

	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "show", outPath,
		"--slide", "3",
	)
	require.NoError(t, err)
	var show SlidesShowResult
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	require.Len(t, show.Slides, 1)
	assert.Equal(t, result.NewSlideURI, show.Slides[0].PartURI)

	listOutput, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "slides", "list", outPath)
	require.NoError(t, err)
	var list SlidesListResult
	require.NoError(t, json.Unmarshal([]byte(listOutput), &list))
	require.Len(t, list.Slides, 3)
	assert.Equal(t, result.NewSlideURI, list.Slides[2].PartURI)
}

func TestCloneSlide_DryRunReportsReadbackWithoutWriting(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "clone-slide", fixturePath,
		"--slide", "1",
		"--dry-run",
	)
	require.NoError(t, err)

	var result cloneSlideResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	assert.Equal(t, 2, result.SlideCountBefore)
	assert.Equal(t, 3, result.SlideCountAfter)
	assert.Equal(t, 2, result.NewSlideNumber)
	require.NotNil(t, result.Source)
	assert.Equal(t, fixturePath, result.Source.File)
	require.NotNil(t, result.Destination)
	assert.Empty(t, result.Destination.File)
	assert.Equal(t, 2, result.Destination.Number)
	assert.Empty(t, result.ReadbackCommand)
	assert.Empty(t, result.SlidesListCommand)
	assert.Contains(t, result.ReadbackCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.SlidesListCommandTemplate, "<out.pptx>")

	pkg, err := opc.Open(fixturePath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	assert.Len(t, graph.Slides, 2)
}

func TestCloneSlide_MissingSourceSlideReturnsTargetNotFound(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	_, err = executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "clone-slide", fixturePath,
		"--slide", "99",
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "clone-slide"}, err, ExitTargetNotFound)
	assert.Contains(t, err.Error(), "slide 99")
}

func TestCloneSlide_MissingSlideFlag(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "cloned.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "clone-slide", fixturePath, "--out", outPath})
	err = cmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "slide")
}
