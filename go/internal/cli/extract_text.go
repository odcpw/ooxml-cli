package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// ExtractTextResult represents the JSON result of the extract text command
type ExtractTextResult struct {
	File   string                   `json:"file"`
	Slides []extract.ExtractedSlide `json:"slides"`
}

var (
	extractTextSlides []int
)

var extractTextCmd = &cobra.Command{
	Use:   "text <file>",
	Short: "Extract text from slides",
	Long: `Extract text content from one or more slides in a PPTX presentation.

Text is organized by shape with normalized placeholder keys. For each shape, 
the extracted text includes paragraphs and plain text representation.

Flags:
  --slide <n>      Slide number to extract (1-indexed). Can be used multiple times.
  --format         Output format: text or json (default: text)`,
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

		// Build request for text extraction
		extractReq := &extract.ExtractTextRequest{
			Session:      session,
			Graph:        graph,
			SlideNumbers: extractTextSlides,
		}

		// Perform text extraction
		result, err := extract.ExtractText(extractReq)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to extract text: %v", err)
		}

		// Set the file path in the result
		result.File = filePath

		// Format and output results
		if config.Format == "json" {
			return outputExtractTextJSON(cmd, result)
		}

		// Default to text output
		return outputExtractTextText(cmd, result)
	},
}

// outputExtractTextJSON outputs the extracted text in JSON format
func outputExtractTextJSON(cmd *cobra.Command, result *extract.TextExtractionResult) error {
	config := GetGlobalConfig(cmd)

	jsonResult := ExtractTextResult{
		File:   result.File,
		Slides: result.Slides,
	}

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(jsonResult, "", "  ")
	} else {
		jsonData, err = json.Marshal(jsonResult)
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

// outputExtractTextText outputs the extracted text in text format
func outputExtractTextText(cmd *cobra.Command, result *extract.TextExtractionResult) error {
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

	for _, slide := range result.Slides {
		fmt.Fprintf(outFile, "=== Slide %d ===\n", slide.Slide)

		if len(slide.Shapes) == 0 {
			fmt.Fprintf(outFile, "  (no text shapes)\n")
			continue
		}

		for _, shape := range slide.Shapes {
			fmt.Fprintf(outFile, "  [%s] %s\n", shape.Key, shape.Name)

			if shape.Text != nil && shape.Text.PlainText != "" {
				// Indent text content
				lines := strings.Split(shape.Text.PlainText, "\n")
				for _, line := range lines {
					fmt.Fprintf(outFile, "    %s\n", line)
				}
			}
		}

		fmt.Fprintf(outFile, "\n")
	}

	return nil
}

// init registers the extract text command
func init() {
	extractTextCmd.Flags().IntSliceVar(&extractTextSlides, "slide", []int{}, "Slide number to extract (1-indexed). Can be used multiple times.")

	extractCmd.AddCommand(extractTextCmd)
}
