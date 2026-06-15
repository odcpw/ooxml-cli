package cli

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/ooxml-cli/ooxml-cli/pkg/serve"
)

// TestInspectErrorNextActionsAreReadFramed pins that a failed inspect (read-only)
// gets recovery guidance framed around re-running inspect, NOT the op tool's
// "retry the op ... or call abort" wording (inspect mutates nothing and has no
// session to abort). The op variant must still mention abort.
func TestInspectErrorNextActionsAreReadFramed(t *testing.T) {
	oe := &apply.OpError{Command: "xlsx cells extract"}
	body := &ErrorBody{Code: codeForExit(ExitInvalidArgs), ExitCode: ExitInvalidArgs}

	inspectActions := nextActionsForInspectError(body, oe)
	if len(inspectActions) == 0 {
		t.Fatal("expected inspect next_actions for a recoverable failure")
	}
	joined := strings.Join(inspectActions, " | ")
	if strings.Contains(joined, "abort") || strings.Contains(joined, "retry the op") {
		t.Fatalf("inspect next_actions must not borrow op/abort wording: %q", joined)
	}
	if !strings.Contains(joined, "re-run inspect") {
		t.Fatalf("inspect next_actions should frame recovery around inspect: %q", joined)
	}

	if op := strings.Join(nextActionsForOpError(body, oe), " | "); !strings.Contains(op, "abort") {
		t.Fatalf("op next_actions should still mention abort, got %q", op)
	}
}

// TestMCPOversizedLineDoesNotKillLoopOrSessions is the MCP-door twin of the serve
// oversized-line regression: mcpLoop.run shared the same 16MB-capped scanner, so
// an over-cap tools/call line (e.g. a base64 image payload) returned ErrTooLong
// and unwound to runMCP's defer engine.Close(), reaping every open session. The
// line must now fail per-request and leave the loop and sessions alive.
func TestMCPOversizedLineDoesNotKillLoopOrSessions(t *testing.T) {
	c := newMCPConn(t)
	input := mcpStageXLSX(t)
	out := filepath.Join(t.TempDir(), "out.xlsx")
	session := mcpOpenSession(c, input, out)

	var in bytes.Buffer
	in.Write(bytes.Repeat([]byte("x"), 17*1024*1024))
	in.WriteByte('\n')
	var outBuf bytes.Buffer
	if rerr := c.loop.run(&in, &outBuf); rerr != nil {
		t.Fatalf("oversized line must not terminate the MCP loop, got %v", rerr)
	}
	var resp rpcResponse
	if err := json.Unmarshal(bytes.TrimSpace(outBuf.Bytes()), &resp); err != nil {
		t.Fatalf("decode oversized-line response: %v", err)
	}
	if resp.Error == nil || resp.Error.Code != rpcParseError {
		t.Fatalf("oversized line should yield a parse error, got %+v", resp)
	}
	if res := c.callTool("validate", map[string]interface{}{"session": session}); res.IsError {
		t.Fatalf("session must survive an oversized line; validate isError: %s", res.StructuredContent)
	}
}

