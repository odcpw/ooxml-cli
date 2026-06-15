package cli

import (
	"archive/zip"
	"encoding/json"
	"io"
	"path/filepath"
	"strings"
	"testing"
)

func readDOCXPartForTest(t *testing.T, path, partName string) string {
	t.Helper()
	reader, err := zip.OpenReader(path)
	if err != nil {
		t.Fatalf("zip.OpenReader(%s) failed: %v", path, err)
	}
	defer reader.Close()
	for _, file := range reader.File {
		if file.Name != partName {
			continue
		}
		rc, err := file.Open()
		if err != nil {
			t.Fatalf("open %s failed: %v", partName, err)
		}
		defer rc.Close()
		data, err := io.ReadAll(rc)
		if err != nil {
			t.Fatalf("read %s failed: %v", partName, err)
		}
		return string(data)
	}
	t.Fatalf("part %s not found in %s", partName, path)
	return ""
}

func TestDOCXStylesApplyCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	docx := findSubcommand(cmd, "docx")
	if docx == nil {
		t.Fatal("docx command is not registered")
	}
	styles := findSubcommand(docx, "styles")
	if styles == nil {
		t.Fatal("docx styles command is not registered")
	}
	if apply := findSubcommand(styles, "apply"); apply == nil {
		t.Fatal("docx styles apply command is not registered")
	}
}

func TestDOCXStylesApplyParagraphJSONReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	outPath := filepath.Join(t.TempDir(), "apply-para.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "Heading2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx styles apply failed: %v", err)
	}
	var result DOCXStylesApplyResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal apply JSON: %v\n%s", err, output)
	}
	if result.File != documentPath || result.Index != 1 || result.BlockID != "body.b1" || result.Target != "paragraph" ||
		result.PreviousStyle != "Heading1" || result.Style != "Heading2" || result.ContentHash == result.PreviousHash {
		t.Fatalf("unexpected apply result: %+v", result)
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
	if blocks.Blocks[0].Paragraph == nil || blocks.Blocks[0].Paragraph.Style != "Heading2" {
		t.Fatalf("unexpected readback style: %+v", blocks.Blocks[0])
	}
}

func TestDOCXStylesApplyParagraphTextOutput(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	outPath := filepath.Join(t.TempDir(), "apply-text.docx")

	output, err := executeRootForXLSXTest(t,
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "Heading2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx styles apply failed: %v", err)
	}
	if !strings.Contains(output, "applied Heading2 style to paragraph 1") {
		t.Fatalf("text output = %q", output)
	}
}

func TestDOCXStylesApplyRunStyleReadback(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	outPath := filepath.Join(t.TempDir(), "apply-run.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "styles", "apply", documentPath,
		"--index", "2",
		"--target", "run",
		"--style", "Emphasis",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx styles apply run failed: %v", err)
	}
	var result DOCXStylesApplyResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal apply JSON: %v\n%s", err, output)
	}
	// rStyle is not folded into the block hash, so it should be unchanged.
	if result.Target != "run" || result.Style != "Emphasis" || result.ContentHash != result.PreviousHash {
		t.Fatalf("unexpected run apply result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	if !strings.Contains(readDOCXPartForTest(t, outPath, "word/document.xml"), `w:rStyle w:val="Emphasis"`) {
		t.Fatalf("expected w:rStyle Emphasis in document.xml")
	}
}

