package cli

// Readback-symmetry parity tests (PR-AGENT-5).
//
// Each test runs a mutation command with --out, parses the mutation result's
// readback object (the "destination"), then runs the corresponding inspect/show
// command (the one the mutation advertises via its generated readback command)
// against the produced file, and unmarshals BOTH into the SAME canonical inspect
// struct. The shared domain fields must match. This locks in shape parity so a
// future change to either side that breaks before/after diffing fails loudly.
//
// Canonical readback shapes (documented in MUTATION.md):
//   - xlsx cells set / xlsx ranges set -> destination is XLSXRangeDestination,
//     whose data block (range/rows/cols/values/types/formulas/...) matches the
//     XLSXRangesExportResult emitted by `xlsx ranges export`.
//   - pptx tables set-cell -> destination is *PPTXTableSummary, the same element
//     type emitted in the Tables slice of `pptx tables show`.
//   - pptx shapes set-bounds -> destination is *PPTXShapeDestination, whose
//     shared fields match the PPTXShapeEntry emitted by `pptx shapes get`.
//   - docx blocks replace -> destination is *extract.BlockReport, the same type
//     emitted in the Blocks slice of `docx blocks --block N --include-runs`.

import (
	"encoding/json"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// xlsxRangeDataBlock holds only the canonical data fields shared by the
// mutation destination and the ranges-export inspect result. Both sides
// unmarshal into this struct; metadata-only fields (file, sheet selectors, the
// export-specific bridge commands) are intentionally ignored.
type xlsxRangeDataBlock struct {
	Range             string     `json:"range"`
	Rows              int        `json:"rows"`
	Cols              int        `json:"cols"`
	Values            [][]any    `json:"values"`
	Types             [][]string `json:"types"`
	Formulas          [][]any    `json:"formulas"`
	StyleIndexes      [][]any    `json:"styleIndexes"`
	NumberFormatIDs   [][]any    `json:"numberFormatIds"`
	NumberFormatCodes [][]any    `json:"numberFormatCodes"`
	FormulaCount      int        `json:"formulaCount"`
}

func TestXLSXCellsSetReadbackSymmetry(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/xlsx/types-and-formulas/workbook.xlsx")
	require.NoError(t, err)
	out := filepath.Join(t.TempDir(), "cells-set.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set", fixture,
		"--cell", "A1", "--value", "Realigned",
		"--out", out,
	)
	require.NoError(t, err)

	var result XLSXCellsSetResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.NotNil(t, result.Destination, "json readback must include destination")
	require.NotEmpty(t, result.RangesExportCommand, "must advertise a ranges export readback command")

	mutationBlock := marshalIntoXLSXRangeBlock(t, result.Destination)

	exportJSON := executeGeneratedOOXMLCommandForXLSXTest(t, result.RangesExportCommand)
	var export XLSXRangesExportResult
	require.NoError(t, json.Unmarshal([]byte(exportJSON), &export))
	inspectBlock := marshalIntoXLSXRangeBlock(t, &export)

	assert.Equal(t, "A1", result.Destination.Range)
	assert.Equal(t, "Realigned", result.Value)
	assertXLSXRangeBlockParity(t, mutationBlock, inspectBlock)
}

func TestXLSXRangesSetReadbackSymmetry(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/xlsx/types-and-formulas/workbook.xlsx")
	require.NoError(t, err)
	out := filepath.Join(t.TempDir(), "ranges-set.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "set", fixture,
		"--sheet", "1",
		"--anchor", "A1",
		"--values", `[[1,2,3],[4,5,6]]`,
		"--out", out,
	)
	require.NoError(t, err)

	var result XLSXRangesSetResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.NotNil(t, result.Destination)
	require.NotEmpty(t, result.RangesExportCommand)
	mutationBlock := marshalIntoXLSXRangeBlock(t, result.Destination)

	exportJSON := executeGeneratedOOXMLCommandForXLSXTest(t, result.RangesExportCommand)
	var export XLSXRangesExportResult
	require.NoError(t, json.Unmarshal([]byte(exportJSON), &export))
	inspectBlock := marshalIntoXLSXRangeBlock(t, &export)

	assert.Equal(t, 2, result.Destination.Rows)
	assert.Equal(t, 3, result.Destination.Cols)
	assertXLSXRangeBlockParity(t, mutationBlock, inspectBlock)
}

func TestPPTXTablesSetCellReadbackSymmetry(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err)
	out := filepath.Join(t.TempDir(), "tables-set.pptx")

	// Table lives on slide 2 in this fixture.
	showJSON, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", fixture, "--slide", "2",
	)
	require.NoError(t, err)
	var pre PPTXTablesShowResult
	require.NoError(t, json.Unmarshal([]byte(showJSON), &pre))
	require.NotEmpty(t, pre.Tables, "fixture must contain a table on slide 2")
	tableID := pre.Tables[0].ShapeID

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "set-cell", fixture,
		"--slide", "2", "--table-id", strconv.Itoa(tableID),
		"--row", "1", "--col", "1", "--text", "Realigned",
		"--out", out,
	)
	require.NoError(t, err)

	var result PPTXTablesSetCellResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.NotNil(t, result.Destination)
	require.NotEmpty(t, result.ReadbackCommand)

	readback := executeGeneratedOOXMLCommandForXLSXTest(t, result.ReadbackCommand)
	var show PPTXTablesShowResult
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	require.Len(t, show.Tables, 1)
	inspect := show.Tables[0]

	// Both sides are the SAME struct type (PPTXTableSummary); compare the shared
	// domain fields. File is mutation-only metadata.
	mutation := *result.Destination
	mutation.File = ""
	inspect.File = ""
	assert.Equal(t, inspect.ShapeID, mutation.ShapeID)
	assert.Equal(t, inspect.ShapeName, mutation.ShapeName)
	assert.Equal(t, inspect.PrimarySelector, mutation.PrimarySelector)
	assert.Equal(t, inspect.Rows, mutation.Rows)
	assert.Equal(t, inspect.Cols, mutation.Cols)
	assert.Equal(t, inspect.Cells, mutation.Cells)
	assert.Equal(t, "Realigned", mutation.Cells[0][0])
}

