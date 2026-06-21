package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

// SlidesDeleteResult represents the JSON result of the slides delete command
type SlidesDeleteResult struct {
	File            string `json:"file"`
	Output          string `json:"output,omitempty"`
	DryRun          bool   `json:"dryRun"`
	DeletedSlide    int    `json:"deletedSlide"`
	RemovedURI      string `json:"removedUri"`
	RemovedNotes    string `json:"removedNotes,omitempty"`
	RemainingSlides int    `json:"remainingSlides"`
	PPTXSlidesMutationReadbackCommands
}

var slidesDeleteCmd = &cobra.Command{
	Use:   "delete <file> <slide-number>",
	Short: "Delete a slide from a presentation",
	Long: `Delete a slide by number from a PPTX presentation.

This command removes:
  - The slide itself
  - Associated relationships
  - Notes slide (if present)
  - Content type overrides

The remaining slides are renumbered automatically.

Exit codes:
  0: Success
  1: File not found
  2: Invalid arguments
  3: Slide number out of range
  4: Other errors`,
	Args: cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		slideNumberStr := args[1]

		// Parse slide number
		var slideNumber int
		if _, err := fmt.Sscanf(slideNumberStr, "%d", &slideNumber); err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid slide number: %s (expected an integer)", slideNumberStr)
		}

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Get mutation options
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDeleteSlide(filePath, slideNumber, mutOpts)
		if err != nil {
			return err
		}

		// Format output
		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			return outputSlidesDeleteJSON(cmd, result)
		}

		return outputSlidesDeleteText(cmd, result)
	},
}

func performDeleteSlide(inputPath string, slideNumber int, mutOpts *MutationOptions) (*SlidesDeleteResult, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *SlidesDeleteResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)

	if err := writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to validate slide number
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}

		if slideNumber < 1 || slideNumber > len(graph.Slides) {
			return NewCLIErrorf(ExitInvalidArgs, "slide number %d out of range (presentation has %d slides)", slideNumber, len(graph.Slides))
		}

		// Get original slide info for reporting
		deletedNotes := graph.Slides[slideNumber-1].NotesPartURI

		// Perform deletion
		deleteResult, err := mutate.DeleteSlide(&mutate.DeleteSlideRequest{
			Package:     pkg,
			SlideNumber: slideNumber,
		})
		if err != nil {
			return fmt.Errorf("failed to delete slide: %w", err)
		}

		graph, err = inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation after deleting slide: %v", err)
		}
		result = &SlidesDeleteResult{
			File:            inputPath,
			Output:          destinationFile,
			DryRun:          mutOpts.DryRun,
			DeletedSlide:    slideNumber,
			RemovedURI:      deleteResult.DeletedSlideURI,
			RemovedNotes:    deletedNotes,
			RemainingSlides: len(graph.Slides),
		}
		result.PPTXSlidesMutationReadbackCommands = pptxSlidesMutationReadbackCommands(destinationFile)
		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, cliErr
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to delete slide: %v", err)
	}

	return result, nil
}

// outputSlidesDeleteJSON outputs the delete result in JSON format
func outputSlidesDeleteJSON(cmd *cobra.Command, result *SlidesDeleteResult) error {
	config := GetGlobalConfig(cmd)

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(result, "", "  ")
	} else {
		jsonData, err = json.Marshal(result)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	return writeCLIOutput(cmd, jsonData)
}

// outputSlidesDeleteText outputs the delete result in text format
func outputSlidesDeleteText(cmd *cobra.Command, result *SlidesDeleteResult) error {
	text := fmt.Sprintf("File: %s\n", result.File)
	if result.Output != "" {
		text += fmt.Sprintf("Output: %s\n", result.Output)
	}
	text += fmt.Sprintf("Deleted slide: %d\n", result.DeletedSlide)
	text += fmt.Sprintf("Removed URI: %s\n", result.RemovedURI)

	if result.RemovedNotes != "" {
		text += fmt.Sprintf("Removed notes: %s\n", result.RemovedNotes)
	} else {
		text += "Removed notes: (none)\n"
	}

	text += fmt.Sprintf("Remaining slides: %d\n", result.RemainingSlides)

	return writeCLIOutput(cmd, []byte(text))
}

// init registers the slides delete command
func init() {
	AddMutationFlags(slidesDeleteCmd)
	slidesCmd.AddCommand(slidesDeleteCmd)
}
