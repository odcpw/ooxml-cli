package cli

import (
	"bytes"
	"encoding/json"
	"encoding/xml"
	"fmt"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"
)

func TestPPTXXLSXBindingsCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	bindings := findSubcommand(pptx, "xlsx-bindings")
	if bindings == nil {
		t.Fatal("pptx xlsx-bindings command is not registered")
	}
	if plan := findSubcommand(bindings, "plan"); plan == nil {
		t.Fatal("pptx xlsx-bindings plan command is not registered")
	}
	if apply := findSubcommand(bindings, "apply"); apply == nil {
		t.Fatal("pptx xlsx-bindings apply command is not registered")
	}
}

func TestPPTXXLSXBindingsPlanJSONResolvesSourcesAndTargets(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writePPTXXLSXBindingsWorkbook(t)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "plan", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:Q4",
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings plan failed: %v", err)
	}

	var result PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal plan JSON: %v\n%s", err, output)
	}
	if result.BindingSource.Range != "A1:Q4" || len(result.Operations) != 3 {
		t.Fatalf("unexpected plan result: %+v", result)
	}
	if result.Operations[0].Op != "replace-text" || result.Operations[0].Source.Range != "AA1" {
		t.Fatalf("unexpected replace op: %+v", result.Operations[0])
	}
	if !strings.Contains(result.Operations[0].EquivalentCommand, "--mode preserve-format") ||
		!strings.Contains(result.Operations[0].EquivalentCommand, "--row-sep '\\n'") ||
		!strings.Contains(result.Operations[0].EquivalentCommand, "--col-sep ' | '") {
		t.Fatalf("replace equivalent command missing options: %s", result.Operations[0].EquivalentCommand)
	}
	if result.Operations[1].Op != "update-table" || result.Operations[1].Source.Range != "AA3:AC5" {
		t.Fatalf("unexpected update op: %+v", result.Operations[1])
	}
	if result.Operations[1].Update == nil || result.Operations[1].Update.FormulaMode != "formula" {
		t.Fatalf("update formula mode was not normalized: %+v", result.Operations[1])
	}
	if !strings.Contains(result.Operations[1].ReadbackCommand, "pptx tables show") {
		t.Fatalf("missing table readback command: %+v", result.Operations[1])
	}
	if !strings.Contains(result.Operations[1].EquivalentCommand, "--formula-mode formula") {
		t.Fatalf("update equivalent command missing formula mode: %s", result.Operations[1].EquivalentCommand)
	}
	if result.Operations[2].Op != "place-table" || result.Operations[2].Source.Range != "AA7:AB8" {
		t.Fatalf("unexpected place op: %+v", result.Operations[2])
	}
	if !strings.Contains(result.Operations[2].EquivalentCommand, "--cy 1200000") ||
		!strings.Contains(result.Operations[2].EquivalentCommand, "--name 'Bound Table'") ||
		!strings.Contains(result.Operations[2].EquivalentCommand, "--header") {
		t.Fatalf("place equivalent command missing options: %s", result.Operations[2].EquivalentCommand)
	}
}

func TestPPTXXLSXBindingsApplyMixedOpsJSONReadbackAndValidate(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writePPTXXLSXBindingsWorkbook(t)
	outPath := filepath.Join(t.TempDir(), "bindings.pptx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "apply", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:Q4",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings apply failed: %v", err)
	}

	var result PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal apply JSON: %v\n%s", err, output)
	}
	if result.Output != outPath || result.DryRun || len(result.Operations) != 3 {
		t.Fatalf("unexpected apply result: %+v", result)
	}
	if result.Operations[0].Status != "applied" || result.Operations[0].Text.Value != "Bound Title" {
		t.Fatalf("unexpected replace operation: %+v", result.Operations[0])
	}
	if result.Operations[1].Update == nil || result.Operations[1].Update.UpdatedCells != 9 {
		t.Fatalf("unexpected update operation: %+v", result.Operations[1])
	}
	placed, ok := result.Operations[2].Destination.(map[string]any)
	if !ok {
		t.Fatalf("place destination type = %T", result.Operations[2].Destination)
	}
	if placed["primarySelector"] == "" {
		t.Fatalf("place operation missing destination selectors: %+v", placed)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	titleReadback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "get", outPath,
		"--slide", "1",
		"--target", "title",
		"--include-text",
	)
	if err != nil {
		t.Fatalf("title readback failed: %v", err)
	}
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(titleReadback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal title readback: %v\n%s", err, titleReadback)
	}
	if len(shapes.Shapes) != 1 || shapes.Shapes[0].TextPreview != "Bound Title" {
		t.Fatalf("unexpected title readback: %+v", shapes.Shapes)
	}

	tableReadback := readPPTXTableSummaryForTest(t, outPath)
	if got := tableReadback.Cells[1][1]; got != "42" {
		t.Fatalf("updated table cell B2 = %q, want 42", got)
	}
}