func TestMCPRejectsInvalidJSONRPCEnvelope(t *testing.T) {
	resetFlags()
	loop := &mcpLoop{engine: serve.NewEngine(serveBinary, t.TempDir())}
	cases := []struct {
		name string
		line string
		want string
	}{
		{"missing jsonrpc", `{"id":1,"method":"tools/list"}`, "missing jsonrpc"},
		{"wrong jsonrpc", `{"jsonrpc":"1.0","id":1,"method":"tools/list"}`, `jsonrpc must be "2.0"`},
		{"object id", `{"jsonrpc":"2.0","id":{"bad":true},"method":"tools/list"}`, "id must be a string, number, or null"},
		{"unknown top field", `{"jsonrpc":"2.0","id":1,"method":"tools/list","extra":true}`, `unknown top-level field "extra"`},
		{"non-string method", `{"jsonrpc":"2.0","id":1,"method":12}`, "method must be a non-empty string"},
		{"batch envelope", `[{"jsonrpc":"2.0","id":1,"method":"tools/list"}]`, "batch requests are not supported"},
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

// mcpConn drives the MCP JSON-RPC loop over in-memory buffers, one request at a
// time, modeled on the serve_test rpcConn. It reuses serveBinary (built once in
// the package TestMain) as the engine's Self.
type mcpConn struct {
	t    *testing.T
	loop *mcpLoop
	id   int
}

func newMCPConn(t *testing.T) *mcpConn {
	t.Helper()
	engine := serve.NewEngine(serveBinary, t.TempDir())
	return &mcpConn{t: t, loop: &mcpLoop{engine: engine}}
}

func (c *mcpConn) call(method string, params map[string]interface{}) rpcResponse {
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

// mustResult returns the parsed JSON-RPC result, failing on a protocol error.
func (c *mcpConn) mustResult(method string, params map[string]interface{}) json.RawMessage {
	c.t.Helper()
	resp := c.call(method, params)
	if resp.Error != nil {
		c.t.Fatalf("%s returned protocol error: code=%d msg=%s data=%+v", method, resp.Error.Code, resp.Error.Message, resp.Error.Data)
	}
	raw, err := json.Marshal(resp.Result)
	if err != nil {
		c.t.Fatalf("re-marshal result: %v", err)
	}
	return raw
}

// callTool calls a tool and returns the parsed CallToolResult, failing on a
// protocol error (tool-level failures live inside the result as isError).
func (c *mcpConn) callTool(name string, args map[string]interface{}) mcpCallToolResult {
	c.t.Helper()
	raw := c.mustResult("tools/call", map[string]interface{}{
		"name":      name,
		"arguments": args,
	})
	var res mcpCallToolResult
	if err := json.Unmarshal(raw, &res); err != nil {
		c.t.Fatalf("decode CallToolResult (raw=%q): %v", raw, err)
	}
	return res
}

func mcpStageXLSX(t *testing.T) string {
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

func mcpOpenSession(c *mcpConn, file, out string) string {
	c.t.Helper()
	res := c.callTool("open", map[string]interface{}{"file": file, "out": out})
	if res.IsError {
		c.t.Fatalf("open returned isError: %s", res.StructuredContent)
	}
	// structuredContent IS the engine JSON with next_actions merged as a sibling.
	var body struct {
		SessionID   string   `json:"sessionId"`
		Type        string   `json:"type"`
		NextActions []string `json:"next_actions"`
	}
	if err := json.Unmarshal(res.StructuredContent, &body); err != nil {
		c.t.Fatalf("decode open structuredContent (raw=%q): %v", res.StructuredContent, err)
	}
	if body.SessionID == "" {
		c.t.Fatalf("open returned empty sessionId: %s", res.StructuredContent)
	}
	if len(body.NextActions) == 0 {
		c.t.Fatalf("open did not emit next_actions: %s", res.StructuredContent)
	}
	if len(res.Content) == 0 || res.Content[0].Type != "text" || res.Content[0].Text == "" {
		c.t.Fatalf("open result missing text content block: %+v", res.Content)
	}
	return body.SessionID
}

func assertMCPErrorNextActions(t *testing.T, structured json.RawMessage, wants ...string) {
	t.Helper()
	var env struct {
		Error struct {
			Data map[string]interface{} `json:"data"`
		} `json:"error"`
	}
	if err := json.Unmarshal(structured, &env); err != nil {
		t.Fatalf("decode MCP error structuredContent: %v\n%s", err, structured)
	}
	rawActions, ok := env.Error.Data["next_actions"].([]interface{})
	if !ok || len(rawActions) == 0 {
		t.Fatalf("MCP error missing next_actions: %s", structured)
	}
	var joined strings.Builder
	for _, action := range rawActions {
		fmt.Fprintf(&joined, " %s", strings.ToLower(fmt.Sprint(action)))
	}
	lower := joined.String()
	for _, want := range wants {
		if !strings.Contains(lower, strings.ToLower(want)) {
			t.Fatalf("MCP next_actions missing %q in %v", want, rawActions)
		}
	}
}

func mcpOpenDryRunSession(c *mcpConn, file string) string {
	c.t.Helper()
	res := c.callTool("open", map[string]interface{}{"file": file, "dryRun": true})
	if res.IsError {
		c.t.Fatalf("open returned isError: %s", res.StructuredContent)
	}
	var body struct {
		SessionID string `json:"sessionId"`
		Type      string `json:"type"`
	}
	if err := json.Unmarshal(res.StructuredContent, &body); err != nil {
		c.t.Fatalf("decode open structuredContent (raw=%q): %v", res.StructuredContent, err)
	}
	if body.SessionID == "" {
		c.t.Fatalf("open returned empty sessionId: %s", res.StructuredContent)
	}
	return body.SessionID
}

// TestMCPInitialize confirms the MCP handshake returns serverInfo + declared
// capabilities and echoes the client's requested protocolVersion.
func TestMCPInitialize(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)
	raw := c.mustResult("initialize", map[string]interface{}{
		"protocolVersion": "2025-06-18",
		"clientInfo":      map[string]interface{}{"name": "test", "version": "1"},
	})
	var r mcpInitializeResult
	if err := json.Unmarshal(raw, &r); err != nil {
		t.Fatalf("decode initialize: %v", err)
	}
	if r.ServerInfo.Name != "ooxml" {
		t.Fatalf("serverInfo.name = %q, want ooxml", r.ServerInfo.Name)
	}
	if r.ServerInfo.Version == "" {
		t.Fatalf("serverInfo.version is empty")
	}
	if r.ProtocolVersion != "2025-06-18" {
		t.Fatalf("protocolVersion = %q, want echoed 2025-06-18", r.ProtocolVersion)
	}
	if r.Capabilities.Tools == nil {
		t.Fatalf("capabilities.tools not declared")
	}
	if r.Capabilities.Resources == nil {
		t.Fatalf("capabilities.resources not declared")
	}
}

// TestMCPInitializeDefaultsProtocolVersion confirms a missing protocolVersion
// falls back to a known-valid revision.
func TestMCPInitializeDefaultsProtocolVersion(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)
	raw := c.mustResult("initialize", nil)
	var r mcpInitializeResult
	if err := json.Unmarshal(raw, &r); err != nil {
		t.Fatalf("decode initialize: %v", err)
	}
	if r.ProtocolVersion != mcpProtocolVersion {
		t.Fatalf("protocolVersion = %q, want default %q", r.ProtocolVersion, mcpProtocolVersion)
	}
}

// TestMCPToolsList confirms the seven generic session tools are advertised with
// valid object-typed JSON-Schema inputSchemas.
func TestMCPToolsList(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)
	raw := c.mustResult("tools/list", nil)
	var r mcpToolsListResult
	if err := json.Unmarshal(raw, &r); err != nil {
		t.Fatalf("decode tools/list: %v", err)
	}
	want := map[string]bool{"open": false, "op": false, "inspect": false, "validate": false, "plan": false, "commit": false, "abort": false}
	for _, tool := range r.Tools {
		if _, ok := want[tool.Name]; ok {
			want[tool.Name] = true
		}
		if tool.Description == "" {
			t.Fatalf("tool %q has empty description", tool.Name)
		}
		// Each inputSchema must be a JSON object with type:"object".
		var schema struct {
			Type       string                 `json:"type"`
			Properties map[string]interface{} `json:"properties"`
		}
		if err := json.Unmarshal(tool.InputSchema, &schema); err != nil {
			t.Fatalf("tool %q inputSchema not valid JSON: %v", tool.Name, err)
		}
		if schema.Type != "object" {
			t.Fatalf("tool %q inputSchema type = %q, want object", tool.Name, schema.Type)
		}
		if len(schema.Properties) == 0 {
			t.Fatalf("tool %q inputSchema has no properties", tool.Name)
		}
	}
	for name, seen := range want {
		if !seen {
			t.Fatalf("tools/list missing tool %q", name)
		}
	}
	if len(r.Tools) != 7 {
		t.Fatalf("tools/list returned %d tools, want 7", len(r.Tools))
	}
}

// TestMCPToolArgumentsRejectUnknownTopLevelFields keeps tools/list schemas and
// tools/call behavior aligned: the schemas advertise additionalProperties:false
// for each tool's argument object, so top-level typos must not be silently
// ignored. The nested op/inspect args map remains command-specific and flexible.
func TestMCPToolArgumentsRejectUnknownTopLevelFields(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)

	resp := c.call("tools/call", map[string]interface{}{
		"name": "open",
		"arguments": map[string]interface{}{
			"file": "/tmp/does-not-matter.xlsx",
			"out":  "/tmp/out.xlsx",
			"typo": true,
		},
	})
	if resp.Error == nil {
		t.Fatalf("expected protocol invalid_params for unknown tool argument")
	}
	if resp.Error.Code != rpcInvalidParams {
		t.Fatalf("unknown argument error code = %d, want %d", resp.Error.Code, rpcInvalidParams)
	}
	if resp.Error.Data == nil || !strings.Contains(resp.Error.Data.Message, "unknown field") {
		t.Fatalf("unknown argument error missing field detail: %+v", resp.Error)
	}
}

