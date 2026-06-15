package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxRowsCmd = &cobra.Command{
	Use:     "rows",
	Aliases: []string{"row"},
	Short:   "Insert and delete worksheet rows",
	Long:    "Commands for conservative structural row edits on plain XLSX worksheets.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var (
	xlsxRowsInsertSheet string
	xlsxRowsInsertAt    int
	xlsxRowsInsertCount int
)

var xlsxRowsInsertCmd = &cobra.Command{
	Use:   "insert <file>",
	Short: "Insert blank worksheet rows",
	Long:  "Insert blank rows before the 1-based --at row. Structural edits refuse worksheets with formulas, merged cells, tables, drawings, filters, hyperlinks, or validation ranges.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXStructureSheet(xlsxRowsInsertSheet); err != nil {
			return err
		}
		if err := parsePositiveIntFlag(xlsxRowsInsertAt, "at"); err != nil {
			return err
		}
		if err := normalizeXLSXStructureCount(xlsxRowsInsertCount); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		result, err := performXLSXRowsInsert(filePath, xlsxRowsInsertSheet, xlsxRowsInsertAt, xlsxRowsInsertCount, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXStructureJSON(cmd, result, "rows insert")
		}
		return outputXLSXStructureText(cmd, result)
	},
}

var (
	xlsxRowsDeleteSheet string
	xlsxRowsDeleteRow   int
	xlsxRowsDeleteCount int
)

var xlsxRowsDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete worksheet rows",
	Long:  "Delete a band of worksheet rows. Structural edits refuse worksheets with formulas, merged cells, tables, drawings, filters, hyperlinks, or validation ranges.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXStructureSheet(xlsxRowsDeleteSheet); err != nil {
			return err
		}
		if err := parsePositiveIntFlag(xlsxRowsDeleteRow, "row"); err != nil {
			return err
		}
		if err := normalizeXLSXStructureCount(xlsxRowsDeleteCount); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		result, err := performXLSXRowsDelete(filePath, xlsxRowsDeleteSheet, xlsxRowsDeleteRow, xlsxRowsDeleteCount, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXStructureJSON(cmd, result, "rows delete")
		}
		return outputXLSXStructureText(cmd, result)
	},
}

func performXLSXRowsInsert(filePath, sheet string, at, count int, mutOpts *MutationOptions) (*XLSXStructureMutationResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}
	var result *XLSXStructureMutationResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheet)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		mutResult, err := xlsxmutate.InsertRows(&xlsxmutate.InsertRowsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    sheetRef,
			At:          at,
			Count:       count,
		})
		if err != nil {
			return mapXLSXStructureMutationError(err)
		}
		result = mapXLSXStructureResult(filePath, mutResult)
		result.Output = destinationFile
		result.DryRun = mutOpts.DryRun
		result.XLSXStructureReadbackCommands = xlsxStructureMutationReadbackCommands(destinationFile, xlsxStructureResultSelector(result))
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func performXLSXRowsDelete(filePath, sheet string, row, count int, mutOpts *MutationOptions) (*XLSXStructureMutationResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}
	var result *XLSXStructureMutationResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheet)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		mutResult, err := xlsxmutate.DeleteRows(&xlsxmutate.DeleteRowsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    sheetRef,
			Row:         row,
			Count:       count,
		})
		if err != nil {
			return mapXLSXStructureMutationError(err)
		}
		result = mapXLSXStructureResult(filePath, mutResult)
		result.Output = destinationFile
		result.DryRun = mutOpts.DryRun
		result.XLSXStructureReadbackCommands = xlsxStructureMutationReadbackCommands(destinationFile, xlsxStructureResultSelector(result))
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	xlsxRowsInsertCmd.Flags().StringVar(&xlsxRowsInsertSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRowsInsertCmd.Flags().IntVar(&xlsxRowsInsertAt, "at", 0, "1-based row position for the inserted rows")
	xlsxRowsInsertCmd.Flags().IntVar(&xlsxRowsInsertCount, "count", 1, "number of rows to insert")
	AddMutationFlags(xlsxRowsInsertCmd)

	xlsxRowsDeleteCmd.Flags().StringVar(&xlsxRowsDeleteSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRowsDeleteCmd.Flags().IntVar(&xlsxRowsDeleteRow, "row", 0, "1-based first row to delete")
	xlsxRowsDeleteCmd.Flags().IntVar(&xlsxRowsDeleteCount, "count", 1, "number of rows to delete")
	AddMutationFlags(xlsxRowsDeleteCmd)

	xlsxRowsCmd.AddCommand(xlsxRowsInsertCmd, xlsxRowsDeleteCmd)
	xlsxCmd.AddCommand(xlsxRowsCmd)
}
