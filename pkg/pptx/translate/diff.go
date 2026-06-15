package translate

import (
	"fmt"
	"sort"
)

// DiffChange represents a single change between two manifests
type DiffChange struct {
	// Type of change: "added", "removed", or "changed"
	Type string

	// The entry (for added and removed) or old entry (for changed)
	Entry *TranslationEntry

	// The new entry (only for changed, nil for added/removed)
	NewEntry *TranslationEntry

	// Reason for change (for changed entries, e.g., "sourceText", "targetText")
	Reason string

	// Optional: what changed (for display purposes)
	Details string
}

// ManifestDiff represents the differences between two manifests
type ManifestDiff struct {
	// Metadata changes
	MetadataChanges []string

	// Added entries (not in first manifest)
	Added []DiffChange

	// Removed entries (not in second manifest)
	Removed []DiffChange

	// Changed entries (in both but with differences)
	Changed []DiffChange

	// Summary: total number of changes
	ChangeCount int
}

// HasChanges returns true if there are any differences
func (d *ManifestDiff) HasChanges() bool {
	return len(d.MetadataChanges) > 0 || len(d.Added) > 0 || len(d.Removed) > 0 || len(d.Changed) > 0
}

// DiffManifests compares two translation manifests and returns a detailed diff.
// The diff reports added, removed, and changed entries in a deterministic order.
//
// Parameters:
//   - first: The original manifest
//   - second: The new manifest
//
// Returns: A ManifestDiff with all changes identified
func DiffManifests(first, second *TranslationManifest) *ManifestDiff {
	diff := &ManifestDiff{
		MetadataChanges: []string{},
		Added:           []DiffChange{},
		Removed:         []DiffChange{},
		Changed:         []DiffChange{},
	}

	// Handle nil manifests
	if first == nil || second == nil {
		if first == nil && second == nil {
			return diff
		}
		if first == nil {
			diff.MetadataChanges = append(diff.MetadataChanges, "first manifest is nil")
		}
		if second == nil {
			diff.MetadataChanges = append(diff.MetadataChanges, "second manifest is nil")
		}
		return diff
	}

	// Compare metadata
	diffMetadata(first.Metadata, second.Metadata, diff)

	// Build maps for efficient lookup
	firstEntries := make(map[string]*TranslationEntry)
	secondEntries := make(map[string]*TranslationEntry)

	for i := range first.Entries {
		firstEntries[first.Entries[i].ID] = &first.Entries[i]
	}

	for i := range second.Entries {
		secondEntries[second.Entries[i].ID] = &second.Entries[i]
	}

	// Find all unique IDs across both manifests
	allIDs := make(map[string]bool)
	for id := range firstEntries {
		allIDs[id] = true
	}
	for id := range secondEntries {
		allIDs[id] = true
	}

	// Convert to sorted slice for deterministic ordering
	var sortedIDs []string
	for id := range allIDs {
		sortedIDs = append(sortedIDs, id)
	}
	sort.Strings(sortedIDs)

	// Process each ID
	for _, id := range sortedIDs {
		firstEntry := firstEntries[id]
		secondEntry := secondEntries[id]

		switch {
		case firstEntry != nil && secondEntry == nil:
			// Entry was removed
			diff.Removed = append(diff.Removed, DiffChange{
				Type:  "removed",
				Entry: firstEntry,
			})
		case firstEntry == nil && secondEntry != nil:
			// Entry was added
			diff.Added = append(diff.Added, DiffChange{
				Type:  "added",
				Entry: secondEntry,
			})
		case firstEntry != nil && secondEntry != nil:
			// Both exist, check for differences
			if !entriesEqual(firstEntry, secondEntry) {
				changes := identifyEntryChanges(firstEntry, secondEntry)
				if len(changes) > 0 {
					// Combine all changes into a single entry
					detailsStr := ""
					for i, change := range changes {
						if i > 0 {
							detailsStr += "; "
						}
						detailsStr += change
					}
					diff.Changed = append(diff.Changed, DiffChange{
						Type:     "changed",
						Entry:    firstEntry,
						NewEntry: secondEntry,
						Details:  detailsStr,
						Reason:   "multiple fields changed",
					})
				}
			}
		}
	}

	// Calculate total change count
	diff.ChangeCount = len(diff.MetadataChanges) + len(diff.Added) + len(diff.Removed) + len(diff.Changed)

	return diff
}

