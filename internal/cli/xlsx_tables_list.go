package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
	"github.com/spf13/cobra"
)

var xlsxTablesListSheet string

var xlsxTablesListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List workbook tables",
	Long:  "List existing XLSX tables discovered from worksheet tableParts relationships.",
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
		if xlsxTablesListSheet != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, xlsxTablesListSheet)
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
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXTablesJSON(cmd, filePath, tables)
		}
		return outputXLSXTablesText(cmd, tables)
	},
}

func init() {
	xlsxTablesListCmd.Flags().StringVar(&xlsxTablesListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxTablesCmd.AddCommand(xlsxTablesListCmd)
}
