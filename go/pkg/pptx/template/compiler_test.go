package template

import (
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

func createSimpleManifestForCompiling() *TemplateManifest {
	now := time.Now()
	return &TemplateManifest{
		ManifestVersion: "1.0",
		Name:            "Simple Template",
		Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
		CreatedAt:       now,
		ModifiedAt:      now,
		Archetypes: []Archetype{
			{
				ID:   "title-slide",
				Name: "Title Slide",
				Slots: []Slot{
					{
						ID:       "title",
						Name:     "Title",
						Kind:     SlotKindText,
						Required: true,
					},
					{
						ID:       "subtitle",
						Name:     "Subtitle",
						Kind:     SlotKindText,
						Required: false,
					},
				},
			},
			{
				ID:   "content-slide",
				Name: "Content Slide",
				Slots: []Slot{
					{
						ID:       "title",
						Name:     "Title",
						Kind:     SlotKindText,
						Required: true,
					},
					{
						ID:       "body",
						Name:     "Body",
						Kind:     SlotKindBullets,
						Required: true,
					},
				},
			},
		},
	}
}

func TestCompilerEngineBasicCreation(t *testing.T) {
	manifest := createSimpleManifestForCompiling()
	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title":    "My Title",
					"subtitle": "My Subtitle",
				},
			},
		},
	}

	options := CompileOptions{
		ArchetypePath:   "archetype.pptx",
		OutputPath:      "output.pptx",
		ContinueOnError: true,
	}

	engine := NewCompilerEngine(manifest, spec, options)
	if engine == nil {
		t.Fatal("Engine creation failed")
	}

	if engine.manifest != manifest {
		t.Error("Engine manifest not set correctly")
	}

	if engine.spec != spec {
		t.Error("Engine spec not set correctly")
	}

	if engine.options.ArchetypePath != "archetype.pptx" {
		t.Error("Engine options not set correctly")
	}
}

func TestCompilerValidationMissingManifest(t *testing.T) {
	spec := &CompilationSpec{
		Slides: []SlideSpec{},
	}

	options := CompileOptions{
		ArchetypePath: "test.pptx",
		OutputPath:    "out.pptx",
	}

	engine := NewCompilerEngine(nil, spec, options)
	err := engine.validateInputs()
	if err == nil || err.Error() != "manifest is nil" {
		t.Errorf("Expected manifest validation error, got: %v", err)
	}
}

func TestCompilerValidationMissingSpec(t *testing.T) {
	manifest := createSimpleManifestForCompiling()
	options := CompileOptions{
		ArchetypePath: "test.pptx",
		OutputPath:    "out.pptx",
	}

	engine := NewCompilerEngine(manifest, nil, options)
	err := engine.validateInputs()
	if err == nil || err.Error() != "spec is nil" {
		t.Errorf("Expected spec validation error, got: %v", err)
	}
}

func TestCompilerValidationMissingArchetypePath(t *testing.T) {
	manifest := createSimpleManifestForCompiling()
	spec := &CompilationSpec{
		Slides: []SlideSpec{},
	}

	options := CompileOptions{
		ArchetypePath: "",
		OutputPath:    "out.pptx",
	}

	engine := NewCompilerEngine(manifest, spec, options)
	err := engine.validateInputs()
	if err == nil || err.Error() != "archetype path is empty" {
		t.Errorf("Expected archetype path validation error, got: %v", err)
	}
}

func TestCompilerValidationMissingOutputPath(t *testing.T) {
	manifest := createSimpleManifestForCompiling()
	spec := &CompilationSpec{
		Slides: []SlideSpec{},
	}

	options := CompileOptions{
		ArchetypePath: "test.pptx",
		OutputPath:    "",
	}

	engine := NewCompilerEngine(manifest, spec, options)
	err := engine.validateInputs()
	if err == nil || err.Error() != "output path is empty" {
		t.Errorf("Expected output path validation error, got: %v", err)
	}
}

func TestCompilerValidationArchetypeNotFound(t *testing.T) {
	manifest := createSimpleManifestForCompiling()
	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title": "Test",
				},
			},
		},
	}

	options := CompileOptions{
		ArchetypePath: "/nonexistent/path/to/archetype.pptx",
		OutputPath:    "out.pptx",
	}

	engine := NewCompilerEngine(manifest, spec, options)
	err := engine.validateInputs()
	if err == nil {
		t.Error("Expected archetype not found error")
	}
}

