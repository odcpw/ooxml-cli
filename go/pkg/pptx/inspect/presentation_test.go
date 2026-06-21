package inspect

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

func TestParsePresentation_MultiLayout(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test slide size (screen 4:3 = 9144000 x 6858000 EMU)
	if graph.SlideSize.CX != 9144000 {
		t.Errorf("slide size CX: got %d, want 9144000", graph.SlideSize.CX)
	}
	if graph.SlideSize.CY != 6858000 {
		t.Errorf("slide size CY: got %d, want 6858000", graph.SlideSize.CY)
	}

	// Test master count (should have 1 master)
	if len(graph.Masters) != 1 {
		t.Errorf("master count: got %d, want 1", len(graph.Masters))
	}

	// Test that the master has the correct part URI
	if graph.Masters[0].PartURI != "/ppt/slideMasters/slideMaster1.xml" {
		t.Errorf("master PartURI: got %s, want /ppt/slideMasters/slideMaster1.xml", graph.Masters[0].PartURI)
	}

	// Test that the master has 11 layout URIs
	if len(graph.Masters[0].LinkedLayoutURIs) != 11 {
		t.Errorf("layout count: got %d, want 11", len(graph.Masters[0].LinkedLayoutURIs))
	}

	// Test that the master has a theme URI
	if graph.Masters[0].ThemeURI != "/ppt/theme/theme1.xml" {
		t.Errorf("theme URI: got %s, want /ppt/theme/theme1.xml", graph.Masters[0].ThemeURI)
	}

	// Test layout count
	if len(graph.Layouts) != 11 {
		t.Errorf("layout count: got %d, want 11", len(graph.Layouts))
	}

	// Test the first layout has correct name
	if graph.Layouts[0].Name != "Title Slide" {
		t.Errorf("layout 1 name: got %s, want Title Slide", graph.Layouts[0].Name)
	}

	// Test the first layout has correct master reference
	if graph.Layouts[0].MasterPartURI != "/ppt/slideMasters/slideMaster1.xml" {
		t.Errorf("layout 1 master: got %s, want /ppt/slideMasters/slideMaster1.xml", graph.Layouts[0].MasterPartURI)
	}

	// Test slide count
	if len(graph.Slides) != 4 {
		t.Errorf("slide count: got %d, want 4", len(graph.Slides))
	}

	// Test that slides are in presentation order (1-based)
	for i, slide := range graph.Slides {
		expectedNumber := i + 1
		if slide.SlideNumber != expectedNumber {
			t.Errorf("slide %d number: got %d, want %d", i, slide.SlideNumber, expectedNumber)
		}
	}

	// Test the first slide
	if graph.Slides[0].PartURI != "/ppt/slides/slide1.xml" {
		t.Errorf("slide 1 PartURI: got %s, want /ppt/slides/slide1.xml", graph.Slides[0].PartURI)
	}

	// Test that the first slide has a layout reference
	if graph.Slides[0].LayoutPartURI != "/ppt/slideLayouts/slideLayout1.xml" {
		t.Errorf("slide 1 layout: got %s, want /ppt/slideLayouts/slideLayout1.xml", graph.Slides[0].LayoutPartURI)
	}

	// Test that the first slide has no notes
	if graph.Slides[0].NotesPartURI != "" {
		t.Errorf("slide 1 notes: got %s, want empty", graph.Slides[0].NotesPartURI)
	}
}

func TestParsePresentation_NotesSlide(t *testing.T) {
	// Open the notes-slide test fixture
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test slide count
	if len(graph.Slides) != 2 {
		t.Errorf("slide count: got %d, want 2", len(graph.Slides))
	}

	// Test that the first slide has no notes
	if graph.Slides[0].NotesPartURI != "" {
		t.Errorf("slide 1 notes: got %s, want empty", graph.Slides[0].NotesPartURI)
	}

	// Test that the second slide has notes
	if graph.Slides[1].NotesPartURI != "/ppt/notesSlides/notesSlide1.xml" {
		t.Errorf("slide 2 notes: got %s, want /ppt/notesSlides/notesSlide1.xml", graph.Slides[1].NotesPartURI)
	}
}