func TestPPTXShapesSetBoundsReadbackSymmetry(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	out := filepath.Join(t.TempDir(), "shapes-set.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "set-bounds", fixture,
		"--slide", "1", "--target", "title",
		"--bounds", "100,200,5000000,1000000",
		"--out", out,
	)
	require.NoError(t, err)

	var result PPTXShapesSetBoundsResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.NotNil(t, result.Destination)
	require.NotEmpty(t, result.ReadbackCommand)

	readback := executeGeneratedOOXMLCommandForXLSXTest(t, result.ReadbackCommand)
	var got PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(readback), &got))
	require.Len(t, got.Shapes, 1)
	inspect := got.Shapes[0]
	dest := result.Destination

	// PPTXShapeDestination and PPTXShapeEntry diverge in field set, but their
	// SHARED fields use the same JSON tags and must agree.
	assert.Equal(t, inspect.ShapeID, dest.ShapeID)
	assert.Equal(t, inspect.ShapeName, dest.ShapeName)
	assert.Equal(t, inspect.TargetKind, dest.TargetKind)
	assert.Equal(t, inspect.PrimarySelector, dest.PrimarySelector)
	assert.Equal(t, inspect.Selectors, dest.Selectors)
	require.NotNil(t, dest.Bounds)
	require.NotNil(t, inspect.Bounds)
	assert.Equal(t, *inspect.Bounds, *dest.Bounds)
	assert.Equal(t, int64(100), dest.Bounds.X)
	assert.Equal(t, int64(200), dest.Bounds.Y)
	// Geometry and imageRef are part of the shared shape shape too; assert parity
	// (both nil for a text placeholder, but the contract must still hold).
	assert.Equal(t, inspect.Geometry, dest.Geometry)
	assert.Equal(t, inspect.ImageRef, dest.ImageRef)
}

