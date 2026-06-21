package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestPPTXTablesCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	tables := findSubcommand(pptx, "tables")
	if tables == nil {
		t.Fatal("pptx tables command is not registered")
	}
	if show := findSubcommand(tables, "show"); show == nil {
		t.Fatal("pptx tables show command is not registered")
	}
	if setCell := findSubcommand(tables, "set-cell"); setCell == nil {
		t.Fatal("pptx tables set-cell command is not registered")
	}
	if updateFromXLSX := findSubcommand(tables, "update-from-xlsx"); updateFromXLSX == nil {
		t.Fatal("pptx tables update-from-xlsx command is not registered")
	}
	for _, name := range []string{"insert-row", "delete-row", "insert-col", "delete-col"} {
		if command := findSubcommand(tables, name); command == nil {
			t.Fatalf("pptx tables %s command is not registered", name)
		}
	}
}

func TestPPTXTablesShowJSON(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", presentationPath,
		"--slide", "2",
	)
	if err != nil {
		t.Fatalf("pptx tables show failed: %v", err)
	}

	var result PPTXTablesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal tables show JSON: %v\n%s", err, output)
	}
	if result.File != presentationPath || result.Slide != 2 {
		t.Fatalf("unexpected show result metadata: %+v", result)
	}
	if len(result.Tables) != 1 {
		t.Fatalf("table count = %d, want 1", len(result.Tables))
	}
	table := result.Tables[0]
	if table.ShapeID != 2 || table.ShapeName != "Table 1" || table.Rows != 3 || table.Cols != 3 {
		t.Fatalf("unexpected table summary: %+v", table)
	}
	if table.TargetKind != "table" || table.PrimarySelector != "table:1" {
		t.Fatalf("unexpected table selectors: %+v", table)
	}
	for _, selector := range []string{"table:1", "shape:2", "~Table 1"} {
		if !containsString(table.Selectors, selector) {
			t.Fatalf("table selectors = %+v, want %s", table.Selectors, selector)
		}
	}
	if table.Cells[1][1] != "R1C1" {
		t.Fatalf("cell readback = %q, want R1C1", table.Cells[1][1])
	}
}

func TestPPTXTablesTargetSelectors(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "target-set-cell.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "set-cell", presentationPath,
		"--slide", "2",
		"--target", "table:1",
		"--row", "2",
		"--col", "2",
		"--text", "Selector Updated",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables set-cell by target failed: %v", err)
	}
	var result PPTXTablesSetCellResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-cell JSON: %v\n%s", err, output)
	}
	if result.TableID != 2 || result.PreviousText != "R1C1" {
		t.Fatalf("unexpected target set-cell result: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, outPath, 3, 3)
	if got := result.Destination.Cells[1][1]; got != "Selector Updated" {
		t.Fatalf("destination cell readback = %q, want Selector Updated", got)
	}

	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", outPath,
		"--slide", "2",
		"--target", "~Table 1",
	)
	if err != nil {
		t.Fatalf("pptx tables show by target failed: %v", err)
	}
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if len(showResult.Tables) != 1 || showResult.Tables[0].Cells[1][1] != "Selector Updated" {
		t.Fatalf("unexpected target readback: %+v", showResult.Tables)
	}
}

func TestPPTXTablesSetCellJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "set-cell.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "set-cell", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "2",
		"--col", "2",
		"--text", "Updated",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables set-cell failed: %v", err)
	}

	var result PPTXTablesSetCellResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-cell JSON: %v\n%s", err, output)
	}
	if result.TableID != 2 || result.Row != 2 || result.Col != 2 || result.Text != "Updated" || result.PreviousText != "R1C1" {
		t.Fatalf("unexpected set-cell result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected set-cell mutation metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, outPath, 3, 3)
	if got := result.Destination.Cells[1][1]; got != "Updated" {
		t.Fatalf("destination updated cell = %q, want Updated", got)
	}
	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if got := showResult.Tables[0].Cells[1][1]; got != "Updated" {
		t.Fatalf("updated cell readback = %q", got)
	}
	if got := showResult.Tables[0].Cells[1][0]; got != "R1C0" {
		t.Fatalf("neighbor cell changed: %q", got)
	}
}

func TestPPTXTablesSetCellDryRunDoesNotWrite(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "set-cell", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "2",
		"--col", "2",
		"--text", "Dry Run",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx tables set-cell dry-run failed: %v", err)
	}
	var result PPTXTablesSetCellResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run set-cell JSON: %v\n%s", err, output)
	}
	if result.Text != "Dry Run" || result.PreviousText != "R1C1" {
		t.Fatalf("unexpected set-cell dry-run result: %+v", result)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected set-cell dry-run metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, "", 3, 3)
	if got := result.Destination.Cells[1][1]; got != "Dry Run" {
		t.Fatalf("dry-run destination cell = %q, want Dry Run", got)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx tables show")

	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", presentationPath,
		"--slide", "2",
		"--table-id", "2",
	)
	if err != nil {
		t.Fatalf("pptx tables show readback failed: %v", err)
	}
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if got := showResult.Tables[0].Cells[1][1]; got != "R1C1" {
		t.Fatalf("dry-run wrote to presentation, readback = %q", got)
	}
}

