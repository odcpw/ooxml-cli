package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

var validateLayoutCmd = &cobra.Command{
	Use:   "validate-layout <file>",
	Short: "Validate slide layout for overflow, collisions, and density",
	Long: `Validate slide layout quality in a PPTX file.

Analyzes all slides for:
- Text overflow (content exceeding shape bounds)
- Shape collisions (overlapping shapes)
- Slide density (area occupancy metrics)

Reports issues in text or JSON format based on --format flag.

Exit codes:
- 0: Success, no issues detected
- 3: File not found
- 4: Unsupported file type (non-PPTX)
- 1: Unexpected error`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open the package
		pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		// Get global config
		config := GetGlobalConfig(cmd)

		// Analyze the presentation
		analysis, err := analyzePresentation(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to analyze presentation: %v", err)
		}

		// Format and output results
		if config.Format == "json" {
			return outputValidateLayoutJSON(cmd, filePath, analysis)
		}

		// Default to text output
		return outputValidateLayoutText(cmd, filePath, analysis)
	},
}

// analyzePresentation analyzes the entire presentation for layout issues
func analyzePresentation(pkg opc.PackageSession) (*model.LayoutQAAnalysis, error) {
	// Parse the presentation structure
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	var slideReports []model.LayoutQAReport

	// Get slide dimensions (use standard if not available: 10 inches x 7.5 inches = 9144000 x 6858000 EMU)
	slideWidth := int64(9144000)
	slideHeight := int64(6858000)
	if graph.SlideSize.CX > 0 && graph.SlideSize.CY > 0 {
		slideWidth = graph.SlideSize.CX
		slideHeight = graph.SlideSize.CY
	}

	// Analyze each slide
	for _, slideRef := range graph.Slides {
		report, err := analyzeSlide(pkg, slideRef, slideWidth, slideHeight)
		if err != nil {
			// Log error but continue with next slide
			fmt.Fprintf(os.Stderr, "Warning: failed to analyze slide %d: %v\n", slideRef.SlideNumber, err)
			continue
		}
		slideReports = append(slideReports, report)
	}

	// Aggregate results into LayoutQAAnalysis
	analysis := inspect.AnalyzePresentationLayoutQA(slideReports)

	return &analysis, nil
}

// analyzeSlide analyzes a single slide for layout issues
func analyzeSlide(pkg opc.PackageSession, slideRef inspect.SlideRef, slideWidth, slideHeight int64) (model.LayoutQAReport, error) {
	// Read the slide XML
	slideXML, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return model.LayoutQAReport{}, fmt.Errorf("failed to read slide XML: %w", err)
	}

	// Enumerate shapes on the slide
	spTree := slideXML.Root().FindElement("cSld").FindElement("spTree")
	if spTree == nil {
		// Empty slide
		return model.LayoutQAReport{
			SlideIndex:  slideRef.SlideNumber - 1,
			SlideNumber: slideRef.SlideNumber,
		}, nil
	}

	shapes := inspect.EnumerateShapes(spTree)

	// Extract text from shapes
	textBlocks := make(map[int]*model.TextBlockInfo)
	for _, sp := range spTree.FindElements("sp") {
		// Get shape ID
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr != nil {
			cNvPr := nvSpPr.FindElement("cNvPr")
			if cNvPr != nil {
				shapeID := cNvPr.SelectAttrValue("id", "")
				if shapeID != "" {
					// Extract text from this shape
					txBody := sp.FindElement("txBody")
					if txBody != nil {
						textBlock := inspect.ExtractTextBody(txBody)
						if textBlock != nil && textBlock.PlainText != "" {
							// Store by shape name as key for now (will match with shapes list)
							shapeName := cNvPr.SelectAttrValue("name", "")
							for i := range shapes {
								if shapes[i].Name == shapeName {
									textBlocks[shapes[i].ID] = textBlock
									break
								}
							}
						}
					}
				}
			}
		}
	}

	// Analyze the slide
	report := inspect.AnalyzeSlideLayoutQA(slideRef.SlideNumber-1, shapes, textBlocks, slideWidth, slideHeight)

	return report, nil
}

// validateLayoutOutput wraps the analysis with a file field for JSON output
type validateLayoutOutput struct {
	File string `json:"file"`
	*model.LayoutQAAnalysis
}