func TestDOCXBlocksReplaceReadbackSymmetry(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/docx/mixed-blocks/document.docx")
	require.NoError(t, err)
	out := filepath.Join(t.TempDir(), "blocks-replace.docx")

	// Resolve the expected hash for a paragraph block (index 2).
	blocksJSON, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", fixture,
	)
	require.NoError(t, err)
	var pre DOCXBlocksResult
	require.NoError(t, json.Unmarshal([]byte(blocksJSON), &pre))
	var hash string
	for _, b := range pre.Blocks {
		if b.Index == 2 {
			hash = b.ContentHash
		}
	}
	require.NotEmpty(t, hash)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", "replace", fixture,
		"--block", "2", "--expect-hash", hash,
		"--text", "Realigned readback",
		"--out", out,
	)
	require.NoError(t, err)

	var result DOCXBlockParagraphResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.NotNil(t, result.Destination, "json readback must include destination block")

	// Inspect the produced file's block 2 with the same detail level.
	showJSON, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", out,
		"--block", "2", "--include-runs",
	)
	require.NoError(t, err)
	var post DOCXBlocksResult
	require.NoError(t, json.Unmarshal([]byte(showJSON), &post))
	require.Len(t, post.Blocks, 1)
	inspect := post.Blocks[0]

	// Same canonical type (extract.BlockReport) on both sides: every field must
	// match exactly.
	assert.Equal(t, inspect, *result.Destination)
	assert.Equal(t, "body.b2", result.Destination.ID)
	assert.Equal(t, "Realigned readback", result.Destination.Text)
	assert.Equal(t, result.ContentHash, result.Destination.ContentHash)
}

// plainStructureWorkbookXML is a formula-free, single-sheet worksheet that
// structural row/column mutations accept (they refuse formulas, merges, tables,
// etc.). Used by the XLSX structure readback-symmetry tests.
const plainStructureWorkbookXML = `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="2"><c r="A2"><v>2</v></c><c r="B2"><v>3</v></c></row>
    <row r="3"><c r="C3"><v>4</v></c></row>
  </sheetData>
</worksheet>`

func TestXLSXRowsInsertReadbackSymmetry(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, plainStructureWorkbookXML)
	out := filepath.Join(t.TempDir(), "rows-insert.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rows", "insert", workbookPath,
		"--sheet", "Sheet1", "--at", "2", "--count", "1",
		"--out", out,
	)
	require.NoError(t, err)

	var result XLSXStructureMutationResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.False(t, result.DryRun)
	require.NotEmpty(t, result.SheetsListCommand, "must advertise a sheets list readback command")
	require.NotEmpty(t, result.ValidateCommand, "must advertise a validate readback command")
	assert.Equal(t, "insert", result.Operation)
	assert.Equal(t, "rows", result.Axis)

	assertXLSXStructureSheetVisibleViaReadback(t, result)
	// Canonical case: force the used-range parity assertion to run so the test
	// genuinely proves sheets-show confirms the structural change.
	assertXLSXStructureSheetShowReadback(t, result, true)
}

func TestXLSXRowsDeleteReadbackSymmetry(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, plainStructureWorkbookXML)
	out := filepath.Join(t.TempDir(), "rows-delete.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rows", "delete", workbookPath,
		"--sheet", "Sheet1", "--row", "2", "--count", "1",
		"--out", out,
	)
	require.NoError(t, err)

	var result XLSXStructureMutationResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.NotEmpty(t, result.SheetsListCommand)
	assert.Equal(t, "delete", result.Operation)
	assert.Equal(t, "rows", result.Axis)

	assertXLSXStructureSheetVisibleViaReadback(t, result)
	assertXLSXStructureSheetShowReadback(t, result, false)
}

