package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXParagraphsSetResult struct {
	File         string `json:"file"`
	Index        int    `json:"index"`
	Style        string `json:"style,omitempty"`
	Text         string `json:"text"`
	PreviousText string `json:"previousText"`
	Flattened    bool   `json:"flattened"`
	Handle       string `json:"handle,omitempty"`
}

var (
	docxParagraphsSetIndex    int
	docxParagraphsSetText     string
	docxParagraphsSetTextFile string
	docxParagraphsSetHandle   string
)

var docxParagraphsSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set DOCX paragraph text",
	Long:  "Set one main-body paragraph's plain text by the block index reported by docx text JSON.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		handleSet := cmd.Flags().Lookup("handle").Changed
		if !handleSet && docxParagraphsSetIndex < 1 {
			return InvalidArgsError("--index must be >= 1 (or pass --handle)")
		}
		if handleSet && cmd.Flags().Lookup("index").Changed {
			return InvalidArgsError("cannot specify both --index and --handle")
		}
		text, err := resolveDOCXParagraphSetText(cmd)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		handleArg := ""
		if handleSet {
			handleArg = docxParagraphsSetHandle
		}
		result, err := performDOCXParagraphsSet(filePath, docxParagraphsSetIndex, handleArg, text, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXParagraphsSetJSON(cmd, result)
		}
		return outputDOCXParagraphsSetText(cmd, result)
	},
}

func resolveDOCXParagraphSetText(cmd *cobra.Command) (string, error) {
	textChanged := cmd.Flags().Lookup("text").Changed
	textFileChanged := cmd.Flags().Lookup("text-file").Changed
	if textChanged == textFileChanged {
		return "", InvalidArgsError("must specify exactly one of --text or --text-file")
	}
	if textChanged {
		if docxParagraphsSetText == "" {
			return "", InvalidArgsError("--text cannot be empty; use docx paragraphs clear")
		}
		return docxParagraphsSetText, nil
	}
	data, err := os.ReadFile(docxParagraphsSetTextFile)
	if err != nil {
		return "", FileNotFoundError(docxParagraphsSetTextFile)
	}
	if len(data) == 0 {
		return "", InvalidArgsError("--text-file cannot be empty; use docx paragraphs clear")
	}
	return string(data), nil
}

func performDOCXParagraphsSet(filePath string, index int, handleArg, text string, mutOpts *MutationOptions) (*DOCXParagraphsSetResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXParagraphsSetResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		// Handle-first: a paragraph handle is authoritative for which paragraph is
		// targeted; --index is ignored when --handle is set.
		targetIndex := index
		if handleArg != "" {
			resolved, herr := resolveDOCXParagraphHandleBlock(pkg, handleArg)
			if herr != nil {
				return herr
			}
			targetIndex = resolved
		}
		setResult, err := docxmutate.SetParagraphText(&docxmutate.SetParagraphTextRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			Index:       targetIndex,
			Text:        text,
		})
		if err != nil {
			return mapDOCXParagraphMutationError(targetIndex, err)
		}
		result = &DOCXParagraphsSetResult{
			File:         filePath,
			Index:        setResult.Index,
			Style:        setResult.Style,
			Text:         setResult.Text,
			PreviousText: setResult.PreviousText,
			Flattened:    setResult.Flattened,
			Handle:       docxParagraphHandleString(setResult.ParaID),
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func mapDOCXParagraphMutationError(index int, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrBlockIndexOutOfRange):
		return TargetNotFoundError(fmt.Sprintf("paragraph index %d", index))
	case errors.Is(err, docxmutate.ErrBlockNotParagraph):
		return NewCLIErrorf(ExitInvalidArgs, "block %d is a table, not a paragraph", index)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate paragraph: %v", err)
	}
}

func outputDOCXParagraphsSetJSON(cmd *cobra.Command, result *DOCXParagraphsSetResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal paragraphs set JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXParagraphsSetText(cmd *cobra.Command, result *DOCXParagraphsSetResult) error {
	text := fmt.Sprintf("set paragraph %d = %q", result.Index, result.Text)
	if result.Flattened {
		text += " (flattened inline content)"
	}
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	docxParagraphsSetCmd.Flags().IntVar(&docxParagraphsSetIndex, "index", 0, "1-based block index from docx text JSON")
	docxParagraphsSetCmd.Flags().StringVar(&docxParagraphsSetHandle, "handle", "", "stable paragraph handle (H:docx/pt:doc/para:m:<paraId>); authoritative for the target, ignores --index")
	docxParagraphsSetCmd.Flags().StringVar(&docxParagraphsSetText, "text", "", "replacement paragraph text")
	docxParagraphsSetCmd.Flags().StringVar(&docxParagraphsSetTextFile, "text-file", "", "path to replacement paragraph text")
	AddMutationFlags(docxParagraphsSetCmd)
	docxParagraphsCmd.AddCommand(docxParagraphsSetCmd)
}
