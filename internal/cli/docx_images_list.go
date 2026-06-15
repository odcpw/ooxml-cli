package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXImagesListResult is the JSON readback shape for docx images list.
type DOCXImagesListResult struct {
	File            string                `json:"file"`
	DocumentPartURI string                `json:"documentPartUri"`
	Images          []extract.ImageReport `json:"images"`
}

var docxImagesListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List inline images in a DOCX document",
	Long:  "Resolve inline images to media parts with index, relationship id, target, content type, and EMU extent.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		result, err := performDOCXImagesList(filePath)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXImagesListJSON(cmd, result)
		}
		return outputDOCXImagesListText(cmd, result)
	},
}

func performDOCXImagesList(filePath string) (*DOCXImagesListResult, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}
	defer pkg.Close()

	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
	}

	extracted, err := extract.ExtractImages(&extract.ExtractImagesRequest{
		Session:     pkg,
		DocumentURI: documentURI,
	})
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to extract DOCX images: %v", err)
	}
	return &DOCXImagesListResult{
		File:            filePath,
		DocumentPartURI: extracted.DocumentPartURI,
		Images:          extracted.Images,
	}, nil
}

func outputDOCXImagesListJSON(cmd *cobra.Command, result *DOCXImagesListResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal DOCX images JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXImagesListText(cmd *cobra.Command, result *DOCXImagesListResult) error {
	var builder strings.Builder
	for _, image := range result.Images {
		builder.WriteString(fmt.Sprintf("image %d: %s (%dx%d)\n", image.Index, image.MediaURI, image.Width, image.Height))
	}
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func init() {
	docxImagesCmd.AddCommand(docxImagesListCmd)
}
