package apply

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"syscall"
	"testing"
)

// builtBinary is the path to a freshly built ./cmd/ooxml, used as Executor.Self
// so tests exercise the real subprocess path. Built once in TestMain.
var builtBinary string

func TestMain(m *testing.M) {
	if mode := os.Getenv("OOXML_APPLY_FAKE_CHILD"); mode != "" {
		os.Exit(runFakeApplyChild(mode))
	}

	dir, err := os.MkdirTemp("", "ooxml-apply-bin-*")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(dir)

	binaryName := "ooxml"
	if runtime.GOOS == "windows" {
		binaryName += ".exe"
	}
	builtBinary = filepath.Join(dir, binaryName)
	build := exec.Command("go", "build", "-o", builtBinary, "github.com/ooxml-cli/ooxml-cli/cmd/ooxml")
	build.Stdout = os.Stderr
	build.Stderr = os.Stderr
	if err := build.Run(); err != nil {
		panic("failed to build ooxml binary for tests: " + err.Error())
	}

	os.Exit(m.Run())
}

func runFakeApplyChild(mode string) int {
	out := argAfter(os.Args[1:], "--out")
	switch mode {
	case "zero":
		return 0
	case "corrupt":
		if out == "" {
			return 2
		}
		if err := os.WriteFile(out, []byte("not-a-zip"), 0o644); err != nil {
			fmt.Fprintln(os.Stderr, err)
			return 1
		}
		return 0
	case "copy":
		src := os.Getenv("OOXML_APPLY_FAKE_SOURCE")
		if src == "" || out == "" {
			return 2
		}
		data, err := os.ReadFile(src)
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			return 1
		}
		if err := os.WriteFile(out, data, 0o644); err != nil {
			fmt.Fprintln(os.Stderr, err)
			return 1
		}
		return 0
	default:
		fmt.Fprintf(os.Stderr, "unknown fake child mode %q\n", mode)
		return 2
	}
}

func argAfter(args []string, flag string) string {
	for i, arg := range args {
		if arg == flag && i+1 < len(args) {
			return args[i+1]
		}
	}
	return ""
}

// repoRoot walks up from the test working directory (pkg/apply) to the module root.
func repoRoot(t *testing.T) string {
	t.Helper()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	// pkg/apply -> repo root is two levels up.
	root := filepath.Join(wd, "..", "..")
	if _, err := os.Stat(filepath.Join(root, "go.mod")); err != nil {
		t.Fatalf("could not locate repo root from %s: %v", wd, err)
	}
	return root
}

