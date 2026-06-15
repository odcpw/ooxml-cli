package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxchart "github.com/ooxml-cli/ooxml-cli/pkg/pptx/chart"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxmodel "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/spf13/cobra"
)

type PPTXChartsResult struct {
	File            string                `json:"file"`
	ValidateCommand string                `json:"validateCommand,omitempty"`
	Charts          []PPTXChartResultItem `json:"charts"`
}

type PPTXChartResultItem struct {
	pptxchart.ChartRef
	ShowCommand string                `json:"showCommand,omitempty"`
	Style       *xlsxchart.ChartStyle `json:"style,omitempty"`
}

var chartsCmd = &cobra.Command{
	Use:     "charts",
	Aliases: []string{"chart"},
	Short:   "Inspect and mutate slide charts",
	Long:    "Commands for inspecting and updating existing PPTX slide charts.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var (
	pptxChartsListSlide int
	pptxChartsShowSlide int
	pptxChartsShowChart string
)

var pptxChartsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List PPTX slide charts",
	Long:  "List existing PPTX slide charts, stable selectors, source formulas, cached previews, and embedded workbook links.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		charts, err := loadPPTXChartsForCLI(filePath, pptxChartsListSlide)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXChartsJSON(cmd, filePath, charts)
		}
		return outputPPTXChartsText(cmd, charts)
	},
}

var pptxChartsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show PPTX chart metadata",
	Long:  "Show one PPTX slide chart, including chart types, series source formulas, cached previews, part metadata, and embedded workbook link.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		charts, err := loadPPTXChartsForCLI(filePath, pptxChartsShowSlide)
		if err != nil {
			return err
		}
		selected, err := selectPPTXChart(charts, pptxChartsShowChart)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXChartsJSON(cmd, filePath, []pptxchart.ChartRef{selected})
		}
		return outputPPTXChartsText(cmd, []pptxchart.ChartRef{selected})
	},
}

func loadPPTXChartsForCLI(filePath string, slide int) ([]pptxchart.ChartRef, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	defer pkg.Close()
	charts, err := pptxchart.List(pkg, slide)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to list PPTX charts: %v", err)
	}
	return charts, nil
}

// inspectPPTXChartStyles reads the practical style of each chart part for JSON
// readback, best-effort (a chart whose style cannot be read is omitted).
func inspectPPTXChartStyles(filePath string, charts []pptxchart.ChartRef) map[string]*xlsxchart.ChartStyle {
	partURIs := make([]string, 0, len(charts))
	for _, chart := range charts {
		partURIs = append(partURIs, chart.PartURI)
	}
	return inspectChartStylesByPart(filePath, opc.PackageTypePPTX, partURIs)
}

func selectPPTXChart(charts []pptxchart.ChartRef, selector string) (pptxchart.ChartRef, error) {
	selected, err := pptxchart.Select(charts, selector)
	if err == nil {
		return selected, nil
	}
	message := err.Error()
	if strings.Contains(message, "not found") || strings.Contains(message, "out of range") || strings.Contains(message, "no charts") {
		candidates := BuildSelectorCandidates(pptxChartSelectorCandidates(charts), selector, maxSelectorCandidates)
		discovery := "ooxml --json pptx charts list <file>"
		if len(candidates) > 0 {
			return pptxchart.ChartRef{}, NewCLIError(ExitTargetNotFound, fmt.Sprintf("%s; did you mean: %s; discover with `%s`", message, strings.Join(candidates, ", "), discovery))
		}
		return pptxchart.ChartRef{}, TargetNotFoundError(message + "; discover charts with `" + discovery + "`")
	}
	return pptxchart.ChartRef{}, NewCLIErrorf(ExitInvalidArgs, "%s", message)
}

func pptxChartSelectorCandidates(charts []pptxchart.ChartRef) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(charts))
	for _, chartRef := range charts {
		withSelectors := pptxchart.WithSelectors(chartRef)
		out = append(out, SelectorCandidate{Primary: withSelectors.PrimarySelector, Selectors: withSelectors.Selectors})
	}
	return out
}

