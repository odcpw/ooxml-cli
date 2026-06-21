package cli

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/ooxml-cli/ooxml-cli/pkg/serve"
)

// serveBinary is a freshly built ./cmd/ooxml used as the engine's Self so the
// subprocess op-dispatch path runs the real commands (the in-process test binary
// cannot dispatch ooxml subcommands). Built once in TestMain.
var serveBinary string

func TestMain(m *testing.M) {
	dir, err := os.MkdirTemp("", "ooxml-serve-bin-*")
	if err != nil {
		panic(err)
	}
	defer os.RemoveAll(dir)

	binaryName := "ooxml"
	if runtime.GOOS == "windows" {
		binaryName += ".exe"
	}
	serveBinary = filepath.Join(dir, binaryName)
	build := exec.Command("go", "build", "-o", serveBinary, "github.com/ooxml-cli/ooxml-cli/cmd/ooxml")
	build.Stdout = os.Stderr
	build.Stderr = os.Stderr
	if err := build.Run(); err != nil {
		panic("failed to build ooxml binary for serve tests: " + err.Error())
	}

	os.Exit(m.Run())
}

func serveRepoRoot(t *testing.T) string {
	t.Helper()
	wd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	// internal/cli -> repo root is two levels up.
	root := filepath.Join(wd, "..", "..")
	if _, err := os.Stat(filepath.Join(root, "go.mod")); err != nil {
		t.Fatalf("could not locate repo root from %s: %v", wd, err)
	}
	return root
}

