package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
	"github.com/spf13/cobra"
)

var (
	xlsxTablesShowSheet string
	xlsxTablesShowTable string
)

var xlsxTablesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show table metadata",
	Long:  "Show one existing XLSX table, including range, columns, style, and part metadata.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheets := workbook.Sheets
		if xlsxTablesShowSheet != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, xlsxTablesShowSheet)
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
		selected, err := selectXLSXTable(tables, xlsxTablesShowTable)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXTablesJSON(cmd, filePath, []model.TableRef{selected})
		}
		return outputXLSXTablesText(cmd, []model.TableRef{selected})
	},
}

func init() {
	xlsxTablesShowCmd.Flags().StringVar(&xlsxTablesShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxTablesShowCmd.Flags().StringVar(&xlsxTablesShowTable, "table", "", "table number, name, or displayName")
	xlsxTablesCmd.AddCommand(xlsxTablesShowCmd)
}
