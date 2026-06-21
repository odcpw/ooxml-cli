package cli

import (
	"fmt"
	"os"

	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXCommentsEditResult is the JSON readback after editing a comment.
type DOCXCommentsEditResult struct {
	File         string `json:"file"`
	CommentID    int    `json:"commentId"`
	Author       string `json:"author"`
	Date         string `json:"date,omitempty"`
	Initials     string `json:"initials,omitempty"`
	Text         string `json:"text"`
	ContentHash  string `json:"contentHash"`
	PreviousText string `json:"previousText"`
	PreviousHash string `json:"previousHash"`
	Operation    string `json:"operation"`
	Handle       string `json:"handle,omitempty"`
}

func newDOCXCommentsEditCmd() *cobra.Command {
	var (
		commentID  int
		handle     string
		text       string
		textFile   string
		author     string
		date       string
		expectHash string
	)
	cmd := &cobra.Command{
		Use:   "edit <file>",
		Short: "Edit an existing comment by ID",
		Long:  "Update a comment's text, author, and/or date, guarded by --expect-hash.",
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
			textSet := cmd.Flags().Lookup("text").Changed
			textFileSet := cmd.Flags().Lookup("text-file").Changed
			authorSet := cmd.Flags().Lookup("author").Changed
			dateSet := cmd.Flags().Lookup("date").Changed
			if textSet && textFileSet {
				return InvalidArgsError("cannot specify both --text and --text-file")
			}
			resolvedText := text
			if textFileSet {
				data, err := os.ReadFile(textFile)
				if err != nil {
					return FileNotFoundError(textFile)
				}
				resolvedText = string(data)
			}
			if !textSet && !textFileSet && !authorSet && !dateSet {
				return InvalidArgsError("specify at least one of --text, --text-file, --author, or --date")
			}
			mutOpts, err := GetValidatedMutationOptions(cmd)
			if err != nil {
				return err
			}

			handleArg := ""
			if handleSet {
				handleArg = handle
			}
			result, err := performDOCXCommentsEdit(filePath, commentID, handleArg, expectHash,
				resolvedText, textSet || textFileSet, author, authorSet, date, dateSet, mutOpts)
			if err != nil {
				return err
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXCommentJSON(cmd, result, "comments edit")
			}
			return writeCLIOutput(cmd, []byte(fmt.Sprintf("edited comment %d: %q", result.CommentID, result.Text)))
		},
	}
	cmd.Flags().IntVar(&commentID, "comment-id", 0, "comment id from list (required unless --handle)")
	cmd.Flags().StringVar(&handle, "handle", "", "stable comment handle (H:docx/pt:doc/comment:n:<id>); authoritative for the target, ignores --comment-id")
	cmd.Flags().StringVar(&text, "text", "", "new comment text")
	cmd.Flags().StringVar(&textFile, "text-file", "", "path to new comment text")
	cmd.Flags().StringVar(&author, "author", "", "update author")
	cmd.Flags().StringVar(&date, "date", "", "update RFC3339 timestamp")
	cmd.Flags().StringVar(&expectHash, "expect-hash", "", "expected sha256: content hash from list")
	AddMutationFlags(cmd)
	return cmd
}

func performDOCXCommentsEdit(filePath string, commentID int, handleArg, expectHash, text string, textSet bool, author string, authorSet bool, date string, dateSet bool, mutOpts *MutationOptions) (*DOCXCommentsEditResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXCommentsEditResult
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
		editResult, err := docxmutate.EditComment(&docxmutate.EditCommentRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			CommentID:    targetID,
			ExpectedHash: expectHash,
			Text:         text,
			TextSet:      textSet,
			Author:       author,
			AuthorSet:    authorSet,
			Date:         date,
			DateSet:      dateSet,
		})
		if err != nil {
			return mapDOCXCommentMutationError(err)
		}
		result = &DOCXCommentsEditResult{
			File:         filePath,
			CommentID:    editResult.CommentID,
			Author:       editResult.Author,
			Date:         editResult.Date,
			Initials:     editResult.Initials,
			Text:         editResult.Text,
			ContentHash:  editResult.ContentHash,
			PreviousText: editResult.PreviousText,
			PreviousHash: editResult.PreviousHash,
			Operation:    "edited",
			Handle:       docxhandle.FormatComment(editResult.CommentID),
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxCommentsCmd.AddCommand(newDOCXCommentsEditCmd())
}
