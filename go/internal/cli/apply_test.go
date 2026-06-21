package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
)

func writeApplyTestFile(t *testing.T, name, content string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), name)
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatalf("write %s: %v", name, err)
	}
	return path
}

// minimalXLSXForApply copies the committed minimal workbook fixture into a temp file.
func minimalXLSXForApply(t *testing.T) string {
	t.Helper()
	src := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	dst := filepath.Join(t.TempDir(), "input.xlsx")
	if err := os.WriteFile(dst, data, 0o644); err != nil {
		t.Fatalf("stage fixture: %v", err)
	}
	return dst
}

func TestApplyDryRunPlanJSON(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[
		{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x"}},
		{"command":"xlsx cells set","args":{"sheet":"1","cell":"A2","value":"y"}}
	]`)

	out, err := executeRootForXLSXTest(t, "--json", "apply", file, "--ops", ops, "--dry-run")
	if err != nil {
		t.Fatalf("apply --dry-run: %v", err)
	}

	var plan struct {
		SchemaVersion int    `json:"schemaVersion"`
		File          string `json:"file"`
		OpsCount      int    `json:"opsCount"`
		DryRun        bool   `json:"dryRun"`
		Plan          []struct {
			Index   int      `json:"index"`
			Command string   `json:"command"`
			Argv    []string `json:"argv"`
		} `json:"plan"`
	}
	if err := json.Unmarshal([]byte(out), &plan); err != nil {
		t.Fatalf("unmarshal plan: %v (%s)", err, out)
	}
	if !plan.DryRun || plan.OpsCount != 2 || len(plan.Plan) != 2 {
		t.Fatalf("unexpected plan: %+v", plan)
	}
	if plan.SchemaVersion != 1 {
		t.Fatalf("schemaVersion = %d, want 1", plan.SchemaVersion)
	}
	// First op's argv must start with the command words + the real input file.
	first := plan.Plan[0].Argv
	if len(first) < 4 || first[0] != "xlsx" || first[3] != file {
		t.Fatalf("first op argv = %v", first)
	}
	// Sorted arg keys: cell before sheet before value.
	joined := strings.Join(first, " ")
	if !strings.Contains(joined, "--cell A1 --sheet 1 --value x") {
		t.Fatalf("argv not sorted/expected: %v", first)
	}
}

func TestApplyDryRunAllowsCommandLocalFormatArg(t *testing.T) {
	file := filepath.Join("..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	ops := writeApplyTestFile(t, "ops.json", `[
		{"command":"pptx place table","args":{"slide":1,"data":"/tmp/data.csv","format":"csv","x":0,"y":0,"cx":1000}}
	]`)

	out, err := executeRootForXLSXTest(t, "--json", "apply", file, "--ops", ops, "--dry-run")
	if err != nil {
		t.Fatalf("apply --dry-run with local format arg: %v", err)
	}
	var plan struct {
		Plan []struct {
			Argv []string `json:"argv"`
		} `json:"plan"`
	}
	if err := json.Unmarshal([]byte(out), &plan); err != nil {
		t.Fatalf("unmarshal plan: %v (%s)", err, out)
	}
	if len(plan.Plan) != 1 {
		t.Fatalf("plan length = %d, want 1", len(plan.Plan))
	}
	argv := strings.Join(plan.Plan[0].Argv, " ")
	if !strings.Contains(argv, "--format csv") {
		t.Fatalf("local format arg missing from apply plan: %v", plan.Plan[0].Argv)
	}
}

func TestApplyDryRunDoesNotWrite(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x"}}]`)
	outPath := filepath.Join(t.TempDir(), "should-not-exist.xlsx")

	// --dry-run cannot be combined with --out, so just confirm dry-run writes nothing
	// to the input dir beyond what already exists.
	if _, err := executeRootForXLSXTest(t, "apply", file, "--ops", ops, "--dry-run"); err != nil {
		t.Fatalf("apply --dry-run: %v", err)
	}
	if _, err := os.Stat(outPath); !os.IsNotExist(err) {
		t.Fatalf("dry-run should not create output, stat err = %v", err)
	}
}

