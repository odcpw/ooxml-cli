package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
	"github.com/spf13/cobra"
)

type XLSXTablesAppendRowsResult struct {
	File           string                      `json:"file"`
	Table          string                      `json:"table"`
	Sheet          string                      `json:"sheet"`
	SheetNumber    int                         `json:"sheetNumber"`
	PreviousRange  string                      `json:"previousRange"`
	Range          string                      `json:"range"`
	AppendRange    string                      `json:"appendRange"`
	RowsAppended   int                         `json:"rowsAppended"`
	Updated        int                         `json:"updated"`
	Created        int                         `json:"created"`
	Cleared        int                         `json:"cleared"`
	Skipped        int                         `json:"skipped"`
	FormulaCount   int                         `json:"formulaCount"`
	DataFormat     string                      `json:"dataFormat"`
	NullPolicy     string                      `json:"nullPolicy"`
	MajorDimension string                      `json:"majorDimension"`
	Output         string                      `json:"output,omitempty"`
	DryRun         bool                        `json:"dryRun"`
	Destination    *XLSXTableAppendDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
	XLSXTableAppendReadbackCommands
}

var (
	xlsxTablesAppendRowsSheet             string
	xlsxTablesAppendRowsTable             string
	xlsxTablesAppendRowsValues            string
	xlsxTablesAppendRowsValuesFile        string
	xlsxTablesAppendRowsDataFormat        string
	xlsxTablesAppendRowsNullPolicy        string
	xlsxTablesAppendRowsRagged            string
	xlsxTablesAppendRowsMaxCells          int
	xlsxTablesAppendRowsOverwriteFormulas bool
)

var xlsxTablesAppendRowsCmd = &cobra.Command{
	Use:   "append-rows <file>",
	Short: "Append rows to an existing table",
	Long:  "Append rows below an existing XLSX table and expand table and autofilter ranges. This command does not create new tables.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		dataFormat, err := rangeio.NormalizeDataFormat(xlsxTablesAppendRowsDataFormat)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		matrix, err := resolveXLSXTablesAppendRowsMatrix(cmd, dataFormat)
		if err != nil {
			return err
		}
		rows, rowCount, colCount, err := rangeio.Rectangularize(matrix.Values, xlsxTablesAppendRowsRagged)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		if rowCount < 1 || colCount < 1 {
			return InvalidArgsError("values matrix cannot be empty")
		}
		if xlsxTablesAppendRowsMaxCells < 0 {
			return InvalidArgsError("--max-cells must be >= 0")
		}
		if xlsxTablesAppendRowsMaxCells > 0 && rowCount*colCount > xlsxTablesAppendRowsMaxCells {
			return NewCLIErrorf(ExitInvalidArgs, "values matrix contains %d cells, above --max-cells %d", rowCount*colCount, xlsxTablesAppendRowsMaxCells)
		}
		nullPolicyText := xlsxTablesAppendRowsNullPolicy
		if !cmd.Flags().Lookup("null-policy").Changed && matrix.NullPolicy != "" {
			nullPolicyText = matrix.NullPolicy
		}
		nullPolicy, err := xlsxmutate.NormalizeRangeNullPolicy(nullPolicyText)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		mutRows, err := xlsxRangeCellsToMutate(rows)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXTablesAppendRows(filePath, mutRows, nullPolicy, dataFormat, matrix.MajorDimension, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXTablesAppendRowsJSON(cmd, result)
		}
		return outputXLSXTablesAppendRowsText(cmd, result)
	},
}

func resolveXLSXTablesAppendRowsMatrix(cmd *cobra.Command, dataFormat string) (*rangeio.Matrix, error) {
	valuesChanged := cmd.Flags().Lookup("values").Changed
	valuesFileChanged := cmd.Flags().Lookup("values-file").Changed
	if valuesChanged == valuesFileChanged {
		return nil, InvalidArgsError("must specify exactly one of --values or --values-file")
	}

	var (
		data []byte
		err  error
	)
	if valuesChanged {
		data = []byte(xlsxTablesAppendRowsValues)
	} else if xlsxTablesAppendRowsValuesFile == "-" {
		data, err = io.ReadAll(cmd.InOrStdin())
	} else {
		data, err = os.ReadFile(xlsxTablesAppendRowsValuesFile)
	}
	if err != nil {
		return nil, FileNotFoundError(xlsxTablesAppendRowsValuesFile)
	}
	matrix, err := rangeio.Decode(data, dataFormat)
	if err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid %s values: %v", dataFormat, err)
	}
	return matrix, nil
}

