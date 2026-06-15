package cli

import (
	"errors"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	pptxselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

var pptxCommentsCmd = &cobra.Command{
	Use:     "comments",
	Aliases: []string{"comment"},
	Short:   "Inspect and mutate PPTX slide comments",
	Long: `Commands for listing, adding, editing, and removing slide comments.

Comments use the widely-loadable legacy form: a per-slide comments part
(/ppt/comments/commentN.xml, root p:cmLst) plus a shared authors part
(/ppt/commentAuthors.xml). Comments are anchored to a slide.`,
	Args: cobra.NoArgs,
	RunE: showHelp,
}

// mapPPTXCommentMutationError translates mutate-layer comment errors to CLI errors.
func mapPPTXCommentMutationError(err error) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, pptxmutate.ErrCommentHashMismatch):
		return InvalidArgsError(err.Error())
	case errors.Is(err, pptxmutate.ErrCommentAmbiguous):
		return InvalidArgsError(err.Error())
	case errors.Is(err, pptxmutate.ErrCommentNotFound):
		return TargetNotFoundError("comment")
	case errors.Is(err, pptxmutate.ErrSlideOutOfRange):
		return InvalidArgsError(err.Error())
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate comments: %v", err)
	}
}

// resolvePPTXCommentText resolves the comment body from --text or --text-file,
// rejecting the case where both are given.
func resolvePPTXCommentText(cmd *cobra.Command, text, textFile string) (string, error) {
	textSet := cmd.Flags().Changed("text")
	fileSet := cmd.Flags().Changed("text-file")
	if textSet && fileSet {
		return "", InvalidArgsError("cannot specify both --text and --text-file")
	}
	if fileSet {
		data, err := os.ReadFile(textFile)
		if err != nil {
			return "", FileNotFoundError(textFile)
		}
		return string(data), nil
	}
	return text, nil
}

// writePPTXCommentsMutationResult marshals a comment mutation result as JSON or
// emits a human-readable summary line.
func writePPTXCommentsMutationResult(cmd *cobra.Command, result any, label, summary string) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal %s JSON: %v", label, err)
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeCLIOutput(cmd, data)
	}
	return writeCLIOutput(cmd, []byte(summary))
}

// pptxCommentsMutationReadbackCommands builds the generated readback/validate/
// render follow-up commands for a comment mutation on a slide.
func pptxCommentsMutationReadbackCommands(destinationFile string, slide int) PPTXBridgeReadbackCommands {
	return pptxBridgeReadbackCommands(destinationFile, slide, func(path string) string {
		return pptxCommentsReadbackCommand(path, slide)
	})
}

func pptxCommentsReadbackCommand(filePath string, slide int) string {
	return fmt.Sprintf("ooxml --json pptx comments list %s --slide %d", pptxXLSXCommandArg(filePath), slide)
}

func annotatePPTXCommentSelectors(listing *pptxinspect.SlideComments, slideID uint32) {
	if listing == nil {
		return
	}
	for i := range listing.Comments {
		comment := &listing.Comments[i]
		handle := pptxCommentHandle(slideID, comment.ID, comment.AuthorID)
		comment.Handle = handle
		comment.PrimarySelector = pptxCommentPrimarySelector(handle, comment.ID, comment.AuthorID)
		comment.Selectors = pptxCommentSelectors(handle, comment.ID, comment.AuthorID)
	}
}

func pptxCommentHandle(slideID uint32, commentID, authorID int) string {
	if slideID == 0 || commentID < 0 || authorID < 0 {
		return ""
	}
	return pptxhandle.FormatComment(slideID, commentID, authorID)
}

func pptxSlideIDByNumber(pkg opc.PackageSession, slideNumber int) uint32 {
	graph, err := pptxinspect.ParsePresentation(pkg)
	if err != nil {
		return 0
	}
	for _, slide := range graph.Slides {
		if slide.SlideNumber == slideNumber {
			return slide.SlideID
		}
	}
	return 0
}

