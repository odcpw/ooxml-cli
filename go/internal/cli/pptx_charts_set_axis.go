package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	"github.com/spf13/cobra"
)

var (
	pptxChartsSetAxisSlide          int
	pptxChartsSetAxisChart          string
	pptxChartsSetAxisAxis           string
	pptxChartsSetAxisTitle          string
	pptxChartsSetAxisExpectTitle    string
	pptxChartsSetAxisHidden         bool
	pptxChartsSetAxisMin            float64
	pptxChartsSetAxisMax            float64
	pptxChartsSetAxisMajorUnit      float64
	pptxChartsSetAxisNumberFormat   string
	pptxChartsSetAxisMajorGridlines bool
	pptxChartsSetAxisMinorGridlines bool
	pptxChartsSetAxisTickFamily     string
	pptxChartsSetAxisTickSize       float64
	pptxChartsSetAxisTickColor      string
	pptxChartsSetAxisTickBold       bool
	pptxChartsSetAxisTickItalic     bool
	pptxChartsSetAxisTitleFamily    string
	pptxChartsSetAxisTitleSize      float64
	pptxChartsSetAxisTitleColor     string
	pptxChartsSetAxisTitleBold      bool
	pptxChartsSetAxisTitleItalic    bool
	pptxChartsSetAxisExpectCount    int
)

var pptxChartsSetAxisCmd = &cobra.Command{
	Use:   "set-axis <file>",
	Short: "Set title, visibility, scale, number format, gridlines, or tick label font on a slide chart axis",
	Long: `Set practical properties of one category or value axis on an existing slide chart.

Select the axis with --axis category or --axis value. Scatter charts have two
value axes, so --axis value is rejected as ambiguous on them. At least one
property flag is required.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		kind, err := parseChartAxisKind(pptxChartsSetAxisAxis)
		if err != nil {
			return err
		}
		flags, err := resolveChartAxisFlags(cmd, axisFlagInputs{
			title:          pptxChartsSetAxisTitle,
			hidden:         pptxChartsSetAxisHidden,
			min:            pptxChartsSetAxisMin,
			max:            pptxChartsSetAxisMax,
			majorUnit:      pptxChartsSetAxisMajorUnit,
			numberFormat:   pptxChartsSetAxisNumberFormat,
			majorGridlines: pptxChartsSetAxisMajorGridlines,
			minorGridlines: pptxChartsSetAxisMinorGridlines,
			tickFamily:     pptxChartsSetAxisTickFamily,
			tickSize:       pptxChartsSetAxisTickSize,
			tickColor:      pptxChartsSetAxisTickColor,
			tickBold:       pptxChartsSetAxisTickBold,
			tickItalic:     pptxChartsSetAxisTickItalic,
			titleFamily:    pptxChartsSetAxisTitleFamily,
			titleSize:      pptxChartsSetAxisTitleSize,
			titleColor:     pptxChartsSetAxisTitleColor,
			titleBold:      pptxChartsSetAxisTitleBold,
			titleItalic:    pptxChartsSetAxisTitleItalic,
		})
		if err != nil {
			return err
		}
		var expectTitle *string
		if cmd.Flags().Changed("expect-axis-title") {
			v := pptxChartsSetAxisExpectTitle
			expectTitle = &v
		}
		var expectCount *int
		if cmd.Flags().Changed("expect-axis-count") {
			if pptxChartsSetAxisExpectCount < 0 {
				return InvalidArgsError("--expect-axis-count must be >= 0")
			}
			v := pptxChartsSetAxisExpectCount
			expectCount = &v
		}

		var previousTitle string
		result, err := runPPTXChartStyleMutation(cmd, filePath, pptxChartsSetAxisSlide, pptxChartsSetAxisChart, "pptx.chart.set-axis", func(pkg opc.PackageSession, chartURI string) error {
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
		return outputPPTXChartStyleResult(cmd, result, fmt.Sprintf("%s axis updated", kind))
	},
}

func init() {
	a := pptxChartsSetAxisCmd.Flags()
	a.IntVar(&pptxChartsSetAxisSlide, "slide", 0, "1-based slide number to search")
	a.StringVar(&pptxChartsSetAxisChart, "chart", "", "chart selector from pptx charts list")
	a.StringVar(&pptxChartsSetAxisAxis, "axis", "", "axis to edit: category or value")
	a.StringVar(&pptxChartsSetAxisTitle, "title", "", "axis title text (empty clears the title)")
	a.StringVar(&pptxChartsSetAxisExpectTitle, "expect-axis-title", "", "guard: require the current axis title to match this text")
	a.BoolVar(&pptxChartsSetAxisHidden, "hidden", false, "hide the axis (c:delete); use --hidden=false to show")
	a.Float64Var(&pptxChartsSetAxisMin, "min", 0, "axis scale minimum")
	a.Float64Var(&pptxChartsSetAxisMax, "max", 0, "axis scale maximum")
	a.Float64Var(&pptxChartsSetAxisMajorUnit, "major-unit", 0, "major unit interval (value axes)")
	a.StringVar(&pptxChartsSetAxisNumberFormat, "number-format", "", "axis number format code, e.g. #,##0")
	a.BoolVar(&pptxChartsSetAxisMajorGridlines, "major-gridlines", false, "show major gridlines; use --major-gridlines=false to hide")
	a.BoolVar(&pptxChartsSetAxisMinorGridlines, "minor-gridlines", false, "show minor gridlines; use --minor-gridlines=false to hide")
	a.StringVar(&pptxChartsSetAxisTickFamily, "tick-label-font-family", "", "tick label font family (a:latin typeface)")
	a.Float64Var(&pptxChartsSetAxisTickSize, "tick-label-font-size", 0, "tick label font size in points")
	a.StringVar(&pptxChartsSetAxisTickColor, "tick-label-font-color", "", "tick label font color as #RRGGBB")
	a.BoolVar(&pptxChartsSetAxisTickBold, "tick-label-font-bold", false, "tick label font bold")
	a.BoolVar(&pptxChartsSetAxisTickItalic, "tick-label-font-italic", false, "tick label font italic")
	a.StringVar(&pptxChartsSetAxisTitleFamily, "title-font-family", "", "axis title font family (a:latin typeface); applies when --title is set")
	a.Float64Var(&pptxChartsSetAxisTitleSize, "title-font-size", 0, "axis title font size in points; applies when --title is set")
	a.StringVar(&pptxChartsSetAxisTitleColor, "title-font-color", "", "axis title font color as #RRGGBB; applies when --title is set")
	a.BoolVar(&pptxChartsSetAxisTitleBold, "title-font-bold", false, "axis title font bold; applies when --title is set")
	a.BoolVar(&pptxChartsSetAxisTitleItalic, "title-font-italic", false, "axis title font italic; applies when --title is set")
	a.IntVar(&pptxChartsSetAxisExpectCount, "expect-axis-count", 0, "guard: require the chart to have this many axes")
	AddMutationFlags(pptxChartsSetAxisCmd)
	chartsCmd.AddCommand(pptxChartsSetAxisCmd)
}