// TestMCPFlow is the end-to-end happy path over tools/call: open -> op (mutation)
// -> inspect (reflects it) -> validate (clean) -> commit (writes a file that
// passes validate --strict).
func TestMCPFlow(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newMCPConn(t)

	session := mcpOpenSession(c, input, outPath)

	// op: set A1.
	opRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "ooxml xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "mcphello"},
	})
	if opRes.IsError {
		t.Fatalf("op returned isError: %s", opRes.StructuredContent)
	}
	if len(opRes.Content) == 0 || opRes.Content[0].Type != "text" {
		t.Fatalf("op result missing text content block: %+v", opRes.Content)
	}
	// next_actions ride as a sibling field inside the engine JSON.
	if !strings.Contains(string(opRes.StructuredContent), "next_actions") {
		t.Fatalf("op result missing next_actions: %s", opRes.StructuredContent)
	}

	// inspect reflects the op.
	insRes := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "ooxml xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1"},
	})
	if insRes.IsError {
		t.Fatalf("inspect returned isError: %s", insRes.StructuredContent)
	}
	if !strings.Contains(string(insRes.StructuredContent), "mcphello") {
		t.Fatalf("inspect did not reflect op: %s", insRes.StructuredContent)
	}

	// validate clean.
	valRes := c.callTool("validate", map[string]interface{}{"session": session})
	if valRes.IsError {
		t.Fatalf("validate returned isError: %s", valRes.StructuredContent)
	}
	if strings.Contains(string(valRes.StructuredContent), "\"severity\":\"error\"") {
		t.Fatalf("validate returned error diagnostic: %s", valRes.StructuredContent)
	}

	// Original untouched, output absent before commit.
	if _, err := os.Stat(outPath); !os.IsNotExist(err) {
		t.Fatalf("output should not exist before commit, stat err=%v", err)
	}

	// commit writes the output.
	comRes := c.callTool("commit", map[string]interface{}{"session": session})
	if comRes.IsError {
		t.Fatalf("commit returned isError: %s", comRes.StructuredContent)
	}

	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "mcphello" {
		t.Fatalf("committed A1 = %q, want mcphello", got)
	}
	if err := exec.Command(serveBinary, "validate", "--strict", outPath).Run(); err != nil {
		t.Fatalf("committed file failed validate --strict: %v", err)
	}
}