func pptxCommentPrimarySelector(handle string, commentID, authorID int) string {
	if strings.TrimSpace(handle) != "" {
		return handle
	}
	return fmt.Sprintf("comment:%d:authorId:%d", commentID, authorID)
}

func pptxCommentSelectors(handle string, commentID, authorID int) []string {
	out := []string{
		fmt.Sprintf("comment:%d:authorId:%d", commentID, authorID),
		fmt.Sprintf("comment:%d", commentID),
		strconv.Itoa(commentID),
		fmt.Sprintf("authorId:%d", authorID),
	}
	if strings.TrimSpace(handle) != "" {
		out = append([]string{handle}, out...)
	}
	return out
}

func resolvePPTXCommentHandleTarget(pkg opc.PackageSession, handleStr string) (int, int, int, error) {
	h, err := pptxhandle.Parse(handleStr)
	if err != nil {
		return 0, 0, 0, mapPPTXHandleError(err)
	}
	if h.Kind != pptxhandle.KindComment {
		return 0, 0, 0, InvalidArgsError("expected a PPTX comment handle (H:pptx/s:<sldId>/comment:idx:<id>:authorId:<id>)")
	}
	graph, err := pptxinspect.ParsePresentation(pkg)
	if err != nil {
		return 0, 0, 0, NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
	}
	slideRef, err := pptxselectors.ResolveSlideRefForHandle(graph, h)
	if err != nil {
		return 0, 0, 0, mapPPTXHandleError(err)
	}
	listing, err := pptxinspect.ListSlideComments(pkg, slideRef.PartURI, slideRef.SlideNumber)
	if err != nil {
		return 0, 0, 0, NewCLIErrorf(ExitUnexpected, "failed to list comments: %v", err)
	}
	matches := 0
	for _, comment := range listing.Comments {
		if comment.ID == h.CommentID && comment.AuthorID == h.AuthorID {
			matches++
		}
	}
	switch matches {
	case 0:
		return 0, 0, 0, mapPPTXHandleError(&pptxhandle.Error{
			Code:    pptxhandle.CodeStale,
			Handle:  handleStr,
			Message: fmt.Sprintf("comment idx %d authorId %d was not found on slide sldId %d", h.CommentID, h.AuthorID, h.SlideID),
		})
	case 1:
		return slideRef.SlideNumber, h.CommentID, h.AuthorID, nil
	default:
		return 0, 0, 0, mapPPTXHandleError(&pptxhandle.Error{
			Code:    pptxhandle.CodeAmbiguous,
			Handle:  handleStr,
			Message: fmt.Sprintf("%d comments share idx %d authorId %d on slide sldId %d", matches, h.CommentID, h.AuthorID, h.SlideID),
		})
	}
}

func pptxCommentNotFoundError(listing *pptxinspect.SlideComments, commentID int) error {
	slide := 0
	if listing != nil {
		slide = listing.Slide
	}
	candidates := pptxCommentSelectorCandidates(listing)
	discovery := "ooxml --json pptx comments list <file>"
	if slide > 0 {
		discovery += " --slide " + strconv.Itoa(slide)
	}
	return SelectorNotFoundError("comment", "comment:"+strconv.Itoa(commentID), BuildSelectorCandidates(candidates, "comment:"+strconv.Itoa(commentID), maxSelectorCandidates), discovery)
}

func pptxCommentSelectorCandidates(listing *pptxinspect.SlideComments) []SelectorCandidate {
	if listing == nil {
		return nil
	}
	out := make([]SelectorCandidate, 0, len(listing.Comments))
	for _, comment := range listing.Comments {
		out = append(out, SelectorCandidate{Primary: comment.PrimarySelector, Selectors: comment.Selectors})
	}
	return out
}

func init() {
	pptxCmd.AddCommand(pptxCommentsCmd)
}
