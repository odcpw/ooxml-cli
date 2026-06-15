package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

// This command consumes the TEMPLATE-1 design-token model (pkg/template) and the
// existing theme + chart mutators to apply colors/fonts (and, opt-in, chart
// series styling) from a source template or a tokens JSON profile onto an
// existing PPTX/XLSX document. Range/cell style transfer is intentionally out of
// scope: the token model carries no per-range style to source from.

var (
	templateApplyFrom    string
	templateApplyTokens  string
	templateApplyProfile string
	templateApplyFor     string
	templateApplyColors  bool
	templateApplyFonts   bool
	templateApplyCharts  bool
	templateApplyRanges  bool
)

// TemplateApplyResult is the deterministic JSON readback of what was applied.
type TemplateApplyResult struct {
	File          string                  `json:"file"`
	Output        string                  `json:"output,omitempty"`
	DryRun        bool                    `json:"dryRun"`
	TargetType    string                  `json:"targetType"`
	ProfileSource string                  `json:"profileSource"`
	ProfileName   string                  `json:"profileName,omitempty"`
	SchemaVersion string                  `json:"schemaVersion,omitempty"`
	Applied       TemplateApplyAppliedSet `json:"applied"`
	Skipped       []string                `json:"skipped"`
	Warnings      []string                `json:"warnings,omitempty"`
	TotalUpdates  int                     `json:"totalUpdates"`
}

type TemplateApplyAppliedSet struct {
	Colors []TemplateAppliedColor `json:"colors"`
	Fonts  *TemplateAppliedFonts  `json:"fonts,omitempty"`
	Charts []TemplateAppliedChart `json:"charts"`
}

type TemplateAppliedColor struct {
	ColorName string `json:"colorName"`
	HexValue  string `json:"hexValue"`
}

type TemplateAppliedFonts struct {
	MajorFont string `json:"majorFont,omitempty"`
	MinorFont string `json:"minorFont,omitempty"`
}

type TemplateAppliedChart struct {
	PartURI         string `json:"partUri"`
	SeriesFillColor string `json:"seriesFillColor,omitempty"`
	SeriesLineColor string `json:"seriesLineColor,omitempty"`
}

var templateApplyCmd = &cobra.Command{
	Use:   "apply <file>",
	Short: "Apply design tokens from a template or profile to a PPTX/XLSX",
	Long: `Apply design tokens (theme colors, major/minor fonts, and optional chart
series styling) from a source template or a tokens JSON profile onto an existing
PPTX or XLSX document.

Provide the token source with exactly one of:
  --from <template>   read tokens from another PPTX/POTX or XLSX/XLTX file
  --tokens <file>     read a TemplateTokens JSON dump (e.g. from 'template tokens')
  --profile <file>    read a saved design profile (from 'template profile save')

By default colors and fonts are applied. Charts are opt-in (--target-charts).
Pass any --target-* flag to apply only the selected categories. Range/cell style
transfer is not supported (the token model carries no per-range style); a request
for --target-ranges is reported as skipped.

Decorative effects (gradients, animations, 3D, conditional formats) in a
hand-authored --tokens profile are refused with an actionable message.

This is a mutation: use --out, --in-place, or --dry-run.

Examples:
  ooxml template apply deck.pptx --from brand.potx --out branded.pptx
  ooxml --json template apply deck.pptx --tokens tokens.json --dry-run
  ooxml template apply report.xlsx --from brand.xltx --target-charts --in-place --backup .bak`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		targetPath := args[0]
		if _, err := os.Stat(targetPath); err != nil {
			return FileNotFoundError(targetPath)
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		// Source detection uses flag values (not .Changed): the shared root
		// command is reused across tests where flag reset marks .Changed=true.
		fromSet := strings.TrimSpace(templateApplyFrom) != ""
		tokensSet := strings.TrimSpace(templateApplyTokens) != ""
		profileSet := strings.TrimSpace(templateApplyProfile) != ""
		nSources := 0
		for _, s := range []bool{fromSet, tokensSet, profileSet} {
			if s {
				nSources++
			}
		}
		if nSources != 1 {
			return InvalidArgsError("specify exactly one token source: --from <template>, --tokens <tokens.json>, or --profile <profile.json>")
		}

		sel, rangesRequested, err := resolveApplySelection()
		if err != nil {
			return err
		}

		// Determine target package family.
		targetKind, err := resolveTemplateTokensKind(targetPath, templateApplyFor)
		if err != nil {
			return err
		}

		// Load the token source.
		var src *tmpl.TemplateTokens
		var profileSource string
		var profileName string
		switch {
		case fromSet:
			profileSource = templateApplyFrom
			srcKind, kerr := resolveTemplateTokensKind(templateApplyFrom, "auto")
			if kerr != nil {
				return kerr
			}
			src, err = extractTemplateTokens(templateApplyFrom, srcKind)
			if err != nil {
				return err
			}
		case profileSet:
			profileSource = templateApplyProfile
			profile, perr := loadDesignProfile(templateApplyProfile)
			if perr != nil {
				return perr
			}
			profileName = profile.Metadata.Name
			// Lift the profile's token subset into the target family's block and
			// run the SAME applier as --from, so a saved profile produces the same
			// changes as applying from the original template.
			src = profile.ToTokens(packageKindString(targetKind))
		default:
			profileSource = templateApplyTokens
			src, profileName, err = loadTokensProfile(templateApplyTokens)
			if err != nil {
				return err
			}
		}

		result, err := performTemplateApply(targetPath, targetKind, src, sel, rangesRequested, mutOpts)
		if err != nil {
			return err
		}
		result.ProfileSource = profileSource
		result.ProfileName = profileName
		if src != nil {
			result.SchemaVersion = src.SchemaVersion
			// Soft schema-version check: a future profile may carry tokens this
			// build does not understand. Warn (do not refuse) so newer profiles
			// remain usable for the fields this build supports.
			if src.SchemaVersion != "" && src.SchemaVersion != tmpl.SchemaVersion {
				result.Warnings = append(result.Warnings, fmt.Sprintf(
					"profile schemaVersion %q differs from supported %q; applied only the tokens this build understands",
					src.SchemaVersion, tmpl.SchemaVersion))
			}
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, result)
		}
		return writeGlobalOutput(cmd, []byte(renderTemplateApplyText(result)))
	},
}

