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
	pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

// This command consumes the TEMPLATE-1 design-token model (pkg/template) and the
// existing theme + chart mutators to apply colors/fonts, PPTX master default
// text styles, and chart series styling from a source template or a tokens JSON
// profile onto an existing PPTX/XLSX document. Range/cell style transfer is
// intentionally out of scope: the token model carries no per-range style to
// source from.

var (
	templateApplyFrom       string
	templateApplyTokens     string
	templateApplyProfile    string
	templateApplyFor        string
	templateApplyColors     bool
	templateApplyFonts      bool
	templateApplyCharts     bool
	templateApplyTextStyles bool
	templateApplyRanges     bool
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
	Colors     []TemplateAppliedColor     `json:"colors"`
	Fonts      *TemplateAppliedFonts      `json:"fonts,omitempty"`
	FontParts  []TemplateAppliedFonts     `json:"fontParts,omitempty"`
	Charts     []TemplateAppliedChart     `json:"charts"`
	TextStyles []TemplateAppliedTextStyle `json:"textStyles"`
}

type TemplateAppliedColor struct {
	PartURI   string `json:"partUri,omitempty"`
	ColorName string `json:"colorName"`
	HexValue  string `json:"hexValue"`
}

type TemplateAppliedFonts struct {
	PartURI   string `json:"partUri,omitempty"`
	MajorFont string `json:"majorFont,omitempty"`
	MinorFont string `json:"minorFont,omitempty"`
}

type TemplateAppliedChart struct {
	PartURI         string `json:"partUri"`
	SeriesFillColor string `json:"seriesFillColor,omitempty"`
	SeriesLineColor string `json:"seriesLineColor,omitempty"`
}

type TemplateAppliedTextStyle struct {
	MasterPartURI   string  `json:"masterPartUri"`
	Role            string  `json:"role"`
	SourceMasterRef string  `json:"sourceMasterRef,omitempty"`
	FontRef         string  `json:"fontRef,omitempty"`
	FontName        string  `json:"fontName,omitempty"`
	SizePt          float64 `json:"sizePt,omitempty"`
	Color           string  `json:"color,omitempty"`
	ColorRef        string  `json:"colorRef,omitempty"`
}

