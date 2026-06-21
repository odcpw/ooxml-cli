package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

// XLSXCommentsUpdateResult is the JSON readback after updating a comment.
type XLSXCommentsUpdateResult struct {
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
	PreviousText    string   `json:"previousText"`
	PreviousHash    string   `json:"previousHash"`
	Operation       string   `json:"operation"`
	Output          string   `json:"output,omitempty"`
	DryRun          bool     `json:"dryRun"`

	ValidateCommand string `json:"validateCommand,omitempty"`
	ListCommand     string `json:"listCommand,omitempty"`
}

var (
	xlsxCommentsUpdateSheet      string
	xlsxCommentsUpdateCommentID  int
	xlsxCommentsUpdateText       string
	xlsxCommentsUpdateTextFile   string
	xlsxCommentsUpdateAuthor     string
	xlsxCommentsUpdateExpectHash string
	xlsxCommentsUpdateHandle     string
)

var xlsxCommentsUpdateCmd = &cobra.Command{
	Use:   "update <file>",
	Short: "Update an existing comment by id",
	Long:  "Update a comment's text and/or author by id, guarded by --expect-hash.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		handleGiven := cmd.Flags().Lookup("handle").Changed
		if !handleGiven && !cmd.Flags().Lookup("comment-id").Changed {
			return InvalidArgsError("either --handle or --comment-id is required")
		}
		if handleGiven && cmd.Flags().Lookup("comment-id").Changed {
			return InvalidArgsError("cannot specify both --handle and --comment-id")
		}
		if !handleGiven && xlsxCommentsUpdateCommentID < 0 {
			return InvalidArgsError("--comment-id must be >= 0")
		}
		textSet := cmd.Flags().Lookup("text").Changed
		textFileSet := cmd.Flags().Lookup("text-file").Changed
		authorSet := cmd.Flags().Lookup("author").Changed
		if textSet && textFileSet {
			return InvalidArgsError("cannot specify both --text and --text-file")
		}
		resolvedText := xlsxCommentsUpdateText
		if textFileSet {
			data, err := os.ReadFile(xlsxCommentsUpdateTextFile)
			if err != nil {
				return FileNotFoundError(xlsxCommentsUpdateTextFile)
			}
			resolvedText = string(data)
		}
		if !textSet && !textFileSet && !authorSet {
			return InvalidArgsError("specify at least one of --text, --text-file, or --author")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXCommentsUpdateResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			var (
				sheetRef  model.SheetRef
				commentID = xlsxCommentsUpdateCommentID
				err       error
			)
			if handleGiven {
				// A comment handle's sheetId scope + anchor cell are authoritative;
				// --sheet is ignored and the comment id is resolved by anchor.
				sheetRef, commentID, err = resolveCommentHandleTarget(pkg, xlsxCommentsUpdateHandle)
			} else {
				sheetRef, err = resolveCommentsSheet(pkg, xlsxCommentsUpdateSheet)
			}
			if err != nil {
				return err
			}
			updateResult, err := xlsxmutate.UpdateComment(&xlsxmutate.UpdateCommentRequest{
				Package:      pkg,
				Sheet:        sheetRef,
				CommentID:    commentID,
				ExpectedHash: xlsxCommentsUpdateExpectHash,
				Text:         resolvedText,
				TextSet:      textSet || textFileSet,
				Author:       xlsxCommentsUpdateAuthor,
				AuthorSet:    authorSet,
			})
			if err != nil {
				return mapXLSXCommentMutationError(err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			handle := xlsxCommentHandle(pkg, sheetRef, updateResult.AnchoredToCell)
			result = &XLSXCommentsUpdateResult{
				File:            filePath,
				Sheet:           sheetRef.Name,
				SheetNumber:     sheetRef.Number,
				CommentID:       updateResult.CommentID,
				Handle:          handle,
				PrimarySelector: xlsxCommentPrimarySelector(handle, updateResult.CommentID),
				Selectors:       xlsxCommentSelectors(handle, updateResult.CommentID, updateResult.AnchoredToCell),
				Author:          updateResult.Author,
				Text:            updateResult.Text,
				ContentHash:     updateResult.ContentHash,
				AnchoredToCell:  updateResult.AnchoredToCell,
				PreviousText:    updateResult.PreviousText,
				PreviousHash:    updateResult.PreviousHash,
				Operation:       "updated",
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
			return writeJSONResult(cmd, result, "comments update")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("updated comment %d on %s: %q", result.CommentID, result.Sheet, result.Text)))
	},
}

func init() {
	xlsxCommentsUpdateCmd.Flags().StringVar(&xlsxCommentsUpdateSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxCommentsUpdateCmd.Flags().IntVar(&xlsxCommentsUpdateCommentID, "comment-id", 0, "comment id from list (required unless --handle)")
	xlsxCommentsUpdateCmd.Flags().StringVar(&xlsxCommentsUpdateHandle, "handle", "", "comment handle (H:xlsx/ws:<sheetId>/comment:a:<A1>); supplies sheet + anchor, ignores --sheet/--comment-id")
	xlsxCommentsUpdateCmd.Flags().StringVar(&xlsxCommentsUpdateText, "text", "", "new comment text")
	xlsxCommentsUpdateCmd.Flags().StringVar(&xlsxCommentsUpdateTextFile, "text-file", "", "path to new comment text")
	xlsxCommentsUpdateCmd.Flags().StringVar(&xlsxCommentsUpdateAuthor, "author", "", "update author")
	xlsxCommentsUpdateCmd.Flags().StringVar(&xlsxCommentsUpdateExpectHash, "expect-hash", "", "expected sha256: content hash from list")
	AddMutationFlags(xlsxCommentsUpdateCmd)
	xlsxCommentsCmd.AddCommand(xlsxCommentsUpdateCmd)
}
