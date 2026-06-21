package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// SlidesListItem represents a single slide in the list
type SlidesListItem struct {
	Number                int      `json:"number"`
	SlideID               uint32   `json:"slideId,omitempty"`
	RelationshipID        string   `json:"relationshipId,omitempty"`
	PartURI               string   `json:"partUri"`
	PrimarySelector       string   `json:"primarySelector,omitempty"`
	Handle                string   `json:"handle,omitempty"`
	Selectors             []string `json:"selectors,omitempty"`
	Layout                string   `json:"layout"`
	LayoutNumber          int      `json:"layoutNumber,omitempty"`
	LayoutPartURI         string   `json:"layoutPartUri,omitempty"`
	NotesPartURI          string   `json:"notesPartUri,omitempty"`
	TextShapes            int      `json:"textShapes"`
	Images                int      `json:"images"`
	Tables                int      `json:"tables"`
	Notes                 bool     `json:"notes"`
	ReadbackCommand       string   `json:"readbackCommand,omitempty"`
	SelectorsCommand      string   `json:"selectorsCommand,omitempty"`
	ShapesCommand         string   `json:"shapesCommand,omitempty"`
	TablesCommand         string   `json:"tablesCommand,omitempty"`
	LayoutReadbackCommand string   `json:"layoutReadbackCommand,omitempty"`
}

// SlidesListResult represents the JSON result of the slides list command
type SlidesListResult struct {
	File   string           `json:"file"`
	Slides []SlidesListItem `json:"slides"`
}

var slidesListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List slides in a presentation",
	Long: `List all slides in a PPTX presentation with layout information and shape counts.

Shows:
  - Slide number
  - Part URI
  - Layout name
  - Number of text shapes
  - Number of images
  - Number of tables
  - Whether the slide has notes`,
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

		// Build slide list
		slides := []SlidesListItem{}
		sldIDCounts := pptxSlideIDCounts(graph)

		for _, slideRef := range graph.Slides {
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

			// Count shapes (text, images, tables)
			spTree := findPPTXShapeTree(slideDoc.Root())
			textShapes := 0
			images := 0
			tables := 0

			if spTree != nil {
				textShapes, images, tables = countPPTXSlideShapeTypes(spTree)
			}

			// Check for notes
			hasNotes := slideRef.NotesPartURI != ""
			tablesCommand := ""
			if tables > 0 {
				tablesCommand = pptxSlideTablesCommand(filePath, slideRef.SlideNumber)
			}
			layoutCommand := ""
			if layoutNumber > 0 {
				layoutCommand = pptxSlideLayoutCommand(filePath, layoutNumber)
			}

			item := SlidesListItem{
				Number:                slideRef.SlideNumber,
				SlideID:               slideRef.SlideID,
				RelationshipID:        slideRef.RelationshipID,
				PartURI:               slideRef.PartURI,
				PrimarySelector:       pptxSlidePrimarySelector(slideRef),
				Handle:                pptxSlideHandleString(slideRef, sldIDCounts),
				Selectors:             pptxSlideSelectors(slideRef),
				Layout:                layoutName,
				LayoutNumber:          layoutNumber,
				LayoutPartURI:         slideRef.LayoutPartURI,
				NotesPartURI:          slideRef.NotesPartURI,
				TextShapes:            textShapes,
				Images:                images,
				Tables:                tables,
				Notes:                 hasNotes,
				ReadbackCommand:       pptxSlideReadbackCommand(filePath, slideRef.SlideNumber),
				SelectorsCommand:      pptxSlideSelectorsCommand(filePath, slideRef.SlideNumber),
				ShapesCommand:         pptxSlideShapesCommand(filePath, slideRef.SlideNumber),
				TablesCommand:         tablesCommand,
				LayoutReadbackCommand: layoutCommand,
			}

			slides = append(slides, item)
		}

		// Format and output results
		if config.Format == "json" {
			return outputSlidesListJSON(cmd, filePath, slides)
		}

		// Default to text output
		return outputSlidesListText(cmd, slides)
	},
}

// outputSlidesListJSON outputs the slides list in JSON format
func outputSlidesListJSON(cmd *cobra.Command, filePath string, slides []SlidesListItem) error {
	config := GetGlobalConfig(cmd)

	result := SlidesListResult{
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

// outputSlidesListText outputs the slides list in text format
func outputSlidesListText(cmd *cobra.Command, slides []SlidesListItem) error {
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

	// Print header
	fmt.Fprintf(outFile, "%-4s %-30s %-25s %-12s %-9s %-8s %-8s\n",
		"[N]", "PartURI", "Layout", "textShapes", "images", "tables", "notes")
	fmt.Fprintf(outFile, "%s\n", strings.Repeat("-", 100))

	// Print each slide
	for _, slide := range slides {
		notes := "no"
		if slide.Notes {
			notes = "yes"
		}

		fmt.Fprintf(outFile, "[%-2d] %-30s %-25s %-12d %-9d %-8d %-8s\n",
			slide.Number,
			truncateStr(slide.PartURI, 28),
			truncateStr(slide.Layout, 23),
			slide.TextShapes,
			slide.Images,
			slide.Tables,
			notes,
		)
	}

	return nil
}

// truncateStr truncates a string to a maximum length
func truncateStr(s string, maxLen int) string {
	if len(s) > maxLen {
		return s[:maxLen-3] + "..."
	}
	return s
}

// init registers the slides list command
func init() {
	slidesCmd.AddCommand(slidesListCmd)
}
