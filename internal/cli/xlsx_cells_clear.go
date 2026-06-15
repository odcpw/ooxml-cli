package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXCellsClearResult struct {
	File        string                `json:"file"`
	Sheet       string                `json:"sheet"`
	SheetNumber int                   `json:"sheetNumber"`
	Range       string                `json:"range"`
	Cleared     int                   `json:"cleared"`
	Refs        []string              `json:"refs"`
	Output      string                `json:"output,omitempty"`
	DryRun      bool                  `json:"dryRun"`
	Destination *XLSXRangeDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
}

var (
	xlsxCellsClearSheet            string
	xlsxCellsClearRange            string
	xlsxCellsClearRef              string
	xlsxCellsClearReadbackMaxCells int
)

var xlsxCellsClearCmd = &cobra.Command{
	Use:   "clear <file>",
	Short: "Clear worksheet cell contents",
	Long:  "Clear cell contents in a worksheet A1 range while preserving cell formatting when present.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		rangeRef, sheetOverride, fromHandle, err := resolveXLSXClearRange(cmd)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		if xlsxCellsClearReadbackMaxCells < 0 {
			return InvalidArgsError("--readback-max-cells must be >= 0")
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXCellsClear(filePath, rangeRef, sheetOverride, fromHandle, mutOpts, wantReadback, xlsxCellsClearReadbackMaxCells)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXCellsClearJSON(cmd, result)
		}
		return outputXLSXCellsClearText(cmd, result)
	},
}

func resolveXLSXClearRange(cmd *cobra.Command) (address.RangeRef, string, bool, error) {
	rangeChanged := cmd.Flags().Lookup("range").Changed
	refChanged := cmd.Flags().Lookup("ref").Changed
	if rangeChanged && refChanged {
		return address.RangeRef{}, "", false, InvalidArgsError("cannot specify both --range and --ref")
	}

	rangeText := xlsxCellsClearRange
	if refChanged {
		rangeText = xlsxCellsClearRef
	}
	if strings.TrimSpace(rangeText) == "" {
		return address.RangeRef{}, "", false, InvalidArgsError("must specify --range")
	}
	if xlsxhandle.IsHandle(rangeText) {
		h, perr := xlsxhandle.Parse(rangeText)
		if perr != nil {
			return address.RangeRef{}, "", false, mapXLSXHandleError(perr)
		}
		if h.Kind != xlsxhandle.KindCell {
			return address.RangeRef{}, "", false, InvalidArgsError("--range handle must be a cell handle (H:xlsx/ws:<sheetId>/cell:a:<A1>)")
		}
		cell, cerr := address.ParseCell(h.CellRef)
		if cerr != nil {
			return address.RangeRef{}, "", false, NewCLIErrorf(ExitInvalidArgs, "invalid cell ref in handle: %v", cerr)
		}
		return address.RangeRef{Start: cell, End: cell}, xlsxhandle.FormatSheet(h.SheetID), true, nil
	}
	rangeRef, err := address.ParseRange(rangeText)
	if err != nil {
		return address.RangeRef{}, "", false, NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
	}
	return rangeRef, "", false, nil
}

func performXLSXCellsClear(filePath string, rangeRef address.RangeRef, sheetOverride string, fromHandle bool, mutOpts *MutationOptions, wantReadback bool, readbackMaxCells int) (*XLSXCellsClearResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXCellsClearResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetSelector := xlsxCellsClearSheet
		if sheetOverride != "" {
			sheetSelector = sheetOverride
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		if fromHandle {
			if err := requireXLSXCellHandleTargetExists(pkg, sheetRef, rangeRef.Start.String()); err != nil {
				return err
			}
		}

		clearResult, err := mutate.ClearCells(&mutate.ClearCellsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    sheetRef,
			Range:       rangeRef,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to clear cells: %v", err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXRangeDestination
		if wantReadback {
			destination, err = collectXLSXRangeDestinationWithMaxCells(pkg, workbook, sheetRef, rangeRef, destinationFile, readbackMaxCells)
			if err != nil {
				return err
			}
		}
		result = &XLSXCellsClearResult{
			File:        filePath,
			Sheet:       sheetRef.Name,
			SheetNumber: sheetRef.Number,
			Range:       rangeRef.String(),
			Cleared:     clearResult.Cleared,
			Refs:        clearResult.Refs,
			Output:      destinationFile,
			DryRun:      mutOpts != nil && mutOpts.DryRun,
			Destination: destination,
		}
		result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputXLSXCellsClearJSON(cmd *cobra.Command, result *XLSXCellsClearResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal cells clear JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXCellsClearText(cmd *cobra.Command, result *XLSXCellsClearResult) error {
	text := fmt.Sprintf("cleared %d cells in %s!%s", result.Cleared, result.Sheet, result.Range)
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxCellsClearCmd.Flags().StringVar(&xlsxCellsClearSheet, "sheet", "", "sheet number (1-based) or exact sheet name (default: first sheet)")
	xlsxCellsClearCmd.Flags().StringVar(&xlsxCellsClearRange, "range", "", "A1 cell or range to clear")
	xlsxCellsClearCmd.Flags().StringVar(&xlsxCellsClearRef, "ref", "", "alias for --range")
	xlsxCellsClearCmd.Flags().IntVar(&xlsxCellsClearReadbackMaxCells, "readback-max-cells", xlsxsheet.DefaultDenseCellLimit, "maximum cells to include in JSON destination readback (0 for unlimited)")
	AddMutationFlags(xlsxCellsClearCmd)
	xlsxCellsCmd.AddCommand(xlsxCellsClearCmd)
}
