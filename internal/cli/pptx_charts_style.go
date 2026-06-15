package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxchart "github.com/ooxml-cli/ooxml-cli/pkg/pptx/chart"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	"github.com/spf13/cobra"
)

// PPTXChartStyleResult is the mutation envelope shared by the slide chart
// styling commands (set-title, set-legend, set-series-style).
type PPTXChartStyleResult struct {
	File          string               `json:"file"`
	Output        string               `json:"output,omitempty"`
	DryRun        bool                 `json:"dryRun"`
	Action        string               `json:"action"`
	Chart         *PPTXChartResultItem `json:"chart,omitempty"`
	PreviousTitle string               `json:"previousTitle,omitempty"`
	LegendRemoved bool                 `json:"legendRemoved,omitempty"`
	Series        int                  `json:"series,omitempty"`
	PreviousType  string               `json:"previousType,omitempty"`
	NewType       string               `json:"newType,omitempty"`
	PreviousFill  string               `json:"previousFill,omitempty"`
	NewFill       string               `json:"newFill,omitempty"`
	AppliedStyle  []string             `json:"appliedStyle,omitempty"`
	Warnings      []string             `json:"warnings,omitempty"`

	ValidateCommand          string `json:"validateCommand,omitempty"`
	ChartShowCommand         string `json:"chartShowCommand,omitempty"`
	RenderCommand            string `json:"renderCommand,omitempty"`
	ValidateCommandTemplate  string `json:"validateCommandTemplate,omitempty"`
	ChartShowCommandTemplate string `json:"chartShowCommandTemplate,omitempty"`
	RenderCommandTemplate    string `json:"renderCommandTemplate,omitempty"`
}

var (
	pptxChartsSetTitleSlide      int
	pptxChartsSetTitleChart      string
	pptxChartsSetTitleTitle      string
	pptxChartsSetTitleExpect     string
	pptxChartsSetTitleFontFamily string
	pptxChartsSetTitleFontSize   float64
	pptxChartsSetTitleFontColor  string
	pptxChartsSetTitleFontBold   bool
	pptxChartsSetTitleFontItalic bool
)

