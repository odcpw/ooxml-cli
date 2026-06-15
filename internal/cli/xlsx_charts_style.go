package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/spf13/cobra"
)

// XLSXChartStyleResult is the mutation envelope shared by the worksheet chart
// styling commands (set-title, set-legend, set-series-style).
type XLSXChartStyleResult struct {
	File          string         `json:"file"`
	Output        string         `json:"output,omitempty"`
	DryRun        bool           `json:"dryRun"`
	Action        string         `json:"action"`
	Chart         *XLSXChartItem `json:"chart,omitempty"`
	PreviousTitle string         `json:"previousTitle,omitempty"`
	LegendRemoved bool           `json:"legendRemoved,omitempty"`
	Series        int            `json:"series,omitempty"`
	PreviousType  string         `json:"previousType,omitempty"`
	NewType       string         `json:"newType,omitempty"`
	PreviousFill  string         `json:"previousFill,omitempty"`
	NewFill       string         `json:"newFill,omitempty"`
	AppliedStyle  []string       `json:"appliedStyle,omitempty"`
	Warnings      []string       `json:"warnings,omitempty"`

	ValidateCommand          string `json:"validateCommand,omitempty"`
	ChartShowCommand         string `json:"chartShowCommand,omitempty"`
	ValidateCommandTemplate  string `json:"validateCommandTemplate,omitempty"`
	ChartShowCommandTemplate string `json:"chartShowCommandTemplate,omitempty"`
}

var (
	xlsxChartsSetTitleSheet      string
	xlsxChartsSetTitleChart      string
	xlsxChartsSetTitleTitle      string
	xlsxChartsSetTitleExpect     string
	xlsxChartsSetTitleFontFamily string
	xlsxChartsSetTitleFontSize   float64
	xlsxChartsSetTitleFontColor  string
	xlsxChartsSetTitleFontBold   bool
	xlsxChartsSetTitleFontItalic bool
)

