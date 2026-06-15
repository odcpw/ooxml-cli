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

// DOCXImagesReplaceResult is the JSON readback shape for docx images replace.
type DOCXImagesReplaceResult struct {
	File           string `json:"file"`
	Index          int    `json:"index"`
	ID             string `json:"id"`
	BlockIndex     int    `json:"blockIndex"`
	BlockID        string `json:"blockId"`
	BlockHash      string `json:"blockHash"`
	PreviousURI    string `json:"previousUri"`
	PreviousType   string `json:"previousContentType"`
	NewURI         string `json:"newUri"`
	NewContentType string `json:"newContentType"`
	Width          int64  `json:"width"`
	Height         int64  `json:"height"`
}

var (
	docxImagesReplaceImage  string
	docxImagesReplaceFile   string
	docxImagesReplaceHash   string
	docxImagesReplaceWidth  int64
	docxImagesReplaceHeight int64
)

var docxImagesReplaceCmd = &cobra.Command{
	Use:   "replace <file>",
	Short: "Replace an inline image's bytes and optionally its EMU extent",
	Long:  "Swap the media bytes of an inline image selected by 1-based index or relationship id, optionally resizing the wp:extent (EMU). Guarded by the containing body block's content hash.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxImagesReplaceImage == "" {
			return InvalidArgsError("--image is required (1-based index or relationship id)")
		}
		if docxImagesReplaceFile == "" {
			return InvalidArgsError("--file is required")
		}
		if _, err := os.Stat(docxImagesReplaceFile); err != nil {
			return FileNotFoundError(docxImagesReplaceFile)
		}
		if docxImagesReplaceWidth < 0 || docxImagesReplaceHeight < 0 {
			return InvalidArgsError("--width and --height must be >= 0 (EMU)")
		}
		if err := requireDOCXImageHashFormat(docxImagesReplaceHash); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		imageData, err := os.ReadFile(docxImagesReplaceFile)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read image file: %v", err)
		}
		contentType, err := docxImageContentType(docxImagesReplaceFile)
		if err != nil {
			return err
		}

		result, err := performDOCXImagesReplace(filePath, imageData, contentType, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXImagesReplaceJSON(cmd, result)
		}
		return outputDOCXImagesReplaceText(cmd, result)
	},
}

func performDOCXImagesReplace(filePath string, imageData []byte, contentType string, mutOpts *MutationOptions) (*DOCXImagesReplaceResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXImagesReplaceResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		replaceResult, err := docxmutate.ReplaceImage(&docxmutate.ReplaceImageRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			Selector:     docxImagesReplaceImage,
			ExpectedHash: docxImagesReplaceHash,
			ImageData:    imageData,
			ContentType:  contentType,
			Width:        docxImagesReplaceWidth,
			Height:       docxImagesReplaceHeight,
		})
		if err != nil {
			return mapDOCXImageMutationError(err)
		}
		result = &DOCXImagesReplaceResult{
			File:           filePath,
			Index:          replaceResult.Index,
			ID:             replaceResult.ID,
			BlockIndex:     replaceResult.BlockIndex,
			BlockID:        replaceResult.BlockID,
			BlockHash:      replaceResult.BlockHash,
			PreviousURI:    replaceResult.PreviousURI,
			PreviousType:   replaceResult.PreviousType,
			NewURI:         replaceResult.NewURI,
			NewContentType: replaceResult.NewContentType,
			Width:          replaceResult.Width,
			Height:         replaceResult.Height,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func mapDOCXImageMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrImageNotFound):
		return TargetNotFoundError("image")
	case errors.Is(err, docxmutate.ErrBlockIndexOutOfRange):
		return TargetNotFoundError("block")
	case errors.Is(err, docxmutate.ErrBlockHashMismatch):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate image: %v", err)
	}
}

func outputDOCXImagesReplaceJSON(cmd *cobra.Command, result *DOCXImagesReplaceResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal images replace JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXImagesReplaceText(cmd *cobra.Command, result *DOCXImagesReplaceResult) error {
	text := fmt.Sprintf("replaced image %d: %s -> %s (%dx%d)", result.Index, result.PreviousURI, result.NewURI, result.Width, result.Height)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	docxImagesReplaceCmd.Flags().StringVar(&docxImagesReplaceImage, "image", "", "1-based image index or relationship id from docx images list")
	docxImagesReplaceCmd.Flags().StringVar(&docxImagesReplaceFile, "file", "", "path to the replacement image file")
	docxImagesReplaceCmd.Flags().StringVar(&docxImagesReplaceHash, "expect-hash", "", "optional sha256: content hash of the body block containing the image")
	docxImagesReplaceCmd.Flags().Int64Var(&docxImagesReplaceWidth, "width", 0, "new width in EMUs (0 leaves the extent unchanged)")
	docxImagesReplaceCmd.Flags().Int64Var(&docxImagesReplaceHeight, "height", 0, "new height in EMUs (0 leaves the extent unchanged)")
	AddMutationFlags(docxImagesReplaceCmd)
	docxImagesCmd.AddCommand(docxImagesReplaceCmd)
}