func TestPPTXTablesSetCellInPlaceJSONReadback(t *testing.T) {
	sourcePath := getPPTXTableTestFilePath("table-slide")
	presentationPath := filepath.Join(t.TempDir(), "presentation.pptx")
	data, err := os.ReadFile(sourcePath)
	if err != nil {
		t.Fatalf("failed to read source presentation: %v", err)
	}
	if err := os.WriteFile(presentationPath, data, 0o644); err != nil {
		t.Fatalf("failed to write temp presentation: %v", err)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "set-cell", presentationPath,
		"--slide", "2",
		"--target", "table:1",
		"--row", "2",
		"--col", "2",
		"--text", "In Place",
		"--in-place",
	)
	if err != nil {
		t.Fatalf("pptx tables set-cell in-place failed: %v", err)
	}
	var result PPTXTablesSetCellResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-cell in-place JSON: %v\n%s", err, output)
	}
	if result.Output != presentationPath || result.DryRun {
		t.Fatalf("unexpected set-cell in-place metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, presentationPath, 3, 3)
	if got := result.Destination.Cells[1][1]; got != "In Place" {
		t.Fatalf("in-place destination cell = %q, want In Place", got)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", presentationPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	readback := readPPTXTableSummaryForTest(t, presentationPath)
	if got := readback.Cells[1][1]; got != "In Place" {
		t.Fatalf("in-place readback cell = %q, want In Place", got)
	}
}

func TestPPTXTablesSetCellTextFileAndClear(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "set-cell-file.pptx")
	textPath := filepath.Join(t.TempDir(), "cell.txt")
	if err := os.WriteFile(textPath, []byte("line 1\nline 2"), 0o644); err != nil {
		t.Fatalf("failed to write text file: %v", err)
	}

	if _, err := executeRootForXLSXTest(t,
		"pptx", "tables", "set-cell", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "1",
		"--col", "1",
		"--text-file", textPath,
		"--out", outPath,
	); err != nil {
		t.Fatalf("pptx tables set-cell text-file failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", outPath,
		"--slide", "2",
		"--table-id", "2",
	)
	if err != nil {
		t.Fatalf("pptx tables show readback failed: %v", err)
	}
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if got := showResult.Tables[0].Cells[0][0]; got != "line 1\nline 2" {
		t.Fatalf("updated cell readback = %q", got)
	}

	clearPath := filepath.Join(t.TempDir(), "clear-cell.pptx")
	if _, err := executeRootForXLSXTest(t,
		"pptx", "tables", "set-cell", outPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "1",
		"--col", "1",
		"--text", "",
		"--out", clearPath,
	); err != nil {
		t.Fatalf("pptx tables set-cell clear failed: %v", err)
	}
}

func TestPPTXTablesInsertRowJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "insert-row.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "insert-row", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--at", "1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables insert-row failed: %v", err)
	}

	var result PPTXTablesInsertRowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal insert-row JSON: %v\n%s", err, output)
	}
	if result.TableID != 2 || result.At != 1 || result.Rows != 4 || result.Cols != 3 || result.CellCount != 3 {
		t.Fatalf("unexpected insert-row result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected insert-row mutation metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, outPath, 4, 3)
	if got := result.Destination.Cells[0][0]; got != "" {
		t.Fatalf("destination inserted row first cell = %q, want empty", got)
	}
	if got := result.Destination.Cells[1][1]; got != "R0C1" {
		t.Fatalf("destination shifted row readback = %q, want R0C1", got)
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")

	table := readPPTXTableSummaryForTest(t, outPath)
	if table.Rows != 4 || table.Cols != 3 {
		t.Fatalf("readback dimensions = %dx%d, want 4x3", table.Rows, table.Cols)
	}
	if got := table.Cells[0][0]; got != "" {
		t.Fatalf("inserted row first cell = %q, want empty", got)
	}
	if got := table.Cells[1][1]; got != "R0C1" {
		t.Fatalf("shifted row readback = %q, want R0C1", got)
	}
}

func TestPPTXTablesDeleteRowJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "delete-row.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "delete-row", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables delete-row failed: %v", err)
	}

	var result PPTXTablesDeleteRowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal delete-row JSON: %v\n%s", err, output)
	}
	if result.TableID != 2 || result.Row != 2 || result.Rows != 2 || result.Cols != 3 || result.CellCount != 3 {
		t.Fatalf("unexpected delete-row result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected delete-row mutation metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, outPath, 2, 3)
	if got := result.Destination.Cells[1][1]; got != "R2C1" {
		t.Fatalf("destination row after deletion = %q, want R2C1", got)
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")

	table := readPPTXTableSummaryForTest(t, outPath)
	if table.Rows != 2 || table.Cols != 3 {
		t.Fatalf("readback dimensions = %dx%d, want 2x3", table.Rows, table.Cols)
	}
	if got := table.Cells[1][1]; got != "R2C1" {
		t.Fatalf("row after deletion = %q, want R2C1", got)
	}
}

func TestPPTXTablesInsertColJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "insert-col.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "insert-col", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--at", "1",
		"--width-emu", "1234567",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables insert-col failed: %v", err)
	}

	var result PPTXTablesInsertColResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal insert-col JSON: %v\n%s", err, output)
	}
	if result.TableID != 2 || result.At != 1 || result.Rows != 3 || result.Cols != 4 || result.RowCount != 3 || result.WidthEMU != 1234567 {
		t.Fatalf("unexpected insert-col result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected insert-col mutation metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, outPath, 3, 4)
	if got := result.Destination.Cells[1][0]; got != "" {
		t.Fatalf("destination inserted column cell = %q, want empty", got)
	}
	if got := result.Destination.Cells[1][1]; got != "R1C0" {
		t.Fatalf("destination shifted column readback = %q, want R1C0", got)
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")

	table := readPPTXTableSummaryForTest(t, outPath)
	if table.Rows != 3 || table.Cols != 4 {
		t.Fatalf("readback dimensions = %dx%d, want 3x4", table.Rows, table.Cols)
	}
	if got := table.Cells[1][0]; got != "" {
		t.Fatalf("inserted column cell = %q, want empty", got)
	}
	if got := table.Cells[1][1]; got != "R1C0" {
		t.Fatalf("shifted column readback = %q, want R1C0", got)
	}
}

func TestPPTXTablesDeleteColJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "delete-col.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "delete-col", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--col", "2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx tables delete-col failed: %v", err)
	}

	var result PPTXTablesDeleteColResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal delete-col JSON: %v\n%s", err, output)
	}
	if result.TableID != 2 || result.Col != 2 || result.Rows != 3 || result.Cols != 2 || result.RowCount != 3 {
		t.Fatalf("unexpected delete-col result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected delete-col mutation metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, outPath, 3, 2)
	if got := result.Destination.Cells[1][1]; got != "R1C2" {
		t.Fatalf("destination column after deletion = %q, want R1C2", got)
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx tables show")

	table := readPPTXTableSummaryForTest(t, outPath)
	if table.Rows != 3 || table.Cols != 2 {
		t.Fatalf("readback dimensions = %dx%d, want 3x2", table.Rows, table.Cols)
	}
	if got := table.Cells[1][1]; got != "R1C2" {
		t.Fatalf("column after deletion = %q, want R1C2", got)
	}
}

func TestPPTXTablesInsertRowAppendBoundary(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	outPath := filepath.Join(t.TempDir(), "append-row.pptx")

	if _, err := executeRootForXLSXTest(t,
		"pptx", "tables", "insert-row", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--at", "4",
		"--out", outPath,
	); err != nil {
		t.Fatalf("pptx tables insert-row append failed: %v", err)
	}

	table := readPPTXTableSummaryForTest(t, outPath)
	if table.Rows != 4 || table.Cells[2][1] != "R2C1" || table.Cells[3][1] != "" {
		t.Fatalf("unexpected append readback: %+v", table.Cells)
	}

	err := expectPPTXTablesCLIError(t,
		[]string{"pptx", "tables", "insert-row", presentationPath, "--slide", "2", "--table-id", "2", "--at", "5", "--dry-run"},
		ExitTargetNotFound,
	)
	if err == nil {
		t.Fatal("expected append out-of-range error")
	}
}

func TestPPTXTablesRowColDryRunDoesNotWrite(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "insert-col", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--at", "1",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx tables insert-col dry-run failed: %v", err)
	}
	var result PPTXTablesInsertColResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run insert-col JSON: %v\n%s", err, output)
	}
	if result.Cols != 4 {
		t.Fatalf("dry-run result cols = %d, want 4", result.Cols)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected insert-col dry-run metadata: %+v", result)
	}
	assertPPTXTableMutationDestinationForTest(t, result.Destination, "", 3, 4)
	if got := result.Destination.Cells[1][0]; got != "" {
		t.Fatalf("dry-run destination inserted column cell = %q, want empty", got)
	}
	if got := result.Destination.Cells[1][1]; got != "R1C0" {
		t.Fatalf("dry-run destination shifted column cell = %q, want R1C0", got)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx tables show")

	table := readPPTXTableSummaryForTest(t, presentationPath)
	if table.Rows != 3 || table.Cols != 3 || table.Cells[0][0] != "R0C0" {
		t.Fatalf("dry-run wrote to presentation: %+v", table)
	}
}