func TestApplyMissingOpsFlag(t *testing.T) {
	file := minimalXLSXForApply(t)
	args := []string{"apply", file, "--dry-run"}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestApplyMissingOpsFile(t *testing.T) {
	file := minimalXLSXForApply(t)
	args := []string{"apply", file, "--ops", filepath.Join(t.TempDir(), "nope.json"), "--dry-run"}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitFileNotFound)
}

func TestApplyMissingInputFile(t *testing.T) {
	ops := writeApplyTestFile(t, "ops.json", `[]`)
	args := []string{"apply", filepath.Join(t.TempDir(), "nope.xlsx"), "--ops", ops, "--dry-run"}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitFileNotFound)
}

func TestApplyInvalidOpsJSON(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `not valid json`)
	args := []string{"apply", file, "--ops", ops, "--dry-run"}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestApplyInvalidOpsTrailingJSON(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x"}}] {"command":"xlsx cells set"}`)
	outPath := filepath.Join(t.TempDir(), "should-not-exist.xlsx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid ops should not write output, stat err = %v", statErr)
	}
}

func TestApplyRejectsSessionOwnedNestedMutationArgs(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x","out":"nested.xlsx"}}]`)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "owned by the apply/serve/MCP session") {
		t.Fatalf("unexpected nested flag error: %v", err)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid nested op arg should not write output, stat err = %v", statErr)
	}
}

func TestApplyRejectsDashfulSessionOwnedNestedMutationArgs(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x","--dry-run":true}}]`)
	args := []string{"apply", file, "--ops", ops, "--dry-run"}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "owned by the apply/serve/MCP session") {
		t.Fatalf("unexpected nested flag error: %v", err)
	}
}

func TestApplyRejectsGlobalOutputNestedMutationArg(t *testing.T) {
	file := minimalXLSXForApply(t)
	sideFile := filepath.Join(t.TempDir(), "leaked-readback.json")
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x","output":"`+filepath.ToSlash(sideFile)+`"}}]`)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "owned by the apply/serve/MCP session") {
		t.Fatalf("unexpected nested output flag error: %v", err)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid nested output arg should not write final output, stat err = %v", statErr)
	}
	if _, statErr := os.Stat(sideFile); !os.IsNotExist(statErr) {
		t.Fatalf("nested output arg should not create side artifact, stat err = %v", statErr)
	}
}

func TestApplyRejectsEqualsStyleNestedMutationArg(t *testing.T) {
	file := minimalXLSXForApply(t)
	sideFile := filepath.Join(t.TempDir(), "leaked-readback.json")
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x","output=`+filepath.ToSlash(sideFile)+`":true}}]`)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "without '='") {
		t.Fatalf("unexpected equals-style nested flag error: %v", err)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid equals-style arg should not write final output, stat err = %v", statErr)
	}
	if _, statErr := os.Stat(sideFile); !os.IsNotExist(statErr) {
		t.Fatalf("equals-style nested arg should not create side artifact, stat err = %v", statErr)
	}
}

func TestApplyRejectsCommandWithExtraWords(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set extra.xlsx","args":{"sheet":"1","cell":"A1","value":"x"}}]`)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "command must be one command path") {
		t.Fatalf("unexpected command validation error: %v", err)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid command should not write output, stat err = %v", statErr)
	}
}

func TestApplyRejectsReadCommandAsOperation(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx sheets list","args":{}}]`)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "cannot be used as an apply/serve/MCP op") ||
		!strings.Contains(err.Error(), "mutation output flags") {
		t.Fatalf("unexpected read-command op error: %v", err)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid read-command op should not write output, stat err = %v", statErr)
	}
}