func TestCompilerValidationInvalidSpec(t *testing.T) {
	manifest := createSimpleManifestForCompiling()
	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "unknown-archetype",
				Content: map[string]interface{}{
					"title": "Test",
				},
			},
		},
	}

	// Create a temporary file for archetype
	tmpFile, err := os.CreateTemp("", "archetype*.pptx")
	if err != nil {
		t.Fatalf("Failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	options := CompileOptions{
		ArchetypePath: tmpFile.Name(),
		OutputPath:    "out.pptx",
	}

	engine := NewCompilerEngine(manifest, spec, options)
	err = engine.validateInputs()
	if err == nil {
		t.Error("Expected spec validation error for unknown archetype")
	}
}

func TestContentToString(t *testing.T) {
	engine := &CompilerEngine{}

	tests := []struct {
		name     string
		content  interface{}
		expected string
	}{
		{
			name:     "string content",
			content:  "hello world",
			expected: "hello world",
		},
		{
			name: "map with text",
			content: map[string]interface{}{
				"text": "hello from map",
			},
			expected: "hello from map",
		},
		{
			name:     "nil content",
			content:  nil,
			expected: "",
		},
		{
			name: "map without text",
			content: map[string]interface{}{
				"other": "value",
			},
			expected: "",
		},
		{
			name:     "number content",
			content:  123,
			expected: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := engine.contentToString(tt.content)
			if result != tt.expected {
				t.Errorf("contentToString() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestParseBulletList(t *testing.T) {
	engine := &CompilerEngine{}

	tests := []struct {
		name     string
		text     string
		expected []string
	}{
		{
			name:     "simple bullets",
			text:     "• Point 1\n• Point 2\n• Point 3",
			expected: []string{"Point 1", "Point 2", "Point 3"},
		},
		{
			name:     "dashes",
			text:     "- Item 1\n- Item 2",
			expected: []string{"Item 1", "Item 2"},
		},
		{
			name:     "asterisks",
			text:     "* First\n* Second",
			expected: []string{"First", "Second"},
		},
		{
			name:     "mixed markers",
			text:     "• First\n- Second\n* Third",
			expected: []string{"First", "Second", "Third"},
		},
		{
			name:     "no markers",
			text:     "First\nSecond\nThird",
			expected: []string{"First", "Second", "Third"},
		},
		{
			name:     "empty lines",
			text:     "• Point 1\n\n• Point 2",
			expected: []string{"Point 1", "Point 2"},
		},
		{
			name:     "single line",
			text:     "Single point",
			expected: []string{"Single point"},
		},
		{
			name:     "empty string",
			text:     "",
			expected: []string{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := engine.parseBulletList(tt.text)
			if len(result) != len(tt.expected) {
				t.Errorf("parseBulletList() returned %d items, want %d", len(result), len(tt.expected))
				return
			}

			for i, r := range result {
				if r != tt.expected[i] {
					t.Errorf("parseBulletList()[%d] = %q, want %q", i, r, tt.expected[i])
				}
			}
		})
	}
}

func TestDetectImageContentType(t *testing.T) {
	engine := &CompilerEngine{}

	tests := []struct {
		name     string
		path     string
		expected string
		wantErr  bool
	}{
		{
			name:     "jpeg extension",
			path:     "/path/to/image.jpg",
			expected: "image/jpeg",
		},
		{
			name:     "jpeg uppercase",
			path:     "/path/to/image.JPEG",
			expected: "image/jpeg",
		},
		{
			name:     "png extension",
			path:     "/path/to/image.png",
			expected: "image/png",
		},
		{
			name:     "gif extension",
			path:     "/path/to/image.gif",
			expected: "image/gif",
		},
		{
			name:     "bmp extension",
			path:     "/path/to/image.bmp",
			expected: "image/bmp",
		},
		{
			name:     "svg extension",
			path:     "/path/to/image.svg",
			expected: "image/svg+xml",
		},
		{
			name:    "unknown extension",
			path:    "/path/to/image.unknown",
			wantErr: true,
		},
		{
			name:    "no extension",
			path:    "/path/to/image",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := engine.detectImageContentType(tt.path)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected unsupported image type error, got nil")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if result != tt.expected {
				t.Errorf("detectImageContentType() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestCompileErrorFormatting(t *testing.T) {
	tests := []struct {
		name string
		err  *CompileError
		want string
	}{
		{
			name: "with slot ID",
			err: &CompileError{
				SlideIndex: 0,
				SlotID:     "title",
				Message:    "slot not found",
			},
			want: "slide 0, slot title: slot not found",
		},
		{
			name: "without slot ID",
			err: &CompileError{
				SlideIndex: 2,
				Message:    "failed to clone slide",
			},
			want: "slide 2: failed to clone slide",
		},
		{
			name: "with error",
			err: &CompileError{
				SlideIndex: 1,
				SlotID:     "image",
				Message:    "image load failed",
				Err:        os.ErrNotExist,
			},
			want: "slide 1, slot image: image load failed",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := tt.err.Error()
			if result != tt.want {
				t.Errorf("Error() = %q, want %q", result, tt.want)
			}
		})
	}
}

func TestImagePathResolution(t *testing.T) {
	engine := &CompilerEngine{
		options: CompileOptions{
			ImageBaseDir: "/base/dir",
		},
	}

	tests := []struct {
		name     string
		baseDir  string
		path     string
		expected string
	}{
		{
			name:     "relative path",
			baseDir:  "/base",
			path:     "images/photo.png",
			expected: filepath.Join("/base", "images/photo.png"),
		},
		{
			name:     "absolute path",
			baseDir:  "/base",
			path:     filepath.Join(filepath.VolumeName(os.TempDir())+string(os.PathSeparator), "absolute", "path", "photo.png"),
			expected: filepath.Join(filepath.VolumeName(os.TempDir())+string(os.PathSeparator), "absolute", "path", "photo.png"),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			engine.options.ImageBaseDir = tt.baseDir

			// Simulate the path resolution logic
			imagePath := tt.path
			if engine.options.ImageBaseDir != "" && !filepath.IsAbs(imagePath) {
				imagePath = filepath.Join(engine.options.ImageBaseDir, imagePath)
			}

			if imagePath != tt.expected {
				t.Errorf("Path resolution = %q, want %q", imagePath, tt.expected)
			}
		})
	}
}

// TestCompilerEngineInitialState verifies the engine starts with an empty runtime state.
func TestCompilerEngineInitialState(t *testing.T) {
	engine := NewCompilerEngine(createSimpleManifestForCompiling(), &CompilationSpec{}, CompileOptions{})
	if engine.seedSlideCount != 0 {
		t.Errorf("seedSlideCount should start at 0, got %d", engine.seedSlideCount)
	}
	if engine.currentLastSlide != 0 {
		t.Errorf("currentLastSlide should start at 0, got %d", engine.currentLastSlide)
	}
}

// Image slot fill tests

func TestFillImageSlotEmptyPath(t *testing.T) {
	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: nil,
		imageCache:    make(map[string][]byte),
	}

	slot := &Slot{
		ID:   "image",
		Name: "Image",
		Kind: SlotKindImage,
	}

	err := engine.fillImageSlot(1, slot, "")
	if err == nil {
		t.Error("expected error for empty image path")
	}
	if err.Error() != "image path is empty" {
		t.Errorf("expected 'image path is empty', got %q", err.Error())
	}
}

func TestFillImageSlotMissingFile(t *testing.T) {
	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: nil,
		imageCache:    make(map[string][]byte),
	}

	slot := &Slot{
		ID:   "image",
		Name: "Image",
		Kind: SlotKindImage,
	}

	err := engine.fillImageSlot(1, slot, "/nonexistent/image.png")
	if err == nil {
		t.Error("expected error for missing image file")
	}
	if !strings.Contains(err.Error(), "failed to load image") {
		t.Errorf("expected error to contain 'failed to load image', got %q", err.Error())
	}
}

func TestFillImageSlotNoSlotIdentifier(t *testing.T) {
	// Create a temporary output file
	tmpFile, err := os.CreateTemp("", "test*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	// Copy a test fixture to the temp file
	src, err := os.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test fixture: %v", err)
	}
	defer src.Close()

	out, err := os.Create(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to create output file: %v", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, src); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}
	out.Close()

	// Open the presentation
	session, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to open session: %v", err)
	}
	defer session.Close()

	slot := &Slot{
		ID:   "image",
		Name: "", // No name
		Kind: SlotKindImage,
		// No PlaceholderKey either
	}

	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: session,
		imageCache:    make(map[string][]byte),
	}

	err = engine.fillImageSlot(1, slot, "../../../testdata/test_image.png")
	if err == nil {
		t.Error("expected error when slot has no Name or PlaceholderKey")
	}
	if !strings.Contains(err.Error(), "neither PlaceholderKey nor Name") {
		t.Errorf("expected error about missing identifier, got %q", err.Error())
	}
}

func TestFillImageSlotInvalidSlideNumber(t *testing.T) {
	// Create a temporary output file
	tmpFile, err := os.CreateTemp("", "test*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	// Copy a test fixture
	src, err := os.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test fixture: %v", err)
	}
	defer src.Close()

	out, err := os.Create(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to create output file: %v", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, src); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}
	out.Close()

	// Open the presentation
	session, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to open session: %v", err)
	}
	defer session.Close()

	slot := &Slot{
		ID:             "image",
		Name:           "Image",
		Kind:           SlotKindImage,
		PlaceholderKey: "pic",
	}

	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: session,
		imageCache:    make(map[string][]byte),
	}

	// Try with invalid slide number (too high)
	err = engine.fillImageSlot(999, slot, "../../../testdata/test_image.png")
	if err == nil {
		t.Error("expected error for invalid slide number")
	}
	if !strings.Contains(err.Error(), "out of range") {
		t.Errorf("expected error about slide number range, got %q", err.Error())
	}
}

func TestFillTableSlotWithHeaders(t *testing.T) {
	tmpFile, err := os.CreateTemp("", "test*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	src, err := os.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test fixture: %v", err)
	}
	defer src.Close()

	out, err := os.Create(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to create output file: %v", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, src); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}
	out.Close()

	session, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to open session: %v", err)
	}
	defer session.Close()

	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: session,
		imageCache:    make(map[string][]byte),
	}

	tableContent := map[string]interface{}{
		"data": []interface{}{
			[]interface{}{"Name", "Age", "City"},
			[]interface{}{"Alice", 30, "New York"},
			[]interface{}{"Bob", 25, "Los Angeles"},
		},
		"hasHeaders": true,
		"bandedRows": true,
	}

	slot := &Slot{
		ID:       "table",
		Name:     "Table",
		Kind:     SlotKindTable,
		Required: false,
	}

	err = engine.fillTableSlot(1, slot, tableContent)
	if err != nil {
		t.Fatalf("fillTableSlot failed: %v", err)
	}

	err = engine.outputSession.SaveAs(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to save session: %v", err)
	}
	err = engine.outputSession.Close()
	if err != nil {
		t.Fatalf("failed to close session: %v", err)
	}

	session2, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to reopen session: %v", err)
	}
	defer session2.Close()

	parsedGraph, err := inspect.ParsePresentation(session2)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	if len(parsedGraph.Slides) == 0 {
		t.Fatal("presentation has no slides")
	}

	slide := parsedGraph.Slides[0]
	slideDoc, err := session2.ReadXMLPart(slide.PartURI)
	if err != nil {
		t.Fatalf("failed to read slide: %v", err)
	}

	spTree := slideDoc.FindElement(".//p:spTree")
	if spTree == nil {
		spTree = slideDoc.FindElement(".//spTree")
	}
	if spTree == nil {
		t.Fatal("slide has no shape tree")
	}

	tables := slideDoc.FindElements(".//p:graphicFrame//a:tbl")
	if len(tables) == 0 {
		tables = slideDoc.FindElements(".//graphicFrame//tbl")
	}
	if len(tables) == 0 {
		t.Fatal("no table found in slide after insertion")
	}

	tbl := tables[0]
	rows := tbl.FindElements(".//a:tr")
	if len(rows) == 0 {
		rows = tbl.FindElements(".//tr")
	}
	if len(rows) < 3 {
		t.Errorf("expected at least 3 rows (1 header + 2 data), got %d", len(rows))
	}
}