func TestPPTXTablesMutationTextOutput(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	tmpDir := t.TempDir()

	setCellPath := filepath.Join(tmpDir, "set-cell.pptx")
	output, err := executeRootForXLSXTest(t,
		"pptx", "tables", "set-cell", presentationPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "1",
		"--col", "1",
		"--text", "Text",
		"--out", setCellPath,
	)
	if err != nil {
		t.Fatalf("pptx tables set-cell text failed: %v", err)
	}
	if output != "set slide 2 table 2 cell R1C1 = \"Text\"\n" {
		t.Fatalf("unexpected set-cell text output: %q", output)
	}

	insertRowPath := filepath.Join(tmpDir, "insert-row.pptx")
	output, err = executeRootForXLSXTest(t,
		"pptx", "tables", "insert-row", setCellPath,
		"--slide", "2",
		"--table-id", "2",
		"--at", "1",
		"--out", insertRowPath,
	)
	if err != nil {
		t.Fatalf("pptx tables insert-row text failed: %v", err)
	}
	if output != "inserted slide 2 table 2 row at 1; table is now 4x3\n" {
		t.Fatalf("unexpected insert-row text output: %q", output)
	}

	deleteRowPath := filepath.Join(tmpDir, "delete-row.pptx")
	output, err = executeRootForXLSXTest(t,
		"pptx", "tables", "delete-row", insertRowPath,
		"--slide", "2",
		"--table-id", "2",
		"--row", "1",
		"--out", deleteRowPath,
	)
	if err != nil {
		t.Fatalf("pptx tables delete-row text failed: %v", err)
	}
	if output != "deleted slide 2 table 2 row 1; table is now 3x3\n" {
		t.Fatalf("unexpected delete-row text output: %q", output)
	}

	insertColPath := filepath.Join(tmpDir, "insert-col.pptx")
	output, err = executeRootForXLSXTest(t,
		"pptx", "tables", "insert-col", deleteRowPath,
		"--slide", "2",
		"--table-id", "2",
		"--at", "1",
		"--out", insertColPath,
	)
	if err != nil {
		t.Fatalf("pptx tables insert-col text failed: %v", err)
	}
	if output != "inserted slide 2 table 2 column at 1; table is now 3x4\n" {
		t.Fatalf("unexpected insert-col text output: %q", output)
	}

	deleteColPath := filepath.Join(tmpDir, "delete-col.pptx")
	output, err = executeRootForXLSXTest(t,
		"pptx", "tables", "delete-col", insertColPath,
		"--slide", "2",
		"--table-id", "2",
		"--col", "1",
		"--out", deleteColPath,
	)
	if err != nil {
		t.Fatalf("pptx tables delete-col text failed: %v", err)
	}
	if output != "deleted slide 2 table 2 column 1; table is now 3x3\n" {
		t.Fatalf("unexpected delete-col text output: %q", output)
	}
}