func stageServeXLSX(t *testing.T) string {
	t.Helper()
	src := filepath.Join(serveRepoRoot(t), "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
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

// rpcConn drives the JSON-RPC loop over in-memory buffers, one request at a time,
// returning the decoded response for each call.
type rpcConn struct {
	t    *testing.T
	loop *serveLoop
	id   int
}

func newRPCConn(t *testing.T) *rpcConn {
	t.Helper()
	engine := serve.NewEngine(serveBinary, t.TempDir())
	return &rpcConn{t: t, loop: &serveLoop{engine: engine}}
}

// call sends one request line through the loop and returns the parsed response.
func (c *rpcConn) call(method string, params map[string]interface{}) rpcResponse {
	c.t.Helper()
	c.id++
	req := map[string]interface{}{
		"jsonrpc": "2.0",
		"id":      c.id,
		"method":  method,
	}
	if params != nil {
		req["params"] = params
	}
	line, err := json.Marshal(req)
	if err != nil {
		c.t.Fatalf("marshal request: %v", err)
	}

	var in bytes.Buffer
	in.Write(line)
	in.WriteByte('\n')
	var out bytes.Buffer
	if rerr := c.loop.run(&in, &out); rerr != nil {
		c.t.Fatalf("loop.run: %v", rerr)
	}

	var resp rpcResponse
	dec := json.NewDecoder(bytes.NewReader(out.Bytes()))
	if err := dec.Decode(&resp); err != nil {
		c.t.Fatalf("decode response (raw=%q): %v", out.String(), err)
	}
	return resp
}

func (c *rpcConn) mustResult(method string, params map[string]interface{}) json.RawMessage {
	c.t.Helper()
	resp := c.call(method, params)
	if resp.Error != nil {
		c.t.Fatalf("%s returned error: code=%d msg=%s data=%+v", method, resp.Error.Code, resp.Error.Message, resp.Error.Data)
	}
	raw, err := json.Marshal(resp.Result)
	if err != nil {
		c.t.Fatalf("re-marshal result: %v", err)
	}
	return raw
}

func openServeSession(c *rpcConn, file, out string) string {
	c.t.Helper()
	raw := c.mustResult("open", map[string]interface{}{"file": file, "out": out})
	var r struct {
		SessionID string `json:"sessionId"`
		Type      string `json:"type"`
	}
	if err := json.Unmarshal(raw, &r); err != nil {
		c.t.Fatalf("decode open result: %v", err)
	}
	if r.SessionID == "" {
		c.t.Fatalf("open returned empty session id")
	}
	return r.SessionID
}

func openServeDryRunSession(c *rpcConn, file string) string {
	c.t.Helper()
	raw := c.mustResult("open", map[string]interface{}{"file": file, "dryRun": true})
	var r struct {
		SessionID string `json:"sessionId"`
		Type      string `json:"type"`
	}
	if err := json.Unmarshal(raw, &r); err != nil {
		c.t.Fatalf("decode open result: %v", err)
	}
	if r.SessionID == "" {
		c.t.Fatalf("open returned empty session id")
	}
	return r.SessionID
}

func assertServeInvalidParams(t *testing.T, resp rpcResponse) {
	t.Helper()
	if resp.Error == nil {
		t.Fatalf("expected invalid params error")
	}
	if resp.Error.Code != rpcInvalidParams {
		t.Fatalf("rpc error code = %d, want %d (%+v)", resp.Error.Code, rpcInvalidParams, resp.Error)
	}
	if resp.Error.Data == nil || resp.Error.Data.ExitCode != ExitInvalidArgs {
		t.Fatalf("unexpected invalid params error body: %+v", resp.Error)
	}
}

// readCellViaBinary reads one cell back through the built binary (out of band).
// It extracts the top-left value of a single-cell range export.
func readCellViaBinary(t *testing.T, file, sheet, cell string) string {
	t.Helper()
	cmd := exec.Command(serveBinary, "--json", "xlsx", "ranges", "export", file, "--sheet", sheet, "--range", cell)
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("readCell: %v", err)
	}
	var result struct {
		Values [][]interface{} `json:"values"`
	}
	if err := json.Unmarshal(out, &result); err != nil {
		t.Fatalf("readCell decode (raw=%q): %v", string(out), err)
	}
	if len(result.Values) == 0 || len(result.Values[0]) == 0 || result.Values[0][0] == nil {
		return ""
	}
	if s, ok := result.Values[0][0].(string); ok {
		return s
	}
	return ""
}

// TestServeInspectRejectsMutationNoLeak proves inspect is truly read-only:
// a mutation command with --out must be rejected before any child process can
// write an artifact outside the held session.
func TestServeInspectRejectsMutationNoLeak(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	leakPath := filepath.Join(t.TempDir(), "leak.xlsx")
	c := newRPCConn(t)
	session := openServeDryRunSession(c, input)

	resp := c.call("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args": map[string]interface{}{
			"sheet": "1",
			"cell":  "A1",
			"value": "leak",
			"out":   leakPath,
		},
	})
	if resp.Error == nil {
		t.Fatalf("expected inspect mutation to be rejected")
	}
	if resp.Error.Data == nil || resp.Error.Data.Code != "invalid_args" {
		t.Fatalf("inspect rejection should be invalid_args, got %+v", resp.Error)
	}
	if _, err := os.Stat(leakPath); !os.IsNotExist(err) {
		t.Fatalf("inspect created leaked output %q (stat err=%v)", leakPath, err)
	}

	raw := c.mustResult("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1", "include-types": true},
	})
	if !strings.Contains(string(raw), "values") {
		t.Fatalf("good inspect after rejection returned unexpected JSON: %s", raw)
	}
	c.mustResult("abort", map[string]interface{}{"session": session})
}

func TestServeInspectRejectsDefaultArtifactCommand(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	c := newRPCConn(t)
	session := openServeDryRunSession(c, input)

	resp := c.call("inspect", map[string]interface{}{
		"session": session,
		"command": "pptx extract images",
	})
	if resp.Error == nil {
		t.Fatalf("expected inspect artifact command to be rejected")
	}
	if resp.Error.Data == nil || resp.Error.Data.Code != "invalid_args" {
		t.Fatalf("inspect artifact rejection should be invalid_args, got %+v", resp.Error)
	}
	if !strings.Contains(resp.Error.Data.Message, "writes image files") {
		t.Fatalf("inspect artifact rejection should explain artifact write: %+v", resp.Error.Data)
	}
	c.mustResult("abort", map[string]interface{}{"session": session})
}

