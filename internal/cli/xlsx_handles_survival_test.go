package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// xlsxTwoSheetWorkbook copies the minimal single-sheet fixture and adds a
// second sheet ("Second"), returning the path to a two-sheet workbook with
// sheetIds 1 (Sheet1) and 2 (Second). Two sheets are needed to exercise
// reorder/insert/delete survival.
func xlsxTwoSheetWorkbook(t *testing.T, dir string) string {
	t.Helper()
	fixture, err := filepath.Abs("../../testdata/xlsx/minimal-workbook/workbook.xlsx")
	require.NoError(t, err)
	base := filepath.Join(dir, "base.xlsx")
	require.NoError(t, copyFileForTest(base, fixture))

	twoSheet := filepath.Join(dir, "two.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "add", base, "--name", "Second", "--out", twoSheet)
	require.NoError(t, err)
	return twoSheet
}

// sheetHandleByName returns the surfaced sheet handle for the named sheet from
// `xlsx sheets list`.
func sheetHandleByName(t *testing.T, file, name string) string {
	t.Helper()
	out, err := runOOXML(t, "--json", "xlsx", "sheets", "list", file)
	require.NoError(t, err)
	var res XLSXSheetsListResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	for _, s := range res.Sheets {
		if s.Name == name {
			require.NotEmpty(t, s.Handle, "expected a handle for sheet %q", name)
			return s.Handle
		}
	}
	t.Fatalf("sheet %q not found", name)
	return ""
}

// sheetIDByName returns the native sheetId of the named sheet from
// `xlsx sheets list`. sheetIds are randomly allocated, so tests that need the
// concrete id (e.g. the legacy sheetId: selector) discover it at runtime.
func sheetIDByName(t *testing.T, file, name string) string {
	t.Helper()
	out, err := runOOXML(t, "--json", "xlsx", "sheets", "list", file)
	require.NoError(t, err)
	var res XLSXSheetsListResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	for _, s := range res.Sheets {
		if s.Name == name {
			require.NotEmpty(t, s.SheetID, "expected a sheetId for sheet %q", name)
			return s.SheetID
		}
	}
	t.Fatalf("sheet %q not found", name)
	return ""
}

// cellValueOnSheet extracts a single cell's value from a worksheet.
func cellValueOnSheet(t *testing.T, file, sheet, ref string) (string, bool) {
	t.Helper()
	out, err := runOOXML(t, "--json", "xlsx", "cells", "extract", file, "--sheet", sheet, "--range", ref)
	require.NoError(t, err)
	var res XLSXCellsExtractResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	if res.Sheet == nil {
		return "", false
	}
	for _, row := range res.Sheet.Rows {
		for _, c := range row.Cells {
			if c.Ref == ref {
				return c.Value, true
			}
		}
	}
	return "", false
}

// TestXLSXSheetHandleSurvivesReorder is a headline proof: a cell handle scoped
// to a sheetId still resolves to the SAME sheet after the workbook's sheets are
// reordered (which changes every sheet's 1-based number) — even when the
// mutation is invoked with a DELIBERATELY WRONG --sheet. The handle's sheetId,
// not --sheet, is authoritative for scope.
func TestXLSXSheetHandleSurvivesReorder(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	// Seed a cell on "Second" (sheetId 2) and capture its handle.
	seeded := filepath.Join(dir, "seeded.xlsx")
	out, err := runOOXML(t, "--json", "xlsx", "cells", "set", two,
		"--sheet", "Second", "--cell", "B7", "--value", "BEFORE", "--out", seeded)
	require.NoError(t, err)
	var setRes XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(out), &setRes))
	// sheetId is randomly allocated, so the handle is discovered at runtime rather
	// than asserted as a literal ws:2.
	require.NotEmpty(t, setRes.Handle)
	cellHandle := setRes.Handle

	// UNRELATED structural edit: move "Second" to position 1. Its sheet NUMBER
	// changes (2 -> 1) but its sheetId (2) does not.
	moved := filepath.Join(dir, "moved.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "move", seeded, "--sheet", "Second", "--to", "1", "--out", moved)
	require.NoError(t, err)

	// Mutate via the cell handle with a WRONG --sheet (9 does not exist). If
	// --sheet were authoritative this would fail; the handle's sheetId 2 wins.
	edited := filepath.Join(dir, "edited.xlsx")
	_, err = runOOXML(t, "xlsx", "cells", "set", moved,
		"--cell", cellHandle, "--sheet", "9", "--value", "SURVIVED", "--out", edited)
	require.NoError(t, err)

	// The value landed on "Second" B7 (the original target), proving survival.
	val, ok := cellValueOnSheet(t, edited, "Second", "B7")
	require.True(t, ok, "cell handle %s must still resolve after reorder", cellHandle)
	assert.Equal(t, "SURVIVED", val)

	// The read path (cells extract) surfaces the same stable handle per cell.
	out, err = runOOXML(t, "--json", "xlsx", "cells", "extract", edited, "--sheet", "Second", "--range", "B7")
	require.NoError(t, err)
	var extract XLSXCellsExtractResult
	require.NoError(t, json.Unmarshal([]byte(out), &extract))
	require.NotNil(t, extract.Sheet)
	foundHandle := ""
	for _, row := range extract.Sheet.Rows {
		for _, c := range row.Cells {
			if c.Ref == "B7" {
				foundHandle = c.Handle
			}
		}
	}
	assert.Equal(t, cellHandle, foundHandle, "cells extract must surface the cell handle on the read path")
}

