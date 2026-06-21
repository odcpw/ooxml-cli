package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	"github.com/spf13/cobra"
)

var (
	xlsxChartsSetAxisSheet          string
	xlsxChartsSetAxisChart          string
	xlsxChartsSetAxisAxis           string
	xlsxChartsSetAxisTitle          string
	xlsxChartsSetAxisExpectTitle    string
	xlsxChartsSetAxisHidden         bool
	xlsxChartsSetAxisMin            float64
	xlsxChartsSetAxisMax            float64
	xlsxChartsSetAxisMajorUnit      float64
	xlsxChartsSetAxisNumberFormat   string
	xlsxChartsSetAxisMajorGridlines bool
	xlsxChartsSetAxisMinorGridlines bool
	xlsxChartsSetAxisTickFamily     string
	xlsxChartsSetAxisTickSize       float64
	xlsxChartsSetAxisTickColor      string
	xlsxChartsSetAxisTickBold       bool
	xlsxChartsSetAxisTickItalic     bool
	xlsxChartsSetAxisTitleFamily    string
	xlsxChartsSetAxisTitleSize      float64
	xlsxChartsSetAxisTitleColor     string
	xlsxChartsSetAxisTitleBold      bool
	xlsxChartsSetAxisTitleItalic    bool
	xlsxChartsSetAxisExpectCount    int
)

var xlsxChartsSetAxisCmd = &cobra.Command{
	Use:   "set-axis <file>",
	Short: "Set title, visibility, scale, number format, gridlines, or tick label font on a worksheet chart axis",
	Long: `Set practical properties of one category or value axis on an existing worksheet chart.

Select the axis with --axis category or --axis value. Scatter charts have two
value axes, so --axis value is rejected as ambiguous on them. At least one
property flag is required.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		kind, err := parseChartAxisKind(xlsxChartsSetAxisAxis)
		if err != nil {
			return err
		}
		flags, err := resolveChartAxisFlags(cmd, axisFlagInputs{
			title:          xlsxChartsSetAxisTitle,
			hidden:         xlsxChartsSetAxisHidden,
			min:            xlsxChartsSetAxisMin,
			max:            xlsxChartsSetAxisMax,
			majorUnit:      xlsxChartsSetAxisMajorUnit,
			numberFormat:   xlsxChartsSetAxisNumberFormat,
			majorGridlines: xlsxChartsSetAxisMajorGridlines,
			minorGridlines: xlsxChartsSetAxisMinorGridlines,
			tickFamily:     xlsxChartsSetAxisTickFamily,
			tickSize:       xlsxChartsSetAxisTickSize,
			tickColor:      xlsxChartsSetAxisTickColor,
			tickBold:       xlsxChartsSetAxisTickBold,
			tickItalic:     xlsxChartsSetAxisTickItalic,
			titleFamily:    xlsxChartsSetAxisTitleFamily,
			titleSize:      xlsxChartsSetAxisTitleSize,
			titleColor:     xlsxChartsSetAxisTitleColor,
			titleBold:      xlsxChartsSetAxisTitleBold,
			titleItalic:    xlsxChartsSetAxisTitleItalic,
		})
		if err != nil {
			return err
		}
		var expectTitle *string
		if cmd.Flags().Changed("expect-axis-title") {
			v := xlsxChartsSetAxisExpectTitle
			expectTitle = &v
		}
		var expectCount *int
		if cmd.Flags().Changed("expect-axis-count") {
			if xlsxChartsSetAxisExpectCount < 0 {
				return InvalidArgsError("--expect-axis-count must be >= 0")
			}
			v := xlsxChartsSetAxisExpectCount
			expectCount = &v
		}

		var previousTitle string
		result, err := runXLSXChartStyleMutation(cmd, filePath, xlsxChartsSetAxisSheet, xlsxChartsSetAxisChart, "xlsx.chart.set-axis", func(pkg opc.PackageSession, chartURI string) error {
			res, err := xlsxchart.SetAxis(buildSetAxisRequest(pkg, chartURI, kind, flags, expectTitle, expectCount))
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%v", err)
			}
			previousTitle = res.PreviousTitle
			return nil
		})
		if err != nil {
			return err
		}
		result.PreviousTitle = previousTitle
		return outputXLSXChartStyleResult(cmd, result, fmt.Sprintf("%s axis updated", kind))
	},
}

func init() {
	a := xlsxChartsSetAxisCmd.Flags()
	a.StringVar(&xlsxChartsSetAxisSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	a.StringVar(&xlsxChartsSetAxisChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	a.StringVar(&xlsxChartsSetAxisAxis, "axis", "", "axis to edit: category or value")
	a.StringVar(&xlsxChartsSetAxisTitle, "title", "", "axis title text (empty clears the title)")
	a.StringVar(&xlsxChartsSetAxisExpectTitle, "expect-axis-title", "", "guard: require the current axis title to match this text")
	a.BoolVar(&xlsxChartsSetAxisHidden, "hidden", false, "hide the axis (c:delete); use --hidden=false to show")
	a.Float64Var(&xlsxChartsSetAxisMin, "min", 0, "axis scale minimum")
	a.Float64Var(&xlsxChartsSetAxisMax, "max", 0, "axis scale maximum")
	a.Float64Var(&xlsxChartsSetAxisMajorUnit, "major-unit", 0, "major unit interval (value axes)")
	a.StringVar(&xlsxChartsSetAxisNumberFormat, "number-format", "", "axis number format code, e.g. #,##0")
	a.BoolVar(&xlsxChartsSetAxisMajorGridlines, "major-gridlines", false, "show major gridlines; use --major-gridlines=false to hide")
	a.BoolVar(&xlsxChartsSetAxisMinorGridlines, "minor-gridlines", false, "show minor gridlines; use --minor-gridlines=false to hide")
	a.StringVar(&xlsxChartsSetAxisTickFamily, "tick-label-font-family", "", "tick label font family (a:latin typeface)")
	a.Float64Var(&xlsxChartsSetAxisTickSize, "tick-label-font-size", 0, "tick label font size in points")
	a.StringVar(&xlsxChartsSetAxisTickColor, "tick-label-font-color", "", "tick label font color as #RRGGBB")
	a.BoolVar(&xlsxChartsSetAxisTickBold, "tick-label-font-bold", false, "tick label font bold")
	a.BoolVar(&xlsxChartsSetAxisTickItalic, "tick-label-font-italic", false, "tick label font italic")
	a.StringVar(&xlsxChartsSetAxisTitleFamily, "title-font-family", "", "axis title font family (a:latin typeface); applies when --title is set")
	a.Float64Var(&xlsxChartsSetAxisTitleSize, "title-font-size", 0, "axis title font size in points; applies when --title is set")
	a.StringVar(&xlsxChartsSetAxisTitleColor, "title-font-color", "", "axis title font color as #RRGGBB; applies when --title is set")
	a.BoolVar(&xlsxChartsSetAxisTitleBold, "title-font-bold", false, "axis title font bold; applies when --title is set")
	a.BoolVar(&xlsxChartsSetAxisTitleItalic, "title-font-italic", false, "axis title font italic; applies when --title is set")
	a.IntVar(&xlsxChartsSetAxisExpectCount, "expect-axis-count", 0, "guard: require the chart to have this many axes")
	AddMutationFlags(xlsxChartsSetAxisCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsSetAxisCmd)
}