func performXLSXTablesAppendRows(filePath string, rows [][]xlsxmutate.RangeCell, nullPolicy xlsxmutate.RangeNullPolicy, dataFormat, majorDimension string, mutOpts *MutationOptions, wantReadback bool) (*XLSXTablesAppendRowsResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXTablesAppendRowsResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheets := workbook.Sheets
		if xlsxTablesAppendRowsSheet != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, xlsxTablesAppendRowsSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(selected); err != nil {
				return err
			}
			sheets = []model.SheetRef{selected}
		}
		tables, err := xlsxtable.List(pkg, workbook, sheets)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list tables: %v", err)
		}
		tableRef, err := selectXLSXTable(tables, xlsxTablesAppendRowsTable)
		if err != nil {
			return err
		}
		appendResult, err := xlsxmutate.AppendTableRows(&xlsxmutate.AppendTableRowsRequest{
			Package:           pkg,
			WorkbookURI:       workbook.PartURI,
			Table:             tableRef,
			Rows:              rows,
			NullPolicy:        nullPolicy,
			OverwriteFormulas: xlsxTablesAppendRowsOverwriteFormulas,
		})
		if err != nil {
			return mapXLSXTablesAppendRowsMutationError(err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXTableAppendDestination
		if wantReadback {
			destination, err = collectXLSXTableAppendDestination(pkg, workbook, tableRef, appendResult.PreviousRange, appendResult.AppendRange, destinationFile)
			if err != nil {
				return err
			}
		}
		result = &XLSXTablesAppendRowsResult{
			File:           filePath,
			Table:          appendResult.Table,
			Sheet:          appendResult.Sheet,
			SheetNumber:    appendResult.SheetNumber,
			PreviousRange:  appendResult.PreviousRange,
			Range:          appendResult.Range,
			AppendRange:    appendResult.AppendRange,
			RowsAppended:   appendResult.RowsAppended,
			Updated:        appendResult.Updated,
			Created:        appendResult.Created,
			Cleared:        appendResult.Cleared,
			Skipped:        appendResult.Skipped,
			FormulaCount:   appendResult.FormulaCount,
			DataFormat:     dataFormat,
			NullPolicy:     string(nullPolicy),
			MajorDimension: majorDimension,
			Output:         destinationFile,
			DryRun:         mutOpts != nil && mutOpts.DryRun,
			Destination:    destination,
		}
		if destination != nil {
			result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination.Appended)
			result.XLSXTableAppendReadbackCommands = xlsxTableAppendReadbackCommands(destination)
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func mapXLSXTablesAppendRowsMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	if errors.Is(err, xlsxmutate.ErrTableHasTotals) ||
		errors.Is(err, xlsxmutate.ErrTableHasCalculatedColumns) ||
		errors.Is(err, xlsxmutate.ErrTableHasUnsupportedFeatures) ||
		errors.Is(err, xlsxmutate.ErrTableColumnCountMismatch) ||
		errors.Is(err, xlsxmutate.ErrTableAppendWouldOverwrite) ||
		errors.Is(err, xlsxmutate.ErrRangeOverwritesFormula) ||
		errors.Is(err, xlsxmutate.ErrRangeIntersectsMergedCell) {
		return InvalidArgsError(err.Error())
	}
	return NewCLIErrorf(ExitInvalidArgs, "failed to append table rows: %v", err)
}

func outputXLSXTablesAppendRowsJSON(cmd *cobra.Command, result *XLSXTablesAppendRowsResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal tables append-rows JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXTablesAppendRowsText(cmd *cobra.Command, result *XLSXTablesAppendRowsResult) error {
	text := fmt.Sprintf("appended %d rows to %s!%s: %s -> %s", result.RowsAppended, result.Sheet, result.Table, result.PreviousRange, result.Range)
	if result.FormulaCount > 0 {
		text += fmt.Sprintf(", formulas %d", result.FormulaCount)
	}
	if result.Skipped > 0 {
		text += fmt.Sprintf(", skipped %d", result.Skipped)
	}
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsTable, "table", "", "table number, name, or displayName")
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsValues, "values", "", "inline JSON/CSV/TSV matrix data")
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsValuesFile, "values-file", "", "path to JSON/CSV/TSV matrix data, or - for stdin")
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsDataFormat, "data-format", "json", "matrix data format: json, csv, or tsv")
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsNullPolicy, "null-policy", "skip", "null handling: skip, clear, or empty-string")
	xlsxTablesAppendRowsCmd.Flags().StringVar(&xlsxTablesAppendRowsRagged, "ragged", "reject", "ragged row handling: reject or fill-empty")
	xlsxTablesAppendRowsCmd.Flags().IntVar(&xlsxTablesAppendRowsMaxCells, "max-cells", 100000, "maximum cells to append (0 for unlimited)")
	xlsxTablesAppendRowsCmd.Flags().BoolVar(&xlsxTablesAppendRowsOverwriteFormulas, "overwrite-formulas", false, "allow writing formulas over existing formula cells")
	AddMutationFlags(xlsxTablesAppendRowsCmd)
	xlsxTablesCmd.AddCommand(xlsxTablesAppendRowsCmd)
}
