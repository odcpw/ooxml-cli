package mutate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// requirePackageValidStrict asserts the package validates with no Error and no
// Warning diagnostics, mirroring the CLI's `validate --strict` semantics where
// warnings are escalated to failures.
func requirePackageValidStrict(t *testing.T, session opc.PackageSession) {
	t.Helper()
	diags, err := validate.ValidatePackage(session)
	require.NoError(t, err)
	for _, diag := range diags {
		require.NotEqual(t, result.Error, diag.Severity, diag.Message)
		require.NotEqual(t, result.Warning, diag.Severity, diag.Message)
	}
}

func TestSetNotesForSlide_CreatesPartWhenAbsent(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "set-notes.pptx")

	pkg := openMutatePackage(t, fixture)
	res, err := SetNotesForSlide(&SetNotesRequest{Package: pkg, SlideNumber: 1, Text: "First line\nSecond line"})
	require.NoError(t, err)
	assert.True(t, res.CreatedPart)
	assert.True(t, res.CreatedRelationship)
	assert.Equal(t, "/ppt/notesSlides/notesSlide1.xml", res.NotesURI)

	// Content-type override is registered.
	assert.Equal(t, notesContentType, pkg.GetContentType(res.NotesURI))

	// Forward slide->notesSlide relationship exists.
	hasNotesRel := false
	for _, rel := range pkg.ListRelationships("/ppt/slides/slide1.xml") {
		if rel.Type == notesRelationshipType {
			hasNotesRel = true
		}
	}
	assert.True(t, hasNotesRel, "expected slide->notesSlide relationship")

	requirePackageValid(t, pkg)

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	report, err := extract.ExtractNotesForSlide(pkg, graph.Slides[0])
	require.NoError(t, err)
	assert.Equal(t, "First line\nSecond line", report.Notes.PlainText)

	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()
}

func TestSetNotesForSlide_UpdatesExisting(t *testing.T) {
	fixture := "../../../testdata/pptx/notes-slide/presentation.pptx"
	pkg := openMutatePackage(t, fixture)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	slideNum := firstSlideWithNotes(t, graph)
	existingURI := graph.Slides[slideNum-1].NotesPartURI

	res, err := SetNotesForSlide(&SetNotesRequest{Package: pkg, SlideNumber: slideNum, Text: "Replacement notes"})
	require.NoError(t, err)
	assert.False(t, res.CreatedPart, "existing part should be reused")
	assert.False(t, res.CreatedRelationship)
	assert.Equal(t, existingURI, res.NotesURI)

	requirePackageValid(t, pkg)

	graph, err = inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	report, err := extract.ExtractNotesForSlide(pkg, graph.Slides[slideNum-1])
	require.NoError(t, err)
	assert.Equal(t, "Replacement notes", report.Notes.PlainText)
}

func TestSetNotesForSlide_LinksNotesMasterWhenPresent(t *testing.T) {
	fixture := "../../../testdata/pptx/notes-slide/presentation.pptx"
	pkg := openMutatePackage(t, fixture)
	defer pkg.Close()

	// Slide 1 in this fixture has no notes but the deck has a notesMaster.
	res, err := SetNotesForSlide(&SetNotesRequest{Package: pkg, SlideNumber: 1, Text: "Notes for slide one"})
	require.NoError(t, err)
	assert.True(t, res.CreatedPart)

	hasMasterRel := false
	for _, rel := range pkg.ListRelationships(res.NotesURI) {
		if rel.Type == notesMasterRelationshipType {
			hasMasterRel = true
		}
	}
	assert.True(t, hasMasterRel, "new notesSlide should link to the existing notesMaster")
	requirePackageValid(t, pkg)
}

func TestSetNotesForSlide_SynthesizesNotesMasterWhenAbsent(t *testing.T) {
	// title-content has a slideMaster/theme but no notesMaster. Creating a
	// notesSlide must synthesize a notesMaster so the inherited color map
	// (clrMapOvr/masterClrMapping) has a master to resolve against.
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	pkg := openMutatePackage(t, fixture)
	defer pkg.Close()

	// Sanity: the source deck genuinely has no notesMaster.
	require.Empty(t, findNotesMasterURI(pkg), "fixture precondition: deck must have no notesMaster")

	res, err := SetNotesForSlide(&SetNotesRequest{Package: pkg, SlideNumber: 1, Text: "Notes needing a master"})
	require.NoError(t, err)
	assert.True(t, res.CreatedPart)

	// A notesMaster part now exists with the correct content type.
	masterURI := findNotesMasterURI(pkg)
	require.NotEmpty(t, masterURI, "a notesMaster part should be synthesized")
	assert.Equal(t, notesMasterContentType, pkg.GetContentType(masterURI))

	// The presentation carries a notesMaster relationship.
	hasPresMasterRel := false
	for _, rel := range pkg.ListRelationships("/ppt/presentation.xml") {
		if rel.Type == notesMasterRelationshipType {
			hasPresMasterRel = true
		}
	}
	assert.True(t, hasPresMasterRel, "presentation should link to the synthesized notesMaster")

	// The new notesSlide links to the notesMaster.
	hasNotesMasterRel := false
	for _, rel := range pkg.ListRelationships(res.NotesURI) {
		if rel.Type == notesMasterRelationshipType {
			hasNotesMasterRel = true
		}
	}
	assert.True(t, hasNotesMasterRel, "new notesSlide should link to the synthesized notesMaster")

	// Strict validation still passes: no Error and no Warning diagnostics
	// (the CLI's --strict mode escalates warnings to failures).
	requirePackageValidStrict(t, pkg)
}

func TestClearNotesForSlide_EmptiesText(t *testing.T) {
	fixture := "../../../testdata/pptx/notes-slide/presentation.pptx"
	pkg := openMutatePackage(t, fixture)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	slideNum := firstSlideWithNotes(t, graph)

	res, err := ClearNotesForSlide(&ClearNotesRequest{Package: pkg, SlideNumber: slideNum})
	require.NoError(t, err)
	assert.False(t, res.CreatedPart)
	assert.Equal(t, "", res.Text)

	requirePackageValid(t, pkg)

	graph, err = inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	report, err := extract.ExtractNotesForSlide(pkg, graph.Slides[slideNum-1])
	require.NoError(t, err)
	assert.Equal(t, "", report.Notes.PlainText)
}

func TestSetNotesForSlide_SlideOutOfRange(t *testing.T) {
	fixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	pkg := openMutatePackage(t, fixture)
	defer pkg.Close()

	_, err := SetNotesForSlide(&SetNotesRequest{Package: pkg, SlideNumber: 99, Text: "x"})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "presentation has")
}

func TestSetNotesForSlide_SpecialCharsRoundTrip(t *testing.T) {
	fixture := "../../../testdata/pptx/minimal-title/presentation.pptx"
	pkg := openMutatePackage(t, fixture)
	defer pkg.Close()

	text := "A & B < C > D\nLine two with \"quotes\""
	_, err := SetNotesForSlide(&SetNotesRequest{Package: pkg, SlideNumber: 1, Text: text})
	require.NoError(t, err)
	requirePackageValid(t, pkg)

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	report, err := extract.ExtractNotesForSlide(pkg, graph.Slides[0])
	require.NoError(t, err)
	assert.Equal(t, text, report.Notes.PlainText)
}
