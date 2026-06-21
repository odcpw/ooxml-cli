package translate

import (
	"fmt"
	"strings"
)

// ValidationError represents a validation failure with context
type ValidationError struct {
	// Error type (e.g., "invalid-id", "missing-field", "duplicate-id")
	Code string

	// Human-readable error message
	Message string

	// Entry ID if applicable (may be empty)
	EntryID string

	// Entry index in manifest if applicable (may be -1)
	EntryIndex int
}

func (e ValidationError) Error() string {
	if e.EntryID != "" {
		return fmt.Sprintf("[%s] entry %s: %s", e.Code, e.EntryID, e.Message)
	}
	if e.EntryIndex >= 0 {
		return fmt.Sprintf("[%s] entry %d: %s", e.Code, e.EntryIndex, e.Message)
	}
	return fmt.Sprintf("[%s] %s", e.Code, e.Message)
}

// ValidationResult contains the outcome of manifest validation
type ValidationResult struct {
	// True if manifest is valid
	Valid bool

	// List of validation errors
	Errors []ValidationError

	// List of warnings (non-fatal issues)
	Warnings []ValidationError
}

// IsValid returns true if the manifest has no errors
func (r *ValidationResult) IsValid() bool {
	return len(r.Errors) == 0
}

// HasWarnings returns true if there are any warnings
func (r *ValidationResult) HasWarnings() bool {
	return len(r.Warnings) > 0
}

// ValidateManifest performs comprehensive validation on a translation manifest.
// Returns a ValidationResult with any errors or warnings found.
//
// Checks performed:
//  1. Metadata validation (version, timestamps, language codes)
//  2. Entry validation (required fields, ID format)
//  3. Entry uniqueness (no duplicate IDs)
//  4. Entry consistency (consistent structure across entries)
func ValidateManifest(manifest *TranslationManifest) *ValidationResult {
	result := &ValidationResult{
		Errors:   []ValidationError{},
		Warnings: []ValidationError{},
	}

	if manifest == nil {
		result.Errors = append(result.Errors, ValidationError{
			Code:    "nil-manifest",
			Message: "manifest is nil",
		})
		result.Valid = false
		return result
	}

	// Validate metadata
	validateMetadata(manifest.Metadata, result)

	// Validate entries
	idSeen := make(map[string]int) // map from ID to entry index
	for i, entry := range manifest.Entries {
		validateEntry(&entry, i, result)

		// Check for duplicate IDs
		if prevIdx, exists := idSeen[entry.ID]; exists {
			result.Errors = append(result.Errors, ValidationError{
				Code:       "duplicate-id",
				Message:    fmt.Sprintf("duplicate ID (first seen at entry %d)", prevIdx),
				EntryID:    entry.ID,
				EntryIndex: i,
			})
		}
		idSeen[entry.ID] = i
	}

	result.Valid = len(result.Errors) == 0
	return result
}

// validateMetadata checks manifest metadata for correctness
func validateMetadata(metadata *ManifestMetadata, result *ValidationResult) {
	if metadata == nil {
		result.Errors = append(result.Errors, ValidationError{
			Code:    "missing-metadata",
			Message: "metadata is nil",
		})
		return
	}

	// Check version
	if metadata.Version == "" {
		result.Errors = append(result.Errors, ValidationError{
			Code:    "missing-version",
			Message: "manifest version is empty",
		})
	} else if !isValidSemanticVersion(metadata.Version) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:    "invalid-version-format",
			Message: fmt.Sprintf("version does not follow semantic versioning: %s", metadata.Version),
		})
	}

	// Check timestamp
	if metadata.ExportedAt.IsZero() {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:    "missing-timestamp",
			Message: "export timestamp is zero",
		})
	}

	// Check language codes (basic validation)
	if metadata.SourceLanguage != "" && !isValidLanguageCode(metadata.SourceLanguage) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:    "invalid-source-language",
			Message: fmt.Sprintf("source language code looks invalid: %s", metadata.SourceLanguage),
		})
	}
	if metadata.TargetLanguage != "" && !isValidLanguageCode(metadata.TargetLanguage) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:    "invalid-target-language",
			Message: fmt.Sprintf("target language code looks invalid: %s", metadata.TargetLanguage),
		})
	}

	// Consistency checks
	if metadata.SlideCount < 0 {
		result.Errors = append(result.Errors, ValidationError{
			Code:    "negative-slide-count",
			Message: "slide count cannot be negative",
		})
	}
	if metadata.EntryCount < 0 {
		result.Errors = append(result.Errors, ValidationError{
			Code:    "negative-entry-count",
			Message: "entry count cannot be negative",
		})
	}
}

