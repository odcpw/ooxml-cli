package serve

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
)

func TestMain(m *testing.M) {
	if mode := os.Getenv("OOXML_SERVE_FAKE_CHILD"); mode != "" {
		os.Exit(runFakeServeChild(mode))
	}
	os.Exit(m.Run())
}

func runFakeServeChild(mode string) int {
	if mode != "dry-run" {
		fmt.Fprintf(os.Stderr, "unknown fake child mode %q\n", mode)
		return 2
	}
	in, out := fakeServeInputOutput(os.Args[1:])
	if in == "" || out == "" {
		fmt.Fprintln(os.Stderr, "missing input or --out")
		return 2
	}
	data, err := os.ReadFile(in)
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	if err := os.WriteFile(out, data, 0o644); err != nil {
		fmt.Fprintln(os.Stderr, err)
		return 1
	}
	fmt.Printf("{\"file\":%q,\"output\":%q,\"dryRun\":false,\"validateCommand\":%q}\n", in, out, "ooxml validate --strict "+out)
	return 0
}

func fakeServeInputOutput(args []string) (string, string) {
	var in, out string
	for i, arg := range args {
		if arg == "--out" && i+1 < len(args) {
			out = args[i+1]
			continue
		}
		if in == "" && looksLikePackagePath(arg) {
			in = arg
		}
	}
	return in, out
}

func looksLikePackagePath(path string) bool {
	lower := strings.ToLower(path)
	for _, ext := range []string{".pptx", ".pptm", ".xlsx", ".xlsm", ".docx", ".docm"} {
		if strings.HasSuffix(lower, ext) {
			return true
		}
	}
	return false
}

// repoRoot walks up from the test working directory (pkg/serve) to the module root.
func repoRoot(t *testing.T) string {
	t.Helper()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	// pkg/serve -> repo root is two levels up.
	root := filepath.Join(wd, "..", "..")
	if _, err := os.Stat(filepath.Join(root, "go.mod")); err != nil {
		t.Fatalf("could not locate repo root from %s: %v", wd, err)
	}
	return root
}

