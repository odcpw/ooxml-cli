package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXTextResult struct {
	File   string        `json:"file"`
	Blocks []model.Block `json:"blocks"`
}

var docxTextCmd = &cobra.Command{
	Use:   "text <file>",
	Short: "Extract main body text from a DOCX document",
	Long:  "Extract paragraphs and flattened table text from the DOCX main document body.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypeDOCX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}

		result, err := extract.ExtractText(&extract.ExtractTextRequest{
			Session:     pkg,
			DocumentURI: documentURI,
		})
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to extract DOCX text: %v", err)
		}
		result.File = filePath

		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTextJSON(cmd, result)
		}
		return outputDOCXTextText(cmd, result)
	},
}

func outputDOCXTextJSON(cmd *cobra.Command, extracted *extract.ExtractedDocument) error {
	config := GetGlobalConfig(cmd)
	result := DOCXTextResult{
		File:   extracted.File,
		Blocks: extracted.Blocks,
	}

	var data []byte
	var err error
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal DOCX text JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXTextText(cmd *cobra.Command, extracted *extract.ExtractedDocument) error {
	var builder strings.Builder
	for i, block := range extracted.Blocks {
		if i > 0 {
			builder.WriteString("\n\n")
		}
		if block.Kind == model.BlockKindTable && block.Table != nil {
			for rowIndex, row := range block.Table.Rows {
				if rowIndex > 0 {
					builder.WriteByte('\n')
				}
				builder.WriteString(strings.Join(row.Cells, "\t"))
			}
			continue
		}
		if block.Style != "" {
			builder.WriteString(fmt.Sprintf("[%s] ", block.Style))
		}
		builder.WriteString(block.Text)
	}
	builder.WriteByte('\n')
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func init() {
	docxCmd.AddCommand(docxTextCmd)
}