func TestDOCXStylesApplyTableStyleReadback(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	outPath := filepath.Join(t.TempDir(), "apply-table.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "table",
		"--style", "TableGrid",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx styles apply table failed: %v", err)
	}
	var result DOCXStylesApplyResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal apply JSON: %v\n%s", err, output)
	}
	if result.Target != "table" || result.Style != "TableGrid" || result.BlockKind != "table" ||
		result.BlockIndex != 3 || result.BlockID != "body.b3" {
		t.Fatalf("unexpected table apply result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
}

func TestDOCXStylesApplyHashGuard(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	hash := docxBlockHashForTest(t, documentPath, 1)

	// Correct hash succeeds.
	if _, err := executeRootForXLSXTest(t,
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "Heading2",
		"--expect-hash", hash,
		"--out", filepath.Join(t.TempDir(), "guard-ok.docx"),
	); err != nil {
		t.Fatalf("docx styles apply with correct hash failed: %v", err)
	}

	// Wrong hash is rejected with InvalidArgs.
	args := []string{
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "Heading2",
		"--expect-hash", "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		"--out", filepath.Join(t.TempDir(), "guard-bad.docx"),
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestDOCXStylesApplyStyleNotFoundListsCandidates(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	args := []string{
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "NoSuchStyle",
		"--out", filepath.Join(t.TempDir(), "missing.docx"),
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitTargetNotFound)
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if !strings.Contains(cliErr.Error(), "Heading1") {
		t.Fatalf("not-found error should list candidate styles: %v", cliErr)
	}
}

func TestDOCXStylesApplyTypeMismatch(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	args := []string{
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "Emphasis",
		"--out", filepath.Join(t.TempDir(), "mismatch.docx"),
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestDOCXStylesApplyParagraphTargetOnTableBlock(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	args := []string{
		"docx", "styles", "apply", documentPath,
		"--index", "3",
		"--target", "paragraph",
		"--style", "Heading2",
		"--out", filepath.Join(t.TempDir(), "wrong-block.docx"),
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestDOCXStylesApplyRejectsBadArgsAndNonDOCX(t *testing.T) {
	documentPath := getDOCXTestFilePath("apply-styles")
	outDir := t.TempDir()

	tests := []struct {
		args []string
		code int
	}{
		{[]string{"docx", "styles", "apply", documentPath, "--index", "0", "--target", "paragraph", "--style", "Heading2", "--out", filepath.Join(outDir, "a.docx")}, ExitInvalidArgs},
		{[]string{"docx", "styles", "apply", documentPath, "--index", "1", "--target", "bogus", "--style", "Heading2", "--out", filepath.Join(outDir, "b.docx")}, ExitInvalidArgs},
		{[]string{"docx", "styles", "apply", documentPath, "--index", "1", "--target", "paragraph", "--style", "", "--out", filepath.Join(outDir, "c.docx")}, ExitInvalidArgs},
		{[]string{"docx", "styles", "apply", documentPath, "--index", "99", "--target", "paragraph", "--style", "Heading2", "--out", filepath.Join(outDir, "d.docx")}, ExitTargetNotFound},
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

	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	_, err := executeRootForXLSXTest(t, "docx", "styles", "apply", workbookPath, "--index", "1", "--target", "paragraph", "--style", "Heading2", "--out", filepath.Join(outDir, "x.docx"))
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

func TestDOCXStylesApplyDryRunDoesNotWrite(t *testing.T) {
	documentPath := filepath.Join(t.TempDir(), "document.docx")
	if err := copyFile(getDOCXTestFilePath("apply-styles"), documentPath); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}

	if _, err := executeRootForXLSXTest(t,
		"docx", "styles", "apply", documentPath,
		"--index", "1",
		"--target", "paragraph",
		"--style", "Heading2",
		"--dry-run",
	); err != nil {
		t.Fatalf("docx styles apply dry-run failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "blocks", documentPath, "--block", "1")
	if err != nil {
		t.Fatalf("docx blocks readback failed: %v", err)
	}
	var blocks DOCXBlocksResult
	if err := json.Unmarshal([]byte(readback), &blocks); err != nil {
		t.Fatalf("failed to unmarshal blocks readback: %v\n%s", err, readback)
	}
	if blocks.Blocks[0].Paragraph == nil || blocks.Blocks[0].Paragraph.Style != "Heading1" {
		t.Fatalf("dry-run mutated the document: %+v", blocks.Blocks[0])
	}
}
