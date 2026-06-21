package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// ExtractNotesResult represents the JSON result of the extract notes command
type ExtractNotesResult struct {
	File  string                `json:"file"`
	Notes []extract.NotesReport `json:"notes"`
}

var (
	notesSlideNum int
)

var extractNotesCmd = &cobra.Command{
	Use:   "notes <file>",
	Short: "Extract speaker notes from slides",
	Long: `Extract speaker notes from one or more slides in a PPTX presentation.

Flags:
  --slide <n>   Slide number to extract notes from (1-indexed). If not specified, extracts notes from all slides.`,
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

		// Determine which slides to extract notes from
		var slidesToProcess []int
		if notesSlideNum > 0 {
			slidesToProcess = append(slidesToProcess, notesSlideNum)
		} else {
			// Process all slides
			for _, slideRef := range graph.Slides {
				slidesToProcess = append(slidesToProcess, slideRef.SlideNumber)
			}
		}

		// Extract notes
		notes := []extract.NotesReport{}

		for _, slideNumber := range slidesToProcess {
			if slideNumber < 1 || slideNumber > len(graph.Slides) {
				return NewCLIErrorf(ExitInvalidArgs, "slide number %d is out of range (1-%d)", slideNumber, len(graph.Slides))
			}

			slideRef := graph.Slides[slideNumber-1]

			// Extract notes for this slide
			notesReport, err := extract.ExtractNotesForSlide(session, slideRef)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to extract notes from slide %d: %v", slideNumber, err)
			}

			notes = append(notes, *notesReport)
		}

		// Format and output results
		if config.Format == "json" {
			return outputExtractNotesJSON(cmd, filePath, notes)
		}

		// Default to text output
		return outputExtractNotesText(cmd, notes)
	},
}

// outputExtractNotesJSON outputs the extracted notes in JSON format
func outputExtractNotesJSON(cmd *cobra.Command, filePath string, notes []extract.NotesReport) error {
	config := GetGlobalConfig(cmd)

	result := ExtractNotesResult{
		File:  filePath,
		Notes: notes,
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

// outputExtractNotesText outputs the extracted notes in text format
func outputExtractNotesText(cmd *cobra.Command, notes []extract.NotesReport) error {
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

	for _, notesReport := range notes {
		fmt.Fprintf(outFile, "Slide %d Notes:\n", notesReport.Slide)
		if notesReport.PartURI != "" {
			fmt.Fprintf(outFile, "Part URI: %s\n", notesReport.PartURI)
		}

		if notesReport.Notes == nil || notesReport.Notes.PlainText == "" {
			fmt.Fprintf(outFile, "(no notes)\n")
		} else {
			fmt.Fprintf(outFile, "%s\n", notesReport.Notes.PlainText)
		}

		fmt.Fprintf(outFile, "\n")
	}

	return nil
}

// init registers the extract notes command
func init() {
	extractNotesCmd.Flags().IntVar(&notesSlideNum, "slide", 0, "Slide number to extract notes from (1-indexed)")

	extractCmd.AddCommand(extractNotesCmd)
}
