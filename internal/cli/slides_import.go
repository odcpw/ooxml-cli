package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

var (
	importSourceFile   string
	importSlideNumber  int
	importLayoutPolicy string
	importThemePolicy  string
	importNotesPolicy  string
	importInsertAfter  int
)

var importSlideCmd = &cobra.Command{
	Use:   "import-slide <target-file>",
	Short: "Import a slide from an external presentation",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		targetPath := args[0]
		if _, err := os.Stat(targetPath); err != nil {
			return FileNotFoundError(targetPath)
		}
		if importSourceFile == "" {
			return InvalidArgsError("--source is required")
		}
		if _, err := os.Stat(importSourceFile); err != nil {
			return FileNotFoundError(importSourceFile)
		}
		if importSlideNumber < 1 {
			return InvalidArgsError("--slide must be >= 1")
		}
		if importLayoutPolicy == "" {
			importLayoutPolicy = "reuse"
		}
		if importThemePolicy == "" {
			importThemePolicy = "reuse"
		}
		if importNotesPolicy == "" {
			importNotesPolicy = "drop"
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performImportSlide(targetPath, mutOpts)
		if err != nil {
			return err
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputImportSlideJSON(cmd, result)
		}
		return outputImportSlideText(cmd, result)
	},
}

type importSlideResult struct {
	NewSlideNumber int    `json:"newSlideNumber"`
	NewSlideID     uint32 `json:"newSlideId"`
	NewSlideURI    string `json:"newSlideUri"`
	NotesURI       string `json:"notesUri,omitempty"`
}

func performImportSlide(targetPath string, mutOpts *MutationOptions) (*importSlideResult, error) {
	// Open source presentation
	sourcePkg, err := openPackageExpectType(importSourceFile, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	defer sourcePkg.Close()

	// Translate policy strings to internal types
	notesPolicy := mutate.NotesDrop
	if importNotesPolicy == "clone" {
		notesPolicy = mutate.NotesClone
	}

	var result *importSlideResult
	writer, err := NewMutationWriter(targetPath, mutOpts)
	if err != nil {
		return nil, err
	}

	if err := writer.Write(func(pkg opc.PackageSession) error {
		importResult, err := mutate.ImportSlide(&mutate.ImportSlideRequest{
			TargetPackage:     pkg,
			SourcePackage:     sourcePkg,
			SourceSlideNumber: importSlideNumber,
			InsertAfter:       importInsertAfter,
			LayoutPolicy:      importLayoutPolicy,
			ThemePolicy:       importThemePolicy,
			NotesPolicy:       notesPolicy,
		})
		if err != nil {
			return err
		}
		result = &importSlideResult{
			NewSlideNumber: importResult.NewSlideNumber,
			NewSlideID:     importResult.NewSlideID,
			NewSlideURI:    importResult.NewSlideURI,
			NotesURI:       importResult.NotesURI,
		}
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to import slide")
	}

	return result, nil
}

func outputImportSlideJSON(cmd *cobra.Command, result *importSlideResult) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal import-slide JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputImportSlideText(cmd *cobra.Command, result *importSlideResult) error {
	text := fmt.Sprintf("Imported slide to position %d (ID: %d)\n", result.NewSlideNumber, result.NewSlideID)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	importSlideCmd.Flags().StringVar(&importSourceFile, "source", "", "path to the source presentation file")
	importSlideCmd.Flags().IntVar(&importSlideNumber, "slide", 0, "1-based slide number in source presentation")
	importSlideCmd.Flags().IntVar(&importInsertAfter, "insert-after", 0, "insert after this 1-based position in target (default: at end)")
	importSlideCmd.Flags().StringVar(&importLayoutPolicy, "layout-policy", "reuse", "layout handling: 'reuse' or 'import'")
	importSlideCmd.Flags().StringVar(&importThemePolicy, "theme-policy", "reuse", "theme handling: 'reuse' or 'import'")
	importSlideCmd.Flags().StringVar(&importNotesPolicy, "notes-policy", "drop", "notes handling: 'drop' or 'clone'")
	importSlideCmd.MarkFlagRequired("source")
	importSlideCmd.MarkFlagRequired("slide")
	AddMutationFlags(importSlideCmd)
	slidesCmd.AddCommand(importSlideCmd)
}
