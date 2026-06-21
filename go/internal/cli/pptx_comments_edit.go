package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

// PPTXCommentsEditResult is the JSON readback after editing a comment.
type PPTXCommentsEditResult struct {
	File            string   `json:"file"`
	Output          string   `json:"output,omitempty"`
	DryRun          bool     `json:"dryRun"`
	Operation       string   `json:"operation"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	PPTXBridgeReadbackCommands
	pptxmutate.EditCommentResult
}

func newPPTXCommentsEditCmd() *cobra.Command {
	var (
		slide      int
		commentID  int
		authorID   int
		text       string
		textFile   string
		author     string
		date       string
		expectHash string
		handle     string
	)
	cmd := &cobra.Command{
		Use:   "edit <file>",
		Short: "Edit a comment's text, author, or date",
		Long: `Update a comment's text, author, and/or date on a slide, guarded by
--expect-hash. At least one of --text/--text-file, --author, or --date is
required.`,
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
			textSet := cmd.Flags().Changed("text")
			fileSet := cmd.Flags().Changed("text-file")
			authorSet := cmd.Flags().Changed("author")
			dateSet := cmd.Flags().Changed("date")
			if !textSet && !fileSet && !authorSet && !dateSet {
				return InvalidArgsError("specify at least one of --text, --text-file, --author, or --date")
			}
			resolvedText, err := resolvePPTXCommentText(cmd, text, textFile)
			if err != nil {
				return err
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			result, err := performPPTXCommentsEdit(filePath, slide, commentID, authorID, authorIDSet, handle, handleGiven, expectHash,
				resolvedText, textSet || fileSet, author, authorSet, date, dateSet, mutOpts)
			if err != nil {
				return err
			}
			return writePPTXCommentsMutationResult(cmd, result, "comments edit",
				fmt.Sprintf("edited comment %d on slide %d: %q", result.CommentID, result.Slide, result.Text))
		},
	}
	cmd.Flags().IntVar(&slide, "slide", 0, "1-based slide number (required unless --handle)")
	cmd.Flags().IntVar(&commentID, "comment-id", 0, "comment id from list (required unless --handle)")
	cmd.Flags().IntVar(&authorID, "author-id", 0, "disambiguate by authorId when --comment-id is shared across authors")
	cmd.Flags().StringVar(&handle, "handle", "", "comment handle from comments list (H:pptx/s:<sldId>/comment:idx:<id>:authorId:<id>)")
	cmd.Flags().StringVar(&text, "text", "", "new comment text")
	cmd.Flags().StringVar(&textFile, "text-file", "", "path to new comment text")
	cmd.Flags().StringVar(&author, "author", "", "update author name")
	cmd.Flags().StringVar(&date, "date", "", "update RFC3339 timestamp (empty clears it)")
	cmd.Flags().StringVar(&expectHash, "expect-hash", "", "expected sha256: content hash from list")
	AddMutationFlags(cmd)
	return cmd
}

func performPPTXCommentsEdit(filePath string, slide, commentID, authorID int, authorIDSet bool, handle string, handleGiven bool, expectHash, text string, textSet bool, author string, authorSet bool, date string, dateSet bool, mutOpts *MutationOptions) (*PPTXCommentsEditResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)

	var result *PPTXCommentsEditResult
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
		editResult, err := pptxmutate.EditComment(&pptxmutate.EditCommentRequest{
			Package:      pkg,
			SlideNumber:  slide,
			CommentID:    commentID,
			AuthorID:     authorID,
			AuthorIDSet:  authorIDSet,
			ExpectedHash: expectHash,
			Text:         text,
			TextSet:      textSet,
			Author:       author,
			AuthorSet:    authorSet,
			Date:         date,
			DateSet:      dateSet,
		})
		if err != nil {
			return mapPPTXCommentMutationError(err)
		}
		slideID := pptxSlideIDByNumber(pkg, editResult.Slide)
		resultHandle := pptxCommentHandle(slideID, editResult.CommentID, editResult.AuthorID)
		result = &PPTXCommentsEditResult{
			File:              filePath,
			Output:            destinationFile,
			DryRun:            mutOpts.DryRun,
			Operation:         "edited",
			Handle:            resultHandle,
			PrimarySelector:   pptxCommentPrimarySelector(resultHandle, editResult.CommentID, editResult.AuthorID),
			Selectors:         pptxCommentSelectors(resultHandle, editResult.CommentID, editResult.AuthorID),
			EditCommentResult: *editResult,
		}
		result.PPTXBridgeReadbackCommands = pptxCommentsMutationReadbackCommands(destinationFile, editResult.Slide)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to edit comment")
	}
	return result, nil
}

func init() {
	pptxCommentsCmd.AddCommand(newPPTXCommentsEditCmd())
}
