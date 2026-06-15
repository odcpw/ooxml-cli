package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxdiff "github.com/ooxml-cli/ooxml-cli/pkg/pptx/diff"
	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDiffCommand_SemanticJSON(t *testing.T) {
	baselinePath, candidatePath := diffFixturePair(t)
	jsonPath := filepath.Join(t.TempDir(), "diff.json")

	origSemantic := semanticDiffFn
	defer func() { semanticDiffFn = origSemantic }()
	semanticDiffFn = func(a, b opc.PackageSession) (*pptxdiff.Report, error) {
		return &pptxdiff.Report{SlideCountA: 1, SlideCountB: 1, SlideCountEqual: true}, nil
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "diff", baselinePath, candidatePath, "--format", "json", "-o", jsonPath})
	require.NoError(t, cmd.Execute())

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var result DiffResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.True(t, result.Semantic.SlideCountEqual)
	assert.Equal(t, "disabled", result.Visual.Status)
}

func TestDiffCommand_RenderThresholdExceeded(t *testing.T) {
	baselinePath, candidatePath := diffFixturePair(t)
	outDir := filepath.Join(t.TempDir(), "diff-artifacts")
	jsonPath := filepath.Join(t.TempDir(), "diff.json")

	origSemantic := semanticDiffFn
	origRender := renderToPDFFn
	origRaster := rasterizeFn
	origVisual := visualDiffFn
	defer func() {
		semanticDiffFn = origSemantic
		renderToPDFFn = origRender
		rasterizeFn = origRaster
		visualDiffFn = origVisual
	}()

	semanticDiffFn = func(a, b opc.PackageSession) (*pptxdiff.Report, error) {
		return &pptxdiff.Report{SlideCountA: 1, SlideCountB: 1, SlideCountEqual: true}, nil
	}
	renderToPDFFn = func(pptxPath string, outDir string) (string, error) {
		require.NoError(t, os.MkdirAll(outDir, 0o755))
		pdfPath := filepath.Join(outDir, filepath.Base(pptxPath)+".pdf")
		require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))
		return pdfPath, nil
	}
	rasterizeFn = func(pdfPath string, outDir string, opts pkgrender.RasterizeOptions) ([]string, error) {
		img := filepath.Join(outDir, "slide-1.png")
		require.NoError(t, os.WriteFile(img, []byte("img"), 0o644))
		return []string{img}, nil
	}
	visualDiffFn = func(imgA, imgB, outDiff string) (float64, error) {
		require.NoError(t, os.WriteFile(outDiff, []byte("diff"), 0o644))
		return 0.05, nil
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "diff", baselinePath, candidatePath, "--render", "--out", outDir, "--threshold", "0.01", "--format", "json", "-o", jsonPath})
	err := cmd.Execute()
	require.Error(t, err)
	cliErr, ok := err.(*CLIError)
	require.True(t, ok)
	assert.Equal(t, ExitDiffThreshold, cliErr.ExitCode)
	assert.True(t, cliErr.Reported)

	data, readErr := os.ReadFile(jsonPath)
	require.NoError(t, readErr)
	var result DiffResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, "ok", result.Visual.Status)
	assert.False(t, result.Visual.Pass)
	require.Len(t, result.Visual.Slides, 1)
	assert.Equal(t, 0.05, result.Visual.Slides[0].Difference)
}

func TestDiffCommand_RenderUnavailablePartialSuccess(t *testing.T) {
	baselinePath, candidatePath := diffFixturePair(t)
	jsonPath := filepath.Join(t.TempDir(), "diff.json")

	origSemantic := semanticDiffFn
	origRender := renderToPDFFn
	defer func() {
		semanticDiffFn = origSemantic
		renderToPDFFn = origRender
	}()

	semanticDiffFn = func(a, b opc.PackageSession) (*pptxdiff.Report, error) {
		return &pptxdiff.Report{SlideCountA: 1, SlideCountB: 1, SlideCountEqual: true}, nil
	}
	renderToPDFFn = func(pptxPath string, outDir string) (string, error) {
		return "", &pkgrender.MissingDependencyError{Tool: "soffice"}
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "diff", baselinePath, candidatePath, "--render", "--format", "json", "-o", jsonPath})
	err := cmd.Execute()
	require.Error(t, err)
	cliErr, ok := err.(*CLIError)
	require.True(t, ok)
	assert.Equal(t, ExitPartialSuccess, cliErr.ExitCode)
	assert.True(t, cliErr.Reported)

	data, readErr := os.ReadFile(jsonPath)
	require.NoError(t, readErr)
	var result DiffResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, "unavailable", result.Visual.Status)
}

func diffFixturePair(t *testing.T) (string, string) {
	t.Helper()
	baselinePath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	candidatePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	return baselinePath, candidatePath
}
