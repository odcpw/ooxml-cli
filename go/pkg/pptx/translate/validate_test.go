package translate

import (
	"testing"
	"time"
)

// TestValidateManifestValid verifies validation of a valid manifest
func TestValidateManifestValid(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     time.Now().UTC(),
			SourceLanguage: "en-US",
			TargetLanguage: "de-DE",
			DeckName:       "test.pptx",
			SlideCount:     2,
			EntryCount:     3,
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
			},
			{
				ID:             "slide:0_body_p0_r0",
				Type:           "body",
				SourceText:     "Body text",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
			},
			{
				ID:             "slide:1_title_p0_r0",
				Type:           "title",
				SourceText:     "Second slide",
				SlideID:        1,
				SlideNumber:    2,
				ParagraphIndex: 0,
				RunIndex:       0,
				SegmentType:    "text",
			},
		},
	}

	result := ValidateManifest(manifest)

	if !result.IsValid() {
		t.Errorf("valid manifest should pass validation, got errors: %v", result.Errors)
	}

	if result.HasWarnings() {
		t.Errorf("valid manifest should have no warnings, got: %v", result.Warnings)
	}
}

// TestValidateManifestNil verifies validation of nil manifest
func TestValidateManifestNil(t *testing.T) {
	result := ValidateManifest(nil)

	if result.IsValid() {
		t.Error("nil manifest should fail validation")
	}

	if len(result.Errors) == 0 {
		t.Error("nil manifest should produce errors")
	}

	// Should have nil-manifest error
	hasNilError := false
	for _, err := range result.Errors {
		if err.Code == "nil-manifest" {
			hasNilError = true
			break
		}
	}
	if !hasNilError {
		t.Error("nil manifest error not found")
	}
}

// TestValidateManifestNilMetadata verifies validation with nil metadata
func TestValidateManifestNilMetadata(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: nil,
		Entries:  []TranslationEntry{},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with nil metadata should fail validation")
	}

	hasMissingMetadata := false
	for _, err := range result.Errors {
		if err.Code == "missing-metadata" {
			hasMissingMetadata = true
			break
		}
	}
	if !hasMissingMetadata {
		t.Error("missing metadata error not found")
	}
}

// TestValidateManifestEmptyVersion verifies validation with missing version
func TestValidateManifestEmptyVersion(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with empty version should fail validation")
	}

	hasMissingVersion := false
	for _, err := range result.Errors {
		if err.Code == "missing-version" {
			hasMissingVersion = true
			break
		}
	}
	if !hasMissingVersion {
		t.Error("missing version error not found")
	}
}

// TestValidateManifestInvalidVersion verifies validation with invalid version format
func TestValidateManifestInvalidVersion(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "invalid",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{},
	}

	result := ValidateManifest(manifest)

	// Should produce a warning, not an error
	if len(result.Warnings) == 0 {
		t.Error("invalid version should produce a warning")
	}

	hasInvalidVersion := false
	for _, warn := range result.Warnings {
		if warn.Code == "invalid-version-format" {
			hasInvalidVersion = true
			break
		}
	}
	if !hasInvalidVersion {
		t.Error("invalid version format warning not found")
	}
}

// TestValidateManifestDuplicateIDs verifies detection of duplicate entry IDs
func TestValidateManifestDuplicateIDs(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title 1",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
			{
				ID:             "slide:0_title_p0_r0", // Duplicate ID
				Type:           "title",
				SourceText:     "Title 2",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with duplicate IDs should fail validation")
	}

	hasDuplicateError := false
	for _, err := range result.Errors {
		if err.Code == "duplicate-id" {
			hasDuplicateError = true
			break
		}
	}
	if !hasDuplicateError {
		t.Error("duplicate ID error not found")
	}
}

// TestValidateManifestMissingEntryID verifies detection of missing entry ID
func TestValidateManifestMissingEntryID(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with missing entry ID should fail validation")
	}

	hasMissingID := false
	for _, err := range result.Errors {
		if err.Code == "missing-id" {
			hasMissingID = true
			break
		}
	}
	if !hasMissingID {
		t.Error("missing ID error not found")
	}
}

