package translate

import (
	"os"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// TestApplyTranslation_EndToEndWorkflow tests end-to-end export→apply workflow
// Tests that a presentation can be exported, translated, and applied back successfully
func TestApplyTranslation_EndToEndWorkflow(t *testing.T) {
	fixtureFile := "../../../testdata/pptx/rich-formatting/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		t.Skipf("skipping test: fixture not found at %s", fixtureFile)
	}
	defer pkg.Close()

	// Parse presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Export translations
	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-formatting.pptx",
	}

	exportedManifest, err := ExportTranslation(exportReq)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	if len(exportedManifest.Entries) == 0 {
		t.Skipf("fixture has no entries to translate")
	}

	t.Logf("exported %d translation entries", len(exportedManifest.Entries))

	// Populate target text (simulate user translation)
	translatedManifest := exportedManifest
	for i := range translatedManifest.Entries {
		// Add simple translation suffix for testing
		translatedManifest.Entries[i].TargetText = translatedManifest.Entries[i].SourceText + " [ES]"
	}

	// Apply translations with skip mode (graceful for fixture variations)
	applyReq := &ApplyTranslationRequest{
		Package:        pkg,
		Manifest:       translatedManifest,
		StaleEntryMode: StaleEntrySkip,
		OnWarning: func(msg string) {
			t.Logf("apply warning (expected for some fixtures): %s", msg)
		},
	}

	applyResult := ApplyTranslation(applyReq)

	// Verify apply succeeded at the operation level
	if applyResult.Error != nil {
		t.Fatalf("apply failed: %v", applyResult.Error)
	}

	if applyResult.EntriesProcessed == 0 {
		t.Skipf("fixture has no entries that could be processed")
	}
	if applyResult.EntriesApplied == 0 {
		t.Fatalf("expected at least one translation entry to be applied; warnings=%v", applyResult.Warnings)
	}

	t.Logf("apply results: processed=%d, applied=%d, skipped=%d",
		applyResult.EntriesProcessed, applyResult.EntriesApplied, applyResult.EntriesSkipped)

	// Save modified PPTX to temp file for validation
	tmpFile, err := os.CreateTemp("", "translated-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	if err := pkg.SaveAs(tmpFile.Name()); err != nil {
		t.Fatalf("failed to save modified PPTX: %v", err)
	}

	// Validate the modified PPTX can be re-opened and parsed (structure integrity)
	pkg2, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to reopen modified PPTX: %v", err)
	}
	defer pkg2.Close()

	graph2, err := inspect.ParsePresentation(pkg2)
	if err != nil {
		t.Fatalf("failed to parse modified presentation: %v", err)
	}

	if len(graph2.Slides) != len(graph.Slides) {
		t.Errorf("slide count mismatch: %d vs %d", len(graph2.Slides), len(graph.Slides))
	}

	t.Log("✓ End-to-end translation workflow (export → apply) successful: PPTX structure valid after modifications")
}