var pptxChartsSetTitleCmd = &cobra.Command{
	Use:   "set-title <file>",
	Short: "Set an existing slide chart's title text and font",
	Long: `Set the literal title text (and optional font) of an existing slide chart.

This edits an existing chart's title; it does not author charts. Cell-linked
titles are rejected rather than silently converted to literal text.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if !cmd.Flags().Changed("title") {
			return InvalidArgsError("--title is required")
		}
		font, err := resolveChartFontFlags(cmd, pptxChartsSetTitleFontFamily, pptxChartsSetTitleFontSize, pptxChartsSetTitleFontColor, pptxChartsSetTitleFontBold, pptxChartsSetTitleFontItalic)
		if err != nil {
			return err
		}
		var expect *string
		if cmd.Flags().Changed("expect-title") {
			v := pptxChartsSetTitleExpect
			expect = &v
		}
		var previousTitle string
		result, err := runPPTXChartStyleMutation(cmd, filePath, pptxChartsSetTitleSlide, pptxChartsSetTitleChart, "pptx.chart.set-title", func(pkg opc.PackageSession, chartURI string) error {
			res, err := xlsxchart.SetTitle(&xlsxchart.SetTitleRequest{
				Package:    pkg,
				ChartURI:   chartURI,
				Text:       pptxChartsSetTitleTitle,
				ExpectText: expect,
				FontFamily: font.family,
				FontSizePt: font.sizePt,
				FontColor:  font.color,
				FontBold:   font.bold,
				FontItalic: font.italic,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%v", err)
			}
			previousTitle = res.PreviousText
			return nil
		})
		if err != nil {
			return err
		}
		result.PreviousTitle = previousTitle
		return outputPPTXChartStyleResult(cmd, result, fmt.Sprintf("title set to %q", pptxChartsSetTitleTitle))
	},
}

var (
	pptxChartsSetLegendSlide    int
	pptxChartsSetLegendChart    string
	pptxChartsSetLegendPosition string
	pptxChartsSetLegendOverlay  bool
	pptxChartsSetLegendExpect   string
)

var pptxChartsSetLegendCmd = &cobra.Command{
	Use:   "set-legend <file>",
	Short: "Set an existing slide chart's legend position and overlay",
	Long:  "Set the legend position and/or overlay of an existing slide chart, or remove the legend with --position none.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		positionChanged := cmd.Flags().Changed("position")
		overlayChanged := cmd.Flags().Changed("overlay")
		if !positionChanged && !overlayChanged {
			return InvalidArgsError("set-legend requires --position and/or --overlay")
		}
		var (
			code   string
			remove bool
			err    error
		)
		if positionChanged {
			code, remove, err = parseChartLegendPosition(pptxChartsSetLegendPosition)
			if err != nil {
				return err
			}
		}
		if remove && overlayChanged {
			return InvalidArgsError("--overlay cannot be combined with --position none")
		}
		var overlay *bool
		if overlayChanged {
			v := pptxChartsSetLegendOverlay
			overlay = &v
		}
		var expect *string
		if cmd.Flags().Changed("expect-position") {
			expectCode, perr := parseChartExpectLegendPosition(pptxChartsSetLegendExpect)
			if perr != nil {
				return perr
			}
			expect = &expectCode
		}
		var removed bool
		result, err := runPPTXChartStyleMutation(cmd, filePath, pptxChartsSetLegendSlide, pptxChartsSetLegendChart, "pptx.chart.set-legend", func(pkg opc.PackageSession, chartURI string) error {
			res, err := xlsxchart.SetLegend(&xlsxchart.SetLegendRequest{
				Package:        pkg,
				ChartURI:       chartURI,
				SetPosition:    positionChanged && !remove,
				Position:       code,
				Remove:         remove,
				Overlay:        overlay,
				ExpectPosition: expect,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%v", err)
			}
			removed = res.Removed
			return nil
		})
		if err != nil {
			return err
		}
		result.LegendRemoved = removed
		summary := "legend updated"
		if removed {
			summary = "legend removed"
		}
		return outputPPTXChartStyleResult(cmd, result, summary)
	},
}

var (
	pptxChartsSetSeriesStyleSlide       int
	pptxChartsSetSeriesStyleChart       string
	pptxChartsSetSeriesStyleSeries      int
	pptxChartsSetSeriesStyleFillColor   string
	pptxChartsSetSeriesStyleLineColor   string
	pptxChartsSetSeriesStyleLineWidth   float64
	pptxChartsSetSeriesStyleMarkerSym   string
	pptxChartsSetSeriesStyleMarkerSize  int
	pptxChartsSetSeriesStyleExpectCount int
)

var pptxChartsSetSeriesStyleCmd = &cobra.Command{
	Use:   "set-series-style <file>",
	Short: "Set fill, line, and marker style on a slide chart series",
	Long:  "Set the fill color, line color/width, and marker symbol/size of one series on an existing slide chart.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxChartsSetSeriesStyleSeries < 1 {
			return InvalidArgsError("--series must be >= 1")
		}
		style, err := resolveChartSeriesStyleFlags(cmd, pptxChartsSetSeriesStyleFillColor, pptxChartsSetSeriesStyleLineColor, pptxChartsSetSeriesStyleLineWidth, pptxChartsSetSeriesStyleMarkerSym, pptxChartsSetSeriesStyleMarkerSize)
		if err != nil {
			return err
		}
		var expectCount *int
		if cmd.Flags().Changed("expect-series-count") {
			if pptxChartsSetSeriesStyleExpectCount < 0 {
				return InvalidArgsError("--expect-series-count must be >= 0")
			}
			v := pptxChartsSetSeriesStyleExpectCount
			expectCount = &v
		}
		result, err := runPPTXChartStyleMutation(cmd, filePath, pptxChartsSetSeriesStyleSlide, pptxChartsSetSeriesStyleChart, "pptx.chart.set-series-style", func(pkg opc.PackageSession, chartURI string) error {
			_, err := xlsxchart.SetSeriesStyle(&xlsxchart.SetSeriesStyleRequest{
				Package:           pkg,
				ChartURI:          chartURI,
				SeriesNumber:      pptxChartsSetSeriesStyleSeries,
				FillColor:         style.fillColor,
				LineColor:         style.lineColor,
				LineWidthPt:       style.lineWidthPt,
				MarkerSymbol:      style.markerSym,
				MarkerSize:        style.markerSize,
				ExpectSeriesCount: expectCount,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%v", err)
			}
			return nil
		})
		if err != nil {
			return err
		}
		result.Series = pptxChartsSetSeriesStyleSeries
		return outputPPTXChartStyleResult(cmd, result, fmt.Sprintf("series %d style updated", pptxChartsSetSeriesStyleSeries))
	},
}

// runPPTXChartStyleMutation selects a slide chart, applies a style mutation to
// its chart part, and assembles the mutation envelope with readback.
func runPPTXChartStyleMutation(cmd *cobra.Command, filePath string, slide int, chartSel, action string, apply func(pkg opc.PackageSession, chartURI string) error) (*PPTXChartStyleResult, error) {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return nil, err
	}
	wantReadback := GetGlobalConfig(cmd).Format == "json"
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}

	var result *PPTXChartStyleResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		charts, err := pptxchart.List(pkg, slide)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list PPTX charts: %v", err)
		}
		selected, err := selectPPTXChart(charts, chartSel)
		if err != nil {
			return err
		}
		if err := apply(pkg, selected.PartURI); err != nil {
			return err
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var chartItem *PPTXChartResultItem
		if wantReadback {
			updatedCharts, err := pptxchart.List(pkg, slide)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read back PPTX charts: %v", err)
			}
			updated, err := selectPPTXChart(updatedCharts, "part:"+selected.PartURI)
			if err != nil {
				return err
			}
			item := pptxChartItemForUpdate(destinationFile, updated)
			if style, err := xlsxchart.InspectStyle(pkg, selected.PartURI); err == nil {
				item.Style = style
			}
			chartItem = &item
		}
		result = buildPPTXChartStyleResult(filePath, destinationFile, mutOpts, action, chartItem, slide, chartSel)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func buildPPTXChartStyleResult(filePath, destinationFile string, mutOpts *MutationOptions, action string, chartItem *PPTXChartResultItem, slide int, chartSel string) *PPTXChartStyleResult {
	result := &PPTXChartStyleResult{
		File:   filePath,
		Output: destinationFile,
		DryRun: mutOpts != nil && mutOpts.DryRun,
		Action: action,
		Chart:  chartItem,
	}
	selector := chartSel
	if chartItem != nil {
		selector = pptxChartGeneratedSelector(chartItem.ChartRef)
	}
	selector = chartStyleSelectorOrDefault(selector)
	if result.Output == "" {
		placeholder := outputPlaceholder()
		result.ValidateCommandTemplate = pptxValidateCommand(placeholder)
		result.ChartShowCommandTemplate = pptxChartShowCommand(placeholder, slide, selector)
		result.RenderCommandTemplate = pptxRenderCommand(placeholder)
	} else {
		result.ValidateCommand = pptxValidateCommand(result.Output)
		result.ChartShowCommand = pptxChartShowCommand(result.Output, slide, selector)
		result.RenderCommand = pptxRenderCommand(result.Output)
	}
	return result
}

func outputPPTXChartStyleResult(cmd *cobra.Command, result *PPTXChartStyleResult, summary string) error {
	if GetGlobalConfig(cmd).Format == "json" {
		config := GetGlobalConfig(cmd)
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
			return NewCLIErrorf(ExitUnexpected, "failed to marshal PPTX chart style JSON: %v", err)
		}
		return writeCLIOutput(cmd, data)
	}
	text := summary
	if result.DryRun {
		text = "would update: " + summary
	}
	if result.Output != "" {
		text += "\noutput: " + result.Output
	}
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	t := pptxChartsSetTitleCmd.Flags()
	t.IntVar(&pptxChartsSetTitleSlide, "slide", 0, "1-based slide number to search")
	t.StringVar(&pptxChartsSetTitleChart, "chart", "", "chart selector from pptx charts list")
	t.StringVar(&pptxChartsSetTitleTitle, "title", "", "new chart title text")
	t.StringVar(&pptxChartsSetTitleExpect, "expect-title", "", "guard: require the current title to match this text")
	t.StringVar(&pptxChartsSetTitleFontFamily, "font-family", "", "title font family (a:latin typeface), e.g. Calibri")
	t.Float64Var(&pptxChartsSetTitleFontSize, "font-size", 0, "title font size in points")
	t.StringVar(&pptxChartsSetTitleFontColor, "font-color", "", "title font color as #RRGGBB")
	t.BoolVar(&pptxChartsSetTitleFontBold, "font-bold", false, "title font bold")
	t.BoolVar(&pptxChartsSetTitleFontItalic, "font-italic", false, "title font italic")
	AddMutationFlags(pptxChartsSetTitleCmd)
	chartsCmd.AddCommand(pptxChartsSetTitleCmd)

	l := pptxChartsSetLegendCmd.Flags()
	l.IntVar(&pptxChartsSetLegendSlide, "slide", 0, "1-based slide number to search")
	l.StringVar(&pptxChartsSetLegendChart, "chart", "", "chart selector from pptx charts list")
	l.StringVar(&pptxChartsSetLegendPosition, "position", "", "legend position: right, left, top, bottom, or none")
	l.BoolVar(&pptxChartsSetLegendOverlay, "overlay", false, "overlay the legend on the plot area; use --overlay=false to disable")
	l.StringVar(&pptxChartsSetLegendExpect, "expect-position", "", "guard: require the current legend position to match")
	AddMutationFlags(pptxChartsSetLegendCmd)
	chartsCmd.AddCommand(pptxChartsSetLegendCmd)

	s := pptxChartsSetSeriesStyleCmd.Flags()
	s.IntVar(&pptxChartsSetSeriesStyleSlide, "slide", 0, "1-based slide number to search")
	s.StringVar(&pptxChartsSetSeriesStyleChart, "chart", "", "chart selector from pptx charts list")
	s.IntVar(&pptxChartsSetSeriesStyleSeries, "series", 1, "1-based chart series number")
	s.StringVar(&pptxChartsSetSeriesStyleFillColor, "fill-color", "", "series fill color as #RRGGBB")
	s.StringVar(&pptxChartsSetSeriesStyleLineColor, "line-color", "", "series line color as #RRGGBB")
	s.Float64Var(&pptxChartsSetSeriesStyleLineWidth, "line-width-pt", 0, "series line width in points")
	s.StringVar(&pptxChartsSetSeriesStyleMarkerSym, "marker-symbol", "", "series marker symbol: circle, square, diamond, triangle, or none")
	s.IntVar(&pptxChartsSetSeriesStyleMarkerSize, "marker-size", 0, "series marker size (2-72)")
	s.IntVar(&pptxChartsSetSeriesStyleExpectCount, "expect-series-count", 0, "guard: require the chart to have this many series")
	AddMutationFlags(pptxChartsSetSeriesStyleCmd)
	chartsCmd.AddCommand(pptxChartsSetSeriesStyleCmd)
}
