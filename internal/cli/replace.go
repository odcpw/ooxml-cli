package cli

import (
	"github.com/spf13/cobra"
)

// replaceCmd represents the replace command group for mutations
var replaceCmd = &cobra.Command{
	Use:   "replace",
	Short: "Replace content in presentations",
	Long:  "Commands for replacing and mutating content in PPTX presentations",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// init registers the replace command group with pptx
func init() {
	pptxCmd.AddCommand(replaceCmd)
}
