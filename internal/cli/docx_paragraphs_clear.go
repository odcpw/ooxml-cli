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

type DOCXParagraphsClearResult struct {
	File         string `json:"file"`
	Index        int    `json:"index"`
	Style        string `json:"style,omitempty"`
	PreviousText string `json:"previousText"`
	Handle       string `json:"handle,omitempty"`
}

var (
	docxParagraphsClearIndex  int
	docxParagraphsClearHandle string
)

var docxParagraphsClearCmd = &cobra.Command{
	Use:   "clear <file>",
	Short: "Clear DOCX paragraph text",
	Long:  "Clear one main-body paragraph's text by the block index reported by docx text JSON.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		handleSet := cmd.Flags().Lookup("handle").Changed
		if !handleSet && docxParagraphsClearIndex < 1 {
			return InvalidArgsError("--index must be >= 1 (or pass --handle)")
		}
		if handleSet && cmd.Flags().Lookup("index").Changed {
			return InvalidArgsError("cannot specify both --index and --handle")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		handleArg := ""
		if handleSet {
			handleArg = docxParagraphsClearHandle
		}
		result, err := performDOCXParagraphsClear(filePath, docxParagraphsClearIndex, handleArg, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXParagraphsClearJSON(cmd, result)
		}
		return outputDOCXParagraphsClearText(cmd, result)
	},
}

func performDOCXParagraphsClear(filePath string, index int, handleArg string, mutOpts *MutationOptions) (*DOCXParagraphsClearResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXParagraphsClearResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		targetIndex := index
		if handleArg != "" {
			resolved, herr := resolveDOCXParagraphHandleBlock(pkg, handleArg)
			if herr != nil {
				return herr
			}
			targetIndex = resolved
		}
		clearResult, err := docxmutate.ClearParagraphText(&docxmutate.ClearParagraphTextRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			Index:       targetIndex,
		})
		if err != nil {
			return mapDOCXParagraphMutationError(targetIndex, err)
		}
		result = &DOCXParagraphsClearResult{
			File:         filePath,
			Index:        clearResult.Index,
			Style:        clearResult.Style,
			PreviousText: clearResult.PreviousText,
			Handle:       docxParagraphHandleString(clearResult.ParaID),
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputDOCXParagraphsClearJSON(cmd *cobra.Command, result *DOCXParagraphsClearResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal paragraphs clear JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXParagraphsClearText(cmd *cobra.Command, result *DOCXParagraphsClearResult) error {
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("cleared paragraph %d", result.Index)))
}

func init() {
	docxParagraphsClearCmd.Flags().IntVar(&docxParagraphsClearIndex, "index", 0, "1-based block index from docx text JSON")
	docxParagraphsClearCmd.Flags().StringVar(&docxParagraphsClearHandle, "handle", "", "stable paragraph handle (H:docx/pt:doc/para:m:<paraId>); authoritative for the target, ignores --index")
	AddMutationFlags(docxParagraphsClearCmd)
	docxParagraphsCmd.AddCommand(docxParagraphsClearCmd)
}
