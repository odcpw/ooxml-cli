package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

// XLSXCommentsRemoveResult is the JSON readback after removing a comment.
type XLSXCommentsRemoveResult struct {
	File            string   `json:"file"`
	Sheet           string   `json:"sheet"`
	SheetNumber     int      `json:"sheetNumber"`
	CommentID       int      `json:"commentId"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	PreviousAuthor  string   `json:"previousAuthor"`
	PreviousText    string   `json:"previousText"`
	PreviousHash    string   `json:"previousHash"`
	AnchoredToCell  string   `json:"anchoredToCell"`
	RemovedPart     bool     `json:"removedPart"`
	Operation       string   `json:"operation"`
	Output          string   `json:"output,omitempty"`
	DryRun          bool     `json:"dryRun"`

	ValidateCommand string `json:"validateCommand,omitempty"`
	ListCommand     string `json:"listCommand,omitempty"`
}

var (
	xlsxCommentsRemoveSheet      string
	xlsxCommentsRemoveCommentID  int
	xlsxCommentsRemoveExpectHash string
	xlsxCommentsRemoveHandle     string
)

var xlsxCommentsRemoveCmd = &cobra.Command{
	Use:     "remove <file>",
	Aliases: []string{"delete"},
	Short:   "Remove a cell comment by id",
	Long:    "Delete a comment entry by id, guarded by --expect-hash. Removing the last comment drops the comments part and its worksheet relationship.",
	Args:    cobra.ExactArgs(1),
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
		if !handleGiven && xlsxCommentsRemoveCommentID < 0 {
			return InvalidArgsError("--comment-id must be >= 0")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXCommentsRemoveResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			var (
				sheetRef  model.SheetRef
				commentID = xlsxCommentsRemoveCommentID
				err       error
			)
			if handleGiven {
				// A comment handle's sheetId scope + anchor cell are authoritative;
				// --sheet is ignored and the comment id is resolved by anchor.
				sheetRef, commentID, err = resolveCommentHandleTarget(pkg, xlsxCommentsRemoveHandle)
			} else {
				sheetRef, err = resolveCommentsSheet(pkg, xlsxCommentsRemoveSheet)
			}
			if err != nil {
				return err
			}
			removeResult, err := xlsxmutate.RemoveComment(&xlsxmutate.RemoveCommentRequest{
				Package:      pkg,
				Sheet:        sheetRef,
				CommentID:    commentID,
				ExpectedHash: xlsxCommentsRemoveExpectHash,
			})
			if err != nil {
				return mapXLSXCommentMutationError(err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			handle := xlsxCommentHandle(pkg, sheetRef, removeResult.AnchoredToCell)
			result = &XLSXCommentsRemoveResult{
				File:            filePath,
				Sheet:           sheetRef.Name,
				SheetNumber:     sheetRef.Number,
				CommentID:       removeResult.CommentID,
				Handle:          handle,
				PrimarySelector: xlsxCommentPrimarySelector(handle, removeResult.CommentID),
				Selectors:       xlsxCommentSelectors(handle, removeResult.CommentID, removeResult.AnchoredToCell),
				PreviousAuthor:  removeResult.PreviousAuthor,
				PreviousText:    removeResult.PreviousText,
				PreviousHash:    removeResult.PreviousHash,
				AnchoredToCell:  removeResult.AnchoredToCell,
				RemovedPart:     removeResult.RemovedPart,
				Operation:       "removed",
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
			return writeJSONResult(cmd, result, "comments remove")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("removed comment %d on %s @ %s by %s", result.CommentID, result.Sheet, result.AnchoredToCell, result.PreviousAuthor)))
	},
}

func init() {
	xlsxCommentsRemoveCmd.Flags().StringVar(&xlsxCommentsRemoveSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxCommentsRemoveCmd.Flags().IntVar(&xlsxCommentsRemoveCommentID, "comment-id", 0, "comment id from list (required unless --handle)")
	xlsxCommentsRemoveCmd.Flags().StringVar(&xlsxCommentsRemoveHandle, "handle", "", "comment handle (H:xlsx/ws:<sheetId>/comment:a:<A1>); supplies sheet + anchor, ignores --sheet/--comment-id")
	xlsxCommentsRemoveCmd.Flags().StringVar(&xlsxCommentsRemoveExpectHash, "expect-hash", "", "expected sha256: content hash from list")
	AddMutationFlags(xlsxCommentsRemoveCmd)
	xlsxCommentsCmd.AddCommand(xlsxCommentsRemoveCmd)
}