// TestValidateManifestInvalidID verifies detection of invalid ID format
func TestValidateManifestInvalidID(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "invalid_format",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with invalid ID format should fail validation")
	}

	hasInvalidID := false
	for _, err := range result.Errors {
		if err.Code == "invalid-id" {
			hasInvalidID = true
			break
		}
	}
	if !hasInvalidID {
		t.Error("invalid ID error not found")
	}
}

// TestValidateManifestMissingType verifies detection of missing entry type
func TestValidateManifestMissingType(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with missing entry type should fail validation")
	}

	hasMissingType := false
	for _, err := range result.Errors {
		if err.Code == "missing-type" {
			hasMissingType = true
			break
		}
	}
	if !hasMissingType {
		t.Error("missing type error not found")
	}
}

// TestValidateManifestUnknownType verifies warning for unknown entry type
func TestValidateManifestUnknownType(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "unknown_type",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	// Should still be valid, but with warning
	if !result.IsValid() {
		t.Error("manifest with unknown type should still be valid")
	}

	if !result.HasWarnings() {
		t.Error("manifest with unknown type should have warnings")
	}

	hasUnknownType := false
	for _, warn := range result.Warnings {
		if warn.Code == "unknown-type" {
			hasUnknownType = true
			break
		}
	}
	if !hasUnknownType {
		t.Error("unknown type warning not found")
	}
}

// TestValidateManifestNegativeSlideID verifies detection of negative slide ID
func TestValidateManifestNegativeSlideID(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        -1,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with negative slide ID should fail validation")
	}

	hasNegativeSlideID := false
	for _, err := range result.Errors {
		if err.Code == "negative-slide-id" {
			hasNegativeSlideID = true
			break
		}
	}
	if !hasNegativeSlideID {
		t.Error("negative slide ID error not found")
	}
}

// TestValidateManifestInvalidSlideNumber verifies detection of invalid slide number
func TestValidateManifestInvalidSlideNumber(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    0, // Should be 1-based
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with invalid slide number should fail validation")
	}

	hasInvalidSlideNum := false
	for _, err := range result.Errors {
		if err.Code == "invalid-slide-number" {
			hasInvalidSlideNum = true
			break
		}
	}
	if !hasInvalidSlideNum {
		t.Error("invalid slide number error not found")
	}
}

// TestValidateManifestNegativeParagraphIndex verifies detection of negative paragraph index
func TestValidateManifestNegativeParagraphIndex(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: -1,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with negative paragraph index should fail validation")
	}

	hasNegativePara := false
	for _, err := range result.Errors {
		if err.Code == "negative-paragraph-index" {
			hasNegativePara = true
			break
		}
	}
	if !hasNegativePara {
		t.Error("negative paragraph index error not found")
	}
}

// TestValidateManifestNegativeRunIndex verifies detection of negative run index
func TestValidateManifestNegativeRunIndex(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       -1,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with negative run index should fail validation")
	}

	hasNegativeRun := false
	for _, err := range result.Errors {
		if err.Code == "negative-run-index" {
			hasNegativeRun = true
			break
		}
	}
	if !hasNegativeRun {
		t.Error("negative run index error not found")
	}
}

// TestValidateManifestEmptySourceText verifies warning for empty source text
func TestValidateManifestEmptySourceText(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "", // Empty
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if !result.HasWarnings() {
		t.Error("manifest with empty source text should have warnings")
	}

	hasEmptySource := false
	for _, warn := range result.Warnings {
		if warn.Code == "empty-source-text" {
			hasEmptySource = true
			break
		}
	}
	if !hasEmptySource {
		t.Error("empty source text warning not found")
	}
}

// TestValidateManifestInvalidLanguageCode verifies warning for invalid language code
func TestValidateManifestInvalidLanguageCode(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     time.Now(),
			SourceLanguage: "x", // Invalid: too short
		},
		Entries: []TranslationEntry{},
	}

	result := ValidateManifest(manifest)

	if !result.HasWarnings() {
		t.Error("manifest with invalid language code should have warnings")
	}

	hasInvalidLang := false
	for _, warn := range result.Warnings {
		if warn.Code == "invalid-source-language" {
			hasInvalidLang = true
			break
		}
	}
	if !hasInvalidLang {
		t.Error("invalid language code warning not found")
	}
}

