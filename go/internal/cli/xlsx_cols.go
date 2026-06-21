package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxColsCmd = &cobra.Command{
	Use:     "cols",
	Aliases: []string{"col", "column", "columns"},
	Short:   "Insert and delete worksheet columns",
	Long:    "Commands for conservative structural column edits on plain XLSX worksheets.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var (
	xlsxColsInsertSheet string
	xlsxColsInsertAt    string
	xlsxColsInsertCount int
)

var xlsxColsInsertCmd = &cobra.Command{
	Use:   "insert <file>",
	Short: "Insert blank worksheet columns",
	Long:  "Insert blank columns before the --at column. Structural edits refuse worksheets with formulas, merged cells, tables, drawings, filters, hyperlinks, validation ranges, or existing column metadata.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXStructureSheet(xlsxColsInsertSheet); err != nil {
			return err
		}
		at, err := parseXLSXStructureColumnFlag(xlsxColsInsertAt, "at")
		if err != nil {
			return err
		}
		if err := normalizeXLSXStructureCount(xlsxColsInsertCount); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		result, err := performXLSXColsInsert(filePath, xlsxColsInsertSheet, at, xlsxColsInsertCount, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXStructureJSON(cmd, result, "cols insert")
		}
		return outputXLSXStructureText(cmd, result)
	},
}

var (
	xlsxColsDeleteSheet string
	xlsxColsDeleteCol   string
	xlsxColsDeleteCount int
)

var xlsxColsDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete worksheet columns",
	Long:  "Delete a band of worksheet columns. Structural edits refuse worksheets with formulas, merged cells, tables, drawings, filters, hyperlinks, validation ranges, or existing column metadata.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXStructureSheet(xlsxColsDeleteSheet); err != nil {
			return err
		}
		column, err := parseXLSXStructureColumnFlag(xlsxColsDeleteCol, "col")
		if err != nil {
			return err
		}
		if err := normalizeXLSXStructureCount(xlsxColsDeleteCount); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		result, err := performXLSXColsDelete(filePath, xlsxColsDeleteSheet, column, xlsxColsDeleteCount, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXStructureJSON(cmd, result, "cols delete")
		}
		return outputXLSXStructureText(cmd, result)
	},
}

func parseXLSXStructureColumnFlag(value, name string) (int, error) {
	column, err := address.ParseColumn(value)
	if err != nil {
		return 0, NewCLIErrorf(ExitInvalidArgs, "invalid --%s: %v", name, err)
	}
	return column, nil
}

func performXLSXColsInsert(filePath, sheet string, at, count int, mutOpts *MutationOptions) (*XLSXStructureMutationResult, error) {
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
		mutResult, err := xlsxmutate.InsertColumns(&xlsxmutate.InsertColumnsRequest{
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

func performXLSXColsDelete(filePath, sheet string, column, count int, mutOpts *MutationOptions) (*XLSXStructureMutationResult, error) {
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
		mutResult, err := xlsxmutate.DeleteColumns(&xlsxmutate.DeleteColumnsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    sheetRef,
			Column:      column,
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
	xlsxColsInsertCmd.Flags().StringVar(&xlsxColsInsertSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxColsInsertCmd.Flags().StringVar(&xlsxColsInsertAt, "at", "", "column position for the inserted columns")
	xlsxColsInsertCmd.Flags().IntVar(&xlsxColsInsertCount, "count", 1, "number of columns to insert")
	AddMutationFlags(xlsxColsInsertCmd)

	xlsxColsDeleteCmd.Flags().StringVar(&xlsxColsDeleteSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxColsDeleteCmd.Flags().StringVar(&xlsxColsDeleteCol, "col", "", "first column to delete")
	xlsxColsDeleteCmd.Flags().IntVar(&xlsxColsDeleteCount, "count", 1, "number of columns to delete")
	AddMutationFlags(xlsxColsDeleteCmd)

	xlsxColsCmd.AddCommand(xlsxColsInsertCmd, xlsxColsDeleteCmd)
	xlsxCmd.AddCommand(xlsxColsCmd)
}