func TestXLSXCellClearHandleSurvivesReorder(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	seeded := filepath.Join(dir, "seeded.xlsx")
	out, err := runOOXML(t, "--json", "xlsx", "cells", "set", two,
		"--sheet", "Second", "--cell", "B7", "--value", "TO_CLEAR", "--out", seeded)
	require.NoError(t, err)
	var setRes XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(out), &setRes))
	require.NotEmpty(t, setRes.Handle)

	moved := filepath.Join(dir, "moved.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "move", seeded, "--sheet", "Second", "--to", "1", "--out", moved)
	require.NoError(t, err)

	cleared := filepath.Join(dir, "cleared.xlsx")
	_, err = runOOXML(t, "--json", "xlsx", "cells", "clear", moved,
		"--range", setRes.Handle, "--sheet", "9", "--out", cleared)
	require.NoError(t, err)

	val, ok := cellValueOnSheet(t, cleared, "Second", "B7")
	if ok {
		assert.Empty(t, val, "cell handle clear should clear the handled cell")
	}
}

func TestXLSXWrongFormatHandleReportsFormatMismatch(t *testing.T) {
	dir := t.TempDir()
	fixture, err := filepath.Abs("../../testdata/xlsx/minimal-workbook/workbook.xlsx")
	require.NoError(t, err)
	base := filepath.Join(dir, "base.xlsx")
	require.NoError(t, copyFileForTest(base, fixture))

	out := filepath.Join(dir, "wrong-format.xlsx")
	_, err = runOOXML(t, "xlsx", "cells", "set", base,
		"--cell", "H:pptx/s:256/shape:n:2", "--value", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), xlsxhandle.CodeFormatMismatch)
	assertCLIHandleCode(t, err, ExitInvalidArgs, xlsxhandle.CodeFormatMismatch)
}

