package cli

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"net/url"
	"sort"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/ooxml-cli/ooxml-cli/pkg/serve"
)

// mcpProtocolVersion is the MCP revision this server speaks when a client does
// not request one we recognize.
const mcpProtocolVersion = "2025-06-18"

// mcpSupportedProtocolVersions are the MCP revisions whose tools/resources subset
// this server implements; a client requesting one of these gets it echoed back,
// otherwise the handshake falls back to mcpProtocolVersion.
var mcpSupportedProtocolVersions = map[string]bool{
	"2025-06-18": true,
	"2025-03-26": true,
	"2024-11-05": true,
}

// mcpServerName is the server identity advertised in the initialize handshake.
const mcpServerName = "ooxml"

// mcpCmd exposes the serve SessionEngine as a Model Context Protocol server over
// newline-delimited JSON-RPC 2.0 on stdin/stdout. It is the THIRD door (alongside
// the CLI and `ooxml serve`) onto the same spine: it authors no mutation logic and
// no schemas of its own — tools and resources are projected from the one
// capabilities contract (buildCapabilitiesDocument) and dispatched to the same
// serve.Engine the `serve` command uses.
var mcpCmd = &cobra.Command{
	Use:   "mcp",
	Short: "Run an MCP (Model Context Protocol) server over stdio",
	Long: `Run an MCP server speaking newline-delimited JSON-RPC 2.0 over stdin/stdout.

This exposes the OOXML session engine as MCP tools and resources so an agent can
open a working copy of a file, apply mutation ops against it, inspect/validate the
evolving state, and commit atomically — all through the standardized MCP tool
interface (stdout is protocol-only; stderr is human/diagnostic logging).

Seven generic SESSION tools cover the session-safe command surface:
  open, op, inspect, validate, plan, commit, abort
The generic op {session, command, args} reaches mutation commands whose
capabilities entry has opCompatible=true. The generic inspect {session, command,
args} reaches read-only, artifact-free commands that can run against the
session's working copy. The exact command/args strings follow the capabilities
contract, discoverable through the resource://capabilities and
resource://command/{path} resources.

This is an MCP SERVER, not a shell command — configure it in your agent's MCP
config and call the tools. Errors that an agent can recover from are returned as
CallToolResult{isError:true} with structured data and next_actions, so an obvious
mistake educates rather than just failing.`,
	Args:          cobra.NoArgs,
	SilenceUsage:  true,
	SilenceErrors: true,
	RunE:          runMCP,
}

func runMCP(cmd *cobra.Command, args []string) error {
	self, err := serveSelfExecutable()
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to resolve own executable path: %v", err)
	}
	engine := serve.NewEngine(self, applyTempBase(cmd))
	// On graceful shutdown (stdio EOF) reap any sessions opened but never
	// committed/aborted, so their working-copy scratch dirs are not leaked.
	defer engine.Close()
	loop := &mcpLoop{engine: engine}
	return loop.run(cmd.InOrStdin(), cmd.OutOrStdout())
}

// mcpLoop drives the MCP JSON-RPC request/response loop over a reader/writer pair.
// It reuses the same bufio framing, notification handling, and param-decode
// discipline as serveLoop; only the method set and the result envelopes differ.
type mcpLoop struct {
	engine serve.SessionEngine
}

func (l *mcpLoop) run(in io.Reader, out io.Writer) error {
	// Bounded, reap-safe framing shared with serveLoop.run (see readRPCLine): an
	// over-cap or malformed line fails per-request and the loop — and every open
	// session — stays alive, instead of the old 16MB bufio.Scanner whose ErrTooLong
	// unwound to runMCP's defer engine.Close() and reaped all sessions.
	reader := bufio.NewReader(in)
	enc := json.NewEncoder(out)
	for {
		line, tooLong, err := readRPCLine(reader)
		if tooLong {
			if encErr := enc.Encode(oversizedLineResponse()); encErr != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to write response: %v", encErr)
			}
		} else if trimmed := strings.TrimSpace(string(line)); trimmed != "" {
			resp := l.handleLine([]byte(trimmed))
			if resp != nil {
				if encErr := enc.Encode(resp); encErr != nil {
					return NewCLIErrorf(ExitUnexpected, "failed to write response: %v", encErr)
				}
			}
		}
		if err != nil {
			if err == io.EOF {
				return nil
			}
			return NewCLIErrorf(ExitUnexpected, "failed to read input: %v", err)
		}
	}
}

func (l *mcpLoop) handleLine(line []byte) *rpcResponse {
	req, invalid := parseRPCRequest(line)
	if invalid != nil {
		return invalid
	}

	result, rerr := l.dispatch(req.Method, req.Params)
	if rerr != nil {
		// A request with no id is a notification; it gets no response even on error.
		if len(req.ID) == 0 {
			return nil
		}
		return &rpcResponse{JSONRPC: "2.0", ID: req.ID, Error: rerr}
	}
	if len(req.ID) == 0 {
		return nil
	}
	return &rpcResponse{JSONRPC: "2.0", ID: req.ID, Result: result}
}

