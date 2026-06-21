package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	"github.com/spf13/cobra"
)

var (
	xlsxChartsSetPlotAreaFillSheet  string
	xlsxChartsSetPlotAreaFillChart  string
	xlsxChartsSetPlotAreaFillColor  string
	xlsxChartsSetPlotAreaFillExpect string
)

var xlsxChartsSetPlotAreaFillCmd = &cobra.Command{
	Use:   "set-plot-area-fill <file>",
	Short: "Set or clear the plot-area background fill of a worksheet chart",
	Long:  "Set the solid fill color of a worksheet chart's plot area, or clear it with --fill-color none.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runXLSXChartSetFill(cmd, args[0], xlsxChartsSetPlotAreaFillSheet, xlsxChartsSetPlotAreaFillChart,
			xlsxChartsSetPlotAreaFillColor, xlsxChartsSetPlotAreaFillExpect, "plot-area", "xlsx.chart.set-plot-area-fill",
			func(req *xlsxchart.SetFillRequest) (*xlsxchart.SetFillResult, error) {
				return xlsxchart.SetPlotAreaFill(req)
			})
	},
}

var (
	xlsxChartsSetChartAreaFillSheet  string
	xlsxChartsSetChartAreaFillChart  string
	xlsxChartsSetChartAreaFillColor  string
	xlsxChartsSetChartAreaFillExpect string
)

var xlsxChartsSetChartAreaFillCmd = &cobra.Command{
	Use:   "set-chart-area-fill <file>",
	Short: "Set or clear the chart-area (background) fill of a worksheet chart",
	Long:  "Set the solid fill color of a worksheet chart's chart-area background, or clear it with --fill-color none.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runXLSXChartSetFill(cmd, args[0], xlsxChartsSetChartAreaFillSheet, xlsxChartsSetChartAreaFillChart,
			xlsxChartsSetChartAreaFillColor, xlsxChartsSetChartAreaFillExpect, "chart-area", "xlsx.chart.set-chart-area-fill",
			func(req *xlsxchart.SetFillRequest) (*xlsxchart.SetFillResult, error) {
				return xlsxchart.SetChartAreaFill(req)
			})
	},
}

// runXLSXChartSetFill is the shared body of the worksheet plot/chart-area fill
// commands; only the area label, action, and setter differ.
func runXLSXChartSetFill(cmd *cobra.Command, filePath, sheetSel, chartSel, fillColor, expectFill, area, action string, setter func(*xlsxchart.SetFillRequest) (*xlsxchart.SetFillResult, error)) error {
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	if !cmd.Flags().Changed("fill-color") {
		return InvalidArgsError("--fill-color is required (a #RRGGBB color or none)")
	}
	color, noFill, err := resolveChartFillColor(fillColor)
	if err != nil {
		return err
	}
	var expect *string
	if cmd.Flags().Changed("expect-fill") {
		v, perr := resolveChartExpectFill(expectFill)
		if perr != nil {
			return perr
		}
		expect = &v
	}
	var previousFill, newFill string
	result, err := runXLSXChartStyleMutation(cmd, filePath, sheetSel, chartSel, action, func(pkg opc.PackageSession, chartURI string) error {
		res, serr := setter(&xlsxchart.SetFillRequest{
			Package:    pkg,
			ChartURI:   chartURI,
			FillColor:  color,
			NoFill:     noFill,
			ExpectFill: expect,
		})
		if serr != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", serr)
		}
		previousFill = res.PreviousFill
		newFill = res.NewFill
		return nil
	})
	if err != nil {
		return err
	}
	result.PreviousFill = previousFill
	result.NewFill = newFill
	summary := fmt.Sprintf("%s fill set to %s", area, fillDisplay(newFill))
	return outputXLSXChartStyleResult(cmd, result, summary)
}

func fillDisplay(fill string) string {
	if fill == "" {
		return "none"
	}
	return "#" + fill
}

var (
	xlsxChartsCopyStyleSheet       string
	xlsxChartsCopyStyleChart       string
	xlsxChartsCopyStyleToChart     string
	xlsxChartsCopyStyleFrom        string
	xlsxChartsCopyStyleFromChart   string
	xlsxChartsCopyStyleExpectCount int
)

