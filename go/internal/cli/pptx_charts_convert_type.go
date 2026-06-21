package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	"github.com/spf13/cobra"
)

var (
	pptxChartsConvertTypeSlide      int
	pptxChartsConvertTypeChart      string
	pptxChartsConvertTypeTo         string
	pptxChartsConvertTypeExpectType string
)

var pptxChartsConvertTypeCmd = &cobra.Command{
	Use:   "convert-type <file>",
	Short: "Convert an existing slide chart to a different chart type",
	Long: `Convert an existing slide chart to a different chart type.

--to selects the target type: bar, column, line, area, pie, or scatter. Series
source references and cached values are preserved; bar and column differ only in
bar direction. Converting to pie drops the axes and rejects multi-series charts;
converting to or from scatter renames the category axis and series sources.
Converting away from a pie chart is not supported. Guard with --expect-type.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		target, expectType, err := resolveChartConvertType(cmd, pptxChartsConvertTypeTo, pptxChartsConvertTypeExpectType)
		if err != nil {
			return err
		}

		var (
			previousType string
			warnings     []string
		)
		result, err := runPPTXChartStyleMutation(cmd, filePath, pptxChartsConvertTypeSlide, pptxChartsConvertTypeChart, "pptx.chart.convert-type", func(pkg opc.PackageSession, chartURI string) error {
			res, err := xlsxchart.ConvertChartType(&xlsxchart.ConvertChartTypeRequest{
				Package:    pkg,
				ChartURI:   chartURI,
				TargetType: target,
				ExpectType: expectType,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%v", err)
			}
			previousType = string(res.PreviousType)
			warnings = res.Warnings
			return nil
		})
		if err != nil {
			return err
		}
		result.PreviousType = previousType
		result.NewType = string(target)
		result.Warnings = warnings
		return outputPPTXChartStyleResult(cmd, result, fmt.Sprintf("chart type converted from %s to %s", previousType, target))
	},
}

func init() {
	c := pptxChartsConvertTypeCmd.Flags()
	c.IntVar(&pptxChartsConvertTypeSlide, "slide", 0, "1-based slide number to search")
	c.StringVar(&pptxChartsConvertTypeChart, "chart", "", "chart selector from pptx charts list")
	c.StringVar(&pptxChartsConvertTypeTo, "to", "", "target chart type: bar, column, line, area, pie, or scatter")
	c.StringVar(&pptxChartsConvertTypeExpectType, "expect-type", "", "guard: require the current chart type to match this")
	AddMutationFlags(pptxChartsConvertTypeCmd)
	chartsCmd.AddCommand(pptxChartsConvertTypeCmd)
}
