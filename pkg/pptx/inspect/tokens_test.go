package inspect

import (
	"os"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func openPPTXFixture(t *testing.T, path string) opc.PackageSession {
	t.Helper()
	if _, err := os.Stat(path); err != nil {
		t.Skipf("fixture not found: %s", path)
	}
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	t.Cleanup(func() { _ = pkg.Close() })
	return pkg
}

func TestExtractPPTXTemplateTokens_ThemeAndFonts(t *testing.T) {
	pkg := openPPTXFixture(t, "../../../testdata/pptx/theme-custom-colors/presentation.pptx")

	tokens, err := ExtractPPTXTemplateTokens(pkg, "presentation.pptx")
	require.NoError(t, err)
	require.NotNil(t, tokens)

	assert.Equal(t, tmpl.SchemaVersion, tokens.SchemaVersion)
	assert.Equal(t, tmpl.KindPPTX, tokens.Type)
	assert.Equal(t, "presentation.pptx", tokens.Source)
	require.NotNil(t, tokens.PPTX)
	require.NotNil(t, tokens.PPTX.Theme)
	require.NotNil(t, tokens.PPTX.Theme.ColorScheme)
	assert.Equal(t, "4F81BD", tokens.PPTX.Theme.ColorScheme.Accent1)
	require.NotNil(t, tokens.PPTX.Theme.FontScheme)
	assert.Equal(t, "Calibri", tokens.PPTX.Theme.FontScheme.MajorFont)

	// Lists are always non-nil (never null in JSON).
	assert.NotNil(t, tokens.PPTX.DefaultTextStyles)
	assert.NotNil(t, tokens.PPTX.TableStyles)
	assert.NotNil(t, tokens.PPTX.ChartStyles)
}

func TestExtractPPTXTemplateTokens_DefaultTextStyles(t *testing.T) {
	pkg := openPPTXFixture(t, "../../../testdata/pptx/minimal-title/presentation.pptx")

	tokens, err := ExtractPPTXTemplateTokens(pkg, "presentation.pptx")
	require.NoError(t, err)
	require.NotNil(t, tokens.PPTX)

	roles := map[string]tmpl.DefaultTextStyle{}
	for _, ds := range tokens.PPTX.DefaultTextStyles {
		roles[ds.Role] = ds
		assert.NotEmpty(t, ds.MasterRef, "every default style records its master")
	}
	title, ok := roles["title"]
	require.True(t, ok, "expected a title default text style")
	assert.Greater(t, title.SizePt, 0.0)
	// minimal-title master titleStyle uses +mj-lt and a scheme color.
	assert.Equal(t, "major", title.FontRef)
	assert.NotEmpty(t, title.ColorRef)
}

func TestExtractPPTXTemplateTokens_TableStyles(t *testing.T) {
	pkg := openPPTXFixture(t, "../../../testdata/pptx/table-styled/presentation.pptx")

	tokens, err := ExtractPPTXTemplateTokens(pkg, "presentation.pptx")
	require.NoError(t, err)
	require.NotNil(t, tokens.PPTX)
	require.NotEmpty(t, tokens.PPTX.TableStyles, "table-styled fixture references a table style id")
	for _, ts := range tokens.PPTX.TableStyles {
		assert.NotEmpty(t, ts.StyleID)
	}
}

func TestExtractPPTXTemplateTokens_Deterministic(t *testing.T) {
	pkg := openPPTXFixture(t, "../../../testdata/pptx/table-styled/presentation.pptx")

	first, err := ExtractPPTXTemplateTokens(pkg, "presentation.pptx")
	require.NoError(t, err)
	second, err := ExtractPPTXTemplateTokens(pkg, "presentation.pptx")
	require.NoError(t, err)
	assert.Equal(t, first, second, "extraction must be deterministic")

	// Table styles are emitted in sorted order.
	prev := ""
	for _, ts := range first.PPTX.TableStyles {
		assert.True(t, prev <= ts.StyleID, "table styles must be in sorted order")
		prev = ts.StyleID
	}
}