// dispatch routes an MCP method. Protocol-level failures (unknown method, bad
// params) return an *rpcError (the JSON-RPC error channel). Recoverable tool/
// resource failures are carried INSIDE a successful result as CallToolResult
// {isError:true} so the agent is educated without killing the stdio loop.
func (l *mcpLoop) dispatch(method string, params json.RawMessage) (interface{}, *rpcError) {
	switch method {
	case "initialize":
		return l.handleInitialize(params)
	case "tools/list":
		return l.handleToolsList()
	case "tools/call":
		return l.handleToolsCall(params)
	case "resources/list":
		return l.handleResourcesList()
	case "resources/templates/list":
		return l.handleResourceTemplatesList()
	case "resources/read":
		return l.handleResourcesRead(params)
	case "ping":
		return map[string]interface{}{}, nil
	default:
		return nil, &rpcError{
			Code:    rpcMethodNotFound,
			Message: "method not found",
			Data: &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs,
				Message: fmt.Sprintf("unknown method: %q", method),
			},
		}
	}
}

// --- initialize handshake ---

type mcpInitializeParams struct {
	ProtocolVersion string                 `json:"protocolVersion,omitempty"`
	Capabilities    map[string]interface{} `json:"capabilities,omitempty"`
	ClientInfo      map[string]interface{} `json:"clientInfo,omitempty"`
	Meta            map[string]interface{} `json:"_meta,omitempty"`
}

type mcpServerInfo struct {
	Name    string `json:"name"`
	Version string `json:"version"`
}

// mcpCapabilities is the declared server capabilities. Empty objects signal that
// the server supports the tools and resources method families.
type mcpCapabilities struct {
	Tools     map[string]interface{} `json:"tools"`
	Resources map[string]interface{} `json:"resources"`
}

type mcpInitializeResult struct {
	ProtocolVersion string          `json:"protocolVersion"`
	Capabilities    mcpCapabilities `json:"capabilities"`
	ServerInfo      mcpServerInfo   `json:"serverInfo"`
}

func (l *mcpLoop) handleInitialize(params json.RawMessage) (interface{}, *rpcError) {
	var p mcpInitializeParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	version := mcpProtocolVersion
	if mcpSupportedProtocolVersions[p.ProtocolVersion] {
		// Echo a recognized requested revision; otherwise fall back to our default
		// so we never advertise a version we do not implement.
		version = p.ProtocolVersion
	}
	return mcpInitializeResult{
		ProtocolVersion: version,
		Capabilities: mcpCapabilities{
			Tools:     map[string]interface{}{},
			Resources: map[string]interface{}{},
		},
		ServerInfo: mcpServerInfo{Name: mcpServerName, Version: Version},
	}, nil
}

// --- tools/list ---

type mcpTool struct {
	Name        string          `json:"name"`
	Description string          `json:"description"`
	InputSchema json.RawMessage `json:"inputSchema"`
}

type mcpToolsListResult struct {
	Tools []mcpTool `json:"tools"`
}

func (l *mcpLoop) handleToolsList() (interface{}, *rpcError) {
	return mcpToolsListResult{Tools: mcpTools()}, nil
}

// --- tools/call ---

type mcpToolsCallParams struct {
	Name      string          `json:"name"`
	Arguments json.RawMessage `json:"arguments,omitempty"`
}

// mcpContent is one content block of a CallToolResult.
type mcpContent struct {
	Type string `json:"type"`
	Text string `json:"text"`
}

// mcpCallToolResult is the MCP tools/call result envelope. A recoverable failure
// is signaled with IsError=true and the structured error living in
// StructuredContent; it is still a SUCCESSFUL JSON-RPC result.
type mcpCallToolResult struct {
	Content           []mcpContent    `json:"content"`
	StructuredContent json.RawMessage `json:"structuredContent,omitempty"`
	IsError           bool            `json:"isError,omitempty"`
}

func (l *mcpLoop) handleToolsCall(params json.RawMessage) (interface{}, *rpcError) {
	var p mcpToolsCallParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Name == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("tools/call requires \"name\""))
	}

	switch p.Name {
	case "open":
		return l.callOpen(p.Arguments)
	case "op":
		return l.callOp(p.Arguments)
	case "inspect":
		return l.callInspect(p.Arguments)
	case "validate":
		return l.callValidate(p.Arguments)
	case "plan":
		return l.callPlan(p.Arguments)
	case "commit":
		return l.callCommit(p.Arguments)
	case "abort":
		return l.callAbort(p.Arguments)
	default:
		// Unknown tool: recoverable isError result carrying the valid tool list so
		// the agent can correct itself, not a protocol error that ends the call.
		return toolError(&ErrorBody{
			Code:     "invalid_args",
			ExitCode: ExitInvalidArgs,
			Message:  fmt.Sprintf("unknown tool: %q", p.Name),
		}, map[string]interface{}{
			"available_tools": mcpToolNames(),
			"fix_hint":        "call one of the available_tools; see tools/list for schemas",
		}, nil), nil
	}
}

func (l *mcpLoop) callOpen(args json.RawMessage) (interface{}, *rpcError) {
	var p openRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.File == "" {
		return toolErrorFromCLI(InvalidArgsError("open requires \"file\""), nil), nil
	}
	res, err := l.engine.Open(serve.OpenParams{
		Path:       p.File,
		Out:        p.Out,
		InPlace:    p.InPlace,
		Backup:     p.Backup,
		NoValidate: p.NoValidate,
		DryRun:     p.DryRun,
	})
	if err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	next := []string{
		fmt.Sprintf("call op/inspect/validate with session=%q (thread this sessionId through every subsequent call)", res.SessionID),
		"call commit to write the output, or abort to discard the working copy",
	}
	return toolSuccess(res, next), nil
}

