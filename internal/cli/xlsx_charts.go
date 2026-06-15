package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/spf13/cobra"
)

type XLSXChartsResult struct {
	File            string          `json:"file"`
	ValidateCommand string          `json:"validateCommand,omitempty"`
	Charts          []XLSXChartItem `json:"charts"`
}

type XLSXChartItem struct {
	model.ChartRef
	ShowCommand          string                         `json:"showCommand,omitempty"`
	SourceExportCommands []XLSXChartSourceExportCommand `json:"sourceExportCommands,omitempty"`
	Style                *xlsxchart.ChartStyle          `json:"style,omitempty"`
}

type XLSXChartSourceExportCommand struct {
	Series              int    `json:"series"`
	Role                string `json:"role"`
	Formula             string `json:"formula,omitempty"`
	Sheet               string `json:"sheet,omitempty"`
	Range               string `json:"range,omitempty"`
	RangesExportCommand string `json:"rangesExportCommand,omitempty"`
}

var (
	xlsxChartsListSheet string
	xlsxChartsShowSheet string
	xlsxChartsShowChart string
)

var xlsxChartsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List workbook charts",
	Long:  "List existing XLSX worksheet-embedded chart definitions, stable selectors, and source ranges.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		charts, err := loadXLSXChartsForCLI(filePath, xlsxChartsListSheet)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXChartsJSON(cmd, filePath, charts)
		}
		return outputXLSXChartsText(cmd, charts)
	},
}

var xlsxChartsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show chart metadata",
	Long:  "Show one XLSX worksheet chart, including anchors, chart types, source formulas, cached previews, and part metadata.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		charts, err := loadXLSXChartsForCLI(filePath, xlsxChartsShowSheet)
		if err != nil {
			return err
		}
		selected, err := selectXLSXChart(charts, xlsxChartsShowChart)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXChartsJSON(cmd, filePath, []model.ChartRef{selected})
		}
		return outputXLSXChartsText(cmd, []model.ChartRef{selected})
	},
}

func loadXLSXChartsForCLI(filePath, sheetSelector string) ([]model.ChartRef, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheets := workbook.Sheets
	if sheetSelector != "" {
		selected, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return nil, err
		}
		if err := requireXLSXWorksheetRef(selected); err != nil {
			return nil, err
		}
		sheets = []model.SheetRef{selected}
	}
	charts, err := xlsxchart.List(pkg, workbook, sheets)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to list charts: %v", err)
	}
	return charts, nil
}

// inspectXLSXChartStyles reads the practical style of each chart part for JSON
// readback. It is best-effort: a chart whose style cannot be read is simply
// omitted rather than failing the whole list/show.
func inspectXLSXChartStyles(filePath string, charts []model.ChartRef) map[string]*xlsxchart.ChartStyle {
	partURIs := make([]string, 0, len(charts))
	for _, chart := range charts {
		partURIs = append(partURIs, chart.PartURI)
	}
	return inspectChartStylesByPart(filePath, opc.PackageTypeXLSX, partURIs)
}

func selectXLSXChart(charts []model.ChartRef, selector string) (model.ChartRef, error) {
	if len(charts) == 0 {
		return model.ChartRef{}, NewCLIErrorf(ExitInvalidArgs, "workbook has no charts")
	}
	selector = strings.TrimSpace(selector)
	if selector == "" {
		if len(charts) == 1 {
			return model.WithChartSelectors(charts[0]), nil
		}
		return model.ChartRef{}, InvalidArgsError("--chart is required when workbook has multiple charts")
	}
	var matches []model.ChartRef
	for _, chartRef := range charts {
		withSelectors := model.WithChartSelectors(chartRef)
		if model.SelectorMatches(withSelectors.Selectors, selector) {
			matches = append(matches, withSelectors)
		}
	}
	if len(matches) == 1 {
		return matches[0], nil
	}
	if len(matches) > 1 {
		candidates := make([]string, 0, len(matches))
		for _, match := range matches {
			candidates = append(candidates, match.PrimarySelector)
		}
		return model.ChartRef{}, NewCLIErrorf(ExitInvalidArgs, "chart selector %q matched multiple charts (%s); use a more specific selector", selector, strings.Join(candidates, ", "))
	}
	if number, err := strconv.Atoi(selector); err == nil {
		if number < 1 || number > len(charts) {
			return model.ChartRef{}, NewCLIErrorf(ExitTargetNotFound, "chart %d is out of range (1-%d)", number, len(charts))
		}
		return model.WithChartSelectors(charts[number-1]), nil
	}
	candidates := chartSelectorCandidates(charts)
	return model.ChartRef{}, SelectorNotFoundError("chart", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), "ooxml --json xlsx charts list <file>")
}

func chartSelectorCandidates(charts []model.ChartRef) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(charts))
	for _, chartRef := range charts {
		withSelectors := model.WithChartSelectors(chartRef)
		out = append(out, SelectorCandidate{Primary: withSelectors.PrimarySelector, Selectors: withSelectors.Selectors})
	}
	return out
}

