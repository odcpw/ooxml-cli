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
	pptxShapesDeleteSlide  int
	pptxShapesDeleteTarget string
)

type PPTXShapesDeleteResult struct {
	File    string                `json:"file"`
	Output  string                `json:"output,omitempty"`
	DryRun  bool                  `json:"dryRun"`
	Deleted *PPTXShapeDestination `json:"deleted,omitempty"`
	mutate.DeleteSlideShapeResult
}

var pptxShapesDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete a slide shape",
	Long:  "Delete one top-level slide shape. This does not mutate the slide layout or master.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxShapesDeleteSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if pptxShapesDeleteTarget == "" {
			return InvalidArgsError("--target is required")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXShapesDelete(filePath, mutOpts)
		if err != nil {
			return err
		}
		data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal shapes delete JSON: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeCLIOutput(cmd, data)
		}
		text := fmt.Sprintf("deleted slide %d shape %d (%s)", result.Slide, result.ShapeID, result.ShapeName)
		if result.Output != "" {
			text += fmt.Sprintf("\nOutput: %s", result.Output)
		}
		if result.Deleted != nil {
			text += fmt.Sprintf("\nDeleted selector: %s", result.Deleted.PrimarySelector)
		}
		return writeCLIOutput(cmd, []byte(text))
	},
}

func performPPTXShapesDelete(filePath string, mutOpts *MutationOptions) (*PPTXShapesDeleteResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	var result *PPTXShapesDeleteResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		deletedTarget, err := collectPPTXShapeDestination(pkg, pptxShapesDeleteSlide, pptxShapesDeleteTarget, filePath, true, true)
		if err != nil {
			return err
		}
		deleted, err := mutate.DeleteSlideShape(&mutate.DeleteSlideShapeRequest{
			Package:     pkg,
			SlideNumber: pptxShapesDeleteSlide,
			Target:      pptxShapesDeleteTarget,
		})
		if err != nil {
			return mapPPTXShapesMutationError(err)
		}
		result = &PPTXShapesDeleteResult{
			File:                   filePath,
			Output:                 destinationFile,
			DryRun:                 mutOpts.DryRun,
			Deleted:                deletedTarget,
			DeleteSlideShapeResult: *deleted,
		}
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to delete slide shape")
	}
	return result, nil
}

func mapPPTXShapesMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "not found (presentation has") || strings.Contains(msg, "slide must be"):
		return InvalidArgsError(msg)
	case strings.Contains(msg, "not found"), strings.Contains(msg, "ambiguous target"):
		return TargetNotFoundError(msg)
	case strings.Contains(msg, "not supported"), strings.Contains(msg, "must be positive"), strings.Contains(msg, "must be >="):
		return InvalidArgsError(msg)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func init() {
	pptxShapesDeleteCmd.Flags().IntVar(&pptxShapesDeleteSlide, "slide", 0, "1-based slide number")
	pptxShapesDeleteCmd.Flags().StringVar(&pptxShapesDeleteTarget, "target", "", "shape selector such as title, body, shape:3, or ~Shape Name")
	pptxShapesDeleteCmd.MarkFlagRequired("slide")
	pptxShapesDeleteCmd.MarkFlagRequired("target")
	AddMutationFlags(pptxShapesDeleteCmd)
	shapesCmd.AddCommand(pptxShapesDeleteCmd)
}