func TestServeRejectsUnknownParams(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	c := newRPCConn(t)

	backupPath := filepath.Join(t.TempDir(), "typo-backup.xlsx")
	resp := c.call("open", map[string]interface{}{"file": input, "inPlace": true, "bakcup": backupPath})
	assertServeInvalidParams(t, resp)
	if _, err := os.Stat(backupPath); !os.IsNotExist(err) {
		t.Fatalf("open with unknown backup typo should not create backup, stat err=%v", err)
	}

	session := openServeDryRunSession(c, input)
	for _, tt := range []struct {
		method string
		params map[string]interface{}
	}{
		{"op", map[string]interface{}{"session": session, "command": "xlsx cells set", "args": map[string]interface{}{"sheet": "1", "cell": "A1", "value": "x"}, "bakcup": backupPath}},
		{"inspect", map[string]interface{}{"session": session, "command": "xlsx ranges export", "args": map[string]interface{}{"sheet": "1", "range": "A1"}, "bakcup": backupPath}},
		{"validate", map[string]interface{}{"session": session, "bakcup": backupPath}},
		{"plan", map[string]interface{}{"session": session, "bakcup": backupPath}},
		{"commit", map[string]interface{}{"session": session, "bakcup": backupPath}},
		{"abort", map[string]interface{}{"session": session, "bakcup": backupPath}},
	} {
		resp := c.call(tt.method, tt.params)
		assertServeInvalidParams(t, resp)
	}
	c.mustResult("abort", map[string]interface{}{"session": session})
}

func TestServeRejectsSessionOwnedNestedMutationArgs(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	c := newRPCConn(t)
	session := openServeDryRunSession(c, input)

	resp := c.call("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args": map[string]interface{}{
			"sheet":   "1",
			"cell":    "A1",
			"value":   "x",
			"dry-run": true,
		},
	})
	if resp.Error == nil || resp.Error.Data == nil {
		t.Fatalf("expected structured invalid_args error, got %+v", resp)
	}
	if resp.Error.Data.Code != codeForExit(ExitInvalidArgs) || resp.Error.Data.ExitCode != ExitInvalidArgs {
		t.Fatalf("unexpected nested mutation arg error: %+v", resp.Error.Data)
	}
	if !strings.Contains(resp.Error.Message, "owned by the apply/serve/MCP session") {
		t.Fatalf("unexpected nested mutation arg message: %+v", resp.Error.Data)
	}
	c.mustResult("abort", map[string]interface{}{"session": session})
}

func TestServeOpRejectsNonOperationCommandsBeforeDispatch(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	c := newRPCConn(t)
	session := openServeDryRunSession(c, input)

	for _, tt := range []struct {
		command string
		want    string
	}{
		{"xlsx sheets list", "mutation output flags"},
		{"pptx slides move", "op can supply only the package file"},
	} {
		resp := c.call("op", map[string]interface{}{
			"session": session,
			"command": tt.command,
			"args":    map[string]interface{}{},
		})
		if resp.Error == nil || resp.Error.Data == nil {
			t.Fatalf("%s: expected structured invalid_args error, got %+v", tt.command, resp)
		}
		if resp.Error.Data.Code != codeForExit(ExitInvalidArgs) || !strings.Contains(resp.Error.Message, tt.want) {
			t.Fatalf("%s: unexpected op compatibility error: %+v", tt.command, resp.Error)
		}
	}

	raw := c.mustResult("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1"},
	})
	if !strings.Contains(string(raw), "values") {
		t.Fatalf("session should remain usable after rejected op, inspect=%s", raw)
	}
	c.mustResult("abort", map[string]interface{}{"session": session})
}

