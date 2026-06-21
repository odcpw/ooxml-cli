package mutate

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestSetFields_Nil(t *testing.T) { _, err := SetFields(nil); require.Error(t, err) }

func TestSetFields_FooterTextOnSlidePlaceholder(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	res, err := SetFields(&SetFieldsRequest{Package: pkg, FooterText: strPtr("Confidential")})
	require.NoError(t, err)
	assert.Equal(t, 1, res.FooterPlaceholdersUpdated)
	requirePackageValid(t, pkg)

	report, err := inspect.ReadFields(pkg)
	require.NoError(t, err)
	require.NotNil(t, report.Slides[0].FooterPlaceholder)
	assert.Equal(t, "Confidential", report.Slides[0].FooterPlaceholder.Text)
}

func TestSetFields_FooterTextIdempotent(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	_, err := SetFields(&SetFieldsRequest{Package: pkg, FooterText: strPtr("Once")})
	require.NoError(t, err)
	res, err := SetFields(&SetFieldsRequest{Package: pkg, FooterText: strPtr("Once")})
	require.NoError(t, err)
	assert.Equal(t, 0, res.FooterPlaceholdersUpdated, "re-setting identical footer text should be a no-op")
}

func TestSetFields_VisibilityCreatesHeaderFooter(t *testing.T) {
	// title-content's master has no p:hf; the toggle must create it.
	pkg := openMutatePackage(t, "../../../testdata/pptx/title-content/presentation.pptx")
	defer pkg.Close()

	res, err := SetFields(&SetFieldsRequest{Package: pkg, ShowDate: boolPtr(false)})
	require.NoError(t, err)
	assert.True(t, res.CreatedHeaderFooter)
	require.Len(t, res.MastersUpdated, 1)
	requirePackageValid(t, pkg)

	report, err := inspect.ReadFields(pkg)
	require.NoError(t, err)
	require.Len(t, report.Masters, 1)
	assert.True(t, report.Masters[0].HasHeaderFooter)
	assert.False(t, report.Masters[0].ShowDate)
	assert.True(t, report.Masters[0].ShowFooter, "untouched toggles stay visible")
}

func TestSetFields_VisibilityUpdatesExistingHeaderFooter(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	res, err := SetFields(&SetFieldsRequest{
		Package:         pkg,
		ShowSlideNumber: boolPtr(false),
		ShowFooter:      boolPtr(false),
	})
	require.NoError(t, err)
	assert.False(t, res.CreatedHeaderFooter, "existing p:hf should be reused")
	require.Len(t, res.MastersUpdated, 1)
	requirePackageValid(t, pkg)

	report, err := inspect.ReadFields(pkg)
	require.NoError(t, err)
	assert.False(t, report.Masters[0].ShowSlideNumber)
	assert.False(t, report.Masters[0].ShowFooter)
	assert.True(t, report.Masters[0].ShowDate)
}

func TestSetFields_DateFormat(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	res, err := SetFields(&SetFieldsRequest{Package: pkg, DateFormat: "date-only"})
	require.NoError(t, err)
	assert.Equal(t, 1, res.DatePlaceholdersUpdated)
	requirePackageValid(t, pkg)

	report, err := inspect.ReadFields(pkg)
	require.NoError(t, err)
	require.NotNil(t, report.Slides[0].DatePlaceholder)
	assert.Equal(t, "datetime1", report.Slides[0].DatePlaceholder.FieldType)
}

func TestSetFields_DateFormatIdempotent(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	_, err := SetFields(&SetFieldsRequest{Package: pkg, DateFormat: "date-only"})
	require.NoError(t, err)
	res, err := SetFields(&SetFieldsRequest{Package: pkg, DateFormat: "date-only"})
	require.NoError(t, err)
	assert.Equal(t, 0, res.DatePlaceholdersUpdated, "re-setting an identical date field type should be a no-op")
	// An a:fld is present and already the requested type: this is a legitimate
	// idempotent no-op, NOT the fieldless case, and must not be flagged. This is
	// what pins the signal to real a:fld presence rather than hadDate && !updated.
	assert.Empty(t, res.SlidesWithDatePlaceholderButNoField,
		"an existing-but-unchanged date field must not be reported as fieldless")
}

func TestSetFields_DatePlaceholderWithoutFieldReported(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	// The fixture's date placeholder holds an a:fld. Rewrite slide 1 so its date
	// placeholder carries only plain run text (no a:fld), the case where
	// --date-format cannot retype anything and must not report a silent success.
	stripDateFieldFromSlide1(t, pkg)

	res, err := SetFields(&SetFieldsRequest{Package: pkg, DateFormat: "date-only"})
	require.NoError(t, err)
	assert.Equal(t, 0, res.DatePlaceholdersUpdated, "no a:fld means nothing to retype")
	assert.Equal(t, []int{1}, res.SlidesWithDatePlaceholderButNoField,
		"present-but-fieldless date placeholder must be observable, not a silent no-op")
	// header-footer has 2 slides; only slide 1 carries the date placeholder, so
	// slide 2 is the genuinely-absent case and must not be conflated with slide 1.
	assert.Equal(t, []int{2}, res.SlidesWithoutDatePlaceholder)
	requirePackageValid(t, pkg)

	// Readback confirms the placeholder is still present but carries no field type.
	report, err := inspect.ReadFields(pkg)
	require.NoError(t, err)
	require.NotNil(t, report.Slides[0].DatePlaceholder)
	assert.Empty(t, report.Slides[0].DatePlaceholder.FieldType)
}

// stripDateFieldFromSlide1 replaces the a:fld of slide 1's date placeholder with a
// plain a:r/a:t run, leaving every other element untouched, and writes it back into
// the session so subsequent reads observe the fieldless placeholder.
func stripDateFieldFromSlide1(t *testing.T, pkg opc.PackageSession) {
	t.Helper()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.NotEmpty(t, graph.Slides)
	slideURI := graph.Slides[0].PartURI

	doc, err := pkg.ReadXMLPart(slideURI)
	require.NoError(t, err)
	fld := doc.Root().FindElement("//a:fld")
	require.NotNil(t, fld, "fixture slide 1 date placeholder should start with an a:fld")
	p := fld.Parent()
	cachedText := ""
	if tEl := fld.FindElement("a:t"); tEl != nil {
		cachedText = tEl.Text()
	}

	r := etree.NewElement("a:r")
	tEl := etree.NewElement("a:t")
	tEl.SetText(cachedText)
	r.AddChild(tEl)
	p.InsertChildAt(fld.Index(), r)
	p.RemoveChild(fld)

	require.NoError(t, pkg.ReplaceXMLPart(slideURI, doc))
}

func TestSetFields_InvalidDateFormat(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()
	_, err := SetFields(&SetFieldsRequest{Package: pkg, DateFormat: "bogus"})
	require.Error(t, err)
}

func TestSetFields_NoChanges(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()
	_, err := SetFields(&SetFieldsRequest{Package: pkg})
	require.Error(t, err)
}

func TestSetFields_SlidesWithoutPlaceholderReported(t *testing.T) {
	pkg := openMutatePackage(t, "../../../testdata/pptx/header-footer/presentation.pptx")
	defer pkg.Close()

	res, err := SetFields(&SetFieldsRequest{Package: pkg, FooterText: strPtr("X")})
	require.NoError(t, err)
	// header-footer has 2 slides; only slide 1 carries the footer placeholder.
	assert.Equal(t, []int{2}, res.SlidesWithoutFooterPlaceholder)
}