func TestMCPOpAcceptsDashfulResourceFlagNames(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newMCPConn(t)
	session := mcpOpenSession(c, input, outPath)

	opRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args": map[string]interface{}{
			"--sheet": "1",
			"--cell":  "A1",
			"--value": "dashful",
		},
	})
	if opRes.IsError {
		t.Fatalf("dashful op returned isError: %s", opRes.StructuredContent)
	}
	insRes := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1"},
	})
	if insRes.IsError || !strings.Contains(string(insRes.StructuredContent), "dashful") {
		t.Fatalf("dashful op did not update session; isError=%t body=%s", insRes.IsError, insRes.StructuredContent)
	}
}

func TestMCPOpRejectsSessionOwnedNestedMutationArgs(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	opRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args": map[string]interface{}{
			"sheet":         "1",
			"cell":          "A1",
			"value":         "x",
			"--no-validate": true,
		},
	})
	if !opRes.IsError {
		t.Fatalf("nested session-owned arg should be isError: %s", opRes.StructuredContent)
	}
	var env struct {
		Error struct {
			Type    string `json:"type"`
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal(opRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode nested arg error: %v\n%s", err, opRes.StructuredContent)
	}
	if env.Error.Type != "invalid_args" || !strings.Contains(env.Error.Message, "owned by the apply/serve/MCP session") {
		t.Fatalf("unexpected nested arg error: %s", opRes.StructuredContent)
	}
	assertMCPErrorNextActions(t, opRes.StructuredContent, "resource://command/xlsx%20cells%20set", "session-owned", "retry")
}

func TestMCPOpRejectsFlagsEmbeddedInCommand(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	opRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set --sheet 1",
		"args":    map[string]interface{}{"cell": "A1", "value": "x"},
	})
	if !opRes.IsError {
		t.Fatalf("embedded command flag should be isError: %s", opRes.StructuredContent)
	}
	var env struct {
		Error struct {
			Type    string `json:"type"`
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal(opRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode embedded command flag error: %v\n%s", err, opRes.StructuredContent)
	}
	if env.Error.Type != "invalid_args" || !strings.Contains(env.Error.Message, `put flag "--sheet" in args`) {
		t.Fatalf("unexpected embedded command flag error: %s", opRes.StructuredContent)
	}
}

func TestMCPOpRejectsNonOperationCommandsBeforeDispatch(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	for _, tt := range []struct {
		command string
		want    string
	}{
		{"xlsx sheets list", "mutation output flags"},
		{"pptx slides move", "op can supply only the package file"},
	} {
		opRes := c.callTool("op", map[string]interface{}{
			"session": session,
			"command": tt.command,
			"args":    map[string]interface{}{},
		})
		if !opRes.IsError {
			t.Fatalf("%s: expected isError result: %s", tt.command, opRes.StructuredContent)
		}
		var env struct {
			Error struct {
				Type    string `json:"type"`
				Message string `json:"message"`
			} `json:"error"`
		}
		if err := json.Unmarshal(opRes.StructuredContent, &env); err != nil {
			t.Fatalf("%s: decode op compatibility error: %v\n%s", tt.command, err, opRes.StructuredContent)
		}
		if env.Error.Type != "invalid_args" || !strings.Contains(env.Error.Message, tt.want) {
			t.Fatalf("%s: unexpected op compatibility error: %s", tt.command, opRes.StructuredContent)
		}
		assertMCPErrorNextActions(t, opRes.StructuredContent, "resource://command/", "retry")
	}

	insRes := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1"},
	})
	if insRes.IsError || !strings.Contains(string(insRes.StructuredContent), "values") {
		t.Fatalf("session should remain usable after rejected op; isError=%t body=%s", insRes.IsError, insRes.StructuredContent)
	}
}

