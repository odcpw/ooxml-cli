package cli

import (
	"os"

	"github.com/spf13/cobra"
)

var pptxNotesClearSlide int

var pptxNotesClearCmd = &cobra.Command{
	Use:   "clear <file>",
	Short: "Clear speaker notes for a slide",
	Long: `Clear the speaker notes text for a slide. The notesSlide part and its
relationship are preserved; only the notes body text is emptied. If the slide
has no notes yet, an empty notesSlide part is created.

Examples:
  ooxml pptx notes clear deck.pptx --slide 1 --out out.pptx
  ooxml pptx notes clear deck.pptx --slide 2 --in-place
  ooxml pptx notes clear deck.pptx --slide 1 --dry-run`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxNotesClearSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		// Clearing is a set with empty text.
		result, err := performPPTXNotesMutation(filePath, pptxNotesClearSlide, "", mutOpts)
		if err != nil {
			return err
		}
		return writePPTXNotesMutationResult(cmd, result, "clear")
	},
}

func init() {
	pptxNotesClearCmd.Flags().IntVar(&pptxNotesClearSlide, "slide", 0, "1-based slide number")
	pptxNotesClearCmd.MarkFlagRequired("slide")
	AddMutationFlags(pptxNotesClearCmd)
	pptxNotesCmd.AddCommand(pptxNotesClearCmd)
}
