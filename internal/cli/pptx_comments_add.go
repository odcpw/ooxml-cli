package cli

import (
	"fmt"
	"os"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

// PPTXCommentsAddResult is the JSON readback after adding a comment.
type PPTXCommentsAddResult struct {
	File            string   `json:"file"`
	Output          string   `json:"output,omitempty"`
	DryRun          bool     `json:"dryRun"`
	Operation       string   `json:"operation"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	PPTXBridgeReadbackCommands
	pptxmutate.AddCommentResult
}

func newPPTXCommentsAddCmd() *cobra.Command {
	var (
		slide    int
		author   string
		initials string
		date     string
		text     string
		textFile string
	)
	cmd := &cobra.Command{
		Use:   "add <file>",
		Short: "Add a comment to a slide",
		Long: `Add a comment anchored to a slide, creating the per-slide comments part, the
shared commentAuthors part, their content-types, and the slide/presentation
relationships if missing.

Examples:
  ooxml pptx comments add deck.pptx --slide 1 --author "Alice" --text "Fix the title" --out out.pptx
  ooxml pptx comments add deck.pptx --slide 2 --author "Bob" --initials BB --text-file note.txt --in-place`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			if slide < 1 {
				return InvalidArgsError("--slide must be >= 1")
			}
			if author == "" {
				return InvalidArgsError("--author is required")
			}
			resolvedText, err := resolvePPTXCommentText(cmd, text, textFile)
			if err != nil {
				return err
			}
			resolvedDate := date
			if !cmd.Flags().Changed("date") {
				resolvedDate = time.Now().UTC().Format(time.RFC3339)
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			result, err := performPPTXCommentsAdd(filePath, slide, author, initials, resolvedDate, resolvedText, mutOpts)
			if err != nil {
				return err
			}
			return writePPTXCommentsMutationResult(cmd, result, "comments add",
				fmt.Sprintf("added comment %d by %s on slide %d", result.CommentID, result.Author, result.Slide))
		},
	}
	cmd.Flags().IntVar(&slide, "slide", 0, "1-based slide number (required)")
	cmd.Flags().StringVar(&author, "author", "", "comment author name (required)")
	cmd.Flags().StringVar(&initials, "initials", "", "comment author initials (optional)")
	cmd.Flags().StringVar(&date, "date", "", "RFC3339 timestamp (default: now)")
	cmd.Flags().StringVar(&text, "text", "", "comment text")
	cmd.Flags().StringVar(&textFile, "text-file", "", "path to comment text")
	AddMutationFlags(cmd)
	return cmd
}

func performPPTXCommentsAdd(filePath string, slide int, author, initials, date, text string, mutOpts *MutationOptions) (*PPTXCommentsAddResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)

	var result *PPTXCommentsAddResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		addResult, err := pptxmutate.AddComment(&pptxmutate.AddCommentRequest{
			Package:     pkg,
			SlideNumber: slide,
			Author:      author,
			Initials:    initials,
			Date:        date,
			Text:        text,
		})
		if err != nil {
			return mapPPTXCommentMutationError(err)
		}
		slideID := pptxSlideIDByNumber(pkg, addResult.Slide)
		handle := pptxCommentHandle(slideID, addResult.CommentID, addResult.AuthorID)
		result = &PPTXCommentsAddResult{
			File:             filePath,
			Output:           destinationFile,
			DryRun:           mutOpts.DryRun,
			Operation:        "added",
			Handle:           handle,
			PrimarySelector:  pptxCommentPrimarySelector(handle, addResult.CommentID, addResult.AuthorID),
			Selectors:        pptxCommentSelectors(handle, addResult.CommentID, addResult.AuthorID),
			AddCommentResult: *addResult,
		}
		result.PPTXBridgeReadbackCommands = pptxCommentsMutationReadbackCommands(destinationFile, addResult.Slide)
		return nil
	}); err != nil {
		return nil, mutationWriteError(err, "failed to add comment")
	}
	return result, nil
}

func init() {
	pptxCommentsCmd.AddCommand(newPPTXCommentsAddCmd())
}
