package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
)

// This file adds the reusable DESIGN PROFILE commands under the cross-family
// `template` group:
//   template profile save <template> --out profile.json   (read-only extract)
//   template profile inspect <profile.json>               (read-only)
// Applying a profile is wired into the existing `template apply` command via its
// --profile flag (see template_apply.go), reusing the same applier so a saved
// profile produces the same changes as apply --from the original template.

var (
	templateProfileFor         string
	templateProfileName        string
	templateProfileDescription string
	templateProfileOut         string
)

// templateProfileCmd is the `template profile` subgroup.
var templateProfileCmd = &cobra.Command{
	Use:   "profile",
	Short: "Save and inspect portable design profiles",
	Long: `Portable, versioned design profiles.

A profile is a compact JSON artifact carrying the design-transfer subset of a
template's tokens (theme colors and major/minor fonts, plus informational
placeholder defaults). Save a profile once from a brand template, then apply it
to many decks/workbooks with 'template apply --profile profile.json' WITHOUT
re-reading the source template.`,
	Args: cobra.NoArgs,
	RunE: showHelp,
}

var templateProfileSaveCmd = &cobra.Command{
	Use:   "save <template>",
	Short: "Extract a reusable design profile from a PPTX/POTX or XLSX/XLTX template",
	Long: `Extract the design-transfer subset of a template into a compact, versioned
JSON profile (schemaVersion ` + tmpl.ProfileSchemaVersion + `).

The profile carries the theme color scheme and major/minor fonts (the tokens the
applier consumes) plus informational placeholder defaults. It deliberately drops
bulky tokens (table styles, named cell styles, chart summaries) that 'template
apply' does not transfer from a profile.

This is a read-only inspection of the template; it never modifies the template.
The profile is written to --out (or stdout when --out is omitted).

Examples:
  ooxml template profile save brand.potx --out brand.json --name "Acme Brand"
  ooxml --json template profile save report.xltx --out report-brand.json
  ooxml template profile save deck.pptx`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		kind, err := resolveTemplateTokensKind(filePath, templateProfileFor)
		if err != nil {
			return err
		}

		tokens, err := extractTemplateTokens(filePath, kind)
		if err != nil {
			return err
		}

		profile := tmpl.ProfileFromTokens(tokens, strings.TrimSpace(templateProfileName), strings.TrimSpace(templateProfileDescription))

		data, err := json.MarshalIndent(profile, "", "  ")
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to encode profile: %v", err)
		}
		data = append(data, '\n')

		if out := strings.TrimSpace(templateProfileOut); out != "" {
			if err := os.WriteFile(out, data, 0o644); err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to write profile %s: %v", out, err)
			}
			// On a successful write, emit a small confirmation so JSON consumers
			// get a structured result and text users get a friendly line.
			if GetGlobalConfig(cmd).Format == "json" {
				return writeGlobalJSON(cmd, profile)
			}
			return writeGlobalOutput(cmd, []byte(fmt.Sprintf("Saved design profile to %s (%d colors, fonts=%t)",
				out, countProfileColors(profile), profileHasFonts(profile))))
		}

		// No --out: stream the profile to stdout (text mode emits the JSON too,
		// since a profile is fundamentally a JSON artifact).
		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, profile)
		}
		return writeGlobalOutput(cmd, data)
	},
}

var templateProfileInspectCmd = &cobra.Command{
	Use:   "inspect <profile.json>",
	Short: "Validate and inspect a saved design profile",
	Long: `Read a saved design profile, validate its schema, and print a summary.

This is read-only. In --json mode the parsed profile is echoed back; in text
mode a human summary of colors and fonts is shown.

Examples:
  ooxml template profile inspect brand.json
  ooxml --json template profile inspect brand.json`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		profile, err := loadDesignProfile(args[0])
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, profile)
		}
		return writeGlobalOutput(cmd, []byte(renderProfileText(profile)))
	},
}

// loadDesignProfile reads and validates a design profile JSON file.
func loadDesignProfile(path string) (*tmpl.DesignProfile, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, FileNotFoundError(path)
	}
	var profile tmpl.DesignProfile
	if err := json.Unmarshal(data, &profile); err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "failed to parse design profile %s: %v", path, err)
	}
	if err := profile.Validate(); err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid design profile %s: %v", path, err)
	}
	return &profile, nil
}

func countProfileColors(p *tmpl.DesignProfile) int {
	if p == nil || p.Design.Theme == nil || p.Design.Theme.ColorScheme == nil {
		return 0
	}
	cs := p.Design.Theme.ColorScheme
	n := 0
	for _, v := range []string{
		cs.Dark1, cs.Light1, cs.Dark2, cs.Light2,
		cs.Accent1, cs.Accent2, cs.Accent3, cs.Accent4, cs.Accent5, cs.Accent6,
		cs.HypLink, cs.FolLink,
	} {
		if v != "" {
			n++
		}
	}
	return n
}

func profileHasFonts(p *tmpl.DesignProfile) bool {
	return p != nil && p.Design.Theme != nil && p.Design.Theme.FontScheme != nil &&
		(p.Design.Theme.FontScheme.MajorFont != "" || p.Design.Theme.FontScheme.MinorFont != "")
}

func renderProfileText(p *tmpl.DesignProfile) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Design Profile (schema %s)\n", p.SchemaVersion)
	if p.Metadata.Name != "" {
		fmt.Fprintf(&b, "  Name:        %s\n", p.Metadata.Name)
	}
	if p.Metadata.Description != "" {
		fmt.Fprintf(&b, "  Description: %s\n", p.Metadata.Description)
	}
	if p.Metadata.SourceFile != "" {
		fmt.Fprintf(&b, "  Source:      %s (%s)\n", p.Metadata.SourceFile, p.Metadata.SourceType)
	}
	renderThemeText(&b, p.Design.Theme)
	if len(p.Design.Placeholders) > 0 {
		fmt.Fprintf(&b, "  Placeholder defaults (%d, informational):\n", len(p.Design.Placeholders))
		for _, ds := range p.Design.Placeholders {
			font := ds.FontName
			if font == "" && ds.FontRef != "" {
				font = "theme:" + ds.FontRef
			}
			fmt.Fprintf(&b, "    - %s: font=%s size=%gpt\n", ds.Role, font, ds.SizePt)
		}
	}
	return strings.TrimRight(b.String(), "\n")
}

func init() {
	templateProfileSaveCmd.Flags().StringVar(&templateProfileFor, "for", "auto",
		"package family: pptx, xlsx, or auto (default: auto-detect)")
	templateProfileSaveCmd.Flags().StringVar(&templateProfileName, "name", "",
		"human-readable profile name stored in metadata")
	templateProfileSaveCmd.Flags().StringVar(&templateProfileDescription, "description", "",
		"profile description stored in metadata")
	templateProfileSaveCmd.Flags().StringVar(&templateProfileOut, "out", "",
		"write the profile to this file (default: stdout)")

	templateProfileCmd.AddCommand(templateProfileSaveCmd)
	templateProfileCmd.AddCommand(templateProfileInspectCmd)
	templateGroupCmd.AddCommand(templateProfileCmd)
}