// TestServeOpenOpInspectValidateCommit is the end-to-end happy path: open -> op
// (a real mutation) -> inspect reflects the change -> validate clean -> commit
// writes a valid file -> a SECOND op sees the first op's result (interleaved
// state) before commit.
func TestServeOpenOpInspectValidateCommit(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newRPCConn(t)

	session := openServeSession(c, input, outPath)

	// First op: set A1.
	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "first"},
	})

	// inspect reflects the first op via a read command against the working copy.
	rawInspect := c.mustResult("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1"},
	})
	if !strings.Contains(string(rawInspect), "first") {
		t.Fatalf("inspect did not reflect first op: %s", rawInspect)
	}

	// Second op: set A2 — must see interleaved state (build on the first op).
	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A2", "value": "second"},
	})

	// A read across A1:A2 should now show BOTH ops applied (interleaved state).
	rawBoth := c.mustResult("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1:A2"},
	})
	if !strings.Contains(string(rawBoth), "first") || !strings.Contains(string(rawBoth), "second") {
		t.Fatalf("inspect did not reflect both ops: %s", rawBoth)
	}

	// validate clean.
	rawVal := c.mustResult("validate", map[string]interface{}{"session": session})
	if !bytes.Contains(rawVal, []byte(`"diagnostics":[]`)) {
		t.Fatalf("validate diagnostics should be an empty array: %s", rawVal)
	}
	if bytes.Contains(rawVal, []byte(`"diagnostics":null`)) {
		t.Fatalf("validate diagnostics should not be null: %s", rawVal)
	}
	var val struct {
		Diagnostics []DiagnosticJSON `json:"diagnostics"`
	}
	if err := json.Unmarshal(rawVal, &val); err != nil {
		t.Fatalf("decode validate: %v", err)
	}
	for _, d := range val.Diagnostics {
		if d.Severity == "error" {
			t.Fatalf("validate returned error diagnostic: %+v", d)
		}
	}

	// Original must be untouched before commit.
	if got := readCellViaBinary(t, input, "1", "A1"); got == "first" {
		t.Fatalf("original was modified before commit: A1=%q", got)
	}
	if _, err := os.Stat(outPath); !os.IsNotExist(err) {
		t.Fatalf("output should not exist before commit, stat err=%v", err)
	}

	// commit writes the output.
	c.mustResult("commit", map[string]interface{}{"session": session})

	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "first" {
		t.Fatalf("committed A1 = %q, want first", got)
	}
	if got := readCellViaBinary(t, outPath, "1", "A2"); got != "second" {
		t.Fatalf("committed A2 = %q, want second", got)
	}

	// Committed file passes strict validation.
	if err := exec.Command(serveBinary, "validate", "--strict", outPath).Run(); err != nil {
		t.Fatalf("committed file failed validate --strict: %v", err)
	}
}

func TestServeCommitRewritesScratchReadbackAndQuotesValidateCommand(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	outPath := filepath.Join(t.TempDir(), "published workbook.xlsx")
	c := newRPCConn(t)
	session := openServeSession(c, input, outPath)

	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "ooxml xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "published"},
	})
	raw := c.mustResult("commit", map[string]interface{}{"session": session})
	var result apply.Result
	if err := json.Unmarshal(raw, &result); err != nil {
		t.Fatalf("decode commit result: %v (%s)", err, raw)
	}
	if result.Output != outPath || result.ValidateCommand == "" {
		t.Fatalf("unexpected commit result metadata: %+v", result)
	}
	if !strings.Contains(result.ValidateCommand, "'"+outPath+"'") {
		t.Fatalf("validate command should quote output with spaces: %q", result.ValidateCommand)
	}
	if len(result.Applied) != 1 || result.Applied[0].Command != "xlsx cells set" || result.Applied[0].Readback == nil {
		t.Fatalf("unexpected applied result: %+v", result.Applied)
	}
	readback := string(result.Applied[0].Readback)
	if !strings.Contains(readback, jsonStringPathFragment(outPath)) {
		t.Fatalf("commit readback should point at published output %q, got %s", outPath, readback)
	}
	if strings.Contains(readback, "ooxml-serve-") {
		t.Fatalf("commit readback leaked serve scratch path: %s", readback)
	}
	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "published" {
		t.Fatalf("committed A1 = %q, want published", got)
	}
}

// TestServeAbortDiscards confirms abort writes no output and leaves the original
// untouched.
func TestServeAbortDiscards(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	before, err := os.ReadFile(input)
	if err != nil {
		t.Fatal(err)
	}
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newRPCConn(t)

	session := openServeSession(c, input, outPath)
	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "discarded"},
	})

	c.mustResult("abort", map[string]interface{}{"session": session})

	if _, err := os.Stat(outPath); !os.IsNotExist(err) {
		t.Fatalf("output should not exist after abort, stat err=%v", err)
	}
	after, err := os.ReadFile(input)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatalf("original changed after abort")
	}

	// Session is consumed: a further op must error.
	resp := c.call("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "x"},
	})
	if resp.Error == nil {
		t.Fatalf("expected error using an aborted session")
	}
}

