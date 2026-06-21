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

// DOCXImagesInsertResult is the JSON readback shape for docx images insert.
type DOCXImagesInsertResult struct {
	File           string `json:"file"`
	Index          int    `json:"index"`
	ID             string `json:"id"`
	InsertAfter    int    `json:"insertAfter"`
	AnchorHash     string `json:"anchorHash,omitempty"`
	MediaURI       string `json:"mediaUri"`
	NewContentType string `json:"newContentType"`
	Width          int64  `json:"width"`
	Height         int64  `json:"height"`
}

var (
	docxImagesInsertAfter  int
	docxImagesInsertFile   string
	docxImagesInsertHash   string
	docxImagesInsertWidth  int64
	docxImagesInsertHeight int64
)

var docxImagesInsertCmd = &cobra.Command{
	Use:   "insert <file>",
	Short: "Insert a new inline image after a body block",
	Long:  "Create a media part, a document relationship, and a body paragraph containing a w:drawing inline run with the given EMU extents. Use --after 0 to insert before the first block.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if docxImagesInsertAfter < 0 {
			return InvalidArgsError("--after must be >= 0")
		}
		if docxImagesInsertFile == "" {
			return InvalidArgsError("--file is required")
		}
		if _, err := os.Stat(docxImagesInsertFile); err != nil {
			return FileNotFoundError(docxImagesInsertFile)
		}
		if docxImagesInsertWidth <= 0 || docxImagesInsertHeight <= 0 {
			return InvalidArgsError("--width and --height are required and must be > 0 (EMU)")
		}
		if docxImagesInsertAfter > 0 {
			if err := requireDOCXBlockHash(docxImagesInsertHash); err != nil {
				return err
			}
		} else if cmd.Flags().Lookup("expect-hash").Changed {
			return InvalidArgsError("--expect-hash cannot be used with --after 0")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		imageData, err := os.ReadFile(docxImagesInsertFile)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read image file: %v", err)
		}
		contentType, err := docxImageContentType(docxImagesInsertFile)
		if err != nil {
			return err
		}

		result, err := performDOCXImagesInsert(filePath, imageData, contentType, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXImagesInsertJSON(cmd, result)
		}
		return outputDOCXImagesInsertText(cmd, result)
	},
}

func performDOCXImagesInsert(filePath string, imageData []byte, contentType string, mutOpts *MutationOptions) (*DOCXImagesInsertResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXImagesInsertResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		insertResult, err := docxmutate.InsertImage(&docxmutate.InsertImageRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			AfterIndex:   docxImagesInsertAfter,
			ExpectedHash: docxImagesInsertHash,
			ImageData:    imageData,
			ContentType:  contentType,
			Width:        docxImagesInsertWidth,
			Height:       docxImagesInsertHeight,
		})
		if err != nil {
			return mapDOCXImageMutationError(err)
		}
		result = &DOCXImagesInsertResult{
			File:           filePath,
			Index:          insertResult.Index,
			ID:             insertResult.ID,
			InsertAfter:    insertResult.InsertAfter,
			AnchorHash:     insertResult.AnchorHash,
			MediaURI:       insertResult.MediaURI,
			NewContentType: insertResult.NewContentType,
			Width:          insertResult.Width,
			Height:         insertResult.Height,
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func outputDOCXImagesInsertJSON(cmd *cobra.Command, result *DOCXImagesInsertResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal images insert JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXImagesInsertText(cmd *cobra.Command, result *DOCXImagesInsertResult) error {
	text := fmt.Sprintf("inserted image at block %d: %s (%dx%d)", result.Index, result.MediaURI, result.Width, result.Height)
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	docxImagesInsertCmd.Flags().IntVar(&docxImagesInsertAfter, "after", 0, "1-based body block index to insert after; 0 inserts before the first block")
	docxImagesInsertCmd.Flags().StringVar(&docxImagesInsertFile, "file", "", "path to the image file")
	docxImagesInsertCmd.Flags().StringVar(&docxImagesInsertHash, "expect-hash", "", "expected sha256: content hash of the anchor block when --after is greater than 0")
	docxImagesInsertCmd.Flags().Int64Var(&docxImagesInsertWidth, "width", 0, "image width in EMUs (required)")
	docxImagesInsertCmd.Flags().Int64Var(&docxImagesInsertHeight, "height", 0, "image height in EMUs (required)")
	AddMutationFlags(docxImagesInsertCmd)
	docxImagesCmd.AddCommand(docxImagesInsertCmd)
}
