package cli

import (
	"github.com/spf13/cobra"
)

// pptxCmd represents the pptx command group
var pptxCmd = &cobra.Command{
	Use:   "pptx",
	Short: "Work with PPTX presentations",
	Long:  "Commands for inspecting, modifying, and analyzing PPTX presentations.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// layoutsCmd represents the layouts command group
var layoutsCmd = &cobra.Command{
	Use:     "layouts",
	Aliases: []string{"layout"},
	Short:   "Inspect slide layouts",
	Long:    "Commands for inspecting slide layouts and their placeholders.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// mastersCmd represents the masters command group
var mastersCmd = &cobra.Command{
	Use:     "masters",
	Aliases: []string{"master"},
	Short:   "Inspect slide masters",
	Long:    "Commands for inspecting slide masters.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// slidesCmd represents the slides command group
var slidesCmd = &cobra.Command{
	Use:     "slides",
	Aliases: []string{"slide"},
	Short:   "Inspect slides",
	Long:    "Commands for inspecting slides.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var shapesCmd = &cobra.Command{
	Use:     "shapes",
	Aliases: []string{"shape"},
	Short:   "Inspect and mutate slide shapes",
	Long:    "Commands for inspecting, moving/resizing, and deleting PPTX slide shapes.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// pptxTextCmd represents the text command group for run/paragraph styling.
var pptxTextCmd = &cobra.Command{
	Use:   "text",
	Short: "Set slide text run/paragraph styling",
	Long:  "Commands for setting run/paragraph-level text properties on slide shapes.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// pptxNotesCmd represents the speaker-notes command group.
var pptxNotesCmd = &cobra.Command{
	Use:     "notes",
	Aliases: []string{"note"},
	Short:   "Set, clear, and show slide speaker notes",
	Long:    "Commands for editing and inspecting per-slide speaker notes (notesSlide parts).",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// translateCmd represents the translate command group
var translateCmd = &cobra.Command{
	Use:   "translate",
	Short: "Export and manage translations",
	Long:  "Commands for exporting presentations for translation and managing translation manifests.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

// init registers all pptx commands
func init() {
	// Add layouts subcommand group to pptx
	pptxCmd.AddCommand(layoutsCmd)

	// Add masters subcommand group to pptx
	pptxCmd.AddCommand(mastersCmd)

	// Add slides subcommand group to pptx
	pptxCmd.AddCommand(slidesCmd)

	// Add shapes subcommand group to pptx
	pptxCmd.AddCommand(shapesCmd)

	// Add text subcommand group to pptx
	pptxCmd.AddCommand(pptxTextCmd)

	// Add notes subcommand group to pptx
	pptxCmd.AddCommand(pptxNotesCmd)

	// Add charts subcommand group to pptx
	pptxCmd.AddCommand(chartsCmd)

	// Add translate subcommand group to pptx
	pptxCmd.AddCommand(translateCmd)

	// Add pptx command to root
	rootCmd.AddCommand(pptxCmd)
}
