package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDOCXCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()

	docx := findSubcommand(cmd, "docx")
	if docx == nil {
		t.Fatal("docx command is not registered")
	}
	if text := findSubcommand(docx, "text"); text == nil {
		t.Fatal("docx text command is not registered")
	}
	if blocks := findSubcommand(docx, "blocks"); blocks == nil {
		t.Fatal("docx blocks command is not registered")
	} else {
		for _, name := range []string{"replace", "delete", "insert-after"} {
			if command := findSubcommand(blocks, name); command == nil {
				t.Fatalf("docx blocks %s command is not registered", name)
			}
		}
	}
	paragraphs := findSubcommand(docx, "paragraphs")
	if paragraphs == nil {
		t.Fatal("docx paragraphs command is not registered")
	}
	if set := findSubcommand(paragraphs, "set"); set == nil {
		t.Fatal("docx paragraphs set command is not registered")
	}
	if clear := findSubcommand(paragraphs, "clear"); clear == nil {
		t.Fatal("docx paragraphs clear command is not registered")
	}
	if appendCmd := findSubcommand(paragraphs, "append"); appendCmd == nil {
		t.Fatal("docx paragraphs append command is not registered")
	}
	if insert := findSubcommand(paragraphs, "insert"); insert == nil {
		t.Fatal("docx paragraphs insert command is not registered")
	}
	tables := findSubcommand(docx, "tables")
	if tables == nil {
		t.Fatal("docx tables command is not registered")
	}
	for _, name := range []string{"show", "set-cell", "clear-cell", "insert-row", "delete-row"} {
		if command := findSubcommand(tables, name); command == nil {
			t.Fatalf("docx tables %s command is not registered", name)
		}
	}
}

func TestInspectDispatchesDOCXJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")

	output, err := executeRootForXLSXTest(t, "--format", "json", "inspect", documentPath)
	if err != nil {
		t.Fatalf("inspect failed: %v", err)
	}

	var result DOCXInspectResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal DOCX inspect JSON: %v\n%s", err, output)
	}
	if result.Type != "docx" {
		t.Fatalf("type = %q, want docx", result.Type)
	}
	if result.Summary == nil || result.Summary.Paragraphs != 1 || result.Summary.Sections != 1 {
		t.Fatalf("summary = %+v, want one paragraph and one section", result.Summary)
	}
}

func TestDOCXTextJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", documentPath)
	if err != nil {
		t.Fatalf("docx text failed: %v", err)
	}

	var result DOCXTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal DOCX text JSON: %v\n%s", err, output)
	}
	if len(result.Blocks) != 2 {
		t.Fatalf("block count = %d, want 2", len(result.Blocks))
	}
	if result.Blocks[0].Style != "Heading1" || result.Blocks[0].Text != "Heading Text" {
		t.Fatalf("first block = %+v, want Heading1 text", result.Blocks[0])
	}
}

func TestDOCXTextTextOutputHandlesTableAndWhitespace(t *testing.T) {
	tablePath := getDOCXTestFilePath("table")
	output, err := executeRootForXLSXTest(t, "docx", "text", tablePath)
	if err != nil {
		t.Fatalf("docx text failed: %v", err)
	}
	if !strings.Contains(output, "A1\tB1\nA2\tB2") {
		t.Fatalf("table text output = %q, want tab-separated rows", output)
	}

	spacePath := getDOCXTestFilePath("space-preserve")
	output, err = executeRootForXLSXTest(t, "docx", "text", spacePath)
	if err != nil {
		t.Fatalf("docx text failed: %v", err)
	}
	if !strings.Contains(output, " pad \ttabbed\nline") {
		t.Fatalf("space text output = %q, want preserved spaces, tab, and break", output)
	}
}

func TestDOCXTextRejectsNonDOCX(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	_, err := executeRootForXLSXTest(t, "docx", "text", workbookPath)
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
}

func TestDOCXBlocksJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("mixed-blocks")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", documentPath,
		"--block", "2",
		"--include-runs",
	)
	if err != nil {
		t.Fatalf("docx blocks failed: %v", err)
	}

	var result DOCXBlocksResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal DOCX blocks JSON: %v\n%s", err, output)
	}
	if result.File != documentPath || result.DocumentPartURI != "/word/document.xml" {
		t.Fatalf("unexpected result metadata: %+v", result)
	}
	if len(result.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(result.Blocks))
	}
	block := result.Blocks[0]
	if block.ID != "body.b2" || block.Index != 2 || block.Kind != "paragraph" || block.Text != "Bold heading" {
		t.Fatalf("unexpected block: %+v", block)
	}
	if !strings.HasPrefix(block.ContentHash, "sha256:") || len(block.ContentHash) != len("sha256:")+64 {
		t.Fatalf("content hash = %q, want sha256 hex", block.ContentHash)
	}
	if block.PrimarySelector != "2" || !containsString(block.Selectors, "2") {
		t.Fatalf("unexpected block selectors: primary=%q selectors=%+v", block.PrimarySelector, block.Selectors)
	}
	if block.Paragraph == nil || block.Paragraph.Style != "Heading1" || len(block.Paragraph.Runs) != 1 || !block.Paragraph.Runs[0].Bold {
		t.Fatalf("paragraph info = %+v, want Heading1 with bold run", block.Paragraph)
	}
}

func TestDOCXBlocksTextOutputIncludesTables(t *testing.T) {
	documentPath := getDOCXTestFilePath("table")

	output, err := executeRootForXLSXTest(t, "docx", "blocks", documentPath)
	if err != nil {
		t.Fatalf("docx blocks failed: %v", err)
	}
	if !strings.Contains(output, "body.b1 [1] table sha256:") || !strings.Contains(output, "A1\tB1\nA2\tB2") {
		t.Fatalf("blocks text output = %q, want table block and flattened text", output)
	}
}

func TestDOCXBlocksRejectsBadBlockAndNonDOCX(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	_, err := executeRootForXLSXTest(t, "docx", "blocks", documentPath, "--block", "99")
	if err == nil {
		t.Fatal("expected missing block error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitTargetNotFound)
	}

	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	_, err = executeRootForXLSXTest(t, "docx", "blocks", workbookPath)
	if err == nil {
		t.Fatal("expected unsupported type error")
	}
	cliErr, ok = err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitUnsupportedType)
	}
}

func TestDOCXBlocksReplaceHashGuardedReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	outPath := filepath.Join(t.TempDir(), "blocks-replace.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", "replace", documentPath,
		"--block", "1",
		"--expect-hash", hash,
		"--text", "Hash-guarded heading",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx blocks replace failed: %v", err)
	}

	var result DOCXBlockParagraphResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal blocks replace JSON: %v\n%s", err, output)
	}
	if result.Index != 1 || result.BlockID != "body.b1" || result.ContentHash == "" || result.ContentHash == hash || result.PreviousKind != "paragraph" || result.PreviousHash != hash || result.PreviousText != "Heading Text" || result.Style != "Heading1" {
		t.Fatalf("unexpected replace result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "blocks", outPath, "--block", "1")
	if err != nil {
		t.Fatalf("docx blocks readback failed: %v", err)
	}
	var blocks DOCXBlocksResult
	if err := json.Unmarshal([]byte(readback), &blocks); err != nil {
		t.Fatalf("failed to unmarshal blocks readback: %v\n%s", err, readback)
	}
	if blocks.Blocks[0].Text != "Hash-guarded heading" || blocks.Blocks[0].Paragraph == nil || blocks.Blocks[0].Paragraph.Style != "Heading1" {
		t.Fatalf("unexpected replace readback: %+v", blocks.Blocks[0])
	}
}

func TestDOCXBlocksDeleteTableHashGuarded(t *testing.T) {
	documentPath := getDOCXTestFilePath("mixed-blocks")
	outPath := filepath.Join(t.TempDir(), "blocks-delete.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", "delete", documentPath,
		"--block", "1",
		"--expect-hash", hash,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx blocks delete failed: %v", err)
	}
	var result DOCXBlockDeleteResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal blocks delete JSON: %v\n%s", err, output)
	}
	if result.Index != 1 || result.BlockID != "body.b1" || result.PreviousKind != "table" || result.PreviousHash != hash || result.PreviousText != "Cell text" {
		t.Fatalf("unexpected delete result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "blocks", outPath)
	if err != nil {
		t.Fatalf("docx blocks readback failed: %v", err)
	}
	var blocks DOCXBlocksResult
	if err := json.Unmarshal([]byte(readback), &blocks); err != nil {
		t.Fatalf("failed to unmarshal blocks readback: %v\n%s", err, readback)
	}
	if len(blocks.Blocks) != 3 || blocks.Blocks[0].Kind != "paragraph" || blocks.Blocks[0].Text != "Bold heading" {
		t.Fatalf("unexpected delete readback: %+v", blocks.Blocks)
	}
}

func TestDOCXBlocksInsertAfterHashGuarded(t *testing.T) {
	documentPath := getDOCXTestFilePath("mixed-blocks")
	outPath := filepath.Join(t.TempDir(), "blocks-insert.docx")
	hash := docxBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", "insert-after", documentPath,
		"--block", "1",
		"--expect-hash", hash,
		"--text", "Inserted after table",
		"--style", "Heading1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx blocks insert-after failed: %v", err)
	}
	var result DOCXBlockParagraphResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal blocks insert-after JSON: %v\n%s", err, output)
	}
	if result.Index != 2 || result.BlockID != "body.b2" || result.ContentHash == "" || result.InsertAfter != 1 || result.AnchorHash != hash || result.Style != "Heading1" || result.Text != "Inserted after table" {
		t.Fatalf("unexpected insert-after result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "blocks", outPath, "--block", "2")
	if err != nil {
		t.Fatalf("docx blocks readback failed: %v", err)
	}
	var blocks DOCXBlocksResult
	if err := json.Unmarshal([]byte(readback), &blocks); err != nil {
		t.Fatalf("failed to unmarshal blocks readback: %v\n%s", err, readback)
	}
	if blocks.Blocks[0].Text != "Inserted after table" || blocks.Blocks[0].Paragraph == nil || blocks.Blocks[0].Paragraph.Style != "Heading1" {
		t.Fatalf("unexpected insert readback: %+v", blocks.Blocks[0])
	}
}

func TestDOCXBlocksMutationsRejectBadHashesAndLastDelete(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	args := []string{
		"docx", "blocks", "replace", documentPath,
		"--block", "1",
		"--expect-hash", "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		"--text", "stale",
		"--dry-run",
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)

	args = []string{
		"docx", "blocks", "delete", documentPath,
		"--block", "1",
		"--expect-hash", "sha256:nothex",
		"--dry-run",
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)

	args = []string{
		"docx", "blocks", "delete", documentPath,
		"--block", "1",
		"--dry-run",
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)

	sectionPath := getDOCXTestFilePath("mixed-blocks")
	args = []string{
		"docx", "blocks", "replace", sectionPath,
		"--block", "3",
		"--expect-hash", docxBlockHashForTest(t, sectionPath, 3),
		"--text", "section unsafe",
		"--dry-run",
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)

	minimalPath := getDOCXTestFilePath("minimal")
	args = []string{
		"docx", "blocks", "delete", minimalPath,
		"--block", "1",
		"--expect-hash", docxBlockHashForTest(t, minimalPath, 1),
		"--dry-run",
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestDOCXParagraphsSetJSONAndReadback(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	outPath := filepath.Join(t.TempDir(), "set.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "set", documentPath,
		"--index", "1",
		"--text", "Updated Heading",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx paragraphs set failed: %v", err)
	}

	var result DOCXParagraphsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set JSON: %v\n%s", err, output)
	}
	if result.File != documentPath || result.Index != 1 || result.Style != "Heading1" || result.Text != "Updated Heading" || result.PreviousText != "Heading Text" {
		t.Fatalf("unexpected set result: %+v", result)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if textResult.Blocks[0].Text != "Updated Heading" || textResult.Blocks[0].Style != "Heading1" || textResult.Blocks[1].Text != "Body text" {
		t.Fatalf("unexpected readback blocks: %+v", textResult.Blocks)
	}
}

func TestDOCXParagraphsSetTextFileAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	outPath := filepath.Join(t.TempDir(), "set-file.docx")
	textPath := filepath.Join(t.TempDir(), "replacement.txt")
	if err := os.WriteFile(textPath, []byte("line 1\tcol 2\nline 2"), 0o644); err != nil {
		t.Fatalf("failed to write text file: %v", err)
	}

	if _, err := executeRootForXLSXTest(t,
		"docx", "paragraphs", "set", documentPath,
		"--index", "1",
		"--text-file", textPath,
		"--out", outPath,
	); err != nil {
		t.Fatalf("docx paragraphs set text-file failed: %v", err)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	if !strings.Contains(readback, "line 1\tcol 2\nline 2") {
		t.Fatalf("readback = %q", readback)
	}
}

func TestDOCXParagraphsClearJSONAndReadback(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	outPath := filepath.Join(t.TempDir(), "clear.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "clear", documentPath,
		"--index", "1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx paragraphs clear failed: %v", err)
	}
	var result DOCXParagraphsClearResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal clear JSON: %v\n%s", err, output)
	}
	if result.Index != 1 || result.Style != "Heading1" || result.PreviousText != "Heading Text" {
		t.Fatalf("unexpected clear result: %+v", result)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if textResult.Blocks[0].Text != "" || textResult.Blocks[0].Style != "Heading1" {
		t.Fatalf("unexpected clear readback: %+v", textResult.Blocks[0])
	}
}

func TestDOCXParagraphsAppendJSONReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	outPath := filepath.Join(t.TempDir(), "append.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "append", documentPath,
		"--text", "Tail Heading",
		"--style", "Heading1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx paragraphs append failed: %v", err)
	}
	var result DOCXParagraphsAppendResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal append JSON: %v\n%s", err, output)
	}
	if result.Index != 3 || result.Style != "Heading1" || result.Text != "Tail Heading" {
		t.Fatalf("unexpected append result: %+v", result)
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
	if len(textResult.Blocks) != 3 || textResult.Blocks[2].Text != "Tail Heading" || textResult.Blocks[2].Style != "Heading1" {
		t.Fatalf("unexpected append readback: %+v", textResult.Blocks)
	}
}