var xlsxChartsCopyStyleCmd = &cobra.Command{
	Use:   "copy-style <file>",
	Short: "Copy a template chart's practical style onto a worksheet chart",
	Long: `Read the style of a template chart and apply its practical title/legend/axis/
series/fill/font defaults to a target worksheet chart.

Only STYLE is copied (fonts, fills, colors, legend position, gridlines, number
formats, marker/line defaults). Content (title text, axis title text, series
names and data) is left untouched on the target.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if !cmd.Flags().Changed("from") {
			return InvalidArgsError("--from <template-file> is required")
		}
		if _, err := os.Stat(xlsxChartsCopyStyleFrom); err != nil {
			return FileNotFoundError(xlsxChartsCopyStyleFrom)
		}
		source, err := readXLSXTemplateChartStyle(xlsxChartsCopyStyleFrom, chartStyleSelectorOrDefault(xlsxChartsCopyStyleFromChart))
		if err != nil {
			return err
		}
		var expectCount *int
		if cmd.Flags().Changed("expect-series-count") {
			if xlsxChartsCopyStyleExpectCount < 0 {
				return InvalidArgsError("--expect-series-count must be >= 0")
			}
			v := xlsxChartsCopyStyleExpectCount
			expectCount = &v
		}
		// --to-chart is an alias for --chart (the target chart selector); --chart
		// keeps parity with the other set-* commands, --to-chart matches the
		// copy-style spec phrasing.
		targetSel := xlsxChartsCopyStyleChart
		if cmd.Flags().Changed("to-chart") {
			if cmd.Flags().Changed("chart") {
				return InvalidArgsError("use --chart or --to-chart, not both")
			}
			targetSel = xlsxChartsCopyStyleToChart
		}
		var applied []string
		result, err := runXLSXChartStyleMutation(cmd, filePath, xlsxChartsCopyStyleSheet, targetSel, "xlsx.chart.copy-style", func(pkg opc.PackageSession, chartURI string) error {
			res, aerr := xlsxchart.ApplyStyle(&xlsxchart.ApplyStyleRequest{
				Package:           pkg,
				ChartURI:          chartURI,
				Source:            source,
				ExpectSeriesCount: expectCount,
			})
			if aerr != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%v", aerr)
			}
			applied = res.Applied
			return nil
		})
		if err != nil {
			return err
		}
		result.AppliedStyle = applied
		return outputXLSXChartStyleResult(cmd, result, fmt.Sprintf("copied %d style facet(s) from template", len(applied)))
	},
}

func init() {
	p := xlsxChartsSetPlotAreaFillCmd.Flags()
	p.StringVar(&xlsxChartsSetPlotAreaFillSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	p.StringVar(&xlsxChartsSetPlotAreaFillChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	p.StringVar(&xlsxChartsSetPlotAreaFillColor, "fill-color", "", "plot-area fill color as #RRGGBB, or none to clear")
	p.StringVar(&xlsxChartsSetPlotAreaFillExpect, "expect-fill", "", "guard: require the current fill to match (#RRGGBB, scheme:<name>, or none)")
	AddMutationFlags(xlsxChartsSetPlotAreaFillCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsSetPlotAreaFillCmd)

	c := xlsxChartsSetChartAreaFillCmd.Flags()
	c.StringVar(&xlsxChartsSetChartAreaFillSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	c.StringVar(&xlsxChartsSetChartAreaFillChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	c.StringVar(&xlsxChartsSetChartAreaFillColor, "fill-color", "", "chart-area fill color as #RRGGBB, or none to clear")
	c.StringVar(&xlsxChartsSetChartAreaFillExpect, "expect-fill", "", "guard: require the current fill to match (#RRGGBB, scheme:<name>, or none)")
	AddMutationFlags(xlsxChartsSetChartAreaFillCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsSetChartAreaFillCmd)

	s := xlsxChartsCopyStyleCmd.Flags()
	s.StringVar(&xlsxChartsCopyStyleSheet, "sheet", "", "sheet number (1-based) or exact sheet name for the target chart")
	s.StringVar(&xlsxChartsCopyStyleChart, "chart", "", "target chart selector")
	s.StringVar(&xlsxChartsCopyStyleToChart, "to-chart", "", "target chart selector (alias for --chart)")
	s.StringVar(&xlsxChartsCopyStyleFrom, "from", "", "template .xlsx file to copy style from")
	s.StringVar(&xlsxChartsCopyStyleFromChart, "from-chart", "", "template chart selector (default chart:1)")
	s.IntVar(&xlsxChartsCopyStyleExpectCount, "expect-series-count", 0, "guard: require the target chart to have this many series")
	AddMutationFlags(xlsxChartsCopyStyleCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsCopyStyleCmd)
}