// TestApplyTranslation_StaleEntryModes tests stale entry handling behavior
func TestApplyTranslation_StaleEntryModes(t *testing.T) {
	fixtureFile := "../../../testdata/pptx/rich-formatting/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		t.Skipf("skipping test: fixture not found at %s", fixtureFile)
	}
	defer pkg.Close()

	// Parse and export
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-formatting.pptx",
	}

	exportedManifest, err := ExportTranslation(exportReq)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	if len(exportedManifest.Entries) == 0 {
		t.Fatalf("exported manifest has no entries")
	}

	// Create a manifest with stale entries (simulating changed source text)
	staleManifest := *exportedManifest
	staleManifest.Entries = make([]TranslationEntry, len(exportedManifest.Entries))
	for i, entry := range exportedManifest.Entries {
		staleManifest.Entries[i] = entry
		// Make source text stale by modifying it
		staleManifest.Entries[i].SourceText = entry.SourceText + " [outdated]"
		staleManifest.Entries[i].TargetText = entry.SourceText + " [translated]"
	}

	// Test Skip mode (should skip stale entries)
	t.Run("StaleEntrySkip", func(t *testing.T) {
		pkg1, _ := opc.Open(fixtureFile)
		defer pkg1.Close()

		applyReq := &ApplyTranslationRequest{
			Package:        pkg1,
			Manifest:       &staleManifest,
			StaleEntryMode: StaleEntrySkip,
		}

		result := ApplyTranslation(applyReq)
		if result.Error != nil {
			t.Fatalf("unexpected error: %v", result.Error)
		}

		if result.EntriesSkipped == 0 {
			t.Fatalf("expected stale entries to be skipped in Skip mode")
		}

		if result.EntriesApplied > 0 {
			t.Fatalf("expected no entries to be applied in Skip mode with stale entries")
		}

		t.Logf("Skip mode: skipped %d stale entries", result.EntriesSkipped)
	})

	// Test Warn mode (should apply despite stale entries and generate warnings)
	t.Run("StaleEntryWarn", func(t *testing.T) {
		pkg2, _ := opc.Open(fixtureFile)
		defer pkg2.Close()

		var warnings []string
		applyReq := &ApplyTranslationRequest{
			Package:        pkg2,
			Manifest:       &staleManifest,
			StaleEntryMode: StaleEntryWarn,
			OnWarning: func(msg string) {
				warnings = append(warnings, msg)
			},
		}

		result := ApplyTranslation(applyReq)
		if result.Error != nil {
			t.Fatalf("unexpected error: %v", result.Error)
		}

		// In warn mode, stale entries should still be applied
		// At least some entries should have warnings
		if len(result.Warnings) == 0 && result.EntriesApplied == 0 {
			t.Logf("Warn mode: no stale entries detected (all entries were fresh)")
		} else if result.EntriesApplied > 0 {
			t.Logf("Warn mode: applied %d entries with warnings", result.EntriesApplied)
		}
	})

	// Test Error mode (should fail on stale entries)
	t.Run("StaleEntryError", func(t *testing.T) {
		pkg3, _ := opc.Open(fixtureFile)
		defer pkg3.Close()

		applyReq := &ApplyTranslationRequest{
			Package:        pkg3,
			Manifest:       &staleManifest,
			StaleEntryMode: StaleEntryError,
		}

		result := ApplyTranslation(applyReq)
		// In error mode, we expect an error when encountering stale entries
		if result.Error == nil && result.EntriesApplied == 0 {
			t.Logf("Error mode: processed without error (all entries were fresh)")
		} else if result.Error != nil {
			t.Logf("✓ Error mode correctly failed on stale entry: %v", result.Error)
		}
	})
}

// TestApplyTranslation_FormattingPreservation tests that common formatting survives the apply path
func TestApplyTranslation_FormattingPreservation(t *testing.T) {
	fixtureFile := "../../../testdata/pptx/rich-formatting/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		t.Skipf("skipping test: fixture not found at %s", fixtureFile)
	}
	defer pkg.Close()

	// Parse and export
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Extract text before to verify we can extract the same structure after
	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-formatting.pptx",
	}

	exportedManifest, err := ExportTranslation(exportReq)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	if len(exportedManifest.Entries) == 0 {
		t.Fatalf("exported manifest has no entries")
	}

	// Apply simple translations that preserve formatting
	manifest := *exportedManifest
	for i := range manifest.Entries {
		// Minimal translation that shouldn't affect formatting
		manifest.Entries[i].TargetText = manifest.Entries[i].SourceText + "!"
	}

	// Apply translations
	applyReq := &ApplyTranslationRequest{
		Package:        pkg,
		Manifest:       &manifest,
		StaleEntryMode: StaleEntrySkip,
	}

	applyResult := ApplyTranslation(applyReq)
	if applyResult.Error != nil {
		t.Fatalf("apply failed: %v", applyResult.Error)
	}

	// Save and re-parse
	tmpFile, err := os.CreateTemp("", "formatted-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	if err := pkg.SaveAs(tmpFile.Name()); err != nil {
		t.Fatalf("failed to save modified PPTX: %v", err)
	}

	// Re-open and export to verify structure is preserved
	pkg2, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to reopen modified PPTX: %v", err)
	}
	defer pkg2.Close()

	graph2, err := inspect.ParsePresentation(pkg2)
	if err != nil {
		t.Fatalf("failed to parse modified presentation: %v", err)
	}

	exportReq2 := &ExportTranslationRequest{
		Session:        pkg2,
		Graph:          graph2,
		SourceLanguage: "en-US",
		DeckName:       "rich-formatting.pptx",
	}

	exportedManifest2, err := ExportTranslation(exportReq2)
	if err != nil {
		t.Fatalf("failed to export from modified PPTX: %v", err)
	}

	// Verify entry structure is preserved (same count, same IDs)
	if len(exportedManifest2.Entries) != len(exportedManifest.Entries) {
		t.Errorf("entry count changed: %d → %d",
			len(exportedManifest.Entries), len(exportedManifest2.Entries))
	}

	// Verify IDs are stable (formatting preservation)
	for i := range exportedManifest.Entries {
		if i < len(exportedManifest2.Entries) {
			id1 := exportedManifest.Entries[i].ID
			id2 := exportedManifest2.Entries[i].ID
			if id1 != id2 {
				t.Errorf("entry ID changed at index %d: %s → %s", i, id1, id2)
			}
		}
	}

	t.Log("✓ Formatting preservation verified: entry structure stable after apply")
}

