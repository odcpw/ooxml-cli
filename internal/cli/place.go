package cli

import (
	"github.com/spf13/cobra"
)

// placeCmd represents the place command group for mutations
var placeCmd = &cobra.Command{
	Use:   "place",
	Short: "Place content on presentations",
	Long:  "Commands for placing new content on PPTX presentations at specified coordinates",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// init registers the place command group with pptx
func init() {
	pptxCmd.AddCommand(placeCmd)
	placeCmd.AddCommand(placeTableCmd)
}