func (l *mcpLoop) callOp(args json.RawMessage) (interface{}, *rpcError) {
	var p opRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" || p.Command == "" {
		return toolErrorFromCLI(InvalidArgsError("op requires \"session\" and \"command\""), nil), nil
	}
	op := apply.Operation{Command: apply.NormalizeCommand(p.Command), Args: p.Args}
	// Validate the op shape through the single canonical validator (apply.ParseOps),
	// surfaced as a recoverable isError result rather than a protocol error.
	if err := validateOperationShape(op); err != nil {
		command := apply.NormalizeCommand(p.Command)
		return toolErrorFromCLIWithNextActions(err, map[string]interface{}{
			"fix_hint": "see resource://command/{path} for this command's argument schema",
		}, []string{
			"read resource://command/" + url.PathEscape(command) + " for accepted args and examples",
			"remove session-owned flags such as out, dryRun, noValidate, help, or output from op args",
			"retry the op with corrected args, or call abort if the session should be discarded",
		}), nil
	}
	ao, err := l.engine.Op(p.Session, op)
	if err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	return toolSuccess(ao, opNextActions(p.Session)), nil
}

func (l *mcpLoop) callInspect(args json.RawMessage) (interface{}, *rpcError) {
	var p opRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" || p.Command == "" {
		return toolErrorFromCLI(InvalidArgsError("inspect requires \"session\" and \"command\""), nil), nil
	}
	raw, err := l.engine.Inspect(p.Session, apply.NormalizeCommand(p.Command), p.Args)
	if err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	return toolSuccess(raw, nil), nil
}

func (l *mcpLoop) callValidate(args json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return toolErrorFromCLI(InvalidArgsError("validate requires \"session\""), nil), nil
	}
	diags, err := l.engine.Validate(p.Session)
	if err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	return toolSuccess(map[string]interface{}{"diagnostics": diagnosticsJSON(diags)}, nil), nil
}

func (l *mcpLoop) callPlan(args json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return toolErrorFromCLI(InvalidArgsError("plan requires \"session\""), nil), nil
	}
	plan, err := l.engine.Plan(p.Session)
	if err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	return toolSuccess(plan, nil), nil
}

func (l *mcpLoop) callCommit(args json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return toolErrorFromCLI(InvalidArgsError("commit requires \"session\""), nil), nil
	}
	res, err := l.engine.Commit(p.Session)
	if err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	var next []string
	if res.ValidateCommand != "" {
		next = append(next, "verify the output: "+res.ValidateCommand)
	}
	return toolSuccess(res, next), nil
}

func (l *mcpLoop) callAbort(args json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeToolArgs(args, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return toolErrorFromCLI(InvalidArgsError("abort requires \"session\""), nil), nil
	}
	if err := l.engine.Abort(p.Session); err != nil {
		return toolErrorFromEngine(err, false), nil
	}
	return toolSuccess(map[string]interface{}{"aborted": true}, nil), nil
}

// opNextActions are the in-session follow-up hints emitted after a successful op.
func opNextActions(session string) []string {
	return []string{
		fmt.Sprintf("call inspect with session=%q to confirm the change against the working copy", session),
		fmt.Sprintf("call validate with session=%q before committing", session),
		fmt.Sprintf("call commit with session=%q to write the output", session),
	}
}

// --- decode + op-shape helpers ---

// decodeToolArgs decodes a tools/call arguments object into dst. A malformed
// arguments object is a protocol-level invalid-params error.
func decodeToolArgs(args json.RawMessage, dst interface{}) *rpcError {
	if len(args) == 0 {
		return nil
	}
	dec := json.NewDecoder(strings.NewReader(string(args)))
	dec.DisallowUnknownFields()
	if err := dec.Decode(dst); err != nil {
		return &rpcError{
			Code:    rpcInvalidParams,
			Message: "invalid params",
			Data: &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: err.Error(),
			},
		}
	}
	var extra interface{}
	if err := dec.Decode(&extra); err != io.EOF {
		if err == nil {
			err = fmt.Errorf("invalid JSON: multiple values in arguments")
		}
		return &rpcError{
			Code:    rpcInvalidParams,
			Message: "invalid params",
			Data: &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: err.Error(),
			},
		}
	}
	return nil
}

// --- CallToolResult builders ---

// toolSuccess builds a non-error CallToolResult. structuredContent IS the engine
// JSON verbatim, with next_actions merged in as a SIBLING field (not a wrapper)
// when present, so an agent reads the engine fields and the hints at one level.
// The text content block mirrors the same JSON for clients that read only content.
func toolSuccess(payload interface{}, nextActions []string) mcpCallToolResult {
	data, err := json.Marshal(payload)
	if err != nil {
		return mcpCallToolResult{
			Content: []mcpContent{{Type: "text", Text: fmt.Sprintf("failed to encode result: %v", err)}},
			IsError: true,
		}
	}
	if len(nextActions) > 0 {
		if merged, ok := mergeNextActions(data, nextActions); ok {
			data = merged
		}
	}
	return mcpCallToolResult{
		Content:           []mcpContent{{Type: "text", Text: string(data)}},
		StructuredContent: data,
	}
}