func TestParsePresentation_MinimalTitle(t *testing.T) {
	// Open the minimal-title test fixture
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test that we have slide size
	if graph.SlideSize.CX == 0 || graph.SlideSize.CY == 0 {
		t.Errorf("slide size not set: CX=%d, CY=%d", graph.SlideSize.CX, graph.SlideSize.CY)
	}

	// Test that we have at least 1 master
	if len(graph.Masters) < 1 {
		t.Errorf("master count: got %d, want >= 1", len(graph.Masters))
	}

	// Test that we have slides
	if len(graph.Slides) < 1 {
		t.Errorf("slide count: got %d, want >= 1", len(graph.Slides))
	}

	// Test that all slides have layout references
	for i, slide := range graph.Slides {
		if slide.LayoutPartURI == "" {
			t.Errorf("slide %d has no layout reference", i+1)
		}
	}
}

func TestParsePresentation_MissingEmptySlideList(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	presentationDoc, err := pkg.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		t.Fatalf("failed to read presentation.xml: %v", err)
	}
	root := presentationDoc.Root()
	slideList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst")
	if slideList == nil {
		t.Fatal("expected slide list in fixture")
	}
	root.RemoveChild(slideList)
	if err := pkg.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		t.Fatalf("failed to patch presentation.xml: %v", err)
	}
	outPath := filepath.Join(t.TempDir(), "no-slides.pptx")
	if err := pkg.SaveAs(outPath); err != nil {
		t.Fatalf("failed to save patched presentation: %v", err)
	}
	pkg.Close()

	patched, err := opc.Open(outPath)
	if err != nil {
		t.Fatalf("failed to reopen patched presentation: %v", err)
	}
	defer patched.Close()

	graph, err := ParsePresentation(patched)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}
	if len(graph.Slides) != 0 {
		t.Fatalf("expected zero slides when p:sldIdLst is missing, got %d", len(graph.Slides))
	}
}

func TestParsePresentation_SingleMaster(t *testing.T) {
	// Open a simple test fixture (title-content)
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test that we have at least 1 master
	if len(graph.Masters) < 1 {
		t.Errorf("master count: got %d, want >= 1", len(graph.Masters))
	}

	// Test that we have slides
	if len(graph.Slides) < 1 {
		t.Errorf("slide count: got %d, want >= 1", len(graph.Slides))
	}

	// Test that all slides have layout references
	for i, slide := range graph.Slides {
		if slide.LayoutPartURI == "" {
			t.Errorf("slide %d has no layout reference", i+1)
		}
	}
}

func TestParsePresentation_LayoutNames(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test that layout names are properly extracted
	expectedNames := map[string]bool{
		"Title Slide":             false,
		"Title and Content":       false,
		"Section Header":          false,
		"Two Content":             false,
		"Comparison":              false,
		"Title Only":              false,
		"Blank":                   false,
		"Content with Caption":    false,
		"Picture with Caption":    false,
		"Title and Vertical Text": false,
		"Vertical Title and Text": false,
	}

	// Check that we have all expected names
	for _, layout := range graph.Layouts {
		if _, exists := expectedNames[layout.Name]; exists {
			expectedNames[layout.Name] = true
		}
	}

	// Verify all expected names are present
	for name, found := range expectedNames {
		if !found {
			t.Errorf("expected layout name not found: %s", name)
		}
	}
}

func TestParsePresentation_SlideLayoutMapping(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test that each slide references a valid layout
	layoutURIs := make(map[string]bool)
	for _, layout := range graph.Layouts {
		layoutURIs[layout.PartURI] = true
	}

	for i, slide := range graph.Slides {
		if !layoutURIs[slide.LayoutPartURI] {
			t.Errorf("slide %d references non-existent layout: %s", i+1, slide.LayoutPartURI)
		}
	}
}

func TestParsePresentation_MasterLayoutMapping(t *testing.T) {
	// Open the multi-layout test fixture
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("ParsePresentation failed: %v", err)
	}

	// Test that each layout's master reference is valid
	masterURIs := make(map[string]bool)
	for _, master := range graph.Masters {
		masterURIs[master.PartURI] = true
	}

	for i, layout := range graph.Layouts {
		if layout.MasterPartURI != "" && !masterURIs[layout.MasterPartURI] {
			t.Errorf("layout %d references non-existent master: %s", i+1, layout.MasterPartURI)
		}
	}
}
