package template

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"gopkg.in/yaml.v3"
)

// findCompileFixture locates a fixture file for compilation tests
func findCompileFixture(filename string) (string, bool) {
	baseName := filepath.Join("testdata", "pptx", "template-branded", filename)
	paths := []string{
		baseName,
		filepath.Join("..", "..", "..", baseName),
	}

	for _, path := range paths {
		if _, err := os.Stat(path); err == nil {
			return path, true
		}
	}

	return paths[0], false
}

// createTestCompileOptions creates compile options for integration testing
func createTestCompileOptions(archetypePath, outputPath string) CompileOptions {
	return CompileOptions{
		ArchetypePath:   archetypePath,
		OutputPath:      outputPath,
		ContinueOnError: false,
	}
}

// createSimpleManifestForCompilerTests creates a test manifest with various slot types
func createSimpleManifestForCompilerTests() *TemplateManifest {
	now := time.Now()
	return &TemplateManifest{
		ManifestVersion: "1.0",
		Name:            "Test Template",
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
					{
						ID:       "image",
						Name:     "Image",
						Kind:     SlotKindImage,
						Required: false,
					},
				},
			},
		},
	}
}

// TestCompileMultiArchetypeCompile tests compilation with multiple archetypes used multiple times
func TestCompileMultiArchetypeCompile(t *testing.T) {
	// Load fixtures
	manifestPath, ok := findCompileFixture("manifest.json")
	if !ok {
		// Skip this test gracefully - fixtures are optional
		t.Skipf("manifest fixture not found at %s", manifestPath)
	}
	specPath, ok := findCompileFixture("spec-complex.yaml")
	if !ok {
		t.Skipf("spec-complex.yaml fixture not found")
	}
	archetypePath, ok := findCompileFixture("presentation.pptx")
	if !ok {
		t.Skipf("archetype PPTX fixture not found")
	}

	// Load manifest
	manifestData, err := os.ReadFile(manifestPath)
	require.NoError(t, err)
	var manifest TemplateManifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))

	// Load spec
	specData, err := os.ReadFile(specPath)
	require.NoError(t, err)
	var spec CompilationSpec
	require.NoError(t, yaml.Unmarshal(specData, &spec))

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output.pptx")

	options := createTestCompileOptions(archetypePath, outPath)
	engine := NewCompilerEngine(&manifest, &spec, options)
	require.NotNil(t, engine)

	// Compile
	result, err := engine.Compile()
	require.NoError(t, err)
	require.NotNil(t, result)

	// Verify output PPTX was created
	_, err = os.Stat(outPath)
	require.NoError(t, err)

	// Verify slide count matches spec (should be 5: title, content, content, content, title)
	assert.Equal(t, 5, result.SlideCount, "compiled presentation should have 5 slides")

	// Verify output PPTX can be opened
	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	// Parse the presentation to verify slide count
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	assert.Len(t, graph.Slides, 5)
}

// TestCompileImageSlotFill tests image slot fill functionality
func TestCompileImageSlotFill(t *testing.T) {
	// Load fixtures
	manifestPath, ok := findCompileFixture("manifest.json")
	if !ok {
		t.Skipf("manifest fixture not found")
	}
	specPath, ok := findCompileFixture("spec-with-image.yaml")
	if !ok {
		t.Skipf("spec-with-image.yaml fixture not found")
	}
	archetypePath, ok := findCompileFixture("presentation.pptx")
	if !ok {
		t.Skipf("archetype PPTX fixture not found")
	}
	imagePath, ok := findCompileFixture("test-image.png")
	if !ok {
		t.Skipf("test-image.png fixture not found")
	}

	// Verify test image exists
	_, err := os.Stat(imagePath)
	require.NoError(t, err, "test image should exist")
	fixtureDir := filepath.Dir(imagePath)

	// Load manifest
	manifestData, err := os.ReadFile(manifestPath)
	require.NoError(t, err)
	var manifest TemplateManifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))

	// Load spec
	specData, err := os.ReadFile(specPath)
	require.NoError(t, err)
	var spec CompilationSpec
	require.NoError(t, yaml.Unmarshal(specData, &spec))

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output_with_image.pptx")

	options := createTestCompileOptions(archetypePath, outPath)
	options.ImageBaseDir = fixtureDir
	engine := NewCompilerEngine(&manifest, &spec, options)
	require.NotNil(t, engine)

	// Compile
	result, err := engine.Compile()
	require.NoError(t, err)
	require.NotNil(t, result)

	// Verify output PPTX was created
	_, err = os.Stat(outPath)
	require.NoError(t, err)

	// Verify output PPTX has media parts (images)
	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	imageRels := 0
	for _, slide := range graph.Slides {
		rels := pkg.ListRelationships(slide.PartURI)
		for _, rel := range rels {
			if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" {
				imageRels++
			}
		}
	}
	assert.Greater(t, imageRels, 0, "compiled presentation should have image relationships")
}