// diffMetadata compares manifest metadata
func diffMetadata(first, second *ManifestMetadata, diff *ManifestDiff) {
	if first == nil && second == nil {
		return
	}

	if first == nil {
		diff.MetadataChanges = append(diff.MetadataChanges, "first metadata is nil")
		return
	}

	if second == nil {
		diff.MetadataChanges = append(diff.MetadataChanges, "second metadata is nil")
		return
	}

	// Compare each field
	if first.Version != second.Version {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("version changed: %s → %s", first.Version, second.Version))
	}

	if first.SourceLanguage != second.SourceLanguage {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("source language changed: %s → %s", first.SourceLanguage, second.SourceLanguage))
	}

	if first.TargetLanguage != second.TargetLanguage {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("target language changed: %s → %s", first.TargetLanguage, second.TargetLanguage))
	}

	if first.DeckName != second.DeckName {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("deck name changed: %s → %s", first.DeckName, second.DeckName))
	}

	if first.SlideCount != second.SlideCount {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("slide count changed: %d → %d", first.SlideCount, second.SlideCount))
	}

	if first.EntryCount != second.EntryCount {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("entry count changed: %d → %d", first.EntryCount, second.EntryCount))
	}

	if first.Notes != second.Notes {
		diff.MetadataChanges = append(diff.MetadataChanges,
			fmt.Sprintf("notes changed: %q → %q", first.Notes, second.Notes))
	}
}

// entriesEqual checks if two translation entries are identical
func entriesEqual(first, second *TranslationEntry) bool {
	if first == nil && second == nil {
		return true
	}
	if first == nil || second == nil {
		return false
	}

	return first.ID == second.ID &&
		first.Type == second.Type &&
		first.SourceText == second.SourceText &&
		first.TargetText == second.TargetText &&
		first.SlideID == second.SlideID &&
		first.SlideName == second.SlideName &&
		first.SlideNumber == second.SlideNumber &&
		first.PlaceholderKey == second.PlaceholderKey &&
		first.ShapeID == second.ShapeID &&
		first.ShapeName == second.ShapeName &&
		first.ParagraphIndex == second.ParagraphIndex &&
		first.RunIndex == second.RunIndex &&
		first.SegmentType == second.SegmentType &&
		first.ContextHash == second.ContextHash &&
		first.Notes == second.Notes &&
		first.IsTranslated == second.IsTranslated &&
		first.IsStale == second.IsStale &&
		bulletMetadataEqual(first.BulletInfo, second.BulletInfo) &&
		runFormattingEqual(first.RunFormat, second.RunFormat)
}

// bulletMetadataEqual checks if two bullet metadata structures are identical
func bulletMetadataEqual(first, second *BulletMetadata) bool {
	if first == nil && second == nil {
		return true
	}
	if first == nil || second == nil {
		return false
	}

	return pointersEqual(first.Level, second.Level) &&
		first.BulletMode == second.BulletMode &&
		first.BulletCharacter == second.BulletCharacter &&
		first.AutoNumberingScheme == second.AutoNumberingScheme &&
		first.BulletFontFamily == second.BulletFontFamily &&
		pointersEqual(first.BulletFontSize, second.BulletFontSize) &&
		first.BulletColor == second.BulletColor
}