// packageKindString maps an opc package type to the template token kind string
// (KindPPTX/KindXLSX) used when lifting a design profile into a TemplateTokens.
func packageKindString(kind opc.PackageType) string {
	if kind == opc.PackageTypeXLSX {
		return tmpl.KindXLSX
	}
	return tmpl.KindPPTX
}

// resolveApplySelection turns the --target-* booleans into a selection. When no
// --target-* flag is set, the default is colors+fonts (charts opt-in). It also
// reports whether --target-ranges was requested (unsupported, reported skipped).
// Selection is derived from flag VALUES (not .Changed) so it is reset-safe under
// the shared root command used in tests.
func resolveApplySelection() (tmpl.ApplySelection, bool, error) {
	anySet := templateApplyColors || templateApplyFonts || templateApplyCharts || templateApplyRanges
	if !anySet {
		// Default: colors + fonts (highest value, safest). Charts opt-in.
		return tmpl.ApplySelection{Colors: true, Fonts: true, Charts: false}, false, nil
	}

	sel := tmpl.ApplySelection{
		Colors: templateApplyColors,
		Fonts:  templateApplyFonts,
		Charts: templateApplyCharts,
	}
	if !sel.Colors && !sel.Fonts && !sel.Charts && !templateApplyRanges {
		return sel, false, InvalidArgsError("no applicable target selected; use --target-colors, --target-fonts, or --target-charts")
	}
	return sel, templateApplyRanges, nil
}

// loadTokensProfile reads a TemplateTokens JSON profile from disk, refusing
// decorative effect keys and validating basic structure.
func loadTokensProfile(path string) (*tmpl.TemplateTokens, string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, "", FileNotFoundError(path)
	}

	// Detect decorative top-level keys before structured decode.
	var raw map[string]interface{}
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "failed to parse tokens profile %s: %v", path, err)
	}
	if dec := tmpl.FindDecorativeKeys(raw); len(dec) > 0 {
		return nil, "", NewCLIErrorf(ExitInvalidArgs,
			"decorative effects (%s) are not supported by 'template apply'; it transfers theme colors, fonts, and chart series styling. Use 'ooxml pptx charts set-series-style' or 'ooxml xlsx charts set-series-style' for custom fills",
			strings.Join(dec, ", "))
	}

	var src tmpl.TemplateTokens
	if err := json.Unmarshal(data, &src); err != nil {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "failed to parse tokens profile %s: %v", path, err)
	}
	if src.PPTX == nil && src.XLSX == nil {
		return nil, "", NewCLIErrorf(ExitInvalidArgs, "tokens profile %s is empty (no pptx or xlsx token block)", path)
	}
	return &src, src.Source, nil
}

