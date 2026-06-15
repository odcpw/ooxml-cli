package cli

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/serve"
)

// serveSelfExecutable resolves the path to the ooxml binary the serve engine
// re-dispatches each op against as a subprocess. It is a package var so tests can
// inject a freshly built binary (the in-process test binary cannot dispatch ooxml
// subcommands), mirroring findSelfExecutable.
var serveSelfExecutable = os.Executable

// JSON-RPC application error codes. These stay clear of the reserved
// -32768..-32000 range and of the 0-9 exit codes; the authoritative exit code
// lives in error.data.exitCode (the existing ErrorBody contract).
const (
	rpcParseError     = -32700
	rpcInvalidRequest = -32600
	rpcMethodNotFound = -32601
	rpcInvalidParams  = -32602
	rpcInternalError  = -32603
)

// rpcRequest is a single JSON-RPC 2.0 request line.
type rpcRequest struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Method  string          `json:"method"`
	Params  json.RawMessage `json:"params,omitempty"`
}

// rpcResponse is a single JSON-RPC 2.0 response line.
type rpcResponse struct {
	JSONRPC string          `json:"jsonrpc"`
	ID      json.RawMessage `json:"id,omitempty"`
	Result  interface{}     `json:"result,omitempty"`
	Error   *rpcError       `json:"error,omitempty"`
}

// rpcError carries the JSON-RPC application code/message plus the existing
// ErrorBody as data so the exit-code contract survives on the wire.
type rpcError struct {
	Code    int        `json:"code"`
	Message string     `json:"message"`
	Data    *ErrorBody `json:"data,omitempty"`
}

var serveCmd = &cobra.Command{
	Use:   "serve",
	Short: "Run a long-lived session engine over newline-delimited JSON-RPC 2.0",
	Long: `Run a long-lived OOXML session engine speaking newline-delimited JSON-RPC 2.0
over stdin/stdout (one compact JSON object per line; stdout is protocol-only,
stderr is human/diagnostic logging).

A session opens a working copy of an input file, applies mutation ops against it
one at a time (each dispatched exactly like 'ooxml apply' for clean isolation),
lets you inspect/validate the working copy between ops, and commits atomically.
The original file is never touched until commit.

Methods: initialize, open, op, inspect, validate, plan, commit, abort,
capabilities. Errors are JSON-RPC error objects whose data field is the same
{code, exitCode, message, diagnostics} envelope the CLI emits in --json mode.`,
	Args:          cobra.NoArgs,
	SilenceUsage:  true,
	SilenceErrors: true,
	RunE:          runServe,
}

func runServe(cmd *cobra.Command, args []string) error {
	self, err := serveSelfExecutable()
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to resolve own executable path: %v", err)
	}
	engine := serve.NewEngine(self, applyTempBase(cmd))
	// On graceful shutdown (stdio EOF) reap any sessions opened but never
	// committed/aborted, so their working-copy scratch dirs are not leaked.
	defer engine.Close()
	loop := &serveLoop{engine: engine}
	return loop.run(cmd.InOrStdin(), cmd.OutOrStdout())
}

// serveLoop drives the JSON-RPC request/response loop over a reader/writer pair.
type serveLoop struct {
	engine serve.SessionEngine
}

func (l *serveLoop) run(in io.Reader, out io.Writer) error {
	reader := bufio.NewReader(in)
	enc := json.NewEncoder(out)
	for {
		line, tooLong, err := readRPCLine(reader)
		if tooLong {
			// Over the per-line cap: emit a per-request parse error and keep serving
			// (readRPCLine already discarded the remainder up to the newline). Never
			// terminate the loop — that would unwind to the caller's defer Close().
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
			// io.EOF (with or without a trailing partial line) is a clean shutdown;
			// any other read error is fatal.
			if err == io.EOF {
				return nil
			}
			return NewCLIErrorf(ExitUnexpected, "failed to read input: %v", err)
		}
	}
}

