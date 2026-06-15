package cli

import (
	"errors"

	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/spf13/cobra"
)

var docxCommentsCmd = &cobra.Command{
	Use:     "comments",
	Aliases: []string{"comment"},
	Short:   "Inspect and mutate DOCX comments",
	Long:    "Commands for listing, adding, editing, and removing document comments.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// mapDOCXCommentMutationError translates mutate-layer comment errors to CLI errors.
func mapDOCXCommentMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrCommentHashMismatch):
		return InvalidArgsError(err.Error())
	case errors.Is(err, docxmutate.ErrCommentNotFound):
		return TargetNotFoundError("comment")
	case errors.Is(err, docxmutate.ErrCommentAnchorOutOfRange):
		return InvalidArgsError(err.Error())
	case errors.Is(err, docxmutate.ErrCommentAnchorNotParagraph):
		return InvalidArgsError(err.Error())
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate comments: %v", err)
	}
}

// outputDOCXCommentJSON marshals a comment mutation result honoring --pretty.
func outputDOCXCommentJSON(cmd *cobra.Command, result interface{}, label string) error {
	return writeLabeledJSON(cmd, result, label)
}

func init() {
	docxCmd.AddCommand(docxCommentsCmd)
}