func outputPPTXChartsJSON(cmd *cobra.Command, filePath string, charts []pptxchart.ChartRef) error {
	config := GetGlobalConfig(cmd)
	styles := inspectPPTXChartStyles(filePath, charts)
	items := make([]PPTXChartResultItem, 0, len(charts))
	for _, chart := range charts {
		item := pptxChartItem(filePath, chart)
		item.Style = styles[chart.PartURI]
		items = append(items, item)
	}
	result := PPTXChartsResult{File: filePath, ValidateCommand: pptxValidateCommand(filePath), Charts: items}
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal PPTX charts JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputPPTXChartsText(cmd *cobra.Command, charts []pptxchart.ChartRef) error {
	if len(charts) == 0 {
		return writeCLIOutput(cmd, []byte("no charts found"))
	}
	var out strings.Builder
	for i, chart := range charts {
		if i > 0 {
			out.WriteString("\n")
		}
		out.WriteString(fmt.Sprintf("[%d] %s\n", chart.Number, pptxChartDisplayName(chart)))
		out.WriteString(fmt.Sprintf("  slide: %d\n", chart.Slide))
		if len(chart.Types) > 0 {
			out.WriteString(fmt.Sprintf("  types: %s\n", strings.Join(chart.Types, ", ")))
		}
		out.WriteString(fmt.Sprintf("  series: %d\n", len(chart.Series)))
		for _, series := range chart.Series {
			out.WriteString(fmt.Sprintf("    [%d] %s\n", series.Number, pptxChartSeriesText(series)))
		}
		if chart.EmbeddedWorkbookPartURI != "" {
			out.WriteString(fmt.Sprintf("  embeddedWorkbook: %s\n", chart.EmbeddedWorkbookPartURI))
		}
		out.WriteString(fmt.Sprintf("  part: %s\n", chart.PartURI))
	}
	return writeCLIOutput(cmd, []byte(out.String()))
}

func pptxChartItem(filePath string, chart pptxchart.ChartRef) PPTXChartResultItem {
	chart = pptxchart.WithSelectors(chart)
	return PPTXChartResultItem{
		ChartRef:    chart,
		ShowCommand: pptxChartShowCommand(filePath, chart.Slide, pptxChartGeneratedSelector(chart)),
	}
}

func pptxChartSelector(chart pptxchart.ChartRef) string {
	if chart.PrimarySelector != "" {
		return chart.PrimarySelector
	}
	if chart.Number > 0 {
		return fmt.Sprintf("chart:%d", chart.Number)
	}
	return "chart:1"
}

func pptxChartGeneratedSelector(chart pptxchart.ChartRef) string {
	if chart.PartURI != "" {
		return "part:" + chart.PartURI
	}
	if chart.ShapeID != "" {
		return "shape:" + chart.ShapeID
	}
	return pptxChartSelector(chart)
}

func pptxChartShowCommand(filePath string, slide int, chartSelector string) string {
	command := fmt.Sprintf("ooxml --json pptx charts show %s", pptxXLSXCommandArg(filePath))
	if slide > 0 {
		command += " --slide " + strconv.Itoa(slide)
	}
	if strings.TrimSpace(chartSelector) != "" {
		command += " --chart " + pptxXLSXCommandArg(chartSelector)
	}
	return command
}

func pptxChartDisplayName(chart pptxchart.ChartRef) string {
	if strings.TrimSpace(chart.ShapeName) != "" {
		return chart.ShapeName
	}
	if strings.TrimSpace(chart.Title) != "" {
		return chart.Title
	}
	if chart.Number > 0 {
		return fmt.Sprintf("chart:%d", chart.Number)
	}
	return "(unnamed)"
}

func pptxChartSeriesText(series pptxchart.SeriesRef) string {
	parts := []string{}
	for _, source := range []struct {
		role string
		ref  string
	}{
		{"name", pptxChartSourceText(series.Name)},
		{"categories", pptxChartSourceText(series.Categories)},
		{"values", pptxChartSourceText(series.Values)},
		{"xValues", pptxChartSourceText(series.XValues)},
		{"yValues", pptxChartSourceText(series.YValues)},
		{"bubbleSize", pptxChartSourceText(series.BubbleSize)},
	} {
		if source.ref != "" {
			parts = append(parts, source.role+"="+source.ref)
		}
	}
	if len(parts) == 0 {
		return "(no sources)"
	}
	return strings.Join(parts, "; ")
}

func pptxChartSourceText(ref *xlsxmodel.ChartDataSourceRef) string {
	if ref == nil {
		return ""
	}
	if ref.Sheet != "" && ref.Range != "" {
		return ref.Sheet + "!" + ref.Range
	}
	if ref.Formula != "" {
		return ref.Formula
	}
	if len(ref.CachePreview) > 0 {
		return strings.Join(ref.CachePreview, ", ")
	}
	return ""
}

func init() {
	pptxChartsListCmd.Flags().IntVar(&pptxChartsListSlide, "slide", 0, "1-based slide number to search")
	pptxChartsShowCmd.Flags().IntVar(&pptxChartsShowSlide, "slide", 0, "1-based slide number to search")
	pptxChartsShowCmd.Flags().StringVar(&pptxChartsShowChart, "chart", "", "chart selector from pptx charts list")
	chartsCmd.AddCommand(pptxChartsListCmd)
	chartsCmd.AddCommand(pptxChartsShowCmd)
}