// maxRPCLineBytes bounds the size of a single newline-delimited JSON-RPC request
// the serve/MCP loops will buffer. It is deliberately generous — far above an op
// embedding a base64 media payload (a ~12MB image is ~16MB base64) — so legitimate
// large ops are NOT rejected, while a pathological or unterminated line cannot
// drive unbounded memory growth. A line over the cap is reported per-request and
// the loop keeps serving (the old 16MB bufio.Scanner cap instead returned an error
// that reaped every open session). It is a var so tests can lower it without
// allocating hundreds of MB.
var maxRPCLineBytes = 256 * 1024 * 1024

// readRPCLine reads one newline-delimited line from r with bounded memory. It
// returns the line WITHOUT requiring the trailing newline. If the line exceeds
// maxRPCLineBytes, it returns tooLong=true with a nil line and CONSUMES the rest
// of the line up to the next newline (discarding it, so memory stays bounded and
// the next line still frames cleanly). err is io.EOF at end of input (possibly
// alongside a final unterminated line) or any underlying read error.
func readRPCLine(r *bufio.Reader) (line []byte, tooLong bool, err error) {
	var buf []byte
	for {
		frag, e := r.ReadSlice('\n')
		if !tooLong {
			if len(buf)+len(frag) > maxRPCLineBytes {
				tooLong = true
				buf = nil // release what we had; we will not return content
			} else {
				buf = append(buf, frag...)
			}
		}
		if e == bufio.ErrBufferFull {
			// Delimiter not yet seen; keep reading fragments of the same line.
			continue
		}
		if tooLong {
			return nil, true, e
		}
		return buf, false, e
	}
}

// oversizedLineResponse is the per-request parse error returned for a line that
// exceeds maxRPCLineBytes. The id is null because the line was never parsed.
func oversizedLineResponse() *rpcResponse {
	return errResponse(nil, rpcParseError, "parse error", &ErrorBody{
		Code:     "invalid_args",
		ExitCode: ExitInvalidArgs,
		Message:  fmt.Sprintf("request line exceeds the %d-byte limit", maxRPCLineBytes),
	})
}

func (l *serveLoop) handleLine(line []byte) *rpcResponse {
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

func parseRPCRequest(line []byte) (rpcRequest, *rpcResponse) {
	trimmed := bytes.TrimSpace(line)
	if len(trimmed) == 0 {
		return rpcRequest{}, errResponse(nil, rpcParseError, "parse error", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "empty request",
		})
	}
	if trimmed[0] == '[' {
		if json.Valid(trimmed) {
			return rpcRequest{}, errResponse(nil, rpcInvalidRequest, "invalid request", &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "batch requests are not supported; send one JSON-RPC request object per line",
			})
		}
		return rpcRequest{}, errResponse(nil, rpcParseError, "parse error", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "invalid JSON batch request",
		})
	}
	if trimmed[0] != '{' && json.Valid(trimmed) {
		return rpcRequest{}, errResponse(nil, rpcInvalidRequest, "invalid request", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "request must be a JSON object",
		})
	}

	var raw map[string]json.RawMessage
	dec := json.NewDecoder(bytes.NewReader(trimmed))
	if err := dec.Decode(&raw); err != nil {
		return rpcRequest{}, errResponse(nil, rpcParseError, "parse error", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: err.Error(),
		})
	}
	var extra any
	if err := dec.Decode(&extra); err != io.EOF {
		message := "trailing JSON value after request object"
		if err != nil {
			message = "trailing data after request object: " + err.Error()
		}
		return rpcRequest{}, errResponse(nil, rpcInvalidRequest, "invalid request", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: message,
		})
	}

	id := json.RawMessage(nil)
	if rawID, ok := raw["id"]; ok {
		if validID, valid := rpcRequestID(rawID); valid {
			id = validID
		} else {
			return rpcRequest{}, errResponse(nil, rpcInvalidRequest, "invalid request", &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "id must be a string, number, or null",
			})
		}
	}

	allowed := map[string]bool{"jsonrpc": true, "id": true, "method": true, "params": true}
	for key := range raw {
		if !allowed[key] {
			return rpcRequest{}, errResponse(id, rpcInvalidRequest, "invalid request", &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: fmt.Sprintf("unknown top-level field %q", key),
			})
		}
	}

	var version string
	if v, ok := raw["jsonrpc"]; !ok {
		return rpcRequest{}, errResponse(id, rpcInvalidRequest, "invalid request", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "missing jsonrpc",
		})
	} else if err := json.Unmarshal(v, &version); err != nil || version != "2.0" {
		return rpcRequest{}, errResponse(id, rpcInvalidRequest, "invalid request", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: `jsonrpc must be "2.0"`,
		})
	}

	var method string
	if v, ok := raw["method"]; !ok {
		return rpcRequest{}, errResponse(id, rpcInvalidRequest, "invalid request", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "missing method",
		})
	} else if err := json.Unmarshal(v, &method); err != nil || method == "" {
		return rpcRequest{}, errResponse(id, rpcInvalidRequest, "invalid request", &ErrorBody{
			Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: "method must be a non-empty string",
		})
	}

	return rpcRequest{
		JSONRPC: version,
		ID:      id,
		Method:  method,
		Params:  raw["params"],
	}, nil
}

