package cli

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

var templateTokensFor string

// templateGroupCmd is a top-level command group for cross-family template
// (design-transfer) operations. It is distinct from `ooxml pptx template`,
// which manages PPTX template manifests; this group spans PPTX and XLSX.
var templateGroupCmd = &cobra.Command{
	Use:   "template",
	Short: "Extract and apply design tokens across PPTX/XLSX templates",
	Long: `Cross-family design-token commands.

The 'tokens' subcommand reads a deterministic TemplateTokens JSON contract
(theme colors, fonts, default text styles, table styles, chart and named cell
styles) from a PPTX/POTX or XLSX/XLTX file. The output is a stable foundation
for design-application commands.`,
	Args: cobra.NoArgs,
	RunE: showHelp,
}

var templateTokensCmd = &cobra.Command{
	Use:   "tokens <file>",
	Short: "Extract design tokens from a PPTX/POTX or XLSX/XLTX template",
	Long: `Extract practical design tokens from a template into deterministic JSON.

The package family is auto-detected; override with --for pptx|xlsx. Output is a
TemplateTokens document (schemaVersion ` + tmpl.SchemaVersion + `) with theme
colors/fonts plus family-specific tokens:
  PPTX: per-master default text styles, table style ids, chart style summaries
  XLSX: theme, named cell styles, chart style summaries

This is a read-only inspection; it never modifies the input file.

Examples:
  ooxml --json template tokens brand-deck.pptx
  ooxml template tokens report-template.xlsx --format text
  ooxml --json template tokens deck.potx --output tokens.json`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		kind, err := resolveTemplateTokensKind(filePath, templateTokensFor)
		if err != nil {
			return err
		}

		tokens, err := extractTemplateTokens(filePath, kind)
		if err != nil {
			return err
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, tokens)
		}
		return writeGlobalOutput(cmd, []byte(renderTemplateTokensText(tokens)))
	},
}

// resolveTemplateTokensKind decides whether to treat the file as PPTX or XLSX,
// honoring an explicit --for and otherwise detecting from package contents.
func resolveTemplateTokensKind(filePath, forFlag string) (opc.PackageType, error) {
	switch strings.ToLower(strings.TrimSpace(forFlag)) {
	case "pptx":
		return opc.PackageTypePPTX, nil
	case "xlsx":
		return opc.PackageTypeXLSX, nil
	case "", "auto":
		// fall through to detection
	default:
		return "", InvalidArgsError("--for must be one of: pptx, xlsx, auto")
	}

	pkg, err := opc.Open(filePath)
	if err != nil {
		return "", FileNotFoundError(filePath)
	}
	defer pkg.Close()

	detected := opc.DetectType(pkg)
	if detected != opc.PackageTypePPTX && detected != opc.PackageTypeXLSX {
		return "", NewCLIErrorf(ExitUnsupportedType,
			"template tokens supports PPTX/POTX and XLSX/XLTX files (detected: %s); pass --for to override",
			detected)
	}
	return detected, nil
}

func extractTemplateTokens(filePath string, kind opc.PackageType) (*tmpl.TemplateTokens, error) {
	pkg, err := openPackageExpectType(filePath, kind)
	if err != nil {
		return nil, err
	}
	defer pkg.Close()

	source := filepath.Base(filePath)
	switch kind {
	case opc.PackageTypePPTX:
		tokens, err := pptxinspect.ExtractPPTXTemplateTokens(pkg, source)
		if err != nil {
			return nil, NewCLIErrorf(ExitUnexpected, "failed to extract PPTX tokens: %v", err)
		}
		// Chart summaries are wired here (not in the extractor) to keep
		// pkg/pptx/inspect free of an xlsx/chart import cycle.
		if tokens.PPTX != nil {
			tokens.PPTX.ChartStyles = xlsxchart.SummarizeChartStyles(pkg, "/ppt/charts/chart")
		}
		return tokens, nil
	case opc.PackageTypeXLSX:
		tokens, err := xlsxinspect.ExtractXLSXTemplateTokens(pkg, source)
		if err != nil {
			return nil, NewCLIErrorf(ExitUnexpected, "failed to extract XLSX tokens: %v", err)
		}
		if tokens.XLSX != nil {
			tokens.XLSX.ChartStyles = xlsxchart.SummarizeChartStyles(pkg, "/xl/charts/chart")
		}
		return tokens, nil
	default:
		return nil, NewCLIErrorf(ExitUnsupportedType, "unsupported package type: %s", kind)
	}
}

