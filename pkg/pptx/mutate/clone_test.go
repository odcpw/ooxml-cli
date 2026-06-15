package mutate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCloneSlide_CopiesSlideContentAndUpdatesPresentation(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "cloned.pptx")

	pkg := openMutatePackage(t, fixture)
	res, err := CloneSlide(&CloneSlideRequest{Package: pkg, SlideNumber: 1})
	require.NoError(t, err)
	require.Equal(t, 2, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err := inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	text, err := extract.ExtractText(&extract.ExtractTextRequest{Session: cloned, Graph: graph, SlideNumbers: []int{1, 2}})
	require.NoError(t, err)
	require.Len(t, text.Slides, 2)
	assert.Equal(t, text.Slides[0].Shapes[0].Text.PlainText, text.Slides[1].Shapes[0].Text.PlainText)
	requirePackageValid(t, cloned)
}

func TestCloneSlide_ClonesNotesWhenPresent(t *testing.T) {
	fixture := "../../../testdata/pptx/notes-slide/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "notes-cloned.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	sourceSlide := firstSlideWithNotes(t, graph)

	res, err := CloneSlide(&CloneSlideRequest{Package: pkg, SlideNumber: sourceSlide})
	require.NoError(t, err)
	assert.NotEmpty(t, res.NotesURI)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err = inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.GreaterOrEqual(t, len(graph.Slides), sourceSlide+1)
	assert.NotEmpty(t, graph.Slides[sourceSlide].NotesPartURI)
	requirePackageValid(t, cloned)
}

func TestCloneSlideClonedNotesBacklinkTargetsClone(t *testing.T) {
	fixture := "../../../testdata/pptx/notes-slide/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "notes-backlink-cloned.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	sourceSlideNumber := firstSlideWithNotes(t, graph)
	sourceSlide := graph.Slides[sourceSlideNumber-1]

	res, err := CloneSlide(&CloneSlideRequest{Package: pkg, SlideNumber: sourceSlideNumber})
	require.NoError(t, err)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err = inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.GreaterOrEqual(t, len(graph.Slides), res.NewSlideNumber)
	clonedSlide := graph.Slides[res.NewSlideNumber-1]
	require.NotEmpty(t, clonedSlide.NotesPartURI)

	backlinkTarget := notesSlideBacklinkTarget(t, cloned, clonedSlide.NotesPartURI)
	assert.Equal(t, clonedSlide.PartURI, backlinkTarget)
	assert.NotEqual(t, sourceSlide.PartURI, backlinkTarget)
	requirePackageValid(t, cloned)
}

func TestCloneSlide_PreservesImageRelationships(t *testing.T) {
	fixture := "../../../testdata/pptx/picture-placeholder/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "image-cloned.pptx")

	pkg := openMutatePackage(t, fixture)
	res, err := CloneSlide(&CloneSlideRequest{Package: pkg, SlideNumber: 2})
	require.NoError(t, err)
	require.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err := inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	sourceImages := slideImages(t, cloned, graph.Slides[1])
	newImages := slideImages(t, cloned, graph.Slides[2])
	require.NotEmpty(t, sourceImages)
	require.NotEmpty(t, newImages)
	assert.Equal(t, sourceImages[0].TargetURI, newImages[0].TargetURI)
	requirePackageValid(t, cloned)
}

func openMutatePackage(t *testing.T, path string) *opc.Package {
	t.Helper()
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	return pkg
}

func firstSlideWithNotes(t *testing.T, graph *inspect.PresentationGraph) int {
	t.Helper()
	for _, slide := range graph.Slides {
		if slide.NotesPartURI != "" {
			return slide.SlideNumber
		}
	}
	t.Fatal("expected a slide with notes")
	return 0
}

func notesSlideBacklinkTarget(t *testing.T, session opc.PackageSession, notesURI string) string {
	t.Helper()
	for _, rel := range session.ListRelationships(notesURI) {
		if rel.Type == slideRelationshipType {
			return opc.ResolveRelationshipTarget(notesURI, rel.Target)
		}
	}
	t.Fatalf("expected notes slide %s to have a slide backlink", notesURI)
	return ""
}

func slideImages(t *testing.T, session opc.PackageSession, slide inspect.SlideRef) []model.ExtractedImageInfo {
	t.Helper()
	doc, err := session.ReadXMLPart(slide.PartURI)
	require.NoError(t, err)
	spTree := doc.Root().FindElement("//p:spTree")
	return inspect.EnumerateImageRelationships(slide.PartURI, session, spTree)
}

func requirePackageValid(t *testing.T, session opc.PackageSession) {
	t.Helper()
	diags, err := validate.ValidatePackage(session)
	require.NoError(t, err)
	for _, diag := range diags {
		require.NotEqual(t, result.Error, diag.Severity, diag.Message)
	}
}
