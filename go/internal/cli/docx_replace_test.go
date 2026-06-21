package cli

import (
	"encoding/json"
	"path/filepath"
	"testing"
)

func TestDOCXReplaceCommandRegistered(t *testing.T) {
	docx := findSubcommand(GetRootCmd(), "docx")
	if docx == nil {
		t.Fatal("docx command is not registered")
	}
	if replace := findSubcommand(docx, "replace"); replace == nil {
		t.Fatal("docx replace command is not registered")
	}
}

func TestDOCXReplaceAcrossRunsJSONReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("split-runs")
	outPath := filepath.Join(t.TempDir(), "replace.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "replace", documentPath,
		"--find", "hello",
		"--replace", "hi",
		"--expect-count", "2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx replace failed: %v", err)
	}

	var result DOCXReplaceResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal replace JSON: %v\n%s", err, output)
	}
	if result.File != documentPath || result.TotalReplacements != 2 || result.AffectedBlockCount != 2 {
		t.Fatalf("unexpected replace result: %+v", result)
	}
	if len(result.BlockSummaries) != 2 || result.BlockSummaries[0].Kind != "paragraph" {
		t.Fatalf("unexpected block summaries: %+v", result.BlockSummaries)
	}
	if result.BlockSummaries[0].ContentHash == result.BlockSummaries[0].PreviousHash {
		t.Fatalf("content hash should differ from previous: %+v", result.BlockSummaries[0])
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if textResult.Blocks[0].Text != "hi world" || textResult.Blocks[1].Text != "say hi again" {
		t.Fatalf("unexpected readback blocks: %+v", textResult.Blocks)
	}
}

func TestDOCXReplaceTableCellsJSONReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")
	outPath := filepath.Join(t.TempDir(), "replace-table.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "replace", documentPath,
		"--find", "A",
		"--replace", "X",
		"--match-case",
		"--expect-count", "2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx replace table cells failed: %v", err)
	}

	var result DOCXReplaceResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal replace JSON: %v\n%s", err, output)
	}
	if result.TotalReplacements != 2 || result.AffectedBlockCount != 1 || len(result.BlockSummaries) != 2 {
		t.Fatalf("unexpected replace result: %+v", result)
	}
	first := result.BlockSummaries[0]
	if first.Kind != "tableCell" || first.TableIndex != 1 || first.RowIndex != 1 || first.ColumnIndex != 1 || first.ParagraphIndex != 1 {
		t.Fatalf("unexpected first summary: %+v", first)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if textResult.Blocks[0].Text != "X1\tB1\nX2\tB2" {
		t.Fatalf("unexpected table readback: %+v", textResult.Blocks[0])
	}
}

func TestDOCXReplaceExpectCountMismatch(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	args := []string{
		"docx", "replace", documentPath,
		"--find", "text",
		"--replace", "copy",
		"--expect-count", "5",
		"--dry-run",
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestDOCXReplaceDryRunDoesNotWrite(t *testing.T) {
	documentPath := filepath.Join(t.TempDir(), "document.docx")
	if err := copyFile(getDOCXTestFilePath("minimal"), documentPath); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "replace", documentPath,
		"--find", "Hello",
		"--replace", "Goodbye",
		"--match-case",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx replace dry-run failed: %v", err)
	}
	var result DOCXReplaceResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if result.TotalReplacements != 1 {
		t.Fatalf("dry-run count = %d, want 1", result.TotalReplacements)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", documentPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if textResult.Blocks[0].Text != "Hello world" {
		t.Fatalf("dry-run wrote to document: %+v", textResult.Blocks)
	}
}

func TestDOCXReplaceWholeWordAndRegex(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")

	// whole-word "ello" should not match inside "Hello".
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "replace", documentPath,
		"--find", "ello",
		"--replace", "x",
		"--whole-word",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx replace whole-word failed: %v", err)
	}
	var wwResult DOCXReplaceResult
	if err := json.Unmarshal([]byte(output), &wwResult); err != nil {
		t.Fatalf("failed to unmarshal whole-word JSON: %v\n%s", err, output)
	}
	if wwResult.TotalReplacements != 0 {
		t.Fatalf("whole-word count = %d, want 0", wwResult.TotalReplacements)
	}

	// regex "w\w+d" matches "world".
	output, err = executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "replace", documentPath,
		"--find", `w\w+d`,
		"--replace", "planet",
		"--regex",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx replace regex failed: %v", err)
	}
	var reResult DOCXReplaceResult
	if err := json.Unmarshal([]byte(output), &reResult); err != nil {
		t.Fatalf("failed to unmarshal regex JSON: %v\n%s", err, output)
	}
	if reResult.TotalReplacements != 1 || reResult.BlockSummaries[0].Text != "Hello planet" {
		t.Fatalf("unexpected regex result: %+v", reResult)
	}
}

func TestDOCXReplaceRejectsBadArgsAndNonDOCX(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")

	tests := []struct {
		args []string
		code int
	}{
		// empty --find
		{[]string{"docx", "replace", documentPath, "--find", "", "--replace", "x", "--dry-run"}, ExitInvalidArgs},
		// invalid regex
		{[]string{"docx", "replace", documentPath, "--find", "(", "--replace", "x", "--regex", "--dry-run"}, ExitInvalidArgs},
	}
	for _, tt := range tests {
		_, err := executeRootForXLSXTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
	}

	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	args := []string{"docx", "replace", workbookPath, "--find", "x", "--replace", "y", "--dry-run"}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitUnsupportedType)
}