// TestServeFailingOpKeepsSessionUsable confirms a failing op returns a JSON-RPC
// error, does not corrupt the working copy, leaves the original untouched, and
// the session can still apply a subsequent good op and commit.
func TestServeFailingOpKeepsSessionUsable(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	before, err := os.ReadFile(input)
	if err != nil {
		t.Fatal(err)
	}
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newRPCConn(t)

	session := openServeSession(c, input, outPath)

	// Good op.
	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "ok"},
	})

	// Failing op: bogus subcommand -> subprocess exits non-zero.
	resp := c.call("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells bogus-subcommand",
		"args":    map[string]interface{}{"sheet": "1"},
	})
	if resp.Error == nil {
		t.Fatalf("expected error for failing op")
	}
	if resp.Error.Data == nil {
		t.Fatalf("expected ErrorBody data on op failure")
	}

	// Original still untouched.
	after, err := os.ReadFile(input)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatalf("original changed after failing op")
	}

	// Session remains usable: a subsequent good op + commit succeeds.
	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A2", "value": "after"},
	})
	c.mustResult("commit", map[string]interface{}{"session": session})

	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "ok" {
		t.Fatalf("committed A1 = %q, want ok (failed op must not have advanced state)", got)
	}
	if got := readCellViaBinary(t, outPath, "1", "A2"); got != "after" {
		t.Fatalf("committed A2 = %q, want after", got)
	}
}

func TestServeFailingOpPreservesChildEnvelope(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newRPCConn(t)
	session := openServeSession(c, input, outPath)

	resp := c.call("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "NoSuchSheet", "cell": "A1", "value": "x"},
	})
	if resp.Error == nil {
		t.Fatalf("expected bad selector op to fail")
	}
	if resp.Error.Data == nil {
		t.Fatalf("expected structured ErrorBody data")
	}
	if resp.Error.Data.Code != codeForExit(ExitTargetNotFound) || resp.Error.Data.ExitCode != ExitTargetNotFound {
		t.Fatalf("serve op error data = %+v, want target_not_found/%d", resp.Error.Data, ExitTargetNotFound)
	}
	if len(resp.Error.Data.Diagnostics) == 0 || resp.Error.Data.Diagnostics[0].Code != "op_failed" {
		t.Fatalf("serve op error missing op_failed diagnostic: %+v", resp.Error.Data.Diagnostics)
	}
	if !strings.Contains(resp.Error.Message, "op 0 (xlsx cells set) failed") {
		t.Fatalf("serve op error missing op context: %s", resp.Error.Message)
	}
	c.mustResult("abort", map[string]interface{}{"session": session})
}

func TestServeOpRejectsFlagsEmbeddedInCommand(t *testing.T) {
	resetFlags()
	input := stageServeXLSX(t)
	c := newRPCConn(t)
	session := openServeDryRunSession(c, input)

	resp := c.call("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set --sheet 1",
		"args":    map[string]interface{}{"cell": "A1", "value": "x"},
	})
	if resp.Error == nil {
		t.Fatalf("expected embedded command flag to fail")
	}
	if resp.Error.Data == nil || resp.Error.Data.Code != "invalid_args" {
		t.Fatalf("embedded command flag error data = %+v", resp.Error.Data)
	}
	if !strings.Contains(resp.Error.Message, `put flag "--sheet" in args`) {
		t.Fatalf("embedded command flag error should explain args usage: %s", resp.Error.Message)
	}
}

// TestServeMultiSourceRejected confirms a clone/import/merge op is rejected with
// a clear error rather than mis-applied.
func TestServeMultiSourceRejected(t *testing.T) {
	resetFlags()
	src := filepath.Join(serveRepoRoot(t), "testdata", "pptx", "title-content", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatal(err)
	}
	input := filepath.Join(t.TempDir(), "deck.pptx")
	if err := os.WriteFile(input, data, 0o644); err != nil {
		t.Fatal(err)
	}
	outPath := filepath.Join(t.TempDir(), "out.pptx")
	c := newRPCConn(t)
	session := openServeSession(c, input, outPath)

	resp := c.call("op", map[string]interface{}{
		"session": session,
		"command": "pptx slides merge",
		"args":    map[string]interface{}{},
	})
	if resp.Error == nil {
		t.Fatalf("expected error for multi-source op")
	}
	if !strings.Contains(resp.Error.Message, "op can supply only the package file") {
		t.Fatalf("expected positional-limit rejection, got %q", resp.Error.Message)
	}
}