func TestPPTXTablesRowColRejectMergedTables(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-merged")
	tests := [][]string{
		{"pptx", "tables", "delete-row", presentationPath, "--slide", "2", "--table-id", "2", "--row", "2", "--dry-run"},
		{"pptx", "tables", "delete-col", presentationPath, "--slide", "2", "--table-id", "2", "--col", "1", "--dry-run"},
		{"pptx", "tables", "insert-row", presentationPath, "--slide", "2", "--table-id", "2", "--at", "3", "--dry-run"},
		{"pptx", "tables", "insert-col", presentationPath, "--slide", "2", "--table-id", "2", "--at", "2", "--dry-run"},
	}
	for _, args := range tests {
		expectPPTXTablesCLIError(t, args, ExitInvalidArgs)
	}
}

func TestPPTXTablesRejectBadArgsAndTargets(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	textPath := filepath.Join(t.TempDir(), "cell.txt")
	if err := os.WriteFile(textPath, []byte("x"), 0o644); err != nil {
		t.Fatalf("failed to write text file: %v", err)
	}

	tests := []struct {
		args []string
		code int
	}{
		{[]string{"pptx", "tables", "show", presentationPath, "--slide", "0"}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "show", presentationPath, "--slide", "2", "--table-id", "99"}, ExitTargetNotFound},
		{[]string{"pptx", "tables", "show", presentationPath, "--slide", "2", "--table-id", "2", "--target", "table:1"}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "show", presentationPath, "--slide", "2", "--target", "title"}, ExitTargetNotFound},
		{[]string{"pptx", "tables", "set-cell", presentationPath, "--slide", "2", "--table-id", "2", "--row", "0", "--col", "1", "--text", "x", "--out", filepath.Join(t.TempDir(), "bad-row.pptx")}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "set-cell", presentationPath, "--slide", "2", "--target", "table:99", "--row", "1", "--col", "1", "--text", "x", "--out", filepath.Join(t.TempDir(), "missing-target.pptx")}, ExitTargetNotFound},
		{[]string{"pptx", "tables", "set-cell", filepath.Join(getTestdataPath(), "pptx", "title-content", "presentation.pptx"), "--slide", "2", "--target", "body", "--row", "1", "--col", "1", "--text", "x", "--out", filepath.Join(t.TempDir(), "non-table-target.pptx")}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "set-cell", presentationPath, "--slide", "2", "--row", "1", "--col", "1", "--text", "x", "--out", filepath.Join(t.TempDir(), "missing-target-flag.pptx")}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "set-cell", presentationPath, "--slide", "2", "--table-id", "2", "--row", "1", "--col", "1", "--out", filepath.Join(t.TempDir(), "missing-text.pptx")}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "set-cell", presentationPath, "--slide", "2", "--table-id", "2", "--row", "1", "--col", "1", "--text", "x", "--text-file", textPath, "--out", filepath.Join(t.TempDir(), "both-text.pptx")}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "set-cell", presentationPath, "--slide", "2", "--table-id", "2", "--row", "9", "--col", "1", "--text", "x", "--out", filepath.Join(t.TempDir(), "missing-cell.pptx")}, ExitTargetNotFound},
		{[]string{"pptx", "tables", "insert-row", presentationPath, "--slide", "2", "--table-id", "2", "--at", "0", "--dry-run"}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "insert-row", presentationPath, "--slide", "2", "--table-id", "99", "--at", "1", "--dry-run"}, ExitTargetNotFound},
		{[]string{"pptx", "tables", "delete-row", presentationPath, "--slide", "2", "--table-id", "2", "--row", "99", "--dry-run"}, ExitTargetNotFound},
		{[]string{"pptx", "tables", "insert-col", presentationPath, "--slide", "2", "--table-id", "2", "--at", "1", "--width-emu", "-1", "--dry-run"}, ExitInvalidArgs},
		{[]string{"pptx", "tables", "delete-col", presentationPath, "--slide", "2", "--table-id", "2", "--col", "99", "--dry-run"}, ExitTargetNotFound},
	}
	for _, tt := range tests {
		_, err := executeRootForXLSXTest(t, tt.args...)
		if err == nil {
			t.Fatalf("%v: expected error", tt.args)
		}
		cliErr, ok := err.(*CLIError)
		if !ok {
			t.Fatalf("%v: error type = %T, want *CLIError", tt.args, err)
		}
		if cliErr.ExitCode != tt.code {
			t.Fatalf("%v: exit code = %d, want %d", tt.args, cliErr.ExitCode, tt.code)
		}
	}
}

