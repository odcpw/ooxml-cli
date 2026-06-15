package cli

import (
	"bytes"
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestPPTXShapesCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	shapes := findSubcommand(pptx, "shapes")
	if shapes == nil {
		t.Fatal("pptx shapes command is not registered")
	}
	for _, name := range []string{"show", "get", "set-bounds", "delete"} {
		if sub := findSubcommand(shapes, name); sub == nil {
			t.Fatalf("pptx shapes %s command is not registered", name)
		}
	}
}

func TestPPTXShapesShowAndGetJSON(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "show", fixturePath,
		"--slide", "2",
		"--include-text",
		"--include-bounds",
	)
	var result PPTXShapesResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal shapes show JSON: %v\n%s", err, output)
	}
	if result.Slide != 2 || result.LayoutName != "Title and Content" || len(result.Shapes) != 2 {
		t.Fatalf("unexpected shapes show result: %+v", result)
	}
	title := result.Shapes[0]
	if title.ShapeID != 2 || title.PrimarySelector != "title" || !containsString(title.Selectors, "title") || !containsString(title.Selectors, "shape:2") {
		t.Fatalf("unexpected title shape entry: %+v", title)
	}
	if title.Selectors == nil {
		t.Fatal("selectors encoded as nil, want empty or populated array")
	}
	body := result.Shapes[1]
	if body.ShapeID != 3 || body.PrimarySelector != "body" || body.Bounds != nil || !strings.Contains(body.TextPreview, "main content") {
		t.Fatalf("unexpected body shape entry: %+v", body)
	}

	getOutput := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "get", fixturePath,
		"--slide", "2",
		"--target", "body",
		"--include-text",
	)
	var getResult PPTXShapesResult
	if err := json.Unmarshal([]byte(getOutput), &getResult); err != nil {
		t.Fatalf("failed to unmarshal shapes get JSON: %v\n%s", err, getOutput)
	}
	if len(getResult.Shapes) != 1 || getResult.Shapes[0].ShapeID != 3 || !containsString(getResult.Shapes[0].Selectors, "body") {
		t.Fatalf("unexpected shapes get result: %+v", getResult.Shapes)
	}
}

func TestPPTXShapesSetBoundsJSONReadbackAndValidate(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "shape-bounds.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "set-bounds", fixturePath,
		"--slide", "2",
		"--target", "body",
		"--bounds", "111111,222222,333333,444444",
		"--out", outPath,
	)
	var result PPTXShapesSetBoundsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-bounds JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected file/output metadata: %+v", result)
	}
	if result.Slide != 2 || result.ShapeID != 3 || result.NewX != 111111 || result.NewY != 222222 || result.NewCX != 333333 || result.NewCY != 444444 {
		t.Fatalf("unexpected set-bounds result: %+v", result)
	}
	if result.Destination == nil || result.Destination.File != outPath || result.Destination.PrimarySelector != "body" {
		t.Fatalf("unexpected set-bounds destination: %+v", result.Destination)
	}
	if !containsString(result.Destination.Selectors, "body") || !containsString(result.Destination.Selectors, "shape:3") {
		t.Fatalf("unexpected set-bounds destination selectors: %+v", result.Destination.Selectors)
	}
	if result.Destination.Bounds == nil || result.Destination.Bounds.X != 111111 || result.Destination.Bounds.Y != 222222 || result.Destination.Bounds.CX != 333333 || result.Destination.Bounds.CY != 444444 {
		t.Fatalf("unexpected set-bounds destination bounds: %+v", result.Destination.Bounds)
	}
	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	var readbackResult PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &readbackResult); err != nil {
		t.Fatalf("failed to unmarshal readback JSON: %v\n%s", err, readback)
	}
	bounds := readbackResult.Shapes[0].Bounds
	if bounds == nil || bounds.X != 111111 || bounds.Y != 222222 || bounds.CX != 333333 || bounds.CY != 444444 {
		t.Fatalf("unexpected readback bounds: %+v", bounds)
	}
}

