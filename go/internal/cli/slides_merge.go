package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

// SlidesMergeResult represents the JSON result of the slides merge command
type SlidesMergeResult struct {
	File             string `json:"file"`
	SourceFile       string `json:"sourceFile"`
	MergedSlideCount int    `json:"mergedSlideCount"`
	TotalSlideCount  int    `json:"totalSlideCount"`
	LayoutPolicy     string `json:"layoutPolicy"`
	ThemePolicy      string `json:"themePolicy"`
}

var slidesMergeCmd = &cobra.Command{
	Use:   "merge <target-file> <source-file>",
	Short: "Merge all slides from a source deck into a target deck",
	Long: `Merge all slides from a source presentation into a target presentation.

This command imports all slides from the source deck into the target deck,
handling media, layouts, masters, and themes according to the specified policies:
  - reuse: Reuse existing layouts/themes in the target (default)
  - import: Import layouts/themes from the source

Example:
  ooxml pptx slides merge deck1.pptx deck2.pptx  # Merge deck2 into deck1
  ooxml pptx slides merge --output merged.pptx deck1.pptx deck2.pptx

Exit codes:
  0: Success
  1: File not found
  2: Invalid arguments
  3: Other errors`,
	Args: cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		targetPath := args[0]
		sourcePath := args[1]

		// Check if files exist
		if _, err := os.Stat(targetPath); err != nil {
			return FileNotFoundError(targetPath)
		}
		if _, err := os.Stat(sourcePath); err != nil {
			return FileNotFoundError(sourcePath)
		}

		// Get mutation options
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		// Get policy flags
		layoutPolicy, _ := cmd.Flags().GetString("layout-policy")
		themePolicy, _ := cmd.Flags().GetString("theme-policy")

		// Perform the merge operation
		result, totalSlides, err := performMergeDeck(targetPath, sourcePath, layoutPolicy, themePolicy, mutOpts)
		if err != nil {
			return err
		}

		// Format output
		config := GetGlobalConfig(cmd)
		outputFileName := filepath.Base(mutOpts.OutPath)
		sourceFileName := filepath.Base(sourcePath)

		if config.Format == "json" {
			return outputSlidesMergeJSON(cmd, outputFileName, sourceFileName, result, totalSlides, layoutPolicy, themePolicy)
		}

		return outputSlidesMergeText(cmd, outputFileName, sourceFileName, result, totalSlides, layoutPolicy, themePolicy)
	},
}

func performMergeDeck(targetPath, sourcePath string, layoutPolicy, themePolicy string, mutOpts *MutationOptions) (*mutate.MergeDeckResult, int, error) {
	// Open source package (read-only)
	sourcePkg, err := openPackageExpectType(sourcePath, opc.PackageTypePPTX)
	if err != nil {
		return nil, 0, err
	}
	defer sourcePkg.Close()

	// Parse source to validate it has slides
	sourceGraph, err := inspect.ParsePresentation(sourcePkg)
	if err != nil {
		return nil, 0, NewCLIErrorf(ExitUnexpected, "failed to parse source presentation: %v", err)
	}

	if len(sourceGraph.Slides) == 0 {
		return nil, 0, NewCLIErrorf(ExitUnexpected, "source presentation has no slides")
	}

	writer, err := NewMutationWriter(targetPath, mutOpts)
	if err != nil {
		return nil, 0, err
	}

	var result *mutate.MergeDeckResult
	var totalSlides int

	if err := writer.Write(func(targetPkg opc.PackageSession) error {
		// Parse target to get initial slide count
		targetGraph, err := inspect.ParsePresentation(targetPkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse target presentation: %v", err)
		}

		initialSlideCount := len(targetGraph.Slides)

		// Perform merge
		mergeResult, err := mutate.MergeDeck(&mutate.MergeDeckRequest{
			TargetPackage: targetPkg,
			SourcePackage: sourcePkg,
			LayoutPolicy:  layoutPolicy,
			ThemePolicy:   themePolicy,
			NotesPolicy:   mutate.NotesClone,
		})
		if err != nil {
			if cliErr, ok := AsCLIError(err); ok {
				return cliErr
			}
			return NewCLIErrorf(ExitUnexpected, "failed to merge decks: %v", err)
		}

		result = mergeResult
		totalSlides = initialSlideCount + mergeResult.MergedSlideCount

		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, 0, cliErr
		}
		return nil, 0, NewCLIErrorf(ExitUnexpected, "failed to merge decks: %v", err)
	}

	return result, totalSlides, nil
}

// outputSlidesMergeJSON outputs the merge result in JSON format
func outputSlidesMergeJSON(cmd *cobra.Command, targetFile, sourceFile string, result *mutate.MergeDeckResult, totalSlides int, layoutPolicy, themePolicy string) error {
	config := GetGlobalConfig(cmd)

	output := SlidesMergeResult{
		File:             targetFile,
		SourceFile:       sourceFile,
		MergedSlideCount: result.MergedSlideCount,
		TotalSlideCount:  totalSlides,
		LayoutPolicy:     layoutPolicy,
		ThemePolicy:      themePolicy,
	}

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

// outputSlidesMergeText outputs the merge result in text format
func outputSlidesMergeText(cmd *cobra.Command, targetFile, sourceFile string, result *mutate.MergeDeckResult, totalSlides int, layoutPolicy, themePolicy string) error {
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

	fmt.Fprintf(outFile, "Target file: %s\n", targetFile)
	fmt.Fprintf(outFile, "Source file: %s\n", sourceFile)
	fmt.Fprintf(outFile, "Merged slides: %d\n", result.MergedSlideCount)
	fmt.Fprintf(outFile, "Total slides: %d\n", totalSlides)
	fmt.Fprintf(outFile, "Layout policy: %s\n", layoutPolicy)
	fmt.Fprintf(outFile, "Theme policy: %s\n", themePolicy)

	return nil
}

// init registers the slides merge command
func init() {
	AddMutationFlags(slidesMergeCmd)
	slidesMergeCmd.Flags().StringP("layout-policy", "l", "reuse", "Layout policy: 'reuse' or 'import'")
	slidesMergeCmd.Flags().StringP("theme-policy", "t", "reuse", "Theme policy: 'reuse' or 'import'")
	slidesCmd.AddCommand(slidesMergeCmd)
}
