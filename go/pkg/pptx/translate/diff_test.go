package translate

import (
	"sort"
	"testing"
	"time"
)

// TestDiffManifestsEmpty verifies diff of two empty manifests
func TestDiffManifestsEmpty(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	diff := DiffManifests(first, second)

	if diff.HasChanges() {
		t.Errorf("empty manifests should have no changes, got: %v", diff)
	}
	if diff.ChangeCount != 0 {
		t.Errorf("change count should be 0, got %d", diff.ChangeCount)
	}
}

// TestDiffManifestsNil verifies diff with nil manifests
func TestDiffManifestsNil(t *testing.T) {
	diff := DiffManifests(nil, nil)

	if diff.HasChanges() {
		t.Errorf("nil manifests should have no changes")
	}

	diff = DiffManifests(nil, NewManifest())
	if !diff.HasChanges() {
		t.Errorf("nil first manifest should be detected as change")
	}

	diff = DiffManifests(NewManifest(), nil)
	if !diff.HasChanges() {
		t.Errorf("nil second manifest should be detected as change")
	}
}

// TestDiffManifestsAddedEntries verifies detection of added entries
func TestDiffManifestsAddedEntries(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	// Add entries only to second manifest
	entry1 := NewEntry("slide:0_title_p0_r0", "title", "Title", 0, 1, 0, 0)
	entry2 := NewEntry("slide:0_body_p0_r0", "body", "Body text", 0, 1, 0, 0)

	second.Entries = append(second.Entries, entry1, entry2)

	diff := DiffManifests(first, second)

	if len(diff.Added) != 2 {
		t.Errorf("expected 2 added entries, got %d", len(diff.Added))
	}

	if diff.ChangeCount != 2 {
		t.Errorf("expected change count 2, got %d", diff.ChangeCount)
	}

	// Verify entries are in deterministic order (sorted by ID)
	expectedOrder := []string{"slide:0_body_p0_r0", "slide:0_title_p0_r0"}
	for i, change := range diff.Added {
		if change.Entry.ID != expectedOrder[i] {
			t.Errorf("entry %d: expected ID %s, got %s", i, expectedOrder[i], change.Entry.ID)
		}
	}
}

// TestDiffManifestsRemovedEntries verifies detection of removed entries
func TestDiffManifestsRemovedEntries(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	// Add entries only to first manifest
	entry1 := NewEntry("slide:0_title_p0_r0", "title", "Title", 0, 1, 0, 0)
	entry2 := NewEntry("slide:1_body_p0_r0", "body", "Body text", 1, 2, 0, 0)

	first.Entries = append(first.Entries, entry1, entry2)

	diff := DiffManifests(first, second)

	if len(diff.Removed) != 2 {
		t.Errorf("expected 2 removed entries, got %d", len(diff.Removed))
	}

	if diff.ChangeCount != 2 {
		t.Errorf("expected change count 2, got %d", diff.ChangeCount)
	}

	// Verify entries are in deterministic order
	if diff.Removed[0].Entry.ID != "slide:0_title_p0_r0" {
		t.Errorf("first removed entry should be slide:0_title_p0_r0, got %s", diff.Removed[0].Entry.ID)
	}
	if diff.Removed[1].Entry.ID != "slide:1_body_p0_r0" {
		t.Errorf("second removed entry should be slide:1_body_p0_r0, got %s", diff.Removed[1].Entry.ID)
	}
}

// TestDiffManifestsChangedEntries verifies detection of changed entries
func TestDiffManifestsChangedEntries(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	// Same ID, different content
	entry1First := NewEntry("slide:0_title_p0_r0", "title", "Original Title", 0, 1, 0, 0)
	entry1Second := NewEntry("slide:0_title_p0_r0", "title", "Changed Title", 0, 1, 0, 0)
	entry1Second.TargetText = "Translated Title"

	first.Entries = append(first.Entries, entry1First)
	second.Entries = append(second.Entries, entry1Second)

	diff := DiffManifests(first, second)

	if len(diff.Changed) != 1 {
		t.Errorf("expected 1 changed entry, got %d", len(diff.Changed))
	}

	if diff.ChangeCount != 1 {
		t.Errorf("expected change count 1, got %d", diff.ChangeCount)
	}

	// Verify change details
	change := diff.Changed[0]
	if change.Type != "changed" {
		t.Errorf("change type should be 'changed', got %s", change.Type)
	}

	if change.Entry.SourceText != "Original Title" {
		t.Errorf("old entry should have original text")
	}

	if change.NewEntry.SourceText != "Changed Title" {
		t.Errorf("new entry should have changed text")
	}

	// Should mention both source and target text changes
	if !contains(change.Details, "sourceText") || !contains(change.Details, "targetText") {
		t.Errorf("details should mention both sourceText and targetText changes: %s", change.Details)
	}
}