func TestServeLoopParseErrorIDNull(t *testing.T) {
	resetFlags()
	c := newRPCConn(t)
	var in bytes.Buffer
	in.WriteString("{not json\n")
	var out bytes.Buffer
	if err := c.loop.run(&in, &out); err != nil {
		t.Fatalf("loop.run: %v", err)
	}
	var resp rpcResponse
	if err := json.Unmarshal(bytes.TrimSpace(out.Bytes()), &resp); err != nil {
		t.Fatalf("decode parse error response: %v (%s)", err, out.String())
	}
	if resp.Error == nil || resp.Error.Code != rpcParseError {
		t.Fatalf("response should be parse error, got %+v", resp)
	}
	if string(resp.ID) != "null" {
		t.Fatalf("parse-error response id = %s, want null", resp.ID)
	}
}

// TestServeOversizedLineDoesNotKillLoopOrSessions is the hunt regression: a
// request line larger than the old 16MB scanner cap must NOT terminate the loop.
// Under the old capped bufio.Scanner, an over-cap line returned bufio.ErrTooLong
// from run(), which unwound to runServe's `defer engine.Close()` and reaped
// EVERY still-open session's uncommitted working copy. The line must instead
// fail per-request and the loop must keep serving, leaving open sessions intact.
func TestServeOversizedLineDoesNotKillLoopOrSessions(t *testing.T) {
	resetFlags()
	src := filepath.Join(serveRepoRoot(t), "testdata", "pptx", "title-content", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatal(err)
	}
	input := filepath.Join(t.TempDir(), "deck.pptx")
	if err := os.WriteFile(input, data, 0o644); err != nil {
		t.Fatal(err)
	}
	outPath := filepath.Join(t.TempDir(), "out.pptx")

	c := newRPCConn(t)
	session := openServeSession(c, input, outPath)

	// Push an oversized (>16MB) non-JSON line directly through the loop. The old
	// capped scanner returned an error here (and call() fatals on a run() error);
	// the new bufio.Reader framing handles it per-request and returns nil.
	var in bytes.Buffer
	in.Write(bytes.Repeat([]byte("x"), 17*1024*1024))
	in.WriteByte('\n')
	var out bytes.Buffer
	if rerr := c.loop.run(&in, &out); rerr != nil {
		t.Fatalf("oversized line must not terminate the loop, got %v", rerr)
	}
	var resp rpcResponse
	if err := json.Unmarshal(bytes.TrimSpace(out.Bytes()), &resp); err != nil {
		t.Fatalf("decode oversized-line response: %v", err)
	}
	if resp.Error == nil || resp.Error.Code != rpcParseError {
		t.Fatalf("oversized line should yield a parse error, got %+v", resp)
	}

	// The previously-open session must still be intact and usable.
	c.mustResult("validate", map[string]interface{}{"session": session})
}

// TestReadRPCLineBoundsAndResyncs pins the bounded framing: a line over
// maxRPCLineBytes is reported tooLong with a nil (memory-bounded) line, the rest
// of that line is discarded up to the newline, and the FOLLOWING line still frames
// cleanly. Uses a small cap so no large allocation is needed.
func TestReadRPCLineBoundsAndResyncs(t *testing.T) {
	orig := maxRPCLineBytes
	maxRPCLineBytes = 64
	t.Cleanup(func() { maxRPCLineBytes = orig })

	// First line is 200 bytes (> cap); second is a valid ~40-byte frame (< cap).
	input := strings.Repeat("x", 200) + "\n" + `{"jsonrpc":"2.0","id":1,"method":"ping"}` + "\n"
	r := bufio.NewReader(strings.NewReader(input))

	line, tooLong, err := readRPCLine(r)
	if err != nil {
		t.Fatalf("first readRPCLine err = %v", err)
	}
	if !tooLong || line != nil {
		t.Fatalf("oversized line: tooLong=%v line=%q, want tooLong=true nil", tooLong, line)
	}

	line, tooLong, err = readRPCLine(r)
	if tooLong {
		t.Fatal("second line should be within cap")
	}
	if got := strings.TrimSpace(string(line)); got != `{"jsonrpc":"2.0","id":1,"method":"ping"}` {
		t.Fatalf("second line did not frame cleanly after an oversized line: %q (err=%v)", got, err)
	}
}

