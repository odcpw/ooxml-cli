package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/capabilities"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func runTemplateTokens(t *testing.T, args ...string) (string, error) {
	t.Helper()
	templateTokensFor = "auto"
	return executeRootForXLSXTest(t, args...)
}

func TestTemplateTokensCmd_PPTX_ChartsWired(t *testing.T) {
	fixture := "../../testdata/pptx/chart-simple/presentation.pptx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	out, err := runTemplateTokens(t, "--json", "template", "tokens", fixture)
	require.NoError(t, err)

	var tokens tmpl.TemplateTokens
	require.NoError(t, json.Unmarshal([]byte(out), &tokens))
	require.NotNil(t, tokens.PPTX)
	require.NotEmpty(t, tokens.PPTX.ChartStyles, "chart styles must be wired into the command output")
}

func TestTemplateTokensCmd_XLSX_ChartsWired(t *testing.T) {
	fixture := "../../testdata/xlsx/chart-workbook/workbook.xlsx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	out, err := runTemplateTokens(t, "--json", "template", "tokens", fixture)
	require.NoError(t, err)

	var tokens tmpl.TemplateTokens
	require.NoError(t, json.Unmarshal([]byte(out), &tokens))
	require.NotNil(t, tokens.XLSX)
	require.NotEmpty(t, tokens.XLSX.ChartStyles, "chart styles must be wired into the command output")
}

func TestTemplateTokensCmd_PPTX_JSON(t *testing.T) {
	fixture := "../../testdata/pptx/theme-custom-colors/presentation.pptx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	out, err := runTemplateTokens(t, "--json", "template", "tokens", fixture)
	require.NoError(t, err)

	var tokens tmpl.TemplateTokens
	require.NoError(t, json.Unmarshal([]byte(out), &tokens))
	assert.Equal(t, tmpl.SchemaVersion, tokens.SchemaVersion)
	assert.Equal(t, tmpl.KindPPTX, tokens.Type)
	require.NotNil(t, tokens.PPTX)
	require.NotNil(t, tokens.PPTX.Theme)
	require.NotNil(t, tokens.PPTX.Theme.ColorScheme)
	assert.Equal(t, "4F81BD", tokens.PPTX.Theme.ColorScheme.Accent1)
	// JSON lists must serialize as arrays, never null.
	assert.NotNil(t, tokens.PPTX.DefaultTextStyles)
	assert.NotNil(t, tokens.PPTX.TableStyles)
	assert.NotNil(t, tokens.PPTX.ChartStyles)
}

func TestTemplateTokensCmd_XLSX_JSON(t *testing.T) {
	fixture := "../../testdata/xlsx/chart-workbook/workbook.xlsx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	out, err := runTemplateTokens(t, "--json", "template", "tokens", fixture)
	require.NoError(t, err)

	var tokens tmpl.TemplateTokens
	require.NoError(t, json.Unmarshal([]byte(out), &tokens))
	assert.Equal(t, tmpl.KindXLSX, tokens.Type)
	require.NotNil(t, tokens.XLSX)
	assert.NotNil(t, tokens.XLSX.NamedCellStyles)
	assert.NotNil(t, tokens.XLSX.ChartStyles)
}

func TestTemplateTokensCmd_TextFormat(t *testing.T) {
	fixture := "../../testdata/pptx/theme-custom-colors/presentation.pptx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	out, err := runTemplateTokens(t, "template", "tokens", fixture)
	require.NoError(t, err)
	assert.Contains(t, out, "Template Tokens (schema 1.0)")
	assert.Contains(t, out, "Type:   pptx")
	assert.Contains(t, out, "Theme:")
}

func TestTemplateTokensCmd_OutputFile(t *testing.T) {
	fixture := "../../testdata/pptx/theme-custom-colors/presentation.pptx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}
	outPath := filepath.Join(t.TempDir(), "tokens.json")

	_, err := runTemplateTokens(t, "--json", "template", "tokens", fixture, "--output", outPath)
	require.NoError(t, err)

	data, err := os.ReadFile(outPath)
	require.NoError(t, err)
	var tokens tmpl.TemplateTokens
	require.NoError(t, json.Unmarshal(data, &tokens))
	assert.Equal(t, tmpl.KindPPTX, tokens.Type)
}

func TestTemplateTokensCmd_ForOverride(t *testing.T) {
	fixture := "../../testdata/xlsx/chart-workbook/workbook.xlsx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	out, err := runTemplateTokens(t, "--json", "template", "tokens", fixture, "--for", "xlsx")
	require.NoError(t, err)
	var tokens tmpl.TemplateTokens
	require.NoError(t, json.Unmarshal([]byte(out), &tokens))
	assert.Equal(t, tmpl.KindXLSX, tokens.Type)
}

func TestTemplateTokensCmd_UnsupportedType(t *testing.T) {
	matches, _ := filepath.Glob("../../testdata/docx/*/document.docx")
	if len(matches) == 0 {
		t.Skip("no docx fixture available")
	}

	_, err := runTemplateTokens(t, "template", "tokens", matches[0])
	require.Error(t, err)
	assert.Contains(t, strings.ToLower(err.Error()), "pptx/potx and xlsx/xltx")
}

func TestTemplateTokensCmd_InvalidFor(t *testing.T) {
	fixture := "../../testdata/pptx/theme-custom-colors/presentation.pptx"
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("fixture not found: %s", fixture)
	}

	_, err := runTemplateTokens(t, "template", "tokens", fixture, "--for", "docx")
	require.Error(t, err)
	assert.Contains(t, strings.ToLower(err.Error()), "--for must be one of")
}

func TestTemplateTokensCmd_CapabilitiesRegistered(t *testing.T) {
	meta, ok := capabilities.MetadataFor("ooxml template tokens")
	require.True(t, ok, "template tokens must have capabilities metadata")
	assert.NotEmpty(t, meta.Examples)
	assert.Contains(t, meta.TargetObjectKinds, "package")
	for _, kind := range meta.TargetObjectKinds {
		assert.True(t, capabilities.IsObjectKind(kind), "target kind %q must be in the closed vocabulary", kind)
	}
}