func TestPPTXXLSXBindingsPlanAndApplyImageOpsJSONReadbackAndValidate(t *testing.T) {
	presentationPath := filepath.Join(getTestdataPath(), "pptx", "picture-placeholder", "presentation.pptx")
	imagePath := placeImageTestImagePath()
	workbookPath := writePPTXXLSXImageBindingsWorkbook(t, imagePath)
	outPath := filepath.Join(t.TempDir(), "image-bindings.pptx")

	planOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "plan", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:K3",
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings image plan failed: %v", err)
	}
	var plan PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(planOutput), &plan); err != nil {
		t.Fatalf("failed to unmarshal image plan JSON: %v\n%s", err, planOutput)
	}
	if len(plan.Operations) != 2 {
		t.Fatalf("image plan operation count = %d", len(plan.Operations))
	}
	if plan.Operations[0].Op != "place-image" || plan.Operations[0].Source != nil {
		t.Fatalf("unexpected place-image plan: %+v", plan.Operations[0])
	}
	if plan.Operations[0].Image == nil || plan.Operations[0].Image.Path != imagePath || plan.Operations[0].Image.ContentType != "image/png" {
		t.Fatalf("unexpected place-image metadata: %+v", plan.Operations[0].Image)
	}
	if !strings.Contains(plan.Operations[0].EquivalentCommand, "pptx place image") ||
		!strings.Contains(plan.Operations[0].EquivalentCommand, "--fit-mode cover") ||
		!strings.Contains(plan.Operations[0].EquivalentCommand, "--name 'Bound Image'") {
		t.Fatalf("place-image equivalent command missing options: %s", plan.Operations[0].EquivalentCommand)
	}
	if plan.Operations[1].Op != "replace-image" || plan.Operations[1].Source != nil {
		t.Fatalf("unexpected replace-image plan: %+v", plan.Operations[1])
	}
	if !strings.Contains(plan.Operations[1].ReadbackCommand, "pptx shapes get") ||
		!strings.Contains(plan.Operations[1].EquivalentCommand, "--target shape:2") {
		t.Fatalf("replace-image plan missing readback/normalized target: %+v", plan.Operations[1])
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "apply", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:K3",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings image apply failed: %v", err)
	}
	var result PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal image apply JSON: %v\n%s", err, output)
	}
	if result.Output != outPath || result.DryRun || len(result.Operations) != 2 {
		t.Fatalf("unexpected image apply result: %+v", result)
	}
	placed, ok := result.Operations[0].Destination.(map[string]any)
	if !ok || placed["primarySelector"] == "" || placed["imageRef"] == nil {
		t.Fatalf("place-image destination missing selector/imageRef: %+v", result.Operations[0].Destination)
	}
	replaced, ok := result.Operations[1].Destination.(map[string]any)
	if !ok || replaced["primarySelector"] != "shape:2" || replaced["imageRef"] == nil {
		t.Fatalf("replace-image destination missing selector/imageRef: %+v", result.Operations[1].Destination)
	}
	if result.Operations[1].Image == nil || result.Operations[1].Image.NewContentType != "image/png" {
		t.Fatalf("replace-image metadata missing new image details: %+v", result.Operations[1].Image)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	placedReadback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "get", outPath,
		"--slide", "1",
		"--target", placed["primarySelector"].(string),
		"--include-bounds",
	)
	if err != nil {
		t.Fatalf("placed image readback failed: %v", err)
	}
	var placedShapes PPTXShapesResult
	if err := json.Unmarshal([]byte(placedReadback), &placedShapes); err != nil {
		t.Fatalf("failed to unmarshal placed image readback: %v\n%s", err, placedReadback)
	}
	if len(placedShapes.Shapes) != 1 || placedShapes.Shapes[0].ImageRef == nil {
		t.Fatalf("unexpected placed image readback: %+v", placedShapes.Shapes)
	}
	replaceReadback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "get", outPath,
		"--slide", "2",
		"--target", "shape:2",
		"--include-bounds",
	)
	if err != nil {
		t.Fatalf("replaced image readback failed: %v", err)
	}
	var replacedShapes PPTXShapesResult
	if err := json.Unmarshal([]byte(replaceReadback), &replacedShapes); err != nil {
		t.Fatalf("failed to unmarshal replaced image readback: %v\n%s", err, replaceReadback)
	}
	if len(replacedShapes.Shapes) != 1 || replacedShapes.Shapes[0].ImageRef == nil || replacedShapes.Shapes[0].ImageRef.ContentType != "image/png" {
		t.Fatalf("unexpected replaced image readback: %+v", replacedShapes.Shapes)
	}
}

