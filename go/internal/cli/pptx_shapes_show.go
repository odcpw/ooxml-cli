package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

var (
	pptxShapesShowSlide         int
	pptxShapesShowIncludeText   bool
	pptxShapesShowIncludeBounds bool
)

var pptxShapesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show targetable shapes on a slide",
	Long:  "Show the published selector surface and optional bounds/text for targetable shapes on one slide.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxShapesShowSlide < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		catalog, err := pptselectors.BuildSlideCatalog(pkg, pptxShapesShowSlide)
		if err != nil {
			return mapPPTXShapeCatalogError(err)
		}
		entries, err := collectPPTXShapeEntries(pkg, catalog, pptxShapesShowIncludeText, pptxShapesShowIncludeBounds)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to collect slide shapes: %v", err)
		}
		result := &PPTXShapesResult{
			File:          filePath,
			Slide:         catalog.SlideNumber,
			PartURI:       catalog.SlidePartURI,
			LayoutName:    catalog.LayoutName,
			LayoutPartURI: catalog.LayoutPartURI,
			Shapes:        entries,
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXShapesJSON(cmd, result)
		}
		return outputPPTXShapesText(cmd, result)
	},
}

func init() {
	pptxShapesShowCmd.Flags().IntVar(&pptxShapesShowSlide, "slide", 0, "1-based slide number")
	pptxShapesShowCmd.Flags().BoolVar(&pptxShapesShowIncludeText, "include-text", false, "include text preview/content where available")
	pptxShapesShowCmd.Flags().BoolVar(&pptxShapesShowIncludeBounds, "include-bounds", false, "include explicit slide shape bounds and geometry")
	pptxShapesShowCmd.MarkFlagRequired("slide")
	shapesCmd.AddCommand(pptxShapesShowCmd)
}