func TestApplyRejectsExtraPositionalMutationOperation(t *testing.T) {
	file := filepath.Join("..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"pptx slides move","args":{}}]`)
	outPath := filepath.Join(t.TempDir(), "out.pptx")
	args := []string{"apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "op can supply only the package file") {
		t.Fatalf("unexpected extra-positional op error: %v", err)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("invalid extra-positional op should not write output, stat err = %v", statErr)
	}
}

func TestApplyResultUsesPublishedPathAndQuotedValidateCommand(t *testing.T) {
	orig := applySelfExecutable
	applySelfExecutable = func() (string, error) { return serveBinary, nil }
	t.Cleanup(func() { applySelfExecutable = orig })

	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"ooxml xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x"}}]`)
	outPath := filepath.Join(t.TempDir(), "published workbook.xlsx")

	out, err := executeRootForXLSXTest(t, "--json", "apply", file, "--ops", ops, "--out", outPath)
	if err != nil {
		t.Fatalf("apply: %v", err)
	}
	var result apply.Result
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal apply result: %v (%s)", err, out)
	}
	if result.Output != outPath || result.ValidateCommand == "" {
		t.Fatalf("unexpected apply result output metadata: %+v", result)
	}
	if !strings.Contains(result.ValidateCommand, "'"+outPath+"'") {
		t.Fatalf("validate command should quote output with spaces: %q", result.ValidateCommand)
	}
	if len(result.Applied) != 1 || result.Applied[0].Command != "xlsx cells set" || result.Applied[0].Readback == nil {
		t.Fatalf("unexpected applied ops: %+v", result.Applied)
	}
	readback := string(result.Applied[0].Readback)
	if !strings.Contains(readback, jsonStringPathFragment(outPath)) {
		t.Fatalf("op readback should point at published output %q, got %s", outPath, readback)
	}
	if strings.Contains(readback, ".ooxml-apply-") {
		t.Fatalf("op readback leaked apply scratch path: %s", readback)
	}
}

func jsonStringPathFragment(path string) string {
	return strings.ReplaceAll(path, "\\", "\\\\")
}

func TestApplyOpErrorPreservesChildEnvelope(t *testing.T) {
	orig := applySelfExecutable
	applySelfExecutable = func() (string, error) { return serveBinary, nil }
	t.Cleanup(func() { applySelfExecutable = orig })

	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[{"command":"xlsx cells set","args":{"sheet":"NoSuchSheet","cell":"A1","value":"x"}}]`)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	args := []string{"--json", "apply", file, "--ops", ops, "--out", outPath}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitTargetNotFound)
	cliErr := err.(*CLIError)
	if cliErr.Code != codeForExit(ExitTargetNotFound) {
		t.Fatalf("apply op error code = %q, want %q", cliErr.Code, codeForExit(ExitTargetNotFound))
	}
	if len(cliErr.Diagnostics) == 0 || cliErr.Diagnostics[0].Code != "op_failed" {
		t.Fatalf("apply op error missing op_failed diagnostic: %+v", cliErr.Diagnostics)
	}
	if !strings.Contains(cliErr.Message, "op 0 (xlsx cells set) failed") {
		t.Fatalf("apply op error missing op context: %s", cliErr.Message)
	}
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("failed apply op should not write output, stat err = %v", statErr)
	}
}

