package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

// XLSXCommentsAddResult is the JSON readback after adding a comment.
type XLSXCommentsAddResult struct {
	File            string   `json:"file"`
	Sheet           string   `json:"sheet"`
	SheetNumber     int      `json:"sheetNumber"`
	CommentID       int      `json:"commentId"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Author          string   `json:"author"`
	Text            string   `json:"text"`
	ContentHash     string   `json:"contentHash"`
	AnchoredToCell  string   `json:"anchoredToCell"`
	CreatedPart     bool     `json:"createdPart"`
	CreatedRef      bool     `json:"createdRef"`
	Operation       string   `json:"operation"`
	Output          string   `json:"output,omitempty"`
	DryRun          bool     `json:"dryRun"`

	ValidateCommand string `json:"validateCommand,omitempty"`
	ListCommand     string `json:"listCommand,omitempty"`
}

var (
	xlsxCommentsAddSheet    string
	xlsxCommentsAddCell     string
	xlsxCommentsAddAuthor   string
	xlsxCommentsAddText     string
	xlsxCommentsAddTextFile string
)

var xlsxCommentsAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Add a cell comment, creating the comments part if needed",
	Long:  "Anchor a comment to a cell (via the legacy <comment ref> attribute), creating the comments part, content-type, and worksheet relationship on demand. A paired VML drawing (xl/drawings/vmlDrawingN.vml) and worksheet <legacyDrawing> reference are also emitted so the note is visible as a box in desktop Excel.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxCommentsAddCell == "" {
			return InvalidArgsError("--cell is required")
		}
		if xlsxCommentsAddAuthor == "" {
			return InvalidArgsError("--author is required")
		}
		resolvedText, err := resolveOptionalDOCXParagraphText(cmd, "text", "text-file", xlsxCommentsAddText, xlsxCommentsAddTextFile)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXCommentsAddResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			sheetRef, err := resolveCommentsSheet(pkg, xlsxCommentsAddSheet)
			if err != nil {
				return err
			}
			addResult, err := xlsxmutate.AddComment(&xlsxmutate.AddCommentRequest{
				Package: pkg,
				Sheet:   sheetRef,
				Cell:    xlsxCommentsAddCell,
				Author:  xlsxCommentsAddAuthor,
				Text:    resolvedText,
			})
			if err != nil {
				return mapXLSXCommentMutationError(err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			handle := xlsxCommentHandle(pkg, sheetRef, addResult.AnchoredToCell)
			result = &XLSXCommentsAddResult{
				File:            filePath,
				Sheet:           sheetRef.Name,
				SheetNumber:     sheetRef.Number,
				CommentID:       addResult.CommentID,
				Handle:          handle,
				PrimarySelector: xlsxCommentPrimarySelector(handle, addResult.CommentID),
				Selectors:       xlsxCommentSelectors(handle, addResult.CommentID, addResult.AnchoredToCell),
				Author:          addResult.Author,
				Text:            addResult.Text,
				ContentHash:     addResult.ContentHash,
				AnchoredToCell:  addResult.AnchoredToCell,
				CreatedPart:     addResult.CreatedPart,
				CreatedRef:      addResult.CreatedRef,
				Operation:       "added",
				Output:          destinationFile,
				DryRun:          mutOpts != nil && mutOpts.DryRun,
			}
			if destinationFile != "" {
				result.ValidateCommand = xlsxValidateCommand(destinationFile)
				result.ListCommand = commentsListCommand(destinationFile, sheetRef)
			}
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "comments add")
		}
		return writeXLSXOutput(cmd, []byte(formatXLSXCommentsAddText(result)))
	},
}

func formatXLSXCommentsAddText(result *XLSXCommentsAddResult) string {
	return fmt.Sprintf("added comment %d on %s @ %s by %s", result.CommentID, result.Sheet, result.AnchoredToCell, result.Author)
}

func init() {
	xlsxCommentsAddCmd.Flags().StringVar(&xlsxCommentsAddSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxCommentsAddCmd.Flags().StringVar(&xlsxCommentsAddCell, "cell", "", "A1 cell reference to anchor the comment to (required)")
	xlsxCommentsAddCmd.Flags().StringVar(&xlsxCommentsAddAuthor, "author", "", "comment author name (required)")
	xlsxCommentsAddCmd.Flags().StringVar(&xlsxCommentsAddText, "text", "", "comment text")
	xlsxCommentsAddCmd.Flags().StringVar(&xlsxCommentsAddTextFile, "text-file", "", "path to comment text")
	AddMutationFlags(xlsxCommentsAddCmd)
	xlsxCommentsCmd.AddCommand(xlsxCommentsAddCmd)
}