// stageXLSX copies the minimal workbook fixture into dir and returns its path.
func stageXLSX(t *testing.T, dir string) string {
	t.Helper()
	src := filepath.Join(repoRoot(t), "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	dst := filepath.Join(dir, "input.xlsx")
	if err := os.WriteFile(dst, data, 0o644); err != nil {
		t.Fatalf("stage fixture: %v", err)
	}
	return dst
}

func mustReadArg(t *testing.T, raw string) apply.Arg {
	t.Helper()
	var arg apply.Arg
	if err := json.Unmarshal([]byte(raw), &arg); err != nil {
		t.Fatalf("unmarshal arg %s: %v", raw, err)
	}
	return arg
}

func TestBuildReadArgvSerializesBoolFlags(t *testing.T) {
	got := buildReadArgv("xlsx ranges export", "in.xlsx", map[string]apply.Arg{
		"includeTypes": mustReadArg(t, "true"),
		"range":        mustReadArg(t, `"A1"`),
		"sheet":        mustReadArg(t, `"1"`),
	})
	want := []string{
		"--json", "xlsx", "ranges", "export", "in.xlsx",
		"--include-types=true",
		"--range", "A1",
		"--sheet", "1",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("buildReadArgv = %v, want %v", got, want)
	}
}

func TestBuildReadArgvNormalizesJSONFriendlyArgNames(t *testing.T) {
	got := buildReadArgv("xlsx ranges export", "in.xlsx", map[string]apply.Arg{
		"includeTypes": mustReadArg(t, "true"),
		"maxRows":      mustReadArg(t, "5"),
		"range":        mustReadArg(t, `"A1:B5"`),
		"sheet":        mustReadArg(t, `"1"`),
	})
	want := []string{
		"--json", "xlsx", "ranges", "export", "in.xlsx",
		"--include-types=true",
		"--max-rows", "5",
		"--range", "A1:B5",
		"--sheet", "1",
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("buildReadArgv = %v, want %v", got, want)
	}
}

func TestValidateReadCommandRejectsMutationsAndArtifacts(t *testing.T) {
	if err := validateReadCommand("xlsx ranges export", map[string]apply.Arg{
		"includeTypes": mustReadArg(t, "true"),
		"range":        mustReadArg(t, `"A1"`),
		"sheet":        mustReadArg(t, `"1"`),
	}); err != nil {
		t.Fatalf("read command rejected: %v", err)
	}

	if err := validateReadCommand("xlsx cells set", nil); err == nil {
		t.Fatal("expected mutation command to be rejected")
	} else if _, ok := err.(*ReadCommandDeniedError); !ok {
		t.Fatalf("mutation command error = %T, want *ReadCommandDeniedError", err)
	}

	if err := validateReadCommand("xlsx ranges export", map[string]apply.Arg{
		"dataOut": mustReadArg(t, `"/tmp/export.csv"`),
		"range":   mustReadArg(t, `"A1"`),
		"sheet":   mustReadArg(t, `"1"`),
	}); err == nil {
		t.Fatal("expected artifact-writing read args to be rejected")
	} else if _, ok := err.(*ReadCommandDeniedError); !ok {
		t.Fatalf("artifact flag error = %T, want *ReadCommandDeniedError", err)
	}

	if err := validateReadCommand("xlsx ranges export", map[string]apply.Arg{
		"data-out=/tmp/export.csv": mustReadArg(t, `true`),
		"range":                    mustReadArg(t, `"A1"`),
		"sheet":                    mustReadArg(t, `"1"`),
	}); err == nil {
		t.Fatal("expected equals-style artifact arg key to be rejected")
	} else if _, ok := err.(*ReadCommandDeniedError); !ok {
		t.Fatalf("equals artifact flag error = %T, want *ReadCommandDeniedError", err)
	} else if !strings.Contains(err.Error(), "without '='") {
		t.Fatalf("equals artifact flag rejection should explain JSON value shape: %v", err)
	}

	if err := validateReadCommand("xlsx ranges export --data-out /tmp/export.csv", map[string]apply.Arg{
		"range": mustReadArg(t, `"A1"`),
		"sheet": mustReadArg(t, `"1"`),
	}); err == nil {
		t.Fatal("expected command-embedded artifact flag to be rejected")
	} else if _, ok := err.(*ReadCommandDeniedError); !ok {
		t.Fatalf("command-embedded flag error = %T, want *ReadCommandDeniedError", err)
	} else if !strings.Contains(err.Error(), `put flag "--data-out" in args`) {
		t.Fatalf("command-embedded flag rejection should explain args usage: %v", err)
	}

	if err := validateReadCommand("pptx extract images", nil); err == nil {
		t.Fatal("expected default artifact-producing command to be rejected")
	} else if _, ok := err.(*ReadCommandDeniedError); !ok {
		t.Fatalf("artifact command error = %T, want *ReadCommandDeniedError", err)
	} else if !strings.Contains(err.Error(), "writes image files") {
		t.Fatalf("artifact command error should explain why inspect rejected it: %v", err)
	}

	for _, tc := range []struct {
		command string
		want    string
	}{
		{"pptx render", "writes PDF/image artifacts"},
		{"pptx extract xml", "writes raw XML files"},
		{"diff /tmp/base.xlsx", "needs both baseline and candidate"},
		{"render /tmp/out", "writes visual artifacts"},
		{"verify /tmp/base.xlsx", "may diff/render against an external baseline"},
		{"pptx diff", "needs both baseline and candidate"},
		{"vba extract", "writes .bas/.cls"},
		{"vba extract-bin", "writes vbaProject.bin"},
		{"vba inspect-bin", "standalone vbaProject.bin"},
		{"capabilities", "session-independent discovery"},
	} {
		t.Run(tc.command, func(t *testing.T) {
			err := validateReadCommand(tc.command, nil)
			if err == nil {
				t.Fatalf("expected %q to be rejected from session inspect", tc.command)
			}
			if _, ok := err.(*ReadCommandDeniedError); !ok {
				t.Fatalf("%q error = %T, want *ReadCommandDeniedError", tc.command, err)
			}
			if !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("%q rejection should mention %q, got %v", tc.command, tc.want, err)
			}
		})
	}
}

