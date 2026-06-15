package translate

import (
	"encoding/json"
	"testing"
	"time"
)

// TestManifestJSONRoundtrip verifies that a manifest can be serialized to JSON
// and deserialized back without loss of information.
func TestManifestJSONRoundtrip(t *testing.T) {
	now := time.Now().UTC()

	original := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     now,
			SourceLanguage: "en-US",
			TargetLanguage: "de-DE",
			DeckName:       "presentation.pptx",
			SlideCount:     3,
			EntryCount:     5,
			Notes:          "Test manifest",
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Hello World",
				TargetText:     "Hallo Welt",
				SlideID:        0,
				SlideName:      "Title Slide",
				SlideNumber:    1,
				PlaceholderKey: "title",
				ShapeID:        1,
				ShapeName:      "Title 1",
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
				ContextHash:    "abc123",
				Notes:          "Main title",
				IsTranslated:   true,
				IsStale:        false,
			},
			{
				ID:             "slide:0_body:0_p0_r0",
				Type:           "body",
				SourceText:     "Content here",
				TargetText:     "",
				SlideID:        0,
				SlideNumber:    1,
				PlaceholderKey: "body:0",
				ShapeID:        2,
				ShapeName:      "Body 1",
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
				Notes:          "Body text",
				IsTranslated:   false,
				IsStale:        false,
			},
		},
	}

	// Serialize to JSON
	data, err := json.MarshalIndent(original, "", "  ")
	if err != nil {
		t.Fatalf("failed to marshal manifest: %v", err)
	}

	// Deserialize back
	deserialized := &TranslationManifest{}
	if err := json.Unmarshal(data, deserialized); err != nil {
		t.Fatalf("failed to unmarshal manifest: %v", err)
	}

	// Verify metadata
	if deserialized.Metadata.Version != original.Metadata.Version {
		t.Errorf("version mismatch: %s != %s",
			deserialized.Metadata.Version, original.Metadata.Version)
	}
	if deserialized.Metadata.SourceLanguage != original.Metadata.SourceLanguage {
		t.Errorf("source language mismatch: %s != %s",
			deserialized.Metadata.SourceLanguage, original.Metadata.SourceLanguage)
	}
	if deserialized.Metadata.EntryCount != original.Metadata.EntryCount {
		t.Errorf("entry count mismatch: %d != %d",
			deserialized.Metadata.EntryCount, original.Metadata.EntryCount)
	}

	// Verify entries
	if len(deserialized.Entries) != len(original.Entries) {
		t.Errorf("entry count mismatch: %d != %d",
			len(deserialized.Entries), len(original.Entries))
	}

	for i, entry := range deserialized.Entries {
		if entry.ID != original.Entries[i].ID {
			t.Errorf("entry %d ID mismatch: %s != %s",
				i, entry.ID, original.Entries[i].ID)
		}
		if entry.SourceText != original.Entries[i].SourceText {
			t.Errorf("entry %d source text mismatch: %s != %s",
				i, entry.SourceText, original.Entries[i].SourceText)
		}
		if entry.TargetText != original.Entries[i].TargetText {
			t.Errorf("entry %d target text mismatch: %s != %s",
				i, entry.TargetText, original.Entries[i].TargetText)
		}
		if entry.IsTranslated != original.Entries[i].IsTranslated {
			t.Errorf("entry %d IsTranslated mismatch: %v != %v",
				i, entry.IsTranslated, original.Entries[i].IsTranslated)
		}
	}
}

// TestNewManifest verifies that NewManifest creates a properly initialized manifest
func TestNewManifest(t *testing.T) {
	manifest := NewManifest()

	if manifest.Metadata == nil {
		t.Fatal("metadata should not be nil")
	}
	if manifest.Metadata.Version != ManifestVersion {
		t.Errorf("version mismatch: %s != %s",
			manifest.Metadata.Version, ManifestVersion)
	}
	if manifest.Metadata.ExportedAt.IsZero() {
		t.Error("exported time should not be zero")
	}
	if len(manifest.Entries) != 0 {
		t.Errorf("entries should be empty initially, got %d", len(manifest.Entries))
	}
}

// TestNewEntry verifies that NewEntry creates a properly initialized entry
func TestNewEntry(t *testing.T) {
	entry := NewEntry("slide:0_title_p0_r0", "title", "Hello", 0, 1, 0, 0)

	if entry.ID != "slide:0_title_p0_r0" {
		t.Errorf("ID mismatch: %s", entry.ID)
	}
	if entry.Type != "title" {
		t.Errorf("Type mismatch: %s", entry.Type)
	}
	if entry.SourceText != "Hello" {
		t.Errorf("SourceText mismatch: %s", entry.SourceText)
	}
	if entry.SlideID != 0 {
		t.Errorf("SlideID mismatch: %d", entry.SlideID)
	}
	if entry.SlideNumber != 1 {
		t.Errorf("SlideNumber mismatch: %d", entry.SlideNumber)
	}
	if entry.ParagraphIndex != 0 {
		t.Errorf("ParagraphIndex mismatch: %d", entry.ParagraphIndex)
	}
	if entry.RunIndex != 0 {
		t.Errorf("RunIndex mismatch: %d", entry.RunIndex)
	}
	if entry.SegmentType != "text" {
		t.Errorf("SegmentType mismatch: %s", entry.SegmentType)
	}
}