// mergeNextActions adds next_actions as a sibling key inside a JSON object body.
// It returns (body, false) unchanged if the body is not a JSON object (the engine
// payloads here always marshal to objects, so the fallback is defensive only).
func mergeNextActions(body json.RawMessage, nextActions []string) (json.RawMessage, bool) {
	var obj map[string]json.RawMessage
	if err := json.Unmarshal(body, &obj); err != nil || obj == nil {
		return body, false
	}
	na, err := json.Marshal(nextActions)
	if err != nil {
		return body, false
	}
	obj["next_actions"] = na
	merged, err := json.Marshal(obj)
	if err != nil {
		return body, false
	}
	return merged, true
}

// mcpToolError is the structured error body carried in a failing CallToolResult.
type mcpToolError struct {
	Type        string                 `json:"type"`
	Message     string                 `json:"message"`
	Recoverable bool                   `json:"recoverable"`
	Data        map[string]interface{} `json:"data,omitempty"`
}

// toolError builds a CallToolResult{isError:true} carrying a structured,
// machine-parseable error plus optional extra data and next_actions.
func toolError(body *ErrorBody, extra map[string]interface{}, nextActions []string) mcpCallToolResult {
	data := map[string]interface{}{
		"exitCode": body.ExitCode,
	}
	if len(body.Diagnostics) > 0 {
		data["diagnostics"] = body.Diagnostics
	}
	for k, v := range extra {
		data[k] = v
	}
	if len(nextActions) > 0 {
		data["next_actions"] = nextActions
	}
	te := mcpToolError{
		Type:        body.Code,
		Message:     body.Message,
		Recoverable: recoverableForExit(body.ExitCode),
		Data:        data,
	}
	envelope := map[string]interface{}{"error": te}
	encoded, err := json.Marshal(envelope)
	if err != nil {
		encoded = json.RawMessage(fmt.Sprintf(`{"error":{"type":"unexpected","message":%q,"recoverable":false}}`, err.Error()))
	}
	return mcpCallToolResult{
		Content:           []mcpContent{{Type: "text", Text: te.Message}},
		StructuredContent: encoded,
		IsError:           true,
	}
}

// toolErrorFromCLI maps a CLIError into a recoverable isError CallToolResult.
func toolErrorFromCLI(err *CLIError, extra map[string]interface{}) mcpCallToolResult {
	return toolErrorFromCLIWithNextActions(err, extra, nil)
}

func toolErrorFromCLIWithNextActions(err *CLIError, extra map[string]interface{}, nextActions []string) mcpCallToolResult {
	code := err.Code
	if code == "" {
		code = codeForExit(err.ExitCode)
	}
	body := &ErrorBody{
		Code:        code,
		ExitCode:    err.ExitCode,
		Message:     err.Message,
		Diagnostics: err.Diagnostics,
	}
	return toolError(body, extra, nextActions)
}

// toolErrorFromEngine maps engine/apply typed errors into the structured isError
// CallToolResult, RE-PARSING an OpError's child --json stderr back into the
// ErrorBody envelope so the fail-helpfully layer (selector candidates, handle
// codes, exit code, diagnostics) survives instead of collapsing to opaque text.
// toolErrorFromEngine maps an engine error to a structured MCP tool error.
// isInspect tailors recovery guidance for the read-only inspect tool, which must
// not borrow the op tool's "retry the op ... or call abort" phrasing.
func toolErrorFromEngine(err error, isInspect bool) mcpCallToolResult {
	switch e := err.(type) {
	case *serve.MultiSourceError:
		return toolError(&ErrorBody{
			Code:     codeForExit(ExitUnsupportedType),
			ExitCode: ExitUnsupportedType,
			Message:  e.Error(),
		}, map[string]interface{}{
			"reason":   "multi-source ops need a second package the working-copy engine cannot stage",
			"fix_hint": "use the `ooxml apply` CLI for clone/import/merge across two packages",
		}, nil)
	case *serve.AddressPositionalHandleAfterShiftError:
		return toolError(&ErrorBody{
			Code:     codeForExit(ExitInvalidArgs),
			ExitCode: ExitInvalidArgs,
			Message:  e.Error(),
		}, map[string]interface{}{
			"reason":   "an address-positional XLSX cell/comment handle was minted before a row/column structural edit in this session",
			"fix_hint": "re-run inspect/find against the current session state, then retry with the fresh handle or explicit sheet/cell coordinates",
		}, []string{
			"call inspect or find on the same session to rediscover the target after the row/column edit",
			"retry the op with a fresh handle, or pass explicit --sheet/--cell style args when the intended address is known",
		})
	case *serve.ReadCommandDeniedError:
		return toolError(&ErrorBody{
			Code:     codeForExit(ExitInvalidArgs),
			ExitCode: ExitInvalidArgs,
			Message:  e.Error(),
		}, map[string]interface{}{
			"reason":   "inspect only runs read-only, artifact-free commands",
			"fix_hint": "use the op tool for mutations; remove output/artifact flags from inspect reads",
		}, []string{
			"use the op tool for mutations, then inspect the same session after the op",
			"for external diff/render/verify workflows, commit first and run the CLI on real output paths",
			"read resource://capabilities or resource://command/{path} to choose a read-only inspect command",
		})
	case *serve.SessionNotFoundError:
		return toolError(&ErrorBody{
			Code:     codeForExit(ExitTargetNotFound),
			ExitCode: ExitTargetNotFound,
			Message:  e.Error(),
		}, map[string]interface{}{
			"fix_hint": "open a new session: call the open tool, then thread the returned sessionId",
		}, []string{"call the open tool to start a fresh session"})
	case *apply.OpError:
		body, extra := opErrorBody(e)
		if isInspect {
			return toolError(body, extra, nextActionsForInspectError(body, e))
		}
		return toolError(body, extra, nextActionsForOpError(body, e))
	case *apply.ValidationError:
		return toolError(&ErrorBody{
			Code:        codeForExit(ExitValidationFailed),
			ExitCode:    ExitValidationFailed,
			Message:     e.Error(),
			Diagnostics: diagnosticsJSON(e.Diagnostics),
		}, map[string]interface{}{
			"fix_hint": "inspect the diagnostics, fix the offending op, and re-validate before commit",
		}, nil)
	default:
		// Heuristic mapping for engine string errors that mirror CLI conditions.
		msg := err.Error()
		switch {
		case strings.HasPrefix(msg, "file not found"):
			return toolError(&ErrorBody{Code: codeForExit(ExitFileNotFound), ExitCode: ExitFileNotFound, Message: msg}, nil, nil)
		case strings.HasPrefix(msg, "unsupported type"):
			return toolError(&ErrorBody{Code: codeForExit(ExitUnsupportedType), ExitCode: ExitUnsupportedType, Message: msg}, nil, nil)
		case strings.Contains(msg, "dry-run") || strings.Contains(msg, "in-place") ||
			strings.Contains(msg, "backup") || strings.Contains(msg, "must specify") ||
			strings.Contains(msg, "committed") || strings.Contains(msg, "aborted"):
			return toolError(&ErrorBody{Code: codeForExit(ExitInvalidArgs), ExitCode: ExitInvalidArgs, Message: msg}, nil, nil)
		default:
			return toolError(&ErrorBody{Code: codeForExit(ExitUnexpected), ExitCode: ExitUnexpected, Message: msg}, nil, nil)
		}
	}
}