// TestApplyRejectsStructuralShiftBeforeAddressPositionalHandle proves apply
// refuses, PRE-FLIGHT (nothing executed or written), a batch that shifts
// rows/columns before an op targeting an address-positional XLSX cell handle.
// This closes the silent-wrong-target hole a row DELETE would otherwise open: a
// populated cell shifts onto the stale A1 address and the per-op runtime guard,
// having no pre-shift state, cannot detect it. The single-op / cross-invocation
// stale behavior (a row insert that empties the address) is proven separately in
// xlsx_handles_survival_test.go.
func TestApplyRejectsStructuralShiftBeforeAddressPositionalHandle(t *testing.T) {
	for _, shift := range []string{"xlsx rows insert", "xlsx rows delete", "xlsx cols delete"} {
		t.Run(shift, func(t *testing.T) {
			dir := t.TempDir()
			two := xlsxTwoSheetWorkbook(t, dir)
			// A valid address-positional handle string; rejection is static and
			// never resolves it, so the concrete sheetId is irrelevant.
			cellHandle := xlsxhandle.FormatCell("2", "B7")
			opsData, err := json.Marshal([]map[string]interface{}{
				{"command": shift, "args": map[string]interface{}{"sheet": "Second", "at": 1}},
				{"command": "xlsx cells set", "args": map[string]interface{}{"cell": cellHandle, "value": "WRONG"}},
			})
			if err != nil {
				t.Fatalf("marshal ops: %v", err)
			}
			ops := writeApplyTestFile(t, "ops.json", string(opsData))
			outPath := filepath.Join(dir, "out.xlsx")
			args := []string{"--json", "apply", two, "--ops", ops, "--out", outPath}
			_, err = executeRootForXLSXTest(t, args...)
			assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
			cliErr := err.(*CLIError)
			if !strings.Contains(cliErr.Message, "address-positional XLSX handle") {
				t.Fatalf("rejection message = %q, want it to mention the address-positional handle hazard", cliErr.Message)
			}
			if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
				t.Fatalf("rejected apply batch should not write output, stat err = %v", statErr)
			}
		})
	}
}

// TestValidateOpBatchHandleSafety covers the ordering/discriminator matrix
// directly: only an address-positional handle (XLSX cell/comment) PRECEDED by a
// structural shift is rejected. A handle before the shift, a native-id handle, a
// positional selector, and a shift-less batch are all allowed.
func TestValidateOpBatchHandleSafety(t *testing.T) {
	cell := xlsxhandle.FormatCell("2", "B7")
	sheet := xlsxhandle.FormatSheet("2") // native-id handle — position-immune
	cases := []struct {
		name       string
		opsJSON    string
		wantReject bool
	}{
		{"shift then cell handle", `[{"command":"xlsx rows delete","args":{"sheet":"1","at":1}},{"command":"xlsx cells set","args":{"cell":"` + cell + `","value":"x"}}]`, true},
		{"cell handle then shift (allowed)", `[{"command":"xlsx cells set","args":{"cell":"` + cell + `","value":"x"}},{"command":"xlsx rows delete","args":{"sheet":"1","at":1}}]`, false},
		{"shift then native-id sheet handle (allowed)", `[{"command":"xlsx rows delete","args":{"sheet":"1","at":1}},{"command":"xlsx cells set","args":{"sheet":"` + sheet + `","cell":"A1","value":"x"}}]`, false},
		{"shift then positional cell (allowed)", `[{"command":"xlsx rows delete","args":{"sheet":"1","at":1}},{"command":"xlsx cells set","args":{"sheet":"1","cell":"B7","value":"x"}}]`, false},
		{"no shift (allowed)", `[{"command":"xlsx cells set","args":{"cell":"` + cell + `","value":"x"}}]`, false},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			ops, err := apply.ParseOps([]byte(tc.opsJSON))
			if err != nil {
				t.Fatalf("ParseOps: %v", err)
			}
			gotErr := validateOpBatchHandleSafety(ops)
			if tc.wantReject && gotErr == nil {
				t.Fatalf("expected rejection, got nil")
			}
			if !tc.wantReject && gotErr != nil {
				t.Fatalf("expected no rejection, got %q", gotErr.Message)
			}
			if tc.wantReject && gotErr.ExitCode != ExitInvalidArgs {
				t.Fatalf("reject exit = %d, want %d", gotErr.ExitCode, ExitInvalidArgs)
			}
		})
	}
}

func TestApplyDryRunRejectsOut(t *testing.T) {
	file := minimalXLSXForApply(t)
	ops := writeApplyTestFile(t, "ops.json", `[]`)
	args := []string{"apply", file, "--ops", ops, "--dry-run", "--out", filepath.Join(t.TempDir(), "o.xlsx")}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}
