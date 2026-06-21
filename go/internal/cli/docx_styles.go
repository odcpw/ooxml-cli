package cli

import "github.com/spf13/cobra"

var docxStylesCmd = &cobra.Command{
	Use:     "styles",
	Aliases: []string{"style"},
	Short:   "Inspect DOCX style definitions from word/styles.xml",
	Long:    "Read-only commands for enumerating and inspecting Word style definitions.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

func init() {
	docxCmd.AddCommand(docxStylesCmd)
}