func TestPPTXXLSXBindingsPlanAndApplySetBoundsJSONReadbackAndValidate(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writePPTXXLSXBoundsBindingsWorkbook(t)
	outPath := filepath.Join(t.TempDir(), "bounds-bindings.pptx")

	planOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "plan", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:H2",
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings bounds plan failed: %v", err)
	}
	var plan PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(planOutput), &plan); err != nil {
		t.Fatalf("failed to unmarshal bounds plan JSON: %v\n%s", err, planOutput)
	}
	if len(plan.Operations) != 1 {
		t.Fatalf("bounds plan operation count = %d", len(plan.Operations))
	}
	op := plan.Operations[0]
	if op.Op != "set-bounds" || op.Source != nil {
		t.Fatalf("unexpected bounds plan operation: %+v", op)
	}
	if op.Bounds == nil || op.Bounds.X != 111111 || op.Bounds.Y != 222222 || op.Bounds.CX != 333333 || op.Bounds.CY != 444444 {
		t.Fatalf("unexpected planned bounds: %+v", op.Bounds)
	}
	plannedDestination, ok := op.Destination.(map[string]any)
	if !ok || plannedDestination["primarySelector"] != "body" {
		t.Fatalf("bounds plan missing destination selector: %+v", op.Destination)
	}
	if !strings.Contains(op.EquivalentCommand, "pptx shapes set-bounds") ||
		!strings.Contains(op.EquivalentCommand, "--bounds 111111,222222,333333,444444") ||
		!strings.Contains(op.ReadbackCommand, "--include-bounds") {
		t.Fatalf("bounds plan missing command hints: %+v", op)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "apply", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:H2",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings bounds apply failed: %v", err)
	}
	var result PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal bounds apply JSON: %v\n%s", err, output)
	}
	if result.Output != outPath || result.DryRun || len(result.Operations) != 1 {
		t.Fatalf("unexpected bounds apply result: %+v", result)
	}
	applied := result.Operations[0]
	if applied.Status != "applied" || applied.Bounds == nil || applied.Bounds.CX != 333333 || applied.Bounds.CY != 444444 {
		t.Fatalf("unexpected bounds apply operation: %+v", applied)
	}
	appliedDestination, ok := applied.Destination.(map[string]any)
	if !ok || appliedDestination["primarySelector"] != "body" {
		t.Fatalf("bounds apply missing destination selector: %+v", applied.Destination)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "get", outPath,
		"--slide", "2",
		"--target", "body",
		"--include-bounds",
	)
	if err != nil {
		t.Fatalf("bounds readback failed: %v", err)
	}
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal bounds readback: %v\n%s", err, readback)
	}
	if len(shapes.Shapes) != 1 {
		t.Fatalf("unexpected bounds readback shape count: %+v", shapes.Shapes)
	}
	bounds := shapes.Shapes[0].Bounds
	if bounds == nil || bounds.X != 111111 || bounds.Y != 222222 || bounds.CX != 333333 || bounds.CY != 444444 {
		t.Fatalf("unexpected bounds readback: %+v", bounds)
	}
}