// validateEntry checks a single translation entry for correctness
func validateEntry(entry *TranslationEntry, index int, result *ValidationResult) {
	// Check required fields
	if entry.ID == "" {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "missing-id",
			Message:    "entry ID is empty",
			EntryIndex: index,
		})
		return // Can't proceed without ID
	}

	if !ValidateID(entry.ID) {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "invalid-id",
			Message:    "entry ID does not match expected format",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	// Check type
	if entry.Type == "" {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "missing-type",
			Message:    "entry type is empty",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	} else if !isValidEntryType(entry.Type) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "unknown-type",
			Message:    fmt.Sprintf("entry type not recognized: %s", entry.Type),
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	// Check source text (required)
	if entry.SourceText == "" {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "empty-source-text",
			Message:    "source text is empty",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	// Check slide ID consistency
	if entry.SlideID < 0 {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "negative-slide-id",
			Message:    "slide ID cannot be negative",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	if entry.SlideNumber <= 0 {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "invalid-slide-number",
			Message:    "slide number must be positive (1-based)",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	// Check text indices
	if entry.ParagraphIndex < 0 {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "negative-paragraph-index",
			Message:    "paragraph index cannot be negative",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	if entry.RunIndex < 0 {
		result.Errors = append(result.Errors, ValidationError{
			Code:       "negative-run-index",
			Message:    "run index cannot be negative",
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	// Check segment type
	if entry.SegmentType != "" && !isValidSegmentType(entry.SegmentType) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "unknown-segment-type",
			Message:    fmt.Sprintf("segment type not recognized: %s", entry.SegmentType),
			EntryID:    entry.ID,
			EntryIndex: index,
		})
	}

	// Validate optional metadata
	if entry.BulletInfo != nil {
		validateBulletMetadata(entry.BulletInfo, entry.ID, index, result)
	}
	if entry.RunFormat != nil {
		validateRunFormatting(entry.RunFormat, entry.ID, index, result)
	}
}

// validateBulletMetadata checks bullet metadata consistency
func validateBulletMetadata(bullet *BulletMetadata, entryID string, entryIndex int, result *ValidationResult) {
	if bullet.BulletMode != "" && !isValidBulletMode(bullet.BulletMode) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "unknown-bullet-mode",
			Message:    fmt.Sprintf("bullet mode not recognized: %s", bullet.BulletMode),
			EntryID:    entryID,
			EntryIndex: entryIndex,
		})
	}

	if bullet.Level != nil && *bullet.Level < 0 {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "negative-bullet-level",
			Message:    "bullet level cannot be negative",
			EntryID:    entryID,
			EntryIndex: entryIndex,
		})
	}
	if bullet.Level != nil && *bullet.Level > 8 {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "excessive-bullet-level",
			Message:    "bullet level exceeds maximum (8)",
			EntryID:    entryID,
			EntryIndex: entryIndex,
		})
	}
}

// validateRunFormatting checks run formatting consistency
func validateRunFormatting(format *RunFormatting, entryID string, entryIndex int, result *ValidationResult) {
	if format.FontSize != nil && *format.FontSize <= 0 {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "invalid-font-size",
			Message:    fmt.Sprintf("font size must be positive, got %.1f", *format.FontSize),
			EntryID:    entryID,
			EntryIndex: entryIndex,
		})
	}

	if format.Language != "" && !isValidLanguageCode(format.Language) {
		result.Warnings = append(result.Warnings, ValidationError{
			Code:       "invalid-language-code",
			Message:    fmt.Sprintf("language code looks invalid: %s", format.Language),
			EntryID:    entryID,
			EntryIndex: entryIndex,
		})
	}
}

// isValidSemanticVersion checks if a version string looks like semantic versioning
// Basic check: X.Y.Z where X, Y, Z are numeric
func isValidSemanticVersion(version string) bool {
	parts := strings.Split(version, ".")
	if len(parts) != 3 {
		return false
	}
	for _, part := range parts {
		if part == "" {
			return false
		}
		for _, c := range part {
			if c < '0' || c > '9' {
				return false
			}
		}
	}
	return true
}

// isValidLanguageCode checks if a language code looks reasonable
// Accepts en, en-US, de-DE, fr, etc.
func isValidLanguageCode(code string) bool {
	if len(code) < 2 {
		return false
	}
	parts := strings.Split(code, "-")
	for _, part := range parts {
		if len(part) < 2 {
			return false
		}
		for _, c := range part {
			if !((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')) {
				return false
			}
		}
	}
	return true
}

// isValidEntryType checks if an entry type is recognized
func isValidEntryType(entryType string) bool {
	validTypes := map[string]bool{
		"title":    true,
		"subtitle": true,
		"body":     true,
		"notes":    true,
		"footer":   true,
		"date":     true,
		"slidenum": true,
		"custom":   true,
	}
	return validTypes[entryType]
}

// isValidSegmentType checks if a segment type is recognized
func isValidSegmentType(segmentType string) bool {
	validTypes := map[string]bool{
		"text":  true,
		"break": true,
		"tab":   true,
		"field": true,
	}
	return validTypes[segmentType]
}

// isValidBulletMode checks if a bullet mode is recognized
func isValidBulletMode(mode string) bool {
	validModes := map[string]bool{
		"buNone":    true,
		"buChar":    true,
		"buAutoNum": true,
	}
	return validModes[mode]
}