// TestXLSXSheetHandleSurvivesInsert proves the sheet handle survives an
// unrelated sheet INSERT before it (which shifts later sheet numbers).
func TestXLSXSheetHandleSurvivesInsert(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	secondHandle := sheetHandleByName(t, two, "Second")
	require.NotEmpty(t, secondHandle)

	// Insert a new first sheet; "Second" shifts position but keeps its sheetId.
	inserted := filepath.Join(dir, "inserted.xlsx")
	_, err := runOOXML(t, "xlsx", "sheets", "add", two, "--name", "Inserted", "--out", inserted)
	require.NoError(t, err)
	// Move the inserted sheet to the front so "Second" is no longer at its old
	// position.
	front := filepath.Join(dir, "front.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "move", inserted, "--sheet", "Inserted", "--to", "1", "--out", front)
	require.NoError(t, err)

	// Resolve the sheet handle through --sheet while using a normal A1 cell ref.
	// A sheet handle can scope creation; a cell handle cannot create a missing
	// cell because cell handles address previously observed cells.
	out, err := runOOXML(t, "--json", "xlsx", "cells", "set", front,
		"--sheet", secondHandle, "--cell", "A1", "--value", "HIT", "--out", filepath.Join(dir, "e.xlsx"))
	require.NoError(t, err)
	var res XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	assert.Equal(t, "Second", res.Sheet, "the sheet handle must still resolve to Second after an insert")
}

// TestXLSXDeletedSheetCleanStaleError proves a deleted scope yields a clean
// HANDLE_SCOPE_STALE error and never a wrong-target hit.
func TestXLSXDeletedSheetCleanStaleError(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	// Capture "Second"'s sheet handle, then delete it. A cell handle scoped to
	// that (now-deleted) sheetId must go stale.
	secondHandle := sheetHandleByName(t, two, "Second")
	require.NotEmpty(t, secondHandle)

	deleted := filepath.Join(dir, "deleted.xlsx")
	_, err := runOOXML(t, "xlsx", "sheets", "delete", two, "--sheet", "Second", "--out", deleted)
	require.NoError(t, err)

	out := filepath.Join(dir, "x.xlsx")
	_, err = runOOXML(t, "xlsx", "cells", "set", deleted,
		"--cell", secondHandle+"/cell:a:A1", "--value", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), xlsxhandle.CodeScopeStale)
	assertCLIHandleCode(t, err, ExitTargetNotFound, xlsxhandle.CodeScopeStale)
	_, statErr := os.Stat(out)
	assert.True(t, os.IsNotExist(statErr), "a stale scope must not write the output file")
}

func TestXLSXCellHandleAfterRowInsertFailsStaleNoMutation(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	seeded := filepath.Join(dir, "seeded.xlsx")
	out, err := runOOXML(t, "--json", "xlsx", "cells", "set", two,
		"--sheet", "Second", "--cell", "B7", "--value", "BEFORE", "--out", seeded)
	require.NoError(t, err)
	var setRes XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(out), &setRes))
	require.NotEmpty(t, setRes.Handle)

	shifted := filepath.Join(dir, "shifted.xlsx")
	_, err = runOOXML(t, "xlsx", "rows", "insert", seeded,
		"--sheet", "Second", "--at", "1", "--out", shifted)
	require.NoError(t, err)

	result := filepath.Join(dir, "result.xlsx")
	_, err = runOOXML(t, "--json", "xlsx", "cells", "set", shifted,
		"--cell", setRes.Handle, "--value", "WRONG", "--out", result)
	require.Error(t, err, "a cell handle whose A1 address shifted must fail stale")
	assertCLIHandleCode(t, err, ExitTargetNotFound, xlsxhandle.CodeStale)

	_, statErr := os.Stat(result)
	assert.True(t, os.IsNotExist(statErr), "a stale cell handle must not write the output file")
	val, ok := cellValueOnSheet(t, shifted, "Second", "B8")
	require.True(t, ok, "row insert should have shifted original B7 to B8")
	assert.Equal(t, "BEFORE", val)
	val, ok = cellValueOnSheet(t, shifted, "Second", "B7")
	assert.False(t, ok && val == "WRONG", "stale handle must not create a wrong-target B7 cell")
}

func TestXLSXCellClearHandleAfterRowInsertFailsStaleNoMutation(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	seeded := filepath.Join(dir, "seeded.xlsx")
	out, err := runOOXML(t, "--json", "xlsx", "cells", "set", two,
		"--sheet", "Second", "--cell", "B7", "--value", "BEFORE", "--out", seeded)
	require.NoError(t, err)
	var setRes XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(out), &setRes))
	require.NotEmpty(t, setRes.Handle)

	shifted := filepath.Join(dir, "shifted.xlsx")
	_, err = runOOXML(t, "xlsx", "rows", "insert", seeded,
		"--sheet", "Second", "--at", "1", "--out", shifted)
	require.NoError(t, err)

	result := filepath.Join(dir, "result.xlsx")
	_, err = runOOXML(t, "--json", "xlsx", "cells", "clear", shifted,
		"--range", setRes.Handle, "--out", result)
	require.Error(t, err, "a clear by cell handle whose A1 address shifted must fail stale")
	assertCLIHandleCode(t, err, ExitTargetNotFound, xlsxhandle.CodeStale)

	_, statErr := os.Stat(result)
	assert.True(t, os.IsNotExist(statErr), "a stale cell handle clear must not write the output file")
	val, ok := cellValueOnSheet(t, shifted, "Second", "B8")
	require.True(t, ok, "row insert should have shifted original B7 to B8")
	assert.Equal(t, "BEFORE", val)
	val, ok = cellValueOnSheet(t, shifted, "Second", "B7")
	assert.False(t, ok && val == "", "stale handle clear must not clear or create the old B7 address")
}