func rpcRequestID(raw json.RawMessage) (json.RawMessage, bool) {
	trimmed := bytes.TrimSpace(raw)
	if len(trimmed) == 0 || !json.Valid(trimmed) {
		return nil, false
	}
	switch trimmed[0] {
	case '"', 'n', '-':
		return append(json.RawMessage(nil), trimmed...), true
	default:
		if trimmed[0] >= '0' && trimmed[0] <= '9' {
			return append(json.RawMessage(nil), trimmed...), true
		}
		return nil, false
	}
}

func (l *serveLoop) dispatch(method string, params json.RawMessage) (interface{}, *rpcError) {
	switch method {
	case "initialize", "capabilities":
		return l.handleInitialize()
	case "open":
		return l.handleOpen(params)
	case "op":
		return l.handleOp(params)
	case "inspect":
		return l.handleInspect(params)
	case "validate":
		return l.handleValidate(params)
	case "plan":
		return l.handlePlan(params)
	case "commit":
		return l.handleCommit(params)
	case "abort":
		return l.handleAbort(params)
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

// --- method params ---

type openRPCParams struct {
	File       string `json:"file"`
	Out        string `json:"out,omitempty"`
	InPlace    bool   `json:"inPlace,omitempty"`
	Backup     string `json:"backup,omitempty"`
	NoValidate bool   `json:"noValidate,omitempty"`
	DryRun     bool   `json:"dryRun,omitempty"`
}

type opRPCParams struct {
	Session string               `json:"session"`
	Command string               `json:"command"`
	Args    map[string]apply.Arg `json:"args,omitempty"`
}

type sessionRPCParams struct {
	Session string `json:"session"`
}

// initializeResult mirrors capabilities --json where sensible plus the serve
// method list, so a client can feature-detect without out-of-band knowledge.
type initializeResult struct {
	Server          string               `json:"server"`
	Version         string               `json:"version"`
	SchemaVersion   int                  `json:"schemaVersion"`
	OpSchemaVersion int                  `json:"opSchemaVersion"`
	Methods         []string             `json:"methods"`
	PackageTypes    []string             `json:"packageTypes"`
	MultiSession    bool                 `json:"multiSession"`
	Engine          string               `json:"engine"`
	Capabilities    capabilitiesDocument `json:"capabilities"`
}

func (l *serveLoop) handleInitialize() (interface{}, *rpcError) {
	return initializeResult{
		Server:          "ooxml-serve",
		Version:         Version,
		SchemaVersion:   serve.SchemaVersion,
		OpSchemaVersion: apply.SchemaVersion,
		Methods: []string{
			"initialize", "open", "op", "inspect",
			"validate", "plan", "commit", "abort", "capabilities",
		},
		PackageTypes: []string{"pptx", "pptm", "xlsx", "xlsm", "docx"},
		MultiSession: true,
		Engine:       "working-copy",
		Capabilities: buildCapabilitiesDocument(),
	}, nil
}

func (l *serveLoop) handleOpen(params json.RawMessage) (interface{}, *rpcError) {
	var p openRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.File == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("open requires \"file\""))
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
		return nil, rpcErrorFromEngine(err)
	}
	return res, nil
}

