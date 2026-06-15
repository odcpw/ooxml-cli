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

// SlidesShowResult represents the JSON result of the slides show command
type SlidesShowResult struct {
	File   string           `json:"file"`
	Slides []SlidesShowItem `json:"slides"`
}

type SlidesShowItem struct {
	model.SlideReport
	SlideID               uint32   `json:"slideId,omitempty"`
	RelationshipID        string   `json:"relationshipId,omitempty"`
	PrimarySelector       string   `json:"primarySelector,omitempty"`
	Selectors             []string `json:"selectors,omitempty"`
	LayoutNumber          int      `json:"layoutNumber,omitempty"`
	LayoutPartURI         string   `json:"layoutPartUri,omitempty"`
	ReadbackCommand       string   `json:"readbackCommand,omitempty"`
	SelectorsCommand      string   `json:"selectorsCommand,omitempty"`
	ShapesCommand         string   `json:"shapesCommand,omitempty"`
	TablesCommand         string   `json:"tablesCommand,omitempty"`
	LayoutReadbackCommand string   `json:"layoutReadbackCommand,omitempty"`
}

var (
	slideNum      int
	includeText   bool
	includeBounds bool
)

var slidesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show detailed information about slides",
	Long: `Show detailed information about one slide or all slides in a PPTX presentation.

Flags:
  --slide <n>          Slide number to show (1-indexed). Omit to show all slides.
  --include-text       Include full text content from shapes
  --include-bounds     Include shape bounds (x, y, cx, cy in EMU)`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open the package
		session, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer session.Close()

		// Parse presentation
		graph, err := inspect.ParsePresentation(session)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Determine which slides to show
		var slidesToShow []int
		if slideNum > 0 {
			slidesToShow = append(slidesToShow, slideNum)
		} else {
			// Show all slides
			for _, slideRef := range graph.Slides {
				slidesToShow = append(slidesToShow, slideRef.SlideNumber)
			}
		}

		// Build slide reports
		slides := []SlidesShowItem{}

		for _, slideNumber := range slidesToShow {
			if slideNumber < 1 || slideNumber > len(graph.Slides) {
				return NewCLIErrorf(ExitInvalidArgs, "slide number %d is out of range (1-%d)", slideNumber, len(graph.Slides))
			}

			slideRef := graph.Slides[slideNumber-1]

			// Read slide XML
			slideDoc, err := session.ReadXMLPart(slideRef.PartURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read slide %d (%s): %v", slideRef.SlideNumber, slideRef.PartURI, err)
			}

			// Get layout name
			layoutName := ""
			for _, layout := range graph.Layouts {
				if layout.PartURI == slideRef.LayoutPartURI {
					layoutName = layout.Name
					break
				}
			}
			layoutNumber := pptxSlideLayoutNumber(graph, slideRef.LayoutPartURI)

			// Extract shapes
			spTree := findPPTXShapeTree(slideDoc.Root())
			shapeList := []model.ShapeInfo{}
			tableCount := 0
			if spTree != nil {
				shapeList = inspect.EnumerateShapes(spTree)
				_, _, tableCount = countPPTXSlideShapeTypes(spTree)
			}

			if includeText {
				attachPPTXSlideText(spTree, shapeList)
			}

			// Convert to pointers
			shapes := make([]*model.ShapeInfo, len(shapeList))
			for i := range shapeList {
				shapes[i] = &shapeList[i]
			}

			// Remove bounds if not requested
			if !includeBounds {
				for i := range shapes {
					shapes[i].Bounds = nil
				}
			}

			tablesCommand := ""
			if tableCount > 0 {
				tablesCommand = pptxSlideTablesCommand(filePath, slideRef.SlideNumber)
			}
			layoutCommand := ""
			if layoutNumber > 0 {
				layoutCommand = pptxSlideLayoutCommand(filePath, layoutNumber)
			}

			report := SlidesShowItem{
				SlideReport: model.SlideReport{
					ID:           fmt.Sprintf("slide%d", slideRef.SlideNumber),
					Slide:        slideRef.SlideNumber,
					PartURI:      slideRef.PartURI,
					LayoutRef:    layoutName,
					NotesPartURI: slideRef.NotesPartURI,
					Shapes:       shapes,
				},
				SlideID:               slideRef.SlideID,
				RelationshipID:        slideRef.RelationshipID,
				PrimarySelector:       pptxSlidePrimarySelector(slideRef),
				Selectors:             pptxSlideSelectors(slideRef),
				LayoutNumber:          layoutNumber,
				LayoutPartURI:         slideRef.LayoutPartURI,
				ReadbackCommand:       pptxSlideReadbackCommand(filePath, slideRef.SlideNumber),
				SelectorsCommand:      pptxSlideSelectorsCommand(filePath, slideRef.SlideNumber),
				ShapesCommand:         pptxSlideShapesCommand(filePath, slideRef.SlideNumber),
				TablesCommand:         tablesCommand,
				LayoutReadbackCommand: layoutCommand,
			}

			slides = append(slides, report)
		}

		// Format and output results
		if config.Format == "json" {
			return outputSlidesShowJSON(cmd, filePath, slides)
		}

		// Default to text output
		return outputSlidesShowText(cmd, slides)
	},
}

