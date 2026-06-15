package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

var (
	docxBlocksInsertAfterBlock    int
	docxBlocksInsertAfterHash     string
	docxBlocksInsertAfterText     string
	docxBlocksInsertAfterTextFile string
	docxBlocksInsertAfterStyle    string
)

var docxBlocksInsertAfterCmd = &cobra.Command{
	Use:   "insert-after <file>",
	Short: "Insert a paragraph after a hash-guarded DOCX body block",
	Long:  "Insert a main-document paragraph after a body block guarded by the content hash reported by docx blocks. Use --block 0 to insert before the first block.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxBlocksInsertAfterBlock < 0 {
			return InvalidArgsError("--block must be >= 0")
		}
		if docxBlocksInsertAfterBlock > 0 {
			if err := requireDOCXBlockHash(docxBlocksInsertAfterHash); err != nil {
				return err
			}
		} else if cmd.Flags().Lookup("expect-hash").Changed {
			return InvalidArgsError("--expect-hash cannot be used with --block 0")
		}
		text, err := resolveOptionalDOCXParagraphText(cmd, "text", "text-file", docxBlocksInsertAfterText, docxBlocksInsertAfterTextFile)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXBlocksInsertAfter(filePath, docxBlocksInsertAfterBlock, docxBlocksInsertAfterHash, text, docxBlocksInsertAfterStyle, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXBlockParagraphJSON(cmd, result, "blocks insert-after")
		}
		return outputDOCXBlockParagraphText(cmd, "inserted", result)
	},
}

func performDOCXBlocksInsertAfter(filePath string, block int, expectedHash, text, style string, mutOpts *MutationOptions) (*DOCXBlockParagraphResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXBlockParagraphResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		insertResult, err := docxmutate.InsertParagraphAfterBlock(&docxmutate.InsertParagraphAfterBlockRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			AfterIndex:   block,
			ExpectedHash: expectedHash,
			Text:         text,
			Style:        style,
		})
		if err != nil {
			return mapDOCXBlockMutationError("block", err)
		}
		result = &DOCXBlockParagraphResult{
			File:        filePath,
			Index:       insertResult.Index,
			BlockID:     fmt.Sprintf("body.b%d", insertResult.Index),
			ContentHash: insertResult.ContentHash,
			AnchorHash:  insertResult.AnchorHash,
			InsertAfter: insertResult.InsertAfter,
			Style:       insertResult.Style,
			Text:        insertResult.Text,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxBlocksInsertAfterCmd.Flags().IntVar(&docxBlocksInsertAfterBlock, "block", 0, "1-based body block index from docx blocks; 0 inserts before the first block")
	docxBlocksInsertAfterCmd.Flags().StringVar(&docxBlocksInsertAfterHash, "expect-hash", "", "expected sha256: content hash from docx blocks when --block is greater than 0")
	docxBlocksInsertAfterCmd.Flags().StringVar(&docxBlocksInsertAfterText, "text", "", "paragraph text; omitted or empty creates a blank paragraph")
	docxBlocksInsertAfterCmd.Flags().StringVar(&docxBlocksInsertAfterTextFile, "text-file", "", "path to paragraph text")
	docxBlocksInsertAfterCmd.Flags().StringVar(&docxBlocksInsertAfterStyle, "style", "", "optional paragraph style ID to apply")
	AddMutationFlags(docxBlocksInsertAfterCmd)
	docxBlocksCmd.AddCommand(docxBlocksInsertAfterCmd)
}
