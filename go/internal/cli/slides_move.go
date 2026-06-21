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

// SlidesMoveResult represents the JSON result of the slides move command
type SlidesMoveResult struct {
	File                    string                 `json:"file"`
	Output                  string                 `json:"output,omitempty"`
	DryRun                  bool                   `json:"dryRun"`
	SlideURI                string                 `json:"slideUri"`
	FromPosition            int                    `json:"fromPosition"`
	ToPosition              int                    `json:"toPosition"`
	IsNoOp                  bool                   `json:"isNoOp"`
	Destination             *cloneSlideDestination `json:"destination,omitempty"`
	ReadbackCommand         string                 `json:"readbackCommand,omitempty"`
	ReadbackCommandTemplate string                 `json:"readbackCommandTemplate,omitempty"`
	PPTXSlidesMutationReadbackCommands
}

var slidesMoveCmd = &cobra.Command{
	Use:   "move <file> <from-position> <to-position>",
	Short: "Move a slide within a presentation",
	Long: `Move a slide to a new position within the same presentation.

The slide's URI and content remain unchanged; only the presentation order changes.

Example:
  ooxml pptx slides move deck.pptx 1 3  # Move slide 1 to position 3

Exit codes:
  0: Success
  1: File not found
  2: Invalid arguments
  3: Position out of range
  4: Other errors`,
	Args: cobra.ExactArgs(3),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		fromStr := args[1]
		toStr := args[2]

		// Parse positions
		var fromPosition, toPosition int
		if _, err := fmt.Sscanf(fromStr, "%d", &fromPosition); err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid from-position: %s (expected an integer)", fromStr)
		}
		if _, err := fmt.Sscanf(toStr, "%d", &toPosition); err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid to-position: %s (expected an integer)", toStr)
		}

		// Check if file exists
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}

		// Get mutation options
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performMoveSlide(inputPath, fromPosition, toPosition, mutOpts)
		if err != nil {
			return err
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputSlidesMoveJSON(cmd, result)
		}

		return outputSlidesMoveText(cmd, result)
	},
}

func performMoveSlide(inputPath string, fromPosition, toPosition int, mutOpts *MutationOptions) (*SlidesMoveResult, error) {
	var result *SlidesMoveResult
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)

	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	if err := writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to validate positions
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}

		if fromPosition < 1 || fromPosition > len(graph.Slides) {
			return NewCLIErrorf(ExitInvalidArgs, "from-position %d out of range (presentation has %d slides)", fromPosition, len(graph.Slides))
		}

		if toPosition < 1 || toPosition > len(graph.Slides) {
			return NewCLIErrorf(ExitInvalidArgs, "to-position %d out of range (valid range: 1-%d)", toPosition, len(graph.Slides))
		}

		// Perform move
		moveResult, err := mutate.MoveSlide(&mutate.MoveSlideRequest{
			Package:     pkg,
			SlideNumber: fromPosition,
			NewPosition: toPosition,
		})
		if err != nil {
			return err
		}

		// Check if it's a no-op
		isNoOp := fromPosition == toPosition

		graph, err = inspect.ParsePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation after moving slide: %v", err)
		}
		destination, err := collectCloneSlideDestination(pkg, graph, destinationFile, moveResult.NewPosition)
		if err != nil {
			return err
		}

		result = &SlidesMoveResult{
			File:         inputPath,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
			SlideURI:     moveResult.SlideURI,
			FromPosition: moveResult.OldPosition,
			ToPosition:   moveResult.NewPosition,
			IsNoOp:       isNoOp,
			Destination:  destination,
		}
		result.PPTXSlidesMutationReadbackCommands = pptxSlidesMutationReadbackCommands(destinationFile)
		if destinationFile == "" {
			result.ReadbackCommandTemplate = pptxSlideReadbackCommand(outputPlaceholder(), moveResult.NewPosition)
		} else {
			result.ReadbackCommand = pptxSlideReadbackCommand(destinationFile, moveResult.NewPosition)
		}

		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to move slide")
	}

	return result, nil
}

// outputSlidesMoveJSON outputs the move result in JSON format
func outputSlidesMoveJSON(cmd *cobra.Command, result *SlidesMoveResult) error {
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

// outputSlidesMoveText outputs the move result in text format
func outputSlidesMoveText(cmd *cobra.Command, result *SlidesMoveResult) error {
	var text string

	if result.IsNoOp {
		text = "No-op: from and to positions are the same\n"
	} else {
		text = fmt.Sprintf("Moved slide %s\n", result.SlideURI)
		text += fmt.Sprintf("From position: %d\n", result.FromPosition)
		text += fmt.Sprintf("To position: %d\n", result.ToPosition)
	}

	return writeCLIOutput(cmd, []byte(text))
}

// init registers the slides move command
func init() {
	slidesCmd.AddCommand(slidesMoveCmd)
	AddMutationFlags(slidesMoveCmd)
}
