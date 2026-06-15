package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXFreezeShowUnfrozen(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "freeze", "show", workbookPath, "--sheet", "1")
	if err != nil {
		t.Fatalf("freeze show failed: %v", err)
	}
	var result XLSXFreezeShowResult
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if result.State != nil {
		t.Fatalf("expected null state on unfrozen sheet, got %+v", result.State)
	}
	if result.SetCommand == "" || !strings.Contains(result.SetCommand, "freeze set") {
		t.Fatalf("missing set command: %+v", result)
	}
}

func TestXLSXFreezeSetAndShow(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "frozen.xlsx")

	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "1", "--cols", "1", "--out", outPath)
	if err != nil {
		t.Fatalf("freeze set failed: %v", err)
	}
	var setResult XLSXFreezeMutationResult
	if err := json.Unmarshal([]byte(out), &setResult); err != nil {
		t.Fatalf("unmarshal set: %v\n%s", err, out)
	}
	if setResult.State == nil || setResult.State.Rows != 1 || setResult.State.Cols != 1 {
		t.Fatalf("unexpected set state: %+v", setResult.State)
	}
	if setResult.State.TopLeftCell != "B2" || !setResult.State.Frozen {
		t.Fatalf("unexpected topLeftCell/frozen: %+v", setResult.State)
	}
	if setResult.ShowCommand == "" || !strings.Contains(setResult.ShowCommand, "freeze show") {
		t.Fatalf("missing show command: %+v", setResult)
	}
	if setResult.ValidateCommand == "" {
		t.Fatalf("missing validate command")
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, setResult.ShowCommand)
	var showResult XLSXFreezeShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal show: %v\n%s", err, showOut)
	}
	if showResult.State == nil || showResult.State.Rows != 1 || showResult.State.Cols != 1 {
		t.Fatalf("show readback mismatch: %+v", showResult.State)
	}
}

func TestXLSXFreezeRowsOnly(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "rows.xlsx")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "2", "--out", outPath)
	if err != nil {
		t.Fatalf("freeze set rows-only failed: %v", err)
	}
	var setResult XLSXFreezeMutationResult
	if err := json.Unmarshal([]byte(out), &setResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if setResult.State == nil || setResult.State.Rows != 2 || setResult.State.Cols != 0 {
		t.Fatalf("unexpected state: %+v", setResult.State)
	}
	if setResult.State.TopLeftCell != "A3" {
		t.Fatalf("expected topLeftCell A3, got %q", setResult.State.TopLeftCell)
	}
}

func TestXLSXFreezeClear(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	frozen := filepath.Join(t.TempDir(), "frozen.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "1", "--cols", "1", "--out", frozen); err != nil {
		t.Fatalf("freeze set failed: %v", err)
	}
	cleared := filepath.Join(t.TempDir(), "cleared.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "freeze", "clear", frozen, "--sheet", "1", "--out", cleared); err != nil {
		t.Fatalf("freeze clear failed: %v", err)
	}
	showOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "freeze", "show", cleared, "--sheet", "1")
	if err != nil {
		t.Fatalf("freeze show failed: %v", err)
	}
	var showResult XLSXFreezeShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, showOut)
	}
	if showResult.State != nil {
		t.Fatalf("expected state cleared, got %+v", showResult.State)
	}
}

func TestXLSXFreezeExpectStateGuard(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")

	// expect-state=none on an unfrozen sheet should succeed.
	okPath := filepath.Join(t.TempDir(), "ok.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "1", "--cols", "1", "--expect-state", "none", "--out", okPath); err != nil {
		t.Fatalf("expect-state=none on unfrozen sheet should succeed: %v", err)
	}

	// expect-state=frozen on an unfrozen sheet should fail.
	badPath := filepath.Join(t.TempDir(), "bad.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "1", "--cols", "1", "--expect-state", "frozen", "--out", badPath)
	if err == nil {
		t.Fatalf("expect-state=frozen on unfrozen sheet should error")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXFreezeSetRequiresRowsOrCols(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "none.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "0", "--cols", "0", "--out", out)
	if err == nil {
		t.Fatalf("expected error when neither rows nor cols provided")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXFreezeSetRowsOutOfBounds(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "oob.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "1048576", "--out", out)
	if err == nil {
		t.Fatalf("expected out-of-bounds error")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXFreezeClearRequiresFrozen(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out := filepath.Join(t.TempDir(), "noop.xlsx")
	_, err := executeRootForXLSXTest(t, "xlsx", "freeze", "clear", workbookPath, "--sheet", "1", "--out", out)
	if err == nil {
		t.Fatalf("expected error clearing an unfrozen sheet")
	}
	if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXFreezeDryRun(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "freeze", "set", workbookPath, "--sheet", "1", "--rows", "1", "--cols", "1", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run failed: %v", err)
	}
	var result XLSXFreezeMutationResult
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, out)
	}
	if !result.DryRun {
		t.Fatalf("expected dryRun true")
	}
	if result.Output != "" {
		t.Fatalf("expected no output path on dry-run, got %q", result.Output)
	}
}

func TestXLSXFreezeInPlaceWithBackup(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	dir := t.TempDir()
	target := filepath.Join(dir, "wb.xlsx")
	data, err := os.ReadFile(workbookPath)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	if err := os.WriteFile(target, data, 0o644); err != nil {
		t.Fatalf("write target: %v", err)
	}
	backupPath := target + ".bak"
	if _, err := executeRootForXLSXTest(t, "xlsx", "freeze", "set", target, "--sheet", "1", "--rows", "1", "--cols", "1", "--in-place", "--backup", backupPath); err != nil {
		t.Fatalf("in-place freeze set failed: %v", err)
	}
	if _, err := os.Stat(backupPath); err != nil {
		t.Fatalf("expected backup file: %v", err)
	}
	showOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "freeze", "show", target, "--sheet", "1")
	if err != nil {
		t.Fatalf("show after in-place failed: %v", err)
	}
	var showResult XLSXFreezeShowResult
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, showOut)
	}
	if showResult.State == nil || showResult.State.Rows != 1 {
		t.Fatalf("expected frozen state after in-place edit, got %+v", showResult.State)
	}
}