// outputSlidesShowJSON outputs the slides show in JSON format
func outputSlidesShowJSON(cmd *cobra.Command, filePath string, slides []SlidesShowItem) error {
	config := GetGlobalConfig(cmd)

	result := SlidesShowResult{
		File:   filePath,
		Slides: slides,
	}

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

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "%s\n", string(jsonData))
	return nil
}

// outputSlidesShowText outputs the slides show in text format
func outputSlidesShowText(cmd *cobra.Command, slides []SlidesShowItem) error {
	config := GetGlobalConfig(cmd)

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	for _, slide := range slides {
		fmt.Fprintf(outFile, "Slide %d: %s\n", slide.Slide, slide.PartURI)
		fmt.Fprintf(outFile, "Layout: %s\n", slide.LayoutRef)
		if slide.SlideID != 0 || slide.RelationshipID != "" {
			fmt.Fprintf(outFile, "Handles: primary=%s slideId=%d relationshipId=%s\n",
				slide.PrimarySelector, slide.SlideID, slide.RelationshipID)
		}

		if len(slide.Shapes) > 0 {
			fmt.Fprintf(outFile, "Shapes:\n")
			for _, shape := range slide.Shapes {
				fmt.Fprintf(outFile, "  [%d] %s (type=%s, placeholder=%v)\n",
					shape.ID, shape.Name, shape.Type, shape.IsPlaceholder)

				if shape.Bounds != nil && includeBounds {
					fmt.Fprintf(outFile, "      bounds: x=%d y=%d cx=%d cy=%d\n",
						shape.Bounds.X, shape.Bounds.Y, shape.Bounds.CX, shape.Bounds.CY)
				}

				if shape.ImageRef != nil {
					fmt.Fprintf(outFile, "      image: %s\n", shape.ImageRef.TargetURI)
				}

				if shape.TableInfo != nil {
					fmt.Fprintf(outFile, "      table: %dx%d\n", shape.TableInfo.Rows, shape.TableInfo.Cols)
				}
			}
		}

		fmt.Fprintf(outFile, "\n")
	}

	return nil
}

// init registers the slides show command
func init() {
	slidesShowCmd.Flags().IntVar(&slideNum, "slide", 0, "Slide number to show (1-indexed)")
	slidesShowCmd.Flags().BoolVar(&includeText, "include-text", false, "Include full text content from shapes")
	slidesShowCmd.Flags().BoolVar(&includeBounds, "include-bounds", false, "Include shape bounds (x, y, cx, cy in EMU)")

	slidesCmd.AddCommand(slidesShowCmd)
}
