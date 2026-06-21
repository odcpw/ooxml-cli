package cli

import (
	"errors"
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxCommentsCmd = &cobra.Command{
	Use:     "comments",
	Aliases: []string{"comment"},
	Short:   "Inspect and mutate XLSX cell comments (legacy notes)",
	Long:    "Commands for listing, adding, updating, and removing worksheet cell comments stored in the legacy notes form (xl/commentsN.xml).",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// resolveCommentsSheet selects and validates the target worksheet.
func resolveCommentsSheet(pkg opc.PackageSession, selector string) (model.SheetRef, error) {
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return model.SheetRef{}, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, selector)
	if err != nil {
		return model.SheetRef{}, err
	}
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		return model.SheetRef{}, err
	}
	return sheetRef, nil
}

// resolveCommentHandleTarget decodes a comment handle, resolves its sheet scope
// (sheetId, ambiguity-safe), and finds the single comment anchored at the
// handle's cell, returning the worksheet and that comment's id. It is used by
// the comment mutation commands so a handle can address a comment by its stable
// (sheetId, anchor cell) identity instead of a positional --comment-id. A
// missing anchor yields HANDLE_STALE; a duplicated anchor yields
// HANDLE_AMBIGUOUS, never a silent first-match.
func resolveCommentHandleTarget(pkg opc.PackageSession, handleStr string) (model.SheetRef, int, error) {
	h, err := xlsxhandle.Parse(handleStr)
	if err != nil {
		return model.SheetRef{}, 0, mapXLSXHandleError(err)
	}
	if h.Kind != xlsxhandle.KindComment {
		return model.SheetRef{}, 0, InvalidArgsError("expected a comment handle (H:xlsx/ws:<sheetId>/comment:a:<A1>)")
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return model.SheetRef{}, 0, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheetRef, err := xlsxhandle.ResolveSheetRef(workbook.Sheets, h)
	if err != nil {
		return model.SheetRef{}, 0, mapXLSXHandleError(err)
	}
	sheetRef = model.WithSheetSelectors(sheetRef)
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		return model.SheetRef{}, 0, err
	}
	listing, err := xlsxinspect.ListComments(pkg, sheetRef)
	if err != nil {
		return model.SheetRef{}, 0, NewCLIErrorf(ExitUnexpected, "failed to list comments: %v", err)
	}
	var matches []int
	for _, c := range listing.Comments {
		if c.AnchoredToCell == h.CellRef {
			matches = append(matches, c.ID)
		}
	}
	switch len(matches) {
	case 0:
		return model.SheetRef{}, 0, mapXLSXHandleError(&xlsxhandle.Error{
			Code:    xlsxhandle.CodeStale,
			Handle:  handleStr,
			Message: fmt.Sprintf("no comment anchored at %s on sheetId %q", h.CellRef, h.SheetID),
		})
	case 1:
		return sheetRef, matches[0], nil
	default:
		return model.SheetRef{}, 0, mapXLSXHandleError(&xlsxhandle.Error{
			Code:    xlsxhandle.CodeAmbiguous,
			Handle:  handleStr,
			Message: fmt.Sprintf("%d comments anchored at %s on sheetId %q; cannot resolve to one", len(matches), h.CellRef, h.SheetID),
		})
	}
}

func xlsxCommentHandle(pkg opc.PackageSession, sheetRef model.SheetRef, anchoredToCell string) string {
	if strings.TrimSpace(sheetRef.SheetID) == "" || strings.TrimSpace(anchoredToCell) == "" {
		return ""
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return ""
	}
	if xlsxSheetIDCounts(workbook.Sheets)[sheetRef.SheetID] > 1 {
		return ""
	}
	return xlsxhandle.FormatComment(sheetRef.SheetID, anchoredToCell)
}

func xlsxCommentPrimarySelector(handle string, commentID int) string {
	if strings.TrimSpace(handle) != "" {
		return handle
	}
	if commentID >= 0 {
		return fmt.Sprintf("%d", commentID)
	}
	return ""
}

func xlsxCommentSelectors(handle string, commentID int, anchoredToCell string) []string {
	var out []string
	if strings.TrimSpace(handle) != "" {
		out = append(out, handle)
	}
	if commentID >= 0 {
		out = append(out, fmt.Sprintf("%d", commentID))
	}
	if strings.TrimSpace(anchoredToCell) != "" {
		out = append(out, anchoredToCell)
	}
	return out
}

// mapXLSXCommentMutationError translates mutate-layer comment errors to CLI errors.
func mapXLSXCommentMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, xlsxmutate.ErrCommentHashMismatch):
		return InvalidArgsError(err.Error())
	case errors.Is(err, xlsxmutate.ErrCommentNotFound):
		return TargetNotFoundError("comment")
	case errors.Is(err, xlsxmutate.ErrCommentExists):
		return InvalidArgsError(err.Error())
	default:
		return NewCLIErrorf(ExitInvalidArgs, "failed to mutate comments: %v", err)
	}
}

func commentsListCommand(filePath string, sheet model.SheetRef) string {
	return fmt.Sprintf("ooxml --json xlsx comments list %s --sheet %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheet)))
}

func init() {
	xlsxCmd.AddCommand(xlsxCommentsCmd)
}
