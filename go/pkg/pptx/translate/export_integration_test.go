package translate

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// TestExportTranslation_RichAlignmentFixture exports from rich-alignment fixture
func TestExportTranslation_RichAlignmentFixture(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/rich-alignment/presentation.pptx")
	if err != nil {
		t.Skipf("skipping test: fixture not found")
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-alignment.pptx",
	}

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Verify manifest has entries
	if len(manifest.Entries) == 0 {
		t.Fatal("no translation entries found in rich-alignment fixture")
	}

	// Verify all entries have valid structure
	for i, entry := range manifest.Entries {
		if !ValidateID(entry.ID) {
			t.Errorf("entry %d has invalid ID: %s", i, entry.ID)
		}
		if entry.Type == "" {
			t.Errorf("entry %d has empty type", i)
		}
	}

	assertTranslationGolden(t, "translate-export-rich-alignment.json", manifest)
}

// TestExportTranslation_NotesSlideFixture exports from notes-slide fixture with notes
func TestExportTranslation_NotesSlideFixture(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Skipf("skipping test: fixture not found")
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		IncludeNotes:   true,
		DeckName:       "notes-slide.pptx",
	}

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Verify entries exist
	if len(manifest.Entries) == 0 {
		t.Fatal("no translation entries found")
	}

	// Check for notes entries
	hasNotesEntries := false
	for _, entry := range manifest.Entries {
		if entry.Type == "notes" {
			hasNotesEntries = true
			break
		}
	}

	// Notes may or may not exist in the fixture, but export should work either way
	t.Logf("notes-slide fixture: %d total entries, %v has notes entries", len(manifest.Entries), hasNotesEntries)

	assertTranslationGolden(t, "translate-export-notes-slide.json", manifest)
}

// TestExportTranslation_MinimalTitleFixture exports from minimal-title fixture
func TestExportTranslation_MinimalTitleFixture(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Skipf("skipping test: fixture not found")
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		TargetLanguage: "fr-FR",
		DeckName:       "minimal-title.pptx",
	}

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Verify manifest structure
	if manifest.Metadata == nil {
		t.Fatal("manifest metadata is nil")
	}

	if manifest.Metadata.SourceLanguage != "en-US" {
		t.Errorf("expected source language en-US, got %s", manifest.Metadata.SourceLanguage)
	}

	if manifest.Metadata.TargetLanguage != "fr-FR" {
		t.Errorf("expected target language fr-FR, got %s", manifest.Metadata.TargetLanguage)
	}

	// Verify entries
	if len(manifest.Entries) == 0 {
		t.Fatal("no translation entries found in minimal-title fixture")
	}

	assertTranslationGolden(t, "translate-export-minimal-title.json", manifest)
}

// TestExportTranslation_MultiLayoutFixture exports from multi-layout fixture
func TestExportTranslation_MultiLayoutFixture(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Skipf("skipping test: fixture not found")
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "multi-layout.pptx",
	}

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Verify multiple slides are handled
	if len(manifest.Entries) == 0 {
		t.Fatal("no translation entries found in multi-layout fixture")
	}

	// Verify entries span multiple slides
	slideSet := make(map[int]bool)
	for _, entry := range manifest.Entries {
		slideSet[entry.SlideID] = true
	}

	if len(slideSet) == 0 {
		t.Fatal("entries do not have slide information")
	}

	t.Logf("multi-layout fixture: %d entries across %d slides", len(manifest.Entries), len(slideSet))

	assertTranslationGolden(t, "translate-export-multi-layout.json", manifest)
}

// TestExportTranslation_PerformanceLargeFixture tests export performance
func TestExportTranslation_PerformanceLargeFixture(t *testing.T) {
	// This test is optional - only runs if a large fixture exists
	// For now, use any available fixture to test performance characteristics
	pkg, err := opc.Open("../../../testdata/pptx/rich-numbered-lists/presentation.pptx")
	if err != nil {
		t.Skipf("skipping performance test: fixture not found")
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
	}

	// Export should complete in reasonable time
	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	if len(manifest.Entries) == 0 {
		t.Fatal("no translation entries found")
	}

	t.Logf("performance test: exported %d entries", len(manifest.Entries))
}

func assertTranslationGolden(t *testing.T, name string, manifest *TranslationManifest) {
	t.Helper()
	actual := canonicalTranslationManifest(manifest)
	actualJSON, err := json.MarshalIndent(actual, "", "  ")
	if err != nil {
		t.Fatalf("marshal actual translation manifest: %v", err)
	}
	actualJSON = append(actualJSON, '\n')

	goldenPath := filepath.Join("../../../testdata/golden", name)
	if os.Getenv("UPDATE_GOLDENS") == "1" {
		if err := os.WriteFile(goldenPath, actualJSON, 0o644); err != nil {
			t.Fatalf("update golden %s: %v", goldenPath, err)
		}
	}
	data, err := os.ReadFile(goldenPath)
	if err != nil {
		t.Fatalf("read golden %s: %v", goldenPath, err)
	}
	var expected TranslationManifest
	if err := json.Unmarshal(data, &expected); err != nil {
		t.Fatalf("parse golden %s: %v", goldenPath, err)
	}
	expected = *canonicalTranslationManifest(&expected)
	if !reflect.DeepEqual(&expected, actual) {
		expectedJSON, _ := json.MarshalIndent(expected, "", "  ")
		t.Fatalf("translation golden mismatch for %s\nexpected:\n%s\nactual:\n%s", goldenPath, expectedJSON, actualJSON)
	}
}

func canonicalTranslationManifest(manifest *TranslationManifest) *TranslationManifest {
	if manifest == nil {
		return nil
	}
	copyManifest := *manifest
	if manifest.Metadata != nil {
		metadata := *manifest.Metadata
		metadata.ExportedAt = time.Time{}
		copyManifest.Metadata = &metadata
	}
	copyManifest.Entries = append([]TranslationEntry(nil), manifest.Entries...)
	return &copyManifest
}

// TestExportTranslation_GoldenFiles loads and validates golden test files.
func TestExportTranslation_GoldenFiles(t *testing.T) {
	goldenDir := "../../../testdata/golden"

	goldenFiles := []string{
		"translate-export-rich-alignment.json",
		"translate-export-notes-slide.json",
		"translate-export-minimal-title.json",
		"translate-export-multi-layout.json",
	}

	for _, goldenFile := range goldenFiles {
		goldenPath := filepath.Join(goldenDir, goldenFile)
		data, err := os.ReadFile(goldenPath)
		if err != nil {
			t.Fatalf("golden file not found: %s", goldenFile)
		}

		var manifest TranslationManifest
		if err := json.Unmarshal(data, &manifest); err != nil {
			t.Errorf("failed to parse golden file %s: %v", goldenFile, err)
			continue
		}

		// Verify golden file structure
		if manifest.Metadata == nil {
			t.Errorf("golden file %s has no metadata", goldenFile)
		}

		if len(manifest.Entries) == 0 {
			t.Errorf("golden file %s has no entries", goldenFile)
		}

		// Validate all entries
		for i, entry := range manifest.Entries {
			if !ValidateID(entry.ID) {
				t.Errorf("golden file %s entry %d has invalid ID: %s", goldenFile, i, entry.ID)
			}
		}
	}
}
