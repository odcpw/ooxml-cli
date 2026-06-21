package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPPTXLayoutsCloneJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-clone.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "clone", fixturePath,
		"--layout", "2",
		"--name", "ImageGridClone",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result cloneLayoutOutput
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, "ImageGridClone", result.NewLayout)
	assert.NotEmpty(t, result.NewURI)
	assert.Contains(t, result.ReadbackCommand, "pptx layouts show")
	assert.Contains(t, result.LayoutsListCommand, "pptx layouts list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForLayoutsTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForLayoutsTest(t, result.ReadbackCommand)
	var show LayoutShowOutput
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	assert.Equal(t, result.NewLayout, show.Name)
	assert.Equal(t, result.NewURI, show.PartURI)

	listOutput := executeGeneratedOOXMLCommandForLayoutsTest(t, result.LayoutsListCommand)
	var list LayoutListOutput
	require.NoError(t, json.Unmarshal([]byte(listOutput), &list))
	assert.True(t, layoutListContainsNameForTest(list, result.NewLayout))
}

func TestPPTXLayoutsRenameJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-rename.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "rename", fixturePath,
		"--layout", "2",
		"--name", "ImageGridRenamed",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result renameLayoutOutput
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, "ImageGridRenamed", result.NewName)
	assert.NotEmpty(t, result.LayoutURI)
	assert.Contains(t, result.ReadbackCommand, "pptx layouts show")
	assert.Contains(t, result.LayoutsListCommand, "pptx layouts list")

	executeGeneratedOOXMLCommandForLayoutsTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForLayoutsTest(t, result.ReadbackCommand)
	var show LayoutShowOutput
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	assert.Equal(t, result.NewName, show.Name)
	assert.Equal(t, result.LayoutURI, show.PartURI)
}

func TestPPTXLayoutsAddPlaceholderJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	workPath := filepath.Join(tmpDir, "layout-source.pptx")
	outPath := filepath.Join(tmpDir, "layout-add-placeholder.pptx")

	_, err = executeRootForXLSXTest(t,
		"pptx", "layouts", "clone", fixturePath,
		"--layout", "2",
		"--name", "ImageGridPlaceholder",
		"--out", workPath,
	)
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "add-placeholder", workPath,
		"--layout", "ImageGridPlaceholder",
		"--type", "pic",
		"--idx", "0",
		"--bounds", "1000,2000,3000,4000",
		"--out", outPath,
	)
	require.NoError(t, err)

	var result addPlaceholderResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, workPath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, "ImageGridPlaceholder", result.Layout)
	assert.Equal(t, "pic", result.Type)
	assert.Equal(t, 0, result.Idx)
	assert.Contains(t, result.ReadbackCommand, "pptx layouts show")
	assert.Contains(t, result.LayoutsListCommand, "pptx layouts list")

	executeGeneratedOOXMLCommandForLayoutsTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForLayoutsTest(t, result.ReadbackCommand)
	var show LayoutShowOutput
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	assert.True(t, layoutShowContainsPlaceholderForTest(show, "pic:0"))
}

func TestPPTXLayoutsAddPlaceholderMissingLayoutListsDiscovery(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	_, err = executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "add-placeholder", fixturePath,
		"--layout", "No Such Layout",
		"--type", "pic",
		"--bounds", "1000,2000,3000,4000",
		"--dry-run",
	)
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok, "error should be CLIError: %v", err)
	assert.Equal(t, ExitTargetNotFound, cliErr.ExitCode)
	assert.Contains(t, err.Error(), "layout not found: No Such Layout")
	assert.Contains(t, err.Error(), "did you mean:")
	assert.Contains(t, err.Error(), "ooxml --json pptx layouts list <file>")
}

