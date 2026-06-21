package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

// PPTXCommentsRemoveResult is the JSON readback after removing a comment.
type PPTXCommentsRemoveResult struct {
	File            string   `json:"file"`
	Output          string   `json:"output,omitempty"`
	DryRun          bool     `json:"dryRun"`
	Operation       string   `json:"operation"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	PPTXBridgeReadbackCommands
	pptxmutate.RemoveCommentResult
}

func newPPTXCommentsRemoveCmd() *cobra.Command {
	var (
		slide      int
		commentID  int
		authorID   int
		expectHash string
		handle     string
	)
	cmd := &cobra.Command{
		Use:   "remove <file>",
		Short: "Remove a comment from a slide",
		Long: `Delete a comment by id from a slide, guarded by --expect-hash. When the slide
has no remaining comments, the per-slide comments part and its relationship are
removed.`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			handleGiven := cmd.Flags().Changed("handle")
			if handleGiven && (cmd.Flags().Changed("slide") || cmd.Flags().Changed("comment-id") || cmd.Flags().Changed("author-id")) {
				return InvalidArgsError("cannot specify --handle with --slide, --comment-id, or --author-id")
			}
			if !handleGiven && slide < 1 {
				return InvalidArgsError("--slide must be >= 1")
			}
			if !handleGiven && !cmd.Flags().Changed("comment-id") {
				return InvalidArgsError("either --handle or --comment-id is required")
			}
			if !handleGiven && commentID < 0 {
				return InvalidArgsError("--comment-id must be >= 0")
			}
			authorIDSet := cmd.Flags().Changed("author-id")
			if authorIDSet && authorID < 0 {
				return InvalidArgsError("--author-id must be >= 0")
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			result, err := performPPTXCommentsRemove(filePath, slide, commentID, authorID, authorIDSet, handle, handleGiven, expectHash, mutOpts)
			if err != nil {
				return err
			}
			return writePPTXCommentsMutationResult(cmd, result, "comments remove",
				fmt.Sprintf("removed comment %d by %s on slide %d", result.CommentID, result.PreviousAuthor, result.Slide))
		},
	}
	cmd.Flags().IntVar(&slide, "slide", 0, "1-based slide number (required unless --handle)")
	cmd.Flags().IntVar(&commentID, "comment-id", 0, "comment id from list (required unless --handle)")
	cmd.Flags().IntVar(&authorID, "author-id", 0, "disambiguate by authorId when --comment-id is shared across authors")
	cmd.Flags().StringVar(&handle, "handle", "", "comment handle from comments list (H:pptx/s:<sldId>/comment:idx:<id>:authorId:<id>)")
	cmd.Flags().StringVar(&expectHash, "expect-hash", "", "expected sha256: content hash from list")
	AddMutationFlags(cmd)
	return cmd
}

func performPPTXCommentsRemove(filePath string, slide, commentID, authorID int, authorIDSet bool, handle string, handleGiven bool, expectHash string, mutOpts *MutationOptions) (*PPTXCommentsRemoveResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)

	var result *PPTXCommentsRemoveResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		if handleGiven {
			resolvedSlide, resolvedCommentID, resolvedAuthorID, err := resolvePPTXCommentHandleTarget(pkg, handle)
			if err != nil {
				return err
			}
			slide = resolvedSlide
			commentID = resolvedCommentID
			authorID = resolvedAuthorID
			authorIDSet = true
		}
		removeResult, err := pptxmutate.RemoveComment(&pptxmutate.RemoveCommentRequest{
			Package:      pkg,
			SlideNumber:  slide,
			CommentID:    commentID,
			AuthorID:     authorID,
			AuthorIDSet:  authorIDSet,
			ExpectedHash: expectHash,
		})
		if err != nil {
			return mapPPTXCommentMutationError(err)
		}
		slideID := pptxSlideIDByNumber(pkg, removeResult.Slide)
		resultHandle := pptxCommentHandle(slideID, removeResult.CommentID, removeResult.AuthorID)
		result = &PPTXCommentsRemoveResult{
			File:                filePath,
			Output:              destinationFile,
			DryRun:              mutOpts.DryRun,
			Operation:           "removed",
			Handle:              resultHandle,
			PrimarySelector:     pptxCommentPrimarySelector(resultHandle, removeResult.CommentID, removeResult.AuthorID),
			Selectors:           pptxCommentSelectors(resultHandle, removeResult.CommentID, removeResult.AuthorID),
			RemoveCommentResult: *removeResult,
		}
		result.PPTXBridgeReadbackCommands = pptxCommentsMutationReadbackCommands(destinationFile, removeResult.Slide)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to remove comment")
	}
	return result, nil
}

func init() {
	pptxCommentsCmd.AddCommand(newPPTXCommentsRemoveCmd())
}
