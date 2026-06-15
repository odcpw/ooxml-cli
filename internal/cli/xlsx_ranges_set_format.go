package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXRangesSetFormatResult struct {
	File           string                `json:"file"`
	Sheet          string                `json:"sheet"`
	SheetNumber    int                   `json:"sheetNumber"`
	Range          string                `json:"range"`
	Rows           int                   `json:"rows"`
	Cols           int                   `json:"cols"`
	Preset         string                `json:"preset,omitempty"`
	FormatCode     string                `json:"formatCode"`
	NumberFormatID int                   `json:"numberFormatId"`
	Builtin        bool                  `json:"builtin"`
	Updated        int                   `json:"updated"`
	Created        int                   `json:"created"`
	CreatedStyles  int                   `json:"createdStyles"`
	StyleIndexes   []int                 `json:"styleIndexes,omitempty"`
	Output         string                `json:"output,omitempty"`
	DryRun         bool                  `json:"dryRun"`
	Destination    *XLSXRangeDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
}

var (
	xlsxRangesSetFormatSheet          string
	xlsxRangesSetFormatRange          string
	xlsxRangesSetFormatPreset         string
	xlsxRangesSetFormatCode           string
	xlsxRangesSetFormatDecimals       int
	xlsxRangesSetFormatCurrencySymbol string
	xlsxRangesSetFormatMaxCells       int
)

var xlsxRangesSetFormatCmd = &cobra.Command{
	Use:   "set-format <file>",
	Short: "Apply a practical number format to a worksheet range",
	Long:  "Apply integer, number, currency, percent, date, datetime, text, general, or custom number formats to a worksheet A1 range.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXRangeSheet(strings.TrimSpace(xlsxRangesSetFormatSheet)); err != nil {
			return err
		}
		if strings.TrimSpace(xlsxRangesSetFormatRange) == "" {
			return InvalidArgsError("--range is required")
		}
		rangeRef, err := address.ParseRange(xlsxRangesSetFormatRange)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
		}
		if err := checkXLSXRangeMaxCells(rangeRef, xlsxRangesSetFormatMaxCells); err != nil {
			return err
		}
		spec, err := mutate.ResolveNumberFormat(mutate.NumberFormatOptions{
			Preset:         xlsxRangesSetFormatPreset,
			FormatCode:     xlsxRangesSetFormatCode,
			Decimals:       xlsxRangesSetFormatDecimals,
			CurrencySymbol: xlsxRangesSetFormatCurrencySymbol,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXRangesSetFormat(filePath, rangeRef, spec, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXRangesSetFormatJSON(cmd, result)
		}
		return outputXLSXRangesSetFormatText(cmd, result)
	},
}

func performXLSXRangesSetFormat(filePath string, rangeRef address.RangeRef, spec mutate.NumberFormatSpec, mutOpts *MutationOptions, wantReadback bool) (*XLSXRangesSetFormatResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXRangesSetFormatResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxRangesSetFormatSheet)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		setResult, err := mutate.SetRangeNumberFormat(&mutate.SetRangeFormatRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			StylesURI:   workbook.StylesURI,
			SheetRef:    sheetRef,
			Range:       rangeRef,
			Format:      spec,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to set range format: %v", err)
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
		result = &XLSXRangesSetFormatResult{
			File:           filePath,
			Sheet:          sheetRef.Name,
			SheetNumber:    sheetRef.Number,
			Range:          setResult.Range,
			Rows:           rows,
			Cols:           cols,
			Preset:         spec.Preset,
			FormatCode:     setResult.FormatCode,
			NumberFormatID: setResult.NumFmtID,
			Builtin:        setResult.Builtin,
			Updated:        setResult.Updated,
			Created:        setResult.Created,
			CreatedStyles:  setResult.CreatedStyles,
			StyleIndexes:   setResult.StyleIndexes,
			Output:         destinationFile,
			DryRun:         mutOpts != nil && mutOpts.DryRun,
			Destination:    destination,
		}
		result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputXLSXRangesSetFormatJSON(cmd *cobra.Command, result *XLSXRangesSetFormatResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal ranges set-format JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXRangesSetFormatText(cmd *cobra.Command, result *XLSXRangesSetFormatResult) error {
	text := fmt.Sprintf("formatted %s!%s (%dx%d): %s", result.Sheet, result.Range, result.Rows, result.Cols, result.FormatCode)
	if result.Created > 0 {
		text += fmt.Sprintf(", created %d blank cells", result.Created)
	}
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxRangesSetFormatCmd.Flags().StringVar(&xlsxRangesSetFormatSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRangesSetFormatCmd.Flags().StringVar(&xlsxRangesSetFormatRange, "range", "", "A1 range to format")
	xlsxRangesSetFormatCmd.Flags().StringVar(&xlsxRangesSetFormatPreset, "preset", "", "format preset: integer, number, currency, percent, date, datetime, text, or general")
	xlsxRangesSetFormatCmd.Flags().StringVar(&xlsxRangesSetFormatCode, "format-code", "", "custom SpreadsheetML number format code")
	xlsxRangesSetFormatCmd.Flags().IntVar(&xlsxRangesSetFormatDecimals, "decimals", 2, "decimal places for number, currency, and percent presets")
	xlsxRangesSetFormatCmd.Flags().StringVar(&xlsxRangesSetFormatCurrencySymbol, "currency-symbol", "$", "currency literal for the currency preset")
	xlsxRangesSetFormatCmd.Flags().IntVar(&xlsxRangesSetFormatMaxCells, "max-cells", 100000, "maximum cells to format (0 for unlimited)")
	AddMutationFlags(xlsxRangesSetFormatCmd)
	xlsxRangesCmd.AddCommand(xlsxRangesSetFormatCmd)
}