// TestOpenScratchOnTargetFilesystem proves that, with no --temp-dir override, a
// non-dry-run session creates its working/scratch dir on the TARGET's filesystem
// (the same directory as the commit target). That makes the commit MoveFile a
// true intra-filesystem os.Rename instead of degrading to copy+remove — the bug
// being fixed. Open does only opc.Open + CopyFile, so Self can be empty.
func TestOpenScratchOnTargetFilesystem(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)

	// Distinct directory for the commit target, standing in for the user's
	// target filesystem (here just a separate dir under the same /tmp; the
	// assertion is about which directory the scratch lands in).
	outDir := t.TempDir()
	out := filepath.Join(outDir, "out.xlsx")

	// Empty TempBase => no --temp-dir override; the fix must base scratch on the
	// target directory.
	e := NewEngine("", "")
	res, err := e.Open(OpenParams{Path: input, Out: out})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	s, err := e.get(res.SessionID)
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	defer e.Abort(res.SessionID)

	gotDir := filepath.Dir(s.tempDir)
	wantDir := filepath.Dir(out)
	if gotDir != wantDir {
		t.Fatalf("scratch dir parent = %q, want target dir %q (commit would be cross-FS copy+remove, not atomic rename)", gotDir, wantDir)
	}
}

// TestCommitPreservesFileMode proves a commit does not silently downgrade the
// published file's permissions. The working copy is staged through a scratch temp
// (CreateTemp's 0600) and published via rename, so without mode preservation the
// 0644 input would become a 0600 output. A zero-op commit needs no subprocess, so
// Self can be empty.
func TestCommitPreservesFileMode(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)
	if err := os.Chmod(input, 0o644); err != nil {
		t.Fatal(err)
	}

	out := filepath.Join(t.TempDir(), "out.xlsx")
	e := NewEngine("", "")
	res, err := e.Open(OpenParams{Path: input, Out: out})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if _, err := e.Commit(res.SessionID); err != nil {
		t.Fatalf("Commit: %v", err)
	}

	info, err := os.Stat(out)
	if err != nil {
		t.Fatalf("stat output: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o644 {
		if runtime.GOOS == "windows" && got&0o600 == 0o600 {
			return
		}
		t.Fatalf("committed output mode = %o, want 0644 (commit must not downgrade permissions)", got)
	}
}

// TestCommitValidationFailureLeavesSessionOpen pins the commit contract whose
// failure branch was untested: a commit whose working copy fails validation
// returns a *apply.ValidationError, writes nothing to the output target, and
// leaves the session OPEN and re-committable (engine.go:402 — it does not mark
// committed or drop the session). The source is an openable-but-error-severity
// fixture (dangling slide-layout relationship): opc.Open succeeds so the session
// opens, but commit-time validation trips on the error diagnostic.
func TestCommitValidationFailureLeavesSessionOpen(t *testing.T) {
	src := filepath.Join(repoRoot(t), "testdata", "pptx", "corrupted-dangling-layout", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	input := filepath.Join(t.TempDir(), "deck.pptx")
	if err := os.WriteFile(input, data, 0o644); err != nil {
		t.Fatal(err)
	}
	out := filepath.Join(t.TempDir(), "out.pptx")

	e := NewEngine("", "")
	res, err := e.Open(OpenParams{Path: input, Out: out})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	_, err = e.Commit(res.SessionID)
	if err == nil {
		t.Fatal("expected validation failure on commit, got nil")
	}
	if _, ok := err.(*apply.ValidationError); !ok {
		t.Fatalf("commit error = %T, want *apply.ValidationError (%v)", err, err)
	}
	if _, statErr := os.Stat(out); !os.IsNotExist(statErr) {
		t.Fatalf("commit must not write output on validation failure, stat err = %v", statErr)
	}
	// The session must remain open and re-committable after a failed commit.
	if _, err := e.get(res.SessionID); err != nil {
		t.Fatalf("session must remain open after a failed commit, got %v", err)
	}
}

// TestCloseReapsUncommittedSessions proves Close() reaps sessions opened but
// never committed/aborted, the normal stdio-EOF shutdown path. Before this fix
// nothing reaped a live session's per-session scratch dir, so each open-without-
// commit leaked a full working copy. The test would fail if Close were a no-op:
// it asserts the scratch dir is removed AND the session is dropped.
func TestCloseReapsUncommittedSessions(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)

	outDir := t.TempDir()
	out := filepath.Join(outDir, "out.xlsx")

	e := NewEngine("", "")
	res, err := e.Open(OpenParams{Path: input, Out: out})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	s, err := e.get(res.SessionID)
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	tempDir := s.tempDir
	if _, err := os.Stat(tempDir); err != nil {
		t.Fatalf("scratch dir should exist after Open: %v", err)
	}

	if err := e.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	if _, err := os.Stat(tempDir); !os.IsNotExist(err) {
		t.Fatalf("scratch dir %q not removed by Close (os.Stat err = %v) — uncommitted working copy leaked", tempDir, err)
	}
	if _, err := e.get(res.SessionID); err == nil {
		t.Fatalf("session %q still present after Close; want it dropped", res.SessionID)
	} else if _, ok := err.(*SessionNotFoundError); !ok {
		t.Fatalf("get after Close returned %T, want *SessionNotFoundError", err)
	}

	// Close is idempotent: a second call with no live sessions is a clean no-op.
	if err := e.Close(); err != nil {
		t.Fatalf("second Close: %v", err)
	}
}

