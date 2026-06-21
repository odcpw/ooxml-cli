package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXCellsSetBatchResult struct {
	File         string                 `json:"file"`
	Sheet        string                 `json:"sheet"`
	SheetNumber  int                    `json:"sheetNumber"`
	Updated      int                    `json:"updated"`
	Created      int                    `json:"created"`
	FormulaCount int                    `json:"formulaCount"`
	Range        string                 `json:"range,omitempty"`
	Cells        []XLSXCellsSetCellInfo `json:"cells,omitempty"`
	Output       string                 `json:"output,omitempty"`
	DryRun       bool                   `json:"dryRun"`
	Destination  *XLSXRangeDestination  `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
}

type XLSXCellsSetCellInfo struct {
	Ref           string               `json:"ref"`
	Type          mutate.CellValueType `json:"type"`
	Value         string               `json:"value"`
	PreviousType  string               `json:"previousType,omitempty"`
	PreviousValue string               `json:"previousValue,omitempty"`
	Created       bool                 `json:"created"`
}

type XLSXCellsSetBatchEntry struct {
	Ref     string `json:"ref"`
	Cell    string `json:"cell,omitempty"`
	Type    string `json:"type,omitempty"`
	Value   string `json:"value,omitempty"`
	Formula string `json:"formula,omitempty"`
}

var (
	xlsxCellsSetBatchSheet            string
	xlsxCellsSetBatchCellsJSON        string
	xlsxCellsSetBatchCellsFile        string
	xlsxCellsSetBatchDetails          bool
	xlsxCellsSetBatchReadbackMaxCells int
)

var xlsxCellsSetBatchCmd = &cobra.Command{
	Use:   "set-batch <file>",
	Short: "Set multiple worksheet cells",
	Long:  "Set multiple worksheet cells in one safe XLSX mutation.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		assignments, err := resolveXLSXSetBatchAssignments(cmd)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		if xlsxCellsSetBatchReadbackMaxCells < 0 {
			return InvalidArgsError("--readback-max-cells must be >= 0")
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXCellsSetBatch(filePath, assignments, mutOpts, wantReadback, xlsxCellsSetBatchReadbackMaxCells)
		if err != nil {
			return err
		}
		if !xlsxCellsSetBatchDetails {
			result.Cells = nil
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXCellsSetBatchJSON(cmd, result)
		}
		return outputXLSXCellsSetBatchText(cmd, result)
	},
}

func resolveXLSXSetBatchAssignments(cmd *cobra.Command) ([]mutate.CellAssignment, error) {
	cellsChanged := cmd.Flags().Lookup("cells").Changed
	cellsFileChanged := cmd.Flags().Lookup("cells-file").Changed
	if cellsChanged == cellsFileChanged {
		return nil, InvalidArgsError("must specify exactly one of --cells or --cells-file")
	}

	var data []byte
	var err error
	if cellsChanged {
		data = []byte(xlsxCellsSetBatchCellsJSON)
	} else if xlsxCellsSetBatchCellsFile == "-" {
		data, err = io.ReadAll(cmd.InOrStdin())
	} else {
		data, err = os.ReadFile(xlsxCellsSetBatchCellsFile)
	}
	if err != nil {
		return nil, FileNotFoundError(xlsxCellsSetBatchCellsFile)
	}

	var entries []XLSXCellsSetBatchEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid cells JSON: %v", err)
	}
	if len(entries) == 0 {
		return nil, InvalidArgsError("cells batch cannot be empty")
	}

	assignments := make([]mutate.CellAssignment, 0, len(entries))
	for idx, entry := range entries {
		ref := entry.Ref
		if ref == "" {
			ref = entry.Cell
		}
		if ref == "" {
			return nil, NewCLIErrorf(ExitInvalidArgs, "cells[%d] missing ref", idx)
		}
		typ, err := normalizeXLSXCellValueType(entry.Type)
		if err != nil {
			return nil, err
		}
		value := entry.Value
		if entry.Formula != "" {
			if entry.Value != "" {
				return nil, NewCLIErrorf(ExitInvalidArgs, "cells[%d] cannot specify both value and formula", idx)
			}
			typ = mutate.CellValueFormula
			value = entry.Formula
		}
		if value == "" {
			return nil, NewCLIErrorf(ExitInvalidArgs, "cells[%d] value cannot be empty; use xlsx cells clear", idx)
		}
		assignments = append(assignments, mutate.CellAssignment{
			Ref:   ref,
			Type:  typ,
			Value: value,
		})
	}
	return assignments, nil
}

func performXLSXCellsSetBatch(filePath string, assignments []mutate.CellAssignment, mutOpts *MutationOptions, wantReadback bool, readbackMaxCells int) (*XLSXCellsSetBatchResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXCellsSetBatchResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxCellsSetBatchSheet)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}

		setResult, err := mutate.SetCells(&mutate.SetCellsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    sheetRef,
			Cells:       assignments,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to set cells: %v", err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXRangeDestination
		if wantReadback && setResult.Range != "" {
			rangeRef, err := address.ParseRange(setResult.Range)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read destination range %q: %v", setResult.Range, err)
			}
			destination, err = collectXLSXRangeDestinationWithMaxCells(pkg, workbook, sheetRef, rangeRef, destinationFile, readbackMaxCells)
			if err != nil {
				return err
			}
		}
		result = &XLSXCellsSetBatchResult{
			File:         filePath,
			Sheet:        sheetRef.Name,
			SheetNumber:  sheetRef.Number,
			Updated:      setResult.Updated,
			Created:      setResult.Created,
			FormulaCount: setResult.FormulaCount,
			Range:        setResult.Range,
			Cells:        make([]XLSXCellsSetCellInfo, 0, len(setResult.Cells)),
			Output:       destinationFile,
			DryRun:       mutOpts != nil && mutOpts.DryRun,
			Destination:  destination,
		}
		result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination)
		for _, cell := range setResult.Cells {
			result.Cells = append(result.Cells, XLSXCellsSetCellInfo{
				Ref:           cell.Ref,
				Type:          cell.Type,
				Value:         cell.Value,
				PreviousType:  cell.PreviousType,
				PreviousValue: cell.PreviousValue,
				Created:       cell.Created,
			})
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputXLSXCellsSetBatchJSON(cmd *cobra.Command, result *XLSXCellsSetBatchResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal cells set-batch JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXCellsSetBatchText(cmd *cobra.Command, result *XLSXCellsSetBatchResult) error {
	text := fmt.Sprintf("set %d cells in %s", result.Updated, result.Sheet)
	if result.Range != "" {
		text += fmt.Sprintf(" (%s)", result.Range)
	}
	if result.FormulaCount > 0 {
		text += fmt.Sprintf("; formulas: %d", result.FormulaCount)
	}
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxCellsSetBatchCmd.Flags().StringVar(&xlsxCellsSetBatchSheet, "sheet", "", "sheet number (1-based) or exact sheet name (default: first sheet)")
	xlsxCellsSetBatchCmd.Flags().StringVar(&xlsxCellsSetBatchCellsJSON, "cells", "", "JSON array of cell assignments")
	xlsxCellsSetBatchCmd.Flags().StringVar(&xlsxCellsSetBatchCellsFile, "cells-file", "", "path to JSON array of cell assignments, or - for stdin")
	xlsxCellsSetBatchCmd.Flags().BoolVar(&xlsxCellsSetBatchDetails, "details", false, "include per-cell details in JSON output")
	xlsxCellsSetBatchCmd.Flags().IntVar(&xlsxCellsSetBatchReadbackMaxCells, "readback-max-cells", xlsxsheet.DefaultDenseCellLimit, "maximum cells to include in JSON destination readback (0 for unlimited)")
	AddMutationFlags(xlsxCellsSetBatchCmd)
	xlsxCellsCmd.AddCommand(xlsxCellsSetBatchCmd)
}
