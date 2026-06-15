package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXRangesSetStyleResult struct {
	File          string                `json:"file"`
	Sheet         string                `json:"sheet"`
	SheetNumber   int                   `json:"sheetNumber"`
	Range         string                `json:"range"`
	Rows          int                   `json:"rows"`
	Cols          int                   `json:"cols"`
	Updated       int                   `json:"updated"`
	Created       int                   `json:"created"`
	CreatedStyles int                   `json:"createdStyles"`
	StyleIndexes  []int                 `json:"styleIndexes,omitempty"`
	Output        string                `json:"output,omitempty"`
	DryRun        bool                  `json:"dryRun"`
	Destination   *XLSXRangeDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
}

var (
	xlsxSetStyleSheet       string
	xlsxSetStyleRange       string
	xlsxSetStyleFontName    string
	xlsxSetStyleFontSize    float64
	xlsxSetStyleFontBold    bool
	xlsxSetStyleFontItalic  bool
	xlsxSetStyleFontUnder   bool
	xlsxSetStyleFontColor   string
	xlsxSetStyleFillColor   string
	xlsxSetStyleBorderStyle string
	xlsxSetStyleBorderColor string
	xlsxSetStyleBorderTop   bool
	xlsxSetStyleBorderBot   bool
	xlsxSetStyleBorderLeft  bool
	xlsxSetStyleBorderRight bool
	xlsxSetStyleAlignH      string
	xlsxSetStyleAlignV      string
	xlsxSetStyleWrap        bool
	xlsxSetStyleMaxCells    int
)

var xlsxRangesSetStyleCmd = &cobra.Command{
	Use:   "set-style <file>",
	Short: "Apply visual cell styles to a worksheet range",
	Long:  "Apply font, fill, border, and alignment styles to a worksheet A1 range, preserving existing number formats and unspecified style attributes.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if err := requireXLSXRangeSheet(xlsxSetStyleSheet); err != nil {
			return err
		}
		if xlsxSetStyleRange == "" {
			return InvalidArgsError("--range is required")
		}
		rangeRef, err := address.ParseRange(xlsxSetStyleRange)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
		}
		if err := checkXLSXRangeMaxCells(rangeRef, xlsxSetStyleMaxCells); err != nil {
			return err
		}
		spec, err := buildCellStyleSpec(cmd)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		wantReadback := GetGlobalConfig(cmd).Format == "json"

		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXRangesSetStyleResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxSetStyleSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			setResult, err := mutate.SetRangeStyle(&mutate.SetRangeStyleRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				StylesURI:   workbook.StylesURI,
				SheetRef:    sheetRef,
				Range:       rangeRef,
				Style:       *spec,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to set range style: %v", err)
			}
			workbook.StylesURI = setResult.StylesURI
			rows, cols := xlsxRangeDimensions(rangeRef)
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			var destination *XLSXRangeDestination
			if wantReadback {
				destination, err = collectXLSXRangeDestination(pkg, workbook, sheetRef, rangeRef, destinationFile)
				if err != nil {
					return err
				}
			}
			result = &XLSXRangesSetStyleResult{
				File:          filePath,
				Sheet:         sheetRef.Name,
				SheetNumber:   sheetRef.Number,
				Range:         setResult.Range,
				Rows:          rows,
				Cols:          cols,
				Updated:       setResult.Updated,
				Created:       setResult.Created,
				CreatedStyles: setResult.CreatedStyles,
				StyleIndexes:  setResult.StyleIndexes,
				Output:        destinationFile,
				DryRun:        mutOpts != nil && mutOpts.DryRun,
				Destination:   destination,
			}
			result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination)
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "ranges set-style")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("styled %s!%s (%dx%d), %d cells", result.Sheet, result.Range, result.Rows, result.Cols, result.Updated)))
	},
}

