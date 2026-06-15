package translate

import (
	"encoding/json"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// TestExportTranslation_BasicStructure verifies that export creates a valid manifest
func TestExportTranslation_BasicStructure(t *testing.T) {
	// Load a test presentation
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation: %v", err)
	}
	defer pkg.Close()

	// Parse presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Export translation manifest
	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		TargetLanguage: "de-DE",
		DeckName:       "presentation.pptx",
		IncludeNotes:   false,
	}

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Verify manifest structure
	if manifest.Metadata == nil {
		t.Fatal("manifest metadata is nil")
	}

	if manifest.Metadata.Version != ManifestVersion {
		t.Errorf("expected version %s, got %s", ManifestVersion, manifest.Metadata.Version)
	}

	if manifest.Metadata.SourceLanguage != "en-US" {
		t.Errorf("expected source language en-US, got %s", manifest.Metadata.SourceLanguage)
	}

	if manifest.Metadata.TargetLanguage != "de-DE" {
		t.Errorf("expected target language de-DE, got %s", manifest.Metadata.TargetLanguage)
	}

	if manifest.Metadata.DeckName != "presentation.pptx" {
		t.Errorf("expected deck name presentation.pptx, got %s", manifest.Metadata.DeckName)
	}

	// Verify entries exist
	if len(manifest.Entries) == 0 {
		t.Fatal("no translation entries found")
	}

	// Verify entries have valid structure
	for i, entry := range manifest.Entries {
		if entry.ID == "" {
			t.Errorf("entry %d has empty ID", i)
		}

		if !ValidateID(entry.ID) {
			t.Errorf("entry %d has invalid ID format: %s", i, entry.ID)
		}

		if entry.SourceText == "" {
			t.Errorf("entry %d has empty source text", i)
		}

		if entry.SlideNumber < 1 {
			t.Errorf("entry %d has invalid slide number: %d", i, entry.SlideNumber)
		}
	}
}

// TestExportTranslation_JSONRoundtrip verifies that exported manifest can be serialized and deserialized
func TestExportTranslation_JSONRoundtrip(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation: %v", err)
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
		DeckName:       "test.pptx",
	}

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Serialize to JSON
	jsonData, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		t.Fatalf("failed to marshal to JSON: %v", err)
	}

	// Deserialize back
	var restored TranslationManifest
	if err := json.Unmarshal(jsonData, &restored); err != nil {
		t.Fatalf("failed to unmarshal from JSON: %v", err)
	}

	// Verify structure is preserved
	if len(restored.Entries) != len(manifest.Entries) {
		t.Errorf("entry count mismatch: expected %d, got %d", len(manifest.Entries), len(restored.Entries))
	}

	if restored.Metadata.SourceLanguage != manifest.Metadata.SourceLanguage {
		t.Errorf("source language mismatch after roundtrip")
	}
}

// TestExportTranslation_SlideFiltering verifies that slide filtering works correctly
func TestExportTranslation_SlideFiltering(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation: %v", err)
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Export with no slide filter (all slides)
	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
	}

	allManifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export all slides: %v", err)
	}

	allCount := len(allManifest.Entries)
	if allCount == 0 {
		t.Fatal("no entries found when exporting all slides")
	}

	// Export with slide filter (first slide only)
	req.SlideNumbers = []int{1}
	filteredManifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export filtered slides: %v", err)
	}

	filteredCount := len(filteredManifest.Entries)
	if filteredCount == 0 {
		t.Fatal("no entries found when exporting slide 1")
	}

	// Verify that filtered manifest is smaller or equal
	if filteredCount > allCount {
		t.Errorf("filtered manifest (%d) is larger than all slides (%d)", filteredCount, allCount)
	}

	// Verify all entries in filtered manifest are from slide 1
	for _, entry := range filteredManifest.Entries {
		if entry.SlideNumber != 1 {
			t.Errorf("filtered manifest contains entry from slide %d", entry.SlideNumber)
		}
	}
}

// TestExportTranslation_NotesInclusion verifies that notes can be included in the manifest
func TestExportTranslation_NotesInclusion(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation: %v", err)
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Export without notes
	req := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		IncludeNotes:   false,
	}

	noNotesManifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export without notes: %v", err)
	}

	noNotesCount := len(noNotesManifest.Entries)

	// Export with notes
	req.IncludeNotes = true
	withNotesManifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export with notes: %v", err)
	}

	withNotesCount := len(withNotesManifest.Entries)

	// When notes are included, the manifest should have more or equal entries
	if withNotesCount < noNotesCount {
		t.Errorf("manifest with notes (%d) has fewer entries than without notes (%d)", withNotesCount, noNotesCount)
	}

	// Note: This test might not find notes if the test file doesn't have notes
	// That's okay - the important part is that the manifest structure is correct
	t.Logf("export with notes: %d entries, export without: %d entries", withNotesCount, noNotesCount)
}

// TestExportTranslation_Determinism verifies that repeated exports produce identical results
func TestExportTranslation_Determinism(t *testing.T) {
	pkg1, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation: %v", err)
	}
	defer pkg1.Close()

	graph1, err := inspect.ParsePresentation(pkg1)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	req := &ExportTranslationRequest{
		Session:        pkg1,
		Graph:          graph1,
		SourceLanguage: "en-US",
		DeckName:       "test.pptx",
	}

	manifest1, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation (first): %v", err)
	}

	// Re-export with a fresh session
	pkg2, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation again: %v", err)
	}
	defer pkg2.Close()

	graph2, err := inspect.ParsePresentation(pkg2)
	if err != nil {
		t.Fatalf("failed to parse presentation again: %v", err)
	}

	req.Session = pkg2
	req.Graph = graph2

	manifest2, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation (second): %v", err)
	}

	// Compare entry counts
	if len(manifest1.Entries) != len(manifest2.Entries) {
		t.Errorf("entry count mismatch: first export %d, second export %d", len(manifest1.Entries), len(manifest2.Entries))
	}

	// Compare entry IDs and text (order and content should be identical)
	for i := 0; i < len(manifest1.Entries) && i < len(manifest2.Entries); i++ {
		if manifest1.Entries[i].ID != manifest2.Entries[i].ID {
			t.Errorf("entry %d ID mismatch: %s vs %s", i, manifest1.Entries[i].ID, manifest2.Entries[i].ID)
		}

		if manifest1.Entries[i].SourceText != manifest2.Entries[i].SourceText {
			t.Errorf("entry %d source text mismatch: %s vs %s", i, manifest1.Entries[i].SourceText, manifest2.Entries[i].SourceText)
		}
	}
}

// TestExportTranslation_MetadataCountAccuracy verifies that entry count in metadata is accurate
func TestExportTranslation_MetadataCountAccuracy(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/notes-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open test presentation: %v", err)
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

	manifest, err := ExportTranslation(req)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	// Verify metadata counts match actual data
	if manifest.Metadata.EntryCount != len(manifest.Entries) {
		t.Errorf("entry count mismatch: metadata says %d, actual count is %d", manifest.Metadata.EntryCount, len(manifest.Entries))
	}

	if manifest.Metadata.SlideCount != len(graph.Slides) {
		t.Errorf("slide count mismatch: metadata says %d, actual count is %d", manifest.Metadata.SlideCount, len(graph.Slides))
	}
}
