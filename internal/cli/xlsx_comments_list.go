package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/spf13/cobra"
)

// XLSXCommentListItem is a listed comment plus its stable handle (sheetId +
// anchor cell). A comment handle survives sheet reorder/rename but, being
// anchored by an A1 cell, NOT a row/column insert that shifts the anchor.
type XLSXCommentListItem struct {
	xlsxinspect.Comment
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
}

// XLSXCommentsListResult is the JSON shape of xlsx comments list.
type XLSXCommentsListResult struct {
	File         string                `json:"file"`
	Sheet        string                `json:"sheet"`
	SheetNumber  int                   `json:"sheetNumber"`
	CommentsPart string                `json:"commentsPart,omitempty"`
	Comments     []XLSXCommentListItem `json:"comments"`
	ListCommand  string                `json:"listCommand,omitempty"`
}

var (
	xlsxCommentsListSheet     string
	xlsxCommentsListCommentID int
)

var xlsxCommentsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List all cell comments on a worksheet",
	Long:  "List each cell comment (id, author, date, text, content hash, and anchor cell). The id is the 0-based position in the comment list.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		sheetRef, err := resolveCommentsSheet(pkg, xlsxCommentsListSheet)
		if err != nil {
			return err
		}
		listing, err := xlsxinspect.ListComments(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list comments: %v", err)
		}

		comments := listing.Comments
		if cmd.Flags().Lookup("comment-id").Changed {
			filtered := make([]xlsxinspect.Comment, 0, 1)
			for _, c := range comments {
				if c.ID == xlsxCommentsListCommentID {
					filtered = append(filtered, c)
				}
			}
			if len(filtered) == 0 {
				return TargetNotFoundError(fmt.Sprintf("comment %d", xlsxCommentsListCommentID))
			}
			comments = filtered
		}

		// Mint a comment handle (sheetId + anchor cell), omitted when the sheet's
		// sheetId is absent or duplicated.
		var counts map[string]int
		if wb, werr := xlsxinspect.ParseWorkbook(pkg); werr == nil {
			counts = xlsxSheetIDCounts(wb.Sheets)
		}
		items := make([]XLSXCommentListItem, 0, len(comments))
		for _, c := range comments {
			handle := ""
			if strings.TrimSpace(sheetRef.SheetID) != "" && (counts == nil || counts[sheetRef.SheetID] <= 1) {
				handle = xlsxhandle.FormatComment(sheetRef.SheetID, c.AnchoredToCell)
			}
			items = append(items, XLSXCommentListItem{
				Comment:         c,
				Handle:          handle,
				PrimarySelector: xlsxCommentPrimarySelector(handle, c.ID),
				Selectors:       xlsxCommentSelectors(handle, c.ID, c.AnchoredToCell),
			})
		}

		result := &XLSXCommentsListResult{
			File:         filePath,
			Sheet:        sheetRef.Name,
			SheetNumber:  sheetRef.Number,
			CommentsPart: listing.CommentsPart,
			Comments:     items,
			ListCommand:  commentsListCommand(filePath, sheetRef),
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "comments list")
		}
		return writeXLSXOutput(cmd, []byte(formatXLSXCommentsListText(result)))
	},
}

func formatXLSXCommentsListText(result *XLSXCommentsListResult) string {
	if len(result.Comments) == 0 {
		return fmt.Sprintf("no comments on %s", result.Sheet)
	}
	var b strings.Builder
	for i, c := range result.Comments {
		if i > 0 {
			b.WriteString("\n")
		}
		b.WriteString(fmt.Sprintf("comment %d @ %s by %s: %q", c.ID, c.AnchoredToCell, c.Author, c.Text))
	}
	return b.String()
}

func init() {
	xlsxCommentsListCmd.Flags().StringVar(&xlsxCommentsListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxCommentsListCmd.Flags().IntVar(&xlsxCommentsListCommentID, "comment-id", 0, "show only the comment with this id")
	xlsxCommentsCmd.AddCommand(xlsxCommentsListCmd)
}
