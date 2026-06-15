package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/translate"
)

var (
	translateExportSlides       []int
	translateExportIncludeNotes bool
	translateExportSourceLang   string
	translateExportTargetLang   string
)

var translateExportCmd = &cobra.Command{
	Use:   "export <file>",
	Short: "Export presentation text for translation",
	Long: `Export translatable text from a PPTX presentation as a translation manifest.

The manifest includes slide/key context, paragraph/run information, bullet metadata,
and optional notes. Exports are deterministic with stable IDs for repeated runs.

Flags:
  --slide <n>              Slide number to export (1-indexed). Can be used multiple times.
  --include-notes          Include speaker notes in export (default: false)
  --source-lang <code>     Source language code (e.g., "en-US")
  --target-lang <code>     Target language code (e.g., "fr-FR")
  --format                 Output format: text or json (default: json)`,
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

		// Build export request
		exportReq := &translate.ExportTranslationRequest{
			Session:        session,
			Graph:          graph,
			SlideNumbers:   translateExportSlides,
			IncludeNotes:   translateExportIncludeNotes,
			SourceLanguage: translateExportSourceLang,
			TargetLanguage: translateExportTargetLang,
			DeckName:       filePath,
		}

		// Perform export
		manifest, err := translate.ExportTranslation(exportReq)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to export translation manifest: %v", err)
		}

		// Format and output results
		if config.Format == "text" {
			return outputTranslateExportText(cmd, manifest)
		}

		// Default to JSON output
		return outputTranslateExportJSON(cmd, manifest, config.Pretty)
	},
}

// outputTranslateExportJSON outputs the manifest in JSON format
func outputTranslateExportJSON(cmd *cobra.Command, manifest *translate.TranslationManifest, pretty bool) error {
	config := GetGlobalConfig(cmd)

	var jsonData []byte
	var err error
	if pretty {
		jsonData, err = json.MarshalIndent(manifest, "", "  ")
	} else {
		jsonData, err = json.Marshal(manifest)
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

// outputTranslateExportText outputs the manifest in text format
func outputTranslateExportText(cmd *cobra.Command, manifest *translate.TranslationManifest) error {
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

	// Print metadata
	fmt.Fprintf(outFile, "Translation Manifest\n")
	fmt.Fprintf(outFile, "====================\n\n")

	if manifest.Metadata != nil {
		fmt.Fprintf(outFile, "Version:         %s\n", manifest.Metadata.Version)
		fmt.Fprintf(outFile, "Exported At:     %s\n", manifest.Metadata.ExportedAt.Format("2006-01-02T15:04:05Z07:00"))
		fmt.Fprintf(outFile, "Deck Name:       %s\n", manifest.Metadata.DeckName)
		fmt.Fprintf(outFile, "Slide Count:     %d\n", manifest.Metadata.SlideCount)
		fmt.Fprintf(outFile, "Entry Count:     %d\n", manifest.Metadata.EntryCount)
		if manifest.Metadata.SourceLanguage != "" {
			fmt.Fprintf(outFile, "Source Language: %s\n", manifest.Metadata.SourceLanguage)
		}
		if manifest.Metadata.TargetLanguage != "" {
			fmt.Fprintf(outFile, "Target Language: %s\n", manifest.Metadata.TargetLanguage)
		}
		if manifest.Metadata.Notes != "" {
			fmt.Fprintf(outFile, "Notes:           %s\n", manifest.Metadata.Notes)
		}
		fmt.Fprintf(outFile, "\n")
	}

	// Print entries grouped by slide
	currentSlide := -1
	for _, entry := range manifest.Entries {
		if entry.SlideID != currentSlide {
			if currentSlide >= 0 {
				fmt.Fprintf(outFile, "\n")
			}
			currentSlide = entry.SlideID
			fmt.Fprintf(outFile, "Slide %d\n", entry.SlideNumber)
			fmt.Fprintf(outFile, "-------\n")
		}

		fmt.Fprintf(outFile, "  ID: %s\n", entry.ID)
		fmt.Fprintf(outFile, "  Type: %s\n", entry.Type)
		fmt.Fprintf(outFile, "  Key: %s\n", entry.PlaceholderKey)
		fmt.Fprintf(outFile, "  Text: %s\n", entry.SourceText)
		if entry.TargetText != "" {
			fmt.Fprintf(outFile, "  Target: %s\n", entry.TargetText)
		}
		if entry.BulletInfo != nil && entry.BulletInfo.Level != nil {
			fmt.Fprintf(outFile, "  Level: %d\n", *entry.BulletInfo.Level)
		}
		fmt.Fprintf(outFile, "\n")
	}

	return nil
}

// init registers the translate export command
func init() {
	translateExportCmd.Flags().IntSliceVar(&translateExportSlides, "slide", []int{}, "Slide number to export (1-indexed). Can be used multiple times.")
	translateExportCmd.Flags().BoolVar(&translateExportIncludeNotes, "include-notes", false, "Include speaker notes in export")
	translateExportCmd.Flags().StringVar(&translateExportSourceLang, "source-lang", "", "Source language code (e.g., en-US)")
	translateExportCmd.Flags().StringVar(&translateExportTargetLang, "target-lang", "", "Target language code (e.g., fr-FR)")

	translateCmd.AddCommand(translateExportCmd)
}
