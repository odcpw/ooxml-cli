package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDOCXTablesShowJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "tables", "show", documentPath)
	if err != nil {
		t.Fatalf("docx tables show failed: %v", err)
	}

	var result DOCXTablesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal tables show JSON: %v\n%s", err, output)
	}
	if len(result.Tables) != 1 {
		t.Fatalf("table count = %d, want 1", len(result.Tables))
	}
	table := result.Tables[0]
	if table.Table != 1 || table.Block != 1 || table.Rows != 2 || table.Cols != 2 || table.Merged {
		t.Fatalf("unexpected table summary: %+v", table)
	}
	if !strings.HasPrefix(table.ContentHash, "sha256:") || len(table.ContentHash) != len("sha256:")+64 {
		t.Fatalf("content hash = %q, want sha256 hex", table.ContentHash)
	}
	if table.PrimarySelector != "1" || !containsString(table.Selectors, "1") {
		t.Fatalf("unexpected table selectors: primary=%q selectors=%+v", table.PrimarySelector, table.Selectors)
	}
	if table.Cells[0][0] != "A1" || table.Cells[1][1] != "B2" {
		t.Fatalf("unexpected cells: %+v", table.Cells)
	}
}

func TestDOCXTablesSetCellJSONReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	outPath := filepath.Join(t.TempDir(), "set-cell.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "set-cell", documentPath,
		"--table", "1",
		"--row", "1",
		"--col", "2",
		"--expect-hash", hash,
		"--text", "Updated\tCell\nLine 2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx tables set-cell failed: %v", err)
	}
	var result DOCXTablesSetCellResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-cell JSON: %v\n%s", err, output)
	}
	if result.PreviousText != "B1" || result.Text != "Updated\tCell\nLine 2" || result.Row != 1 || result.Col != 2 {
		t.Fatalf("unexpected set-cell result: %+v", result)
	}
	if result.Block != 1 || result.PreviousHash != hash || result.ContentHash == hash || !strings.HasPrefix(result.ContentHash, "sha256:") {
		t.Fatalf("unexpected hash metadata: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("mutated DOCX did not validate: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "tables", "show", outPath, "--table", "1")
	if err != nil {
		t.Fatalf("docx tables show readback failed: %v", err)
	}
	var shown DOCXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &shown); err != nil {
		t.Fatalf("failed to unmarshal readback JSON: %v\n%s", err, readback)
	}
	if shown.Tables[0].Cells[0][1] != "Updated\tCell\nLine 2" {
		t.Fatalf("updated cell readback = %+v", shown.Tables[0].Cells)
	}
}

func TestDOCXTablesClearInsertDeleteRow(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	clearPath := filepath.Join(t.TempDir(), "clear-cell.docx")
	insertPath := filepath.Join(t.TempDir(), "insert-row.docx")
	deletePath := filepath.Join(t.TempDir(), "delete-row.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "clear-cell", documentPath,
		"--table", "1",
		"--row", "2",
		"--col", "1",
		"--expect-hash", hash,
		"--out", clearPath,
	)
	if err != nil {
		t.Fatalf("docx tables clear-cell failed: %v", err)
	}
	var clearResult DOCXTablesClearCellResult
	if err := json.Unmarshal([]byte(output), &clearResult); err != nil {
		t.Fatalf("failed to unmarshal clear-cell JSON: %v\n%s", err, output)
	}
	if clearResult.PreviousText != "A2" {
		t.Fatalf("unexpected clear result: %+v", clearResult)
	}
	if clearResult.PreviousHash != hash || clearResult.ContentHash == hash {
		t.Fatalf("unexpected clear hash metadata: %+v", clearResult)
	}

	output, err = executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "insert-row", clearPath,
		"--table", "1",
		"--at", "2",
		"--expect-hash", clearResult.ContentHash,
		"--out", insertPath,
	)
	if err != nil {
		t.Fatalf("docx tables insert-row failed: %v", err)
	}
	var insertResult DOCXTablesInsertRowResult
	if err := json.Unmarshal([]byte(output), &insertResult); err != nil {
		t.Fatalf("failed to unmarshal insert-row JSON: %v\n%s", err, output)
	}
	if insertResult.Rows != 3 || insertResult.Cols != 2 || insertResult.Row != 2 {
		t.Fatalf("unexpected insert-row result: %+v", insertResult)
	}
	if insertResult.PreviousHash != clearResult.ContentHash || insertResult.ContentHash == clearResult.ContentHash {
		t.Fatalf("unexpected insert hash metadata: %+v", insertResult)
	}

	output, err = executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "delete-row", insertPath,
		"--table", "1",
		"--row", "2",
		"--expect-hash", insertResult.ContentHash,
		"--out", deletePath,
	)
	if err != nil {
		t.Fatalf("docx tables delete-row failed: %v", err)
	}
	var deleteResult DOCXTablesDeleteRowResult
	if err := json.Unmarshal([]byte(output), &deleteResult); err != nil {
		t.Fatalf("failed to unmarshal delete-row JSON: %v\n%s", err, output)
	}
	if deleteResult.Rows != 2 || deleteResult.Cols != 2 {
		t.Fatalf("unexpected delete-row result: %+v", deleteResult)
	}
	if deleteResult.PreviousHash != insertResult.ContentHash || deleteResult.ContentHash == insertResult.ContentHash {
		t.Fatalf("unexpected delete hash metadata: %+v", deleteResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", deletePath); err != nil {
		t.Fatalf("mutated DOCX did not validate: %v", err)
	}
}