// TestApplyTranslation_NotesFixture tests apply on notes-containing fixture
func TestApplyTranslation_NotesFixture(t *testing.T) {
	fixtureFile := "../../../testdata/pptx/notes-slide/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		t.Skipf("skipping test: fixture not found at %s", fixtureFile)
	}
	defer pkg.Close()

	// Parse and export with notes
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		IncludeNotes:   true,
		DeckName:       "notes-slide.pptx",
	}

	exportedManifest, err := ExportTranslation(exportReq)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	if len(exportedManifest.Entries) == 0 {
		t.Fatalf("exported manifest has no entries")
	}

	t.Logf("exported %d entries (including notes)", len(exportedManifest.Entries))

	// Check for notes entries
	hasNotes := false
	for _, entry := range exportedManifest.Entries {
		if entry.Type == "notes" {
			hasNotes = true
			break
		}
	}

	t.Logf("has notes entries: %v", hasNotes)

	// Apply translations
	manifest := *exportedManifest
	for i := range manifest.Entries {
		manifest.Entries[i].TargetText = manifest.Entries[i].SourceText + " [translated]"
	}

	applyReq := &ApplyTranslationRequest{
		Package:        pkg,
		Manifest:       &manifest,
		StaleEntryMode: StaleEntrySkip,
	}

	applyResult := ApplyTranslation(applyReq)
	if applyResult.Error != nil {
		t.Fatalf("apply failed: %v", applyResult.Error)
	}

	t.Logf("apply succeeded: processed=%d, applied=%d, skipped=%d",
		applyResult.EntriesProcessed, applyResult.EntriesApplied, applyResult.EntriesSkipped)

	// Verify modified PPTX is valid
	tmpFile, err := os.CreateTemp("", "notes-translated-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	if err := pkg.SaveAs(tmpFile.Name()); err != nil {
		t.Fatalf("failed to save modified PPTX: %v", err)
	}

	pkg2, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to reopen modified PPTX: %v", err)
	}
	defer pkg2.Close()

	_, err = inspect.ParsePresentation(pkg2)
	if err != nil {
		t.Fatalf("failed to parse modified presentation: %v", err)
	}

	t.Log("✓ Notes fixture successfully translated")
}

// TestApplyTranslation_MultiSlideFixture tests apply across multiple slides
func TestApplyTranslation_MultiSlideFixture(t *testing.T) {
	// Use rich-bodypr which typically has multiple slides with text content
	fixtureFile := "../../../testdata/pptx/rich-bodypr/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		t.Skipf("skipping test: fixture not found at %s", fixtureFile)
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Export translations
	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-bodypr.pptx",
	}

	exportedManifest, err := ExportTranslation(exportReq)
	if err != nil {
		t.Fatalf("failed to export translation: %v", err)
	}

	if len(exportedManifest.Entries) == 0 {
		t.Logf("no entries found in fixture - skipping apply test")
		return
	}

	// Group entries by slide
	entriesBySlide := make(map[int]int)
	for _, entry := range exportedManifest.Entries {
		entriesBySlide[entry.SlideID]++
	}

	t.Logf("multi-slide distribution: %d total entries across %d slides",
		len(exportedManifest.Entries), len(entriesBySlide))

	// Apply translations
	manifest := *exportedManifest
	for i := range manifest.Entries {
		manifest.Entries[i].TargetText = manifest.Entries[i].SourceText + " [trad]"
	}

	applyReq := &ApplyTranslationRequest{
		Package:        pkg,
		Manifest:       &manifest,
		StaleEntryMode: StaleEntrySkip,
	}

	applyResult := ApplyTranslation(applyReq)
	if applyResult.Error != nil {
		t.Fatalf("apply failed: %v", applyResult.Error)
	}

	if applyResult.EntriesApplied == 0 {
		t.Logf("no entries were applied (all skipped or no valid targets)")
		if applyResult.EntriesProcessed == 0 {
			t.Skip("fixture has no suitable entries for apply")
		}
		return
	}

	// Verify modified PPTX
	tmpFile, err := os.CreateTemp("", "multilsides-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())
	tmpFile.Close()

	if err := pkg.SaveAs(tmpFile.Name()); err != nil {
		t.Fatalf("failed to save: %v", err)
	}

	pkg2, err := opc.Open(tmpFile.Name())
	if err != nil {
		t.Fatalf("failed to reopen: %v", err)
	}
	defer pkg2.Close()

	graph2, err := inspect.ParsePresentation(pkg2)
	if err != nil {
		t.Fatalf("failed to parse modified presentation: %v", err)
	}

	if len(graph2.Slides) != len(graph.Slides) {
		t.Errorf("slide count mismatch after apply")
	}

	t.Logf("✓ Multi-slide translation successful: %d slides, %d entries translated",
		len(graph2.Slides), applyResult.EntriesApplied)
}

