package cli

import "github.com/spf13/cobra"

var docxCmd = &cobra.Command{
	Use:   "docx",
	Short: "Work with DOCX documents",
	Long:  "Commands for inspecting and mutating Word DOCX documents.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

func init() {
	rootCmd.AddCommand(docxCmd)
}
