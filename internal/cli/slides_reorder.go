package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

// SlidesReorderResult represents the JSON result of the slides reorder command
type SlidesReorderResult struct {
	File       string `json:"file"`
	Output     string `json:"output,omitempty"`
	DryRun     bool   `json:"dryRun"`
	NewOrder   []int  `json:"newOrder"`
	SlideCount int    `json:"slideCount"`
	PPTXSlidesMutationReadbackCommands
}

var slidesReorderCmd = &cobra.Command{
	Use:   "reorder <file> <order>",
	Short: "Reorder slides in a presentation using a full permutation",
	Long: `Reorder all slides in a PPTX presentation using a complete explicit permutation.

The order argument must be a comma-separated list of all slide numbers in the desired order.
For example, to move slide 3 to the front, then slides 1, 2, 4: "3,1,2,4"

Requirements:
  - All slides must appear exactly once (no duplicates or omissions)
  - Slide numbers must be in the range 1..N (where N is the total slide count)

Example:
  ooxml pptx slides reorder deck.pptx "2,1,3"  # Swap slides 1 and 2
  ooxml pptx slides reorder deck.pptx "4,3,2,1" # Reverse order

Exit codes:
  0: Success
  1: File not found
  2: Invalid arguments or invalid permutation
  4: Other errors`,
	Args: cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		orderStr := args[1]

		// Check if file exists
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}

		// Get mutation options
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performReorderSlides(inputPath, orderStr, mutOpts)
		if err != nil {
			return err
		}

		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			return outputSlidesReorderJSON(cmd, result)
		}

		return outputSlidesReorderText(cmd, result)
	},
}

func performReorderSlides(inputPath string, orderStr string, mutOpts *MutationOptions) (*SlidesReorderResult, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *SlidesReorderResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)

	if err := writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to validate
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}

		numSlides := len(graph.Slides)

		// Validate the order string before making changes
		orderParts := strings.Split(strings.TrimSpace(orderStr), ",")
		if len(orderParts) != numSlides {
			return NewCLIErrorf(ExitInvalidArgs, "permutation has %d elements but presentation has %d slides", len(orderParts), numSlides)
		}

		// Perform reorder
		reorderResult, err := mutate.ReorderSlides(&mutate.ReorderSlideRequest{
			Package: pkg,
			Order:   orderStr,
		})
		if err != nil {
			return fmt.Errorf("failed to reorder slides: %w", err)
		}

		graph, err = inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation after reordering slides: %v", err)
		}
		result = &SlidesReorderResult{
			File:       inputPath,
			Output:     destinationFile,
			DryRun:     mutOpts.DryRun,
			NewOrder:   reorderResult.NewOrder,
			SlideCount: len(graph.Slides),
		}
		result.PPTXSlidesMutationReadbackCommands = pptxSlidesMutationReadbackCommands(destinationFile)
		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, cliErr
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to reorder slides: %v", err)
	}

	return result, nil
}

// outputSlidesReorderJSON outputs the reorder result in JSON format
func outputSlidesReorderJSON(cmd *cobra.Command, result *SlidesReorderResult) error {
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

// outputSlidesReorderText outputs the reorder result in text format
func outputSlidesReorderText(cmd *cobra.Command, result *SlidesReorderResult) error {
	text := fmt.Sprintf("File: %s\n", result.File)
	if result.Output != "" {
		text += fmt.Sprintf("Output: %s\n", result.Output)
	}
	text += fmt.Sprintf("New slide order: %v\n", result.NewOrder)

	return writeCLIOutput(cmd, []byte(text))
}

// init registers the slides reorder command
func init() {
	AddMutationFlags(slidesReorderCmd)
	slidesCmd.AddCommand(slidesReorderCmd)
}
