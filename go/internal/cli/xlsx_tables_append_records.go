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

type XLSXTablesAppendRecordsResult struct {
	File               string                      `json:"file"`
	Table              string                      `json:"table"`
	Sheet              string                      `json:"sheet"`
	SheetNumber        int                         `json:"sheetNumber"`
	PreviousRange      string                      `json:"previousRange"`
	Range              string                      `json:"range"`
	AppendRange        string                      `json:"appendRange"`
	RowsAppended       int                         `json:"rowsAppended"`
	Updated            int                         `json:"updated"`
	Created            int                         `json:"created"`
	Cleared            int                         `json:"cleared"`
	Skipped            int                         `json:"skipped"`
	FormulaCount       int                         `json:"formulaCount"`
	DataFormat         string                      `json:"dataFormat"`
	NullPolicy         string                      `json:"nullPolicy"`
	MissingPolicy      string                      `json:"missingPolicy"`
	IgnoredExtraFields bool                        `json:"ignoredExtraFields"`
	Columns            []string                    `json:"columns"`
	Output             string                      `json:"output,omitempty"`
	DryRun             bool                        `json:"dryRun"`
	Destination        *XLSXTableAppendDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
	XLSXTableAppendReadbackCommands
}

var (
	xlsxTablesAppendRecordsSheet             string
	xlsxTablesAppendRecordsTable             string
	xlsxTablesAppendRecordsExpectRange       string
	xlsxTablesAppendRecordsRecords           string
	xlsxTablesAppendRecordsRecordsFile       string
	xlsxTablesAppendRecordsMissing           string
	xlsxTablesAppendRecordsNullPolicy        string
	xlsxTablesAppendRecordsMaxCells          int
	xlsxTablesAppendRecordsIgnoreExtraFields bool
	xlsxTablesAppendRecordsOverwriteFormulas bool
)

var xlsxTablesAppendRecordsCmd = &cobra.Command{
	Use:   "append-records <file>",
	Short: "Append JSON records to an existing table by column name",
	Long:  "Append JSON object records to an existing XLSX table by matching object keys to table column names.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxTablesAppendRecordsTable == "" {
			return InvalidArgsError("--table is required")
		}
		if xlsxTablesAppendRecordsExpectRange == "" {
			return InvalidArgsError("--expect-range is required")
		}
		if xlsxTablesAppendRecordsMaxCells < 0 {
			return InvalidArgsError("--max-cells must be >= 0")
		}
		missingPolicy, err := rangeio.NormalizeMissingPolicy(xlsxTablesAppendRecordsMissing)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		nullPolicy, err := xlsxmutate.NormalizeRangeNullPolicy(xlsxTablesAppendRecordsNullPolicy)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		recordSet, err := resolveXLSXTablesAppendRecords(cmd)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXTablesAppendRecords(filePath, recordSet.Records, missingPolicy, nullPolicy, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXTablesAppendRecordsJSON(cmd, result)
		}
		return outputXLSXTablesAppendRecordsText(cmd, result)
	},
}

func resolveXLSXTablesAppendRecords(cmd *cobra.Command) (*rangeio.RecordSet, error) {
	recordsChanged := cmd.Flags().Lookup("records").Changed
	recordsFileChanged := cmd.Flags().Lookup("records-file").Changed
	if recordsChanged == recordsFileChanged {
		return nil, InvalidArgsError("must specify exactly one of --records or --records-file")
	}

	var (
		data []byte
		err  error
	)
	if recordsChanged {
		data = []byte(xlsxTablesAppendRecordsRecords)
	} else if xlsxTablesAppendRecordsRecordsFile == "-" {
		data, err = io.ReadAll(cmd.InOrStdin())
	} else {
		data, err = os.ReadFile(xlsxTablesAppendRecordsRecordsFile)
	}
	if err != nil {
		return nil, FileNotFoundError(xlsxTablesAppendRecordsRecordsFile)
	}
	recordSet, err := rangeio.DecodeRecords(data)
	if err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid JSON records: %v", err)
	}
	return recordSet, nil
}