// stageFixture copies the minimal workbook fixture into a temp dir and returns
// the copy's path.
func stageFixture(t *testing.T) string {
	t.Helper()
	src := filepath.Join(repoRoot(t), "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
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

func mustArg(t *testing.T, v string) Arg {
	t.Helper()
	b, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	return Arg{raw: b}
}

func mustJSONArg(t *testing.T, v any) Arg {
	t.Helper()
	b, err := json.Marshal(v)
	if err != nil {
		t.Fatal(err)
	}
	return Arg{raw: b}
}

func TestBuildArgv(t *testing.T) {
	op := Operation{
		Command: "xlsx cells set",
		Args: map[string]Arg{
			"value":       mustArg(t, "x"),
			"cell":        mustArg(t, "A1"),
			"dry-run":     mustJSONArg(t, false),
			"include-all": mustJSONArg(t, true),
			"sheet":       mustArg(t, "1"),
		},
	}
	got := buildArgv(op, "in.xlsx", "out.xlsx")
	// Keys sorted; JSON bools use --flag=true/false so Cobra/pflag does not
	// treat the bool value as an extra positional arg.
	want := []string{
		"xlsx", "cells", "set", "in.xlsx",
		"--cell", "A1",
		"--dry-run=false",
		"--include-all=true",
		"--sheet", "1",
		"--value", "x",
		"--out", "out.xlsx", "--json", "--no-validate",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("buildArgv = %v, want %v", got, want)
	}
}

func TestBuildArgvAcceptsDashfulArgNames(t *testing.T) {
	op := Operation{
		Command: "xlsx cells set",
		Args: map[string]Arg{
			"--cell":  mustArg(t, "A1"),
			"--sheet": mustArg(t, "1"),
			"--value": mustArg(t, "x"),
		},
	}
	got := buildArgv(op, "in.xlsx", "out.xlsx")
	joined := strings.Join(got, " ")
	if strings.Contains(joined, "----") {
		t.Fatalf("dashful JSON arg keys must not produce double-prefixed flags: %v", got)
	}
	for _, want := range []string{"--cell", "--sheet", "--value"} {
		if !containsArgForApplyTest(got, want) {
			t.Fatalf("missing %s in argv: %v", want, got)
		}
	}
}

func TestBuildArgvNormalizesJSONFriendlyArgNames(t *testing.T) {
	op := Operation{
		Command: "pptx replace text-occurrences",
		Args: map[string]Arg{
			"expectCount":    mustJSONArg(t, 2),
			"expectPlanHash": mustArg(t, "sha256:abc"),
			"forSlides":      mustArg(t, "H:pptx/s:257"),
			"ignoreCase":     mustJSONArg(t, true),
			"matchText":      mustArg(t, "Old"),
			"newText":        mustArg(t, "New"),
		},
	}
	got := buildArgv(op, "in.pptx", "out.pptx")
	for _, want := range []string{
		"--expect-count", "2",
		"--expect-plan-hash", "sha256:abc",
		"--for-slides", "H:pptx/s:257",
		"--ignore-case=true",
		"--match-text", "Old",
		"--new-text", "New",
	} {
		if !containsArgForApplyTest(got, want) {
			t.Fatalf("buildArgv missing normalized arg %q: %v", want, got)
		}
	}
}

func TestArgString(t *testing.T) {
	cases := []struct {
		raw  string
		want string
	}{
		{`"hello"`, "hello"},
		{`1`, "1"},
		{`1.5`, "1.5"},
		{`true`, "true"},
		{`false`, "false"},
		{`null`, ""},
	}
	for _, c := range cases {
		a := Arg{raw: json.RawMessage(c.raw)}
		if got := a.String(); got != c.want {
			t.Errorf("Arg(%s).String() = %q, want %q", c.raw, got, c.want)
		}
	}
}

func containsArgForApplyTest(args []string, want string) bool {
	for _, arg := range args {
		if arg == want || strings.HasPrefix(arg, want+"=") {
			return true
		}
	}
	return false
}

func jsonPathFragmentForApplyTest(path string) string {
	return strings.ReplaceAll(path, "\\", "\\\\")
}

func TestParseOps(t *testing.T) {
	ops, err := ParseOps([]byte(`[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x"}}]`))
	if err != nil {
		t.Fatalf("ParseOps: %v", err)
	}
	if len(ops) != 1 || ops[0].Command != "xlsx cells set" {
		t.Fatalf("unexpected ops: %+v", ops)
	}
	if got := ops[0].Args["sheet"].String(); got != "1" {
		t.Fatalf("sheet arg = %q", got)
	}

	if _, err := ParseOps([]byte(`not json`)); err == nil {
		t.Fatal("expected error for invalid JSON")
	}
	if _, err := ParseOps([]byte(`[]`)); err != nil {
		t.Fatalf("empty array should parse: %v", err)
	}
	if _, err := ParseOps([]byte(`[{"args":{}}]`)); err == nil {
		t.Fatal("expected error for missing command")
	}
	ops, err = ParseOps([]byte(`[{"command":"ooxml xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x"}}]`))
	if err != nil {
		t.Fatalf("ParseOps should accept capabilities command paths: %v", err)
	}
	if ops[0].Command != "xlsx cells set" {
		t.Fatalf("capabilities command path normalized to %q, want xlsx cells set", ops[0].Command)
	}
	if _, err := ParseOps([]byte(`[{"command":"xlsx cells set","args":{}}] {"command":"xlsx sheets list"}`)); err == nil {
		t.Fatal("expected error for trailing JSON after ops array")
	}
	if _, err := ParseOps([]byte(`[{"command":"xlsx cells set","args":{},"unknown":true}]`)); err == nil {
		t.Fatal("expected error for unknown op field")
	}
	if _, err := ParseOps([]byte(`[{"command":"xlsx cells set --sheet 1","args":{"cell":"A1","value":"x"}}]`)); err == nil {
		t.Fatal("expected error for flag embedded in command")
	} else if !strings.Contains(err.Error(), `put flag "--sheet" in args`) {
		t.Fatalf("embedded flag error = %v", err)
	}
	if _, err := ParseOps([]byte(`[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x","output=/tmp/leak.json":true}}]`)); err == nil {
		t.Fatal("expected error for arg key with equals")
	} else if !strings.Contains(err.Error(), "without '='") {
		t.Fatalf("equals arg key error = %v", err)
	}
}

func TestParseOpsRejectsSessionOwnedMutationArgs(t *testing.T) {
	for _, tc := range []string{"out", "--out", "in-place", "inPlace", "dry-run", "dryRun", "backup", "no-validate", "noValidate", "output", "--output", "json", "keep-temp", "temp-dir", "verbosity", "strict", "help", "h"} {
		raw := []byte(`[{"command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"x","` + tc + `":"owned"}}]`)
		if _, err := ParseOps(raw); err == nil {
			t.Fatalf("ParseOps accepted session-owned arg %q", tc)
		} else if !strings.Contains(err.Error(), "owned by the apply/serve/MCP session") {
			t.Fatalf("ParseOps error for %q = %v", tc, err)
		}
	}
}

// TestParseOpsRejectsCaseVariantArgKeyCollision pins that two normalization-
// equivalent arg keys are rejected at parse time rather than silently mis-bound
// (pflag would keep only the last in sorted order and drop the other).
func TestParseOpsRejectsCaseVariantArgKeyCollision(t *testing.T) {
	for _, pair := range [][2]string{{"Sheet", "sheet"}, {"in_place", "inPlace"}, {"cell", "Cell"}} {
		// in_place/inPlace are also session-owned; this still rejects (either guard
		// fires), so only assert rejection, not the specific message, for that pair.
		raw := []byte(`[{"command":"xlsx cells set","args":{"` + pair[0] + `":"1","` + pair[1] + `":"2","value":"x"}}]`)
		if _, err := ParseOps(raw); err == nil {
			t.Fatalf("ParseOps accepted colliding keys %q/%q", pair[0], pair[1])
		}
	}
	// A clean, non-session collision reports the flag-collision message specifically.
	raw := []byte(`[{"command":"xlsx cells set","args":{"Sheet":"1","sheet":"2","value":"x"}}]`)
	_, err := ParseOps(raw)
	if err == nil || !strings.Contains(err.Error(), "both map to flag") {
		t.Fatalf("ParseOps collision error = %v, want a 'both map to flag' rejection", err)
	}
}

func TestParseOpsAllowsLocalFormatArg(t *testing.T) {
	ops, err := ParseOps([]byte(`[{"command":"pptx place table","args":{"slide":1,"data":"/tmp/data.csv","format":"csv","x":0,"y":0,"cx":1000}}]`))
	if err != nil {
		t.Fatalf("ParseOps should allow command-local format arg: %v", err)
	}
	if got := ops[0].Args["format"].String(); got != "csv" {
		t.Fatalf("format arg = %q, want csv", got)
	}
}

func TestShellCommandQuotesSpacesAndSingleQuotes(t *testing.T) {
	got := ShellCommand("ooxml", "validate", "--strict", "/tmp/O'Brien Files/out.xlsx")
	want := `ooxml validate --strict '/tmp/O'"'"'Brien Files/out.xlsx'`
	if got != want {
		t.Fatalf("ShellCommand = %q, want %q", got, want)
	}
}

func TestExecuteBoolArg(t *testing.T) {
	input := stageFixture(t)
	out := filepath.Join(t.TempDir(), "out.xlsx")
	ops := []Operation{
		{Command: "xlsx names add", Args: map[string]Arg{
			"name":   mustArg(t, "MyName"),
			"ref":    mustArg(t, "Sheet1!$A$1"),
			"hidden": mustJSONArg(t, true),
		}},
	}
	e := &Executor{Self: builtBinary, TempDir: t.TempDir()}
	if _, err := e.Execute(input, ops, out, "", false); err != nil {
		t.Fatalf("Execute with bool arg: %v", err)
	}
	cmd := exec.Command(builtBinary, "--json", "xlsx", "names", "show", out, "--name", "MyName")
	raw, err := cmd.Output()
	if err != nil {
		t.Fatalf("names show: %v", err)
	}
	var body struct {
		Name struct {
			Hidden bool `json:"hidden"`
		} `json:"name"`
	}
	if err := json.Unmarshal(raw, &body); err != nil {
		t.Fatalf("decode names show: %v (%s)", err, raw)
	}
	if !body.Name.Hidden {
		t.Fatalf("hidden bool arg was not applied: %s", raw)
	}
}

func TestExecuteSuccess(t *testing.T) {
	input := stageFixture(t)
	out := filepath.Join(t.TempDir(), "out.xlsx")
	ops := []Operation{
		{Command: "xlsx cells set", Args: map[string]Arg{"sheet": mustArg(t, "1"), "cell": mustArg(t, "A1"), "value": mustArg(t, "first")}},
		{Command: "xlsx cells set", Args: map[string]Arg{"sheet": mustArg(t, "1"), "cell": mustArg(t, "A2"), "value": mustArg(t, "second")}},
	}
	e := &Executor{Self: builtBinary, TempDir: t.TempDir()}
	applied, err := e.Execute(input, ops, out, "", false)
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if len(applied) != 2 {
		t.Fatalf("applied = %d, want 2", len(applied))
	}
	if applied[0].Readback == nil {
		t.Fatal("expected readback JSON for op 0")
	}
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("output not written: %v", err)
	}

	// Verify the second op's mutation actually landed by reading back A2.
	got := readCell(t, out, "1", "A2")
	if got != "second" {
		t.Fatalf("A2 = %q, want %q", got, "second")
	}
}

func TestExecuteRewritesScratchReadbackToPublishedOutput(t *testing.T) {
	input := stageFixture(t)
	out := filepath.Join(t.TempDir(), "published workbook.xlsx")
	ops := []Operation{
		{Command: "ooxml xlsx cells set", Args: map[string]Arg{"sheet": mustArg(t, "1"), "cell": mustArg(t, "A1"), "value": mustArg(t, "published")}},
	}
	e := &Executor{Self: builtBinary, TempDir: filepath.Dir(out)}
	applied, err := e.Execute(input, ops, out, "", false)
	if err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if len(applied) != 1 || applied[0].Command != "xlsx cells set" || applied[0].Readback == nil {
		t.Fatalf("unexpected applied op: %+v", applied)
	}
	readback := string(applied[0].Readback)
	if !strings.Contains(readback, jsonPathFragmentForApplyTest(out)) {
		t.Fatalf("readback should point at committed output %q, got %s", out, readback)
	}
	if strings.Contains(readback, ".ooxml-apply-") {
		t.Fatalf("readback leaked apply scratch path: %s", readback)
	}
	if got := readCell(t, out, "1", "A1"); got != "published" {
		t.Fatalf("A1 = %q, want published", got)
	}
}

func TestExecuteOpFailureStopsChainNoOutput(t *testing.T) {
	input := stageFixture(t)
	out := filepath.Join(t.TempDir(), "out.xlsx")
	ops := []Operation{
		{Command: "xlsx cells set", Args: map[string]Arg{"sheet": mustArg(t, "1"), "cell": mustArg(t, "A1"), "value": mustArg(t, "ok")}},
		// Bogus command word -> subprocess exits non-zero.
		{Command: "xlsx cells bogus-subcommand", Args: map[string]Arg{"sheet": mustArg(t, "1")}},
	}
	e := &Executor{Self: builtBinary, TempDir: t.TempDir()}
	_, err := e.Execute(input, ops, out, "", true)
	if err == nil {
		t.Fatal("expected error")
	}
	opErr, ok := err.(*OpError)
	if !ok {
		t.Fatalf("error type = %T, want *OpError", err)
	}
	if opErr.FailedOpIndex != 1 {
		t.Fatalf("FailedOpIndex = %d, want 1", opErr.FailedOpIndex)
	}
	if _, statErr := os.Stat(out); !os.IsNotExist(statErr) {
		t.Fatalf("output should not exist after failure, stat err = %v", statErr)
	}
}

func TestExecuteZeroByteChildOutputStopsChainNoOutputEvenWithoutValidation(t *testing.T) {
	input := stageFixture(t)
	out := filepath.Join(t.TempDir(), "out.xlsx")
	t.Setenv("OOXML_APPLY_FAKE_CHILD", "zero")

	e := &Executor{Self: os.Args[0], TempDir: t.TempDir()}
	_, err := e.Execute(input, []Operation{{Command: "xlsx cells set"}}, out, "", true)
	if err == nil {
		t.Fatal("expected zero-byte child output to fail")
	}
	opErr, ok := err.(*OpError)
	if !ok {
		t.Fatalf("error type = %T, want *OpError (%v)", err, err)
	}
	if opErr.FailedOpIndex != 0 || !strings.Contains(opErr.Error(), "did not write output package") {
		t.Fatalf("unexpected op error: %+v", opErr)
	}
	if _, statErr := os.Stat(out); !os.IsNotExist(statErr) {
		t.Fatalf("output should not exist after zero-byte child output, stat err = %v", statErr)
	}
}

func TestExecuteInPlaceWithBackup(t *testing.T) {
	input := stageFixture(t)
	backup := input + ".bak"
	ops := []Operation{
		{Command: "xlsx cells set", Args: map[string]Arg{"sheet": mustArg(t, "1"), "cell": mustArg(t, "A1"), "value": mustArg(t, "changed")}},
	}
	e := &Executor{Self: builtBinary, TempDir: t.TempDir()}
	if _, err := e.Execute(input, ops, input, backup, false); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if _, err := os.Stat(backup); err != nil {
		t.Fatalf("backup not created: %v", err)
	}
	if got := readCell(t, input, "1", "A1"); got != "changed" {
		t.Fatalf("A1 = %q, want changed", got)
	}
}

// TestCopyFileCrashSafeDoesNotTruncateDst proves the cross-FS copy fallback never
// truncates or destroys an existing destination on a mid-copy failure. It forces a
// read error partway through by passing a DIRECTORY as the source: os.Open(dir)
// succeeds but the first Read returns an error. The pre-existing dst (and the
// in-place-commit scenario it stands for) must be left byte-for-byte intact.
func TestCopyFileCrashSafeDoesNotTruncateDst(t *testing.T) {
	dir := t.TempDir()

	// A source that errors partway: a directory. os.Open succeeds, io.Copy fails.
	srcDir := filepath.Join(dir, "srcdir")
	if err := os.Mkdir(srcDir, 0o755); err != nil {
		t.Fatal(err)
	}

	dst := filepath.Join(dir, "dst.bin")
	sentinel := []byte("ORIGINAL-CONTENTS-MUST-SURVIVE")
	if err := os.WriteFile(dst, sentinel, 0o644); err != nil {
		t.Fatal(err)
	}

	if err := copyFile(srcDir, dst); err == nil {
		t.Fatalf("copyFile(dir, dst) = nil, want a copy error")
	}

	got, err := os.ReadFile(dst)
	if err != nil {
		t.Fatalf("dst destroyed by failed copy: %v", err)
	}
	if !reflect.DeepEqual(got, sentinel) {
		t.Fatalf("dst was modified by a failed copy: got %q, want %q (must be untouched)", got, sentinel)
	}

	// No sibling temp must be left behind in dst's directory after the failure.
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	for _, e := range entries {
		if strings.HasPrefix(e.Name(), ".ooxml-copy-") {
			t.Fatalf("leftover sibling temp after failed copy: %s", e.Name())
		}
	}
}

// TestExecutePreservesFileMode proves the apply publish does not silently
// downgrade the output's permissions. A zero-op apply stages the input through a
// scratch temp (CreateTemp's 0600) and publishes via rename, so without mode
// preservation a 0644 input would yield a 0600 output.
func TestExecutePreservesFileMode(t *testing.T) {
	input := stageFixture(t)
	if err := os.Chmod(input, 0o644); err != nil {
		t.Fatal(err)
	}
	out := filepath.Join(t.TempDir(), "out.xlsx")

	e := &Executor{Self: builtBinary, TempDir: filepath.Dir(out)}
	if _, err := e.Execute(input, nil, out, "", false); err != nil {
		t.Fatalf("Execute: %v", err)
	}

	info, err := os.Stat(out)
	if err != nil {
		t.Fatalf("stat output: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o644 {
		if runtime.GOOS == "windows" && got&0o600 == 0o600 {
			return
		}
		t.Fatalf("published output mode = %o, want 0644 (apply must not downgrade permissions)", got)
	}
}

func TestExecuteCorruptChildOutputIsOpError(t *testing.T) {
	input := stageFixture(t)
	out := filepath.Join(t.TempDir(), "out.xlsx")
	t.Setenv("OOXML_APPLY_FAKE_CHILD", "corrupt")

	e := &Executor{Self: os.Args[0], TempDir: t.TempDir()}
	_, err := e.Execute(input, []Operation{{Command: "xlsx cells set"}}, out, "", false)
	if err == nil {
		t.Fatal("expected op error for corrupt child output")
	}
	if _, ok := err.(*OpError); !ok {
		t.Fatalf("corrupt child output error = %T, want *OpError (%v)", err, err)
	}
	if _, statErr := os.Stat(out); !os.IsNotExist(statErr) {
		t.Fatalf("output should not exist after corrupt child output, stat err = %v", statErr)
	}
}

// TestExecuteFinalValidationFailureWritesNothing covers the apply contract's
// central data-integrity guarantee whose FAILURE branch had no test: every op
// succeeds, but the resulting package fails the single final validation pass, so
// nothing is published and a *ValidationError is returned. A fake child copies a
// known structurally-openable-but-error-severity fixture (a dangling slide-layout
// relationship) to --out, so ensureSubprocessWrotePackage (opc.Open) passes but
// validateFinal (validate.ValidatePackage) trips on the error-severity diagnostic.
func TestExecuteFinalValidationFailureWritesNothing(t *testing.T) {
	input := stagePPTXFixture(t)
	out := filepath.Join(t.TempDir(), "out.pptx")
	fixture := filepath.Join(repoRoot(t), "testdata", "pptx", "corrupted-dangling-layout", "presentation.pptx")
	tempDir := t.TempDir()
	t.Setenv("OOXML_APPLY_FAKE_CHILD", "copy")
	t.Setenv("OOXML_APPLY_FAKE_SOURCE", fixture)

	e := &Executor{Self: os.Args[0], TempDir: tempDir}
	// noValidate=false: final validation must run and fail.
	_, err := e.Execute(input, []Operation{{Command: "pptx slides reorder"}}, out, "", false)
	if err == nil {
		t.Fatal("expected final validation failure, got nil")
	}
	verr, ok := err.(*ValidationError)
	if !ok {
		t.Fatalf("error = %T, want *ValidationError (%v)", err, err)
	}
	if len(verr.Diagnostics) == 0 {
		t.Fatal("ValidationError carries no diagnostics")
	}
	if _, statErr := os.Stat(out); !os.IsNotExist(statErr) {
		t.Fatalf("output must not be written on final validation failure, stat err = %v", statErr)
	}
	entries, _ := os.ReadDir(tempDir)
	for _, ent := range entries {
		if strings.HasPrefix(ent.Name(), ".ooxml-apply-") {
			t.Errorf("scratch temp leaked after validation failure: %s", ent.Name())
		}
	}
}

// TestMoveFileCrossFilesystemFallback pins the EXDEV branch of moveFile that a
// single-filesystem test never reaches: when rename fails, moveFile must copy
// src to dst byte-for-byte and remove src, and a Remove failure must be swallowed
// (the move still succeeds because dst is already complete). Driven via the
// osRename seam returning a synthetic cross-device error.
func TestMoveFileCrossFilesystemFallback(t *testing.T) {
	orig := osRename
	osRename = func(string, string) error { return &os.LinkError{Op: "rename", Err: syscall.EXDEV} }
	t.Cleanup(func() { osRename = orig })

	dir := t.TempDir()
	src := filepath.Join(dir, "src.bin")
	dst := filepath.Join(dir, "dst.bin")
	content := []byte("rolling-temp payload\x00\x01\x02")
	if err := os.WriteFile(src, content, 0o644); err != nil {
		t.Fatalf("write src: %v", err)
	}
	if err := moveFile(src, dst); err != nil {
		t.Fatalf("moveFile cross-fs fallback: %v", err)
	}
	got, err := os.ReadFile(dst)
	if err != nil {
		t.Fatalf("read dst: %v", err)
	}
	if string(got) != string(content) {
		t.Fatalf("dst content = %q, want %q", got, content)
	}
	if _, statErr := os.Stat(src); !os.IsNotExist(statErr) {
		t.Fatalf("src should be removed after cross-fs move, stat err = %v", statErr)
	}
}

// readCell shells out to the built binary to read one cell's value back.
func readCell(t *testing.T, file, sheet, cell string) string {
	t.Helper()
	cmd := exec.Command(builtBinary, "--json", "xlsx", "ranges", "export", file, "--sheet", sheet, "--range", cell)
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("readCell: %v", err)
	}
	var payload struct {
		Values [][]string `json:"values"`
	}
	if err := json.Unmarshal(out, &payload); err != nil {
		t.Fatalf("readCell unmarshal: %v (%s)", err, out)
	}
	if len(payload.Values) == 0 || len(payload.Values[0]) == 0 {
		return ""
	}
	return payload.Values[0][0]
}

// stagePPTXFixture copies the title-content deck (two slides, sldId 256 then 257,
// each carrying a title shape with native cNvPr id 2) into a temp dir.
func stagePPTXFixture(t *testing.T) string {
	t.Helper()
	src := filepath.Join(repoRoot(t), "testdata", "pptx", "title-content", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read pptx fixture: %v", err)
	}
	dst := filepath.Join(t.TempDir(), "input.pptx")
	if err := os.WriteFile(dst, data, 0o644); err != nil {
		t.Fatalf("stage pptx fixture: %v", err)
	}
	return dst
}

// titleTextOnSlide shells to the built binary and returns the text preview of the
// title shape (native cNvPr id 2) on the given 1-based slide of a deck.
func titleTextOnSlide(t *testing.T, file string, slide int) string {
	t.Helper()
	cmd := exec.Command(builtBinary, "--json", "pptx", "shapes", "show", file, "--slide", itoa(slide), "--include-text")
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("titleTextOnSlide(slide %d): %v", slide, err)
	}
	var payload struct {
		Shapes []struct {
			ShapeID     int    `json:"shapeId"`
			TextPreview string `json:"textPreview"`
		} `json:"shapes"`
	}
	if err := json.Unmarshal(out, &payload); err != nil {
		t.Fatalf("titleTextOnSlide unmarshal: %v (%s)", err, out)
	}
	for _, s := range payload.Shapes {
		if s.ShapeID == 2 {
			return s.TextPreview
		}
	}
	t.Fatalf("no shape id 2 on slide %d", slide)
	return ""
}

