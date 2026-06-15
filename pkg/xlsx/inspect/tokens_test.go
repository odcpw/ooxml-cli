package inspect

import (
	"os"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func openXLSXFixture(t *testing.T, path string) opc.PackageSession {
	t.Helper()
	if _, err := os.Stat(path); err != nil {
		t.Skipf("fixture not found: %s", path)
	}
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	t.Cleanup(func() { _ = pkg.Close() })
	return pkg
}

func TestExtractXLSXTemplateTokens_Structure(t *testing.T) {
	pkg := openXLSXFixture(t, "../../../testdata/xlsx/minimal-workbook/workbook.xlsx")

	tokens, err := ExtractXLSXTemplateTokens(pkg, "workbook.xlsx")
	require.NoError(t, err)
	require.NotNil(t, tokens)

	assert.Equal(t, tmpl.SchemaVersion, tokens.SchemaVersion)
	assert.Equal(t, tmpl.KindXLSX, tokens.Type)
	assert.Equal(t, "workbook.xlsx", tokens.Source)
	require.NotNil(t, tokens.XLSX)

	// Lists are always non-nil even when empty (never null in JSON).
	assert.NotNil(t, tokens.XLSX.NamedCellStyles)
	assert.NotNil(t, tokens.XLSX.ChartStyles)
}

func TestExtractXLSXTemplateTokens_Deterministic(t *testing.T) {
	pkg := openXLSXFixture(t, "../../../testdata/xlsx/types-and-formulas/workbook.xlsx")

	first, err := ExtractXLSXTemplateTokens(pkg, "workbook.xlsx")
	require.NoError(t, err)
	second, err := ExtractXLSXTemplateTokens(pkg, "workbook.xlsx")
	require.NoError(t, err)
	assert.Equal(t, first, second, "extraction must be deterministic")
}

// TestExtractNamedCellStyles_Synthetic verifies the named-style resolver against
// a hand-built styles.xml, since the repo fixtures carry no named cell styles.
func TestExtractNamedCellStyles_Synthetic(t *testing.T) {
	stylesXML := `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="1"><numFmt numFmtId="164" formatCode="0.00%"/></numFmts>
  <fonts count="2">
    <font><sz val="11"/><name val="Calibri"/></font>
    <font><b/><sz val="14"/><color rgb="FFFFFFFF"/><name val="Arial"/></font>
  </fonts>
  <fills count="3">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="FF4472C4"/></patternFill></fill>
  </fills>
  <cellStyleXfs count="2">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
    <xf numFmtId="164" fontId="1" fillId="2" borderId="0"/>
  </cellStyleXfs>
  <cellStyles count="2">
    <cellStyle name="Normal" xfId="0" builtinId="0"/>
    <cellStyle name="Heading 1" xfId="1"/>
  </cellStyles>
</styleSheet>`

	doc := etree.NewDocument()
	require.NoError(t, doc.ReadFromString(stylesXML))
	styles := namedCellStylesFromDoc(doc)
	require.Len(t, styles, 2)

	assert.Equal(t, "Normal", styles[0].Name)
	assert.True(t, styles[0].Builtin)
	assert.Equal(t, "Calibri", styles[0].FontName)
	assert.Equal(t, 11.0, styles[0].SizePt)

	h1 := styles[1]
	assert.Equal(t, "Heading 1", h1.Name)
	assert.False(t, h1.Builtin)
	assert.Equal(t, "Arial", h1.FontName)
	assert.Equal(t, 14.0, h1.SizePt)
	assert.True(t, h1.Bold)
	assert.Equal(t, "FFFFFF", h1.Color)
	assert.Equal(t, "4472C4", h1.FillColor)
	assert.Equal(t, "0.00%", h1.NumberFormatCode)
}