func (l *serveLoop) handleOp(params json.RawMessage) (interface{}, *rpcError) {
	var p opRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" || p.Command == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("op requires \"session\" and \"command\""))
	}
	op := apply.Operation{Command: apply.NormalizeCommand(p.Command), Args: p.Args}
	// Validate the op shape through the single canonical validator.
	if rerr := validateOpShape(op); rerr != nil {
		return nil, rerr
	}
	ao, err := l.engine.Op(p.Session, op)
	if err != nil {
		return nil, rpcErrorFromEngine(err)
	}
	return ao, nil
}

func (l *serveLoop) handleInspect(params json.RawMessage) (interface{}, *rpcError) {
	var p opRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" || p.Command == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("inspect requires \"session\" and \"command\""))
	}
	raw, err := l.engine.Inspect(p.Session, apply.NormalizeCommand(p.Command), p.Args)
	if err != nil {
		return nil, rpcErrorFromEngine(err)
	}
	return raw, nil
}

func (l *serveLoop) handleValidate(params json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("validate requires \"session\""))
	}
	diags, err := l.engine.Validate(p.Session)
	if err != nil {
		return nil, rpcErrorFromEngine(err)
	}
	return map[string]interface{}{"diagnostics": diagnosticsJSON(diags)}, nil
}

func (l *serveLoop) handlePlan(params json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("plan requires \"session\""))
	}
	plan, err := l.engine.Plan(p.Session)
	if err != nil {
		return nil, rpcErrorFromEngine(err)
	}
	return plan, nil
}

func (l *serveLoop) handleCommit(params json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("commit requires \"session\""))
	}
	res, err := l.engine.Commit(p.Session)
	if err != nil {
		return nil, rpcErrorFromEngine(err)
	}
	return res, nil
}

func (l *serveLoop) handleAbort(params json.RawMessage) (interface{}, *rpcError) {
	var p sessionRPCParams
	if rerr := decodeParams(params, &p); rerr != nil {
		return nil, rerr
	}
	if p.Session == "" {
		return nil, rpcErrorFromCLI(InvalidArgsError("abort requires \"session\""))
	}
	if err := l.engine.Abort(p.Session); err != nil {
		return nil, rpcErrorFromEngine(err)
	}
	return map[string]interface{}{"aborted": true}, nil
}

// validateOpShape runs the op through apply.ParseOps (the single canonical op
// validator) so a malformed op is rejected the same way `ooxml apply` would.
func validateOpShape(op apply.Operation) *rpcError {
	encoded, err := json.Marshal([]apply.Operation{op})
	if err != nil {
		return rpcErrorFromCLI(InvalidArgsError(err.Error()))
	}
	if _, err := apply.ParseOps(encoded); err != nil {
		return rpcErrorFromCLI(InvalidArgsError(err.Error()))
	}
	if err := validateKnownOperationCommand(op.Command); err != nil {
		return rpcErrorFromCLI(err)
	}
	return nil
}

func decodeParams(params json.RawMessage, dst interface{}) *rpcError {
	if len(params) == 0 {
		return nil
	}
	dec := json.NewDecoder(bytes.NewReader(params))
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
	var extra any
	if err := dec.Decode(&extra); err != io.EOF {
		message := "trailing JSON value after params object"
		if err != nil {
			message = "trailing data after params object: " + err.Error()
		}
		return &rpcError{
			Code:    rpcInvalidParams,
			Message: "invalid params",
			Data: &ErrorBody{
				Code: "invalid_args", ExitCode: ExitInvalidArgs, Message: message,
			},
		}
	}
	return nil
}