func performTemplateApply(targetPath string, targetKind opc.PackageType, src *tmpl.TemplateTokens, sel tmpl.ApplySelection, rangesRequested bool, mutOpts *MutationOptions) (*TemplateApplyResult, error) {
	plan, planErr := tmpl.BuildApplyPlan(src, sel)
	// planErr means nothing to apply; we still surface skipped reasons unless the
	// caller also passed no usable selection. Treat it as a clear failure.
	if planErr != nil {
		// Build a helpful message listing skipped reasons.
		reasons := ""
		if plan != nil && len(plan.Skipped) > 0 {
			reasons = ": " + strings.Join(plan.Skipped, "; ")
		}
		return nil, NewCLIErrorf(ExitInvalidArgs, "%v%s", planErr, reasons)
	}

	result := &TemplateApplyResult{
		File:       targetPath,
		DryRun:     mutOpts.DryRun,
		TargetType: targetKind.String(),
		Applied: TemplateApplyAppliedSet{
			Colors: []TemplateAppliedColor{},
			Charts: []TemplateAppliedChart{},
		},
		Skipped: append([]string{}, plan.Skipped...),
	}
	if rangesRequested {
		result.Skipped = append(result.Skipped,
			"ranges: range/cell style transfer is not supported (no per-range style in the token model)")
	}

	writer, err := NewMutationWriterForType(targetPath, mutOpts, targetKind)
	if err != nil {
		return nil, err
	}

	if err := writer.Write(func(pkg opc.PackageSession) error {
		themeURI, themeErr := resolveTargetThemeURI(pkg, targetKind)

		// Apply theme colors.
		if len(plan.Colors) > 0 {
			if themeErr != nil {
				result.Skipped = append(result.Skipped, "colors: "+themeErr.Error())
			} else {
				for _, c := range plan.Colors {
					req := &pptxmutate.UpdateThemeColorRequest{
						Package:   pkg,
						ThemeURI:  themeURI,
						ColorName: c.Name,
						HexValue:  c.Hex,
					}
					if err := pptxmutate.UpdateThemeColor(req); err != nil {
						result.Skipped = append(result.Skipped, fmt.Sprintf("color %s: %v", c.Name, err))
						continue
					}
					result.Applied.Colors = append(result.Applied.Colors, TemplateAppliedColor{
						ColorName: c.Name, HexValue: c.Hex,
					})
				}
			}
		}

		// Apply theme fonts.
		if plan.Fonts != nil {
			if themeErr != nil {
				result.Skipped = append(result.Skipped, "fonts: "+themeErr.Error())
			} else {
				req := &pptxmutate.UpdateThemeFontRequest{
					Package:   pkg,
					ThemeURI:  themeURI,
					MajorFont: plan.Fonts.MajorFont,
					MinorFont: plan.Fonts.MinorFont,
				}
				if err := pptxmutate.UpdateThemeFont(req); err != nil {
					result.Skipped = append(result.Skipped, fmt.Sprintf("fonts: %v", err))
				} else {
					result.Applied.Fonts = &TemplateAppliedFonts{
						MajorFont: plan.Fonts.MajorFont,
						MinorFont: plan.Fonts.MinorFont,
					}
				}
			}
		}

		// Apply chart series styling to every chart part in the target.
		if plan.Chart != nil {
			chartURIs := listChartParts(pkg, targetKind)
			if len(chartURIs) == 0 {
				result.Skipped = append(result.Skipped, "charts: target has no chart parts")
			}
			for _, uri := range chartURIs {
				req := &xlsxchart.SetSeriesStyleRequest{
					Package:      pkg,
					ChartURI:     uri,
					SeriesNumber: 1,
					FillColor:    plan.Chart.SeriesFillColor,
					LineColor:    plan.Chart.SeriesLineColor,
				}
				res, err := xlsxchart.SetSeriesStyle(req)
				if err != nil {
					result.Skipped = append(result.Skipped, fmt.Sprintf("chart %s: %v", uri, err))
					continue
				}
				applied := TemplateAppliedChart{PartURI: uri}
				applied.SeriesFillColor = res.Series.FillColor
				applied.SeriesLineColor = res.Series.LineColor
				result.Applied.Charts = append(result.Applied.Charts, applied)
			}
		}

		return nil
	}); err != nil {
		return nil, err
	}

	result.Output = mutOpts.OutPath
	if mutOpts.InPlace {
		result.Output = targetPath
	}
	result.TotalUpdates = len(result.Applied.Colors) + len(result.Applied.Charts)
	if result.Applied.Fonts != nil {
		result.TotalUpdates++
	}
	sort.Strings(result.Skipped)
	return result, nil
}

