package template

import (
	"fmt"
	"regexp"

	pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// This file holds the dependency-free planning + validation logic for
// `ooxml template apply`. It deliberately imports nothing from pkg/pptx/inspect,
// pkg/xlsx/inspect, or pkg/xlsx/chart, because those packages import
// pkg/template (the token model) and an import here would create a cycle. The
// CLI layer (internal/cli/template_apply.go) performs the actual OOXML mutations
// using the plans this file produces.

// ColorTokenName is the OOXML clrScheme element name a color token maps to,
// as accepted by pkg/pptx/mutate.UpdateThemeColor.
type ColorToken struct {
	// Name is the OOXML color element name: dk1, lt1, dk2, lt2, accent1..accent6,
	// hlink, folHlink.
	Name string
	// Hex is the validated 6-character RRGGBB value to apply.
	Hex string
}

// FontPlan captures the major/minor font typefaces to apply to a theme.
type FontPlan struct {
	MajorFont string
	MinorFont string
}

// ChartTokenPlan is a representative chart style summary to apply to every chart
// part found in the target document.
type ChartTokenPlan struct {
	SeriesFillColor string
	SeriesLineColor string
}

// ApplyPlan is the resolved, validated set of mutations to perform, derived from
// a TemplateTokens source under a selection of target categories.
type ApplyPlan struct {
	// Colors are theme color tokens that passed per-token validation.
	Colors []ColorToken
	// Fonts is non-nil when at least one of major/minor font is to be applied.
	Fonts *FontPlan
	// Chart is non-nil when a chart style token is to be applied to charts.
	Chart *ChartTokenPlan
	// Skipped lists human-readable reasons that individual tokens were not
	// applied (empty/unresolved color, no theme present, etc.).
	Skipped []string
}

// ApplySelection chooses which token categories to apply. When none are set
// explicitly the caller should treat that as "all supported".
type ApplySelection struct {
	Colors bool
	Fonts  bool
	Charts bool
}

var hexColorRe = regexp.MustCompile(`^[0-9A-Fa-f]{6}$`)

// DecorativeKeys are top-level JSON keys in a hand-authored --tokens profile that
// represent decorative effects this command intentionally does not support.
var DecorativeKeys = []string{"gradients", "animations", "3dFormats", "conditionalFormats", "transitions"}

// colorTokenMapping maps the JSON field names exposed by pptxmodel.ColorScheme to
// the OOXML clrScheme element names accepted by the theme mutator. Order is
// deterministic for stable output.
var colorTokenOrder = []struct {
	ooxmlName string
	get       func(*pptxmodel.ColorScheme) string
}{
	{"dk1", func(c *pptxmodel.ColorScheme) string { return c.Dark1 }},
	{"lt1", func(c *pptxmodel.ColorScheme) string { return c.Light1 }},
	{"dk2", func(c *pptxmodel.ColorScheme) string { return c.Dark2 }},
	{"lt2", func(c *pptxmodel.ColorScheme) string { return c.Light2 }},
	{"accent1", func(c *pptxmodel.ColorScheme) string { return c.Accent1 }},
	{"accent2", func(c *pptxmodel.ColorScheme) string { return c.Accent2 }},
	{"accent3", func(c *pptxmodel.ColorScheme) string { return c.Accent3 }},
	{"accent4", func(c *pptxmodel.ColorScheme) string { return c.Accent4 }},
	{"accent5", func(c *pptxmodel.ColorScheme) string { return c.Accent5 }},
	{"accent6", func(c *pptxmodel.ColorScheme) string { return c.Accent6 }},
	{"hlink", func(c *pptxmodel.ColorScheme) string { return c.HypLink }},
	{"folHlink", func(c *pptxmodel.ColorScheme) string { return c.FolLink }},
}

// IsValidHex reports whether v is a 6-character RRGGBB hex string.
func IsValidHex(v string) bool {
	return hexColorRe.MatchString(v)
}

// themeOf returns the theme carried by either the PPTX or XLSX block.
func themeOf(t *TemplateTokens) *pptxmodel.ThemeInfo {
	if t == nil {
		return nil
	}
	if t.PPTX != nil {
		return t.PPTX.Theme
	}
	if t.XLSX != nil {
		return t.XLSX.Theme
	}
	return nil
}

// chartSummariesOf returns the chart style summaries carried by the source.
func chartSummariesOf(t *TemplateTokens) []ChartStyleSummary {
	if t == nil {
		return nil
	}
	if t.PPTX != nil {
		return t.PPTX.ChartStyles
	}
	if t.XLSX != nil {
		return t.XLSX.ChartStyles
	}
	return nil
}

// BuildApplyPlan resolves a TemplateTokens source into a concrete, validated
// ApplyPlan for the requested selection. It never errors on individual tokens:
// unresolved or empty tokens are recorded in Skipped so the caller can report
// applied-vs-skipped. It returns an error only when the source is empty/unusable
// for the requested selection (so the caller can fail fast with a clear message).
func BuildApplyPlan(src *TemplateTokens, sel ApplySelection) (*ApplyPlan, error) {
	if src == nil {
		return nil, fmt.Errorf("token source is empty")
	}
	plan := &ApplyPlan{}
	theme := themeOf(src)

	if sel.Colors {
		if theme == nil || theme.ColorScheme == nil {
			plan.Skipped = append(plan.Skipped, "colors: source has no theme color scheme")
		} else {
			cs := theme.ColorScheme
			for _, m := range colorTokenOrder {
				val := m.get(cs)
				if val == "" {
					plan.Skipped = append(plan.Skipped, fmt.Sprintf("color %s: not present in source theme", m.ooxmlName))
					continue
				}
				if !IsValidHex(val) {
					plan.Skipped = append(plan.Skipped, fmt.Sprintf("color %s: value %q is not a 6-digit hex (likely a system color); skipped", m.ooxmlName, val))
					continue
				}
				plan.Colors = append(plan.Colors, ColorToken{Name: m.ooxmlName, Hex: val})
			}
		}
	}

	if sel.Fonts {
		if theme == nil || theme.FontScheme == nil {
			plan.Skipped = append(plan.Skipped, "fonts: source has no theme font scheme")
		} else {
			fs := theme.FontScheme
			if fs.MajorFont == "" && fs.MinorFont == "" {
				plan.Skipped = append(plan.Skipped, "fonts: source theme has no major/minor font")
			} else {
				plan.Fonts = &FontPlan{MajorFont: fs.MajorFont, MinorFont: fs.MinorFont}
			}
		}
	}

	if sel.Charts {
		summaries := chartSummariesOf(src)
		chart := representativeChart(summaries)
		if chart == nil {
			plan.Skipped = append(plan.Skipped, "charts: source has no chart style with a series fill/line color")
		} else {
			plan.Chart = chart
		}
	}

	if len(plan.Colors) == 0 && plan.Fonts == nil && plan.Chart == nil {
		return plan, fmt.Errorf("nothing to apply for the requested selection (see skipped reasons)")
	}
	return plan, nil
}

// representativeChart picks the first source chart summary carrying a usable
// series fill or line color. The series colors of one representative chart are
// applied to series 1 of every target chart; this is intentionally simple and
// design-transfer oriented rather than a per-chart copy.
func representativeChart(summaries []ChartStyleSummary) *ChartTokenPlan {
	for _, s := range summaries {
		fill := ""
		line := ""
		if IsValidHex(s.SeriesFillColor) {
			fill = s.SeriesFillColor
		}
		if IsValidHex(s.SeriesLineColor) {
			line = s.SeriesLineColor
		}
		if fill != "" || line != "" {
			return &ChartTokenPlan{SeriesFillColor: fill, SeriesLineColor: line}
		}
	}
	return nil
}

// FindDecorativeKeys returns any decorative top-level keys present in a decoded
// --tokens JSON object, so the CLI can refuse with an actionable message.
func FindDecorativeKeys(raw map[string]interface{}) []string {
	var found []string
	for _, k := range DecorativeKeys {
		if _, ok := raw[k]; ok {
			found = append(found, k)
		}
	}
	return found
}
