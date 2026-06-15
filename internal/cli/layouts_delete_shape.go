package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	layoutDeleteShapeLayout string
	layoutDeleteShapeTarget string
)

type deleteLayoutShapeOutput struct {
	File      string `json:"file"`
	Output    string `json:"output,omitempty"`
	DryRun    bool   `json:"dryRun"`
	Layout    string `json:"layout"`
	LayoutURI string `json:"layoutUri"`
	ShapeID   int    `json:"shapeId"`
	ShapeName string `json:"shapeName"`
	PPTXLayoutMutationReadbackCommands
}

var layoutsDeleteShapeCmd = &cobra.Command{
	Use:   "delete-shape <file>",
	Short: "Delete a shape or placeholder from a layout",
	Long: `Delete a shape or placeholder from a layout using a target selector.

Target examples:
  --target title
  --target shape:3
  --target pic:1
  --target '~Picture Placeholder 1'`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}
		if strings.TrimSpace(layoutDeleteShapeLayout) == "" {
			return InvalidArgsError("--layout must be specified")
		}
		if strings.TrimSpace(layoutDeleteShapeTarget) == "" {
			return InvalidArgsError("--target must be specified")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDeleteLayoutShape(inputPath, mutOpts)
		if err != nil {
			return err
		}

		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal delete result: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("Deleted shape %q (ID %d) from layout %q\n", result.ShapeName, result.ShapeID, result.Layout)))
	},
}

func performDeleteLayoutShape(inputPath string, mutOpts *MutationOptions) (*deleteLayoutShapeOutput, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *deleteLayoutShapeOutput
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}
		layoutURI, err := resolveLayoutSelector(graph, layoutDeleteShapeLayout)
		if err != nil {
			return err
		}
		deleted, err := mutate.DeleteLayoutShape(&mutate.DeleteLayoutShapeRequest{
			Package:       pkg,
			LayoutPartURI: layoutURI,
			Target:        layoutDeleteShapeTarget,
		})
		if err != nil {
			return err
		}
		layoutName := layoutNameByURI(graph, layoutURI)
		result = &deleteLayoutShapeOutput{
			File:      inputPath,
			Output:    destinationFile,
			DryRun:    mutOpts.DryRun,
			Layout:    layoutName,
			LayoutURI: layoutURI,
			ShapeID:   deleted.ShapeID,
			ShapeName: deleted.ShapeName,
		}
		result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, layoutName)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to delete layout shape")
	}
	return result, nil
}

func init() {
	layoutsDeleteShapeCmd.Flags().StringVar(&layoutDeleteShapeLayout, "layout", "", "layout number (1-based) or exact layout name")
	layoutsDeleteShapeCmd.Flags().StringVar(&layoutDeleteShapeTarget, "target", "", "shape selector within the layout")
	layoutsDeleteShapeCmd.MarkFlagRequired("layout")
	layoutsDeleteShapeCmd.MarkFlagRequired("target")
	AddMutationFlags(layoutsDeleteShapeCmd)
	layoutsCmd.AddCommand(layoutsDeleteShapeCmd)
}