// TestDiffManifestsDeterministicOrdering verifies that diff results are sorted
func TestDiffManifestsDeterministicOrdering(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	// Add entries in non-alphabetical order
	entries := []TranslationEntry{
		NewEntry("slide:2_title_p0_r0", "title", "Slide 2 Title", 2, 3, 0, 0),
		NewEntry("slide:0_title_p0_r0", "title", "Slide 0 Title", 0, 1, 0, 0),
		NewEntry("slide:1_title_p0_r0", "title", "Slide 1 Title", 1, 2, 0, 0),
	}

	second.Entries = entries

	diff := DiffManifests(first, second)

	// Verify entries are sorted
	expectedOrder := []string{
		"slide:0_title_p0_r0",
		"slide:1_title_p0_r0",
		"slide:2_title_p0_r0",
	}

	for i, change := range diff.Added {
		if change.Entry.ID != expectedOrder[i] {
			t.Errorf("added entry %d: expected %s, got %s", i, expectedOrder[i], change.Entry.ID)
		}
	}
}

// TestDiffManifestsMixedChanges verifies handling of multiple change types
func TestDiffManifestsMixedChanges(t *testing.T) {
	first := NewManifest()
	first.Metadata.Version = "1.0.0"
	first.Metadata.SlideCount = 2
	first.Entries = []TranslationEntry{
		NewEntry("slide:0_title_p0_r0", "title", "Title 1", 0, 1, 0, 0),
		NewEntry("slide:1_body_p0_r0", "body", "Body 1", 1, 2, 0, 0),
		NewEntry("slide:1_body_p1_r0", "body", "Body 2", 1, 2, 1, 0),
	}

	second := NewManifest()
	second.Metadata.Version = "1.1.0"
	second.Metadata.SlideCount = 3
	second.Entries = []TranslationEntry{
		NewEntry("slide:0_title_p0_r0", "title", "Title 1 Modified", 0, 1, 0, 0), // Changed
		NewEntry("slide:1_body_p0_r0", "body", "Body 1", 1, 2, 0, 0),             // Same
		// "slide:1_body_p1_r0" is removed
		NewEntry("slide:2_title_p0_r0", "title", "Title 3", 2, 3, 0, 0), // Added
	}

	diff := DiffManifests(first, second)

	if len(diff.MetadataChanges) != 2 {
		t.Errorf("expected 2 metadata changes, got %d", len(diff.MetadataChanges))
	}

	if len(diff.Changed) != 1 {
		t.Errorf("expected 1 changed entry, got %d", len(diff.Changed))
	}

	if len(diff.Removed) != 1 {
		t.Errorf("expected 1 removed entry, got %d", len(diff.Removed))
	}

	if len(diff.Added) != 1 {
		t.Errorf("expected 1 added entry, got %d", len(diff.Added))
	}

	expectedTotal := len(diff.MetadataChanges) + len(diff.Changed) + len(diff.Removed) + len(diff.Added)
	if diff.ChangeCount != expectedTotal {
		t.Errorf("change count should be %d, got %d", expectedTotal, diff.ChangeCount)
	}
}