func TestPPTXXLSXBindingsSetBoundsDryRunDoesNotWrite(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")
	workbookPath := writePPTXXLSXBoundsBindingsWorkbook(t)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "apply", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:H2",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings bounds dry-run failed: %v", err)
	}
	var result PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal bounds dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || len(result.Operations) != 1 {
		t.Fatalf("unexpected bounds dry-run result: %+v", result)
	}
	op := result.Operations[0]
	if op.Status != "dry-run" || op.Bounds == nil || op.Bounds.X != 111111 || op.Bounds.Y != 222222 || op.Bounds.CX != 333333 || op.Bounds.CY != 444444 {
		t.Fatalf("unexpected bounds dry-run operation: %+v", op)
	}
	destination, ok := op.Destination.(map[string]any)
	if !ok || destination["file"] != nil || destination["primarySelector"] != "body" {
		t.Fatalf("unexpected bounds dry-run destination: %+v", op.Destination)
	}

	readback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "get", presentationPath,
		"--slide", "2",
		"--target", "body",
		"--include-bounds",
	)
	if err != nil {
		t.Fatalf("source bounds readback failed: %v", err)
	}
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal source bounds readback: %v\n%s", err, readback)
	}
	if len(shapes.Shapes) != 1 {
		t.Fatalf("unexpected source bounds readback shape count: %+v", shapes.Shapes)
	}
	if bounds := shapes.Shapes[0].Bounds; bounds != nil && bounds.X == 111111 && bounds.Y == 222222 && bounds.CX == 333333 && bounds.CY == 444444 {
		t.Fatalf("source presentation was changed by dry-run: %+v", bounds)
	}
}

func TestPPTXXLSXBindingsApplyDryRunDoesNotWrite(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writePPTXXLSXBindingsWorkbook(t)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "apply", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:Q4",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx xlsx-bindings apply dry-run failed: %v", err)
	}
	var result PPTXXLSXBindingsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || len(result.Operations) != 3 {
		t.Fatalf("unexpected dry-run result: %+v", result)
	}
	for _, op := range result.Operations {
		if op.Status != "dry-run" {
			t.Fatalf("operation status = %q, want dry-run: %+v", op.Status, op)
		}
	}

	titleReadback, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "shapes", "get", presentationPath,
		"--slide", "1",
		"--target", "title",
		"--include-text",
	)
	if err != nil {
		t.Fatalf("title readback failed: %v", err)
	}
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(titleReadback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal title readback: %v\n%s", err, titleReadback)
	}
	if len(shapes.Shapes) != 1 || shapes.Shapes[0].TextPreview == "Bound Title" {
		t.Fatalf("dry-run wrote to source presentation: %+v", shapes.Shapes)
	}
}

func TestPPTXXLSXBindingsRejectsStaleSourceRange(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writePPTXXLSXStaleBindingWorkbook(t)

	_, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "plan", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:G2",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "xlsx-bindings", "plan"}, err, ExitInvalidArgs)
	if err == nil || !strings.Contains(err.Error(), "expect-source-range mismatch") {
		t.Fatalf("error = %v, want expect-source-range mismatch", err)
	}
}

func TestPPTXXLSXBindingsRejectsDuplicateTargets(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writePPTXXLSXDuplicateBindingWorkbook(t)

	_, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "plan", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:G3",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "xlsx-bindings", "plan"}, err, ExitInvalidArgs)
	if err == nil || !strings.Contains(err.Error(), "duplicates destination target") {
		t.Fatalf("error = %v, want duplicate target", err)
	}
}

func TestPPTXXLSXBindingsRejectsNonTextReplaceTargetDuringPlan(t *testing.T) {
	presentationPath := getPPTXTableTestFilePath("table-slide")
	workbookPath := writePPTXXLSXNonTextTargetBindingWorkbook(t)

	_, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "xlsx-bindings", "plan", presentationPath,
		"--workbook", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:G2",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "xlsx-bindings", "plan"}, err, ExitInvalidArgs)
	if err == nil || !strings.Contains(err.Error(), "non-text") {
		t.Fatalf("error = %v, want non-text target", err)
	}
}

