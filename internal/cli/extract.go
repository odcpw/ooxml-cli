package cli

import (
	"github.com/spf13/cobra"
)

// extractCmd represents the extract command group
var extractCmd = &cobra.Command{
	Use:   "extract",
	Short: "Extract resources from presentations",
	Long:  "Commands for extracting resources (images, text, XML, notes, etc.) from PPTX presentations.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// init registers the extract command to pptx
func init() {
	pptxCmd.AddCommand(extractCmd)
}
