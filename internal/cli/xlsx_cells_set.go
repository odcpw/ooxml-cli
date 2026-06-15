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
	"github.com/spf13/cobra"
)

type XLSXCellsSetResult struct {
	File          string                `json:"file"`
	Sheet         string                `json:"sheet"`
	SheetNumber   int                   `json:"sheetNumber"`
	Ref           string                `json:"ref"`
	Handle        string                `json:"handle,omitempty"`
	Type          mutate.CellValueType  `json:"type"`
	Value         string                `json:"value"`
	PreviousType  string                `json:"previousType,omitempty"`
	PreviousValue string                `json:"previousValue,omitempty"`
	Created       bool                  `json:"created"`
	Output        string                `json:"output,omitempty"`
	DryRun        bool                  `json:"dryRun"`
	Destination   *XLSXRangeDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
}

var (
	xlsxCellsSetSheet   string
	xlsxCellsSetCell    string
	xlsxCellsSetRef     string
	xlsxCellsSetValue   string
	xlsxCellsSetFormula string
	xlsxCellsSetType    string
)

var xlsxCellsSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set a worksheet cell value",
	Long:  "Set one worksheet cell to a string, number, boolean, or formula value.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		cellRef, sheetSelector, fromHandle, err := resolveXLSXSetCell(cmd)
		if err != nil {
			return err
		}
		valueType, value, err := resolveXLSXSetValue(cmd)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXCellsSet(filePath, cellRef, sheetSelector, fromHandle, valueType, value, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXCellsSetJSON(cmd, result)
		}
		return outputXLSXCellsSetText(cmd, result)
	},
}

// resolveXLSXSetCell returns the target cell A1 ref and, when --cell is a CELL
// HANDLE, a sheet-selector override carrying the handle's authoritative sheetId
// scope. When the override is non-empty it MUST be used for sheet selection
// instead of --sheet (the handle's scope wins, mirroring the PPTX "handle
// target ignores --slide" contract).
func resolveXLSXSetCell(cmd *cobra.Command) (cellRef string, sheetSelector string, fromHandle bool, err error) {
	cellChanged := cmd.Flags().Lookup("cell").Changed
	refChanged := cmd.Flags().Lookup("ref").Changed
	if cellChanged && refChanged {
		return "", "", false, InvalidArgsError("cannot specify both --cell and --ref")
	}

	refText := xlsxCellsSetCell
	if refChanged {
		refText = xlsxCellsSetRef
	}
	if strings.TrimSpace(refText) == "" {
		return "", "", false, InvalidArgsError("must specify --cell")
	}

	// Handle-first branch: a cell handle supplies BOTH the cell ref and the
	// sheet scope. --sheet is ignored for resolution.
	if xlsxhandle.IsHandle(refText) {
		h, perr := xlsxhandle.Parse(refText)
		if perr != nil {
			return "", "", false, mapXLSXHandleError(perr)
		}
		if h.Kind != xlsxhandle.KindCell {
			return "", "", false, InvalidArgsError("--cell handle must be a cell handle (H:xlsx/ws:<sheetId>/cell:a:<A1>)")
		}
		ref, cerr := address.ParseCell(h.CellRef)
		if cerr != nil {
			return "", "", false, NewCLIErrorf(ExitInvalidArgs, "invalid cell ref in handle: %v", cerr)
		}
		// The sheet override is the bare sheet-scope handle; selectXLSXSheet
		// re-resolves it through the same ambiguity-safe sheetId search.
		return ref.String(), xlsxhandle.FormatSheet(h.SheetID), true, nil
	}

	if strings.Contains(refText, ":") {
		return "", "", false, InvalidArgsError("--cell must be a single cell reference, not a range")
	}
	ref, err := address.ParseCell(refText)
	if err != nil {
		return "", "", false, NewCLIErrorf(ExitInvalidArgs, "invalid --cell: %v", err)
	}
	return ref.String(), "", false, nil
}