func TestPPTXXLSXBindingsRejectsBadImageOps(t *testing.T) {
	presentationPath := filepath.Join(getTestdataPath(), "pptx", "picture-placeholder", "presentation.pptx")
	imagePath := placeImageTestImagePath()
	missingImagePath := filepath.Join(t.TempDir(), "missing.png")

	tests := []struct {
		name string
		row  []xlsxBindingCell
		code int
		want string
	}{
		{
			name: "missing image",
			row:  []xlsxBindingCell{{"A2", "missing"}, {"B2", "place-image"}, {"C2", "1"}, {"E2", missingImagePath}, {"G2", "0"}, {"H2", "0"}, {"I2", "1000"}, {"J2", "1000"}},
			code: ExitFileNotFound,
			want: "not found",
		},
		{
			name: "invalid fit",
			row:  []xlsxBindingCell{{"A2", "fit"}, {"B2", "place-image"}, {"C2", "1"}, {"E2", imagePath}, {"F2", "squash"}, {"G2", "0"}, {"H2", "0"}, {"I2", "1000"}, {"J2", "1000"}},
			code: ExitInvalidArgs,
			want: "fit mode",
		},
		{
			name: "zero dimension",
			row:  []xlsxBindingCell{{"A2", "zero"}, {"B2", "place-image"}, {"C2", "1"}, {"E2", imagePath}, {"G2", "0"}, {"H2", "0"}, {"I2", "1000"}, {"J2", "0"}},
			code: ExitInvalidArgs,
			want: "cx and cy",
		},
		{
			name: "non image target",
			row:  []xlsxBindingCell{{"A2", "target"}, {"B2", "replace-image"}, {"C2", "1"}, {"D2", "title"}, {"E2", imagePath}},
			code: ExitInvalidArgs,
			want: "not an image",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			workbookPath := writePPTXXLSXImageBindingRowsWorkbook(t, tt.row)
			_, err := executeRootForXLSXTest(t,
				"--format", "json",
				"pptx", "xlsx-bindings", "plan", presentationPath,
				"--workbook", workbookPath,
				"--sheet", "Sheet1",
				"--range", "A1:K2",
			)
			assertCLIExitCodeForXLSXTest(t, []string{"pptx", "xlsx-bindings", "plan"}, err, tt.code)
			if err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("error = %v, want containing %q", err, tt.want)
			}
		})
	}
}

func TestPPTXXLSXBindingsRejectsBadBoundsOps(t *testing.T) {
	presentationPath := pptxShapesFixturePath(t, "title-content")

	tests := []struct {
		name string
		row  []xlsxBindingCell
		code int
		want string
	}{
		{
			name: "missing target",
			row:  []xlsxBindingCell{{"A2", "missing"}, {"B2", "set-bounds"}, {"C2", "2"}, {"E2", "0"}, {"F2", "0"}, {"G2", "1000"}, {"H2", "1000"}},
			code: ExitInvalidArgs,
			want: "target is required",
		},
		{
			name: "zero dimension",
			row:  []xlsxBindingCell{{"A2", "zero"}, {"B2", "set-bounds"}, {"C2", "2"}, {"D2", "body"}, {"E2", "0"}, {"F2", "0"}, {"G2", "1000"}, {"H2", "0"}},
			code: ExitInvalidArgs,
			want: "cx and cy",
		},
		{
			name: "missing coordinate",
			row:  []xlsxBindingCell{{"A2", "missing-x"}, {"B2", "set-bounds"}, {"C2", "2"}, {"D2", "body"}, {"F2", "0"}, {"G2", "1000"}, {"H2", "1000"}},
			code: ExitInvalidArgs,
			want: "x, y, cx, and cy",
		},
		{
			name: "missing shape",
			row:  []xlsxBindingCell{{"A2", "missing-shape"}, {"B2", "set-bounds"}, {"C2", "2"}, {"D2", "shape:999"}, {"E2", "0"}, {"F2", "0"}, {"G2", "1000"}, {"H2", "1000"}},
			code: ExitTargetNotFound,
			want: "shape:999",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			workbookPath := writePPTXXLSXBoundsBindingRowsWorkbook(t, tt.row)
			_, err := executeRootForXLSXTest(t,
				"--format", "json",
				"pptx", "xlsx-bindings", "plan", presentationPath,
				"--workbook", workbookPath,
				"--sheet", "Sheet1",
				"--range", "A1:H2",
			)
			assertCLIExitCodeForXLSXTest(t, []string{"pptx", "xlsx-bindings", "plan"}, err, tt.code)
			if err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("error = %v, want containing %q", err, tt.want)
			}
		})
	}
}

type xlsxBindingCell struct {
	Ref   string
	Value string
}