func TestDOCXParagraphsAppendAllowsEmptyText(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	outPath := filepath.Join(t.TempDir(), "append-blank.docx")

	if _, err := executeRootForXLSXTest(t,
		"docx", "paragraphs", "append", documentPath,
		"--out", outPath,
	); err != nil {
		t.Fatalf("docx paragraphs append blank failed: %v", err)
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
	if len(textResult.Blocks) != 2 || textResult.Blocks[1].Text != "" {
		t.Fatalf("unexpected blank append readback: %+v", textResult.Blocks)
	}
}

func TestDOCXParagraphsAppendDryRunDoesNotWrite(t *testing.T) {
	documentPath := filepath.Join(t.TempDir(), "document.docx")
	if err := copyFile(getDOCXTestFilePath("minimal"), documentPath); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "append", documentPath,
		"--text", "Dry run tail",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx paragraphs append dry-run failed: %v", err)
	}
	var result DOCXParagraphsAppendResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal append dry-run JSON: %v\n%s", err, output)
	}
	if result.Index != 2 || result.Text != "Dry run tail" {
		t.Fatalf("unexpected append dry-run result: %+v", result)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", documentPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if len(textResult.Blocks) != 1 || textResult.Blocks[0].Text != "Hello world" {
		t.Fatalf("dry-run wrote to document: %+v", textResult.Blocks)
	}
}

func TestDOCXParagraphsInsertAtStartTextFileAndReadback(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	outPath := filepath.Join(t.TempDir(), "insert-start.docx")
	textPath := filepath.Join(t.TempDir(), "insert.txt")
	if err := os.WriteFile(textPath, []byte("Lead\tparagraph\nline 2"), 0o644); err != nil {
		t.Fatalf("failed to write text file: %v", err)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "insert", documentPath,
		"--insert-after", "0",
		"--text-file", textPath,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx paragraphs insert failed: %v", err)
	}
	var result DOCXParagraphsInsertResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal insert JSON: %v\n%s", err, output)
	}
	if result.Index != 1 || result.InsertAfter != 0 || result.Text != "Lead\tparagraph\nline 2" {
		t.Fatalf("unexpected insert result: %+v", result)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if len(textResult.Blocks) != 3 || textResult.Blocks[0].Text != "Lead\tparagraph\nline 2" || textResult.Blocks[1].Text != "Heading Text" {
		t.Fatalf("unexpected insert readback: %+v", textResult.Blocks)
	}
}