func TestMCPInspectRejectsDefaultArtifactCommand(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	insRes := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "pptx extract images",
	})
	if !insRes.IsError {
		t.Fatalf("inspect artifact command should be isError: %s", insRes.StructuredContent)
	}
	var env struct {
		Error struct {
			Type    string `json:"type"`
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal(insRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode artifact rejection: %v\n%s", err, insRes.StructuredContent)
	}
	if env.Error.Type != "invalid_args" || !strings.Contains(env.Error.Message, "writes image files") {
		t.Fatalf("unexpected artifact rejection: %s", insRes.StructuredContent)
	}
}

func TestMCPInspectRejectsSessionIncompatibleRead(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	insRes := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "diff /tmp/base.xlsx",
	})
	if !insRes.IsError {
		t.Fatalf("inspect session-incompatible command should be isError: %s", insRes.StructuredContent)
	}
	var env struct {
		Error struct {
			Type        string                 `json:"type"`
			Message     string                 `json:"message"`
			Recoverable bool                   `json:"recoverable"`
			Data        map[string]interface{} `json:"data"`
		} `json:"error"`
	}
	if err := json.Unmarshal(insRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode session-incompatible rejection: %v\n%s", err, insRes.StructuredContent)
	}
	if env.Error.Type != "invalid_args" || !env.Error.Recoverable {
		t.Fatalf("unexpected session-incompatible rejection contract: %s", insRes.StructuredContent)
	}
	if !strings.Contains(env.Error.Message, "needs both baseline and candidate") {
		t.Fatalf("session-incompatible rejection should explain diff shape: %s", insRes.StructuredContent)
	}
	if _, ok := env.Error.Data["fix_hint"]; !ok {
		t.Fatalf("session-incompatible rejection missing fix_hint: %s", insRes.StructuredContent)
	}
	assertMCPErrorNextActions(t, insRes.StructuredContent, "commit first", "resource://capabilities")
}

func TestMCPDryRunOpReadbackDoesNotClaimRealWrite(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	opRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "dryrun"},
	})
	if opRes.IsError {
		t.Fatalf("dry-run op returned isError: %s", opRes.StructuredContent)
	}
	body := string(opRes.StructuredContent)
	if strings.Contains(body, "ooxml-serve-") || strings.Contains(body, "working-") {
		t.Fatalf("dry-run MCP op leaked scratch path: %s", body)
	}
	var payload struct {
		Readback struct {
			Output string `json:"output"`
			DryRun bool   `json:"dryRun"`
		} `json:"readback"`
	}
	if err := json.Unmarshal(opRes.StructuredContent, &payload); err != nil {
		t.Fatalf("decode dry-run op structuredContent: %v\n%s", err, body)
	}
	if !payload.Readback.DryRun || payload.Readback.Output != "<dry-run-output>" {
		t.Fatalf("dry-run op readback should be explicitly dry-run with placeholder output: %s", body)
	}
}

// TestMCPInspectRejectsMutationNoLeak proves the MCP inspect tool cannot be
// used as a mutation/output bypass.
func TestMCPInspectRejectsMutationNoLeak(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	leakPath := filepath.Join(t.TempDir(), "leak.xlsx")
	c := newMCPConn(t)
	session := mcpOpenDryRunSession(c, input)

	res := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args": map[string]interface{}{
			"sheet": "1",
			"cell":  "A1",
			"value": "leak",
			"out":   leakPath,
		},
	})
	if !res.IsError {
		t.Fatalf("inspect mutation should be an isError result: %s", res.StructuredContent)
	}
	if _, err := os.Stat(leakPath); !os.IsNotExist(err) {
		t.Fatalf("inspect created leaked output %q (stat err=%v)", leakPath, err)
	}
	var env struct {
		Error struct {
			Type        string                 `json:"type"`
			Recoverable bool                   `json:"recoverable"`
			Data        map[string]interface{} `json:"data"`
		} `json:"error"`
	}
	if err := json.Unmarshal(res.StructuredContent, &env); err != nil {
		t.Fatalf("decode inspect error structuredContent: %v (%s)", err, res.StructuredContent)
	}
	if env.Error.Type != "invalid_args" || !env.Error.Recoverable {
		t.Fatalf("unexpected inspect error contract: %s", res.StructuredContent)
	}
	if _, ok := env.Error.Data["fix_hint"]; !ok {
		t.Fatalf("inspect error missing fix_hint: %s", res.StructuredContent)
	}
	assertMCPErrorNextActions(t, res.StructuredContent, "op tool", "commit first", "resource://capabilities")

	good := c.callTool("inspect", map[string]interface{}{
		"session": session,
		"command": "xlsx ranges export",
		"args":    map[string]interface{}{"sheet": "1", "range": "A1", "include-types": true},
	})
	if good.IsError {
		t.Fatalf("good inspect after rejection returned isError: %s", good.StructuredContent)
	}
	if !strings.Contains(string(good.StructuredContent), "values") {
		t.Fatalf("good inspect after rejection returned unexpected JSON: %s", good.StructuredContent)
	}
	abort := c.callTool("abort", map[string]interface{}{"session": session})
	if abort.IsError {
		t.Fatalf("abort after rejection returned isError: %s", abort.StructuredContent)
	}
}

