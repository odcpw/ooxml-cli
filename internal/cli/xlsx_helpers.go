package cli

import (
	"errors"
	"fmt"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func selectXLSXSheet(sheets []model.SheetRef, selector string) (model.SheetRef, error) {
	if len(sheets) == 0 {
		return model.SheetRef{}, NewCLIErrorf(ExitInvalidArgs, "workbook has no sheets")
	}
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return sheets[0], nil
	}
	// Handle-first branch (additive): a handle's sheetId is the authoritative
	// scope and is resolved by SEARCHING for the matching <sheet sheetId=>,
	// refusing on a duplicate, never first-winning. Any non-handle string falls
	// through to the legacy selector logic below unchanged. A sheet handle
	// resolves directly; a cell/comment handle's scope still selects its sheet
	// (the cell/comment objref is consumed by the cell/comment command paths).
	if xlsxhandle.IsHandle(selector) {
		h, err := xlsxhandle.Parse(selector)
		if err != nil {
			return model.SheetRef{}, mapXLSXHandleError(err)
		}
		if h.Kind == xlsxhandle.KindDefinedName {
			return model.SheetRef{}, mapXLSXHandleError(&xlsxhandle.Error{
				Code:    xlsxhandle.CodeMalformed,
				Handle:  selector,
				Message: "a workbook-scoped defined-name handle does not select a sheet",
			})
		}
		// A bare --sheet accepts a SHEET handle only. A cell/comment handle is
		// rejected here rather than silently coerced to its sheet scope, because
		// the cell/anchor part would be dropped; those handles belong on the
		// cell/comment command flags (--cell / --handle), which consume the full
		// objref.
		if h.Kind != xlsxhandle.KindSheet {
			return model.SheetRef{}, mapXLSXHandleError(&xlsxhandle.Error{
				Code:    xlsxhandle.CodeMalformed,
				Handle:  selector,
				Message: "expected a sheet handle (H:xlsx/ws:<sheetId>); a cell/comment handle belongs on the cell/comment flag",
			})
		}
		ref, err := xlsxhandle.ResolveSheetRef(sheets, h)
		if err != nil {
			return model.SheetRef{}, mapXLSXHandleError(err)
		}
		return model.WithSheetSelectors(ref), nil
	}
	for _, sheet := range sheets {
		withSelectors := model.WithSheetSelectors(sheet)
		if model.SelectorMatches(withSelectors.Selectors, selector) {
			return withSelectors, nil
		}
	}
	if number, err := strconv.Atoi(selector); err == nil {
		if number < 1 || number > len(sheets) {
			return model.SheetRef{}, NewCLIErrorf(ExitTargetNotFound, "sheet %d is out of range (1-%d)", number, len(sheets))
		}
		return model.WithSheetSelectors(sheets[number-1]), nil
	}
	candidates := sheetSelectorCandidates(sheets)
	return model.SheetRef{}, SelectorNotFoundError("sheet", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), "ooxml --json xlsx sheets list <file>")
}

func sheetSelectorCandidates(sheets []model.SheetRef) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(sheets))
	for _, sheet := range sheets {
		withSelectors := model.WithSheetSelectors(sheet)
		out = append(out, SelectorCandidate{Primary: withSelectors.PrimarySelector, Selectors: withSelectors.Selectors})
	}
	return out
}

func isXLSXWorksheetRef(sheet model.SheetRef) bool {
	if sheet.RelationshipType == namespaces.RelWorksheet {
		return true
	}
	if sheet.RelationshipType != "" {
		return false
	}
	return strings.HasPrefix(sheet.PartURI, "/xl/worksheets/")
}

func requireXLSXWorksheetRef(sheet model.SheetRef) error {
	if isXLSXWorksheetRef(sheet) {
		return nil
	}
	return NewCLIErrorf(ExitInvalidArgs, "sheet %q is not a worksheet", sheet.Name)
}

