package cli

import "github.com/spf13/cobra"

// templateCmd represents the template command group
var templateCmd = &cobra.Command{
	Use:   "template",
	Short: "Work with template manifests and compilation",
	Long:  "Commands for capturing, inspecting, and compiling presentations from templates.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// init registers the template command to pptx
func init() {
	// Add template command to pptx
	pptxCmd.AddCommand(templateCmd)
}