// TestMCPResourcesCapabilities confirms resources/list advertises only concrete
// readable resources and resources/read returns the capabilities document body.
func TestMCPResourcesCapabilities(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)

	listRaw := c.mustResult("resources/list", nil)
	var list mcpResourcesListResult
	if err := json.Unmarshal(listRaw, &list); err != nil {
		t.Fatalf("decode resources/list: %v", err)
	}
	var sawCaps, sawAgentGuide bool
	for _, r := range list.Resources {
		if r.URI == "resource://capabilities" {
			sawCaps = true
		}
		if r.URI == "resource://agent-guide" {
			sawAgentGuide = true
		}
		if r.URI == "resource://command/{path}" {
			t.Fatalf("resources/list advertised URI template as concrete resource: %+v", r)
		}
	}
	if !sawCaps {
		t.Fatalf("resources/list missing capabilities resource: %+v", list.Resources)
	}
	if !sawAgentGuide {
		t.Fatalf("resources/list missing agent-guide resource: %+v", list.Resources)
	}

	readRaw := c.mustResult("resources/read", map[string]interface{}{"uri": "resource://capabilities"})
	var read mcpResourcesReadResult
	if err := json.Unmarshal(readRaw, &read); err != nil {
		t.Fatalf("decode resources/read: %v", err)
	}
	if len(read.Contents) != 1 {
		t.Fatalf("resources/read returned %d contents, want 1", len(read.Contents))
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(read.Contents[0].Text), &doc); err != nil {
		t.Fatalf("capabilities body not a capabilities document: %v", err)
	}
	if doc.Tool != "ooxml" {
		t.Fatalf("capabilities doc tool = %q, want ooxml", doc.Tool)
	}
	if len(doc.Commands) == 0 {
		t.Fatalf("capabilities doc has no commands")
	}

	readRaw = c.mustResult("resources/read", map[string]interface{}{"uri": "resource://agent-guide"})
	if err := json.Unmarshal(readRaw, &read); err != nil {
		t.Fatalf("decode agent-guide resources/read: %v", err)
	}
	if len(read.Contents) != 1 {
		t.Fatalf("agent-guide resources/read returned %d contents, want 1", len(read.Contents))
	}
	var guide robotDocsGuide
	if err := json.Unmarshal([]byte(read.Contents[0].Text), &guide); err != nil {
		t.Fatalf("agent-guide body not a robotDocsGuide: %v", err)
	}
	if guide.Tool != "ooxml" || !containsRobotDocsCommand(guide.Sections, "ooxml agent guide") {
		t.Fatalf("agent-guide body missing expected agent guide contract: %+v", guide)
	}
}

// TestMCPResourceTemplates confirms dynamic command resources are exposed
// through resources/templates/list instead of resources/list.
func TestMCPResourceTemplates(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)

	templatesRaw := c.mustResult("resources/templates/list", nil)
	var templates mcpResourceTemplatesListResult
	if err := json.Unmarshal(templatesRaw, &templates); err != nil {
		t.Fatalf("decode resources/templates/list: %v", err)
	}
	for _, tmpl := range templates.ResourceTemplates {
		if tmpl.URITemplate == "resource://command/{path}" {
			if tmpl.MIMEType != "application/json" {
				t.Fatalf("command resource template mimeType = %q, want application/json", tmpl.MIMEType)
			}
			return
		}
	}
	t.Fatalf("resources/templates/list missing command URI template: %+v", templates.ResourceTemplates)
}

