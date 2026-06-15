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

var (
	extractXMLSlides  []int
	extractXMLLayouts []int
	extractXMLMasters []int
	extractXMLOut     string
)

var extractXMLCmd = &cobra.Command{
	Use:   "xml <file>",
	Short: "Extract raw XML from slides, layouts, and masters",
	Long: `Extract raw XML content and relationships from PPTX presentations.

Flags:
  --slide <n>       Slide number to extract (1-indexed). Can be used multiple times.
  --layout <n>      Layout number to extract (1-indexed). Can be used multiple times.
  --master <n>      Master number to extract (1-indexed). Can be used multiple times.
  --out <dir>       Output directory for extracted XML files (required)

For debugging and reverse engineering. Preserves original package bytes without reserialization.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Validate output directory
		if extractXMLOut == "" {
			return NewCLIErrorf(ExitInvalidArgs, "output directory required (--out)")
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

		// Create output directory
		if err := os.MkdirAll(extractXMLOut, 0755); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output directory: %v", err)
		}

		// Extract selected items
		result := &extract.ExtractXMLResult{
			File:      filePath,
			OutputDir: extractXMLOut,
		}

		// Determine what to extract
		var itemsToExtract []extract.ExtractItem

		// Process slides
		if len(extractXMLSlides) > 0 {
			for _, slideNum := range extractXMLSlides {
				if slideNum < 1 || slideNum > len(graph.Slides) {
					return NewCLIErrorf(ExitInvalidArgs, "slide number %d is out of range (1-%d)", slideNum, len(graph.Slides))
				}
				slideRef := graph.Slides[slideNum-1]
				itemsToExtract = append(itemsToExtract, extract.ExtractItem{
					Type:    "slide",
					Number:  slideNum,
					PartURI: slideRef.PartURI,
				})
			}
		}

		// Process layouts
		if len(extractXMLLayouts) > 0 {
			for _, layoutNum := range extractXMLLayouts {
				if layoutNum < 1 || layoutNum > len(graph.Layouts) {
					return NewCLIErrorf(ExitInvalidArgs, "layout number %d is out of range (1-%d)", layoutNum, len(graph.Layouts))
				}
				layoutRef := graph.Layouts[layoutNum-1]
				itemsToExtract = append(itemsToExtract, extract.ExtractItem{
					Type:    "layout",
					Number:  layoutNum,
					PartURI: layoutRef.PartURI,
				})
			}
		}

		// Process masters
		if len(extractXMLMasters) > 0 {
			for _, masterNum := range extractXMLMasters {
				if masterNum < 1 || masterNum > len(graph.Masters) {
					return NewCLIErrorf(ExitInvalidArgs, "master number %d is out of range (1-%d)", masterNum, len(graph.Masters))
				}
				masterRef := graph.Masters[masterNum-1]
				itemsToExtract = append(itemsToExtract, extract.ExtractItem{
					Type:    "master",
					Number:  masterNum,
					PartURI: masterRef.PartURI,
				})
			}
		}

		// If nothing selected, default to all slides
		if len(itemsToExtract) == 0 {
			for idx, slideRef := range graph.Slides {
				itemsToExtract = append(itemsToExtract, extract.ExtractItem{
					Type:    "slide",
					Number:  idx + 1,
					PartURI: slideRef.PartURI,
				})
			}
		}

		// Extract each item
		for _, item := range itemsToExtract {
			if err := extract.ExtractXML(session, &item, extractXMLOut); err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to extract %s %d: %v", item.Type, item.Number, err)
			}
			result.ExtractedItems = append(result.ExtractedItems, fmt.Sprintf("%s-%d", item.Type, item.Number))
		}

		// Output result
		if err := outputExtractXMLResult(cmd, result); err != nil {
			return err
		}

		return nil
	},
}

// outputExtractXMLResult outputs the extraction result
func outputExtractXMLResult(cmd *cobra.Command, result *extract.ExtractXMLResult) error {
	config := GetGlobalConfig(cmd)

	var outWriter io.Writer

	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outWriter = file
	} else {
		outWriter = cmd.OutOrStdout()
	}

	if config.Format == "json" {
		// JSON output
		jsonData := map[string]interface{}{
			"file":       result.File,
			"output_dir": result.OutputDir,
			"extracted":  result.ExtractedItems,
		}

		var output []byte
		var err error
		if config.Pretty {
			output, err = json.MarshalIndent(jsonData, "", "  ")
		} else {
			output, err = json.Marshal(jsonData)
		}

		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
		}

		fmt.Fprintf(outWriter, "%s\n", string(output))
	} else {
		// Text output
		fmt.Fprintf(outWriter, "Extracted XML to: %s\n", result.OutputDir)
		fmt.Fprintf(outWriter, "Items:\n")
		for _, item := range result.ExtractedItems {
			fmt.Fprintf(outWriter, "  - %s\n", item)
		}
	}

	return nil
}

// init registers the extract xml command
func init() {
	extractXMLCmd.Flags().IntSliceVar(&extractXMLSlides, "slide", []int{}, "Slide number to extract (1-indexed, can be used multiple times)")
	extractXMLCmd.Flags().IntSliceVar(&extractXMLLayouts, "layout", []int{}, "Layout number to extract (1-indexed, can be used multiple times)")
	extractXMLCmd.Flags().IntSliceVar(&extractXMLMasters, "master", []int{}, "Master number to extract (1-indexed, can be used multiple times)")
	extractXMLCmd.Flags().StringVar(&extractXMLOut, "out", "", "Output directory for extracted XML files (required)")

	extractCmd.AddCommand(extractXMLCmd)
}