// outputValidateLayoutJSON outputs the validation results in JSON format
func outputValidateLayoutJSON(cmd *cobra.Command, filePath string, analysis *model.LayoutQAAnalysis) error {
	config := GetGlobalConfig(cmd)

	// Create output structure - marshal the struct directly so averageDensity is a number
	output := &validateLayoutOutput{
		File:             filePath,
		LayoutQAAnalysis: analysis,
	}

	// Marshal to JSON
	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(output, "", "  ")
	} else {
		jsonData, err = json.Marshal(output)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	// Write to output
	var outWriter io.Writer
	if config.Output != "" {
		outFile, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer outFile.Close()
		outWriter = outFile
	} else {
		outWriter = cmd.OutOrStdout()
	}

	fmt.Fprintf(outWriter, "%s\n", string(jsonData))
	return nil
}

// outputValidateLayoutText outputs the validation results in human-readable text format
func outputValidateLayoutText(cmd *cobra.Command, filePath string, analysis *model.LayoutQAAnalysis) error {
	config := GetGlobalConfig(cmd)

	var outWriter io.Writer
	if config.Output != "" {
		outFile, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer outFile.Close()
		outWriter = outFile
	} else {
		outWriter = cmd.OutOrStdout()
	}

	// Summary header
	fmt.Fprintf(outWriter, "Layout Validation Report\n")
	fmt.Fprintf(outWriter, "========================\n")
	fmt.Fprintf(outWriter, "File: %s\n", filePath)
	fmt.Fprintf(outWriter, "Total Slides: %d\n", analysis.TotalSlides)
	fmt.Fprintf(outWriter, "Slides with Issues: %d\n", analysis.SlidesWithIssues)
	fmt.Fprintf(outWriter, "Slides with High Density: %d\n", analysis.SlidesWithHighDensity)
	fmt.Fprintf(outWriter, "Average Density: %.1f%%\n", analysis.AverageDensity)
	fmt.Fprintf(outWriter, "Total Text Overflows: %d\n", analysis.TotalTextOverflows)
	fmt.Fprintf(outWriter, "Total Collisions: %d\n", analysis.TotalCollisions)
	fmt.Fprintf(outWriter, "\n")

	if !analysis.HasIssues {
		fmt.Fprintf(outWriter, "✓ No issues detected.\n")
		return nil
	}

	// Detail sections
	fmt.Fprintf(outWriter, "Issues by Slide\n")
	fmt.Fprintf(outWriter, "---------------\n")

	for _, report := range analysis.SlideReports {
		if !report.HasIssues && (report.Density == nil || report.Density.Classification != "dense") {
			continue
		}

		fmt.Fprintf(outWriter, "\nSlide %d:\n", report.SlideNumber)

		// Density info
		if report.Density != nil {
			fmt.Fprintf(outWriter, "  Density: %.1f%% (%s, %d shapes)\n",
				report.Density.DensityPercentage,
				report.Density.Classification,
				report.Density.ShapeCount)
		}

		// Text overflows
		if len(report.TextOverflows) > 0 {
			fmt.Fprintf(outWriter, "  Text Overflows: %d\n", len(report.TextOverflows))
			for _, overflow := range report.TextOverflows {
				fmt.Fprintf(outWriter, "    - %s (ID: %d) [%s]: %s\n",
					overflow.ShapeName,
					overflow.ShapeID,
					overflow.Severity,
					overflow.Reason)
			}
		}

		// Collisions
		if len(report.Collisions) > 0 {
			fmt.Fprintf(outWriter, "  Collisions: %d\n", len(report.Collisions))
			for _, collision := range report.Collisions {
				fmt.Fprintf(outWriter, "    - %s (ID: %d) overlaps %s (ID: %d) [%s]: %.1f%% overlap\n",
					collision.ShapeName1,
					collision.ShapeID1,
					collision.ShapeName2,
					collision.ShapeID2,
					collision.Severity,
					collision.OverlapPercentageOfSmaller)
			}
		}
	}

	return nil
}

// init registers the validate-layout command with the pptx command
func init() {
	pptxCmd.AddCommand(validateLayoutCmd)
}