// TestDiffManifestsMetadataChanges verifies detection of metadata changes
func TestDiffManifestsMetadataChanges(t *testing.T) {
	fixedTime := time.Date(2026, 3, 10, 12, 0, 0, 0, time.UTC)

	first := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     fixedTime,
			SourceLanguage: "en-US",
			TargetLanguage: "de-DE",
			DeckName:       "presentation.pptx",
			SlideCount:     5,
			EntryCount:     10,
			Notes:          "Original",
		},
		Entries: []TranslationEntry{},
	}

	second := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.1.0",
			ExportedAt:     fixedTime,
			SourceLanguage: "en-US",
			TargetLanguage: "fr-FR",
			DeckName:       "presentation-updated.pptx",
			SlideCount:     6,
			EntryCount:     12,
			Notes:          "Updated",
		},
		Entries: []TranslationEntry{},
	}

	diff := DiffManifests(first, second)

	expectedChanges := 6 // version, targetLanguage, deckName, slideCount, entryCount, notes
	if len(diff.MetadataChanges) != expectedChanges {
		t.Errorf("expected %d metadata changes, got %d", expectedChanges, len(diff.MetadataChanges))
	}

	if diff.ChangeCount != expectedChanges {
		t.Errorf("change count should be %d, got %d", expectedChanges, diff.ChangeCount)
	}
}

// TestDiffManifestsBulletMetadataChange verifies detection of bullet metadata changes
func TestDiffManifestsBulletMetadataChange(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	level1 := int32(1)
	level2 := int32(2)

	entry1 := NewEntry("slide:0_body_p0_r0", "body", "Text", 0, 1, 0, 0)
	entry1.BulletInfo = &BulletMetadata{
		Level:       &level1,
		BulletMode:  "buChar",
		BulletColor: "FF0000",
	}

	entry2 := NewEntry("slide:0_body_p0_r0", "body", "Text", 0, 1, 0, 0)
	entry2.BulletInfo = &BulletMetadata{
		Level:       &level2,
		BulletMode:  "buChar",
		BulletColor: "00FF00",
	}

	first.Entries = append(first.Entries, entry1)
	second.Entries = append(second.Entries, entry2)

	diff := DiffManifests(first, second)

	if len(diff.Changed) != 1 {
		t.Errorf("expected 1 changed entry, got %d", len(diff.Changed))
	}

	change := diff.Changed[0]
	if !contains(change.Details, "bulletInfo") {
		t.Errorf("details should mention bulletInfo change, got: %s", change.Details)
	}
}

// TestDiffManifestsRunFormattingChange verifies detection of run formatting changes
func TestDiffManifestsRunFormattingChange(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	fontSize1 := 12.0
	fontSize2 := 14.0
	bold := true

	entry1 := NewEntry("slide:0_title_p0_r0", "title", "Title", 0, 1, 0, 0)
	entry1.RunFormat = &RunFormatting{
		FontFamily: "Arial",
		FontSize:   &fontSize1,
		Bold:       &bold,
	}

	entry2 := NewEntry("slide:0_title_p0_r0", "title", "Title", 0, 1, 0, 0)
	entry2.RunFormat = &RunFormatting{
		FontFamily: "Arial",
		FontSize:   &fontSize2,
		Bold:       &bold,
	}

	first.Entries = append(first.Entries, entry1)
	second.Entries = append(second.Entries, entry2)

	diff := DiffManifests(first, second)

	if len(diff.Changed) != 1 {
		t.Errorf("expected 1 changed entry, got %d", len(diff.Changed))
	}

	change := diff.Changed[0]
	if !contains(change.Details, "runFormat") {
		t.Errorf("details should mention runFormat change, got: %s", change.Details)
	}
}

// TestDiffManifestsTargetTextChange verifies detection of translation changes
func TestDiffManifestsTargetTextChange(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	entry1 := NewEntry("slide:0_title_p0_r0", "title", "Hello", 0, 1, 0, 0)
	entry1.TargetText = ""
	entry1.IsTranslated = false

	entry2 := NewEntry("slide:0_title_p0_r0", "title", "Hello", 0, 1, 0, 0)
	entry2.TargetText = "Hallo"
	entry2.IsTranslated = true

	first.Entries = append(first.Entries, entry1)
	second.Entries = append(second.Entries, entry2)

	diff := DiffManifests(first, second)

	if len(diff.Changed) != 1 {
		t.Errorf("expected 1 changed entry, got %d", len(diff.Changed))
	}

	// Should report both targetText and isTranslated changes
	change := diff.Changed[0]
	detailsStr := change.Details
	if !contains(detailsStr, "targetText") || !contains(detailsStr, "isTranslated") {
		t.Errorf("details should mention both targetText and isTranslated: %s", detailsStr)
	}

	// Verify both old and new entries
	if change.Entry.IsTranslated != false {
		t.Errorf("old entry should have IsTranslated=false")
	}
	if change.NewEntry.IsTranslated != true {
		t.Errorf("new entry should have IsTranslated=true")
	}
}

