package cli

import "github.com/spf13/cobra"

var xlsxWorkbookCmd = &cobra.Command{
	Use:   "workbook",
	Short: "Workbook-level operations",
	Long:  "Commands for inspecting and mutating workbook-level properties such as document metadata and calculation settings.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

var xlsxWorkbookMetadataCmd = &cobra.Command{
	Use:     "metadata",
	Aliases: []string{"meta", "properties", "props"},
	Short:   "Inspect and update workbook metadata and calc settings",
	Long:    "Commands for reading and updating core/app document properties and workbook calculation settings.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

func init() {
	xlsxWorkbookCmd.AddCommand(xlsxWorkbookMetadataCmd)
	xlsxCmd.AddCommand(xlsxWorkbookCmd)
}
