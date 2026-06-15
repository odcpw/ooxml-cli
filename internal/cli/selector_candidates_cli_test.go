package cli

import (
	"path/filepath"
	"strings"
	"testing"
)

// These command-path tests assert that a near-miss selector produces an error that
// lists nearby valid candidates and/or a discovery command, while preserving the
// existing ExitTargetNotFound exit code.

func TestXLSXSheetNotFoundListsCandidates(t *testing.T) {
	// shared-strings has sheets Summary (sheetId:1) and Data (sheetId:2).
	fixture := filepath.Join(getTestdataPath(), "xlsx", "shared-strings", "workbook.xlsx")
	_, err := executeRootForXLSXTest(t, "--json", "xlsx", "sheets", "show",
		fixture, "--sheet", "Summ")
	if err == nil {
		t.Fatal("expected error for near-miss sheet selector")
	}
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T", err)
	}
	if cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitTargetNotFound)
	}
	msg := cliErr.Message
	if !strings.Contains(msg, "sheet not found: Summ") {
		t.Fatalf("message missing entity/selector: %q", msg)
	}
	if !strings.Contains(msg, "did you mean:") {
		t.Fatalf("message missing candidate list: %q", msg)
	}
	if !strings.Contains(msg, "sheetId:1") {
		t.Fatalf("message missing candidate for Summary sheet: %q", msg)
	}
	if !strings.Contains(msg, "ooxml --json xlsx sheets list") {
		t.Fatalf("message missing discovery command: %q", msg)
	}
}

func TestXLSXChartNotFoundListsCandidates(t *testing.T) {
	// chart-workbook has a single chart (chart:1).
	fixture := filepath.Join(getTestdataPath(), "xlsx", "chart-workbook", "workbook.xlsx")
	_, err := executeRootForXLSXTest(t, "--json", "xlsx", "charts", "show",
		fixture, "--chart", "nope")
	if err == nil {
		t.Fatal("expected error for near-miss chart selector")
	}
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T", err)
	}
	if cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitTargetNotFound)
	}
	if !strings.Contains(cliErr.Message, "chart not found: nope") {
		t.Fatalf("message missing entity/selector: %q", cliErr.Message)
	}
	if !strings.Contains(cliErr.Message, "did you mean:") || !strings.Contains(cliErr.Message, "chart:1") {
		t.Fatalf("message missing chart candidate: %q", cliErr.Message)
	}
	if !strings.Contains(cliErr.Message, "ooxml --json xlsx charts list") {
		t.Fatalf("message missing discovery command: %q", cliErr.Message)
	}
}

func TestPPTXChartNotFoundListsCandidates(t *testing.T) {
	// chart-simple deck has at least one slide chart (chart:1).
	fixture := filepath.Join(getTestdataPath(), "pptx", "chart-simple", "presentation.pptx")
	_, err := executeRootForXLSXTest(t, "--json", "pptx", "charts", "show",
		fixture, "--chart", "nope")
	if err == nil {
		t.Fatal("expected error for near-miss pptx chart selector")
	}
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T", err)
	}
	if cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitTargetNotFound)
	}
	if !strings.Contains(cliErr.Message, "did you mean:") || !strings.Contains(cliErr.Message, "chart:1") {
		t.Fatalf("message missing pptx chart candidate: %q", cliErr.Message)
	}
	if !strings.Contains(cliErr.Message, "discover with") {
		t.Fatalf("message missing discovery command: %q", cliErr.Message)
	}
}
