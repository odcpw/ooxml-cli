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
	layoutSetBoundsLayout string
	layoutSetBoundsTarget string
	layoutSetBoundsValue  string
)

type setBoundsLayoutOutput struct {
	File      string `json:"file"`
	Output    string `json:"output,omitempty"`
	DryRun    bool   `json:"dryRun"`
	Layout    string `json:"layout"`
	LayoutURI string `json:"layoutUri"`
	ShapeID   int    `json:"shapeId"`
	ShapeName string `json:"shapeName"`
	OldX      int64  `json:"oldX"`
	OldY      int64  `json:"oldY"`
	OldCX     int64  `json:"oldCx"`
	OldCY     int64  `json:"oldCy"`
	NewX      int64  `json:"newX"`
	NewY      int64  `json:"newY"`
	NewCX     int64  `json:"newCx"`
	NewCY     int64  `json:"newCy"`
	PPTXLayoutMutationReadbackCommands
}

var layoutsSetBoundsCmd = &cobra.Command{
	Use:   "set-bounds <file>",
	Short: "Move/resize a layout shape by setting explicit bounds",
	Long: `Set explicit bounds for a layout shape or placeholder.

Bounds format: x,y,cx,cy (EMU units)
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
		if strings.TrimSpace(layoutSetBoundsLayout) == "" {
			return InvalidArgsError("--layout must be specified")
		}
		if strings.TrimSpace(layoutSetBoundsTarget) == "" {
			return InvalidArgsError("--target must be specified")
		}
		if strings.TrimSpace(layoutSetBoundsValue) == "" {
			return InvalidArgsError("--bounds must be specified in format x,y,cx,cy")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performSetLayoutBounds(inputPath, mutOpts)
		if err != nil {
			return err
		}

		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal bounds result: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("Updated bounds for shape %q (ID %d) on layout %q\n", result.ShapeName, result.ShapeID, result.Layout)))
	},
}

func performSetLayoutBounds(inputPath string, mutOpts *MutationOptions) (*setBoundsLayoutOutput, error) {
	writer, err := NewMutationWriter(inputPath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *setBoundsLayoutOutput
	destinationFile := mutationOutputPathForResult(inputPath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}
		layoutURI, err := resolveLayoutSelector(graph, layoutSetBoundsLayout)
		if err != nil {
			return err
		}
		x, y, cx, cy, err := parseBounds(layoutSetBoundsValue)
		if err != nil {
			return fmt.Errorf("invalid bounds: %w", err)
		}
		updated, err := mutate.SetLayoutShapeBounds(&mutate.SetLayoutShapeBoundsRequest{
			Package:       pkg,
			LayoutPartURI: layoutURI,
			Target:        layoutSetBoundsTarget,
			X:             x,
			Y:             y,
			CX:            cx,
			CY:            cy,
		})
		if err != nil {
			return err
		}
		layoutName := layoutNameByURI(graph, layoutURI)
		result = &setBoundsLayoutOutput{
			File:      inputPath,
			Output:    destinationFile,
			DryRun:    mutOpts.DryRun,
			Layout:    layoutName,
			LayoutURI: layoutURI,
			ShapeID:   updated.ShapeID,
			ShapeName: updated.ShapeName,
			OldX:      updated.OldX,
			OldY:      updated.OldY,
			OldCX:     updated.OldCX,
			OldCY:     updated.OldCY,
			NewX:      updated.NewX,
			NewY:      updated.NewY,
			NewCX:     updated.NewCX,
			NewCY:     updated.NewCY,
		}
		result.PPTXLayoutMutationReadbackCommands = pptxLayoutMutationReadbackCommands(destinationFile, layoutName)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to set layout shape bounds")
	}
	return result, nil
}

func init() {
	layoutsSetBoundsCmd.Flags().StringVar(&layoutSetBoundsLayout, "layout", "", "layout number (1-based) or exact layout name")
	layoutsSetBoundsCmd.Flags().StringVar(&layoutSetBoundsTarget, "target", "", "shape selector within the layout")
	layoutsSetBoundsCmd.Flags().StringVar(&layoutSetBoundsValue, "bounds", "", "bounds in EMU units: x,y,cx,cy")
	layoutsSetBoundsCmd.MarkFlagRequired("layout")
	layoutsSetBoundsCmd.MarkFlagRequired("target")
	layoutsSetBoundsCmd.MarkFlagRequired("bounds")
	AddMutationFlags(layoutsSetBoundsCmd)
	layoutsCmd.AddCommand(layoutsSetBoundsCmd)
}