// TestMCPResourceCommand confirms resource://command/{path} resolves command
// schemas using both the op vocabulary and the full capabilities command path.
func TestMCPResourceCommand(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)

	readRaw := c.mustResult("resources/read", map[string]interface{}{
		"uri": "resource://command/xlsx%20cells%20set",
	})
	var read mcpResourcesReadResult
	if err := json.Unmarshal(readRaw, &read); err != nil {
		t.Fatalf("decode resources/read: %v", err)
	}
	if len(read.Contents) != 1 {
		t.Fatalf("resources/read returned %d contents, want 1", len(read.Contents))
	}
	var cmd capabilityCommand
	if err := json.Unmarshal([]byte(read.Contents[0].Text), &cmd); err != nil {
		t.Fatalf("command body not a capabilityCommand: %v", err)
	}
	if cmd.Path != "ooxml xlsx cells set" {
		t.Fatalf("command resource path = %q, want \"ooxml xlsx cells set\"", cmd.Path)
	}
	argNames := map[string]bool{}
	for _, flag := range cmd.LocalFlags {
		if flag.ArgName != "" {
			argNames[flag.ArgName] = true
		}
	}
	for _, want := range []string{"sheet", "cell", "value"} {
		if !argNames[want] {
			t.Fatalf("command resource missing dashless argName %q in flags: %+v", want, cmd.LocalFlags)
		}
	}

	readRaw = c.mustResult("resources/read", map[string]interface{}{
		"uri": "resource://command/" + url.PathEscape("ooxml xlsx cells set"),
	})
	if err := json.Unmarshal(readRaw, &read); err != nil {
		t.Fatalf("decode full-path resources/read: %v", err)
	}
	if len(read.Contents) != 1 {
		t.Fatalf("full-path resources/read returned %d contents, want 1", len(read.Contents))
	}
	if err := json.Unmarshal([]byte(read.Contents[0].Text), &cmd); err != nil {
		t.Fatalf("full-path command body not a capabilityCommand: %v", err)
	}
	if cmd.Path != "ooxml xlsx cells set" {
		t.Fatalf("full-path command resource path = %q, want \"ooxml xlsx cells set\"", cmd.Path)
	}

	for _, ccmd := range buildCapabilitiesDocument().Commands {
		resp := c.call("resources/read", map[string]interface{}{
			"uri": "resource://command/" + url.PathEscape(ccmd.Path),
		})
		if resp.Error != nil {
			t.Fatalf("command resource for %q failed: code=%d msg=%s data=%+v", ccmd.Path, resp.Error.Code, resp.Error.Message, resp.Error.Data)
		}
	}

	// Unknown command -> clean protocol error, loop survives.
	resp := c.call("resources/read", map[string]interface{}{
		"uri": "resource://command/xlsx%20nonexistent%20command",
	})
	if resp.Error == nil {
		t.Fatalf("expected protocol error for unknown command resource")
	}
}

// TestMCPBadOpRecoverable confirms a real command with a bad selector returns a
// recoverable isError CallToolResult carrying structured data (re-parsed from the
// child --json ErrorBody) and does NOT kill the loop or corrupt the session.
func TestMCPBadOpRecoverable(t *testing.T) {
	resetFlags()
	input := mcpStageXLSX(t)
	outPath := filepath.Join(t.TempDir(), "out.xlsx")
	c := newMCPConn(t)
	session := mcpOpenSession(c, input, outPath)

	// Real command, bogus sheet selector -> child emits a --json ErrorBody.
	badRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "NoSuchSheet", "cell": "A1", "value": "x"},
	})
	if !badRes.IsError {
		t.Fatalf("bad op should be isError, got: %s", badRes.StructuredContent)
	}
	var env struct {
		Error struct {
			Type        string                 `json:"type"`
			Message     string                 `json:"message"`
			Recoverable bool                   `json:"recoverable"`
			Data        map[string]interface{} `json:"data"`
		} `json:"error"`
	}
	if err := json.Unmarshal(badRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode error structuredContent (raw=%q): %v", badRes.StructuredContent, err)
	}
	if env.Error.Type == "" {
		t.Fatalf("error.type empty (re-parse failed?): %s", badRes.StructuredContent)
	}
	// A bad selector is a recoverable, structured failure (not opaque text).
	if !env.Error.Recoverable {
		t.Fatalf("bad-selector op should be recoverable, got type=%q: %s", env.Error.Type, badRes.StructuredContent)
	}
	if env.Error.Data == nil {
		t.Fatalf("error.data missing structured fields: %s", badRes.StructuredContent)
	}
	if _, ok := env.Error.Data["exitCode"]; !ok {
		t.Fatalf("error.data missing exitCode: %s", badRes.StructuredContent)
	}
	actions, ok := env.Error.Data["next_actions"].([]interface{})
	if !ok || len(actions) == 0 {
		t.Fatalf("recoverable op error missing next_actions: %s", badRes.StructuredContent)
	}
	joinedActions := ""
	for _, action := range actions {
		joinedActions += " " + strings.ToLower(action.(string))
	}
	for _, want := range []string{"inspect", "resource://command/xlsx%20cells%20set", "retry"} {
		if !strings.Contains(joinedActions, want) {
			t.Fatalf("next_actions missing %q: %v", want, actions)
		}
	}

	// Loop survived: session is still usable. A good op then commits cleanly.
	goodRes := c.callTool("op", map[string]interface{}{
		"session": session,
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "recovered"},
	})
	if goodRes.IsError {
		t.Fatalf("good op after bad op returned isError: %s", goodRes.StructuredContent)
	}
	comRes := c.callTool("commit", map[string]interface{}{"session": session})
	if comRes.IsError {
		t.Fatalf("commit after recovery returned isError: %s", comRes.StructuredContent)
	}
	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "recovered" {
		t.Fatalf("committed A1 = %q, want recovered", got)
	}
}