func TestPPTXLayoutsSetBoundsAndDeleteShapeJSONReadbackCommands(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	workPath := filepath.Join(tmpDir, "layout-source.pptx")
	boundsPath := filepath.Join(tmpDir, "layout-bounds.pptx")
	deletePath := filepath.Join(tmpDir, "layout-delete.pptx")

	_, err = executeRootForXLSXTest(t,
		"pptx", "layouts", "clone", fixturePath,
		"--layout", "2",
		"--name", "ImageGridEdit",
		"--out", workPath,
	)
	require.NoError(t, err)

	boundsOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "set-bounds", workPath,
		"--layout", "ImageGridEdit",
		"--target", "shape:3",
		"--bounds", "111111,222222,333333,444444",
		"--out", boundsPath,
	)
	require.NoError(t, err)

	var boundsResult setBoundsLayoutOutput
	require.NoError(t, json.Unmarshal([]byte(boundsOutput), &boundsResult))
	assert.Equal(t, workPath, boundsResult.File)
	assert.Equal(t, boundsPath, boundsResult.Output)
	assert.False(t, boundsResult.DryRun)
	assert.Equal(t, "ImageGridEdit", boundsResult.Layout)
	assert.Contains(t, boundsResult.ReadbackCommand, "pptx layouts show")
	executeGeneratedOOXMLCommandForLayoutsTest(t, boundsResult.ValidateCommand)
	boundsReadback := executeGeneratedOOXMLCommandForLayoutsTest(t, boundsResult.ReadbackCommand)
	var boundsShow LayoutShowOutput
	require.NoError(t, json.Unmarshal([]byte(boundsReadback), &boundsShow))
	assert.True(t, layoutShowContainsPlaceholderForTest(boundsShow, "shape:3"))

	deleteOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "delete-shape", boundsPath,
		"--layout", "ImageGridEdit",
		"--target", "shape:3",
		"--out", deletePath,
	)
	require.NoError(t, err)

	var deleteResult deleteLayoutShapeOutput
	require.NoError(t, json.Unmarshal([]byte(deleteOutput), &deleteResult))
	assert.Equal(t, boundsPath, deleteResult.File)
	assert.Equal(t, deletePath, deleteResult.Output)
	assert.False(t, deleteResult.DryRun)
	assert.Equal(t, "ImageGridEdit", deleteResult.Layout)
	assert.Contains(t, deleteResult.ReadbackCommand, "pptx layouts show")
	executeGeneratedOOXMLCommandForLayoutsTest(t, deleteResult.ValidateCommand)
	deleteReadback := executeGeneratedOOXMLCommandForLayoutsTest(t, deleteResult.ReadbackCommand)
	var deleteShow LayoutShowOutput
	require.NoError(t, json.Unmarshal([]byte(deleteReadback), &deleteShow))
	assert.False(t, layoutShowContainsPlaceholderForTest(deleteShow, "shape:3"))
}

func TestPPTXLayoutsMutationDryRunTemplates(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "layouts", "rename", fixturePath,
		"--layout", "2",
		"--name", "DryRunLayout",
		"--dry-run",
	)
	require.NoError(t, err)

	var result renameLayoutOutput
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	assert.Empty(t, result.ReadbackCommand)
	assert.Empty(t, result.LayoutsListCommand)
	assert.Empty(t, result.ValidateCommand)
	assert.Empty(t, result.RenderCommand)
	assert.Contains(t, result.ReadbackCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.LayoutsListCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.ValidateCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.RenderCommandTemplate, "<out.pptx>")
}

func executeGeneratedOOXMLCommandForLayoutsTest(t *testing.T, command string) string {
	t.Helper()
	if !strings.HasPrefix(command, "ooxml ") {
		t.Fatalf("generated command must start with ooxml: %s", command)
	}
	args, err := splitGeneratedOOXMLCommandForLayoutsTest(command)
	require.NoError(t, err)
	output, err := executeRootForXLSXTest(t, args[1:]...)
	if err != nil {
		t.Fatalf("generated command failed: %v\ncommand=%s\noutput=%s", err, command, output)
	}
	return output
}

func splitGeneratedOOXMLCommandForLayoutsTest(command string) ([]string, error) {
	args := []string{}
	var current strings.Builder
	inSingle := false
	for i := 0; i < len(command); i++ {
		ch := command[i]
		switch {
		case inSingle && ch == '\'':
			inSingle = false
		case !inSingle && ch == '\'':
			inSingle = true
		case !inSingle && (ch == ' ' || ch == '\t' || ch == '\n'):
			if current.Len() > 0 {
				args = append(args, current.String())
				current.Reset()
			}
		default:
			current.WriteByte(ch)
		}
	}
	if inSingle {
		return nil, assert.AnError
	}
	if current.Len() > 0 {
		args = append(args, current.String())
	}
	return args, nil
}

func layoutListContainsNameForTest(result LayoutListOutput, name string) bool {
	for _, layout := range result.Layouts {
		if layout.Name == name {
			return true
		}
	}
	return false
}

func layoutShowContainsPlaceholderForTest(result LayoutShowOutput, key string) bool {
	for _, placeholder := range result.Placeholders {
		if placeholder.Key == key {
			return true
		}
	}
	return false
}