func TestDOCXParagraphsInsertAfterTable(t *testing.T) {
	documentPath := getDOCXTestFilePath("mixed-blocks")
	outPath := filepath.Join(t.TempDir(), "insert-after-table.docx")

	if _, err := executeRootForXLSXTest(t,
		"docx", "paragraphs", "insert", documentPath,
		"--insert-after", "1",
		"--text", "After table",
		"--out", outPath,
	); err != nil {
		t.Fatalf("docx paragraphs insert after table failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "text", outPath)
	if err != nil {
		t.Fatalf("docx text readback failed: %v", err)
	}
	var textResult DOCXTextResult
	if err := json.Unmarshal([]byte(readback), &textResult); err != nil {
		t.Fatalf("failed to unmarshal text JSON: %v\n%s", err, readback)
	}
	if len(textResult.Blocks) != 5 || textResult.Blocks[0].Kind != "table" || textResult.Blocks[1].Text != "After table" {
		t.Fatalf("unexpected table insert readback: %+v", textResult.Blocks)
	}
}

func TestDOCXParagraphsSetRejectsBadTargets(t *testing.T) {
	documentPath := getDOCXTestFilePath("mixed-blocks")
	outPath := filepath.Join(t.TempDir(), "out.docx")

	tests := []struct {
		args []string
		code int
	}{
		{[]string{"docx", "paragraphs", "set", documentPath, "--index", "1", "--text", "x", "--out", outPath}, ExitInvalidArgs},
		{[]string{"docx", "paragraphs", "set", documentPath, "--index", "99", "--text", "x", "--out", outPath}, ExitTargetNotFound},
		{[]string{"docx", "paragraphs", "set", documentPath, "--index", "2", "--text", "", "--out", outPath}, ExitInvalidArgs},
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

func TestDOCXParagraphsInsertRejectsBadArgs(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	textPath := filepath.Join(t.TempDir(), "text.txt")
	if err := os.WriteFile(textPath, []byte("x"), 0o644); err != nil {
		t.Fatalf("failed to write text file: %v", err)
	}

	tests := []struct {
		args []string
		code int
	}{
		{[]string{"docx", "paragraphs", "insert", documentPath, "--insert-after", "99", "--text", "x", "--out", filepath.Join(t.TempDir(), "missing.docx")}, ExitTargetNotFound},
		{[]string{"docx", "paragraphs", "insert", documentPath, "--insert-after", "-1", "--text", "x", "--out", filepath.Join(t.TempDir(), "negative.docx")}, ExitInvalidArgs},
		{[]string{"docx", "paragraphs", "insert", documentPath, "--insert-after", "1", "--text", "x", "--text-file", textPath, "--out", filepath.Join(t.TempDir(), "both.docx")}, ExitInvalidArgs},
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

func TestDOCXParagraphsRejectNonDOCX(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	_, err := executeRootForXLSXTest(t, "docx", "paragraphs", "set", workbookPath, "--index", "1", "--text", "x", "--out", filepath.Join(t.TempDir(), "out.docx"))
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

	_, err = executeRootForXLSXTest(t, "docx", "paragraphs", "append", workbookPath, "--text", "x", "--out", filepath.Join(t.TempDir(), "append.docx"))
	if err == nil {
		t.Fatal("expected unsupported type error")
	}
	cliErr, ok = err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitUnsupportedType)
	}
}

func getDOCXTestFilePath(fixtureDir string) string {
	return filepath.Join(getTestdataPath(), "docx", fixtureDir, "document.docx")
}

func docxBlockHashForTest(t *testing.T, documentPath string, block int) string {
	t.Helper()
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "blocks", documentPath,
		"--block", fmt.Sprintf("%d", block),
	)
	if err != nil {
		t.Fatalf("docx blocks failed: %v", err)
	}
	var result DOCXBlocksResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal docx blocks JSON: %v\n%s", err, output)
	}
	if len(result.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(result.Blocks))
	}
	return result.Blocks[0].ContentHash
}
