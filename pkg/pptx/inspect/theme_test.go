package inspect

import (
	"os"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
)

func TestParseTheme(t *testing.T) {
	// Use the rich-alignment fixture which has a theme
	filePath := "../../../testdata/pptx/rich-alignment/presentation.pptx"

	// Check if file exists
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("Test fixture not found: %s", filePath)
	}

	pkg, err := opc.Open(filePath)
	assert.NoError(t, err)
	defer pkg.Close()

	// Parse the theme
	theme, err := ParseTheme(pkg, "/ppt/theme/theme1.xml")
	assert.NoError(t, err)
	assert.NotNil(t, theme)

	// Verify theme name
	assert.NotEmpty(t, theme.Name)

	// Verify color scheme was parsed
	assert.NotNil(t, theme.ColorScheme)
	assert.NotEmpty(t, theme.ColorScheme.Name)

	// Verify accent colors are present
	assert.NotEmpty(t, theme.ColorScheme.Accent1)
	assert.NotEmpty(t, theme.ColorScheme.Accent2)

	// Verify font scheme was parsed
	assert.NotNil(t, theme.FontScheme)
	assert.NotEmpty(t, theme.FontScheme.Name)

	// Verify fonts are present
	assert.NotEmpty(t, theme.FontScheme.MajorFont)
	assert.NotEmpty(t, theme.FontScheme.MinorFont)

	// Expected values from theme1.xml in the fixture
	assert.Equal(t, "Office Theme", theme.Name)
	assert.Equal(t, "Office", theme.ColorScheme.Name)
	assert.Equal(t, "Calibri", theme.FontScheme.MajorFont)
	assert.Equal(t, "Calibri", theme.FontScheme.MinorFont)
	assert.Equal(t, "4F81BD", theme.ColorScheme.Accent1)
	assert.Equal(t, "C0504D", theme.ColorScheme.Accent2)
}

func TestParseThemeWithEmptyURI(t *testing.T) {
	filePath := "../../../testdata/pptx/rich-alignment/presentation.pptx"

	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("Test fixture not found: %s", filePath)
	}

	pkg, err := opc.Open(filePath)
	assert.NoError(t, err)
	defer pkg.Close()

	// Parse with empty theme URI
	theme, err := ParseTheme(pkg, "")
	assert.NoError(t, err)
	assert.Nil(t, theme)
}

func TestExtractDefaultTextStyleInfo(t *testing.T) {
	filePath := "../../../testdata/pptx/rich-alignment/presentation.pptx"

	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("Test fixture not found: %s", filePath)
	}

	pkg, err := opc.Open(filePath)
	assert.NoError(t, err)
	defer pkg.Close()

	// Extract default text style info
	info := ExtractDefaultTextStyleInfo(pkg, "/ppt/theme/theme1.xml")
	assert.NotNil(t, info)

	// Verify theme name
	assert.Equal(t, "Office Theme", info.ThemeName)

	// Verify fonts
	assert.Equal(t, "Calibri", info.MajorFont)
	assert.Equal(t, "Calibri", info.MinorFont)

	// Verify accent colors are present
	assert.NotEmpty(t, info.AccentColors)
	assert.Greater(t, len(info.AccentColors), 0)
	assert.Contains(t, info.AccentColors, "4F81BD") // Accent1
	assert.Contains(t, info.AccentColors, "C0504D") // Accent2
}

func TestExtractDefaultTextStyleInfoWithEmptyURI(t *testing.T) {
	filePath := "../../../testdata/pptx/rich-alignment/presentation.pptx"

	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("Test fixture not found: %s", filePath)
	}

	pkg, err := opc.Open(filePath)
	assert.NoError(t, err)
	defer pkg.Close()

	// Extract with empty theme URI
	info := ExtractDefaultTextStyleInfo(pkg, "")
	assert.Nil(t, info)
}

func TestParseThemeWithEACSFonts(t *testing.T) {
	// Test that EA/CS fonts are parsed (even if empty in this fixture)
	filePath := "../../../testdata/pptx/rich-alignment/presentation.pptx"

	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("Test fixture not found: %s", filePath)
	}

	pkg, err := opc.Open(filePath)
	assert.NoError(t, err)
	defer pkg.Close()

	// Parse the theme
	theme, err := ParseTheme(pkg, "/ppt/theme/theme1.xml")
	assert.NoError(t, err)
	assert.NotNil(t, theme)

	// Verify font scheme was parsed with EA/CS fields
	assert.NotNil(t, theme.FontScheme)

	// FontScheme should have the new EA/CS fields (may be empty in this fixture)
	_ = theme.FontScheme.EastAsianMajorFont
	_ = theme.FontScheme.EastAsianMinorFont
	_ = theme.FontScheme.ComplexScriptMajorFont
	_ = theme.FontScheme.ComplexScriptMinorFont

	// Verify basic fonts are still present
	assert.NotEmpty(t, theme.FontScheme.MajorFont)
	assert.NotEmpty(t, theme.FontScheme.MinorFont)
}