func buildCellStyleSpec(cmd *cobra.Command) (*mutate.CellStyleSpec, error) {
	spec := &mutate.CellStyleSpec{}
	flags := cmd.Flags()

	fontTouched := flags.Changed("font-name") || flags.Changed("font-size") || flags.Changed("font-bold") ||
		flags.Changed("font-italic") || flags.Changed("font-underline") || flags.Changed("font-color")
	if fontTouched {
		font := &mutate.FontSpec{}
		if flags.Changed("font-name") {
			font.Name, font.HasName = xlsxSetStyleFontName, true
		}
		if flags.Changed("font-size") {
			font.Size, font.HasSize = xlsxSetStyleFontSize, true
		}
		if flags.Changed("font-bold") {
			b := xlsxSetStyleFontBold
			font.Bold = &b
		}
		if flags.Changed("font-italic") {
			b := xlsxSetStyleFontItalic
			font.Italic = &b
		}
		if flags.Changed("font-underline") {
			b := xlsxSetStyleFontUnder
			font.Underline = &b
		}
		if flags.Changed("font-color") {
			font.Color, font.HasColor = xlsxSetStyleFontColor, true
		}
		spec.Font = font
	}

	if flags.Changed("fill-color") {
		spec.Fill = &mutate.FillSpec{Color: xlsxSetStyleFillColor}
	}

	if flags.Changed("border-style") || flags.Changed("border-color") ||
		flags.Changed("border-top") || flags.Changed("border-bottom") ||
		flags.Changed("border-left") || flags.Changed("border-right") {
		border := &mutate.BorderSpec{Style: xlsxSetStyleBorderStyle, Color: xlsxSetStyleBorderColor}
		anyEdge := flags.Changed("border-top") || flags.Changed("border-bottom") || flags.Changed("border-left") || flags.Changed("border-right")
		if anyEdge {
			border.Top = xlsxSetStyleBorderTop
			border.Bottom = xlsxSetStyleBorderBot
			border.Left = xlsxSetStyleBorderLeft
			border.Right = xlsxSetStyleBorderRight
		} else {
			// border-style/color without explicit edges => all four edges.
			border.Top, border.Bottom, border.Left, border.Right = true, true, true, true
		}
		if border.Style == "" {
			return nil, InvalidArgsError("--border-style is required when setting borders")
		}
		spec.Border = border
	}

	if flags.Changed("alignment-horizontal") || flags.Changed("alignment-vertical") || flags.Changed("alignment-wrap-text") {
		align := &mutate.AlignmentSpec{}
		if flags.Changed("alignment-horizontal") {
			align.Horizontal, align.HasH = xlsxSetStyleAlignH, true
		}
		if flags.Changed("alignment-vertical") {
			align.Vertical, align.HasV = xlsxSetStyleAlignV, true
		}
		if flags.Changed("alignment-wrap-text") {
			w := xlsxSetStyleWrap
			align.WrapText = &w
		}
		spec.Alignment = align
	}

	if spec.Font == nil && spec.Fill == nil && spec.Border == nil && spec.Alignment == nil {
		return nil, InvalidArgsError("specify at least one style flag (font/fill/border/alignment)")
	}
	if err := mutate.ValidateCellStyleSpec(spec); err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "%v", err)
	}
	return spec, nil
}

func init() {
	f := xlsxRangesSetStyleCmd.Flags()
	f.StringVar(&xlsxSetStyleSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	f.StringVar(&xlsxSetStyleRange, "range", "", "A1 range to style")
	f.StringVar(&xlsxSetStyleFontName, "font-name", "", "font family name")
	f.Float64Var(&xlsxSetStyleFontSize, "font-size", 11, "font size in points")
	f.BoolVar(&xlsxSetStyleFontBold, "font-bold", false, "bold text")
	f.BoolVar(&xlsxSetStyleFontItalic, "font-italic", false, "italic text")
	f.BoolVar(&xlsxSetStyleFontUnder, "font-underline", false, "underline text")
	f.StringVar(&xlsxSetStyleFontColor, "font-color", "", "font color hex such as #1A2B3C")
	f.StringVar(&xlsxSetStyleFillColor, "fill-color", "", "solid fill color hex such as #FFF2CC")
	f.StringVar(&xlsxSetStyleBorderStyle, "border-style", "", "border style: thin, medium, thick, double, dotted, dashed")
	f.StringVar(&xlsxSetStyleBorderColor, "border-color", "", "border color hex (default black)")
	f.BoolVar(&xlsxSetStyleBorderTop, "border-top", false, "apply border to top edge only")
	f.BoolVar(&xlsxSetStyleBorderBot, "border-bottom", false, "apply border to bottom edge only")
	f.BoolVar(&xlsxSetStyleBorderLeft, "border-left", false, "apply border to left edge only")
	f.BoolVar(&xlsxSetStyleBorderRight, "border-right", false, "apply border to right edge only")
	f.StringVar(&xlsxSetStyleAlignH, "alignment-horizontal", "", "horizontal alignment: left, center, right, fill, justify")
	f.StringVar(&xlsxSetStyleAlignV, "alignment-vertical", "", "vertical alignment: top, center, bottom")
	f.BoolVar(&xlsxSetStyleWrap, "alignment-wrap-text", false, "wrap text in cells")
	f.IntVar(&xlsxSetStyleMaxCells, "max-cells", 100000, "maximum cells to style (0 for unlimited)")
	AddMutationFlags(xlsxRangesSetStyleCmd)
	xlsxRangesCmd.AddCommand(xlsxRangesSetStyleCmd)
}