func TestXLSXColsInsertReadbackSymmetry(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, plainStructureWorkbookXML)
	out := filepath.Join(t.TempDir(), "cols-insert.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cols", "insert", workbookPath,
		"--sheet", "Sheet1", "--at", "B", "--count", "1",
		"--out", out,
	)
	require.NoError(t, err)

	var result XLSXStructureMutationResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.NotEmpty(t, result.SheetsListCommand)
	assert.Equal(t, "insert", result.Operation)
	assert.Equal(t, "cols", result.Axis)

	assertXLSXStructureSheetVisibleViaReadback(t, result)
	assertXLSXStructureSheetShowReadback(t, result, false)
}

func TestXLSXColsDeleteReadbackSymmetry(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, plainStructureWorkbookXML)
	out := filepath.Join(t.TempDir(), "cols-delete.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cols", "delete", workbookPath,
		"--sheet", "Sheet1", "--col", "B", "--count", "1",
		"--out", out,
	)
	require.NoError(t, err)

	var result XLSXStructureMutationResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.NotEmpty(t, result.SheetsListCommand)
	assert.Equal(t, "delete", result.Operation)
	assert.Equal(t, "cols", result.Axis)

	assertXLSXStructureSheetVisibleViaReadback(t, result)
	assertXLSXStructureSheetShowReadback(t, result, false)
}

// TestXLSXStructureReadbackDryRunTemplates verifies that a --dry-run run emits
// the *Template form (placeholder-based) rather than concrete commands.
func TestXLSXStructureReadbackDryRunTemplates(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, plainStructureWorkbookXML)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rows", "insert", workbookPath,
		"--sheet", "Sheet1", "--at", "2", "--count", "1",
		"--dry-run",
	)
	require.NoError(t, err)

	var result XLSXStructureMutationResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.True(t, result.DryRun)
	assert.Empty(t, result.Output)
	assert.Empty(t, result.SheetsListCommand)
	assert.Empty(t, result.SheetShowCommand)
	assert.Empty(t, result.ValidateCommand)
	assert.NotEmpty(t, result.SheetsListCommandTemplate)
	assert.NotEmpty(t, result.ValidateCommandTemplate)
	// The sheets-show template must target the real mutated sheet (not a
	// defaulted "1" selector), so a --dry-run caller learns exactly which
	// readback confirms the structural change once they re-run with --out.
	require.NotEmpty(t, result.SheetShowCommandTemplate)
	assert.Contains(t, result.SheetShowCommandTemplate, "xlsx sheets show")
	assert.Contains(t, result.SheetShowCommandTemplate, "--sheet "+result.Sheet)
}

// assertXLSXStructureSheetVisibleViaReadback runs the advertised sheets-list
// readback command against the produced file and asserts the mutated sheet is
// present with the same SheetNumber. XLSXSheetListItem and the structure
// mutation result share only sheet identity (number/name); operation/start/count
// are mutation-only, so we lock parity on the sheet identity the readback proves.
func assertXLSXStructureSheetVisibleViaReadback(t *testing.T, result XLSXStructureMutationResult) {
	t.Helper()
	listJSON := executeGeneratedOOXMLCommandForXLSXTest(t, result.SheetsListCommand)
	var list XLSXSheetsListResult
	require.NoError(t, json.Unmarshal([]byte(listJSON), &list))

	var found *XLSXSheetListItem
	for i := range list.Sheets {
		if list.Sheets[i].Number == result.SheetNumber {
			found = &list.Sheets[i]
			break
		}
	}
	require.NotNil(t, found, "mutated sheet %d must appear in readback listing", result.SheetNumber)
	assert.Equal(t, result.Sheet, found.Name)
}

