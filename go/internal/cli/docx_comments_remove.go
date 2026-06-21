package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXCommentsRemoveResult is the JSON readback after removing a comment.
type DOCXCommentsRemoveResult struct {
	File                string `json:"file"`
	CommentID           int    `json:"commentId"`
	PreviousAuthor      string `json:"previousAuthor"`
	PreviousText        string `json:"previousText"`
	PreviousHash        string `json:"previousHash"`
	RangeMarkersRemoved bool   `json:"rangeMarkersRemoved"`
	Operation           string `json:"operation"`
}

func newDOCXCommentsRemoveCmd() *cobra.Command {
	var (
		commentID  int
		handle     string
		expectHash string
	)
	cmd := &cobra.Command{
		Use:   "remove <file>",
		Short: "Remove a comment and its range markers",
		Long:  "Delete a comment entry and its w:commentRangeStart/End markers and reference run, guarded by --expect-hash.",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			handleSet := cmd.Flags().Lookup("handle").Changed
			if handleSet && cmd.Flags().Lookup("comment-id").Changed {
				return InvalidArgsError("cannot specify both --comment-id and --handle")
			}
			if !handleSet && !cmd.Flags().Lookup("comment-id").Changed {
				return InvalidArgsError("--comment-id is required (or pass --handle)")
			}
			if !handleSet && commentID < 0 {
				return InvalidArgsError("--comment-id must be >= 0")
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			handleArg := ""
			if handleSet {
				handleArg = handle
			}
			result, err := performDOCXCommentsRemove(filePath, commentID, handleArg, expectHash, mutOpts)
			if err != nil {
				return err
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXCommentJSON(cmd, result, "comments remove")
			}
			return writeCLIOutput(cmd, []byte(fmt.Sprintf("removed comment %d by %s", result.CommentID, result.PreviousAuthor)))
		},
	}
	cmd.Flags().IntVar(&commentID, "comment-id", 0, "comment id from list (required unless --handle)")
	cmd.Flags().StringVar(&handle, "handle", "", "stable comment handle (H:docx/pt:doc/comment:n:<id>); authoritative for the target, ignores --comment-id")
	cmd.Flags().StringVar(&expectHash, "expect-hash", "", "expected sha256: content hash from list")
	AddMutationFlags(cmd)
	return cmd
}

func performDOCXCommentsRemove(filePath string, commentID int, handleArg, expectHash string, mutOpts *MutationOptions) (*DOCXCommentsRemoveResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXCommentsRemoveResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		targetID := commentID
		if handleArg != "" {
			resolved, herr := resolveDOCXCommentHandleID(pkg, handleArg)
			if herr != nil {
				return herr
			}
			targetID = resolved
		}
		removeResult, err := docxmutate.RemoveComment(&docxmutate.RemoveCommentRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			CommentID:    targetID,
			ExpectedHash: expectHash,
		})
		if err != nil {
			return mapDOCXCommentMutationError(err)
		}
		result = &DOCXCommentsRemoveResult{
			File:                filePath,
			CommentID:           removeResult.CommentID,
			PreviousAuthor:      removeResult.PreviousAuthor,
			PreviousText:        removeResult.PreviousText,
			PreviousHash:        removeResult.PreviousHash,
			RangeMarkersRemoved: removeResult.RangeMarkersRemoved,
			Operation:           "removed",
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxCommentsCmd.AddCommand(newDOCXCommentsRemoveCmd())
}