func resolveXLSXSetValue(cmd *cobra.Command) (mutate.CellValueType, string, error) {
	formulaChanged := cmd.Flags().Lookup("formula").Changed
	valueChanged := cmd.Flags().Lookup("value").Changed
	if formulaChanged && valueChanged {
		return "", "", InvalidArgsError("cannot specify both --value and --formula")
	}

	valueType, err := normalizeXLSXCellValueType(xlsxCellsSetType)
	if err != nil {
		return "", "", err
	}
	if formulaChanged {
		if strings.TrimSpace(xlsxCellsSetFormula) == "" {
			return "", "", InvalidArgsError("--formula cannot be empty")
		}
		return mutate.CellValueFormula, xlsxCellsSetFormula, nil
	}
	if !valueChanged {
		return "", "", InvalidArgsError("must specify --value or --formula")
	}
	if xlsxCellsSetValue == "" {
		return "", "", InvalidArgsError("--value cannot be empty; use xlsx cells clear")
	}
	return valueType, xlsxCellsSetValue, nil
}

func normalizeXLSXCellValueType(value string) (mutate.CellValueType, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", "string":
		return mutate.CellValueString, nil
	case "number":
		return mutate.CellValueNumber, nil
	case "bool", "boolean":
		return mutate.CellValueBool, nil
	case "formula":
		return mutate.CellValueFormula, nil
	case "auto":
		return mutate.CellValueAuto, nil
	default:
		return "", NewCLIErrorf(ExitInvalidArgs, "invalid --type %q (must be string, number, bool, formula, or auto)", value)
	}
}

func performXLSXCellsSet(filePath, cellRef, sheetSelector string, fromHandle bool, valueType mutate.CellValueType, value string, mutOpts *MutationOptions, wantReadback bool) (*XLSXCellsSetResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXCellsSetResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetArg := xlsxCellsSetSheet
		if sheetSelector != "" {
			// A cell handle's scope is authoritative; --sheet is ignored.
			sheetArg = sheetSelector
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetArg)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		if fromHandle {
			if err := requireXLSXCellHandleTargetExists(pkg, sheetRef, cellRef); err != nil {
				return err
			}
		}

		setResult, err := mutate.SetCell(&mutate.SetCellRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    sheetRef,
			Cell:        cellRef,
			Value:       value,
			Type:        valueType,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to set cell: %v", err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXRangeDestination
		if wantReadback {
			cell, err := address.ParseCell(setResult.Ref)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read destination cell %q: %v", setResult.Ref, err)
			}
			destination, err = collectXLSXRangeDestination(pkg, workbook, sheetRef, address.RangeRef{Start: cell, End: cell}, destinationFile)
			if err != nil {
				return err
			}
		}
		result = &XLSXCellsSetResult{
			File:          filePath,
			Sheet:         sheetRef.Name,
			SheetNumber:   sheetRef.Number,
			Ref:           setResult.Ref,
			Handle:        xlsxCellHandleString(sheetRef, setResult.Ref, xlsxSheetIDCounts(workbook.Sheets)),
			Type:          setResult.Type,
			Value:         setResult.Value,
			PreviousType:  setResult.PreviousType,
			PreviousValue: setResult.PreviousValue,
			Created:       setResult.Created,
			Output:        destinationFile,
			DryRun:        mutOpts != nil && mutOpts.DryRun,
			Destination:   destination,
		}
		result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputXLSXCellsSetJSON(cmd *cobra.Command, result *XLSXCellsSetResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal cells set JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXCellsSetText(cmd *cobra.Command, result *XLSXCellsSetResult) error {
	text := fmt.Sprintf("set %s!%s = %s (%s)\n", result.Sheet, result.Ref, result.Value, result.Type)
	return writeXLSXOutput(cmd, []byte(strings.TrimRight(text, "\n")))
}

func init() {
	xlsxCellsSetCmd.Flags().StringVar(&xlsxCellsSetSheet, "sheet", "", "sheet number (1-based) or exact sheet name (default: first sheet)")
	xlsxCellsSetCmd.Flags().StringVar(&xlsxCellsSetCell, "cell", "", "single A1 cell reference")
	xlsxCellsSetCmd.Flags().StringVar(&xlsxCellsSetRef, "ref", "", "alias for --cell")
	xlsxCellsSetCmd.Flags().StringVar(&xlsxCellsSetValue, "value", "", "cell value")
	xlsxCellsSetCmd.Flags().StringVar(&xlsxCellsSetFormula, "formula", "", "formula expression, with or without a leading =")
	xlsxCellsSetCmd.Flags().StringVar(&xlsxCellsSetType, "type", "string", "cell value type: string, number, bool, formula, or auto")
	AddMutationFlags(xlsxCellsSetCmd)
	xlsxCellsCmd.AddCommand(xlsxCellsSetCmd)
}