// assertXLSXStructureSheetShowReadback exercises the sheets-show readback that
// actually confirms the structural mutation. A bare sheets-list cannot reflect
// inserted/deleted rows or columns; sheets-show re-reports the mutated sheet's
// used range. This runs the advertised SheetShowCommand against the produced
// file and asserts (1) the command targets sheets-show for the mutated sheet,
// and (2) the post-mutation used range it reports matches result.NewUsedRange,
// which is the change the readback is supposed to let a caller verify.
func assertXLSXStructureSheetShowReadback(t *testing.T, result XLSXStructureMutationResult, requireUsedRange bool) {
	t.Helper()
	require.NotEmpty(t, result.SheetShowCommand, "must advertise a sheets-show readback command")
	require.Contains(t, result.SheetShowCommand, "xlsx sheets show",
		"structure readback must use sheets show, not a bare sheets list")
	require.Contains(t, result.SheetShowCommand, "--sheet "+result.Sheet,
		"sheets-show readback must target the mutated sheet")

	showJSON := executeGeneratedOOXMLCommandForXLSXTest(t, result.SheetShowCommand)
	var show XLSXSheetsShowResult
	require.NoError(t, json.Unmarshal([]byte(showJSON), &show))
	require.Len(t, show.Sheets, 1, "sheets-show readback must report exactly the mutated sheet")

	shown := show.Sheets[0]
	assert.Equal(t, result.Sheet, shown.Name)
	assert.Equal(t, result.SheetNumber, shown.Number)
	// The used-range parity is the part that actually distinguishes sheets-show
	// from a hollow sheets-list: it proves the readback reflects the structural
	// change. At least the canonical row-insert test forces it to run.
	if requireUsedRange {
		require.NotEmpty(t, result.NewUsedRange,
			"mutation must report a post-mutation used range for the readback to confirm")
	}
	if result.NewUsedRange != "" {
		assert.Equal(t, result.NewUsedRange, shown.UsedRange.Ref,
			"sheets-show readback must reflect the post-mutation used range")
	}
}

func TestDOCXTablesInsertRowReadbackSymmetry(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	out := filepath.Join(t.TempDir(), "tables-insert-row.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "insert-row", documentPath,
		"--table", "1", "--at", "2", "--expect-hash", hash,
		"--out", out,
	)
	require.NoError(t, err)

	var result DOCXTablesInsertRowResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.False(t, result.DryRun)
	require.NotEmpty(t, result.TablesShowCommand)
	require.NotEmpty(t, result.ValidateCommand)

	inspect := docxTableReadbackSummary(t, result.TablesShowCommand)
	assert.Equal(t, result.Table, inspect.Table)
	assert.Equal(t, result.Block, inspect.Block)
	assert.Equal(t, result.ContentHash, inspect.ContentHash)
	assert.Equal(t, result.Rows, inspect.Rows)
	assert.Equal(t, result.Cols, inspect.Cols)
}

func TestDOCXTablesDeleteRowReadbackSymmetry(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	out := filepath.Join(t.TempDir(), "tables-delete-row.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "delete-row", documentPath,
		"--table", "1", "--row", "2", "--expect-hash", hash,
		"--out", out,
	)
	require.NoError(t, err)

	var result DOCXTablesDeleteRowResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.NotEmpty(t, result.TablesShowCommand)

	inspect := docxTableReadbackSummary(t, result.TablesShowCommand)
	assert.Equal(t, result.Table, inspect.Table)
	assert.Equal(t, result.Block, inspect.Block)
	assert.Equal(t, result.ContentHash, inspect.ContentHash)
	assert.Equal(t, result.Rows, inspect.Rows)
}

func TestDOCXTablesSetCellReadbackSymmetry(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	out := filepath.Join(t.TempDir(), "tables-set-cell.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "set-cell", documentPath,
		"--table", "1", "--row", "1", "--col", "2", "--expect-hash", hash,
		"--text", "Realigned",
		"--out", out,
	)
	require.NoError(t, err)

	var result DOCXTablesSetCellResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.NotEmpty(t, result.TablesShowCommand)
	assert.Equal(t, "Realigned", result.Text)

	inspect := docxTableReadbackSummary(t, result.TablesShowCommand)
	assert.Equal(t, result.Table, inspect.Table)
	assert.Equal(t, result.Block, inspect.Block)
	assert.Equal(t, result.ContentHash, inspect.ContentHash)
	// Cells is populated by default in `docx tables show`; the mutated cell text
	// must be visible at the 0-based (row-1, col-1) coordinate.
	require.Greater(t, len(inspect.Cells), result.Row-1)
	require.Greater(t, len(inspect.Cells[result.Row-1]), result.Col-1)
	assert.Equal(t, result.Text, inspect.Cells[result.Row-1][result.Col-1])
}

