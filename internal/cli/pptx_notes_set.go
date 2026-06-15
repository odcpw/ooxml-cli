package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

var (
	pptxNotesSetSlide int
	pptxNotesSetText  string
)

// PPTXNotesSetResult is the JSON readback contract for set/clear notes.
type PPTXNotesSetResult struct {
	File   string `json:"file"`
	Output string `json:"output,omitempty"`
	DryRun bool   `json:"dryRun"`
	PPTXBridgeReadbackCommands
	mutate.SetNotesResult
}

var pptxNotesSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set speaker notes text for a slide",
	Long: `Set (replacing) the speaker notes text for a slide. Creates the notesSlide
part, its slide relationship, and content-type override when the slide has no
notes yet. Embedded newlines in --text become separate notes paragraphs.

Examples:
  ooxml pptx notes set deck.pptx --slide 1 --text "Remember to mention Q3 numbers" --out out.pptx
  ooxml pptx notes set deck.pptx --slide 2 --text $'First line\nSecond line' --in-place
  ooxml pptx notes set deck.pptx --slide 1 --text "draft" --dry-run`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxNotesSetSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if !cmd.Flags().Changed("text") {
			return InvalidArgsError("--text is required")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXNotesMutation(filePath, pptxNotesSetSlide, pptxNotesSetText, mutOpts)
		if err != nil {
			return err
		}
		return writePPTXNotesMutationResult(cmd, result, "set")
	},
}

// performPPTXNotesMutation applies a set/clear notes mutation and assembles the
// readback result. An empty text performs a clear.
func performPPTXNotesMutation(filePath string, slide int, text string, mutOpts *MutationOptions) (*PPTXNotesSetResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	var result *PPTXNotesSetResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		updated, err := mutate.SetNotesForSlide(&mutate.SetNotesRequest{
			Package:     pkg,
			SlideNumber: slide,
			Text:        text,
		})
		if err != nil {
			return mapPPTXNotesMutationError(err)
		}
		result = &PPTXNotesSetResult{
			File:           filePath,
			Output:         destinationFile,
			DryRun:         mutOpts.DryRun,
			SetNotesResult: *updated,
		}
		result.PPTXBridgeReadbackCommands = pptxNotesMutationReadbackCommands(destinationFile, updated.Slide)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to set notes")
	}
	return result, nil
}

func writePPTXNotesMutationResult(cmd *cobra.Command, result *PPTXNotesSetResult, verb string) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal notes %s JSON: %v", verb, err)
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeCLIOutput(cmd, data)
	}
	text := fmt.Sprintf("%s notes on slide %d (%s)", verb, result.Slide, result.NotesURI)
	if result.Output != "" {
		text += fmt.Sprintf("\nOutput: %s", result.Output)
	}
	return writeCLIOutput(cmd, []byte(text))
}

// mapPPTXNotesMutationError maps mutate-layer errors to CLI errors, treating
// slide-range errors as invalid-args.
func mapPPTXNotesMutationError(err error) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "presentation has"),
		strings.Contains(msg, "slide must be"),
		strings.Contains(msg, "no body placeholder"):
		return InvalidArgsError(msg)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func init() {
	pptxNotesSetCmd.Flags().IntVar(&pptxNotesSetSlide, "slide", 0, "1-based slide number")
	pptxNotesSetCmd.Flags().StringVar(&pptxNotesSetText, "text", "", "notes text (embedded \\n separates paragraphs)")
	pptxNotesSetCmd.MarkFlagRequired("slide")
	pptxNotesSetCmd.MarkFlagRequired("text")
	AddMutationFlags(pptxNotesSetCmd)
	pptxNotesCmd.AddCommand(pptxNotesSetCmd)
}