func itoa(n int) string {
	b, _ := json.Marshal(n)
	return string(b)
}

// TestApplyBatchSurvivesStructuralShift_PPTX is the PR-HANDLES-1 HEADLINE proof.
//
// A single apply batch runs TWO ops sequentially against the EVOLVING file:
//
//	op1: pptx clone-slide --slide 1 --insert-after 0   (STRUCTURAL edit)
//	op2: pptx replace text --target H:pptx/s:257/shape:n:2 --text SURVIVED
//
// op1 inserts a cloned slide (fresh sldId 258) at position 2, which SHIFTS the
// original second slide (sldId 257, title "Content Slide") from position 2 to
// position 3. op2 targets that slide's title by its durable HANDLE (sldId 257),
// so even though its POSITION moved, the edit lands on the correct shape — proving
// the headline promise: "apply batches do not break when slides shift."
//
// The test then runs the SAME batch with op2 expressed POSITIONALLY (--slide 2)
// and asserts it lands on the WRONG slide (the clone at position 2), with the
// intended target left untouched. This silent-wrong-target contrast is the
// strongest demonstration of why handles are necessary.
func TestApplyBatchSurvivesStructuralShift_PPTX(t *testing.T) {
	e := &Executor{Self: builtBinary, TempDir: t.TempDir()}

	// --- HANDLE batch: op2 targets sldId 257 by handle. ---
	{
		input := stagePPTXFixture(t)
		out := filepath.Join(t.TempDir(), "handle-out.pptx")
		ops := []Operation{
			{Command: "pptx clone-slide", Args: map[string]Arg{
				"slide":        mustArg(t, "1"),
				"insert-after": mustArg(t, "0"),
			}},
			{Command: "pptx replace text", Args: map[string]Arg{
				"target": mustArg(t, "H:pptx/s:257/shape:n:2"),
				"text":   mustArg(t, "SURVIVED"),
			}},
		}
		applied, err := e.Execute(input, ops, out, "", false)
		if err != nil {
			t.Fatalf("handle batch Execute: %v", err)
		}
		if len(applied) != 2 {
			t.Fatalf("handle batch applied = %d, want 2", len(applied))
		}
		// op1 shifted sldId 257 to position 3; the handle still landed there.
		if got := titleTextOnSlide(t, out, 3); got != "SURVIVED" {
			t.Fatalf("HANDLE op2 should land on slide 3 (sldId 257), got title %q", got)
		}
		// The clone now at position 2 is the ORIGINAL slide-1 title, untouched.
		if got := titleTextOnSlide(t, out, 2); got == "SURVIVED" {
			t.Fatalf("HANDLE op2 wrongly hit the cloned slide 2: %q", got)
		}
	}

	// --- POSITIONAL batch: the SAME structural op1, but op2 uses --slide 2. ---
	// It lands on the CLONE (position 2), not the intended sldId 257 (now at
	// position 3). The intended target keeps its original text — a silent wrong
	// target, which is exactly the failure handles prevent.
	{
		input := stagePPTXFixture(t)
		out := filepath.Join(t.TempDir(), "positional-out.pptx")
		ops := []Operation{
			{Command: "pptx clone-slide", Args: map[string]Arg{
				"slide":        mustArg(t, "1"),
				"insert-after": mustArg(t, "0"),
			}},
			{Command: "pptx replace text", Args: map[string]Arg{
				"slide":  mustArg(t, "2"),
				"target": mustArg(t, "shape:2"),
				"text":   mustArg(t, "WRONGTARGET"),
			}},
		}
		if _, err := e.Execute(input, ops, out, "", false); err != nil {
			t.Fatalf("positional batch Execute: %v", err)
		}
		// The intended target (sldId 257, now slide 3) is UNTOUCHED by the
		// positional op, proving the positional batch missed it.
		if got := titleTextOnSlide(t, out, 3); got != "Content Slide" {
			t.Fatalf("positional op2 should NOT have touched slide 3 (sldId 257); got %q", got)
		}
		// The positional --slide 2 hit the clone instead — the wrong object.
		if got := titleTextOnSlide(t, out, 2); got != "WRONGTARGET" {
			t.Fatalf("positional op2 should have hit the clone at slide 2; got %q", got)
		}
	}
}
