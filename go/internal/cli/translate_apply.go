package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/translate"
)

var (
	translateApplyManifestPath string
	translateApplyStaleMode    string
	translateApplyOutputPath   string
)

var translateApplyCmd = &cobra.Command{
	Use:   "apply <file> <manifest>",
	Short: "Apply translations from a manifest to a PPTX",
	Long: `Apply translations from a translation manifest to a PPTX presentation.

Loads a translation manifest (created by "translate export"), verifies source-text
freshness, and applies the translations back to the presentation. Handles stale entries
(where source text has changed) based on the --stale mode.

Flags:
  --stale <mode>    How to handle stale entries: skip (default), warn, or error
  --output <file>   Write modified PPTX to this path (default: overwrite input)`,
	Args: cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		manifestPath := args[1]

		// Check if files exist
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		if _, err := os.Stat(manifestPath); err != nil {
			return FileNotFoundError(manifestPath)
		}

		// Read the manifest file
		manifestData, err := os.ReadFile(manifestPath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read manifest: %v", err)
		}

		// Parse manifest JSON
		var manifest translate.TranslationManifest
		if err := json.Unmarshal(manifestData, &manifest); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse manifest JSON: %v", err)
		}

		// Open the PPTX file
		session, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer session.Close()

		// Validate stale mode
		staleMode := translate.StaleEntryMode(translateApplyStaleMode)
		if staleMode == "" {
			staleMode = translate.StaleEntrySkip
		} else if staleMode != translate.StaleEntrySkip &&
			staleMode != translate.StaleEntryWarn &&
			staleMode != translate.StaleEntryError {
			return NewCLIErrorf(ExitInvalidArgs,
				"invalid --stale mode: %q (must be 'skip', 'warn', or 'error')",
				translateApplyStaleMode)
		}

		// Build apply request
		applyReq := &translate.ApplyTranslationRequest{
			Package:        session,
			Manifest:       &manifest,
			StaleEntryMode: staleMode,
			OnWarning: func(msg string) {
				fmt.Fprintf(os.Stderr, "WARNING: %s\n", msg)
			},
		}

		// Apply translations
		result := translate.ApplyTranslation(applyReq)

		// Handle errors
		if result.Error != nil {
			return NewCLIErrorf(ExitUnexpected, "translation apply failed: %v", result.Error)
		}

		// Determine output path
		outputPath := filePath
		if translateApplyOutputPath != "" {
			outputPath = translateApplyOutputPath
		}

		// Save the modified presentation
		if outputPath == filePath {
			// Overwrite original
			if err := session.SaveAs(outputPath); err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to save modified PPTX: %v", err)
			}
		} else {
			// Save to new location
			if err := session.SaveAs(outputPath); err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to save PPTX to %s: %v", outputPath, err)
			}
		}

		// Output results
		config := GetGlobalConfig(cmd)
		return outputApplyResult(cmd, result, config.Pretty)
	},
}

// ApplyResult is the JSON output structure for the apply command
type ApplyResult struct {
	EntriesProcessed int      `json:"entriesProcessed"`
	EntriesApplied   int      `json:"entriesApplied"`
	EntriesSkipped   int      `json:"entriesSkipped"`
	Warnings         []string `json:"warnings,omitempty"`
}

// outputApplyResult outputs the apply result in JSON or text format
func outputApplyResult(cmd *cobra.Command, result *translate.ApplyTranslationResult, pretty bool) error {
	config := GetGlobalConfig(cmd)

	applyResult := ApplyResult{
		EntriesProcessed: result.EntriesProcessed,
		EntriesApplied:   result.EntriesApplied,
		EntriesSkipped:   result.EntriesSkipped,
		Warnings:         result.Warnings,
	}

	var jsonData []byte
	var err error
	if pretty {
		jsonData, err = json.MarshalIndent(applyResult, "", "  ")
	} else {
		jsonData, err = json.Marshal(applyResult)
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

	if config.Format == "json" {
		fmt.Fprintf(outFile, "%s\n", string(jsonData))
	} else {
		// Text format
		fmt.Fprintf(outFile, "Translation Apply Result\n")
		fmt.Fprintf(outFile, "========================\n\n")
		fmt.Fprintf(outFile, "Entries Processed: %d\n", applyResult.EntriesProcessed)
		fmt.Fprintf(outFile, "Entries Applied:  %d\n", applyResult.EntriesApplied)
		fmt.Fprintf(outFile, "Entries Skipped:  %d\n", applyResult.EntriesSkipped)

		if len(applyResult.Warnings) > 0 {
			fmt.Fprintf(outFile, "\nWarnings:\n")
			for i, w := range applyResult.Warnings {
				fmt.Fprintf(outFile, "  %d. %s\n", i+1, w)
			}
		}
	}

	return nil
}

// init registers the translate apply command
func init() {
	translateApplyCmd.Flags().StringVar(&translateApplyStaleMode, "stale", "skip",
		"How to handle stale entries: skip (default), warn, or error")
	translateApplyCmd.Flags().StringVar(&translateApplyOutputPath, "output", "",
		"Output PPTX path (default: overwrite input)")

	translateCmd.AddCommand(translateApplyCmd)
}
