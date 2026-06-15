package cli

import (
	"regexp"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	"github.com/spf13/cobra"
)

var docxImagesCmd = &cobra.Command{
	Use:   "images",
	Short: "Inspect and mutate inline images in a DOCX document",
	Long:  "List, replace, and insert inline images (w:drawing/wp:inline) in the main document body.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

var docxImageHashPattern = regexp.MustCompile(`^sha256:[0-9a-f]{64}$`)

// requireDOCXImageHashFormat validates an optional --expect-hash value's shape.
func requireDOCXImageHashFormat(value string) error {
	if strings.TrimSpace(value) == "" {
		return nil
	}
	if !docxImageHashPattern.MatchString(value) {
		return InvalidArgsError("--expect-hash must match sha256:<64 lowercase hex chars> from docx blocks")
	}
	return nil
}

// docxImageContentType maps a file extension to a supported image MIME type.
func docxImageContentType(filePath string) (string, error) {
	contentType, ok := imagex.ContentTypeFromPath(filePath)
	if !ok {
		return "", NewCLIErrorf(ExitUnsupportedType, "unsupported image extension %q", filePath)
	}
	return contentType, nil
}

func init() {
	docxCmd.AddCommand(docxImagesCmd)
}
