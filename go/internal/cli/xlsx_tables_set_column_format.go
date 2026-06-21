package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
	"github.com/spf13/cobra"
)

type XLSXTablesSetColumnFormatResult struct {
	File           string                `json:"file"`
	Table          string                `json:"table"`
	TableNumber    int                   `json:"tableNumber"`
	Sheet          string                `json:"sheet"`
	SheetNumber    int                   `json:"sheetNumber"`
	Column         string                `json:"column"`
	ColumnIndex    int                   `json:"columnIndex"`
	Range          string                `json:"range"`
	TableRange     string                `json:"tableRange"`
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
	XLSXTableAppendReadbackCommands
}

var (
	xlsxTablesSetColumnFormatSheet          string
	xlsxTablesSetColumnFormatTable          string
	xlsxTablesSetColumnFormatColumn         string
	xlsxTablesSetColumnFormatExpectColumn   string
	xlsxTablesSetColumnFormatPreset         string
	xlsxTablesSetColumnFormatCode           string
	xlsxTablesSetColumnFormatDecimals       int
	xlsxTablesSetColumnFormatCurrencySymbol string
	xlsxTablesSetColumnFormatMaxCells       int
)

var xlsxTablesSetColumnFormatCmd = &cobra.Command{
	Use:   "set-column-format <file>",
	Short: "Apply a number format to a table column by name",
	Long:  "Apply integer, number, currency, percent, date, datetime, text, general, or custom number formats to the data cells of a named table column.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		columnName := strings.TrimSpace(xlsxTablesSetColumnFormatColumn)
		if columnName == "" {
			return InvalidArgsError("--column is required")
		}
		if xlsxTablesSetColumnFormatMaxCells < 0 {
			return InvalidArgsError("--max-cells must be >= 0")
		}
		spec, err := xlsxmutate.ResolveNumberFormat(xlsxmutate.NumberFormatOptions{
			Preset:         xlsxTablesSetColumnFormatPreset,
			FormatCode:     xlsxTablesSetColumnFormatCode,
			Decimals:       xlsxTablesSetColumnFormatDecimals,
			CurrencySymbol: xlsxTablesSetColumnFormatCurrencySymbol,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXTablesSetColumnFormat(filePath, columnName, spec, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXTablesSetColumnFormatJSON(cmd, result)
		}
		return outputXLSXTablesSetColumnFormatText(cmd, result)
	},
}

func performXLSXTablesSetColumnFormat(filePath, columnName string, spec xlsxmutate.NumberFormatSpec, mutOpts *MutationOptions, wantReadback bool) (*XLSXTablesSetColumnFormatResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXTablesSetColumnFormatResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheets := workbook.Sheets
		if strings.TrimSpace(xlsxTablesSetColumnFormatSheet) != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, xlsxTablesSetColumnFormatSheet)
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
		tableRef, err := selectXLSXTable(tables, xlsxTablesSetColumnFormatTable)
		if err != nil {
			return err
		}

		columnIndex := -1
		for idx, col := range tableRef.Columns {
			if col.Name == columnName {
				columnIndex = idx
				break
			}
		}
		if columnIndex < 0 {
			return TargetNotFoundError(fmt.Sprintf("column %q not found in table %q", columnName, tableRef.DisplayName))
		}
		if expect := strings.TrimSpace(xlsxTablesSetColumnFormatExpectColumn); expect != "" && expect != tableRef.Columns[columnIndex].Name {
			return NewCLIErrorf(ExitInvalidArgs, "resolved column %q does not match --expect-column %q", tableRef.Columns[columnIndex].Name, expect)
		}

		rangeRef, err := xlsxmutate.ResolveTableColumnDataRange(tableRef, columnName)
		if err != nil {
			return mapXLSXTablesSetColumnFormatError(err)
		}
		if err := checkXLSXRangeMaxCells(rangeRef, xlsxTablesSetColumnFormatMaxCells); err != nil {
			return err
		}

		sheetRef := model.SheetRef{
			Name:             tableRef.Sheet,
			Number:           tableRef.SheetNumber,
			PartURI:          tableRef.SheetPartURI,
			RelationshipType: namespaces.RelWorksheet,
		}
		setResult, err := xlsxmutate.SetRangeNumberFormat(&xlsxmutate.SetRangeFormatRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			StylesURI:   workbook.StylesURI,
			SheetRef:    sheetRef,
			Range:       rangeRef,
			Format:      spec,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to set table column format: %v", err)
		}
		workbook.StylesURI = setResult.StylesURI

		destinationSheet := sheetRefForTableAppendDestination(workbook, tableRef)
		rows, cols := xlsxRangeDimensions(rangeRef)
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXRangeDestination
		if wantReadback {
			destination, err = collectXLSXRangeDestination(pkg, workbook, destinationSheet, rangeRef, destinationFile)
			if err != nil {
				return err
			}
		}

		result = &XLSXTablesSetColumnFormatResult{
			File:           filePath,
			Table:          tableRef.DisplayName,
			TableNumber:    tableRef.Number,
			Sheet:          tableRef.Sheet,
			SheetNumber:    tableRef.SheetNumber,
			Column:         tableRef.Columns[columnIndex].Name,
			ColumnIndex:    columnIndex,
			Range:          setResult.Range,
			TableRange:     tableRef.Range,
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
		result.XLSXTableAppendReadbackCommands = xlsxTablesSetColumnFormatTableCommands(tableRef, destinationSheet, destinationFile)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func xlsxTablesSetColumnFormatTableCommands(tableRef model.TableRef, sheetRef model.SheetRef, destinationFile string) XLSXTableAppendReadbackCommands {
	sheetSelector := xlsxSheetSelector(sheetRef.PrimarySelector, tableRef.Sheet, tableRef.SheetNumber)
	tableSelector := xlsxTableSelector(tableRef.PrimarySelector, tableRef.DisplayName, tableRef.Number)
	if destinationFile == "" {
		placeholder := xlsxOutputPlaceholder()
		return XLSXTableAppendReadbackCommands{
			TableShowCommandTemplate:   xlsxTableShowCommand(placeholder, sheetSelector, tableSelector),
			TableExportCommandTemplate: xlsxTableExportReadbackCommand(placeholder, sheetSelector, tableSelector),
		}
	}
	return XLSXTableAppendReadbackCommands{
		TableShowCommand:   xlsxTableShowCommand(destinationFile, sheetSelector, tableSelector),
		TableExportCommand: xlsxTableExportReadbackCommand(destinationFile, sheetSelector, tableSelector),
	}
}

func mapXLSXTablesSetColumnFormatError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	if errors.Is(err, xlsxmutate.ErrTableColumnNotFound) {
		return TargetNotFoundError(err.Error())
	}
	if errors.Is(err, xlsxmutate.ErrTableColumnHasNoDataRows) {
		return InvalidArgsError(err.Error())
	}
	return NewCLIErrorf(ExitInvalidArgs, "failed to resolve table column range: %v", err)
}

func outputXLSXTablesSetColumnFormatJSON(cmd *cobra.Command, result *XLSXTablesSetColumnFormatResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal tables set-column-format JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXTablesSetColumnFormatText(cmd *cobra.Command, result *XLSXTablesSetColumnFormatResult) error {
	text := fmt.Sprintf("formatted table %s column %s (%d data rows x %d col): %s, updated %d cells", result.Table, result.Column, result.Rows, result.Cols, result.FormatCode, result.Updated)
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatTable, "table", "", "table number, name, or displayName")
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatColumn, "column", "", "exact table column name to format")
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatExpectColumn, "expect-column", "", "guard: confirm the resolved column name before mutating")
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatPreset, "preset", "", "format preset: integer, number, currency, percent, date, datetime, text, or general")
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatCode, "format-code", "", "custom SpreadsheetML number format code")
	xlsxTablesSetColumnFormatCmd.Flags().IntVar(&xlsxTablesSetColumnFormatDecimals, "decimals", 2, "decimal places for number, currency, and percent presets")
	xlsxTablesSetColumnFormatCmd.Flags().StringVar(&xlsxTablesSetColumnFormatCurrencySymbol, "currency-symbol", "$", "currency literal for the currency preset")
	xlsxTablesSetColumnFormatCmd.Flags().IntVar(&xlsxTablesSetColumnFormatMaxCells, "max-cells", 100000, "maximum cells to format (0 for unlimited)")
	AddMutationFlags(xlsxTablesSetColumnFormatCmd)
	xlsxTablesCmd.AddCommand(xlsxTablesSetColumnFormatCmd)
}