// TestDiffManifestsDriftScenario1 verifies drift detection: slides added between exports
func TestDiffManifestsDriftScenario1(t *testing.T) {
	// Original: 2 slides, 2 entries
	first := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			SlideCount: 2,
			EntryCount: 2,
		},
		Entries: []TranslationEntry{
			NewEntry("slide:0_title_p0_r0", "title", "Slide 1", 0, 1, 0, 0),
			NewEntry("slide:1_title_p0_r0", "title", "Slide 2", 1, 2, 0, 0),
		},
	}

	// Updated: 3 slides, 3 entries (new slide added)
	second := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			SlideCount: 3,
			EntryCount: 3,
		},
		Entries: []TranslationEntry{
			NewEntry("slide:0_title_p0_r0", "title", "Slide 1", 0, 1, 0, 0),
			NewEntry("slide:1_title_p0_r0", "title", "Slide 2", 1, 2, 0, 0),
			NewEntry("slide:2_title_p0_r0", "title", "Slide 3", 2, 3, 0, 0),
		},
	}

	diff := DiffManifests(first, second)

	// Should have 1 added entry (new slide)
	if len(diff.Added) != 1 {
		t.Errorf("expected 1 added entry, got %d", len(diff.Added))
	}

	// Should have metadata change (slideCount and entryCount)
	if len(diff.MetadataChanges) != 2 {
		t.Errorf("expected 2 metadata changes, got %d", len(diff.MetadataChanges))
	}
}

// TestDiffManifestsDriftScenario2 verifies drift detection: slide content modified
func TestDiffManifestsDriftScenario2(t *testing.T) {
	// Original export
	first := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			SlideCount: 1,
			EntryCount: 2,
		},
		Entries: []TranslationEntry{
			NewEntry("slide:0_title_p0_r0", "title", "Original Title", 0, 1, 0, 0),
			NewEntry("slide:0_body_p0_r0", "body", "Original Body", 0, 1, 0, 0),
		},
	}

	// Second export: title text changed (source modified)
	second := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			SlideCount: 1,
			EntryCount: 2,
		},
		Entries: []TranslationEntry{
			NewEntry("slide:0_title_p0_r0", "title", "Modified Title", 0, 1, 0, 0),
			NewEntry("slide:0_body_p0_r0", "body", "Original Body", 0, 1, 0, 0),
		},
	}

	diff := DiffManifests(first, second)

	// Should have 1 changed entry (title modified)
	if len(diff.Changed) != 1 {
		t.Errorf("expected 1 changed entry, got %d", len(diff.Changed))
	}

	change := diff.Changed[0]
	if !contains(change.Details, "sourceText") {
		t.Errorf("details should mention sourceText change: %s", change.Details)
	}

	if change.Entry.SourceText != "Original Title" {
		t.Errorf("old entry should have original text")
	}

	if change.NewEntry.SourceText != "Modified Title" {
		t.Errorf("new entry should have modified text")
	}
}

// TestDiffManifestsDriftScenario3 verifies drift detection: entries deleted
func TestDiffManifestsDriftScenario3(t *testing.T) {
	// Original: multiple entries
	first := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			SlideCount: 1,
			EntryCount: 3,
		},
		Entries: []TranslationEntry{
			NewEntry("slide:0_title_p0_r0", "title", "Title", 0, 1, 0, 0),
			NewEntry("slide:0_body_p0_r0", "body", "Body 1", 0, 1, 0, 0),
			NewEntry("slide:0_body_p1_r0", "body", "Body 2", 0, 1, 1, 0),
		},
	}

	// Updated: body paragraph deleted
	second := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			SlideCount: 1,
			EntryCount: 2,
		},
		Entries: []TranslationEntry{
			NewEntry("slide:0_title_p0_r0", "title", "Title", 0, 1, 0, 0),
			NewEntry("slide:0_body_p0_r0", "body", "Body 1", 0, 1, 0, 0),
		},
	}

	diff := DiffManifests(first, second)

	// Should have 1 removed entry
	if len(diff.Removed) != 1 {
		t.Errorf("expected 1 removed entry, got %d", len(diff.Removed))
	}

	if diff.Removed[0].Entry.ID != "slide:0_body_p1_r0" {
		t.Errorf("removed entry should be body p1, got %s", diff.Removed[0].Entry.ID)
	}

	// Should have metadata change (entryCount)
	if len(diff.MetadataChanges) != 1 {
		t.Errorf("expected 1 metadata change, got %d", len(diff.MetadataChanges))
	}
}

