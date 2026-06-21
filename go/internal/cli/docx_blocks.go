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

type DOCXBlocksResult struct {
	File            string                `json:"file"`
	DocumentPartURI string                `json:"documentPartUri"`
	Blocks          []extract.BlockReport `json:"blocks"`
}

var (
	docxBlocksBlock       int
	docxBlocksIncludeRuns bool
)

var docxBlocksCmd = &cobra.Command{
	Use:   "blocks <file>",
	Short: "Show stable DOCX body blocks",
	Long:  "Show main-document body blocks with stable IDs, content hashes, paragraph metadata, table cells, and optional run details.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxBlocksBlock < 0 {
			return InvalidArgsError("--block must be >= 0")
		}

		result, err := performDOCXBlocksShow(filePath, docxBlocksBlock, docxBlocksIncludeRuns)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXBlocksJSON(cmd, result)
		}
		return outputDOCXBlocksText(cmd, result)
	},
}

func performDOCXBlocksShow(filePath string, block int, includeRuns bool) (*DOCXBlocksResult, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}
	defer pkg.Close()

	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
	}

	result, err := extract.ExtractBlocks(&extract.ExtractBlocksRequest{
		Session:     pkg,
		DocumentURI: documentURI,
		Block:       block,
		IncludeRuns: includeRuns,
	})
	if err != nil {
		if block > 0 && strings.Contains(err.Error(), "not found") {
			return nil, TargetNotFoundError(fmt.Sprintf("block %d", block))
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to extract DOCX blocks: %v", err)
	}
	return &DOCXBlocksResult{
		File:            filePath,
		DocumentPartURI: result.DocumentPartURI,
		Blocks:          result.Blocks,
	}, nil
}

func outputDOCXBlocksJSON(cmd *cobra.Command, result *DOCXBlocksResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal DOCX blocks JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXBlocksText(cmd *cobra.Command, result *DOCXBlocksResult) error {
	var builder strings.Builder
	for _, block := range result.Blocks {
		if builder.Len() > 0 {
			builder.WriteByte('\n')
		}
		label := string(block.Kind)
		if block.Kind == model.BlockKindParagraph && block.Paragraph != nil && block.Paragraph.Style != "" {
			label += ":" + block.Paragraph.Style
		}
		builder.WriteString(fmt.Sprintf("%s [%d] %s %s", block.ID, block.Index, label, block.ContentHash))
		if block.Text != "" {
			builder.WriteString("\n")
			builder.WriteString(block.Text)
		}
	}
	builder.WriteByte('\n')
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func init() {
	docxBlocksCmd.Flags().IntVar(&docxBlocksBlock, "block", 0, "1-based body block index to show")
	docxBlocksCmd.Flags().BoolVar(&docxBlocksIncludeRuns, "include-runs", false, "include paragraph run text and basic run properties")
	docxCmd.AddCommand(docxBlocksCmd)
}