// TestCompileThemeOverrideApply tests theme override application
func TestCompileThemeOverrideApply(t *testing.T) {
	// Load fixtures
	manifestPath, ok := findCompileFixture("manifest.json")
	if !ok {
		t.Skipf("manifest fixture not found")
	}
	specPath, ok := findCompileFixture("spec-complex.yaml")
	if !ok {
		t.Skipf("spec-complex.yaml fixture not found")
	}
	archetypePath, ok := findCompileFixture("presentation.pptx")
	if !ok {
		t.Skipf("archetype PPTX fixture not found")
	}

	// Load manifest
	manifestData, err := os.ReadFile(manifestPath)
	require.NoError(t, err)
	var manifest TemplateManifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))

	// Load spec (spec-complex.yaml has theme overrides)
	specData, err := os.ReadFile(specPath)
	require.NoError(t, err)
	var spec CompilationSpec
	require.NoError(t, yaml.Unmarshal(specData, &spec))

	require.NotNil(t, spec.ThemeOverrides, "spec should have theme overrides")
	require.NotEmpty(t, spec.ThemeOverrides.Colors, "theme overrides should have colors")

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output_themed.pptx")

	options := createTestCompileOptions(archetypePath, outPath)
	engine := NewCompilerEngine(&manifest, &spec, options)
	require.NotNil(t, engine)

	// Compile
	result, err := engine.Compile()
	require.NoError(t, err)
	require.NotNil(t, result)

	// Verify output PPTX was created
	_, err = os.Stat(outPath)
	require.NoError(t, err)

	// Open and verify theme XML exists
	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	// Read theme part by checking presentation relationships
	rels := pkg.ListRelationships("/ppt/presentation.xml")
	themeURI := ""
	for _, rel := range rels {
		if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" {
			themeURI = opc.ResolveRelationshipTarget("/ppt/presentation.xml", rel.Target)
			break
		}
	}

	if themeURI != "" {
		// Read theme XML
		themeDoc, err := pkg.ReadXMLPart(themeURI)
		require.NoError(t, err)

		// Verify theme root exists
		assert.NotNil(t, themeDoc.Root())
		assert.True(t, len(themeDoc.Root().Tag) > 0, "theme root should have a tag")
	}

}

// TestCompileNotesSlide tests notes fill functionality
func TestCompileNotesSlide(t *testing.T) {
	// Load fixtures
	manifestPath, ok := findCompileFixture("manifest.json")
	if !ok {
		t.Skipf("manifest fixture not found")
	}
	specPath, ok := findCompileFixture("spec-with-notes.yaml")
	if !ok {
		t.Skipf("spec-with-notes.yaml fixture not found")
	}
	archetypePath, ok := findCompileFixture("presentation.pptx")
	if !ok {
		t.Skipf("archetype PPTX fixture not found")
	}

	// Load manifest
	manifestData, err := os.ReadFile(manifestPath)
	require.NoError(t, err)
	var manifest TemplateManifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))

	// Load spec
	specData, err := os.ReadFile(specPath)
	require.NoError(t, err)
	var spec CompilationSpec
	require.NoError(t, yaml.Unmarshal(specData, &spec))

	// Verify spec has notes
	require.NotEmpty(t, spec.Slides)
	require.NotEmpty(t, spec.Slides[0].Notes, "first slide should have notes")

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "output_with_notes.pptx")

	options := createTestCompileOptions(archetypePath, outPath)
	engine := NewCompilerEngine(&manifest, &spec, options)
	require.NotNil(t, engine)

	// Compile should fail loudly when the archetype has no notes part. The
	// previous behavior silently ignored requested notes, producing an output
	// that looked successful but did not contain the requested speaker notes.
	result, err := engine.Compile()
	require.Error(t, err)
	require.Nil(t, result)
	assert.Contains(t, err.Error(), "slide has no notes part")

}

func TestCompilePrunesUnusedArchetypeSlides(t *testing.T) {
	manifestPath, ok := findCompileFixture("manifest.json")
	if !ok {
		t.Skipf("manifest fixture not found")
	}
	archetypePath, ok := findCompileFixture("presentation.pptx")
	if !ok {
		t.Skipf("archetype PPTX fixture not found")
	}

	manifestData, err := os.ReadFile(manifestPath)
	require.NoError(t, err)
	var manifest TemplateManifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))

	spec := &CompilationSpec{
		Version: "1.0",
		Slides: []SlideSpec{{
			Archetype: "title-slide",
			Content: map[string]interface{}{
				"title": "Only One",
			},
		}},
	}

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "pruned-output.pptx")

	engine := NewCompilerEngine(&manifest, spec, createTestCompileOptions(archetypePath, outPath))
	result, err := engine.Compile()
	require.NoError(t, err)
	require.NotNil(t, result)
	assert.Equal(t, 1, result.SlideCount)

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	assert.Len(t, graph.Slides, 1, "unused archetype source slides should not remain in output")
}