func requireXLSXCellHandleTargetExists(pkg opc.PackageSession, sheet model.SheetRef, cellRef string) error {
	doc, err := pkg.ReadXMLPart(sheet.PartURI)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to read worksheet %s: %v", sheet.PartURI, err)
	}
	root := doc.Root()
	if root == nil {
		return NewCLIErrorf(ExitUnexpected, "worksheet part %s has no root element", sheet.PartURI)
	}
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	if sheetData != nil {
		for _, row := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
			for _, cell := range namespaces.FindChildren(row, namespaces.NsSpreadsheetML, "c") {
				if cell.SelectAttrValue("r", "") == cellRef {
					return nil
				}
			}
		}
	}
	return mapXLSXHandleError(&xlsxhandle.Error{
		Code:    xlsxhandle.CodeStale,
		Handle:  xlsxhandle.FormatCell(sheet.SheetID, cellRef),
		Message: fmt.Sprintf("cell %s no longer exists on sheet %q; row/column structure may have shifted", cellRef, sheet.Name),
	})
}

// xlsxSheetIDCounts tallies how many sheets carry each native <sheet sheetId=>,
// so surfacing can omit a handle for any non-unique sheetId (mirrors
// pptxSlideIDCounts). Sheets with an empty sheetId are not counted.
func xlsxSheetIDCounts(sheets []model.SheetRef) map[string]int {
	counts := make(map[string]int, len(sheets))
	for _, sheet := range sheets {
		if strings.TrimSpace(sheet.SheetID) != "" {
			counts[sheet.SheetID]++
		}
	}
	return counts
}

// xlsxSheetHandleString mints a stable sheet handle (H:xlsx/ws:<sheetId>) for a
// sheet, or "" when the sheet has no native sheetId OR when the sheetId is
// shared by more than one sheet (a handle for a duplicated sheetId would
// mis-resolve, so we never mint one). counts may be nil to skip the uniqueness
// check.
func xlsxSheetHandleString(sheet model.SheetRef, counts map[string]int) string {
	if strings.TrimSpace(sheet.SheetID) == "" {
		return ""
	}
	if counts != nil && counts[sheet.SheetID] > 1 {
		return ""
	}
	return xlsxhandle.FormatSheet(sheet.SheetID)
}

// xlsxCellHandleString mints a stable cell handle scoped to the sheet's
// sheetId, or "" when no handle can be minted for the sheet (no sheetId or a
// duplicated sheetId). The A1 ref is honestly positional within the grid: the
// handle survives sheet reorder/rename but NOT a row/column insert that shifts
// the address (see pkg/xlsx/handle doc).
func xlsxCellHandleString(sheet model.SheetRef, cellRef string, counts map[string]int) string {
	if strings.TrimSpace(sheet.SheetID) == "" || strings.TrimSpace(cellRef) == "" {
		return ""
	}
	if counts != nil && counts[sheet.SheetID] > 1 {
		return ""
	}
	return xlsxhandle.FormatCell(sheet.SheetID, cellRef)
}

// mapXLSXHandleError maps a typed XLSX handle error to a CLI error with the
// right exit code (mirrors mapPPTXHandleError). A stale/scope-stale/ambiguous
// handle is a "not found" condition; a malformed or format-mismatched handle is
// an invalid-args condition.
func mapXLSXHandleError(err error) error {
	if err == nil {
		return nil
	}
	switch {
	case xlsxhandle.IsCode(err, xlsxhandle.CodeMalformed),
		xlsxhandle.IsCode(err, xlsxhandle.CodeFormatMismatch):
		return xlsxHandleCLIError(err, ExitInvalidArgs)
	case xlsxhandle.IsCode(err, xlsxhandle.CodeScopeStale),
		xlsxhandle.IsCode(err, xlsxhandle.CodeStale),
		xlsxhandle.IsCode(err, xlsxhandle.CodeAmbiguous):
		return xlsxHandleCLIError(err, ExitTargetNotFound)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func xlsxHandleCLIError(err error, exitCode int) *CLIError {
	var xerr *xlsxhandle.Error
	if errors.As(err, &xerr) && xerr.Code != "" {
		return &CLIError{ExitCode: exitCode, Code: xerr.Code, Message: err.Error()}
	}
	return &CLIError{ExitCode: exitCode, Message: err.Error()}
}