func TestPPTXTablesRejectNonPPTX(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	tests := [][]string{
		{"pptx", "tables", "show", workbookPath, "--slide", "1"},
		{"pptx", "tables", "insert-row", workbookPath, "--slide", "1", "--table-id", "1", "--at", "1", "--dry-run"},
		{"pptx", "tables", "delete-row", workbookPath, "--slide", "1", "--table-id", "1", "--row", "1", "--dry-run"},
		{"pptx", "tables", "insert-col", workbookPath, "--slide", "1", "--table-id", "1", "--at", "1", "--dry-run"},
		{"pptx", "tables", "delete-col", workbookPath, "--slide", "1", "--table-id", "1", "--col", "1", "--dry-run"},
		{"pptx", "tables", "update-from-xlsx", workbookPath, "--slide", "1", "--table-id", "1", "--workbook", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--dry-run"},
	}
	for _, args := range tests {
		expectPPTXTablesCLIError(t, args, ExitUnsupportedType)
	}
}

func getPPTXTableTestFilePath(fixtureDir string) string {
	return filepath.Join(getTestdataPath(), "pptx", fixtureDir, "presentation.pptx")
}

func readPPTXTableSummaryForTest(t *testing.T, filePath string) PPTXTableSummary {
	t.Helper()
	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "tables", "show", filePath,
		"--slide", "2",
		"--table-id", "2",
	)
	if err != nil {
		t.Fatalf("pptx tables show readback failed: %v", err)
	}
	var showResult PPTXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, readback)
	}
	if len(showResult.Tables) != 1 {
		t.Fatalf("readback table count = %d, want 1", len(showResult.Tables))
	}
	return showResult.Tables[0]
}

