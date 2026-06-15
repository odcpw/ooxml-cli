package chart

import (
	"os"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func openFixture(t *testing.T, path string) opc.PackageSession {
	t.Helper()
	if _, err := os.Stat(path); err != nil {
		t.Skipf("fixture not found: %s", path)
	}
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	t.Cleanup(func() { _ = pkg.Close() })
	return pkg
}

func TestSummarizeChartStyles_PPTX(t *testing.T) {
	pkg := openFixture(t, "../../../testdata/pptx/chart-simple/presentation.pptx")

	summaries := SummarizeChartStyles(pkg, "/ppt/charts/chart")
	require.NotEmpty(t, summaries, "chart-simple fixture has chart parts")

	prev := ""
	for _, cs := range summaries {
		assert.NotEmpty(t, cs.PartURI)
		assert.NotEmpty(t, cs.ChartType)
		assert.True(t, prev <= cs.PartURI, "summaries must be in sorted partUri order")
		prev = cs.PartURI
	}
}

func TestSummarizeChartStyles_XLSX(t *testing.T) {
	pkg := openFixture(t, "../../../testdata/xlsx/chart-workbook/workbook.xlsx")

	summaries := SummarizeChartStyles(pkg, "/xl/charts/chart")
	require.NotEmpty(t, summaries, "chart-workbook fixture has a chart part")
	for _, cs := range summaries {
		assert.NotEmpty(t, cs.PartURI)
	}
}

func TestSummarizeChartStyles_NilSession(t *testing.T) {
	assert.Equal(t, 0, len(SummarizeChartStyles(nil, "/xl/charts/chart")))
}

func TestSummarizeChartStyles_NoCharts(t *testing.T) {
	pkg := openFixture(t, "../../../testdata/xlsx/minimal-workbook/workbook.xlsx")
	summaries := SummarizeChartStyles(pkg, "/xl/charts/chart")
	assert.NotNil(t, summaries)
	assert.Empty(t, summaries)
}
