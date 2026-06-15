package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type PlaceTableFromXLSXResult struct {
	File        string                `json:"file"`
	Output      string                `json:"output,omitempty"`
	DryRun      bool                  `json:"dryRun,omitempty"`
	Source      XLSXRangeSource       `json:"source"`
	Destination PlaceTableDestination `json:"destination"`
	PPTXBridgeReadbackCommands
}

type PlaceTableDestination struct {
	File            string     `json:"file,omitempty"`
	Slide           int        `json:"slide"`
	ShapeID         int        `json:"shapeId"`
	ShapeName       string     `json:"shapeName"`
	PrimarySelector string     `json:"primarySelector,omitempty"`
	Selectors       []string   `json:"selectors,omitempty"`
	Rows            int        `json:"rows"`
	Cols            int        `json:"cols"`
	Cells           [][]string `json:"cells,omitempty"`
	X               int64      `json:"x"`
	Y               int64      `json:"y"`
	CX              int64      `json:"cx"`
	CY              int64      `json:"cy"`
}

var (
	placeTableFromXLSXSlide         int
	placeTableFromXLSXWorkbook      string
	placeTableFromXLSXSheet         string
	placeTableFromXLSXRange         string
	placeTableFromXLSXTable         string
	placeTableFromXLSXMaxCells      int
	placeTableFromXLSXFormulaMode   string
	placeTableFromXLSXExpectRange   string
	placeTableFromXLSXX             int64
	placeTableFromXLSXY             int64
	placeTableFromXLSXWidth         int64
	placeTableFromXLSXHeight        int64
	placeTableFromXLSXHasHeader     bool
	placeTableFromXLSXHasBandedRows bool
	placeTableFromXLSXHeaderColor   string
	placeTableFromXLSXBand1Color    string
	placeTableFromXLSXBand2Color    string
	placeTableFromXLSXFontSize      int
	placeTableFromXLSXBorderColor   string
	placeTableFromXLSXBorderWidth   int64
	placeTableFromXLSXName          string
)

var placeTableFromXLSXCmd = &cobra.Command{
	Use:   "table-from-xlsx <file>",
	Short: "Place a PPTX table from an XLSX range or table",
	Long: `Place a new PowerPoint table on a slide from a rectangular XLSX worksheet range or named workbook table.

Required source flags:
  --workbook <path>
  one of:
    --sheet <sheet> --range <A1-ref>
    --table <selector> [--sheet <sheet>]

Required destination flags:
  --slide <n>
  --cx <emus>
  exactly one of --out <path>, --in-place, or --dry-run`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if strings.TrimSpace(placeTableFromXLSXWorkbook) == "" {
			return InvalidArgsError("--workbook is required")
		}
		if _, err := os.Stat(placeTableFromXLSXWorkbook); err != nil {
			return FileNotFoundError(placeTableFromXLSXWorkbook)
		}
		if placeTableFromXLSXSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if placeTableFromXLSXWidth <= 0 {
			return InvalidArgsError(fmt.Sprintf("table width must be positive: cx=%d", placeTableFromXLSXWidth))
		}
		if strings.TrimSpace(placeTableFromXLSXRange) != "" && strings.TrimSpace(placeTableFromXLSXTable) != "" {
			return InvalidArgsError("specify only one of --range or --table")
		}
		if strings.TrimSpace(placeTableFromXLSXRange) == "" && strings.TrimSpace(placeTableFromXLSXTable) == "" {
			return InvalidArgsError("must specify --range or --table")
		}
		formulaMode, err := normalizeXLSXFormulaMode(placeTableFromXLSXFormulaMode, "--formula-mode")
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPlaceTableFromXLSX(filePath, mutOpts, formulaMode)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPlaceTableFromXLSXJSON(cmd, result)
		}
		return outputPlaceTableFromXLSXText(cmd, result)
	},
}

func performPlaceTableFromXLSX(filePath string, mutOpts *MutationOptions, formulaMode string) (*PlaceTableFromXLSXResult, error) {
	source, tableData, err := loadPlaceTableFromXLSXData(formulaMode)
	if err != nil {
		return nil, err
	}
	if err := checkExpectedXLSXSourceRange(source.Range, placeTableFromXLSXExpectRange); err != nil {
		return nil, err
	}

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PlaceTableFromXLSXResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}
		if placeTableFromXLSXSlide > len(graph.Slides) {
			return InvalidArgsError(fmt.Sprintf("slide number %d out of range (1-%d)", placeTableFromXLSXSlide, len(graph.Slides)))
		}
		slideRef := graph.Slides[placeTableFromXLSXSlide-1]
		insertRes, err := mutate.InsertTable(&mutate.InsertTableRequest{
			Package:         pkg,
			SlideRef:        &slideRef,
			Data:            tableData,
			X:               placeTableFromXLSXX,
			Y:               placeTableFromXLSXY,
			Width:           placeTableFromXLSXWidth,
			Height:          placeTableFromXLSXHeight,
			HasHeader:       placeTableFromXLSXHasHeader,
			HasBandedRows:   placeTableFromXLSXHasBandedRows,
			HeaderFillColor: placeTableFromXLSXHeaderColor,
			BandFill1Color:  placeTableFromXLSXBand1Color,
			BandFill2Color:  placeTableFromXLSXBand2Color,
			DefaultFontSize: placeTableFromXLSXFontSize,
			BorderColor:     placeTableFromXLSXBorderColor,
			BorderWidth:     placeTableFromXLSXBorderWidth,
			ShapeName:       placeTableFromXLSXName,
		})
		if err != nil {
			return fmt.Errorf("failed to insert table: %w", err)
		}

		summary, err := collectPPTXSingleTable(pkg, &slideRef, insertRes.ShapeID)
		if err != nil {
			return err
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &PlaceTableFromXLSXResult{
			File:   filePath,
			Output: destinationFile,
			DryRun: mutOpts.DryRun,
			Source: *source,
			Destination: PlaceTableDestination{
				File:            destinationFile,
				Slide:           placeTableFromXLSXSlide,
				ShapeID:         insertRes.ShapeID,
				ShapeName:       insertRes.ShapeName,
				PrimarySelector: summary.PrimarySelector,
				Selectors:       append([]string{}, summary.Selectors...),
				Rows:            len(tableData),
				Cols:            len(tableData[0]),
				Cells:           summary.Cells,
				X:               placeTableFromXLSXX,
				Y:               placeTableFromXLSXY,
				CX:              insertRes.Width,
				CY:              insertRes.Height,
			},
		}
		result.PPTXBridgeReadbackCommands = pptxBridgeReadbackCommands(destinationFile, placeTableFromXLSXSlide, func(path string) string {
			return pptxTableReadbackCommand(path, placeTableFromXLSXSlide, summary.PrimarySelector)
		})
		return nil
	}); err != nil {
		return nil, err
	}
	if result == nil {
		return nil, NewCLIErrorf(ExitUnexpected, "table-from-xlsx did not produce a result")
	}
	return result, nil
}

