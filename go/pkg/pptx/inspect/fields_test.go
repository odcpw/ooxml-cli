package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func openFieldsFixture(t *testing.T, name string) opc.PackageSession {
	t.Helper()
	pkg, err := opc.Open("../../../testdata/pptx/" + name + "/presentation.pptx")
	require.NoError(t, err)
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

func TestReadFields_HeaderFooterFixture(t *testing.T) {
	pkg := openFieldsFixture(t, "header-footer")
	report, err := ReadFields(pkg)
	require.NoError(t, err)

	require.Len(t, report.Masters, 1)
	m := report.Masters[0]
	assert.True(t, m.HasHeaderFooter)
	assert.True(t, m.ShowSlideNumber)
	assert.True(t, m.ShowFooter)
	assert.True(t, m.ShowDate)

	require.GreaterOrEqual(t, len(report.Slides), 1)
	s := report.Slides[0]
	require.NotNil(t, s.FooterPlaceholder, "slide 1 should carry a footer placeholder")
	require.NotNil(t, s.DatePlaceholder, "slide 1 should carry a date placeholder")
	require.NotNil(t, s.SlideNumberPlaceholder, "slide 1 should carry a slide-number placeholder")
	assert.Equal(t, "datetimeFigureOut", s.DatePlaceholder.FieldType)
	assert.Equal(t, "slidenum", s.SlideNumberPlaceholder.FieldType)
}

func TestReadFields_NoHeaderFooterDefaultsTrue(t *testing.T) {
	// title-content has no p:hf on its master; defaults must report visible.
	pkg := openFieldsFixture(t, "title-content")
	report, err := ReadFields(pkg)
	require.NoError(t, err)

	require.Len(t, report.Masters, 1)
	m := report.Masters[0]
	assert.False(t, m.HasHeaderFooter)
	assert.True(t, m.ShowSlideNumber)
	assert.True(t, m.ShowFooter)
	assert.True(t, m.ShowDate)
}

func TestReadFields_SlideWithoutPlaceholders(t *testing.T) {
	pkg := openFieldsFixture(t, "title-content")
	report, err := ReadFields(pkg)
	require.NoError(t, err)
	require.GreaterOrEqual(t, len(report.Slides), 1)
	s := report.Slides[0]
	assert.Nil(t, s.FooterPlaceholder)
	assert.Nil(t, s.DatePlaceholder)
	assert.Nil(t, s.SlideNumberPlaceholder)
}