// opErrorBody re-parses an OpError's child stderr (the failing subprocess was
// invoked with --json, so its stderr is the CLI's {error:{...}} envelope) back
// into a structured ErrorBody. When the stderr is NOT a parseable envelope (e.g.
// a bogus subcommand printing plain cobra usage), it falls back to invalid_args
// carrying the raw stderr as a diagnostic.
func opErrorBody(e *apply.OpError) (*ErrorBody, map[string]interface{}) {
	extra := map[string]interface{}{
		"failedOpIndex": e.FailedOpIndex,
		"command":       e.Command,
	}
	var env ErrorEnvelope
	if err := json.Unmarshal([]byte(strings.TrimSpace(e.Stderr)), &env); err == nil && env.Error.Code != "" {
		body := env.Error
		// The CLI's selector "did you mean" candidates live inside the message;
		// surface the discovery hint so the agent knows it can recover.
		if strings.Contains(body.Message, "did you mean") {
			extra["fix_hint"] = "re-run inspect to discover valid selectors, then retry the op"
		}
		return &body, extra
	}
	// Not a JSON envelope: synthesize an invalid_args error from the raw stderr.
	return &ErrorBody{
		Code:     codeForExit(ExitInvalidArgs),
		ExitCode: ExitInvalidArgs,
		Message:  e.Error(),
		Diagnostics: []DiagnosticJSON{{
			Code:     "op_failed",
			Severity: "error",
			Message:  fmt.Sprintf("op %d (%s); child stderr: %s", e.FailedOpIndex, e.Command, strings.TrimSpace(e.Stderr)),
		}},
	}, extra
}

func nextActionsForOpError(body *ErrorBody, e *apply.OpError) []string {
	if body == nil || !recoverableForExit(body.ExitCode) {
		return nil
	}
	command := apply.NormalizeCommand(e.Command)
	if command == "" {
		command = e.Command
	}
	resource := "resource://command/" + url.PathEscape(command)
	return []string{
		"call inspect on the same session to list valid targets/selectors before retrying",
		"read " + resource + " for the command's accepted args, examples, and common errors",
		"retry the op with corrected args, or call abort if the session should be discarded",
	}
}

// nextActionsForInspectError mirrors nextActionsForOpError for the read-only
// inspect tool: it drops the op/abort phrasing (inspect mutates nothing and has
// no session to abort) and frames recovery around re-running inspect.
func nextActionsForInspectError(body *ErrorBody, e *apply.OpError) []string {
	if body == nil || !recoverableForExit(body.ExitCode) {
		return nil
	}
	command := apply.NormalizeCommand(e.Command)
	if command == "" {
		command = e.Command
	}
	resource := "resource://command/" + url.PathEscape(command)
	return []string{
		"adjust the inspect args (e.g. selector/target) and re-run inspect",
		"re-run inspect to discover valid targets/selectors for this command",
		"read " + resource + " for the command's accepted args and examples",
	}
}