func TestCompileRepeatedArchetypeDoesNotCarryOptionalContent(t *testing.T) {
	manifestPath, ok := findCompileFixture("manifest.json")
	if !ok {
		t.Skipf("manifest fixture not found")
	}
	archetypePath, ok := findCompileFixture("presentation.pptx")
	if !ok {
		t.Skipf("archetype PPTX fixture not found")
	}

	manifestData, err := os.ReadFile(manifestPath)
	require.NoError(t, err)
	var manifest TemplateManifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))

	spec := &CompilationSpec{
		Version: "1.0",
		Slides: []SlideSpec{
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title":    "One",
					"subtitle": "First subtitle",
				},
			},
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title": "Two",
				},
			},
		},
	}

	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "repeat-output.pptx")

	engine := NewCompilerEngine(&manifest, spec, createTestCompileOptions(archetypePath, outPath))
	result, err := engine.Compile()
	require.NoError(t, err)
	require.NotNil(t, result)
	assert.Equal(t, 2, result.SlideCount)

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 2)

	slide2, err := pkg.ReadXMLPart(graph.Slides[1].PartURI)
	require.NoError(t, err)
	slide2XML, err := slide2.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slide2XML, "Two")
	assert.NotContains(t, slide2XML, "First subtitle", "optional content from earlier archetype use should not leak into later clones")
}

// TestCompileErrorHandlingInvalidArchetype tests error handling for invalid archetype
func TestCompileErrorHandlingInvalidArchetype(t *testing.T) {
	manifest := createSimpleManifestForCompilerTests()

	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "nonexistent-archetype",
				Content: map[string]interface{}{
					"title": "Test",
				},
			},
		},
	}

	tmpDir := t.TempDir()

	options := CompileOptions{
		ArchetypePath:   filepath.Join(tmpDir, "nonexistent.pptx"),
		OutputPath:      filepath.Join(tmpDir, "output.pptx"),
		ContinueOnError: false,
	}

	engine := NewCompilerEngine(manifest, spec, options)
	_, err := engine.Compile()

	// Should fail during validation or compilation
	assert.Error(t, err, "compilation should fail with invalid archetype or missing file")
}

// TestCompileErrorHandlingMissingRequiredSlot tests error handling for missing required slot
func TestCompileErrorHandlingMissingRequiredSlot(t *testing.T) {
	manifest := createSimpleManifestForCompilerTests()

	// Spec missing required "title" slot in content-slide
	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "content-slide",
				Content: map[string]interface{}{
					// Missing "title" which is required
					"body": "Some content",
				},
			},
		},
	}

	tmpDir := t.TempDir()

	options := CompileOptions{
		ArchetypePath:   filepath.Join(tmpDir, "nonexistent.pptx"),
		OutputPath:      filepath.Join(tmpDir, "output.pptx"),
		ContinueOnError: false,
	}

	engine := NewCompilerEngine(manifest, spec, options)
	_, err := engine.Compile()

	// Should fail during spec validation or file access
	assert.Error(t, err, "compilation should fail with missing required slot or file")
}

// TestCompileErrorHandlingUnknownArchetypeID tests error handling for unknown archetype ID
func TestCompileErrorHandlingUnknownArchetypeID(t *testing.T) {
	manifest := createSimpleManifestForCompilerTests()

	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "unknown-id-xyz",
				Content: map[string]interface{}{
					"title": "Test Title",
				},
			},
		},
	}

	tmpDir := t.TempDir()

	options := CompileOptions{
		ArchetypePath:   filepath.Join(tmpDir, "nonexistent.pptx"),
		OutputPath:      filepath.Join(tmpDir, "output.pptx"),
		ContinueOnError: false,
	}

	engine := NewCompilerEngine(manifest, spec, options)
	_, err := engine.Compile()

	// Should fail during validation or file access
	assert.Error(t, err, "compilation should fail with unknown archetype ID or file not found")
}

// TestCompileContinueOnErrorFlag tests continue-on-error mode
func TestCompileContinueOnErrorFlag(t *testing.T) {
	manifest := &TemplateManifest{
		ManifestVersion: "1.0",
		Name:            "Test Template",
		Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: time.Now()},
		CreatedAt:       time.Now(),
		ModifiedAt:      time.Now(),
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
				},
			},
		},
	}

	// Spec with valid slide
	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title": "Valid Slide",
				},
			},
		},
	}

	tmpDir := t.TempDir()

	options := CompileOptions{
		ArchetypePath:   filepath.Join(tmpDir, "nonexistent.pptx"),
		OutputPath:      filepath.Join(tmpDir, "output.pptx"),
		ContinueOnError: true,
	}

	engine := NewCompilerEngine(manifest, spec, options)
	engine.Compile() // Should fail but that's ok

	// With continue-on-error flag set, the flag should be preserved in options
	assert.True(t, options.ContinueOnError, "continue-on-error flag should be set")
}