func writePPTXXLSXBindingsWorkbook(t *testing.T) string {
	t.Helper()
	cells := []xlsxBindingCell{
		{"A1", "id"}, {"B1", "op"}, {"C1", "slide"}, {"D1", "target"}, {"E1", "sourceSheet"}, {"F1", "sourceRange"}, {"G1", "expectSourceRange"}, {"H1", "formulaMode"}, {"I1", "mode"}, {"J1", "rowSep"}, {"K1", "colSep"}, {"L1", "x"}, {"M1", "y"}, {"N1", "cx"}, {"O1", "cy"}, {"P1", "name"}, {"Q1", "header"},
		{"A2", "title-update"}, {"B2", "replace-text"}, {"C2", "1"}, {"D2", "title"}, {"E2", "Sheet1"}, {"F2", "AA1"}, {"G2", "AA1"}, {"H2", "value"}, {"I2", "preserve-format"}, {"J2", `\n`}, {"K2", " | "},
		{"A3", "table-update"}, {"B3", "update-table"}, {"C3", "2"}, {"D3", "table:1"}, {"E3", "Sheet1"}, {"F3", "AA3:AC5"}, {"G3", "AA3:AC5"}, {"H3", "Formula"},
		{"A4", "table-place"}, {"B4", "place-table"}, {"C4", "1"}, {"E4", "Sheet1"}, {"F4", "AA7:AB8"}, {"G4", "AA7:AB8"}, {"H4", "value"}, {"L4", "0"}, {"M4", "2500000"}, {"N4", "2500000"}, {"O4", "1200000"}, {"P4", "Bound Table"}, {"Q4", "true"},
		{"AA1", "Bound Title"},
		{"AA3", "A"}, {"AB3", "B"}, {"AC3", "C"},
		{"AA4", "North"}, {"AB4", "42"}, {"AC4", "ok"},
		{"AA5", "South"}, {"AB5", "55"}, {"AC5", "done"},
		{"AA7", "Name"}, {"AB7", "Score"},
		{"AA8", "East"}, {"AB8", "9"},
	}
	return writeTestXLSXWithSheetXML(t, worksheetXMLFromBindingCells(t, "A1:AC8", cells))
}

func writePPTXXLSXDuplicateBindingWorkbook(t *testing.T) string {
	t.Helper()
	cells := []xlsxBindingCell{
		{"A1", "id"}, {"B1", "op"}, {"C1", "slide"}, {"D1", "target"}, {"E1", "sourceSheet"}, {"F1", "sourceRange"}, {"G1", "expectSourceRange"},
		{"A2", "one"}, {"B2", "replace-text"}, {"C2", "1"}, {"D2", "title"}, {"E2", "Sheet1"}, {"F2", "AA1"}, {"G2", "AA1"},
		{"A3", "two"}, {"B3", "replace-text"}, {"C3", "1"}, {"D3", "title"}, {"E3", "Sheet1"}, {"F3", "AA2"}, {"G3", "AA2"},
		{"AA1", "One"}, {"AA2", "Two"},
	}
	return writeTestXLSXWithSheetXML(t, worksheetXMLFromBindingCells(t, "A1:AA3", cells))
}

func writePPTXXLSXStaleBindingWorkbook(t *testing.T) string {
	t.Helper()
	cells := []xlsxBindingCell{
		{"A1", "id"}, {"B1", "op"}, {"C1", "slide"}, {"D1", "target"}, {"E1", "sourceSheet"}, {"F1", "sourceRange"}, {"G1", "expectSourceRange"},
		{"A2", "stale"}, {"B2", "replace-text"}, {"C2", "1"}, {"D2", "title"}, {"E2", "Sheet1"}, {"F2", "AA1"}, {"G2", "AA2"},
		{"AA1", "One"},
	}
	return writeTestXLSXWithSheetXML(t, worksheetXMLFromBindingCells(t, "A1:AA2", cells))
}

func writePPTXXLSXNonTextTargetBindingWorkbook(t *testing.T) string {
	t.Helper()
	cells := []xlsxBindingCell{
		{"A1", "id"}, {"B1", "op"}, {"C1", "slide"}, {"D1", "target"}, {"E1", "sourceSheet"}, {"F1", "sourceRange"}, {"G1", "expectSourceRange"},
		{"A2", "bad-target"}, {"B2", "replace-text"}, {"C2", "2"}, {"D2", "table:1"}, {"E2", "Sheet1"}, {"F2", "AA1"}, {"G2", "AA1"},
		{"AA1", "One"},
	}
	return writeTestXLSXWithSheetXML(t, worksheetXMLFromBindingCells(t, "A1:AA2", cells))
}

