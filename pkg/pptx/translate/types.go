package translate

import (
	"encoding/json"
	"time"
)

// ManifestVersion defines the version of the manifest schema
// Follows semantic versioning: MAJOR.MINOR.PATCH
// Used for compatibility checking and migration planning
const ManifestVersion = "1.0.0"

// ManifestMetadata contains deck-level information about a translation manifest
type ManifestMetadata struct {
	// Version of the manifest schema (1.0.0, 2.0.0, etc.)
	Version string `json:"version"`

	// Export timestamp (ISO 8601 format)
	ExportedAt time.Time `json:"exportedAt"`

	// Source language code (e.g., "en-US", "de-DE")
	SourceLanguage string `json:"sourceLanguage,omitempty"`

	// Target language code for translation (e.g., "fr-FR", "ja-JP")
	TargetLanguage string `json:"targetLanguage,omitempty"`

	// Deck file name or identifier
	DeckName string `json:"deckName,omitempty"`

	// Total number of slides in the deck
	SlideCount int `json:"slideCount,omitempty"`

	// Entry count (number of translatable text entries)
	EntryCount int `json:"entryCount,omitempty"`

	// Optional notes about the export (locale-specific, content type, etc.)
	Notes string `json:"notes,omitempty"`
}

// BulletMetadata contains bullet-level formatting information for a paragraph
type BulletMetadata struct {
	// Paragraph indent level (0-8)
	Level *int32 `json:"level,omitempty"`

	// Bullet mode (buNone, buChar, buAutoNum)
	BulletMode string `json:"bulletMode,omitempty"`

	// Bullet character if BulletMode is buChar (e.g., "•", "-", "*")
	BulletCharacter string `json:"bulletCharacter,omitempty"`

	// Auto-numbering scheme if BulletMode is buAutoNum (e.g., "stdAutoNum")
	AutoNumberingScheme string `json:"autoNumberingScheme,omitempty"`

	// Font family for bullet (e.g., "Wingdings")
	BulletFontFamily string `json:"bulletFontFamily,omitempty"`

	// Font size for bullet in points * 100
	BulletFontSize *int32 `json:"bulletFontSize,omitempty"`

	// Bullet color as RGB hex (e.g., "FF0000" for red)
	BulletColor string `json:"bulletColor,omitempty"`
}

// RunFormatting contains character-level formatting information
type RunFormatting struct {
	// Font family name
	FontFamily string `json:"fontFamily,omitempty"`

	// Font size in points
	FontSize *float64 `json:"fontSize,omitempty"`

	// Text decorations
	Bold      *bool  `json:"bold,omitempty"`
	Italic    *bool  `json:"italic,omitempty"`
	Underline string `json:"underline,omitempty"` // Line, double, heavy, dash, etc.
	Strike    string `json:"strike,omitempty"`    // Single, double

	// Color information
	Color      string `json:"color,omitempty"`      // RGB hex color
	ThemeColor string `json:"themeColor,omitempty"` // Theme color name

	// Language code (e.g., "en-US", "fr-FR")
	Language string `json:"language,omitempty"`
}

// TranslationEntry represents a single translatable text unit in the manifest
// Entries are stable across exports — same text at same location produces same ID
type TranslationEntry struct {
	// Stable, deterministic ID for this entry
	// Format: <slide-id>_<shape-key>_<para-idx>_<run-idx>
	// Examples: "slide:0_title_0_0", "slide:0_body:0_1_2"
	ID string `json:"id"`

	// Type of content (title, subtitle, body, notes, etc.)
	Type string `json:"type"`

	// Source text in the original language
	SourceText string `json:"sourceText"`

	// Target text for translation (may be empty or contain translated content)
	// If empty, this entry needs translation
	TargetText string `json:"targetText,omitempty"`

	// Slide-level context
	SlideID     int    `json:"slideId"`             // 0-based slide index
	SlideName   string `json:"slideName,omitempty"` // Optional slide name/title
	SlideNumber int    `json:"slideNumber"`         // 1-based slide number for display

	// Shape-level context
	PlaceholderKey string `json:"placeholderKey,omitempty"` // Placeholder key (e.g., "title", "body:0")
	ShapeID        int    `json:"shapeId,omitempty"`        // Internal shape ID
	ShapeName      string `json:"shapeName,omitempty"`      // Shape name from PPTX

	// Text location within the shape
	ParagraphIndex int `json:"paragraphIndex"` // 0-based paragraph index
	RunIndex       int `json:"runIndex"`       // 0-based run index within paragraph

	// Text segment type (text, break, tab, field)
	SegmentType string `json:"segmentType,omitempty"`

	// Bullet and formatting metadata
	BulletInfo *BulletMetadata `json:"bulletInfo,omitempty"`
	RunFormat  *RunFormatting  `json:"runFormat,omitempty"`

	// Context hash: SHA256 of surrounding text for freshness validation
	// Used to detect if the original text has been modified since export
	ContextHash string `json:"contextHash,omitempty"`

	// Optional notes for translators (e.g., "This is a title", "Table cell context")
	Notes string `json:"notes,omitempty"`

	// Markers for translation state
	IsTranslated bool `json:"isTranslated,omitempty"` // True if targetText has been filled
	IsStale      bool `json:"isStale,omitempty"`      // True if source text changed since export
}

// TranslationManifest is the top-level structure for a deck's translation manifest
type TranslationManifest struct {
	// Metadata about the manifest
	Metadata *ManifestMetadata `json:"metadata"`

	// Ordered list of translatable entries
	Entries []TranslationEntry `json:"entries"`
}

// NewManifest creates a new empty translation manifest with default metadata
func NewManifest() *TranslationManifest {
	return &TranslationManifest{
		Metadata: &ManifestMetadata{
			Version:    ManifestVersion,
			ExportedAt: time.Now().UTC(),
		},
		Entries: []TranslationEntry{},
	}
}

// NewEntry creates a new translation entry with the given parameters
func NewEntry(
	id, entryType, sourceText string,
	slideID, slideNumber int,
	paragraphIdx, runIdx int,
) TranslationEntry {
	return TranslationEntry{
		ID:             id,
		Type:           entryType,
		SourceText:     sourceText,
		SlideID:        slideID,
		SlideNumber:    slideNumber,
		ParagraphIndex: paragraphIdx,
		RunIndex:       runIdx,
		SegmentType:    "text",
	}
}

// MarshalJSON returns the JSON encoding of the manifest
func (m *TranslationManifest) MarshalJSON() ([]byte, error) {
	return json.Marshal(*m)
}
