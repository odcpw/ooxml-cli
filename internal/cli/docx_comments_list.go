package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

// DOCXCommentsListResult is the JSON shape of docx comments list.
type DOCXCommentsListResult struct {
	File            string                `json:"file"`
	DocumentPartURI string                `json:"documentPartUri"`
	CommentsPart    string                `json:"commentsPart,omitempty"`
	Comments        []docxinspect.Comment `json:"comments"`
}

func newDOCXCommentsListCmd() *cobra.Command {
	var commentID int
	cmd := &cobra.Command{
		Use:   "list <file>",
		Short: "List all comments in the document",
		Long:  "List each comment (id, author, date, text, content hash, and anchored block).",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			pkg, err := openPackageExpectType(filePath, opc.PackageTypeDOCX)
			if err != nil {
				return err
			}
			defer pkg.Close()

			documentURI, err := docxinspect.FindMainDocumentPart(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
			}
			listing, err := docxinspect.ListComments(pkg, documentURI)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to list comments: %v", err)
			}

			comments := listing.Comments
			if cmd.Flags().Lookup("comment-id").Changed {
				filtered := make([]docxinspect.Comment, 0, 1)
				for _, c := range comments {
					if c.ID == commentID {
						filtered = append(filtered, c)
					}
				}
				if len(filtered) == 0 {
					return TargetNotFoundError(fmt.Sprintf("comment %d", commentID))
				}
				comments = filtered
			}

			result := &DOCXCommentsListResult{
				File:            filePath,
				DocumentPartURI: listing.DocumentPartURI,
				CommentsPart:    listing.CommentsPart,
				Comments:        comments,
			}
			if GetGlobalConfig(cmd).Format == "json" {
				return outputDOCXCommentsListJSON(cmd, result)
			}
			return outputDOCXCommentsListText(cmd, result)
		},
	}
	cmd.Flags().IntVar(&commentID, "comment-id", 0, "show only the comment with this id")
	return cmd
}

func outputDOCXCommentsListJSON(cmd *cobra.Command, result *DOCXCommentsListResult) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal comments list JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDOCXCommentsListText(cmd *cobra.Command, result *DOCXCommentsListResult) error {
	if len(result.Comments) == 0 {
		return writeCLIOutput(cmd, []byte("no comments"))
	}
	var b strings.Builder
	for i, c := range result.Comments {
		if i > 0 {
			b.WriteString("\n")
		}
		b.WriteString(fmt.Sprintf("comment %d by %s: %q", c.ID, c.Author, c.Text))
	}
	return writeCLIOutput(cmd, []byte(b.String()))
}

func init() {
	docxCommentsCmd.AddCommand(newDOCXCommentsListCmd())
}