// resolveTargetThemeURI finds the theme part URI for the target document family.
func resolveTargetThemeURI(pkg opc.PackageSession, kind opc.PackageType) (string, error) {
	switch kind {
	case opc.PackageTypePPTX:
		graph, err := pptxinspect.ParsePresentation(pkg)
		if err != nil {
			return "", fmt.Errorf("failed to parse presentation: %v", err)
		}
		if graph != nil && len(graph.Masters) > 0 && graph.Masters[0].ThemeURI != "" {
			return graph.Masters[0].ThemeURI, nil
		}
		return "/ppt/theme/theme1.xml", nil
	case opc.PackageTypeXLSX:
		wb, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return "", fmt.Errorf("failed to parse workbook: %v", err)
		}
		if wb == nil || wb.ThemeURI == "" {
			return "", fmt.Errorf("workbook has no theme part to apply colors/fonts to")
		}
		return wb.ThemeURI, nil
	default:
		return "", fmt.Errorf("unsupported package type: %s", kind)
	}
}

// listChartParts enumerates DrawingML chart parts for the target family.
func listChartParts(pkg opc.PackageSession, kind opc.PackageType) []string {
	prefix := "/ppt/charts/chart"
	if kind == opc.PackageTypeXLSX {
		prefix = "/xl/charts/chart"
	}
	uris := []string{}
	for _, p := range pkg.ListParts() {
		if strings.HasPrefix(p.URI, prefix) &&
			strings.HasSuffix(p.URI, ".xml") && !strings.Contains(p.URI, "/_rels/") {
			uris = append(uris, p.URI)
		}
	}
	sort.Strings(uris)
	return uris
}

func renderTemplateApplyText(r *TemplateApplyResult) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Template apply (%s)\n", r.TargetType)
	fmt.Fprintf(&b, "  Target: %s\n", r.File)
	if r.DryRun {
		fmt.Fprintf(&b, "  Mode:   dry-run (no output written)\n")
	} else if r.Output != "" {
		fmt.Fprintf(&b, "  Output: %s\n", r.Output)
	}
	fmt.Fprintf(&b, "  Source: %s\n", r.ProfileSource)

	fmt.Fprintf(&b, "  Applied colors (%d):\n", len(r.Applied.Colors))
	for _, c := range r.Applied.Colors {
		fmt.Fprintf(&b, "    - %s = #%s\n", c.ColorName, c.HexValue)
	}
	if r.Applied.Fonts != nil {
		fmt.Fprintf(&b, "  Applied fonts: major=%s minor=%s\n",
			r.Applied.Fonts.MajorFont, r.Applied.Fonts.MinorFont)
	}
	fmt.Fprintf(&b, "  Applied charts (%d):\n", len(r.Applied.Charts))
	for _, c := range r.Applied.Charts {
		fmt.Fprintf(&b, "    - %s fill=%s line=%s\n", c.PartURI, c.SeriesFillColor, c.SeriesLineColor)
	}
	if len(r.Skipped) > 0 {
		fmt.Fprintf(&b, "  Skipped (%d):\n", len(r.Skipped))
		for _, s := range r.Skipped {
			fmt.Fprintf(&b, "    - %s\n", s)
		}
	}
	fmt.Fprintf(&b, "  Total updates: %d\n", r.TotalUpdates)
	return strings.TrimRight(b.String(), "\n")
}

func init() {
	templateApplyCmd.Flags().StringVar(&templateApplyFrom, "from", "",
		"source template file (PPTX/POTX or XLSX/XLTX) to read tokens from")
	templateApplyCmd.Flags().StringVar(&templateApplyTokens, "tokens", "",
		"source TemplateTokens JSON dump to read tokens from")
	templateApplyCmd.Flags().StringVar(&templateApplyProfile, "profile", "",
		"saved design profile JSON (from 'template profile save') to apply")
	templateApplyCmd.Flags().StringVar(&templateApplyFor, "for", "auto",
		"target package family: pptx, xlsx, or auto (default: auto-detect)")
	templateApplyCmd.Flags().BoolVar(&templateApplyColors, "target-colors", false,
		"apply theme colors (default when no --target-* flag is set)")
	templateApplyCmd.Flags().BoolVar(&templateApplyFonts, "target-fonts", false,
		"apply major/minor fonts (default when no --target-* flag is set)")
	templateApplyCmd.Flags().BoolVar(&templateApplyCharts, "target-charts", false,
		"apply chart series fill/line styling to all charts")
	templateApplyCmd.Flags().BoolVar(&templateApplyRanges, "target-ranges", false,
		"(unsupported) range/cell style transfer; reported as skipped")

	AddMutationFlags(templateApplyCmd)
	templateGroupCmd.AddCommand(templateApplyCmd)
}
