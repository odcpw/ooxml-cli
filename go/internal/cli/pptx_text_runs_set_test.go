package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestPPTXTextSetCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	text := findSubcommand(pptx, "text")
	if text == nil {
		t.Fatal("pptx text command is not registered")
	}
	if findSubcommand(text, "set") == nil {
		t.Fatal("pptx text set command is not registered")
	}
}

func TestPPTXTextSetRunJSONReadbackAndValidate(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "text-set.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "text", "set", fixturePath,
		"--slide", "2",
		"--target", "title",
		"--paragraph", "0",
		"--run-index", "0",
		"--bold",
		"--italic",
		"--font-size", "28",
		"--color", "ff0000",
		"--font-family", "Arial",
		"--out", outPath,
	)
	var result PPTXTextRunsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal text set JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected file/output metadata: %+v", result)
	}
	if result.Slide != 2 || result.ShapeID != 2 || result.ParagraphIndex != 0 {
		t.Fatalf("unexpected text set result: %+v", result.SetRunPropertiesResult)
	}
	if result.RunIndex == nil || *result.RunIndex != 0 || len(result.AppliedRuns) != 1 || result.AppliedRuns[0] != 0 {
		t.Fatalf("unexpected applied runs: %+v", result.SetRunPropertiesResult)
	}
	if len(result.NewProperties) != 1 {
		t.Fatalf("expected 1 new property snapshot, got %d", len(result.NewProperties))
	}
	np := result.NewProperties[0]
	if np.Bold == nil || !*np.Bold || np.Italic == nil || !*np.Italic {
		t.Fatalf("expected bold+italic, got %+v", np)
	}
	if np.FontSize == nil || *np.FontSize != 28 || np.Color != "FF0000" || np.FontFamily != "Arial" {
		t.Fatalf("unexpected new props: %+v", np)
	}
	if result.Destination == nil || result.Destination.File != outPath || result.Destination.PrimarySelector != "title" {
		t.Fatalf("unexpected destination: %+v", result.Destination)
	}

	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	if strings.TrimSpace(readback) == "" {
		t.Fatal("empty readback output")
	}
}

func TestPPTXTextSetUnderlineAliasesNormalizeToDrawingMLTokens(t *testing.T) {
	cases := []struct {
		alias string
		want  string
	}{
		{"single", "sng"},
		{"double", "dbl"},
	}
	for _, tc := range cases {
		tc := tc
		t.Run(tc.alias, func(t *testing.T) {
			fixturePath := pptxShapesFixturePath(t, "title-content")
			outPath := filepath.Join(t.TempDir(), "underline-"+tc.alias+".pptx")

			output := executePPTXShapesCommand(t,
				"--format", "json",
				"pptx", "text", "set", fixturePath,
				"--slide", "2",
				"--target", "title",
				"--paragraph", "0",
				"--run-index", "0",
				"--underline", tc.alias,
				"--out", outPath,
			)
			var result PPTXTextRunsSetResult
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Fatalf("failed to unmarshal text set JSON: %v\n%s", err, output)
			}
			if len(result.NewProperties) != 1 {
				t.Fatalf("expected 1 new property snapshot, got %d", len(result.NewProperties))
			}
			if result.NewProperties[0].Underline != tc.want {
				t.Fatalf("--underline %s: got u=%q, want %q", tc.alias, result.NewProperties[0].Underline, tc.want)
			}
			assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
		})
	}
}

func TestPPTXTextSetAllRunsInParagraphWhenRunIndexOmitted(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "rich-alignment")
	outPath := filepath.Join(t.TempDir(), "all-runs.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "text", "set", fixturePath,
		"--slide", "1",
		"--target", "shape:2",
		"--paragraph", "0",
		"--bold",
		"--out", outPath,
	)
	var result PPTXTextRunsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal text set JSON: %v\n%s", err, output)
	}
	if result.RunIndex != nil {
		t.Fatalf("expected nil runIndex when --run-index omitted, got %v", *result.RunIndex)
	}
	if len(result.AppliedRuns) == 0 {
		t.Fatalf("expected at least one applied run, got %+v", result.AppliedRuns)
	}
	for _, np := range result.NewProperties {
		if np.Bold == nil || !*np.Bold {
			t.Fatalf("expected every applied run bold, got %+v", np)
		}
	}
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
}

func TestPPTXTextSetHyperlinkRegistersExternalRelationship(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "hyperlink.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "text", "set", fixturePath,
		"--slide", "2",
		"--target", "title",
		"--paragraph", "0",
		"--run-index", "0",
		"--hyperlink", "https://example.com",
		"--out", outPath,
	)
	var result PPTXTextRunsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal text set JSON: %v\n%s", err, output)
	}
	if len(result.NewProperties) != 1 || !strings.HasPrefix(result.NewProperties[0].Hyperlink, "rId") {
		t.Fatalf("expected hyperlink rId, got %+v", result.NewProperties)
	}
	// validate --strict must pass on the generated output.
	assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
}

func TestPPTXTextSetRemoveBoldRoundTrip(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	dir := t.TempDir()
	boldPath := filepath.Join(dir, "bold.pptx")
	plainPath := filepath.Join(dir, "plain.pptx")

	executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "0", "--run-index", "0",
		"--bold", "--out", boldPath,
	)
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "text", "set", boldPath,
		"--slide", "2", "--target", "title", "--paragraph", "0", "--run-index", "0",
		"--remove-bold", "--out", plainPath,
	)
	var result PPTXTextRunsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal text set JSON: %v\n%s", err, output)
	}
	if len(result.OldProperties) != 1 || result.OldProperties[0].Bold == nil || !*result.OldProperties[0].Bold {
		t.Fatalf("expected old bold=true, got %+v", result.OldProperties)
	}
	if len(result.NewProperties) != 1 || result.NewProperties[0].Bold != nil {
		t.Fatalf("expected new bold cleared, got %+v", result.NewProperties)
	}
}

func TestPPTXTextSetDryRunDoesNotWriteAndIncludesTemplates(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "0", "--run-index", "0",
		"--bold", "--dry-run",
	)
	var result PPTXTextRunsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}
	if result.Destination == nil || result.Destination.File != "" {
		t.Fatalf("unexpected dry-run destination: %+v", result.Destination)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx shapes get")
}

func TestPPTXTextSetParagraphOutOfRangeIsInvalidArgs(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "99", "--bold",
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func TestPPTXTextSetRunIndexOutOfRangeIsInvalidArgs(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "0", "--run-index", "99", "--bold",
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func TestPPTXTextSetInvalidColorIsInvalidArgs(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "0", "--color", "ZZZZZZ",
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func TestPPTXTextSetMutualExclusivityIsInvalidArgs(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "0", "--bold", "--remove-bold",
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func TestPPTXTextSetNoStylingFlagsIsInvalidArgs(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "title", "--paragraph", "0",
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func TestPPTXTextSetUnknownTargetReturnsCandidates(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "text", "set", fixturePath,
		"--slide", "2", "--target", "nonexistent", "--paragraph", "0", "--bold",
	)
	if err == nil {
		t.Fatal("expected error for unknown target")
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitTargetNotFound && cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("unexpected exit code %d for unknown target", cliErr.ExitCode)
	}
}