// TestXLSXDeletedThenAddedSheetDoesNotReuseID is the Finding-1 regression: a
// handle minted for a sheet that is later DELETED must NOT silently re-point at a
// sheet ADDED afterward. The old nextSheetID = max(existing)+1 scheme freed the
// highest sheetId on delete and handed it to the next add, so a pre-delete handle
// resolved cleanly to the WRONG sheet (silent wrong-target). With random sheetId
// allocation the freed id is effectively never reused, so the stale handle
// resolves to ZERO matches (CodeScopeStale) and writes nothing.
func TestXLSXDeletedThenAddedSheetDoesNotReuseID(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	// Capture a cell handle scoped to "Second" (the highest-id sheet, the one
	// whose id max+1 would have freed and reused).
	seeded := filepath.Join(dir, "seeded.xlsx")
	out, err := runOOXML(t, "--json", "xlsx", "cells", "set", two,
		"--sheet", "Second", "--cell", "B7", "--value", "ORIGINAL", "--out", seeded)
	require.NoError(t, err)
	var setRes XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(out), &setRes))
	cellHandle := setRes.Handle
	require.NotEmpty(t, cellHandle)

	// Delete "Second" (frees its sheetId under the old scheme).
	deleted := filepath.Join(dir, "deleted.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "delete", seeded, "--sheet", "Second", "--out", deleted)
	require.NoError(t, err)

	// Add a brand-new sheet. Under max+1 it would REUSE "Second"'s freed id, so
	// the pre-delete handle would silently resolve to this new wrong sheet.
	added := filepath.Join(dir, "added.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "add", deleted, "--name", "Fresh", "--out", added)
	require.NoError(t, err)

	// The old handle must go stale, NOT write into "Fresh".
	result := filepath.Join(dir, "result.xlsx")
	_, err = runOOXML(t, "xlsx", "cells", "set", added,
		"--cell", cellHandle, "--value", "WRONG", "--out", result)
	require.Error(t, err, "a handle to a deleted sheet must not resolve to a newly added sheet")
	assert.Contains(t, err.Error(), xlsxhandle.CodeScopeStale)
	assertCLIHandleCode(t, err, ExitTargetNotFound, xlsxhandle.CodeScopeStale)
	_, statErr := os.Stat(result)
	assert.True(t, os.IsNotExist(statErr), "a stale handle must not write the output file")

	// "Fresh" must be untouched by the stale handle.
	val, ok := cellValueOnSheet(t, added, "Fresh", "B7")
	assert.False(t, ok && val == "WRONG", "the stale handle silently wrote into the wrong (newly added) sheet")
}

// TestXLSXDefinedNameHandleSurvivesReorder proves a workbook-scoped defined-name
// handle resolves to the same name after sheets are reordered, because a defined
// name is a native, position-independent address (name-based resolution).
//
// LIMITATION (cell-vs-row-insert, documented per the task): a bare cell handle
// (A1 scoped to a sheetId) survives sheet reorder/rename but does NOT survive a
// row/column insert that shifts the A1 address — grid coordinates have no native
// per-cell id. The defined-name handle would be the stable cell address that
// survives a row insert, EXCEPT that ooxml-cli's structural row/column commands
// REFUSE any workbook containing defined names (ErrWorkbookHasDefinedNames:
// "workbook has defined names"). Empirically confirmed: `xlsx names add` then
// `xlsx rows insert` fails with that error. So no tool-reachable structural edit
// shifts a cell sitting under a defined name; the defined-name handle's
// stability is delivered by name-based resolution, not by ref rewriting. We
// therefore prove the REACHABLE survival (sheet reorder) here.
func TestXLSXDefinedNameHandleSurvivesReorder(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	named := filepath.Join(dir, "named.xlsx")
	_, err := runOOXML(t, "xlsx", "names", "add", two, "--name", "SalesTotal", "--sheet", "Second", "--ref", "A1", "--out", named)
	require.NoError(t, err)

	// Reorder sheets; the workbook-scoped name is unaffected by sheet position.
	moved := filepath.Join(dir, "moved.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "move", named, "--sheet", "Second", "--to", "1", "--out", moved)
	require.NoError(t, err)

	// Resolve the SAME defined-name handle after the reorder via names rename.
	out, err := runOOXML(t, "--json", "xlsx", "names", "rename", moved,
		"--name", "H:xlsx/wb/name:n:SalesTotal", "--new-name", "Renamed", "--out", filepath.Join(dir, "r.xlsx"))
	require.NoError(t, err)
	var res XLSXNameMutationResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	require.NotNil(t, res.Name)
	assert.Equal(t, "Renamed", res.Name.Name)
	assert.Equal(t, "SalesTotal", res.PreviousName)
}

// TestXLSXCommentHandleSurvivesReorder proves a comment handle (sheetId + anchor
// cell) resolves to the same comment after a sheet reorder, and that removal via
// the handle targets the right comment.
func TestXLSXCommentHandleSurvivesReorder(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	secondHandle := sheetHandleByName(t, two, "Second")
	require.NotEmpty(t, secondHandle)

	withComment := filepath.Join(dir, "comment.xlsx")
	_, err := runOOXML(t, "xlsx", "comments", "add", two,
		"--sheet", "Second", "--cell", "C3", "--author", "Me", "--text", "Note", "--out", withComment)
	require.NoError(t, err)

	moved := filepath.Join(dir, "moved.xlsx")
	_, err = runOOXML(t, "xlsx", "sheets", "move", withComment, "--sheet", "Second", "--to", "1", "--out", moved)
	require.NoError(t, err)

	commentHandle := secondHandle + "/comment:a:C3"

	// Update the comment via its handle after the reorder; --sheet and
	// --comment-id are omitted.
	updated := filepath.Join(dir, "updated.xlsx")
	out, err := runOOXML(t, "--json", "xlsx", "comments", "update", moved,
		"--handle", commentHandle, "--text", "Updated", "--out", updated)
	require.NoError(t, err)
	var updateRes XLSXCommentsUpdateResult
	require.NoError(t, json.Unmarshal([]byte(out), &updateRes))
	assert.Equal(t, "Second", updateRes.Sheet)
	assert.Equal(t, "C3", updateRes.AnchoredToCell)
	assert.Equal(t, "Updated", updateRes.Text)

	// Remove the comment via its handle after the reorder; --sheet is omitted.
	out, err = runOOXML(t, "--json", "xlsx", "comments", "remove", updated,
		"--handle", commentHandle, "--out", filepath.Join(dir, "removed.xlsx"))
	require.NoError(t, err)
	var res XLSXCommentsRemoveResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	assert.Equal(t, "Second", res.Sheet)
	assert.Equal(t, "C3", res.AnchoredToCell)
}

// TestXLSXLegacySelectorsStillResolve is the back-compat guard: every legacy
// sheet selector keeps resolving unchanged after the handle-first branch was
// added to selectXLSXSheet.
func TestXLSXLegacySelectorsStillResolve(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)

	secondSheetID := sheetIDByName(t, two, "Second")
	for _, sel := range []string{"Second", "sheet:2", "sheetId:" + secondSheetID, "name:Second", "~Second", "#2", "2"} {
		out, err := runOOXML(t, "--json", "xlsx", "cells", "set", two,
			"--sheet", sel, "--cell", "A1", "--value", "ok", "--out", filepath.Join(dir, "leg.xlsx"))
		require.NoError(t, err, "legacy selector %q must still resolve", sel)
		var res XLSXCellsSetResult
		require.NoError(t, json.Unmarshal([]byte(out), &res))
		assert.Equal(t, "Second", res.Sheet, "selector %q resolved to wrong sheet", sel)
	}
}

func assertCLIHandleCode(t *testing.T, err error, wantExit int, wantCode string) {
	t.Helper()
	cliErr, ok := err.(*CLIError)
	require.True(t, ok, "error type = %T, want *CLIError", err)
	assert.Equal(t, wantExit, cliErr.ExitCode)
	assert.Equal(t, wantCode, cliErr.Code)
}