func TestFillTableSlotUpdatesCapturedTableInPlace(t *testing.T) {
	session, err := opc.Open("../../../testdata/pptx/table-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table fixture: %v", err)
	}
	defer session.Close()

	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: session,
		imageCache:    make(map[string][]byte),
	}

	rows, cols, tableID := 3, 3, 2
	slot := &Slot{
		ID:        "captured-table",
		Name:      "Captured Table",
		Kind:      SlotKindTable,
		TableRows: &rows,
		TableCols: &cols,
		TableID:   &tableID,
	}
	tableContent := map[string]interface{}{
		"data": []interface{}{
			[]interface{}{"A", "B", "C"},
			[]interface{}{"D", "E", "F"},
			[]interface{}{"G", "H", "I"},
		},
	}

	if err := engine.fillTableSlot(2, slot, tableContent); err != nil {
		t.Fatalf("fillTableSlot failed: %v", err)
	}

	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}
	slideDoc, err := session.ReadXMLPart(graph.Slides[1].PartURI)
	if err != nil {
		t.Fatalf("failed to read slide: %v", err)
	}
	tables := slideDoc.FindElements(".//a:tbl")
	if len(tables) != 1 {
		t.Fatalf("table fill should update in place; found %d tables", len(tables))
	}
	table := inspect.ParseTable(tables[0])
	if table == nil {
		t.Fatal("table parse returned nil")
	}
	if got := table.Cells[0][0]; got != "A" {
		t.Fatalf("top-left cell = %q, want A", got)
	}
	if got := table.Cells[2][2]; got != "I" {
		t.Fatalf("bottom-right cell = %q, want I", got)
	}
}

