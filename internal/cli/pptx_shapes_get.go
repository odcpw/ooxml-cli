package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

var (
	pptxShapesGetSlide         int
	pptxShapesGetTarget        string
	pptxShapesGetIncludeText   bool
	pptxShapesGetIncludeBounds bool
)

var pptxShapesGetCmd = &cobra.Command{
	Use:   "get <file>",
	Short: "Get one targetable slide shape",
	Long:  "Resolve one slide shape selector and return shape metadata plus optional bounds/text.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxShapesGetSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if pptxShapesGetTarget == "" {
			return InvalidArgsError("--target is required")
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		catalog, err := pptselectors.BuildSlideCatalog(pkg, pptxShapesGetSlide)
		if err != nil {
			return mapPPTXShapeCatalogError(err)
		}
		target, _, err := catalog.ResolveTargetElement(pptxShapesGetTarget)
		if err != nil {
			return mapPPTXShapeCatalogError(err)
		}
		entries, err := collectPPTXShapeEntries(pkg, catalog, pptxShapesGetIncludeText, pptxShapesGetIncludeBounds)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to collect slide shapes: %v", err)
		}
		var selected []PPTXShapeEntry
		for _, entry := range entries {
			if entry.ShapeID == target.ShapeID {
				selected = []PPTXShapeEntry{entry}
				break
			}
		}
		if len(selected) == 0 {
			return TargetNotFoundError(pptxShapesGetTarget)
		}

		result := &PPTXShapesResult{
			File:          filePath,
			Slide:         catalog.SlideNumber,
			PartURI:       catalog.SlidePartURI,
			LayoutName:    catalog.LayoutName,
			LayoutPartURI: catalog.LayoutPartURI,
			Shapes:        selected,
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXShapesJSON(cmd, result)
		}
		return outputPPTXShapesText(cmd, result)
	},
}

func init() {
	pptxShapesGetCmd.Flags().IntVar(&pptxShapesGetSlide, "slide", 0, "1-based slide number")
	pptxShapesGetCmd.Flags().StringVar(&pptxShapesGetTarget, "target", "", "shape selector such as title, body, shape:3, or ~Shape Name")
	pptxShapesGetCmd.Flags().BoolVar(&pptxShapesGetIncludeText, "include-text", false, "include text preview/content where available")
	pptxShapesGetCmd.Flags().BoolVar(&pptxShapesGetIncludeBounds, "include-bounds", false, "include explicit slide shape bounds and geometry")
	pptxShapesGetCmd.MarkFlagRequired("slide")
	pptxShapesGetCmd.MarkFlagRequired("target")
	shapesCmd.AddCommand(pptxShapesGetCmd)
}