func TestDOCXTablesDryRunDoesNotWrite(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	hash := docxBlockHashForTest(t, documentPath, 1)
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "tables", "set-cell", documentPath,
		"--table", "1",
		"--row", "1",
		"--col", "1",
		"--expect-hash", hash,
		"--text", "Dry run",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx tables set-cell dry-run failed: %v", err)
	}
	var result DOCXTablesSetCellResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "tables", "show", documentPath, "--table", "1")
	if err != nil {
		t.Fatalf("docx tables show readback failed: %v", err)
	}
	var shown DOCXTablesShowResult
	if err := json.Unmarshal([]byte(readback), &shown); err != nil {
		t.Fatalf("failed to unmarshal readback JSON: %v\n%s", err, readback)
	}
	if shown.Tables[0].Cells[0][0] == "Dry run" {
		t.Fatalf("dry-run wrote to source document")
	}
}

func TestDOCXTablesRejectBadTargetsAndNonDOCX(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	mergedPath := getDOCXTestFilePath("merged-table")
	outPath := filepath.Join(t.TempDir(), "out.docx")
	tableHash := docxBlockHashForTest(t, documentPath, 1)
	mergedHash := docxBlockHashForTest(t, mergedPath, 1)
	wrongHash := "sha256:0000000000000000000000000000000000000000000000000000000000000000"

	for _, tc := range []struct {
		args []string
		code int
	}{
		{[]string{"docx", "tables", "set-cell", documentPath, "--table", "1", "--row", "9", "--col", "1", "--expect-hash", tableHash, "--text", "x", "--out", outPath}, ExitTargetNotFound},
		{[]string{"docx", "tables", "set-cell", documentPath, "--table", "1", "--row", "1", "--col", "1", "--text", "x", "--out", outPath}, ExitInvalidArgs},
		{[]string{"docx", "tables", "set-cell", documentPath, "--table", "1", "--row", "1", "--col", "1", "--expect-hash", wrongHash, "--text", "x", "--out", outPath}, ExitInvalidArgs},
		{[]string{"docx", "tables", "insert-row", mergedPath, "--table", "1", "--at", "2", "--expect-hash", mergedHash, "--out", outPath}, ExitInvalidArgs},
		{[]string{"docx", "tables", "delete-row", mergedPath, "--table", "1", "--row", "1", "--expect-hash", mergedHash, "--out", outPath}, ExitInvalidArgs},
	} {
		_, err := executeRootForXLSXTest(t, tc.args...)
		if err == nil {
			t.Fatalf("%v: expected error", tc.args)
		}
		cliErr, ok := err.(*CLIError)
		if !ok {
			t.Fatalf("%v: error type = %T, want *CLIError", tc.args, err)
		}
		if cliErr.ExitCode != tc.code {
			t.Fatalf("%v: exit code = %d, want %d", tc.args, cliErr.ExitCode, tc.code)
		}
	}

	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	_, err := executeRootForXLSXTest(t, "docx", "tables", "show", workbookPath)
	if err == nil {
		t.Fatal("expected unsupported type error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitUnsupportedType)
	}

	if _, err := os.Stat(outPath); !os.IsNotExist(err) {
		t.Fatalf("unexpected output artifact at %s", outPath)
	}
}