func TestPPTXShapesSetBoundsDryRunIncludesDestinationReadback(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "set-bounds", fixturePath,
		"--slide", "2",
		"--target", "body",
		"--bounds", "555555,666666,777777,888888",
		"--dry-run",
	)
	var result PPTXShapesSetBoundsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-bounds dry-run JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != "" || !result.DryRun {
		t.Fatalf("unexpected dry-run file/output metadata: %+v", result)
	}
	if result.Destination == nil || result.Destination.File != "" || result.Destination.Bounds == nil {
		t.Fatalf("unexpected dry-run destination: %+v", result.Destination)
	}
	if result.Destination.Bounds.X != 555555 || result.Destination.Bounds.Y != 666666 || result.Destination.Bounds.CX != 777777 || result.Destination.Bounds.CY != 888888 {
		t.Fatalf("unexpected dry-run destination bounds: %+v", result.Destination.Bounds)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx shapes get")

	readback := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "get", fixturePath,
		"--slide", "2",
		"--target", "body",
		"--include-bounds",
	)
	var readbackResult PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &readbackResult); err != nil {
		t.Fatalf("failed to unmarshal source readback JSON: %v\n%s", err, readback)
	}
	if len(readbackResult.Shapes) != 1 {
		t.Fatalf("unexpected source readback: %+v", readbackResult.Shapes)
	}
	if bounds := readbackResult.Shapes[0].Bounds; bounds != nil && bounds.X == 555555 && bounds.Y == 666666 && bounds.CX == 777777 && bounds.CY == 888888 {
		t.Fatalf("source fixture unexpectedly has dry-run bounds: %+v", bounds)
	}
}

func TestPPTXShapesDeleteJSONReadbackAndValidate(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "shape-delete.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "delete", fixturePath,
		"--slide", "2",
		"--target", "title",
		"--out", outPath,
	)
	var result PPTXShapesDeleteResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal delete JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected delete file/output metadata: %+v", result)
	}
	if result.Slide != 2 || result.ShapeID != 2 {
		t.Fatalf("unexpected delete result: %+v", result)
	}
	if result.Deleted == nil || result.Deleted.File != fixturePath || result.Deleted.PrimarySelector != "title" {
		t.Fatalf("unexpected deleted target metadata: %+v", result.Deleted)
	}
	if !containsString(result.Deleted.Selectors, "title") || !containsString(result.Deleted.Selectors, "shape:2") {
		t.Fatalf("unexpected deleted selectors: %+v", result.Deleted.Selectors)
	}
	if _, err := executePPTXShapesCommandErr(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate delete output failed: %v", err)
	}
	_, err := executePPTXShapesCommandErr(t,
		"--format", "json",
		"pptx", "shapes", "get", outPath,
		"--slide", "2",
		"--target", "title",
	)
	if err == nil {
		t.Fatal("expected deleted title target to be missing")
	}
}

func TestPPTXShapesDeleteDryRunReportsDeletedTargetAndPreservesSource(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "delete", fixturePath,
		"--slide", "2",
		"--target", "title",
		"--dry-run",
	)
	var result PPTXShapesDeleteResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal delete dry-run JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != "" || !result.DryRun {
		t.Fatalf("unexpected delete dry-run file/output metadata: %+v", result)
	}
	if result.Deleted == nil || result.Deleted.File != fixturePath || result.Deleted.PrimarySelector != "title" || result.Deleted.ShapeID != 2 {
		t.Fatalf("unexpected delete dry-run metadata: %+v", result.Deleted)
	}

	readback := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "shapes", "get", fixturePath,
		"--slide", "2",
		"--target", "title",
	)
	var readbackResult PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &readbackResult); err != nil {
		t.Fatalf("failed to unmarshal source readback JSON: %v\n%s", err, readback)
	}
	if len(readbackResult.Shapes) != 1 || readbackResult.Shapes[0].ShapeID != 2 {
		t.Fatalf("source fixture title missing after dry-run: %+v", readbackResult.Shapes)
	}
}

func TestPPTXShapesInvalidSlideIsInvalidArgs(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "shapes", "show", fixturePath,
		"--slide", "99",
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func pptxShapesFixturePath(t *testing.T, fixture string) string {
	t.Helper()
	path, err := filepath.Abs(filepath.Join("../../testdata/pptx", fixture, "presentation.pptx"))
	if err != nil {
		t.Fatalf("failed to build fixture path: %v", err)
	}
	return path
}

func executePPTXShapesCommand(t *testing.T, args ...string) string {
	t.Helper()
	output, err := executePPTXShapesCommandErr(t, args...)
	if err != nil {
		t.Fatalf("%v failed: %v", args, err)
	}
	return output
}

func executePPTXShapesCommandErr(t *testing.T, args ...string) (string, error) {
	t.Helper()
	cmd := newTestRootCmd(t)
	cmd.SetArgs(args)
	var output bytes.Buffer
	cmd.SetOut(&output)
	cmd.SetErr(&bytes.Buffer{})
	err := cmd.Execute()
	return output.String(), err
}

func assertPPTXShapesExitCode(t *testing.T, err error, want int) {
	t.Helper()
	if err == nil {
		t.Fatalf("expected exit code %d error", want)
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != want {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, want)
	}
}