func writePPTXXLSXImageBindingsWorkbook(t *testing.T, imagePath string) string {
	t.Helper()
	return writePPTXXLSXImageBindingRowsWorkbook(t, []xlsxBindingCell{
		{"A2", "place-hero"}, {"B2", "place-image"}, {"C2", "1"}, {"E2", imagePath}, {"F2", "cover"}, {"G2", "0"}, {"H2", "2200000"}, {"I2", "1800000"}, {"J2", "1200000"}, {"K2", "Bound Image"},
		{"A3", "replace-photo"}, {"B3", "replace-image"}, {"C3", "2"}, {"D3", "~Picture 1"}, {"E3", imagePath}, {"F3", "contain"},
	})
}

func writePPTXXLSXImageBindingRowsWorkbook(t *testing.T, rowCells []xlsxBindingCell) string {
	t.Helper()
	cells := []xlsxBindingCell{
		{"A1", "id"}, {"B1", "op"}, {"C1", "slide"}, {"D1", "target"}, {"E1", "imagePath"}, {"F1", "fitMode"}, {"G1", "x"}, {"H1", "y"}, {"I1", "cx"}, {"J1", "cy"}, {"K1", "name"},
	}
	cells = append(cells, rowCells...)
	return writeTestXLSXWithSheetXML(t, worksheetXMLFromBindingCells(t, "A1:K3", cells))
}

func writePPTXXLSXBoundsBindingsWorkbook(t *testing.T) string {
	t.Helper()
	return writePPTXXLSXBoundsBindingRowsWorkbook(t, []xlsxBindingCell{
		{"A2", "body-layout"}, {"B2", "set-shape-bounds"}, {"C2", "2"}, {"D2", "body"}, {"E2", "111111"}, {"F2", "222222"}, {"G2", "333333"}, {"H2", "444444"},
	})
}

func writePPTXXLSXBoundsBindingRowsWorkbook(t *testing.T, rowCells []xlsxBindingCell) string {
	t.Helper()
	cells := []xlsxBindingCell{
		{"A1", "id"}, {"B1", "op"}, {"C1", "slide"}, {"D1", "target"}, {"E1", "x"}, {"F1", "y"}, {"G1", "cx"}, {"H1", "cy"},
	}
	cells = append(cells, rowCells...)
	return writeTestXLSXWithSheetXML(t, worksheetXMLFromBindingCells(t, "A1:H2", cells))
}

func worksheetXMLFromBindingCells(t *testing.T, dimension string, cells []xlsxBindingCell) string {
	t.Helper()
	byRow := map[int][]xlsxBindingCell{}
	for _, cell := range cells {
		row := bindingCellRowForTest(t, cell.Ref)
		byRow[row] = append(byRow[row], cell)
	}
	rows := make([]int, 0, len(byRow))
	for row := range byRow {
		rows = append(rows, row)
	}
	sort.Ints(rows)

	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>` + "\n")
	builder.WriteString(`<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">` + "\n")
	builder.WriteString(fmt.Sprintf(`  <dimension ref="%s"/>`, dimension) + "\n")
	builder.WriteString("  <sheetData>\n")
	for _, row := range rows {
		sort.Slice(byRow[row], func(i, j int) bool {
			return byRow[row][i].Ref < byRow[row][j].Ref
		})
		builder.WriteString(fmt.Sprintf(`    <row r="%d">`, row))
		for _, cell := range byRow[row] {
			builder.WriteString(fmt.Sprintf(`<c r="%s" t="inlineStr"><is><t>%s</t></is></c>`, cell.Ref, escapeXMLTextForBindingTest(cell.Value)))
		}
		builder.WriteString("</row>\n")
	}
	builder.WriteString("  </sheetData>\n")
	builder.WriteString("</worksheet>")
	return builder.String()
}

func escapeXMLTextForBindingTest(value string) string {
	var buf bytes.Buffer
	_ = xml.EscapeText(&buf, []byte(value))
	return buf.String()
}

func bindingCellRowForTest(t *testing.T, ref string) int {
	t.Helper()
	var digits strings.Builder
	for _, r := range ref {
		if r >= '0' && r <= '9' {
			digits.WriteRune(r)
		}
	}
	row, err := strconv.Atoi(digits.String())
	if err != nil {
		t.Fatalf("bad cell ref %q: %v", ref, err)
	}
	return row
}