func assertPPTXTableMutationDestinationForTest(t *testing.T, destination *PPTXTableSummary, filePath string, rows, cols int) {
	t.Helper()
	if destination == nil {
		t.Fatal("expected table mutation destination readback")
	}
	if destination.File != filePath {
		t.Fatalf("destination file = %q, want %q", destination.File, filePath)
	}
	if destination.Slide != 2 || destination.ShapeID != 2 || destination.ShapeName != "Table 1" {
		t.Fatalf("unexpected destination table identity: %+v", destination)
	}
	if destination.TargetKind != "table" || destination.PrimarySelector != "table:1" {
		t.Fatalf("unexpected destination selectors: %+v", destination)
	}
	for _, selector := range []string{"table:1", "shape:2", "~Table 1"} {
		if !containsString(destination.Selectors, selector) {
			t.Fatalf("destination selectors missing %s: %+v", selector, destination.Selectors)
		}
	}
	if destination.Rows != rows || destination.Cols != cols {
		t.Fatalf("destination dimensions = %dx%d, want %dx%d", destination.Rows, destination.Cols, rows, cols)
	}
	if len(destination.Cells) != rows {
		t.Fatalf("destination row count = %d, want %d: %+v", len(destination.Cells), rows, destination.Cells)
	}
	for index, row := range destination.Cells {
		if len(row) != cols {
			t.Fatalf("destination row %d column count = %d, want %d: %+v", index, len(row), cols, row)
		}
	}
}

func expectPPTXTablesCLIError(t *testing.T, args []string, code int) error {
	t.Helper()
	_, err := executeRootForXLSXTest(t, args...)
	if err == nil {
		t.Fatalf("%v: expected error", args)
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("%v: error type = %T, want *CLIError", args, err)
	}
	if cliErr.ExitCode != code {
		t.Fatalf("%v: exit code = %d, want %d", args, cliErr.ExitCode, code)
	}
	return err
}
