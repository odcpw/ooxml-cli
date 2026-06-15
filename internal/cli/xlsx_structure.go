package cli

import (
	"errors"
	"fmt"
	"strings"

	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXStructureMutationResult struct {
	File         string `json:"file"`
	Sheet        string `json:"sheet"`
	SheetNumber  int    `json:"sheetNumber"`
	Axis         string `json:"axis"`
	Operation    string `json:"operation"`
	Start        int    `json:"start"`
	StartColumn  string `json:"startColumn,omitempty"`
	Count        int    `json:"count"`
	ShiftedRows  int    `json:"shiftedRows,omitempty"`
	ShiftedCells int    `json:"shiftedCells"`
	RemovedRows  int    `json:"removedRows,omitempty"`
	RemovedCells int    `json:"removedCells,omitempty"`
	OldUsedRange string `json:"oldUsedRange,omitempty"`
	NewUsedRange string `json:"newUsedRange,omitempty"`
	Output       string `json:"output,omitempty"`
	DryRun       bool   `json:"dryRun"`
	XLSXStructureReadbackCommands
}

func mapXLSXStructureResult(filePath string, result *xlsxmutate.StructureMutationResult) *XLSXStructureMutationResult {
	if result == nil {
		return nil
	}
	return &XLSXStructureMutationResult{
		File:         filePath,
		Sheet:        result.Sheet,
		SheetNumber:  result.SheetNumber,
		Axis:         result.Axis,
		Operation:    result.Operation,
		Start:        result.Start,
		StartColumn:  result.StartColumn,
		Count:        result.Count,
		ShiftedRows:  result.ShiftedRows,
		ShiftedCells: result.ShiftedCells,
		RemovedRows:  result.RemovedRows,
		RemovedCells: result.RemovedCells,
		OldUsedRange: result.OldUsedRange,
		NewUsedRange: result.NewUsedRange,
	}
}

// xlsxStructureResultSelector derives the sheet selector for the mutated
// worksheet from the mutation result. The structure result carries the sheet
// name and 1-based number (but no PrimarySelector), so this reuses the
// name/number fallback path of xlsxSheetSelector to produce a selector the
// sheets-show readback can target.
func xlsxStructureResultSelector(result *XLSXStructureMutationResult) string {
	if result == nil {
		return ""
	}
	return xlsxSheetSelector("", result.Sheet, result.SheetNumber)
}

func requireXLSXStructureSheet(sheet string) error {
	if strings.TrimSpace(sheet) == "" {
		return InvalidArgsError("--sheet is required for structural row/column edits")
	}
	return nil
}

func normalizeXLSXStructureCount(count int) error {
	if count < 1 {
		return InvalidArgsError("--count must be positive")
	}
	return nil
}

func mapXLSXStructureMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, xlsxmutate.ErrWorksheetHasFormulas),
		errors.Is(err, xlsxmutate.ErrWorkbookHasDefinedNames),
		errors.Is(err, xlsxmutate.ErrWorkbookHasCalcChain),
		errors.Is(err, xlsxmutate.ErrWorksheetHasMergedCells),
		errors.Is(err, xlsxmutate.ErrWorksheetHasTables),
		errors.Is(err, xlsxmutate.ErrWorksheetHasAutofilter),
		errors.Is(err, xlsxmutate.ErrWorksheetHasDrawings),
		errors.Is(err, xlsxmutate.ErrWorksheetHasHyperlinks),
		errors.Is(err, xlsxmutate.ErrWorksheetHasConditionalFormatting),
		errors.Is(err, xlsxmutate.ErrWorksheetHasDataValidations),
		errors.Is(err, xlsxmutate.ErrWorksheetHasColumnMetadata),
		errors.Is(err, xlsxmutate.ErrWorksheetHasInvalidReferences),
		errors.Is(err, xlsxmutate.ErrWorksheetHasUnsupportedStructure),
		errors.Is(err, xlsxmutate.ErrWorksheetStructureOutOfBounds):
		return InvalidArgsError(err.Error())
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate worksheet structure: %v", err)
	}
}

func outputXLSXStructureJSON(cmd *cobra.Command, value *XLSXStructureMutationResult, label string) error {
	data, err := marshalLabeledJSON(cmd, value, label)
	if err != nil {
		return err
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXStructureText(cmd *cobra.Command, result *XLSXStructureMutationResult) error {
	text := fmt.Sprintf("%s %s %s %d count %d on %s", result.Operation, result.Axis, result.Sheet, result.Start, result.Count, result.File)
	if result.StartColumn != "" {
		text = fmt.Sprintf("%s %s %s %s count %d on %s", result.Operation, result.Axis, result.Sheet, result.StartColumn, result.Count, result.File)
	}
	return writeXLSXOutput(cmd, []byte(text))
}
