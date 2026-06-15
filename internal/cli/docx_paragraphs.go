package cli

import "github.com/spf13/cobra"

var docxParagraphsCmd = &cobra.Command{
	Use:     "paragraphs",
	Aliases: []string{"paragraph"},
	Short:   "Mutate DOCX body paragraphs",
	Long:    "Commands for mutating main document body paragraphs by block index.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

func init() {
	docxCmd.AddCommand(docxParagraphsCmd)
}
