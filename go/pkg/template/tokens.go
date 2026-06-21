// Package template defines the neutral, cross-family design-token model that the
// `ooxml template tokens` command emits and that the later template-apply and
// template-profile slices consume.
//
// The model is intentionally family-neutral: a single TemplateTokens value
// carries either a PPTX block or an XLSX block (never both, since a token dump
// is produced from one source file). Types are deliberately small, exported, and
// JSON-stable so downstream slices can depend on the contract.
//
// It reuses pkg/pptx/model.ThemeInfo for the theme color/font scheme so the
// theme readback shape is identical to `ooxml pptx theme` output, but defines
// its own simplified chart-style summary rather than embedding the full chart
// style snapshot (which carries more detail than design transfer needs).
package template

import pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"

// SchemaVersion is the immutable contract version of TemplateTokens. Bump only
// on a breaking change to the JSON shape; downstream slices pin against it.
const SchemaVersion = "1.0"

// Token source kinds.
const (
	KindPPTX = "pptx"
	KindXLSX = "xlsx"
)

// TemplateTokens is the top-level deterministic token contract. Exactly one of
// PPTX or XLSX is populated, selected by Type.
type TemplateTokens struct {
	SchemaVersion string      `json:"schemaVersion"`
	Type          string      `json:"type"`   // "pptx" | "xlsx"
	Source        string      `json:"source"` // input filename (basename)
	PPTX          *PPTXTokens `json:"pptx,omitempty"`
	XLSX          *XLSXTokens `json:"xlsx,omitempty"`
}

// PPTXTokens holds the design tokens extracted from a PPTX/POTX template.
type PPTXTokens struct {
	Theme             *pptxmodel.ThemeInfo `json:"theme,omitempty"`
	DefaultTextStyles []DefaultTextStyle   `json:"defaultTextStyles"`
	TableStyles       []TableStyle         `json:"tableStyles"`
	ChartStyles       []ChartStyleSummary  `json:"chartStyles"`
}

// XLSXTokens holds the design tokens extracted from an XLSX/XLTX template.
type XLSXTokens struct {
	Theme           *pptxmodel.ThemeInfo `json:"theme,omitempty"`
	NamedCellStyles []NamedCellStyle     `json:"namedCellStyles"`
	ChartStyles     []ChartStyleSummary  `json:"chartStyles"`
}

// DefaultTextStyle is a per-master default text style for one placeholder role,
// read from the slide master's <p:txStyles> level-1 defaults.
type DefaultTextStyle struct {
	MasterRef string  `json:"masterRef"`         // part URI of the slide master
	Role      string  `json:"role"`              // "title" | "body" | "other"
	FontRef   string  `json:"fontRef,omitempty"` // theme alias: "major" | "minor" | "" when literal
	FontName  string  `json:"fontName,omitempty"`
	SizePt    float64 `json:"sizePt,omitempty"`
	Color     string  `json:"color,omitempty"`    // hex RRGGBB
	ColorRef  string  `json:"colorRef,omitempty"` // theme color name (e.g. "tx1", "accent1")
}

// TableStyle is a referenced table style id and (when resolvable) name, gathered
// from slide tables and the table-style part.
type TableStyle struct {
	StyleID string `json:"styleId"`
	Name    string `json:"name,omitempty"`
}

// ChartStyleSummary is a practical, design-transfer subset of a chart's style.
type ChartStyleSummary struct {
	PartURI         string `json:"partUri"`
	ChartType       string `json:"chartType,omitempty"`
	SeriesFillColor string `json:"seriesFillColor,omitempty"`
	SeriesLineColor string `json:"seriesLineColor,omitempty"`
	TitleFontFamily string `json:"titleFontFamily,omitempty"`
}

// NamedCellStyle is a workbook named cell style (xl/styles.xml <cellStyle>)
// resolved to its practical font/fill/number-format properties.
type NamedCellStyle struct {
	Name             string  `json:"name"`
	Builtin          bool    `json:"builtin"`
	FontName         string  `json:"fontName,omitempty"`
	SizePt           float64 `json:"sizePt,omitempty"`
	Bold             bool    `json:"bold,omitempty"`
	Italic           bool    `json:"italic,omitempty"`
	Color            string  `json:"color,omitempty"`     // hex RRGGBB
	FillColor        string  `json:"fillColor,omitempty"` // hex RRGGBB
	NumberFormatCode string  `json:"numberFormatCode,omitempty"`
}

// NewTokens returns an empty TemplateTokens with the pinned schema version.
func NewTokens(kind, source string) *TemplateTokens {
	return &TemplateTokens{
		SchemaVersion: SchemaVersion,
		Type:          kind,
		Source:        source,
	}
}
