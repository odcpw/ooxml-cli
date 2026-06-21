package cli

import (
	"encoding/json"
	"os"
	"testing"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/translate"
)

// TestApplyResultStructure verifies the ApplyResult structure
func TestApplyResultStructure(t *testing.T) {
	result := ApplyResult{
		EntriesProcessed: 10,
		EntriesApplied:   8,
		EntriesSkipped:   2,
		Warnings:         []string{"warning 1", "warning 2"},
	}

	// Verify it can be marshaled to JSON
	jsonData, err := json.Marshal(result)
	if err != nil {
		t.Errorf("failed to marshal ApplyResult: %v", err)
	}

	// Verify it can be unmarshaled back
	var unmarshaled ApplyResult
	if err := json.Unmarshal(jsonData, &unmarshaled); err != nil {
		t.Errorf("failed to unmarshal ApplyResult: %v", err)
	}

	if unmarshaled.EntriesProcessed != result.EntriesProcessed {
		t.Errorf("EntriesProcessed mismatch: %d != %d", unmarshaled.EntriesProcessed, result.EntriesProcessed)
	}

	if unmarshaled.EntriesApplied != result.EntriesApplied {
		t.Errorf("EntriesApplied mismatch: %d != %d", unmarshaled.EntriesApplied, result.EntriesApplied)
	}

	if unmarshaled.EntriesSkipped != result.EntriesSkipped {
		t.Errorf("EntriesSkipped mismatch: %d != %d", unmarshaled.EntriesSkipped, result.EntriesSkipped)
	}

	if len(unmarshaled.Warnings) != len(result.Warnings) {
		t.Errorf("Warnings length mismatch: %d != %d", len(unmarshaled.Warnings), len(result.Warnings))
	}
}

// TestApplyResultMarshaling verifies JSON marshaling of ApplyResult
func TestApplyResultMarshaling(t *testing.T) {
	result := ApplyResult{
		EntriesProcessed: 5,
		EntriesApplied:   3,
		EntriesSkipped:   2,
		Warnings:         []string{},
	}

	// Marshal with indent
	jsonData, err := json.MarshalIndent(result, "", "  ")
	if err != nil {
		t.Errorf("failed to marshal with indent: %v", err)
	}

	if len(jsonData) == 0 {
		t.Error("marshal produced empty JSON")
	}

	// Verify it contains expected fields
	var m map[string]interface{}
	if err := json.Unmarshal(jsonData, &m); err != nil {
		t.Errorf("failed to unmarshal JSON: %v", err)
	}

	expectedKeys := []string{"entriesProcessed", "entriesApplied", "entriesSkipped"}
	for _, key := range expectedKeys {
		if _, ok := m[key]; !ok {
			t.Errorf("missing expected key in JSON: %s", key)
		}
	}
}

// TestTranslateManifestRoundtrip verifies that a manifest can be saved and loaded
func TestTranslateManifestRoundtrip(t *testing.T) {
	// Create a test manifest
	manifest := &translate.TranslationManifest{
		Metadata: &translate.ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     time.Now().UTC(),
			SourceLanguage: "en-US",
			TargetLanguage: "de-DE",
			DeckName:       "test.pptx",
			SlideCount:     2,
			EntryCount:     3,
		},
		Entries: []translate.TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Hello",
				TargetText:     "Hallo",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
			},
		},
	}

	// Marshal to JSON
	jsonData, err := json.Marshal(manifest)
	if err != nil {
		t.Errorf("failed to marshal manifest: %v", err)
	}

	// Write to temp file
	tmpFile, err := os.CreateTemp("", "manifest-*.json")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	defer os.Remove(tmpFile.Name())

	if _, err := tmpFile.Write(jsonData); err != nil {
		t.Errorf("failed to write manifest: %v", err)
	}
	tmpFile.Close()

	// Read back
	readData, err := os.ReadFile(tmpFile.Name())
	if err != nil {
		t.Errorf("failed to read manifest: %v", err)
	}

	// Unmarshal
	var readManifest translate.TranslationManifest
	if err := json.Unmarshal(readData, &readManifest); err != nil {
		t.Errorf("failed to unmarshal manifest: %v", err)
	}

	// Verify content
	if readManifest.Metadata.Version != manifest.Metadata.Version {
		t.Errorf("version mismatch: %q != %q", readManifest.Metadata.Version, manifest.Metadata.Version)
	}

	if len(readManifest.Entries) != len(manifest.Entries) {
		t.Errorf("entry count mismatch: %d != %d", len(readManifest.Entries), len(manifest.Entries))
	}

	if readManifest.Entries[0].ID != manifest.Entries[0].ID {
		t.Errorf("entry ID mismatch: %q != %q", readManifest.Entries[0].ID, manifest.Entries[0].ID)
	}
}
