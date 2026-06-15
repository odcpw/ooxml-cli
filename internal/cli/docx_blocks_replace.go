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
	docxBlocksReplaceBlock    int
	docxBlocksReplaceHash     string
	docxBlocksReplaceText     string
	docxBlocksReplaceTextFile string
	docxBlocksReplaceStyle    string
)

var docxBlocksReplaceCmd = &cobra.Command{
	Use:   "replace <file>",
	Short: "Replace a hash-guarded DOCX body block with a paragraph",
	Long:  "Replace one main-document body block with a paragraph, guarded by the content hash reported by docx blocks.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxBlocksReplaceBlock < 1 {
			return InvalidArgsError("--block must be >= 1")
		}
		if err := requireDOCXBlockHash(docxBlocksReplaceHash); err != nil {
			return err
		}
		text, err := resolveOptionalDOCXParagraphText(cmd, "text", "text-file", docxBlocksReplaceText, docxBlocksReplaceTextFile)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXBlocksReplace(filePath, docxBlocksReplaceBlock, docxBlocksReplaceHash, text, docxBlocksReplaceStyle, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXBlockParagraphJSON(cmd, result, "blocks replace")
		}
		return outputDOCXBlockParagraphText(cmd, "replaced", result)
	},
}

func performDOCXBlocksReplace(filePath string, block int, expectedHash, text, style string, mutOpts *MutationOptions) (*DOCXBlockParagraphResult, error) {
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
		replaceResult, err := docxmutate.ReplaceBlockWithParagraph(&docxmutate.ReplaceBlockWithParagraphRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			Index:        block,
			ExpectedHash: expectedHash,
			Text:         text,
			Style:        style,
		})
		if err != nil {
			return mapDOCXBlockMutationError("block", err)
		}
		result = &DOCXBlockParagraphResult{
			File:         filePath,
			Index:        replaceResult.Index,
			BlockID:      fmt.Sprintf("body.b%d", replaceResult.Index),
			ContentHash:  replaceResult.ContentHash,
			PreviousKind: string(replaceResult.PreviousKind),
			PreviousHash: replaceResult.PreviousHash,
			PreviousText: replaceResult.PreviousText,
			Style:        replaceResult.Style,
			Text:         replaceResult.Text,
			Destination:  collectDOCXBlockDestination(pkg, replaceResult.Index),
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxBlocksReplaceCmd.Flags().IntVar(&docxBlocksReplaceBlock, "block", 0, "1-based body block index from docx blocks")
	docxBlocksReplaceCmd.Flags().StringVar(&docxBlocksReplaceHash, "expect-hash", "", "expected sha256: content hash from docx blocks")
	docxBlocksReplaceCmd.Flags().StringVar(&docxBlocksReplaceText, "text", "", "replacement paragraph text; omitted or empty creates a blank paragraph")
	docxBlocksReplaceCmd.Flags().StringVar(&docxBlocksReplaceTextFile, "text-file", "", "path to replacement paragraph text")
	docxBlocksReplaceCmd.Flags().StringVar(&docxBlocksReplaceStyle, "style", "", "optional paragraph style ID; default preserves paragraph style when replacing a paragraph")
	AddMutationFlags(docxBlocksReplaceCmd)
	docxBlocksCmd.AddCommand(docxBlocksReplaceCmd)
}