func performXLSXTablesAppendRecords(filePath string, records []map[string]rangeio.Cell, missingPolicy string, nullPolicy xlsxmutate.RangeNullPolicy, mutOpts *MutationOptions, wantReadback bool) (*XLSXTablesAppendRecordsResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXTablesAppendRecordsResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheets := workbook.Sheets
		if xlsxTablesAppendRecordsSheet != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, xlsxTablesAppendRecordsSheet)
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
		tableRef, err := selectXLSXTable(tables, xlsxTablesAppendRecordsTable)
		if err != nil {
			return err
		}
		if tableRef.Range != xlsxTablesAppendRecordsExpectRange {
			return NewCLIErrorf(ExitInvalidArgs, "table range mismatch: expected %s but found %s", xlsxTablesAppendRecordsExpectRange, tableRef.Range)
		}
		columns := tableColumnNames(tableRef)
		rows, err := rangeio.RecordsToRows(records, columns, missingPolicy, xlsxTablesAppendRecordsIgnoreExtraFields)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		if xlsxTablesAppendRecordsMaxCells > 0 && len(rows)*len(columns) > xlsxTablesAppendRecordsMaxCells {
			return NewCLIErrorf(ExitInvalidArgs, "records contain %d cells, above --max-cells %d", len(rows)*len(columns), xlsxTablesAppendRecordsMaxCells)
		}
		mutRows, err := xlsxRangeCellsToMutate(rows)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		appendResult, err := xlsxmutate.AppendTableRows(&xlsxmutate.AppendTableRowsRequest{
			Package:           pkg,
			WorkbookURI:       workbook.PartURI,
			Table:             tableRef,
			Rows:              mutRows,
			NullPolicy:        nullPolicy,
			OverwriteFormulas: xlsxTablesAppendRecordsOverwriteFormulas,
		})
		if err != nil {
			return mapXLSXTablesAppendRecordsMutationError(err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXTableAppendDestination
		if wantReadback {
			destination, err = collectXLSXTableAppendDestination(pkg, workbook, tableRef, appendResult.PreviousRange, appendResult.AppendRange, destinationFile)
			if err != nil {
				return err
			}
		}
		result = &XLSXTablesAppendRecordsResult{
			File:               filePath,
			Table:              appendResult.Table,
			Sheet:              appendResult.Sheet,
			SheetNumber:        appendResult.SheetNumber,
			PreviousRange:      appendResult.PreviousRange,
			Range:              appendResult.Range,
			AppendRange:        appendResult.AppendRange,
			RowsAppended:       appendResult.RowsAppended,
			Updated:            appendResult.Updated,
			Created:            appendResult.Created,
			Cleared:            appendResult.Cleared,
			Skipped:            appendResult.Skipped,
			FormulaCount:       appendResult.FormulaCount,
			DataFormat:         "json",
			NullPolicy:         string(nullPolicy),
			MissingPolicy:      missingPolicy,
			IgnoredExtraFields: xlsxTablesAppendRecordsIgnoreExtraFields,
			Columns:            columns,
			Output:             destinationFile,
			DryRun:             mutOpts != nil && mutOpts.DryRun,
			Destination:        destination,
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

func tableColumnNames(tableRef model.TableRef) []string {
	columns := make([]string, len(tableRef.Columns))
	for i, column := range tableRef.Columns {
		columns[i] = column.Name
	}
	return columns
}

func mapXLSXTablesAppendRecordsMutationError(err error) error {
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
	return NewCLIErrorf(ExitInvalidArgs, "failed to append table records: %v", err)
}

func outputXLSXTablesAppendRecordsJSON(cmd *cobra.Command, result *XLSXTablesAppendRecordsResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal tables append-records JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXTablesAppendRecordsText(cmd *cobra.Command, result *XLSXTablesAppendRecordsResult) error {
	text := fmt.Sprintf("appended %d records to %s!%s: %s -> %s", result.RowsAppended, result.Sheet, result.Table, result.PreviousRange, result.Range)
	if result.FormulaCount > 0 {
		text += fmt.Sprintf(", formulas %d", result.FormulaCount)
	}
	if result.Skipped > 0 {
		text += fmt.Sprintf(", skipped %d", result.Skipped)
	}
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsTable, "table", "", "table number, name, or displayName")
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsExpectRange, "expect-range", "", "expected current table range from xlsx tables show")
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsRecords, "records", "", "inline JSON record array or object with records")
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsRecordsFile, "records-file", "", "path to JSON records, or - for stdin")
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsMissing, "missing", "reject", "missing field handling: reject, skip, or empty-string")
	xlsxTablesAppendRecordsCmd.Flags().StringVar(&xlsxTablesAppendRecordsNullPolicy, "null-policy", "skip", "null handling for explicit nulls and --missing skip: skip, clear, or empty-string")
	xlsxTablesAppendRecordsCmd.Flags().IntVar(&xlsxTablesAppendRecordsMaxCells, "max-cells", 100000, "maximum cells to append (0 for unlimited)")
	xlsxTablesAppendRecordsCmd.Flags().BoolVar(&xlsxTablesAppendRecordsIgnoreExtraFields, "ignore-extra-fields", false, "ignore record fields that do not match table columns")
	xlsxTablesAppendRecordsCmd.Flags().BoolVar(&xlsxTablesAppendRecordsOverwriteFormulas, "overwrite-formulas", false, "allow writing formulas over existing formula cells")
	AddMutationFlags(xlsxTablesAppendRecordsCmd)
	xlsxTablesCmd.AddCommand(xlsxTablesAppendRecordsCmd)
}