// recoverableForExit derives whether an agent should retry a failure from its
// exit code: argument/lookup/validation failures are recoverable; an unexpected
// crash or an unsupported file type is not.
func recoverableForExit(exit int) bool {
	switch exit {
	case ExitInvalidArgs, ExitFileNotFound, ExitValidationFailed,
		ExitTargetNotFound, ExitDiffThreshold, ExitPartialSuccess:
		return true
	default:
		return false
	}
}

// --- resources ---

type mcpResource struct {
	URI         string `json:"uri"`
	Name        string `json:"name"`
	Description string `json:"description"`
	MIMEType    string `json:"mimeType"`
}

type mcpResourcesListResult struct {
	Resources []mcpResource `json:"resources"`
}

func (l *mcpLoop) handleResourcesList() (interface{}, *rpcError) {
	return mcpResourcesListResult{Resources: []mcpResource{
		{
			URI:         "resource://capabilities",
			Name:        "capabilities",
			Description: "The full machine-readable CLI contract: the command inventory, per-command flags, object kinds, exit codes, workflows, and the stable-handle grammar. This is the menu of valid command strings for the generic op/inspect tools.",
			MIMEType:    "application/json",
		},
		{
			URI:         "resource://agent-guide",
			Name:        "agent-guide",
			Description: "A compact, paste-ready guide for agent workflows across PPTX, XLSX, VBA, and DOCX. Same content as `ooxml agent guide --json`.",
			MIMEType:    "application/json",
		},
	}}, nil
}

type mcpResourceTemplate struct {
	URITemplate string `json:"uriTemplate"`
	Name        string `json:"name"`
	Description string `json:"description"`
	MIMEType    string `json:"mimeType"`
}

type mcpResourceTemplatesListResult struct {
	ResourceTemplates []mcpResourceTemplate `json:"resourceTemplates"`
}

func (l *mcpLoop) handleResourceTemplatesList() (interface{}, *rpcError) {
	return mcpResourceTemplatesListResult{ResourceTemplates: []mcpResourceTemplate{
		{
			URITemplate: "resource://command/{path}",
			Name:        "command",
			Description: "One command's flag schema, examples, common errors, and target object kinds. The path is the URL-encoded op-vocabulary command string (e.g. resource://command/xlsx%20cells%20set). Read the concrete URI to learn the args object to pass to the generic op/inspect tools for that command.",
			MIMEType:    "application/json",
		},
	}}, nil
}

type mcpResourcesReadParams struct {
	URI string `json:"uri"`
}

type mcpResourceContents struct {
	URI      string `json:"uri"`
	MIMEType string `json:"mimeType"`
	Text     string `json:"text"`
}

type mcpResourcesReadResult struct {
	Contents []mcpResourceContents `json:"contents"`
}

func (l *mcpLoop) handleResourcesRead(params json.RawMessage) (interface{}, *rpcError) {
	var p mcpResourcesReadParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.URI == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("resources/read requires \"uri\""))
	}

	switch {
	case p.URI == "resource://capabilities":
		return resourceContents(p.URI, buildCapabilitiesDocument())
	case p.URI == "resource://agent-guide":
		return resourceContents(p.URI, buildRobotDocsGuide())
	case strings.HasPrefix(p.URI, "resource://command/"):
		return l.readCommandResource(p.URI)
	default:
		return nil, rpcErrorFromCLI(TargetNotFoundError(fmt.Sprintf("unknown resource: %q", p.URI)))
	}
}

// readCommandResource resolves resource://command/{path} where {path} is either
// the op-vocabulary command string ("xlsx cells set") or the full capabilities
// command path ("ooxml xlsx cells set"), URL-encoded.
func (l *mcpLoop) readCommandResource(uri string) (interface{}, *rpcError) {
	encoded := strings.TrimPrefix(uri, "resource://command/")
	decoded, err := url.PathUnescape(encoded)
	if err != nil {
		return nil, rpcErrorFromCLI(InvalidArgsError(fmt.Sprintf("invalid command path in uri: %v", err)))
	}
	decoded = strings.TrimSpace(decoded)
	if decoded == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("resource://command/{path} requires a command path"))
	}
	normalized := apply.NormalizeCommand(decoded)
	wantPath := "ooxml " + normalized
	doc := buildCapabilitiesDocument()
	for _, c := range doc.Commands {
		if c.Path == wantPath {
			return resourceContents(uri, c)
		}
	}
	return nil, rpcErrorFromCLI(TargetNotFoundError(fmt.Sprintf("unknown command: %q; discover valid commands via resource://capabilities", decoded)))
}

// resourceContents marshals a resource body into the MCP resources/read envelope.
func resourceContents(uri string, body interface{}) (interface{}, *rpcError) {
	data, err := json.Marshal(body)
	if err != nil {
		return nil, rpcErrorFromCLI(NewCLIErrorf(ExitUnexpected, "failed to encode resource: %v", err))
	}
	return mcpResourcesReadResult{Contents: []mcpResourceContents{{
		URI:      uri,
		MIMEType: "application/json",
		Text:     string(data),
	}}}, nil
}

// --- tool catalog ---

// mcpToolNames returns the names of the generic session tools.
func mcpToolNames() []string {
	tools := mcpTools()
	names := make([]string, 0, len(tools))
	for _, t := range tools {
		names = append(names, t.Name)
	}
	sort.Strings(names)
	return names
}

