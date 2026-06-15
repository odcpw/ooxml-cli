package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

func TestPPTXFieldsCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	fields := findSubcommand(pptx, "fields")
	if fields == nil {
		t.Fatal("pptx fields command is not registered")
	}
	for _, sub := range []string{"inspect", "set"} {
		if findSubcommand(fields, sub) == nil {
			t.Fatalf("pptx fields %s command is not registered", sub)
		}
	}
}

func TestPPTXFieldsInspectJSON(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "header-footer")
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "fields", "inspect", fixture,
	)
	var report inspect.FieldsReport
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		t.Fatalf("failed to unmarshal fields inspect JSON: %v\n%s", err, output)
	}
	if len(report.Masters) != 1 {
		t.Fatalf("expected 1 master, got %d", len(report.Masters))
	}
	if !report.Masters[0].HasHeaderFooter {
		t.Fatalf("expected master to report a p:hf element")
	}
	if len(report.Slides) == 0 || report.Slides[0].FooterPlaceholder == nil {
		t.Fatalf("expected slide 1 to carry a footer placeholder: %+v", report.Slides)
	}
}

func TestPPTXFieldsSetReadbackAndValidate(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "header-footer")
	outPath := filepath.Join(t.TempDir(), "fields-set.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "fields", "set", fixture,
		"--footer", "Confidential",
		"--show-slide-number=false",
		"--date-format", "date-only",
		"--out", outPath,
	)
	var result PPTXFieldsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal fields set JSON: %v\n%s", err, output)
	}
	if result.File != fixture || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected metadata: %+v", result)
	}
	if result.FooterText == nil || *result.FooterText != "Confidential" {
		t.Fatalf("unexpected footer text: %+v", result.FooterText)
	}
	if result.ShowSlideNumber == nil || *result.ShowSlideNumber {
		t.Fatalf("expected show-slide-number=false: %+v", result.ShowSlideNumber)
	}
	if result.DateFormat != "date-only" {
		t.Fatalf("unexpected date format: %q", result.DateFormat)
	}
	if result.FooterPlaceholdersUpdated != 1 || result.DatePlaceholdersUpdated != 1 {
		t.Fatalf("unexpected placeholder counts: %+v", result.SetFieldsResult)
	}

	// Readback via the generated inspect command target.
	readback := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "fields", "inspect", outPath,
	)
	var report inspect.FieldsReport
	if err := json.Unmarshal([]byte(readback), &report); err != nil {
		t.Fatalf("failed to unmarshal fields readback JSON: %v\n%s", err, readback)
	}
	if report.Slides[0].FooterPlaceholder == nil || report.Slides[0].FooterPlaceholder.Text != "Confidential" {
		t.Fatalf("footer text not persisted: %+v", report.Slides[0].FooterPlaceholder)
	}
	if report.Masters[0].ShowSlideNumber {
		t.Fatalf("slide-number visibility not persisted on master")
	}
	if result.ReadbackCommand == "" || result.ValidateCommand == "" || result.RenderCommand == "" {
		t.Fatalf("expected readback/validate/render commands: %+v", result.PPTXBridgeReadbackCommands)
	}
}

func TestPPTXFieldsSetDryRunDoesNotWrite(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "header-footer")
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "fields", "set", fixture,
		"--footer", "Draft",
		"--dry-run",
	)
	var result PPTXFieldsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal fields set JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("expected dry-run with no output: %+v", result)
	}
	if result.ValidateCommandTemplate == "" {
		t.Fatalf("expected template readback commands for dry-run: %+v", result.PPTXBridgeReadbackCommands)
	}
}

func TestPPTXFieldsSetRequiresAFlag(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "header-footer")
	outPath := filepath.Join(t.TempDir(), "noop.pptx")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "fields", "set", fixture, "--out", outPath,
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
	if _, statErr := os.Stat(outPath); statErr == nil {
		t.Fatalf("expected no output file when no flags provided")
	}
}

func TestPPTXFieldsSetRejectsBadDateFormat(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "header-footer")
	outPath := filepath.Join(t.TempDir(), "bad.pptx")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "fields", "set", fixture,
		"--date-format", "bogus",
		"--out", outPath,
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}