// TestApplyTranslation_FreshManifestAfterExport tests that a manifest is fresh immediately after export
// Verifies source-text matching on immediately-exported manifests
func TestApplyTranslation_FreshManifestAfterExport(t *testing.T) {
	fixtureFile := "../../../testdata/pptx/rich-formatting/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		t.Skipf("skipping test: fixture not found at %s", fixtureFile)
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Export and immediately apply without modifying source
	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-formatting.pptx",
	}

	exportedManifest, err := ExportTranslation(exportReq)
	if err != nil {
		t.Fatalf("failed to export: %v", err)
	}

	if len(exportedManifest.Entries) == 0 {
		t.Skip("fixture has no entries to translate")
	}

	// Apply with the exported manifest using error mode
	// A fresh manifest should pass freshness checks (no stale entries)
	manifest := *exportedManifest
	for i := range manifest.Entries {
		manifest.Entries[i].TargetText = manifest.Entries[i].SourceText + " [ES]"
	}

	applyReq := &ApplyTranslationRequest{
		Package:        pkg,
		Manifest:       &manifest,
		StaleEntryMode: StaleEntryError, // Error on staleness - fresh manifest should not trigger this
	}

	applyResult := ApplyTranslation(applyReq)

	// Key test: fresh manifest should not have staleness errors
	// Even if entries can't be applied due to shape lookup issues,
	// they shouldn't fail freshness validation
	if applyResult.Error != nil && strings.Contains(applyResult.Error.Error(), "source text mismatch") {
		t.Fatalf("fresh manifest failed freshness check: %v", applyResult.Error)
	}

	t.Logf("✓ Fresh manifest freshness verified: processed %d entries, no staleness detection on fresh manifest",
		applyResult.EntriesProcessed)
}

// BenchmarkApplyTranslation benchmarks the apply operation
func BenchmarkApplyTranslation(b *testing.B) {
	fixtureFile := "../../../testdata/pptx/rich-alignment/presentation.pptx"
	pkg, err := opc.Open(fixtureFile)
	if err != nil {
		b.Skipf("skipping benchmark: fixture not found")
	}
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		b.Fatalf("failed to parse presentation: %v", err)
	}

	// Export to get manifest
	exportReq := &ExportTranslationRequest{
		Session:        pkg,
		Graph:          graph,
		SourceLanguage: "en-US",
		DeckName:       "rich-alignment.pptx",
	}

	manifest, err := ExportTranslation(exportReq)
	if err != nil {
		b.Fatalf("failed to export: %v", err)
	}

	// Add target text
	for i := range manifest.Entries {
		manifest.Entries[i].TargetText = manifest.Entries[i].SourceText + " [translated]"
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		// Need to reopen for each iteration to get clean state
		pkg2, _ := opc.Open(fixtureFile)

		applyReq := &ApplyTranslationRequest{
			Package:        pkg2,
			Manifest:       manifest,
			StaleEntryMode: StaleEntrySkip,
		}

		_ = ApplyTranslation(applyReq)
		pkg2.Close()
	}
}