// mcpTools returns the seven generic SESSION tools with JSON-Schema input schemas
// and skill-quality documentation. They surface the serve.SessionEngine 1:1: the
// generic op covers opCompatible mutation commands and generic inspect covers
// session-safe read commands; command/args follow the capabilities contract.
func mcpTools() []mcpTool {
	return []mcpTool{
		{
			Name: "open",
			Description: `Open a working copy of an OOXML file and start a session. Returns a sessionId you thread through every subsequent op/inspect/validate/commit/abort call.

Discovery
- file: an absolute or relative path to a .pptx/.pptm/.xlsx/.xlsm/.docx file.
- which package types are openable: read resource://capabilities (packageTypes).

When to use
- Always first. The original file is NEVER modified until commit; abort discards the working copy.
- NOT for one-off reads of a file you will not mutate — but it is still the entry point for inspect within a session.

Do / Don't
- Do choose exactly one output target: out (a new path), inPlace (overwrite the original), or dryRun (validate only, write nothing).
- Do save the returned sessionId — it is the durable handle for this working copy.
- Don't combine out with inPlace, or use backup without inPlace, or combine dryRun with out/inPlace.

Example
{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"open","arguments":{"file":"deck.pptx","out":"edited.pptx"}}}

Common mistakes
- Missing output target: you must set out, inPlace:true, or dryRun:true.
- Forgetting to thread sessionId: every later call needs session=<the returned sessionId>.`,
			InputSchema: schemaOpen(),
		},
		{
			Name: "op",
			Description: `Apply ONE mutation operation to the session's working copy. The generic command/args shape reaches commands whose capabilities entry has opCompatible=true; the working copy advances only on success, and a failed op leaves the session usable at its last-good state.

Discovery
- session: the sessionId returned by open.
- command: a command string from the inventory in resource://capabilities with opCompatible=true (e.g. "xlsx cells set", "pptx replace text"). Use the op vocabulary WITHOUT the leading "ooxml ".
- args: the named flags for that command, as a JSON object. Prefer each resource://command/{path} localFlags[].argName value (dashless); legacy localFlags[].name values with leading "--" are accepted and normalized too.

When to use
- To change the file (set a cell, replace text, update a chart, add a name, ...).
- NOT for reads — use inspect. NOT for multi-source clone/import/merge across two packages — those are unsupported in a session; use the apply CLI.

Do / Don't
- Do pass values as JSON (args:{"sheet":"1","cell":"B2","value":"=SUM(A1:A10)"}); never shell-quote.
- Do inspect/validate between ops to confirm the evolving state before commit.
- Don't reuse an XLSX address-positional cell/comment handle after a prior row/column insert/delete in the same session; inspect/find again to get a fresh handle, or use explicit sheet/cell coordinates.

Example
{"jsonrpc":"2.0","id":"2","method":"tools/call","params":{"name":"op","arguments":{"session":"s1","command":"xlsx cells set","args":{"sheet":"1","cell":"A1","value":"hello"}}}}

Common mistakes
- target_not_found: the selector did not match; re-run inspect to discover valid selectors, then retry.
- invalid_args: a flag name/value was wrong; check resource://command/{path}.`,
			InputSchema: schemaOp(),
		},
		{
			Name: "inspect",
			Description: `Run ONE read-only, artifact-free command against the session's working copy and return its JSON verbatim. It reflects every op applied so far. Commands that need external packages or write artifacts (render, diff, extract, raw --output paths) should be run through the CLI after commit.

Discovery
- session: the sessionId returned by open.
- command: a read command string from resource://capabilities (e.g. "inspect", "xlsx ranges export", "pptx slides show"). Use the op vocabulary WITHOUT the leading "ooxml ".
- args: named flags as a JSON object; prefer localFlags[].argName from resource://command/{path}. Leading "--" flag names are accepted and normalized.

When to use
- To confirm a mutation took effect, or to discover selectors/values before an op.
- NOT to change the file — use op.

Do / Don't
- Do call inspect after an op to verify before committing.
- Do use inspect to discover valid selectors when an op returns target_not_found.
- Don't pass a stale sessionId — open a new session if it was committed/aborted.

Example
{"jsonrpc":"2.0","id":"3","method":"tools/call","params":{"name":"inspect","arguments":{"session":"s1","command":"xlsx ranges export","args":{"sheet":"1","range":"A1:B2"}}}}

Common mistakes
- Using op for a read: inspect is the read door.
- Wrong command vocabulary: drop the leading "ooxml " (use "xlsx sheets list", not "ooxml xlsx sheets list").`,
			InputSchema: schemaInspect(),
		},
		{
			Name: "validate",
			Description: `Validate the session's current working copy and return its diagnostics (does not commit).

Discovery
- session: the sessionId returned by open.

When to use
- After a batch of ops, before commit, to confirm the package is still well-formed.
- NOT to write the file — use commit (which validates by default).

Do / Don't
- Do treat any error-severity diagnostic as a blocker; fix the offending op and re-validate.
- Don't skip validation on a file you will hand to a user.

Example
{"jsonrpc":"2.0","id":"4","method":"tools/call","params":{"name":"validate","arguments":{"session":"s1"}}}

Common mistakes
- Expecting validate to write output: it only reports; commit writes.`,
			InputSchema: schemaSession("Validate the current working copy of this session."),
		},
		{
			Name: "plan",
			Description: `Return the would-apply plan for the ops buffered in the session, without committing — the exact subprocess argv each op resolves to.

Discovery
- session: the sessionId returned by open.

When to use
- To preview/audit the op sequence before commit.
- NOT to change or validate the file.

Do / Don't
- Do use plan to confirm the ordered ops match your intent.
- Don't expect a readback of effects — plan shows intended argv, not results (use inspect for effects).

Example
{"jsonrpc":"2.0","id":"5","method":"tools/call","params":{"name":"plan","arguments":{"session":"s1"}}}`,
			InputSchema: schemaSession("Return the buffered op plan for this session."),
		},
		{
			Name: "commit",
			Description: `Validate (unless the session opened with noValidate) and atomically write the working copy to the session's output target. After commit the session is consumed.

Discovery
- session: the sessionId returned by open. The output target (out/inPlace/dryRun) was fixed at open time.

When to use
- Once you are done applying ops and validation is clean.
- NOT to keep editing afterward — open a new session for further changes.

Do / Don't
- Do run validate (or rely on commit's validate-by-default) before publishing.
- Do verify the committed file with the returned validateCommand.
- Don't expect the session to remain usable after commit — it is dropped.

Example
{"jsonrpc":"2.0","id":"6","method":"tools/call","params":{"name":"commit","arguments":{"session":"s1"}}}

Common mistakes
- Committing a session whose validation failed: the session stays open and re-committable; fix the op first.`,
			InputSchema: schemaSession("Commit (atomically write) the working copy of this session."),
		},
		{
			Name: "abort",
			Description: `Discard the session's working copy. Nothing is written and the original file is untouched. The session is consumed.

Discovery
- session: the sessionId returned by open.

When to use
- To throw away edits without writing.
- NOT to undo a single op — abort discards the whole session; open again to start over.

Do / Don't
- Do abort to cleanly release a working copy you no longer need.
- Don't reuse the sessionId afterward — it is dropped.

Example
{"jsonrpc":"2.0","id":"7","method":"tools/call","params":{"name":"abort","arguments":{"session":"s1"}}}`,
			InputSchema: schemaSession("Discard the working copy of this session."),
		},
	}
}