// TestValidateManifestNegativeBulletLevel verifies warning for negative bullet level
func TestValidateManifestNegativeBulletLevel(t *testing.T) {
	level := int32(-1)
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_body_p0_r0",
				Type:           "body",
				SourceText:     "Text",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				BulletInfo: &BulletMetadata{
					Level: &level,
				},
			},
		},
	}

	result := ValidateManifest(manifest)

	if !result.HasWarnings() {
		t.Error("manifest with negative bullet level should have warnings")
	}

	hasNegLevel := false
	for _, warn := range result.Warnings {
		if warn.Code == "negative-bullet-level" {
			hasNegLevel = true
			break
		}
	}
	if !hasNegLevel {
		t.Error("negative bullet level warning not found")
	}
}

// TestValidateManifestExcessiveBulletLevel verifies warning for excessive bullet level
func TestValidateManifestExcessiveBulletLevel(t *testing.T) {
	level := int32(9) // Max is 8
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_body_p0_r0",
				Type:           "body",
				SourceText:     "Text",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				BulletInfo: &BulletMetadata{
					Level: &level,
				},
			},
		},
	}

	result := ValidateManifest(manifest)

	if !result.HasWarnings() {
		t.Error("manifest with excessive bullet level should have warnings")
	}

	hasExcessLevel := false
	for _, warn := range result.Warnings {
		if warn.Code == "excessive-bullet-level" {
			hasExcessLevel = true
			break
		}
	}
	if !hasExcessLevel {
		t.Error("excessive bullet level warning not found")
	}
}

// TestValidateManifestNegativeFontSize verifies warning for negative font size
func TestValidateManifestNegativeFontSize(t *testing.T) {
	fontSize := -12.0
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    "1.0.0",
			ExportedAt: time.Now(),
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "title",
				SourceText:     "Title",
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
				RunFormat: &RunFormatting{
					FontSize: &fontSize,
				},
			},
		},
	}

	result := ValidateManifest(manifest)

	if !result.HasWarnings() {
		t.Error("manifest with negative font size should have warnings")
	}

	hasInvalidSize := false
	for _, warn := range result.Warnings {
		if warn.Code == "invalid-font-size" {
			hasInvalidSize = true
			break
		}
	}
	if !hasInvalidSize {
		t.Error("invalid font size warning not found")
	}
}

// TestValidateManifestMultipleErrors verifies manifest with multiple errors
func TestValidateManifestMultipleErrors(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version: "", // Missing version
		},
		Entries: []TranslationEntry{
			{
				ID:             "invalid_id", // Invalid ID (wrong format)
				Type:           "title",
				SourceText:     "Title",
				SlideID:        -1, // Negative
				SlideNumber:    0,  // Invalid
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if result.IsValid() {
		t.Error("manifest with multiple errors should fail validation")
	}

	// Should have at least 5 errors:
	// - missing version
	// - invalid ID format
	// - negative slide ID
	// - invalid slide number
	if len(result.Errors) < 4 {
		t.Errorf("expected at least 4 errors, got %d: %v", len(result.Errors), result.Errors)
	}
}

// TestValidateManifestWarningsNotErrors verifies warnings don't fail validation
func TestValidateManifestWarningsNotErrors(t *testing.T) {
	manifest := &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:        "1.0.0",
			ExportedAt:     time.Now(),
			SourceLanguage: "x", // Invalid (warning only)
		},
		Entries: []TranslationEntry{
			{
				ID:             "slide:0_title_p0_r0",
				Type:           "unknown_type", // Unknown type (warning only)
				SourceText:     "",             // Empty (warning only)
				SlideID:        0,
				SlideNumber:    1,
				ParagraphIndex: 0,
				RunIndex:       0,
			},
		},
	}

	result := ValidateManifest(manifest)

	if !result.IsValid() {
		t.Error("manifest with only warnings should still be valid")
	}

	if len(result.Warnings) < 3 {
		t.Errorf("expected at least 3 warnings, got %d", len(result.Warnings))
	}
}
