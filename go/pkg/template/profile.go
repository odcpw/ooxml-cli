package template

// This file defines the portable DESIGN PROFILE: a compact, versioned JSON
// artifact that serializes the design-transfer SUBSET of TemplateTokens (theme
// color scheme + major/minor font scheme, plus informational placeholder
// defaults) so a brand can be saved once and applied to many decks/workbooks
// WITHOUT re-reading the source template.
//
// A profile is intentionally a strict subset of TemplateTokens, not a new model:
// it stores the theme block verbatim (reusing pkg/pptx/model.ThemeInfo) so that
// applying a profile lifts back into a *TemplateTokens and runs the EXACT same
// applier (BuildApplyPlan + performTemplateApply) as `template apply --from`.
// This makes the save -> apply round-trip produce identical Applied/Skipped sets
// to apply --from the original template.
//
// What a profile deliberately drops vs. full tokens (to stay "compact"):
//   - PPTX: table styles, chart style summaries
//   - XLSX: named cell styles, chart style summaries
// Chart series styling is NOT part of a profile; it is opt-in per-document via
// `template apply --from <template> --target-charts`.

import (
	"fmt"

	pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// ProfileSchemaVersion is the immutable contract version of DesignProfile. Bump
// only on a breaking change to the JSON shape.
const ProfileSchemaVersion = "1.0"

// ProfileFormat is the fixed format tag that identifies a design profile file,
// so a consumer can distinguish it from a raw TemplateTokens dump.
const ProfileFormat = "ooxml-design-profile"

// DesignProfile is the top-level portable profile artifact.
type DesignProfile struct {
	// SchemaVersion pins the profile JSON shape (see ProfileSchemaVersion).
	SchemaVersion string `json:"schemaVersion"`
	// Format is always ProfileFormat; lets readers verify the file kind.
	Format string `json:"format"`
	// Metadata carries human-facing identification (name/description/source).
	Metadata ProfileMetadata `json:"metadata"`
	// Design is the applied-token subset.
	Design ProfileDesign `json:"design"`
}

// ProfileMetadata identifies a profile. All fields are optional except that
// SourceType records which family the profile was saved from.
type ProfileMetadata struct {
	Name        string `json:"name,omitempty"`
	Description string `json:"description,omitempty"`
	// SourceFile is the basename of the template the profile was saved from.
	SourceFile string `json:"sourceFile,omitempty"`
	// SourceType is "pptx" or "xlsx" (the family the profile was saved from).
	// Colors/fonts are family-neutral, so a pptx-saved profile applies to xlsx
	// and vice versa; this is informational only.
	SourceType string `json:"sourceType,omitempty"`
}

// ProfileDesign holds the design tokens carried by the profile.
type ProfileDesign struct {
	// Theme carries the color scheme + font scheme verbatim (the only block the
	// applier consumes). Stored as ThemeInfo so apply lifts it back losslessly.
	Theme *pptxmodel.ThemeInfo `json:"theme,omitempty"`
	// Placeholders are informational per-role default text styles (font/size/
	// color), captured for reference. They are NOT mutated on apply (applying
	// placeholder defaults would require re-styling every slide); the applier
	// ignores them.
	Placeholders []DefaultTextStyle `json:"placeholders,omitempty"`
}

// ProfileFromTokens distills a full TemplateTokens value into a compact
// DesignProfile, keeping only the design-transfer subset. name/description are
// optional human labels. It never errors; an empty/themeless source yields a
// profile whose Design.Theme is nil (apply will then report nothing to apply).
func ProfileFromTokens(tokens *TemplateTokens, name, description string) *DesignProfile {
	p := &DesignProfile{
		SchemaVersion: ProfileSchemaVersion,
		Format:        ProfileFormat,
		Metadata: ProfileMetadata{
			Name:        name,
			Description: description,
		},
	}
	if tokens == nil {
		return p
	}
	p.Metadata.SourceFile = tokens.Source
	p.Metadata.SourceType = tokens.Type
	if tokens.PPTX != nil {
		p.Design.Theme = tokens.PPTX.Theme
		p.Design.Placeholders = tokens.PPTX.DefaultTextStyles
	} else if tokens.XLSX != nil {
		p.Design.Theme = tokens.XLSX.Theme
	}
	return p
}

// ToTokens lifts a profile back into a TemplateTokens value carrying only the
// theme subset, ready to feed the shared applier (BuildApplyPlan). The returned
// tokens use the profile's SchemaVersion-equivalent token schema version and the
// requested target kind block (PPTX or XLSX) — colors/fonts are family-neutral,
// so the block choice only affects where themeOf looks.
func (p *DesignProfile) ToTokens(targetKind string) *TemplateTokens {
	src := p.Metadata.SourceFile
	if src == "" {
		src = p.Metadata.Name
	}
	t := NewTokens(targetKind, src)
	switch targetKind {
	case KindXLSX:
		t.XLSX = &XLSXTokens{
			Theme:           p.Design.Theme,
			NamedCellStyles: []NamedCellStyle{},
			ChartStyles:     []ChartStyleSummary{},
		}
	default: // KindPPTX
		t.PPTX = &PPTXTokens{
			Theme:             p.Design.Theme,
			DefaultTextStyles: p.Design.Placeholders,
			TableStyles:       []TableStyle{},
			ChartStyles:       []ChartStyleSummary{},
		}
	}
	return t
}

// Validate enforces the minimal profile invariants. It is intentionally lenient
// about extra fields (forward-compat) but strict about the required identity
// fields and any present color values being well-formed hex.
func (p *DesignProfile) Validate() error {
	if p == nil {
		return fmt.Errorf("profile is empty")
	}
	if p.Format != ProfileFormat {
		return fmt.Errorf("not a design profile: format %q (want %q)", p.Format, ProfileFormat)
	}
	if p.SchemaVersion == "" {
		return fmt.Errorf("profile is missing schemaVersion")
	}
	// Colors, when present, must be 6-digit hex (system colors are stored as the
	// computed RGB by the theme reader, so this is the normal case). An empty
	// value is allowed (the applier records it as skipped, not an error).
	if p.Design.Theme != nil && p.Design.Theme.ColorScheme != nil {
		cs := p.Design.Theme.ColorScheme
		for _, kv := range []struct {
			name  string
			value string
		}{
			{"dk1", cs.Dark1}, {"lt1", cs.Light1}, {"dk2", cs.Dark2}, {"lt2", cs.Light2},
			{"accent1", cs.Accent1}, {"accent2", cs.Accent2}, {"accent3", cs.Accent3},
			{"accent4", cs.Accent4}, {"accent5", cs.Accent5}, {"accent6", cs.Accent6},
			{"hlink", cs.HypLink}, {"folHlink", cs.FolLink},
		} {
			if kv.value != "" && !IsValidHex(kv.value) {
				return fmt.Errorf("color %s value %q is not a 6-digit RRGGBB hex", kv.name, kv.value)
			}
		}
	}
	return nil
}

// HasDesign reports whether the profile carries any applicable token (a theme
// color scheme or font scheme). A profile without these is a no-op on apply.
func (p *DesignProfile) HasDesign() bool {
	if p == nil || p.Design.Theme == nil {
		return false
	}
	return p.Design.Theme.ColorScheme != nil || p.Design.Theme.FontScheme != nil
}
