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
	docxBlocksDeleteBlock int
	docxBlocksDeleteHash  string
)

var docxBlocksDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete a hash-guarded DOCX body block",
	Long:  "Delete one main-document paragraph or table block, guarded by the content hash reported by docx blocks.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxBlocksDeleteBlock < 1 {
			return InvalidArgsError("--block must be >= 1")
		}
		if err := requireDOCXBlockHash(docxBlocksDeleteHash); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXBlocksDelete(filePath, docxBlocksDeleteBlock, docxBlocksDeleteHash, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXBlockDeleteJSON(cmd, result)
		}
		return outputDOCXBlockDeleteText(cmd, result)
	},
}

func performDOCXBlocksDelete(filePath string, block int, expectedHash string, mutOpts *MutationOptions) (*DOCXBlockDeleteResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXBlockDeleteResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		deleteResult, err := docxmutate.DeleteBlock(&docxmutate.DeleteBlockRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			Index:        block,
			ExpectedHash: expectedHash,
		})
		if err != nil {
			return mapDOCXBlockMutationError("block", err)
		}
		result = &DOCXBlockDeleteResult{
			File:         filePath,
			Index:        deleteResult.Index,
			BlockID:      fmt.Sprintf("body.b%d", deleteResult.Index),
			PreviousKind: string(deleteResult.PreviousKind),
			PreviousHash: deleteResult.PreviousHash,
			PreviousText: deleteResult.PreviousText,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxBlocksDeleteCmd.Flags().IntVar(&docxBlocksDeleteBlock, "block", 0, "1-based body block index from docx blocks")
	docxBlocksDeleteCmd.Flags().StringVar(&docxBlocksDeleteHash, "expect-hash", "", "expected sha256: content hash from docx blocks")
	AddMutationFlags(docxBlocksDeleteCmd)
	docxBlocksCmd.AddCommand(docxBlocksDeleteCmd)
}