func TestServeRejectsInvalidJSONRPCEnvelope(t *testing.T) {
	resetFlags()
	loop := &serveLoop{engine: serve.NewEngine(serveBinary, t.TempDir())}
	cases := []struct {
		name string
		line string
		want string
	}{
		{"missing jsonrpc", `{"id":1,"method":"initialize"}`, "missing jsonrpc"},
		{"wrong jsonrpc", `{"jsonrpc":"1.0","id":1,"method":"initialize"}`, `jsonrpc must be "2.0"`},
		{"object id", `{"jsonrpc":"2.0","id":{"bad":true},"method":"initialize"}`, "id must be a string, number, or null"},
		{"unknown top field", `{"jsonrpc":"2.0","id":1,"method":"initialize","extra":true}`, `unknown top-level field "extra"`},
		{"non-string method", `{"jsonrpc":"2.0","id":1,"method":12}`, "method must be a non-empty string"},
		{"batch envelope", `[{"jsonrpc":"2.0","id":1,"method":"initialize"}]`, "batch requests are not supported"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			resp := loop.handleLine([]byte(tc.line))
			if resp == nil || resp.Error == nil {
				t.Fatalf("expected invalid request error for %s, got %+v", tc.line, resp)
			}
			if resp.Error.Code != rpcInvalidRequest {
				t.Fatalf("error code = %d, want %d", resp.Error.Code, rpcInvalidRequest)
			}
			if resp.Error.Data == nil || !strings.Contains(resp.Error.Data.Message, tc.want) {
				t.Fatalf("error data = %+v, want message containing %q", resp.Error.Data, tc.want)
			}
		})
	}
}

// TestServeCloneSlideAllowed confirms that clone-slide (a single-deck op, NOT a
// two-source op) dispatches normally through serve and commits a valid file.
func TestServeCloneSlideAllowed(t *testing.T) {
	resetFlags()
	src := filepath.Join(serveRepoRoot(t), "testdata", "pptx", "title-content", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatal(err)
	}
	input := filepath.Join(t.TempDir(), "deck.pptx")
	if err := os.WriteFile(input, data, 0o644); err != nil {
		t.Fatal(err)
	}
	outPath := filepath.Join(t.TempDir(), "out.pptx")
	c := newRPCConn(t)
	session := openServeSession(c, input, outPath)

	c.mustResult("op", map[string]interface{}{
		"session": session,
		"command": "pptx clone-slide",
		"args":    map[string]interface{}{"slide": "1"},
	})
	c.mustResult("commit", map[string]interface{}{"session": session})

	if err := exec.Command(serveBinary, "validate", "--strict", outPath).Run(); err != nil {
		t.Fatalf("committed clone-slide file failed validate --strict: %v", err)
	}
}

// TestServeInitialize confirms the handshake advertises the method set.
func TestServeInitialize(t *testing.T) {
	resetFlags()
	c := newRPCConn(t)
	raw := c.mustResult("initialize", nil)
	var r initializeResult
	if err := json.Unmarshal(raw, &r); err != nil {
		t.Fatalf("decode initialize: %v", err)
	}
	if r.Server != "ooxml-serve" {
		t.Fatalf("server = %q", r.Server)
	}
	want := map[string]bool{"open": false, "op": false, "inspect": false, "validate": false, "plan": false, "commit": false, "abort": false}
	for _, m := range r.Methods {
		if _, ok := want[m]; ok {
			want[m] = true
		}
	}
	for m, seen := range want {
		if !seen {
			t.Fatalf("initialize missing method %q", m)
		}
	}
}