func outputXLSXChartsJSON(cmd *cobra.Command, filePath string, charts []model.ChartRef) error {
	config := GetGlobalConfig(cmd)
	styles := inspectXLSXChartStyles(filePath, charts)
	items := make([]XLSXChartItem, 0, len(charts))
	for _, chart := range charts {
		item := xlsxChartItem(filePath, chart)
		item.Style = styles[chart.PartURI]
		items = append(items, item)
	}
	result := XLSXChartsResult{File: filePath, ValidateCommand: xlsxValidateCommand(filePath), Charts: items}
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal charts JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXChartsText(cmd *cobra.Command, charts []model.ChartRef) error {
	if len(charts) == 0 {
		return writeXLSXOutput(cmd, []byte("no charts found"))
	}

	out := ""
	for i, chartRef := range charts {
		if i > 0 {
			out += "\n"
		}
		out += fmt.Sprintf("[%d] %s\n", chartRef.Number, chartDisplayName(chartRef))
		out += fmt.Sprintf("  sheet: %s (%d)\n", chartRef.Sheet, chartRef.SheetNumber)
		if chartRef.Title != "" {
			out += fmt.Sprintf("  title: %s\n", chartRef.Title)
		}
		if len(chartRef.Types) > 0 {
			out += fmt.Sprintf("  types: %s\n", strings.Join(chartRef.Types, ", "))
		}
		if chartRef.Anchor != nil && chartRef.Anchor.Type != "" {
			out += fmt.Sprintf("  anchor: %s\n", chartRef.Anchor.Type)
		}
		out += fmt.Sprintf("  series: %d\n", len(chartRef.Series))
		for _, series := range chartRef.Series {
			out += fmt.Sprintf("    [%d] %s\n", series.Number, chartSeriesText(series))
		}
		out += fmt.Sprintf("  part: %s\n", chartRef.PartURI)
	}
	return writeXLSXOutput(cmd, []byte(out))
}

func xlsxChartItem(filePath string, chart model.ChartRef) XLSXChartItem {
	chart = model.WithChartSelectors(chart)
	chartSelector := xlsxChartSelector(chart)
	sheetSelector := xlsxChartSheetSelector(chart)
	return XLSXChartItem{
		ChartRef:             chart,
		ShowCommand:          xlsxChartShowCommand(filePath, sheetSelector, chartSelector),
		SourceExportCommands: xlsxChartSourceExportCommands(filePath, chart),
	}
}

func xlsxChartSourceExportCommands(filePath string, chart model.ChartRef) []XLSXChartSourceExportCommand {
	var commands []XLSXChartSourceExportCommand
	for _, series := range chart.Series {
		for _, source := range chartSeriesSources(series) {
			if source.Ref == nil || source.Ref.Sheet == "" || source.Ref.Range == "" {
				continue
			}
			commands = append(commands, XLSXChartSourceExportCommand{
				Series:              series.Number,
				Role:                source.Role,
				Formula:             source.Ref.Formula,
				Sheet:               source.Ref.Sheet,
				Range:               source.Ref.Range,
				RangesExportCommand: xlsxRangesExportCommand(filePath, source.Ref.Sheet, source.Ref.Range),
			})
		}
	}
	return commands
}

type chartSeriesSource struct {
	Role string
	Ref  *model.ChartDataSourceRef
}

func chartSeriesSources(series model.ChartSeriesRef) []chartSeriesSource {
	return []chartSeriesSource{
		{Role: "name", Ref: series.Name},
		{Role: "categories", Ref: series.Categories},
		{Role: "values", Ref: series.Values},
		{Role: "xValues", Ref: series.XValues},
		{Role: "yValues", Ref: series.YValues},
		{Role: "bubbleSize", Ref: series.BubbleSize},
	}
}

func chartSeriesText(series model.ChartSeriesRef) string {
	parts := []string{}
	for _, source := range chartSeriesSources(series) {
		if source.Ref == nil {
			continue
		}
		text := chartSourceText(source.Role, source.Ref)
		if text != "" {
			parts = append(parts, text)
		}
	}
	if len(parts) == 0 {
		return "(no source formulas)"
	}
	return strings.Join(parts, "; ")
}

func chartSourceText(role string, source *model.ChartDataSourceRef) string {
	if source == nil {
		return ""
	}
	if source.Sheet != "" && source.Range != "" {
		return fmt.Sprintf("%s=%s!%s", role, source.Sheet, source.Range)
	}
	if source.Formula != "" {
		return fmt.Sprintf("%s=%s", role, source.Formula)
	}
	if len(source.CachePreview) > 0 {
		return fmt.Sprintf("%s=%s", role, strings.Join(source.CachePreview, ", "))
	}
	return ""
}

func xlsxChartSelector(chart model.ChartRef) string {
	if chart.PrimarySelector != "" {
		return chart.PrimarySelector
	}
	if chart.Name != "" {
		return chart.Name
	}
	if chart.Number > 0 {
		return fmt.Sprintf("chart:%d", chart.Number)
	}
	return "1"
}

func xlsxChartSheetSelector(chart model.ChartRef) string {
	if chart.Sheet != "" {
		return chart.Sheet
	}
	if chart.SheetNumber > 0 {
		return fmt.Sprintf("sheet:%d", chart.SheetNumber)
	}
	return ""
}

func xlsxChartShowCommand(filePath, sheetSelector, chartSelector string) string {
	args := []string{"ooxml", "--json", "xlsx", "charts", "show", pptxXLSXCommandArg(filePath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--chart", chartSelector)
	return strings.Join(args, " ")
}

func chartDisplayName(chart model.ChartRef) string {
	if strings.TrimSpace(chart.Name) != "" {
		return chart.Name
	}
	if strings.TrimSpace(chart.Title) != "" {
		return chart.Title
	}
	if chart.Number > 0 {
		return fmt.Sprintf("chart:%d", chart.Number)
	}
	return "(unnamed)"
}

func init() {
	xlsxChartsListCmd.Flags().StringVar(&xlsxChartsListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxChartsShowCmd.Flags().StringVar(&xlsxChartsShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxChartsShowCmd.Flags().StringVar(&xlsxChartsShowChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	xlsxChartsCmd.AddCommand(xlsxChartsListCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsShowCmd)
}
