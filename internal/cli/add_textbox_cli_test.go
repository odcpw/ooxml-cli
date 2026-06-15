package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestAddTextbox_WithOut(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "textbox.pptx")

	cmd := newTestRootCmd(t)
	var stdout bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetArgs([]string{
		"--format", "json",
		"pptx", "add-textbox", fixturePath,
		"--slide", "1",
		"--x", "100000",
		"--y", "100000",
		"--cx", "3000000",
		"--cy", "800000",
		"--text", "Hello Rich Box",
		"--bold",
		"--color", "FF0000",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	var result AddTextboxCLIResult
	require.NoError(t, json.Unmarshal(stdout.Bytes(), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.NotZero(t, result.ShapeID)
	assert.NotEmpty(t, result.ShapeName)
	require.NotNil(t, result.Destination)
	assert.Equal(t, outPath, result.Destination.File)
	assert.Equal(t, 1, result.Destination.Slide)
	assert.Equal(t, result.ShapeID, result.Destination.ShapeID)
	assert.True(t, containsString(result.Destination.Selectors, "shape:"+strconv.Itoa(result.ShapeID)))
	assert.Contains(t, result.Destination.TextPreview, "Hello Rich Box")
	require.NotNil(t, result.Destination.Bounds)
	assert.Equal(t, int64(100000), result.Destination.Bounds.X)
	assert.Equal(t, int64(100000), result.Destination.Bounds.Y)
	assert.Equal(t, int64(3000000), result.Destination.Bounds.CX)
	assert.Equal(t, int64(800000), result.Destination.Bounds.CY)

	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	var shapes PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(readback), &shapes))
	require.Len(t, shapes.Shapes, 1)
	assert.Equal(t, result.ShapeID, shapes.Shapes[0].ShapeID)
	assert.Contains(t, shapes.Shapes[0].TextPreview, "Hello Rich Box")
	require.NotNil(t, shapes.Shapes[0].Bounds)
	assert.Equal(t, int64(3000000), shapes.Shapes[0].Bounds.CX)

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	text, err := extract.ExtractText(&extract.ExtractTextRequest{Session: pkg, Graph: graph, SlideNumbers: []int{1}})
	require.NoError(t, err)
	require.Len(t, text.Slides, 1)

	found := false
	for _, shape := range text.Slides[0].Shapes {
		if shape.Text != nil && shape.Text.PlainText == "Hello Rich Box" {
			found = true
			break
		}
	}
	assert.True(t, found, "expected inserted text box text to be extracted")
}

func TestAddTextbox_DryRunJSONIncludesDestinationReadback(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	reportPath := filepath.Join(t.TempDir(), "add-textbox-dry-run.json")

	cmd := newTestRootCmd(t)
	var stdout bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetArgs([]string{
		"--format", "json",
		"-o", reportPath,
		"pptx", "add-textbox", fixturePath,
		"--slide", "1",
		"--x", "120000",
		"--y", "130000",
		"--cx", "2100000",
		"--cy", "510000",
		"--text", "Dry Run Box",
		"--dry-run",
	})
	require.NoError(t, cmd.Execute())
	assert.Empty(t, stdout.String())

	data, err := os.ReadFile(reportPath)
	require.NoError(t, err)
	var result AddTextboxCLIResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	assert.NotZero(t, result.ShapeID)
	require.NotNil(t, result.Destination)
	assert.Empty(t, result.Destination.File)
	assert.Equal(t, 1, result.Destination.Slide)
	assert.Equal(t, result.ShapeID, result.Destination.ShapeID)
	assert.Equal(t, "shape:"+strconv.Itoa(result.ShapeID), result.Destination.PrimarySelector)
	assert.Contains(t, result.Destination.TextPreview, "Dry Run Box")
	require.NotNil(t, result.Destination.Bounds)
	assert.Equal(t, int64(120000), result.Destination.Bounds.X)
	assert.Equal(t, int64(130000), result.Destination.Bounds.Y)
	assert.Equal(t, int64(2100000), result.Destination.Bounds.CX)
	assert.Equal(t, int64(510000), result.Destination.Bounds.CY)
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx shapes get")

	slide1 := readSlideXML(t, fixturePath, 1)
	assert.NotContains(t, slide1, "Dry Run Box")
}

func TestAddTextbox_RequiresMutationTarget(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "add-textbox", fixturePath,
		"--slide", "1",
		"--x", "100000",
		"--y", "100000",
		"--cx", "3000000",
		"--cy", "800000",
		"--text", "Hello Rich Box",
	})
	err = cmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "must specify exactly one of --out, --in-place, or --dry-run")
}

func TestAddTextboxPostWriteValidationFailurePreservesDiagnostics(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/corrupted-missing-media/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "bad-textbox.pptx")

	cmd := newTestRootCmd(t)
	cmd.SilenceUsage = true
	cmd.SilenceErrors = true
	cmd.SetArgs([]string{
		"--format", "json",
		"pptx", "add-textbox", fixturePath,
		"--slide", "1",
		"--x", "100000",
		"--y", "100000",
		"--cx", "3000000",
		"--cy", "800000",
		"--text", "Triggers post-write validation",
		"--out", outPath,
	})

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetErr(&stderr)

	err = cmd.Execute()
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok, "expected CLIError, got %T", err)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)
	assert.Equal(t, "validation_failed", cliErr.Code)
	assert.True(t, diagnosticJSONContains(cliErr.Diagnostics, "REL_DANGLING_TARGET"), "diagnostics = %#v", cliErr.Diagnostics)
	assert.True(t, diagnosticJSONContains(cliErr.Diagnostics, "PPTX_MISSING_MEDIA"), "diagnostics = %#v", cliErr.Diagnostics)
	assert.Empty(t, stdout.String())
	assert.Empty(t, stderr.String())
	assert.NoFileExists(t, outPath)
}