func renderTemplateTokensText(t *tmpl.TemplateTokens) string {
	var b strings.Builder
	fmt.Fprintf(&b, "Template Tokens (schema %s)\n", t.SchemaVersion)
	fmt.Fprintf(&b, "  Source: %s\n", t.Source)
	fmt.Fprintf(&b, "  Type:   %s\n", t.Type)

	if t.PPTX != nil {
		renderThemeText(&b, t.PPTX.Theme)
		fmt.Fprintf(&b, "  Default text styles (%d):\n", len(t.PPTX.DefaultTextStyles))
		for _, ds := range t.PPTX.DefaultTextStyles {
			font := ds.FontName
			if font == "" && ds.FontRef != "" {
				font = "theme:" + ds.FontRef
			}
			color := ds.Color
			if color == "" && ds.ColorRef != "" {
				color = "theme:" + ds.ColorRef
			}
			fmt.Fprintf(&b, "    - %s [%s]: font=%s size=%gpt color=%s\n",
				ds.Role, filepath.Base(ds.MasterRef), font, ds.SizePt, color)
		}
		fmt.Fprintf(&b, "  Table styles (%d):\n", len(t.PPTX.TableStyles))
		for _, ts := range t.PPTX.TableStyles {
			name := ts.Name
			if name == "" {
				name = "(unnamed)"
			}
			fmt.Fprintf(&b, "    - %s %s\n", ts.StyleID, name)
		}
		renderChartStylesText(&b, t.PPTX.ChartStyles)
	}

	if t.XLSX != nil {
		renderThemeText(&b, t.XLSX.Theme)
		fmt.Fprintf(&b, "  Named cell styles (%d):\n", len(t.XLSX.NamedCellStyles))
		for _, ncs := range t.XLSX.NamedCellStyles {
			fmt.Fprintf(&b, "    - %s: font=%s size=%gpt bold=%t color=%s fill=%s numFmt=%q\n",
				ncs.Name, ncs.FontName, ncs.SizePt, ncs.Bold, ncs.Color, ncs.FillColor, ncs.NumberFormatCode)
		}
		renderChartStylesText(&b, t.XLSX.ChartStyles)
	}

	return strings.TrimRight(b.String(), "\n")
}

func renderThemeText(b *strings.Builder, theme *pptxmodel.ThemeInfo) {
	if theme == nil {
		fmt.Fprintf(b, "  Theme: (none)\n")
		return
	}
	fmt.Fprintf(b, "  Theme: %s\n", theme.Name)
	if theme.ColorScheme != nil {
		c := theme.ColorScheme
		fmt.Fprintf(b, "    Colors: dk1=%s lt1=%s dk2=%s lt2=%s accent1=%s accent2=%s accent3=%s accent4=%s accent5=%s accent6=%s hlink=%s folHlink=%s\n",
			c.Dark1, c.Light1, c.Dark2, c.Light2,
			c.Accent1, c.Accent2, c.Accent3, c.Accent4, c.Accent5, c.Accent6,
			c.HypLink, c.FolLink)
	}
	if theme.FontScheme != nil {
		fmt.Fprintf(b, "    Fonts: major=%s minor=%s\n", theme.FontScheme.MajorFont, theme.FontScheme.MinorFont)
	}
}

func renderChartStylesText(b *strings.Builder, summaries []tmpl.ChartStyleSummary) {
	fmt.Fprintf(b, "  Chart styles (%d):\n", len(summaries))
	// stable order already guaranteed by extractor; defensively sort by partUri
	sorted := make([]tmpl.ChartStyleSummary, len(summaries))
	copy(sorted, summaries)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i].PartURI < sorted[j].PartURI })
	for _, cs := range sorted {
		fmt.Fprintf(b, "    - %s: type=%s seriesFill=%s seriesLine=%s titleFont=%s\n",
			filepath.Base(cs.PartURI), cs.ChartType, cs.SeriesFillColor, cs.SeriesLineColor, cs.TitleFontFamily)
	}
}

func init() {
	templateTokensCmd.Flags().StringVar(&templateTokensFor, "for", "auto",
		"package family: pptx, xlsx, or auto (default: auto-detect)")
	templateGroupCmd.AddCommand(templateTokensCmd)
	rootCmd.AddCommand(templateGroupCmd)
}
