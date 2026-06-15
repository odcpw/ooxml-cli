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

func TestTemplateCompile_SimpleSpec(t *testing.T) {
	// Paths to test fixtures
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", archetypePath,
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	// Verify output file exists
	_, err = os.Stat(outPath)
	require.NoError(t, err)

	// Verify output PPTX has correct number of slides
	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	// The spec has 2 slides
	assert.Len(t, graph.Slides, 2)
}

func TestTemplateCompile_WithJSONOutput(t *testing.T) {
	// Paths to test fixtures
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output.pptx")
	jsonPath := filepath.Join(tmpDir, "result.json")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", archetypePath,
		"--out", outPath,
		"--format", "json",
		"-o", jsonPath,
	})
	require.NoError(t, cmd.Execute())

	// Verify JSON output
	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)

	var result map[string]interface{}
	require.NoError(t, json.Unmarshal(data, &result))

	assert.Equal(t, outPath, result["outputPath"])
	assert.Equal(t, float64(2), result["slideCount"]) // JSON numbers are floats
	assert.Greater(t, result["slotsSucceeded"], float64(0))
}

func TestTemplateCompile_MissingArchetype(t *testing.T) {
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)

	outPath := filepath.Join(t.TempDir(), "output.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", "/nonexistent/file.pptx",
		"--out", outPath,
	})
	err = cmd.Execute()
	require.Error(t, err)
}

func TestTemplateCompile_MissingRequiredFlags(t *testing.T) {
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", archetypePath,
		// Missing --out flag
	})
	err = cmd.Execute()
	require.Error(t, err)
}

func TestTemplateCompile_InvalidManifest(t *testing.T) {
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	outPath := filepath.Join(t.TempDir(), "output.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		"/nonexistent/manifest.json", specPath,
		"--archetype", archetypePath,
		"--out", outPath,
	})
	err = cmd.Execute()
	require.Error(t, err)
}

func TestTemplateCompile_WithContinueOnErrorFlag(t *testing.T) {
	// Paths to test fixtures
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", archetypePath,
		"--out", outPath,
		"--continue-on-error",
	})
	require.NoError(t, cmd.Execute())

	// Verify output file exists
	_, err = os.Stat(outPath)
	require.NoError(t, err)
}

func TestTemplateCompile_WithImageBaseDir(t *testing.T) {
	// Paths to test fixtures
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output.pptx")
	imageBaseDir, err := filepath.Abs("../../testdata/pptx/template-branded")
	require.NoError(t, err)

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", archetypePath,
		"--out", outPath,
		"--image-base-dir", imageBaseDir,
	})
	require.NoError(t, cmd.Execute())

	// Verify output file exists
	_, err = os.Stat(outPath)
	require.NoError(t, err)

	// Verify output PPTX has correct number of slides
	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)

	// The spec has 2 slides
	assert.Len(t, graph.Slides, 2)
}

func TestTemplateCompile_JSONFormatOutput(t *testing.T) {
	// This test verifies that --format json with JSON output structure includes slotsSucceeded
	manifestPath, err := filepath.Abs("../../testdata/pptx/template-branded/manifest.json")
	require.NoError(t, err)
	specPath, err := filepath.Abs("../../testdata/pptx/template-branded/spec-simple.yaml")
	require.NoError(t, err)
	archetypePath, err := filepath.Abs("../../testdata/pptx/template-branded/presentation.pptx")
	require.NoError(t, err)

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output.pptx")
	jsonPath := filepath.Join(tmpDir, "result.json")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "template", "compile",
		manifestPath, specPath,
		"--archetype", archetypePath,
		"--out", outPath,
		"--format", "json",
		"-o", jsonPath,
	})
	require.NoError(t, cmd.Execute())

	// Verify JSON output file exists
	_, err = os.Stat(jsonPath)
	require.NoError(t, err)

	// Verify JSON output structure
	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)

	var result map[string]interface{}
	require.NoError(t, json.Unmarshal(data, &result))

	// Verify required fields
	assert.Contains(t, result, "outputPath")
	assert.Contains(t, result, "slideCount")
	assert.Contains(t, result, "slotsSucceeded")
	assert.Contains(t, result, "slotsAttempted")
	assert.Contains(t, result, "duration")

	// Verify slotsSucceeded is greater than 0
	slotsSucceeded, ok := result["slotsSucceeded"].(float64)
	require.True(t, ok, "slotsSucceeded should be a number")
	assert.Greater(t, slotsSucceeded, float64(0), "slotsSucceeded should be > 0 for successful compilation")
}

func resetTemplateCompileGlobals() {
	compileArchetype = ""
	compileOutput = ""
	compileContinueError = false
	compileImageBaseDir = ""
	compileFormat = ""
}

func init() {
	// Ensure cleanup happens for template compile tests
	originalResetTestGlobals := resetTestGlobals
	resetTestGlobals = func() {
		originalResetTestGlobals()
		resetTemplateCompileGlobals()
	}
}
