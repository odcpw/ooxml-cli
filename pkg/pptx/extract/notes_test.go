package extract

import (
	"path/filepath"
	"runtime"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// getTestFilePath returns the full path to a test fixture file
func getTestFilePath(fixtureDir, filename string) string {
	_, currentFile, _, _ := runtime.Caller(0)
	projectRoot := filepath.Dir(filepath.Dir(filepath.Dir(filepath.Dir(currentFile))))
	return filepath.Join(projectRoot, "testdata", "pptx", fixtureDir, filename)
}

func TestExtractNotesForSlideWithNotes(t *testing.T) {
	testFile := getTestFilePath("notes-slide", "presentation.pptx")

	session, err := opc.Open(testFile)
	if err != nil {
		t.Skipf("failed to open test file: %v", err)
	}
	defer session.Close()

	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Find the slide with notes
	var slideWithNotes *inspect.SlideRef
	for _, slide := range graph.Slides {
		if slide.NotesPartURI != "" {
			slideWithNotes = &slide
			break
		}
	}

	if slideWithNotes == nil {
		t.Skip("no slide with notes found in fixture")
	}

	report, err := ExtractNotesForSlide(session, *slideWithNotes)
	if err != nil {
		t.Fatalf("ExtractNotesForSlide failed: %v", err)
	}

	if report == nil {
		t.Error("expected NotesReport, got nil")
	}

	if report.Slide == 0 {
		t.Error("expected slide number > 0")
	}

	if report.Notes == nil {
		t.Error("expected Notes to be non-nil")
	}

	if report.Notes.PlainText == "" {
		t.Error("expected notes text to be non-empty")
	}

	// Verify the notes contain expected text
	if len(report.Notes.Paragraphs) == 0 {
		t.Error("expected at least one paragraph in notes")
	}
}

func TestExtractNotesForSlideWithoutNotes(t *testing.T) {
	testFile := getTestFilePath("minimal-title", "presentation.pptx")

	session, err := opc.Open(testFile)
	if err != nil {
		t.Skipf("failed to open test file: %v", err)
	}
	defer session.Close()

	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	if len(graph.Slides) == 0 {
		t.Skip("no slides in fixture")
	}

	// Test first slide (should have no notes)
	report, err := ExtractNotesForSlide(session, graph.Slides[0])
	if err != nil {
		t.Fatalf("ExtractNotesForSlide failed: %v", err)
	}

	if report == nil {
		t.Error("expected NotesReport, got nil")
	}

	if report.Notes == nil {
		t.Error("expected Notes to be non-nil")
	}

	// Empty notes should be valid
	if report.Notes.PlainText != "" {
		t.Errorf("expected empty notes text for slide without notes, got: %s", report.Notes.PlainText)
	}

	if len(report.Notes.Paragraphs) != 0 {
		t.Errorf("expected no paragraphs for slide without notes, got %d", len(report.Notes.Paragraphs))
	}
}

func TestExtractNotesFromMultipleSlides(t *testing.T) {
	testFile := getTestFilePath("notes-handout", "presentation.pptx")

	session, err := opc.Open(testFile)
	if err != nil {
		t.Skipf("failed to open test file: %v", err)
	}
	defer session.Close()

	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	if len(graph.Slides) < 2 {
		t.Skip("fixture has fewer than 2 slides")
	}

	// Extract notes from multiple slides
	slideCount := len(graph.Slides)
	if slideCount > 3 {
		slideCount = 3 // Limit to 3 for testing
	}

	for i := 0; i < slideCount; i++ {
		report, err := ExtractNotesForSlide(session, graph.Slides[i])
		if err != nil {
			t.Errorf("ExtractNotesForSlide failed for slide %d: %v", i+1, err)
			continue
		}

		if report == nil {
			t.Errorf("expected NotesReport for slide %d, got nil", i+1)
			continue
		}

		if report.Slide != i+1 {
			t.Errorf("expected slide number %d, got %d", i+1, report.Slide)
		}

		if report.Notes == nil {
			t.Errorf("expected Notes to be non-nil for slide %d", i+1)
		}
	}
}