// Helper function to check if a string contains a substring
func contains(str, substr string) bool {
	for i := 0; i <= len(str)-len(substr); i++ {
		if str[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

// TestDiffManifestsLargeDrift verifies diff performance with many entries
func TestDiffManifestsLargeDrift(t *testing.T) {
	// Create manifests with many entries
	first := NewManifest()
	first.Metadata.SlideCount = 10
	first.Metadata.EntryCount = 100

	second := NewManifest()
	second.Metadata.SlideCount = 11
	second.Metadata.EntryCount = 105

	// Add entries to both manifests
	for i := 0; i < 100; i++ {
		slideID := i / 10
		entry := NewEntry(
			GenerateEntryID(slideID, "body", i%5, 0),
			"body",
			"Body text "+string(rune(48+i%10)),
			slideID,
			slideID+1,
			i%5,
			0,
		)
		first.Entries = append(first.Entries, entry)
		second.Entries = append(second.Entries, entry)
	}

	// Add some new entries to second manifest
	for i := 100; i < 105; i++ {
		slideID := 10
		entry := NewEntry(
			GenerateEntryID(slideID, "body", i%5, 0),
			"body",
			"New body text "+string(rune(48+i%10)),
			slideID,
			slideID+1,
			i%5,
			0,
		)
		second.Entries = append(second.Entries, entry)
	}

	// Perform diff
	diff := DiffManifests(first, second)

	// Should detect added entries
	if len(diff.Added) != 5 {
		t.Errorf("expected 5 added entries, got %d", len(diff.Added))
	}

	// Verify deterministic ordering of added entries
	var prevID string
	for _, change := range diff.Added {
		if prevID > change.Entry.ID {
			t.Errorf("added entries not in sorted order: %s after %s", change.Entry.ID, prevID)
		}
		prevID = change.Entry.ID
	}
}

// TestDiffManifestsDeterministicOrderingLarge verifies large diff is sorted
func TestDiffManifestsDeterministicOrderingLarge(t *testing.T) {
	first := NewManifest()
	second := NewManifest()

	// Add entries in pseudo-random order
	ids := []string{
		"slide:5_body_p0_r0",
		"slide:0_title_p0_r0",
		"slide:3_body_p1_r2",
		"slide:1_title_p0_r0",
		"slide:2_body_p0_r1",
		"slide:10_body_p0_r0",
		"slide:9_title_p0_r0",
		"slide:4_body_p0_r0",
	}

	for _, id := range ids {
		entry := TranslationEntry{
			ID:          id,
			Type:        "body",
			SourceText:  "Text for " + id,
			SlideID:     0,
			SlideNumber: 1,
		}
		second.Entries = append(second.Entries, entry)
	}

	diff := DiffManifests(first, second)

	// Verify all entries are present
	if len(diff.Added) != len(ids) {
		t.Errorf("expected %d added entries, got %d", len(ids), len(diff.Added))
	}

	// Verify entries are sorted
	for i := 1; i < len(diff.Added); i++ {
		if diff.Added[i-1].Entry.ID > diff.Added[i].Entry.ID {
			t.Errorf("added entries not properly sorted at index %d: %s > %s",
				i, diff.Added[i-1].Entry.ID, diff.Added[i].Entry.ID)
		}
	}

	// Verify sort order matches Go's sort
	var expectedIDs []string
	for _, id := range ids {
		expectedIDs = append(expectedIDs, id)
	}
	sort.Strings(expectedIDs)

	for i, change := range diff.Added {
		if change.Entry.ID != expectedIDs[i] {
			t.Errorf("entry %d: expected %s, got %s", i, expectedIDs[i], change.Entry.ID)
		}
	}
}
