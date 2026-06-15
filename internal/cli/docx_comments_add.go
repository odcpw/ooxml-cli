package cli

import (
	"fmt"
	"os"
	"time"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXCommentsAddResult is the JSON readback after adding a comment.
type DOCXCommentsAddResult struct {
	File            string `json:"file"`
	CommentID       int    `json:"commentId"`
	Author          string `json:"author"`
	Date            string `json:"date,omitempty"`
	Initials        string `json:"initials,omitempty"`
	Text            string `json:"text"`
	ContentHash     string `json:"contentHash"`
	AnchoredToBlock int    `json:"anchoredToBlock"`
	CreatedPart     bool   `json:"createdPart"`
	CreatedRef      bool   `json:"createdRef"`
	Operation       string `json:"operation"`
}

func newDOCXCommentsAddCmd() *cobra.Command {
	var (
		anchorBlock int
		author      string
		initials    string
		date        string
		text        string
		textFile    string
	)
	cmd := &cobra.Command{
		Use:   "add <file>",
		Short: "Add a comment to a document block/range",
		Long:  "Anchor a comment to a body block, creating the comments part, content-type, and relationship if missing.",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			if author == "" {
				return InvalidArgsError("--author is required")
			}
			if cmd.Flags().Lookup("anchor-block").Changed && anchorBlock < 1 {
				return InvalidArgsError("--anchor-block must be >= 1")
			}
			resolvedText, err := resolveOptionalDOCXParagraphText(cmd, "text", "text-file", text, textFile)
			if err != nil {
				return err
			}
			resolvedDate := date
			if !cmd.Flags().Lookup("date").Changed {
				resolvedDate = time.Now().UTC().Format(time.RFC3339)
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			result, err := performDOCXCommentsAdd(filePath, anchorBlock, author, initials, resolvedDate, resolvedText, mutOpts)
			if err != nil {
				return err
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXCommentJSON(cmd, result, "comments add")
			}
			return writeCLIOutput(cmd, []byte(fmt.Sprintf("added comment %d by %s anchored to block %d", result.CommentID, result.Author, result.AnchoredToBlock)))
		},
	}
	cmd.Flags().IntVar(&anchorBlock, "anchor-block", 0, "1-based body block index to anchor to (default: first block)")
	cmd.Flags().StringVar(&author, "author", "", "comment author name (required)")
	cmd.Flags().StringVar(&initials, "initials", "", "comment author initials (optional)")
	cmd.Flags().StringVar(&date, "date", "", "RFC3339 timestamp (default: now)")
	cmd.Flags().StringVar(&text, "text", "", "comment text")
	cmd.Flags().StringVar(&textFile, "text-file", "", "path to comment text")
	AddMutationFlags(cmd)
	return cmd
}

func performDOCXCommentsAdd(filePath string, anchorBlock int, author, initials, date, text string, mutOpts *MutationOptions) (*DOCXCommentsAddResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXCommentsAddResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		addResult, err := docxmutate.AddComment(&docxmutate.AddCommentRequest{
			Package:     pkg,
			DocumentURI: documentURI,
			AnchorBlock: anchorBlock,
			Author:      author,
			Initials:    initials,
			Date:        date,
			Text:        text,
		})
		if err != nil {
			return mapDOCXCommentMutationError(err)
		}
		result = &DOCXCommentsAddResult{
			File:            filePath,
			CommentID:       addResult.CommentID,
			Author:          addResult.Author,
			Date:            addResult.Date,
			Initials:        addResult.Initials,
			Text:            addResult.Text,
			ContentHash:     addResult.ContentHash,
			AnchoredToBlock: addResult.AnchoredToBlock,
			CreatedPart:     addResult.CreatedPart,
			CreatedRef:      addResult.CreatedRef,
			Operation:       "added",
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxCommentsCmd.AddCommand(newDOCXCommentsAddCmd())
}