// runFormattingEqual checks if two run formatting structures are identical
func runFormattingEqual(first, second *RunFormatting) bool {
	if first == nil && second == nil {
		return true
	}
	if first == nil || second == nil {
		return false
	}

	return first.FontFamily == second.FontFamily &&
		pointersEqual(first.FontSize, second.FontSize) &&
		pointersEqual(first.Bold, second.Bold) &&
		pointersEqual(first.Italic, second.Italic) &&
		first.Underline == second.Underline &&
		first.Strike == second.Strike &&
		first.Color == second.Color &&
		first.ThemeColor == second.ThemeColor &&
		first.Language == second.Language
}

// pointersEqual is a helper to compare pointers of various types
func pointersEqual[T comparable](first, second *T) bool {
	if first == nil && second == nil {
		return true
	}
	if first == nil || second == nil {
		return false
	}
	return *first == *second
}

// identifyEntryChanges identifies specific fields that changed in an entry
func identifyEntryChanges(first, second *TranslationEntry) []string {
	var changes []string

	if first.SourceText != second.SourceText {
		changes = append(changes, fmt.Sprintf("sourceText: %q → %q", first.SourceText, second.SourceText))
	}

	if first.TargetText != second.TargetText {
		changes = append(changes, fmt.Sprintf("targetText: %q → %q", first.TargetText, second.TargetText))
	}

	if first.Type != second.Type {
		changes = append(changes, fmt.Sprintf("type: %s → %s", first.Type, second.Type))
	}

	if first.SlideID != second.SlideID {
		changes = append(changes, fmt.Sprintf("slideId: %d → %d", first.SlideID, second.SlideID))
	}

	if first.SlideName != second.SlideName {
		changes = append(changes, fmt.Sprintf("slideName: %q → %q", first.SlideName, second.SlideName))
	}

	if first.SlideNumber != second.SlideNumber {
		changes = append(changes, fmt.Sprintf("slideNumber: %d → %d", first.SlideNumber, second.SlideNumber))
	}

	if first.PlaceholderKey != second.PlaceholderKey {
		changes = append(changes, fmt.Sprintf("placeholderKey: %q → %q", first.PlaceholderKey, second.PlaceholderKey))
	}

	if first.ShapeID != second.ShapeID {
		changes = append(changes, fmt.Sprintf("shapeId: %d → %d", first.ShapeID, second.ShapeID))
	}

	if first.ShapeName != second.ShapeName {
		changes = append(changes, fmt.Sprintf("shapeName: %q → %q", first.ShapeName, second.ShapeName))
	}

	if first.ParagraphIndex != second.ParagraphIndex {
		changes = append(changes, fmt.Sprintf("paragraphIndex: %d → %d", first.ParagraphIndex, second.ParagraphIndex))
	}

	if first.RunIndex != second.RunIndex {
		changes = append(changes, fmt.Sprintf("runIndex: %d → %d", first.RunIndex, second.RunIndex))
	}

	if first.SegmentType != second.SegmentType {
		changes = append(changes, fmt.Sprintf("segmentType: %q → %q", first.SegmentType, second.SegmentType))
	}

	if first.ContextHash != second.ContextHash {
		changes = append(changes, fmt.Sprintf("contextHash: %q → %q", first.ContextHash, second.ContextHash))
	}

	if first.Notes != second.Notes {
		changes = append(changes, fmt.Sprintf("notes: %q → %q", first.Notes, second.Notes))
	}

	if first.IsTranslated != second.IsTranslated {
		changes = append(changes, fmt.Sprintf("isTranslated: %v → %v", first.IsTranslated, second.IsTranslated))
	}

	if first.IsStale != second.IsStale {
		changes = append(changes, fmt.Sprintf("isStale: %v → %v", first.IsStale, second.IsStale))
	}

	if !bulletMetadataEqual(first.BulletInfo, second.BulletInfo) {
		changes = append(changes, "bulletInfo changed")
	}

	if !runFormattingEqual(first.RunFormat, second.RunFormat) {
		changes = append(changes, "runFormat changed")
	}

	return changes
}