// --- JSON-Schema builders (raw, stable field ordering) ---

func mustSchema(v interface{}) json.RawMessage {
	data, err := json.Marshal(v)
	if err != nil {
		// The schema literals are static; a marshal failure is a programming error.
		panic(fmt.Sprintf("failed to marshal tool input schema: %v", err))
	}
	return data
}

func schemaOpen() json.RawMessage {
	return mustSchema(map[string]interface{}{
		"type": "object",
		"properties": map[string]interface{}{
			"file":       prop("string", "Path to the OOXML file to open (.pptx/.pptm/.xlsx/.xlsm/.docx). The original is never modified until commit."),
			"out":        prop("string", "Commit target path for a new file. Mutually exclusive with inPlace and dryRun."),
			"inPlace":    prop("boolean", "Commit back over the original file. Mutually exclusive with out and dryRun."),
			"backup":     prop("string", "When set with inPlace, the path the original is copied to before the in-place commit."),
			"noValidate": prop("boolean", "Skip validate-by-default on commit."),
			"dryRun":     prop("boolean", "Accept ops/inspect/validate but write nothing on commit. Mutually exclusive with out and inPlace."),
		},
		"required":             []string{"file"},
		"additionalProperties": false,
	})
}

func schemaOp() json.RawMessage {
	return mustSchema(map[string]interface{}{
		"type": "object",
		"properties": map[string]interface{}{
			"session": prop("string", "The sessionId returned by open."),
			"command": prop("string", "A mutation command string from resource://capabilities, in op vocabulary without the leading \"ooxml \" (e.g. \"xlsx cells set\")."),
			"args": map[string]interface{}{
				"type":        "object",
				"description": "Named flags for the command as a JSON object (no leading \"--\"; values as JSON scalars). See resource://command/{path} for the exact schema.",
			},
		},
		"required":             []string{"session", "command"},
		"additionalProperties": false,
	})
}

func schemaInspect() json.RawMessage {
	return mustSchema(map[string]interface{}{
		"type": "object",
		"properties": map[string]interface{}{
			"session": prop("string", "The sessionId returned by open."),
			"command": prop("string", "A read-only command string from resource://capabilities, in op vocabulary without the leading \"ooxml \" (e.g. \"xlsx ranges export\")."),
			"args": map[string]interface{}{
				"type":        "object",
				"description": "Named flags for the read command as a JSON object. See resource://command/{path} for the exact schema.",
			},
		},
		"required":             []string{"session", "command"},
		"additionalProperties": false,
	})
}

func schemaSession(desc string) json.RawMessage {
	return mustSchema(map[string]interface{}{
		"type":        "object",
		"description": desc,
		"properties": map[string]interface{}{
			"session": prop("string", "The sessionId returned by open."),
		},
		"required":             []string{"session"},
		"additionalProperties": false,
	})
}

func prop(typ, desc string) map[string]interface{} {
	return map[string]interface{}{"type": typ, "description": desc}
}

func init() {
	// mcp has no local flags (mirrors serve), so nothing to add to resetFlags().
	rootCmd.AddCommand(mcpCmd)
}
