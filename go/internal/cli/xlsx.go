package cli

import "github.com/spf13/cobra"

var xlsxCmd = &cobra.Command{
	Use:   "xlsx",
	Short: "Work with XLSX workbooks",
	Long:  "Commands for inspecting and mutating XLSX workbooks.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

var xlsxSheetsCmd = &cobra.Command{
	Use:     "sheets",
	Aliases: []string{"sheet"},
	Short:   "Inspect and mutate workbook sheets",
	Long:    "Commands for inspecting and mutating workbook sheets.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var xlsxTablesCmd = &cobra.Command{
	Use:     "tables",
	Aliases: []string{"table"},
	Short:   "Inspect and mutate workbook tables",
	Long:    "Commands for inspecting existing XLSX tables and appending rows to them.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var xlsxPivotsCmd = &cobra.Command{
	Use:     "pivots",
	Aliases: []string{"pivot"},
	Short:   "Inspect workbook PivotTables",
	Long:    "Commands for inspecting existing XLSX PivotTable definitions and cache metadata.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var xlsxChartsCmd = &cobra.Command{
	Use:     "charts",
	Aliases: []string{"chart"},
	Short:   "Inspect workbook charts",
	Long:    "Commands for inspecting existing XLSX worksheet chart definitions and source ranges.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

func init() {
	xlsxCmd.AddCommand(xlsxSheetsCmd)
	xlsxCmd.AddCommand(xlsxTablesCmd)
	xlsxCmd.AddCommand(xlsxPivotsCmd)
	xlsxCmd.AddCommand(xlsxChartsCmd)
	rootCmd.AddCommand(xlsxCmd)
}
