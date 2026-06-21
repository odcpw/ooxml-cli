package model

// Segment type constants
const (
	SegmentText  = "text"
	SegmentBreak = "break"
	SegmentTab   = "tab"
	SegmentField = "field"
)

// TextBlockInfo represents the text content and properties of a shape
type TextBlockInfo struct {
	Paragraphs     []Paragraph         `json:"paragraphs"`               // List of paragraphs
	PlainText      string              `json:"plainText"`                // Plain text representation
	BodyProperties *TextBodyProperties `json:"bodyProperties,omitempty"` // Text body properties
}

// TextBodyProperties represents text body level properties (a:bodyPr)
type TextBodyProperties struct {
	// Anchor point for text (top, middle, bottom, etc.)
	Anchor string `json:"anchor,omitempty"`

	// Vertical anchor position (unused in most cases)
	AnchorCtr *bool `json:"anchorCtr,omitempty"`

	// Text wrapping mode (square, none, etc.)
	Wrap string `json:"wrap,omitempty"`

	// Inset distance from border (in EMU)
	LeftInset   *int64 `json:"leftInset,omitempty"`
	TopInset    *int64 `json:"topInset,omitempty"`
	RightInset  *int64 `json:"rightInset,omitempty"`
	BottomInset *int64 `json:"bottomInset,omitempty"`

	// Autofit type (noAutofit, normAutofit, spAutoFit)
	AutofitType string `json:"autofitType,omitempty"`

	// Column count (for text columns)
	Columns *int32 `json:"columns,omitempty"`

	// Rotation angle for vertical text
	Rot *int32 `json:"rot,omitempty"`

	// Vertical text orientation
	VerticalMode string `json:"verticalMode,omitempty"`

	// Right-to-left text
	RtlCol *bool `json:"rtlCol,omitempty"`
}

// Paragraph represents a paragraph within text
type Paragraph struct {
	Runs       []interface{}        `json:"runs,omitempty"`       // Text runs (interface{} for rich segments)
	Text       string               `json:"text"`                 // Plain text of the paragraph
	Properties *ParagraphProperties `json:"properties,omitempty"` // Paragraph properties
	Level      *int32               `json:"level,omitempty"`      // Paragraph indent level (0-8)
	Segments   []TextSegment        `json:"segments,omitempty"`   // Ordered segments (text, break, tab, field)
}

// ParagraphProperties represents properties of a paragraph (a:pPr)
type ParagraphProperties struct {
	// Indentation level for bullets
	Level *int32 `json:"level,omitempty"`

	// Alignment (l, ctr, r, just, dist)
	Alignment string `json:"alignment,omitempty"`

	// Spacing before paragraph (in EMU)
	SpaceBefore *int64 `json:"spaceBefore,omitempty"`

	// Spacing after paragraph (in EMU)
	SpaceAfter *int64 `json:"spaceAfter,omitempty"`

	// Line spacing (in EMU)
	LineSpacing *int64 `json:"lineSpacing,omitempty"`

	// Line spacing type (sngSpaced, pct, spcPts)
	LineSpacingType string `json:"lineSpacingType,omitempty"`

	// Default run properties for this paragraph
	DefaultRunProps *RunProperties `json:"defaultRunProperties,omitempty"`

	// Bullet mode (buNone, buChar, buAutoNum)
	BulletMode string `json:"bulletMode,omitempty"`

	// Bullet character (e.g., "•")
	BulletCharacter string `json:"bulletCharacter,omitempty"`

	// Auto-numbering scheme
	AutoNumberingScheme string `json:"autoNumberingScheme,omitempty"`

	// Hanging indent for bullet (in EMU)
	BulletIndent *int64 `json:"bulletIndent,omitempty"`

	// Font size for bullet (in points * 100)
	BulletFontSize *int32 `json:"bulletFontSize,omitempty"`

	// Font family for bullet
	BulletFontFamily string `json:"bulletFontFamily,omitempty"`

	// Bullet color (RGB hex like "FF0000" for red)
	BulletColor string `json:"bulletColor,omitempty"`
}

// TextRun represents a run of text with the same formatting
type TextRun struct {
	Type       string         `json:"type,omitempty"`       // Always "text" for compatibility
	Text       string         `json:"text"`                 // The text content
	Properties *RunProperties `json:"properties,omitempty"` // Run properties
}

// RunProperties represents properties of a text run (a:rPr)
type RunProperties struct {
	// Font properties
	FontFamily string   `json:"fontFamily,omitempty"` // Font name (e.g., "Arial")
	FontSize   *float64 `json:"fontSize,omitempty"`   // Font size in points

	// Text decorations
	Bold      *bool  `json:"bold,omitempty"`
	Italic    *bool  `json:"italic,omitempty"`
	Underline string `json:"underline,omitempty"` // Line, double, heavy, dash, etc.
	Strike    string `json:"strike,omitempty"`    // Single, double
	Baseline  string `json:"baseline,omitempty"`  // Superscript, subscript

	// Color
	Color      string      `json:"color,omitempty"`      // RGB hex color (e.g., "FF0000")
	ThemeColor string      `json:"themeColor,omitempty"` // Theme color name (accent1, etc.)
	ThemeTint  *int32      `json:"themeTint,omitempty"`  // Theme color tint (0-100000)
	ThemeShade *int32      `json:"themeShade,omitempty"` // Theme color shade (0-100000)
	SolidColor *SolidColor `json:"solidColor,omitempty"` // Rich color information

	// Language
	Language string `json:"language,omitempty"` // Language code (e.g., "en-US")

	// Additional properties
	Dirty         *bool `json:"dirty,omitempty"`         // Indicates run properties override defaults
	SmartTagClean *bool `json:"smartTagClean,omitempty"` // Smart tag indicator
}

// Break represents a line break segment (a:br)
type Break struct {
	Type       string         `json:"type"`                 // Always "break"
	Properties *RunProperties `json:"properties,omitempty"` // Run properties for the break
}

// Tab represents a tab segment (a:tab)
type Tab struct {
	Type       string         `json:"type"`                 // Always "tab"
	Properties *RunProperties `json:"properties,omitempty"` // Run properties for the tab
}

// Field represents a field segment (a:fld)
type Field struct {
	Type       string         `json:"type"`                 // Always "field"
	ID         string         `json:"id,omitempty"`         // Field ID
	FieldType  string         `json:"fieldType,omitempty"`  // Field type (date, time, slidenum, etc.)
	Format     string         `json:"format,omitempty"`     // Format string
	Properties *RunProperties `json:"properties,omitempty"` // Run properties for the field
	Text       string         `json:"text,omitempty"`       // Cached field text
}

// SolidColor represents a solid color in various formats (RGB, theme color, etc.)
type SolidColor struct {
	Type  string   `json:"type"`            // srgbClr, schemeClr, etc.
	Value string   `json:"value,omitempty"` // Color value (RGB hex or scheme name)
	Alpha *float64 `json:"alpha,omitempty"` // Alpha value (0-1)
	Tint  *float64 `json:"tint,omitempty"`  // Tint adjustment (0-1)
	Shade *float64 `json:"shade,omitempty"` // Shade adjustment (0-1)
}

// TextSegment represents a single segment of text (run, break, tab, or field)
type TextSegment struct {
	Type       string         `json:"type"`                 // text, break, tab, field
	Text       string         `json:"text,omitempty"`       // Text content (for text and field types)
	Properties *RunProperties `json:"properties,omitempty"` // Run properties
}