var templateApplyCmd = &cobra.Command{
	Use:   "apply <file>",
	Short: "Apply design tokens from a template or profile to a PPTX/XLSX",
	Long: `Apply design tokens (theme colors, major/minor fonts, optional PPTX master
default text styles, and optional chart series styling) from a source template
or a tokens JSON profile onto an existing PPTX or XLSX document.

Provide the token source with exactly one of:
  --from <template>   read tokens from another PPTX/POTX or XLSX/XLTX file
  --tokens <file>     read a TemplateTokens JSON dump (e.g. from 'template tokens')
  --profile <file>    read a saved design profile (from 'template profile save')

By default colors and fonts are applied. PPTX master default text styles and
charts are opt-in (--target-text-styles, --target-charts). Pass any --target-*
flag to apply only the selected categories. Range/cell style transfer is not
supported (the token model carries no per-range style); a request for
--target-ranges is reported as skipped.

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
// --target-* flag is set, the default is colors+fonts (text styles and charts
// opt-in). It also reports whether --target-ranges was requested (unsupported,
// reported skipped).
// Selection is derived from flag VALUES (not .Changed) so it is reset-safe under
// the shared root command used in tests.
func resolveApplySelection() (tmpl.ApplySelection, bool, error) {
	anySet := templateApplyColors || templateApplyFonts || templateApplyCharts || templateApplyTextStyles || templateApplyRanges
	if !anySet {
		// Default: colors + fonts (highest value, safest). Charts opt-in.
		return tmpl.ApplySelection{Colors: true, Fonts: true, Charts: false}, false, nil
	}

	sel := tmpl.ApplySelection{
		Colors:     templateApplyColors,
		Fonts:      templateApplyFonts,
		Charts:     templateApplyCharts,
		TextStyles: templateApplyTextStyles,
	}
	if !sel.Colors && !sel.Fonts && !sel.Charts && !sel.TextStyles && !templateApplyRanges {
		return sel, false, InvalidArgsError("no applicable target selected; use --target-colors, --target-fonts, --target-text-styles, or --target-charts")
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
			"decorative effects (%s) are not supported by 'template apply'; it transfers theme colors, fonts, PPTX master default text styles, and chart series styling. Use 'ooxml pptx charts set-series-style' or 'ooxml xlsx charts set-series-style' for custom fills",
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
			Colors:     []TemplateAppliedColor{},
			Charts:     []TemplateAppliedChart{},
			TextStyles: []TemplateAppliedTextStyle{},
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
		themeURIs, themeErr := resolveTargetThemeURIs(pkg, targetKind)

		// Apply theme colors.
		if len(plan.Colors) > 0 {
			if themeErr != nil {
				result.Skipped = append(result.Skipped, "colors: "+themeErr.Error())
			} else {
				for _, themeURI := range themeURIs {
					currentTheme, _ := pptxinspect.ParseTheme(pkg, themeURI)
					for _, c := range plan.Colors {
						if equalHex(themeColorValue(currentTheme, c.Name), c.Hex) {
							result.Skipped = append(result.Skipped, fmt.Sprintf("color %s in %s: already set to #%s", c.Name, themeURI, strings.ToUpper(c.Hex)))
							continue
						}
						req := &pptxmutate.UpdateThemeColorRequest{
							Package:   pkg,
							ThemeURI:  themeURI,
							ColorName: c.Name,
							HexValue:  c.Hex,
						}
						if err := pptxmutate.UpdateThemeColor(req); err != nil {
							result.Skipped = append(result.Skipped, fmt.Sprintf("color %s in %s: %v", c.Name, themeURI, err))
							continue
						}
						result.Applied.Colors = append(result.Applied.Colors, TemplateAppliedColor{
							PartURI: themeURI, ColorName: c.Name, HexValue: strings.ToUpper(c.Hex),
						})
					}
				}
			}
		}

		// Apply theme fonts.
		if plan.Fonts != nil {
			if themeErr != nil {
				result.Skipped = append(result.Skipped, "fonts: "+themeErr.Error())
			} else {
				for _, themeURI := range themeURIs {
					currentTheme, _ := pptxinspect.ParseTheme(pkg, themeURI)
					if themeFontsSame(currentTheme, plan.Fonts) {
						result.Skipped = append(result.Skipped, fmt.Sprintf("fonts in %s: already up to date", themeURI))
						continue
					}
					req := &pptxmutate.UpdateThemeFontRequest{
						Package:   pkg,
						ThemeURI:  themeURI,
						MajorFont: plan.Fonts.MajorFont,
						MinorFont: plan.Fonts.MinorFont,
					}
					if err := pptxmutate.UpdateThemeFont(req); err != nil {
						result.Skipped = append(result.Skipped, fmt.Sprintf("fonts in %s: %v", themeURI, err))
					} else {
						applied := TemplateAppliedFonts{
							PartURI:   themeURI,
							MajorFont: plan.Fonts.MajorFont,
							MinorFont: plan.Fonts.MinorFont,
						}
						if result.Applied.Fonts == nil {
							result.Applied.Fonts = &TemplateAppliedFonts{
								MajorFont: plan.Fonts.MajorFont,
								MinorFont: plan.Fonts.MinorFont,
							}
						}
						result.Applied.FontParts = append(result.Applied.FontParts, applied)
					}
				}
			}
		}

		// Apply PPTX master default text styles by role to every target master.
		if len(plan.TextStyles) > 0 {
			if targetKind != opc.PackageTypePPTX {
				result.Skipped = append(result.Skipped, "text styles: target is not a PPTX package")
			} else {
				masterURIs, masterErr := resolveTargetMasterURIs(pkg)
				if masterErr != nil {
					result.Skipped = append(result.Skipped, "text styles: "+masterErr.Error())
				} else {
					plannedStyles := representativeTextStylesByRole(plan.TextStyles, &result.Warnings)
					currentStyles := currentDefaultTextStylesByMaster(pkg)
					for _, masterURI := range masterURIs {
						for _, style := range plannedStyles {
							currentStyle := currentStyles[masterURI][style.Role]
							if sameDefaultTextStyle(currentStyle, style) {
								result.Skipped = append(result.Skipped, fmt.Sprintf("text style %s in %s: already up to date", style.Role, masterURI))
								continue
							}
							mergedStyle := mergeDefaultTextStyle(currentStyle, style)
							req := &pptxmutate.UpdateMasterDefaultTextStyleRequest{
								Package:       pkg,
								MasterPartURI: masterURI,
								StyleType:     style.Role,
								Style:         defaultTextStyleMutation(mergedStyle),
							}
							if err := pptxmutate.UpdateMasterDefaultTextStyle(req); err != nil {
								result.Skipped = append(result.Skipped, fmt.Sprintf("text style %s in %s: %v", style.Role, masterURI, err))
								continue
							}
							result.Applied.TextStyles = append(result.Applied.TextStyles, TemplateAppliedTextStyle{
								MasterPartURI:   masterURI,
								Role:            style.Role,
								SourceMasterRef: style.MasterRef,
								FontRef:         style.FontRef,
								FontName:        style.FontName,
								SizePt:          style.SizePt,
								Color:           strings.ToUpper(style.Color),
								ColorRef:        style.ColorRef,
							})
						}
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
				if chartSeriesAlreadyStyled(pkg, uri, plan.Chart) {
					result.Skipped = append(result.Skipped, fmt.Sprintf("chart %s: series 1 already has requested styling", uri))
					continue
				}
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
	result.TotalUpdates += len(result.Applied.FontParts) + len(result.Applied.TextStyles)
	sort.Strings(result.Skipped)
	sort.Strings(result.Warnings)
	return result, nil
}

// resolveTargetThemeURIs finds theme part URIs for the target document family.
// PPTX decks may reference multiple theme parts through multiple masters; apply
// to all of them so mixed-master decks do not end up half-branded.
func resolveTargetThemeURIs(pkg opc.PackageSession, kind opc.PackageType) ([]string, error) {
	switch kind {
	case opc.PackageTypePPTX:
		graph, err := pptxinspect.ParsePresentation(pkg)
		if err != nil {
			return nil, fmt.Errorf("failed to parse presentation: %v", err)
		}
		seen := map[string]bool{}
		var uris []string
		if graph != nil {
			for _, master := range graph.Masters {
				if master.ThemeURI == "" || seen[master.ThemeURI] {
					continue
				}
				seen[master.ThemeURI] = true
				uris = append(uris, master.ThemeURI)
			}
		}
		if len(uris) == 0 {
			uris = append(uris, "/ppt/theme/theme1.xml")
		}
		sort.Strings(uris)
		return uris, nil
	case opc.PackageTypeXLSX:
		wb, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return nil, fmt.Errorf("failed to parse workbook: %v", err)
		}
		if wb == nil || wb.ThemeURI == "" {
			return nil, fmt.Errorf("workbook has no theme part to apply colors/fonts to")
		}
		return []string{wb.ThemeURI}, nil
	default:
		return nil, fmt.Errorf("unsupported package type: %s", kind)
	}
}

func resolveTargetMasterURIs(pkg opc.PackageSession) ([]string, error) {
	graph, err := pptxinspect.ParsePresentation(pkg)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %v", err)
	}
	if graph == nil || len(graph.Masters) == 0 {
		return nil, fmt.Errorf("presentation has no slide masters")
	}
	seen := map[string]bool{}
	var uris []string
	for _, master := range graph.Masters {
		if master.PartURI == "" || seen[master.PartURI] {
			continue
		}
		seen[master.PartURI] = true
		uris = append(uris, master.PartURI)
	}
	sort.Strings(uris)
	return uris, nil
}

func themeColorValue(theme *pptxmodel.ThemeInfo, colorName string) string {
	if theme == nil || theme.ColorScheme == nil {
		return ""
	}
	cs := theme.ColorScheme
	switch colorName {
	case "dk1":
		return cs.Dark1
	case "lt1":
		return cs.Light1
	case "dk2":
		return cs.Dark2
	case "lt2":
		return cs.Light2
	case "accent1":
		return cs.Accent1
	case "accent2":
		return cs.Accent2
	case "accent3":
		return cs.Accent3
	case "accent4":
		return cs.Accent4
	case "accent5":
		return cs.Accent5
	case "accent6":
		return cs.Accent6
	case "hlink":
		return cs.HypLink
	case "folHlink":
		return cs.FolLink
	default:
		return ""
	}
}

func equalHex(a, b string) bool {
	return strings.EqualFold(strings.TrimPrefix(strings.TrimSpace(a), "#"), strings.TrimPrefix(strings.TrimSpace(b), "#"))
}

func themeFontsSame(theme *pptxmodel.ThemeInfo, plan *tmpl.FontPlan) bool {
	if theme == nil || theme.FontScheme == nil || plan == nil {
		return false
	}
	if plan.MajorFont != "" && theme.FontScheme.MajorFont != plan.MajorFont {
		return false
	}
	if plan.MinorFont != "" && theme.FontScheme.MinorFont != plan.MinorFont {
		return false
	}
	return true
}

func representativeTextStylesByRole(styles []tmpl.DefaultTextStyle, warnings *[]string) []tmpl.DefaultTextStyle {
	byRole := map[string]tmpl.DefaultTextStyle{}
	for _, style := range styles {
		if _, exists := byRole[style.Role]; exists {
			if warnings != nil {
				*warnings = append(*warnings, fmt.Sprintf("text styles: multiple source %s defaults found; using the first for every target master", style.Role))
			}
			continue
		}
		byRole[style.Role] = style
	}
	ordered := make([]tmpl.DefaultTextStyle, 0, len(byRole))
	for _, role := range []string{"title", "body", "other"} {
		if style, ok := byRole[role]; ok {
			ordered = append(ordered, style)
		}
	}
	return ordered
}

func currentDefaultTextStylesByMaster(pkg opc.PackageSession) map[string]map[string]tmpl.DefaultTextStyle {
	out := map[string]map[string]tmpl.DefaultTextStyle{}
	tokens, err := pptxinspect.ExtractPPTXTemplateTokens(pkg, "")
	if err != nil || tokens == nil || tokens.PPTX == nil {
		return out
	}
	for _, style := range tokens.PPTX.DefaultTextStyles {
		if out[style.MasterRef] == nil {
			out[style.MasterRef] = map[string]tmpl.DefaultTextStyle{}
		}
		out[style.MasterRef][style.Role] = style
	}
	return out
}

func defaultTextStyleMutation(style tmpl.DefaultTextStyle) *pptxmutate.DefaultTextStyleInfo {
	fontName := strings.TrimSpace(style.FontName)
	switch strings.ToLower(strings.TrimSpace(style.FontRef)) {
	case "major":
		fontName = "+mj-lt"
	case "minor":
		fontName = "+mn-lt"
	}
	color := strings.TrimSpace(style.ColorRef)
	if color == "" {
		color = strings.ToUpper(strings.TrimPrefix(strings.TrimSpace(style.Color), "#"))
	}
	return &pptxmutate.DefaultTextStyleInfo{
		StyleType: style.Role,
		FontSize:  int(style.SizePt*100 + 0.5),
		FontName:  fontName,
		Color:     color,
	}
}

func mergeDefaultTextStyle(current, desired tmpl.DefaultTextStyle) tmpl.DefaultTextStyle {
	merged := current
	merged.MasterRef = desired.MasterRef
	merged.Role = desired.Role
	if desired.SizePt > 0 {
		merged.SizePt = desired.SizePt
	}
	if desired.FontRef != "" {
		merged.FontRef = desired.FontRef
		merged.FontName = ""
	}
	if desired.FontName != "" {
		merged.FontName = desired.FontName
		merged.FontRef = ""
	}
	if desired.ColorRef != "" {
		merged.ColorRef = desired.ColorRef
		merged.Color = ""
	}
	if desired.Color != "" {
		merged.Color = strings.ToUpper(strings.TrimPrefix(strings.TrimSpace(desired.Color), "#"))
		merged.ColorRef = ""
	}
	return merged
}

func sameDefaultTextStyle(current, desired tmpl.DefaultTextStyle) bool {
	if desired.SizePt > 0 && current.SizePt != desired.SizePt {
		return false
	}
	if desired.FontRef != "" && current.FontRef != desired.FontRef {
		return false
	}
	if desired.FontName != "" && current.FontName != desired.FontName {
		return false
	}
	if desired.ColorRef != "" && current.ColorRef != desired.ColorRef {
		return false
	}
	if desired.Color != "" && !equalHex(current.Color, desired.Color) {
		return false
	}
	return desired.SizePt > 0 || desired.FontRef != "" || desired.FontName != "" || desired.ColorRef != "" || desired.Color != ""
}

func chartSeriesAlreadyStyled(pkg opc.PackageSession, chartURI string, plan *tmpl.ChartTokenPlan) bool {
	if plan == nil {
		return false
	}
	style, err := xlsxchart.InspectStyle(pkg, chartURI)
	if err != nil || style == nil || len(style.Series) == 0 {
		return false
	}
	series := style.Series[0]
	if plan.SeriesFillColor != "" && !equalHex(series.FillColor, plan.SeriesFillColor) {
		return false
	}
	if plan.SeriesLineColor != "" && !equalHex(series.LineColor, plan.SeriesLineColor) {
		return false
	}
	return plan.SeriesFillColor != "" || plan.SeriesLineColor != ""
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
		if c.PartURI != "" {
			fmt.Fprintf(&b, "    - %s %s = #%s\n", c.PartURI, c.ColorName, c.HexValue)
		} else {
			fmt.Fprintf(&b, "    - %s = #%s\n", c.ColorName, c.HexValue)
		}
	}
	if r.Applied.Fonts != nil {
		fmt.Fprintf(&b, "  Applied fonts: major=%s minor=%s\n",
			r.Applied.Fonts.MajorFont, r.Applied.Fonts.MinorFont)
		for _, f := range r.Applied.FontParts {
			fmt.Fprintf(&b, "    - %s major=%s minor=%s\n", f.PartURI, f.MajorFont, f.MinorFont)
		}
	}
	fmt.Fprintf(&b, "  Applied text styles (%d):\n", len(r.Applied.TextStyles))
	for _, s := range r.Applied.TextStyles {
		fmt.Fprintf(&b, "    - %s %s", s.MasterPartURI, s.Role)
		if s.SizePt > 0 {
			fmt.Fprintf(&b, " size=%.2fpt", s.SizePt)
		}
		if s.FontRef != "" {
			fmt.Fprintf(&b, " fontRef=%s", s.FontRef)
		} else if s.FontName != "" {
			fmt.Fprintf(&b, " font=%s", s.FontName)
		}
		if s.ColorRef != "" {
			fmt.Fprintf(&b, " colorRef=%s", s.ColorRef)
		} else if s.Color != "" {
			fmt.Fprintf(&b, " color=#%s", s.Color)
		}
		fmt.Fprintln(&b)
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
	templateApplyCmd.Flags().BoolVar(&templateApplyTextStyles, "target-text-styles", false,
		"apply PPTX master default text styles by role")
	templateApplyCmd.Flags().BoolVar(&templateApplyRanges, "target-ranges", false,
		"(unsupported) range/cell style transfer; reported as skipped")

	AddMutationFlags(templateApplyCmd)
	templateGroupCmd.AddCommand(templateApplyCmd)
}