func TestDOCXTablesClearCellReadbackSymmetry(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	out := filepath.Join(t.TempDir(), "tables-clear-cell.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "clear-cell", documentPath,
		"--table", "1", "--row", "1", "--col", "2", "--expect-hash", hash,
		"--out", out,
	)
	require.NoError(t, err)

	var result DOCXTablesClearCellResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.Equal(t, out, result.Output)
	require.NotEmpty(t, result.TablesShowCommand)

	inspect := docxTableReadbackSummary(t, result.TablesShowCommand)
	assert.Equal(t, result.Table, inspect.Table)
	assert.Equal(t, result.Block, inspect.Block)
	assert.Equal(t, result.ContentHash, inspect.ContentHash)
	require.Greater(t, len(inspect.Cells), result.Row-1)
	require.Greater(t, len(inspect.Cells[result.Row-1]), result.Col-1)
	assert.Empty(t, inspect.Cells[result.Row-1][result.Col-1])
}

// TestDOCXTableReadbackDryRunTemplates verifies the --dry-run template branch.
func TestDOCXTableReadbackDryRunTemplates(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "set-cell", documentPath,
		"--table", "1", "--row", "1", "--col", "2", "--expect-hash", hash,
		"--text", "Realigned",
		"--dry-run",
	)
	require.NoError(t, err)

	var result DOCXTablesSetCellResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	require.True(t, result.DryRun)
	assert.Empty(t, result.Output)
	assert.Empty(t, result.TablesShowCommand)
	assert.Empty(t, result.ValidateCommand)
	assert.NotEmpty(t, result.TablesShowCommandTemplate)
	assert.NotEmpty(t, result.ValidateCommandTemplate)
	assert.NotEmpty(t, result.TablesListCommandTemplate)
}

// docxTableReadbackSummary executes the advertised `docx tables show --table N`
// command and returns the single table summary it yields.
func docxTableReadbackSummary(t *testing.T, command string) DOCXTableSummary {
	t.Helper()
	showJSON := executeGeneratedOOXMLCommandForXLSXTest(t, command)
	var show DOCXTablesShowResult
	require.NoError(t, json.Unmarshal([]byte(showJSON), &show))
	require.Len(t, show.Tables, 1, "scoped tables show must return exactly one table")
	return show.Tables[0]
}

// marshalIntoXLSXRangeBlock round-trips any struct carrying the canonical XLSX
// range data tags into the shared xlsxRangeDataBlock view.
func marshalIntoXLSXRangeBlock(t *testing.T, v any) xlsxRangeDataBlock {
	t.Helper()
	raw, err := json.Marshal(v)
	require.NoError(t, err)
	var block xlsxRangeDataBlock
	require.NoError(t, json.Unmarshal(raw, &block))
	return block
}

func assertXLSXRangeBlockParity(t *testing.T, mutation, inspect xlsxRangeDataBlock) {
	t.Helper()
	assert.Equal(t, inspect.Range, mutation.Range)
	assert.Equal(t, inspect.Rows, mutation.Rows)
	assert.Equal(t, inspect.Cols, mutation.Cols)
	assert.Equal(t, inspect.Values, mutation.Values)
	assert.Equal(t, inspect.Types, mutation.Types)
	assert.Equal(t, inspect.Formulas, mutation.Formulas)
	assert.Equal(t, inspect.StyleIndexes, mutation.StyleIndexes)
	assert.Equal(t, inspect.NumberFormatIDs, mutation.NumberFormatIDs)
	assert.Equal(t, inspect.NumberFormatCodes, mutation.NumberFormatCodes)
	assert.Equal(t, inspect.FormulaCount, mutation.FormulaCount)
}