func loadPlaceTableFromXLSXData(formulaMode string) (*XLSXRangeSource, [][]string, error) {
	source, matrix, err := loadXLSXRangeOrTableSourceForCLI(
		placeTableFromXLSXWorkbook,
		placeTableFromXLSXSheet,
		placeTableFromXLSXRange,
		placeTableFromXLSXTable,
		placeTableFromXLSXMaxCells,
	)
	if err != nil {
		return nil, nil, err
	}
	tableData := xlsxRangeStringsFromMatrix(matrix, formulaMode)
	if len(tableData) == 0 || len(tableData[0]) == 0 {
		return nil, nil, InvalidArgsError("source range is empty")
	}
	return source, tableData, nil
}

func mutationOutputPathForResult(inputPath string, mutOpts *MutationOptions) string {
	if mutOpts == nil || mutOpts.DryRun {
		return ""
	}
	if mutOpts.InPlace {
		return inputPath
	}
	return mutOpts.OutPath
}

func outputPlaceTableFromXLSXJSON(cmd *cobra.Command, result *PlaceTableFromXLSXResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal table-from-xlsx JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputPlaceTableFromXLSXText(cmd *cobra.Command, result *PlaceTableFromXLSXResult) error {
	text := fmt.Sprintf("placed slide %d table %s from %s!%s (%dx%d)", result.Destination.Slide, result.Destination.PrimarySelector, result.Source.Sheet, result.Source.Range, result.Source.Rows, result.Source.Cols)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	placeTableFromXLSXCmd.Flags().IntVarP(&placeTableFromXLSXSlide, "slide", "s", 0, "slide number (1-based, required)")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXWorkbook, "workbook", "", "source XLSX workbook path (required)")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXSheet, "sheet", "", "source sheet selector")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXRange, "range", "", "source A1 range")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXTable, "table", "", "source workbook table selector")
	placeTableFromXLSXCmd.Flags().IntVar(&placeTableFromXLSXMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXFormulaMode, "formula-mode", "value", "formula handling: value or formula")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXExpectRange, "expect-source-range", "", "fail if the resolved XLSX source range differs from this A1 range")
	placeTableFromXLSXCmd.Flags().Int64Var(&placeTableFromXLSXX, "x", 0, "left position in EMUs")
	placeTableFromXLSXCmd.Flags().Int64Var(&placeTableFromXLSXY, "y", 0, "top position in EMUs")
	placeTableFromXLSXCmd.Flags().Int64Var(&placeTableFromXLSXWidth, "cx", 0, "table width in EMUs (required)")
	placeTableFromXLSXCmd.Flags().Int64Var(&placeTableFromXLSXHeight, "cy", 0, "table height in EMUs (optional, auto-calculated if 0)")
	placeTableFromXLSXCmd.Flags().BoolVar(&placeTableFromXLSXHasHeader, "header", false, "first row is header")
	placeTableFromXLSXCmd.Flags().BoolVar(&placeTableFromXLSXHasBandedRows, "banded-rows", false, "alternate row fills")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXHeaderColor, "header-color", "4472C4", "header background color (hex)")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXBand1Color, "band1-color", "D9E1F2", "band 1 background color (hex)")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXBand2Color, "band2-color", "", "band 2 background color (hex, optional)")
	placeTableFromXLSXCmd.Flags().IntVar(&placeTableFromXLSXFontSize, "font-size", 18, "default font size in points")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXBorderColor, "border-color", "000000", "border color (hex)")
	placeTableFromXLSXCmd.Flags().Int64Var(&placeTableFromXLSXBorderWidth, "border-width", 19050, "border width in EMUs")
	placeTableFromXLSXCmd.Flags().StringVar(&placeTableFromXLSXName, "name", "", "shape name (auto-generated if empty)")
	AddMutationFlags(placeTableFromXLSXCmd)
	_ = placeTableFromXLSXCmd.MarkFlagRequired("workbook")
	_ = placeTableFromXLSXCmd.MarkFlagRequired("slide")
	_ = placeTableFromXLSXCmd.MarkFlagRequired("cx")
	placeCmd.AddCommand(placeTableFromXLSXCmd)
}
