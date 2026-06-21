package cli

import (
	"encoding/json"
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXParagraphsInsertResult struct {
	File        string `json:"file"`
	Index       int    `json:"index"`
	InsertAfter int    `json:"insertAfter"`
	Style       string `json:"style,omitempty"`
	Text        string `json:"text"`
}

var (
	docxParagraphsInsertAfter    int
	docxParagraphsInsertText     string
	docxParagraphsInsertTextFile string
	docxParagraphsInsertStyle    string
)

var docxParagraphsInsertCmd = &cobra.Command{
	Use:   "insert <file>",
	Short: "Insert a DOCX body paragraph",
	Long:  "Insert a main document body paragraph after a block index from docx text JSON. Use --insert-after 0 to prepend.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxParagraphsInsertAfter < 0 {
			return InvalidArgsError("--insert-after must be >= 0")
		}
		text, err := resolveOptionalDOCXParagraphText(cmd, "text", "text-file", docxParagraphsInsertText, docxParagraphsInsertTextFile)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXParagraphsInsert(filePath, docxParagraphsInsertAfter, text, docxParagraphsInsertStyle, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXParagraphsInsertJSON(cmd, result)
		}
		return outputDOCXParagraphsInsertText(cmd, result)
	},
}

func performDOCXParagraphsInsert(filePath string, insertAfter int, text, style string, mutOpts *MutationOptions) (*DOCXParagraphsInsertResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXParagraphsInsertResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		insertResult, err := docxmutate.InsertParagraph(&docxmutate.InsertParagraphRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			AfterIndex:  insertAfter,
			Text:        text,
			Style:       style,
		})
		if err != nil {
			return mapDOCXParagraphStructuralMutationError(fmt.Sprintf("block index %d", insertAfter), err)
		}
		result = &DOCXParagraphsInsertResult{
			File:        filePath,
			Index:       insertResult.Index,
			InsertAfter: insertAfter,
			Style:       insertResult.Style,
			Text:        insertResult.Text,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputDOCXParagraphsInsertJSON(cmd *cobra.Command, result *DOCXParagraphsInsertResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal paragraphs insert JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXParagraphsInsertText(cmd *cobra.Command, result *DOCXParagraphsInsertResult) error {
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("inserted paragraph %d", result.Index)))
}

func init() {
	docxParagraphsInsertCmd.Flags().IntVar(&docxParagraphsInsertAfter, "insert-after", 0, "0 to prepend, or a 1-based block index from docx text JSON")
	docxParagraphsInsertCmd.Flags().StringVar(&docxParagraphsInsertText, "text", "", "paragraph text; omitted or empty creates a blank paragraph")
	docxParagraphsInsertCmd.Flags().StringVar(&docxParagraphsInsertTextFile, "text-file", "", "path to paragraph text; empty files create blank paragraphs")
	docxParagraphsInsertCmd.Flags().StringVar(&docxParagraphsInsertStyle, "style", "", "optional paragraph style ID to apply")
	AddMutationFlags(docxParagraphsInsertCmd)
	docxParagraphsCmd.AddCommand(docxParagraphsInsertCmd)
}