func TestFillTableSlotWithoutHeaders(t *testing.T) {
	tmpFile, err := os.CreateTemp("", "test*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	src, err := os.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test fixture: %v", err)
	}
	defer src.Close()

	out, err := os.Create(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to create output file: %v", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, src); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}
	out.Close()

	session, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to open session: %v", err)
	}
	defer session.Close()

	engine := &CompilerEngine{
		manifest:      &TemplateManifest{},
		spec:          &CompilationSpec{},
		outputSession: session,
		imageCache:    make(map[string][]byte),
	}

	tableContent := map[string]interface{}{
		"data": []interface{}{
			[]interface{}{"Product A", "100"},
			[]interface{}{"Product B", "200"},
			[]interface{}{"Product C", "150"},
		},
		"hasHeaders": false,
		"bandedRows": false,
	}

	slot := &Slot{
		ID:       "table",
		Name:     "Table",
		Kind:     SlotKindTable,
		Required: false,
	}

	err = engine.fillTableSlot(1, slot, tableContent)
	if err != nil {
		t.Fatalf("fillTableSlot failed: %v", err)
	}

	err = engine.outputSession.SaveAs(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to save session: %v", err)
	}
	err = engine.outputSession.Close()
	if err != nil {
		t.Fatalf("failed to close session: %v", err)
	}

	session2, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to reopen session: %v", err)
	}
	defer session2.Close()

	parsedGraph, err := inspect.ParsePresentation(session2)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	if len(parsedGraph.Slides) == 0 {
		t.Fatal("presentation has no slides")
	}

	slide := parsedGraph.Slides[0]
	slideDoc, err := session2.ReadXMLPart(slide.PartURI)
	if err != nil {
		t.Fatalf("failed to read slide: %v", err)
	}

	spTree := slideDoc.FindElement(".//p:spTree")
	if spTree == nil {
		spTree = slideDoc.FindElement(".//spTree")
	}
	if spTree == nil {
		t.Fatal("slide has no shape tree")
	}

	tables := slideDoc.FindElements(".//p:graphicFrame//a:tbl")
	if len(tables) == 0 {
		tables = slideDoc.FindElements(".//graphicFrame//tbl")
	}
	if len(tables) == 0 {
		t.Fatal("no table found in slide after insertion")
	}

	tbl := tables[0]
	rows := tbl.FindElements(".//a:tr")
	if len(rows) == 0 {
		rows = tbl.FindElements(".//tr")
	}
	if len(rows) != 3 {
		t.Errorf("expected 3 rows, got %d", len(rows))
	}
}