// TestCloseAfterCommitIsNoOp confirms Close after a session was already committed
// (and thus dropped) reaps nothing and errors not — the committed file survives.
func TestCloseAfterCommitIsNoOp(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)

	out := filepath.Join(t.TempDir(), "out.xlsx")
	e := NewEngine("", "")
	res, err := e.Open(OpenParams{Path: input, Out: out})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	if _, err := e.Commit(res.SessionID); err != nil {
		t.Fatalf("Commit: %v", err)
	}

	if err := e.Close(); err != nil {
		t.Fatalf("Close after commit: %v", err)
	}
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("committed output missing after Close: %v", err)
	}
}

// TestOpenScratchHonorsTempDirOverride confirms the --temp-dir override (a
// non-empty TempBase) still wins for non-dry-run sessions: the scratch dir is
// created under the override, NOT the target directory.
func TestOpenScratchHonorsTempDirOverride(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)

	outDir := t.TempDir()
	out := filepath.Join(outDir, "out.xlsx")

	override := t.TempDir()
	e := NewEngine("", override)
	res, err := e.Open(OpenParams{Path: input, Out: out})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer e.Abort(res.SessionID)

	s, err := e.get(res.SessionID)
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if gotDir := filepath.Dir(s.tempDir); gotDir != override {
		t.Fatalf("scratch dir parent = %q, want override %q", gotDir, override)
	}
}

// TestOpenScratchDryRunUsesTempBase confirms a dry-run session (which never
// writes) falls back to TempBase/OS-default temp and does NOT touch the (absent)
// target directory.
func TestOpenScratchDryRunUsesTempBase(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)

	override := t.TempDir()
	e := NewEngine("", override)
	res, err := e.Open(OpenParams{Path: input, DryRun: true})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer e.Abort(res.SessionID)

	s, err := e.get(res.SessionID)
	if err != nil {
		t.Fatalf("get session: %v", err)
	}
	if gotDir := filepath.Dir(s.tempDir); gotDir != override {
		t.Fatalf("dry-run scratch dir parent = %q, want TempBase %q", gotDir, override)
	}
}

func TestDryRunOpReadbackDoesNotClaimRealWrite(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)
	self := fakeOOXMLForServeDryRunTest(t)

	e := NewEngine(self, t.TempDir())
	res, err := e.Open(OpenParams{Path: input, DryRun: true})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer e.Abort(res.SessionID)

	ao, err := e.Op(res.SessionID, apply.Operation{
		Command: "xlsx cells set",
		Args: map[string]apply.Arg{
			"sheet": mustReadArg(t, `"1"`),
			"cell":  mustReadArg(t, `"A1"`),
			"value": mustReadArg(t, `"dry"`),
		},
	})
	if err != nil {
		t.Fatalf("Op: %v", err)
	}
	if ao.Readback == nil {
		t.Fatal("missing op readback")
	}
	text := string(ao.Readback)
	if strings.Contains(text, "ooxml-serve-") || strings.Contains(text, "working-") {
		t.Fatalf("dry-run op readback leaked scratch path: %s", text)
	}
	var payload struct {
		Output          string `json:"output"`
		DryRun          bool   `json:"dryRun"`
		ValidateCommand string `json:"validateCommand"`
	}
	if err := json.Unmarshal(ao.Readback, &payload); err != nil {
		t.Fatalf("readback is not JSON: %v\n%s", err, text)
	}
	if !payload.DryRun {
		t.Fatalf("dry-run op readback claimed dryRun=false: %s", text)
	}
	if payload.Output != "<dry-run-output>" || !strings.Contains(payload.ValidateCommand, "<dry-run-output>") {
		t.Fatalf("dry-run paths were not rewritten: %+v readback=%s", payload, text)
	}
}