func errResponse(id json.RawMessage, code int, msg string, data *ErrorBody) *rpcResponse {
	if len(id) == 0 {
		id = json.RawMessage("null")
	}
	return &rpcResponse{
		JSONRPC: "2.0",
		ID:      id,
		Error:   &rpcError{Code: code, Message: msg, Data: data},
	}
}

// rpcErrorFromCLI wraps a CLIError as a JSON-RPC error carrying the existing
// ErrorBody as data and the exit code on the wire.
func rpcErrorFromCLI(err *CLIError) *rpcError {
	code := err.Code
	if code == "" {
		code = codeForExit(err.ExitCode)
	}
	return &rpcError{
		Code:    rpcCodeForExit(err.ExitCode),
		Message: err.Message,
		Data: &ErrorBody{
			Code:        code,
			ExitCode:    err.ExitCode,
			Message:     err.Message,
			Diagnostics: err.Diagnostics,
		},
	}
}

// rpcErrorFromEngine maps engine/apply typed errors into the same ErrorBody
// contract the CLI emits, so serve and apply report failures identically.
func rpcErrorFromEngine(err error) *rpcError {
	switch e := err.(type) {
	case *serve.MultiSourceError:
		return rpcErrorFromCLI(&CLIError{
			ExitCode: ExitUnsupportedType,
			Code:     codeForExit(ExitUnsupportedType),
			Message:  e.Error(),
		})
	case *serve.AddressPositionalHandleAfterShiftError:
		return rpcErrorFromCLI(&CLIError{
			ExitCode: ExitInvalidArgs,
			Code:     codeForExit(ExitInvalidArgs),
			Message:  e.Error(),
		})
	case *serve.ReadCommandDeniedError:
		return rpcErrorFromCLI(&CLIError{
			ExitCode: ExitInvalidArgs,
			Code:     codeForExit(ExitInvalidArgs),
			Message:  e.Error(),
		})
	case *serve.SessionNotFoundError:
		return rpcErrorFromCLI(&CLIError{
			ExitCode: ExitTargetNotFound,
			Code:     codeForExit(ExitTargetNotFound),
			Message:  e.Error(),
		})
	case *apply.OpError:
		return rpcErrorFromCLI(cliErrorFromOpError(e))
	case *apply.ValidationError:
		var diags []result.Diagnostic
		if e.Diagnostics != nil {
			diags = e.Diagnostics
		}
		return rpcErrorFromCLI(ValidationFailedErrorWithDiagnostics(e.Error(), diags))
	default:
		// Heuristic mapping for engine string errors that mirror CLI conditions.
		msg := err.Error()
		switch {
		case strings.HasPrefix(msg, "file not found"):
			return rpcErrorFromCLI(&CLIError{ExitCode: ExitFileNotFound, Code: codeForExit(ExitFileNotFound), Message: msg})
		case strings.HasPrefix(msg, "unsupported type"):
			return rpcErrorFromCLI(&CLIError{ExitCode: ExitUnsupportedType, Code: codeForExit(ExitUnsupportedType), Message: msg})
		case strings.Contains(msg, "dry-run") || strings.Contains(msg, "in-place") ||
			strings.Contains(msg, "backup") || strings.Contains(msg, "must specify"):
			return rpcErrorFromCLI(InvalidArgsError(msg))
		default:
			return rpcErrorFromCLI(NewCLIErrorf(ExitUnexpected, "%s", msg))
		}
	}
}

// rpcCodeForExit maps an exit code to a JSON-RPC application error code outside
// the reserved range. The authoritative exit code stays in error.data.exitCode.
func rpcCodeForExit(exit int) int {
	switch exit {
	case ExitInvalidArgs:
		return -32010
	case ExitFileNotFound:
		return -32011
	case ExitUnsupportedType:
		return -32012
	case ExitValidationFailed:
		return -32013
	case ExitTargetNotFound:
		return -32014
	default:
		return rpcInternalError
	}
}

func init() {
	rootCmd.AddCommand(serveCmd)
}
