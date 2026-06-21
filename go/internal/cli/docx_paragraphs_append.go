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

type DOCXParagraphsAppendResult struct {
	File  string `json:"file"`
	Index int    `json:"index"`
	Style string `json:"style,omitempty"`
	Text  string `json:"text"`
}

var (
	docxParagraphsAppendText     string
	docxParagraphsAppendTextFile string
	docxParagraphsAppendStyle    string
)

var docxParagraphsAppendCmd = &cobra.Command{
	Use:   "append <file>",
	Short: "Append a DOCX body paragraph",
	Long:  "Append a main document body paragraph, preserving trailing section properties.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		text, err := resolveOptionalDOCXParagraphText(cmd, "text", "text-file", docxParagraphsAppendText, docxParagraphsAppendTextFile)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXParagraphsAppend(filePath, text, docxParagraphsAppendStyle, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXParagraphsAppendJSON(cmd, result)
		}
		return outputDOCXParagraphsAppendText(cmd, result)
	},
}

func performDOCXParagraphsAppend(filePath, text, style string, mutOpts *MutationOptions) (*DOCXParagraphsAppendResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXParagraphsAppendResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		appendResult, err := docxmutate.AppendParagraph(&docxmutate.AppendParagraphRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			Text:        text,
			Style:       style,
		})
		if err != nil {
			return mapDOCXParagraphStructuralMutationError("append paragraph", err)
		}
		result = &DOCXParagraphsAppendResult{
			File:  filePath,
			Index: appendResult.Index,
			Style: appendResult.Style,
			Text:  appendResult.Text,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func resolveOptionalDOCXParagraphText(cmd *cobra.Command, textFlag, textFileFlag, textValue, textFileValue string) (string, error) {
	textChanged := cmd.Flags().Lookup(textFlag).Changed
	textFileChanged := cmd.Flags().Lookup(textFileFlag).Changed
	if textChanged && textFileChanged {
		return "", InvalidArgsError("cannot specify both --text and --text-file")
	}
	if textChanged {
		return textValue, nil
	}
	if textFileChanged {
		data, err := os.ReadFile(textFileValue)
		if err != nil {
			return "", FileNotFoundError(textFileValue)
		}
		return string(data), nil
	}
	return "", nil
}

func mapDOCXParagraphStructuralMutationError(target string, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrBlockIndexOutOfRange):
		return TargetNotFoundError(target)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate paragraph: %v", err)
	}
}

func outputDOCXParagraphsAppendJSON(cmd *cobra.Command, result *DOCXParagraphsAppendResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal paragraphs append JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXParagraphsAppendText(cmd *cobra.Command, result *DOCXParagraphsAppendResult) error {
	return writeCLIOutput(cmd, []byte(fmt.Sprintf("appended paragraph %d", result.Index)))
}

func init() {
	docxParagraphsAppendCmd.Flags().StringVar(&docxParagraphsAppendText, "text", "", "paragraph text; omitted or empty creates a blank paragraph")
	docxParagraphsAppendCmd.Flags().StringVar(&docxParagraphsAppendTextFile, "text-file", "", "path to paragraph text; empty files create blank paragraphs")
	docxParagraphsAppendCmd.Flags().StringVar(&docxParagraphsAppendStyle, "style", "", "optional paragraph style ID to apply")
	AddMutationFlags(docxParagraphsAppendCmd)
	docxParagraphsCmd.AddCommand(docxParagraphsAppendCmd)
}