func TestOpRejectsAddressPositionalHandleAfterStructuralShift(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)
	self := fakeOOXMLForServeDryRunTest(t)

	e := NewEngine(self, t.TempDir())
	res, err := e.Open(OpenParams{Path: input, DryRun: true})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer e.Abort(res.SessionID)

	if _, err := e.Op(res.SessionID, apply.Operation{
		Command: "xlsx rows delete",
		Args: map[string]apply.Arg{
			"sheet": mustReadArg(t, `"1"`),
			"at":    mustReadArg(t, `1`),
		},
	}); err != nil {
		t.Fatalf("structural shift op should succeed: %v", err)
	}

	_, err = e.Op(res.SessionID, apply.Operation{
		Command: "xlsx cells set",
		Args: map[string]apply.Arg{
			"cell":  mustReadArg(t, `"H:xlsx/ws:1/cell:a:B7"`),
			"value": mustReadArg(t, `"wrong-target"`),
		},
	})
	if err == nil {
		t.Fatal("expected address-positional handle to be rejected after structural shift")
	}
	if !strings.Contains(err.Error(), "address-positional XLSX handle") {
		t.Fatalf("error should explain stale address-positional handle hazard: %v", err)
	}
	if !strings.Contains(err.Error(), "re-run inspect/find") {
		t.Fatalf("error should tell agent how to recover: %v", err)
	}
	if !strings.Contains(err.Error(), "op 0 (xlsx rows delete)") {
		t.Fatalf("error should identify the earlier structural op: %v", err)
	}
	if _, ok := err.(*AddressPositionalHandleAfterShiftError); !ok {
		t.Fatalf("error = %T, want *AddressPositionalHandleAfterShiftError", err)
	}
}

func TestOpAllowsPositionalCellAfterStructuralShift(t *testing.T) {
	inputDir := t.TempDir()
	input := stageXLSX(t, inputDir)
	self := fakeOOXMLForServeDryRunTest(t)

	e := NewEngine(self, t.TempDir())
	res, err := e.Open(OpenParams{Path: input, DryRun: true})
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer e.Abort(res.SessionID)

	if _, err := e.Op(res.SessionID, apply.Operation{
		Command: "xlsx rows delete",
		Args: map[string]apply.Arg{
			"sheet": mustReadArg(t, `"1"`),
			"at":    mustReadArg(t, `1`),
		},
	}); err != nil {
		t.Fatalf("structural shift op should succeed: %v", err)
	}
	if _, err := e.Op(res.SessionID, apply.Operation{
		Command: "xlsx cells set",
		Args: map[string]apply.Arg{
			"sheet": mustReadArg(t, `"1"`),
			"cell":  mustReadArg(t, `"B7"`),
			"value": mustReadArg(t, `"explicit-target"`),
		},
	}); err != nil {
		t.Fatalf("explicit sheet/cell target should remain allowed after structural shift: %v", err)
	}
}

func TestMarkDryRunReadbackAddsMissingTopLevelField(t *testing.T) {
	value, changed := markDryRunReadback(map[string]any{
		"output": "<dry-run-output>",
	})
	if !changed {
		t.Fatal("expected missing dryRun field to be added")
	}
	payload, ok := value.(map[string]any)
	if !ok {
		t.Fatalf("markDryRunReadback returned %T", value)
	}
	if payload["dryRun"] != true {
		t.Fatalf("dryRun = %#v, want true", payload["dryRun"])
	}
}

func TestMarkDryRunReadbackLeavesTrueFieldStable(t *testing.T) {
	value, changed := markDryRunReadback(map[string]any{
		"dryRun": true,
	})
	if changed {
		t.Fatal("dryRun:true should already satisfy the contract")
	}
	payload, ok := value.(map[string]any)
	if !ok || payload["dryRun"] != true {
		t.Fatalf("unexpected payload: %#v", value)
	}
}

func fakeOOXMLForServeDryRunTest(t *testing.T) string {
	t.Helper()
	t.Setenv("OOXML_SERVE_FAKE_CHILD", "dry-run")
	return os.Args[0]
}
