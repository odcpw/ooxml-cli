package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

var (
	pptxShapesSetBoundsSlide  int
	pptxShapesSetBoundsTarget string
	pptxShapesSetBoundsValue  string
)

type PPTXShapesSetBoundsResult struct {
	File        string                `json:"file"`
	Output      string                `json:"output,omitempty"`
	DryRun      bool                  `json:"dryRun"`
	Destination *PPTXShapeDestination `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
	mutate.SetSlideShapeBoundsResult
}

var pptxShapesSetBoundsCmd = &cobra.Command{
	Use:   "set-bounds <file>",
	Short: "Move or resize a slide shape",
	Long:  "Set explicit slide-level shape bounds in EMUs. Placeholder bounds become slide-level overrides.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxShapesSetBoundsSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if pptxShapesSetBoundsTarget == "" {
			return InvalidArgsError("--target is required")
		}
		if pptxShapesSetBoundsValue == "" {
			return InvalidArgsError("--bounds must be specified in format x,y,cx,cy")
		}
		x, y, cx, cy, err := parseBounds(pptxShapesSetBoundsValue)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --bounds: %v", err)
		}
		if cx <= 0 || cy <= 0 {
			return InvalidArgsError("--bounds width and height must be positive")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXShapesSetBounds(filePath, x, y, cx, cy, mutOpts)
		if err != nil {
			return err
		}
		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal shapes set-bounds JSON: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		text := fmt.Sprintf("updated slide %d shape %d bounds to %d,%d,%d,%d", result.Slide, result.ShapeID, result.NewX, result.NewY, result.NewCX, result.NewCY)
		if result.Output != "" {
			text += fmt.Sprintf("\nOutput: %s", result.Output)
		}
		if result.Destination != nil {
			text += fmt.Sprintf("\nSelector: %s", result.Destination.PrimarySelector)
		}
		return writeCLIOutput(cmd, []byte(text))
	},
}

func performPPTXShapesSetBounds(filePath string, x, y, cx, cy int64, mutOpts *MutationOptions) (*PPTXShapesSetBoundsResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	var result *PPTXShapesSetBoundsResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		updated, err := mutate.SetSlideShapeBounds(&mutate.SetSlideShapeBoundsRequest{
			Package:     pkg,
			SlideNumber: pptxShapesSetBoundsSlide,
			Target:      pptxShapesSetBoundsTarget,
			X:           x,
			Y:           y,
			CX:          cx,
			CY:          cy,
		})
		if err != nil {
			return mapPPTXShapesMutationError(err)
		}
		destination, err := collectPPTXShapeDestination(pkg, updated.Slide, updated.Target, destinationFile, true, true)
		if err != nil {
			return err
		}
		result = &PPTXShapesSetBoundsResult{
			File:                      filePath,
			Output:                    destinationFile,
			DryRun:                    mutOpts.DryRun,
			Destination:               destination,
			SetSlideShapeBoundsResult: *updated,
		}
		result.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, false, true)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to set slide shape bounds")
	}
	return result, nil
}

func init() {
	pptxShapesSetBoundsCmd.Flags().IntVar(&pptxShapesSetBoundsSlide, "slide", 0, "1-based slide number")
	pptxShapesSetBoundsCmd.Flags().StringVar(&pptxShapesSetBoundsTarget, "target", "", "shape selector such as title, body, shape:3, or ~Shape Name")
	pptxShapesSetBoundsCmd.Flags().StringVar(&pptxShapesSetBoundsValue, "bounds", "", "bounds in EMU units: x,y,cx,cy")
	pptxShapesSetBoundsCmd.MarkFlagRequired("slide")
	pptxShapesSetBoundsCmd.MarkFlagRequired("target")
	pptxShapesSetBoundsCmd.MarkFlagRequired("bounds")
	AddMutationFlags(pptxShapesSetBoundsCmd)
	shapesCmd.AddCommand(pptxShapesSetBoundsCmd)
}