var xlsxChartsSetTitleCmd = &cobra.Command{
	Use:   "set-title <file>",
	Short: "Set an existing worksheet chart's title text and font",
	Long: `Set the literal title text (and optional font) of an existing worksheet chart.

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
		font, err := resolveChartFontFlags(cmd, xlsxChartsSetTitleFontFamily, xlsxChartsSetTitleFontSize, xlsxChartsSetTitleFontColor, xlsxChartsSetTitleFontBold, xlsxChartsSetTitleFontItalic)
		if err != nil {
			return err
		}
		var expect *string
		if cmd.Flags().Changed("expect-title") {
			v := xlsxChartsSetTitleExpect
			expect = &v
		}
		var previousTitle string
		result, err := runXLSXChartStyleMutation(cmd, filePath, xlsxChartsSetTitleSheet, xlsxChartsSetTitleChart, "xlsx.chart.set-title", func(pkg opc.PackageSession, chartURI string) error {
			res, err := xlsxchart.SetTitle(&xlsxchart.SetTitleRequest{
				Package:    pkg,
				ChartURI:   chartURI,
				Text:       xlsxChartsSetTitleTitle,
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
		return outputXLSXChartStyleResult(cmd, result, fmt.Sprintf("title set to %q", xlsxChartsSetTitleTitle))
	},
}

var (
	xlsxChartsSetLegendSheet    string
	xlsxChartsSetLegendChart    string
	xlsxChartsSetLegendPosition string
	xlsxChartsSetLegendOverlay  bool
	xlsxChartsSetLegendExpect   string
)

var xlsxChartsSetLegendCmd = &cobra.Command{
	Use:   "set-legend <file>",
	Short: "Set an existing worksheet chart's legend position and overlay",
	Long:  "Set the legend position and/or overlay of an existing worksheet chart, or remove the legend with --position none.",
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
			code, remove, err = parseChartLegendPosition(xlsxChartsSetLegendPosition)
			if err != nil {
				return err
			}
		}
		if remove && overlayChanged {
			return InvalidArgsError("--overlay cannot be combined with --position none")
		}
		var overlay *bool
		if overlayChanged {
			v := xlsxChartsSetLegendOverlay
			overlay = &v
		}
		var expect *string
		if cmd.Flags().Changed("expect-position") {
			expectCode, perr := parseChartExpectLegendPosition(xlsxChartsSetLegendExpect)
			if perr != nil {
				return perr
			}
			expect = &expectCode
		}
		var removed bool
		result, err := runXLSXChartStyleMutation(cmd, filePath, xlsxChartsSetLegendSheet, xlsxChartsSetLegendChart, "xlsx.chart.set-legend", func(pkg opc.PackageSession, chartURI string) error {
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
		return outputXLSXChartStyleResult(cmd, result, summary)
	},
}

var (
	xlsxChartsSetSeriesStyleSheet       string
	xlsxChartsSetSeriesStyleChart       string
	xlsxChartsSetSeriesStyleSeries      int
	xlsxChartsSetSeriesStyleFillColor   string
	xlsxChartsSetSeriesStyleLineColor   string
	xlsxChartsSetSeriesStyleLineWidth   float64
	xlsxChartsSetSeriesStyleMarkerSym   string
	xlsxChartsSetSeriesStyleMarkerSize  int
	xlsxChartsSetSeriesStyleExpectCount int
)

var xlsxChartsSetSeriesStyleCmd = &cobra.Command{
	Use:   "set-series-style <file>",
	Short: "Set fill, line, and marker style on a worksheet chart series",
	Long:  "Set the fill color, line color/width, and marker symbol/size of one series on an existing worksheet chart.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxChartsSetSeriesStyleSeries < 1 {
			return InvalidArgsError("--series must be >= 1")
		}
		style, err := resolveChartSeriesStyleFlags(cmd, xlsxChartsSetSeriesStyleFillColor, xlsxChartsSetSeriesStyleLineColor, xlsxChartsSetSeriesStyleLineWidth, xlsxChartsSetSeriesStyleMarkerSym, xlsxChartsSetSeriesStyleMarkerSize)
		if err != nil {
			return err
		}
		var expectCount *int
		if cmd.Flags().Changed("expect-series-count") {
			if xlsxChartsSetSeriesStyleExpectCount < 0 {
				return InvalidArgsError("--expect-series-count must be >= 0")
			}
			v := xlsxChartsSetSeriesStyleExpectCount
			expectCount = &v
		}
		result, err := runXLSXChartStyleMutation(cmd, filePath, xlsxChartsSetSeriesStyleSheet, xlsxChartsSetSeriesStyleChart, "xlsx.chart.set-series-style", func(pkg opc.PackageSession, chartURI string) error {
			_, err := xlsxchart.SetSeriesStyle(&xlsxchart.SetSeriesStyleRequest{
				Package:           pkg,
				ChartURI:          chartURI,
				SeriesNumber:      xlsxChartsSetSeriesStyleSeries,
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
		result.Series = xlsxChartsSetSeriesStyleSeries
		return outputXLSXChartStyleResult(cmd, result, fmt.Sprintf("series %d style updated", xlsxChartsSetSeriesStyleSeries))
	},
}

// runXLSXChartStyleMutation selects a worksheet chart, applies a style mutation
// to its chart part, and assembles the mutation envelope with readback.
func runXLSXChartStyleMutation(cmd *cobra.Command, filePath, sheetSel, chartSel, action string, apply func(pkg opc.PackageSession, chartURI string) error) (*XLSXChartStyleResult, error) {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return nil, err
	}
	wantReadback := GetGlobalConfig(cmd).Format == "json"
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXChartStyleResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheets := workbook.Sheets
		if strings.TrimSpace(sheetSel) != "" {
			sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSel)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			sheets = []model.SheetRef{sheetRef}
		}
		charts, err := xlsxchart.List(pkg, workbook, sheets)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list charts: %v", err)
		}
		selected, err := selectXLSXChart(charts, chartSel)
		if err != nil {
			return err
		}
		if err := apply(pkg, selected.PartURI); err != nil {
			return err
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var chartItem *XLSXChartItem
		if wantReadback {
			updatedCharts, err := xlsxchart.List(pkg, workbook, sheets)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read back charts: %v", err)
			}
			updated, err := selectXLSXChart(updatedCharts, "part:"+selected.PartURI)
			if err != nil {
				return err
			}
			item := xlsxChartItemForUpdate(destinationFile, updated)
			if style, err := xlsxchart.InspectStyle(pkg, selected.PartURI); err == nil {
				item.Style = style
			}
			chartItem = &item
		}
		result = buildXLSXChartStyleResult(filePath, destinationFile, mutOpts, action, chartItem, sheetSel, chartSel)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func buildXLSXChartStyleResult(filePath, destinationFile string, mutOpts *MutationOptions, action string, chartItem *XLSXChartItem, sheetSel, chartSel string) *XLSXChartStyleResult {
	result := &XLSXChartStyleResult{
		File:   filePath,
		Output: destinationFile,
		DryRun: mutOpts != nil && mutOpts.DryRun,
		Action: action,
		Chart:  chartItem,
	}
	selector := chartSel
	if chartItem != nil {
		selector = xlsxChartSelector(chartItem.ChartRef)
	}
	selector = chartStyleSelectorOrDefault(selector)
	if result.Output == "" {
		placeholder := xlsxOutputPlaceholder()
		result.ValidateCommandTemplate = xlsxValidateCommand(placeholder)
		result.ChartShowCommandTemplate = xlsxChartShowCommand(placeholder, sheetSel, selector)
	} else {
		result.ValidateCommand = xlsxValidateCommand(result.Output)
		result.ChartShowCommand = xlsxChartShowCommand(result.Output, sheetSel, selector)
	}
	return result
}

func outputXLSXChartStyleResult(cmd *cobra.Command, result *XLSXChartStyleResult, summary string) error {
	if GetGlobalConfig(cmd).Format == "json" {
		return writeJSONResult(cmd, result, "chart style")
	}
	action := summary
	if result.DryRun {
		action = "would update: " + summary
	}
	text := action
	if result.Output != "" {
		text += "\noutput: " + result.Output
	}
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	t := xlsxChartsSetTitleCmd.Flags()
	t.StringVar(&xlsxChartsSetTitleSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	t.StringVar(&xlsxChartsSetTitleChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	t.StringVar(&xlsxChartsSetTitleTitle, "title", "", "new chart title text")
	t.StringVar(&xlsxChartsSetTitleExpect, "expect-title", "", "guard: require the current title to match this text")
	t.StringVar(&xlsxChartsSetTitleFontFamily, "font-family", "", "title font family (a:latin typeface), e.g. Calibri")
	t.Float64Var(&xlsxChartsSetTitleFontSize, "font-size", 0, "title font size in points")
	t.StringVar(&xlsxChartsSetTitleFontColor, "font-color", "", "title font color as #RRGGBB")
	t.BoolVar(&xlsxChartsSetTitleFontBold, "font-bold", false, "title font bold")
	t.BoolVar(&xlsxChartsSetTitleFontItalic, "font-italic", false, "title font italic")
	AddMutationFlags(xlsxChartsSetTitleCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsSetTitleCmd)

	l := xlsxChartsSetLegendCmd.Flags()
	l.StringVar(&xlsxChartsSetLegendSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	l.StringVar(&xlsxChartsSetLegendChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	l.StringVar(&xlsxChartsSetLegendPosition, "position", "", "legend position: right, left, top, bottom, or none")
	l.BoolVar(&xlsxChartsSetLegendOverlay, "overlay", false, "overlay the legend on the plot area; use --overlay=false to disable")
	l.StringVar(&xlsxChartsSetLegendExpect, "expect-position", "", "guard: require the current legend position to match")
	AddMutationFlags(xlsxChartsSetLegendCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsSetLegendCmd)

	s := xlsxChartsSetSeriesStyleCmd.Flags()
	s.StringVar(&xlsxChartsSetSeriesStyleSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	s.StringVar(&xlsxChartsSetSeriesStyleChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	s.IntVar(&xlsxChartsSetSeriesStyleSeries, "series", 1, "1-based chart series number")
	s.StringVar(&xlsxChartsSetSeriesStyleFillColor, "fill-color", "", "series fill color as #RRGGBB")
	s.StringVar(&xlsxChartsSetSeriesStyleLineColor, "line-color", "", "series line color as #RRGGBB")
	s.Float64Var(&xlsxChartsSetSeriesStyleLineWidth, "line-width-pt", 0, "series line width in points")
	s.StringVar(&xlsxChartsSetSeriesStyleMarkerSym, "marker-symbol", "", "series marker symbol: circle, square, diamond, triangle, or none")
	s.IntVar(&xlsxChartsSetSeriesStyleMarkerSize, "marker-size", 0, "series marker size (2-72)")
	s.IntVar(&xlsxChartsSetSeriesStyleExpectCount, "expect-series-count", 0, "guard: require the chart to have this many series")
	AddMutationFlags(xlsxChartsSetSeriesStyleCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsSetSeriesStyleCmd)
}