// TestMCPUnknownToolAndSession confirms an unknown tool and an unknown session id
// each return a clean, recoverable error WITHOUT killing the stdio loop.
func TestMCPUnknownToolAndSession(t *testing.T) {
	resetFlags()
	c := newMCPConn(t)

	// Unknown tool: recoverable isError carrying available_tools.
	unkRes := c.callTool("frobnicate", map[string]interface{}{})
	if !unkRes.IsError {
		t.Fatalf("unknown tool should be isError")
	}
	var env struct {
		Error struct {
			Type        string                 `json:"type"`
			Recoverable bool                   `json:"recoverable"`
			Data        map[string]interface{} `json:"data"`
		} `json:"error"`
	}
	if err := json.Unmarshal(unkRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode unknown-tool error: %v", err)
	}
	if _, ok := env.Error.Data["available_tools"]; !ok {
		t.Fatalf("unknown tool error missing available_tools: %s", unkRes.StructuredContent)
	}

	// Unknown session id: recoverable isError with a re-open hint.
	sessRes := c.callTool("op", map[string]interface{}{
		"session": "s-does-not-exist",
		"command": "xlsx cells set",
		"args":    map[string]interface{}{"sheet": "1", "cell": "A1", "value": "x"},
	})
	if !sessRes.IsError {
		t.Fatalf("unknown session should be isError")
	}
	if err := json.Unmarshal(sessRes.StructuredContent, &env); err != nil {
		t.Fatalf("decode unknown-session error: %v", err)
	}
	if env.Error.Type != "target_not_found" {
		t.Fatalf("unknown session error type = %q, want target_not_found", env.Error.Type)
	}
	if !env.Error.Recoverable {
		t.Fatalf("unknown session should be recoverable")
	}

	// Unknown METHOD is a protocol error, but the loop must survive (a subsequent
	// valid request on a fresh line still works — exercised by tools/list here).
	resp := c.call("no/such/method", nil)
	if resp.Error == nil {
		t.Fatalf("unknown method should be a protocol error")
	}
	if _, err := json.Marshal(c.mustResult("tools/list", nil)); err != nil {
		t.Fatalf("loop did not survive unknown method: %v", err)
	}
}

// TestMCPLoopSurvivesMalformedLine confirms a malformed JSON line returns a parse
// error and does not end the loop for subsequent well-formed lines.
func TestMCPLoopSurvivesMalformedLine(t *testing.T) {
	resetFlags()
	engine := serve.NewEngine(serveBinary, t.TempDir())
	loop := &mcpLoop{engine: engine}

	var in bytes.Buffer
	in.WriteString("{not json\n")
	in.WriteString(`{"jsonrpc":"2.0","id":1,"method":"tools/list"}` + "\n")
	in.WriteString(`{"jsonrpc":"2.0","method":"notifications/initialized"}` + "\n")
	var out bytes.Buffer
	if err := loop.run(&in, &out); err != nil {
		t.Fatalf("loop.run: %v", err)
	}

	dec := json.NewDecoder(bytes.NewReader(out.Bytes()))
	// First response: parse error (id null).
	var first rpcResponse
	if err := dec.Decode(&first); err != nil {
		t.Fatalf("decode first response: %v", err)
	}
	if first.Error == nil || first.Error.Code != rpcParseError {
		t.Fatalf("first response should be parse error, got %+v", first)
	}
	if string(first.ID) != "null" {
		t.Fatalf("parse-error response id = %s, want null", first.ID)
	}
	// Second response: tools/list result.
	var second rpcResponse
	if err := dec.Decode(&second); err != nil {
		t.Fatalf("decode second response: %v", err)
	}
	if second.Error != nil {
		t.Fatalf("second response should succeed, got error %+v", second.Error)
	}
	// The notification (no id) produced no third response.
	var third rpcResponse
	if err := dec.Decode(&third); err == nil {
		t.Fatalf("notification should produce no response, got %+v", third)
	}
}