// TestManifestWithOptionalFields verifies handling of optional fields
func TestManifestWithOptionalFields(t *testing.T) {
	entry := TranslationEntry{
		ID:             "slide:0_title_p0_r0",
		Type:           "title",
		SourceText:     "Title",
		SlideID:        0,
		SlideNumber:    1,
		ParagraphIndex: 0,
		RunIndex:       0,
		SegmentType:    "text",
		BulletInfo:     nil,   // Optional
		RunFormat:      nil,   // Optional
		ContextHash:    "",    // Optional
		Notes:          "",    // Optional
		IsTranslated:   false, // Optional
		IsStale:        false, // Optional
		PlaceholderKey: "",    // Optional
		ShapeID:        0,     // Optional
		ShapeName:      "",    // Optional
		SlideName:      "",    // Optional
	}

	// Should serialize without error
	data, err := json.Marshal(entry)
	if err != nil {
		t.Fatalf("failed to marshal entry: %v", err)
	}

	// Should deserialize without error
	var deserialized TranslationEntry
	if err := json.Unmarshal(data, &deserialized); err != nil {
		t.Fatalf("failed to unmarshal entry: %v", err)
	}

	if deserialized.ID != entry.ID {
		t.Errorf("ID mismatch: %s != %s", deserialized.ID, entry.ID)
	}
}

// TestBulletMetadata verifies bullet metadata structure and serialization
func TestBulletMetadata(t *testing.T) {
	level := int32(1)
	fontSize := int32(2400) // 24pt * 100

	bullet := &BulletMetadata{
		Level:               &level,
		BulletMode:          "buChar",
		BulletCharacter:     "•",
		BulletFontFamily:    "Wingdings",
		BulletFontSize:      &fontSize,
		BulletColor:         "FF0000",
		AutoNumberingScheme: "",
	}

	data, err := json.Marshal(bullet)
	if err != nil {
		t.Fatalf("failed to marshal bullet metadata: %v", err)
	}

	var deserialized BulletMetadata
	if err := json.Unmarshal(data, &deserialized); err != nil {
		t.Fatalf("failed to unmarshal bullet metadata: %v", err)
	}

	if deserialized.Level == nil || *deserialized.Level != level {
		t.Errorf("level mismatch")
	}
	if deserialized.BulletCharacter != "•" {
		t.Errorf("bullet character mismatch: %s", deserialized.BulletCharacter)
	}
}

// TestRunFormatting verifies run formatting structure and serialization
func TestRunFormatting(t *testing.T) {
	fontSize := 12.0
	bold := true

	format := &RunFormatting{
		FontFamily: "Arial",
		FontSize:   &fontSize,
		Bold:       &bold,
		Italic:     nil,
		Color:      "FF0000",
		Language:   "en-US",
	}

	data, err := json.Marshal(format)
	if err != nil {
		t.Fatalf("failed to marshal run formatting: %v", err)
	}

	var deserialized RunFormatting
	if err := json.Unmarshal(data, &deserialized); err != nil {
		t.Fatalf("failed to unmarshal run formatting: %v", err)
	}

	if deserialized.FontFamily != "Arial" {
		t.Errorf("font family mismatch: %s", deserialized.FontFamily)
	}
	if deserialized.FontSize == nil || *deserialized.FontSize != fontSize {
		t.Errorf("font size mismatch")
	}
	if deserialized.Bold == nil || !*deserialized.Bold {
		t.Errorf("bold mismatch")
	}
}

// TestManifestEmpty verifies an empty manifest is valid JSON
func TestManifestEmpty(t *testing.T) {
	manifest := NewManifest()
	manifest.Entries = []TranslationEntry{} // Empty list

	data, err := json.Marshal(manifest)
	if err != nil {
		t.Fatalf("failed to marshal empty manifest: %v", err)
	}

	var deserialized TranslationManifest
	if err := json.Unmarshal(data, &deserialized); err != nil {
		t.Fatalf("failed to unmarshal empty manifest: %v", err)
	}

	if len(deserialized.Entries) != 0 {
		t.Errorf("expected 0 entries, got %d", len(deserialized.Entries))
	}
}

// TestManifestStability verifies that serializing the same manifest multiple times produces identical JSON
func TestManifestStability(t *testing.T) {
	// Use fixed time for reproducibility
	fixedTime := time.Date(2026, 3, 10, 21, 0, 0, 0, time.UTC)

	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     fixedTime,
			SourceLanguage: "en-US",
			TargetLanguage: "de-DE",
			DeckName:       "test.pptx",
			SlideCount:     1,
			EntryCount:     1,
			Notes:          "Test",
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Test Title",
				TargetText:     "Test Titel",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
				IsTranslated:   true,
			},
		},
	}

	// Serialize twice
	data1, err := json.Marshal(manifest)
	if err != nil {
		t.Fatalf("first serialization failed: %v", err)
	}

	data2, err := json.Marshal(manifest)
	if err != nil {
		t.Fatalf("second serialization failed: %v", err)
	}

	// Results should be identical
	if string(data1) != string(data2) {
		t.Errorf("serialization not stable:\nfirst:  %s\nsecond: %s",
			string(data1), string(data2))
	}
}
