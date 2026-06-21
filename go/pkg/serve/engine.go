// Package serve implements the long-lived session engine behind `ooxml serve`.
//
// It exposes a SessionEngine interface that fixes the held-session PROTOCOL the
// client perceives (open -> interleaved op/inspect/validate/plan -> commit|abort)
// and a working-copy MVP backend. On open the input is copied to a working temp;
// each op is dispatched EXACTLY like apply.Executor (a subprocess of the ooxml
// binary against the working temp via apply.RunOp), so all existing mutation
// commands work untouched with clean global-flag isolation. inspect/validate run
// the existing read paths against the working temp; commit is an atomic
// rename of the working temp to the target. No new mutation logic is introduced.
//
// The faster in-memory held-session backend is a FUTURE fast-path behind the
// same interface and is intentionally NOT built here.
package serve

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// SchemaVersion identifies the serve engine contract. Distinct from
// apply.SchemaVersion because the serve contract is a superset; bump on breaking
// changes.
const SchemaVersion = 1

// OpenParams configures a new session.
type OpenParams struct {
	// Path is the input OOXML file. It is never modified until commit.
	Path string
	// Out is the commit target when not in-place. Mutually exclusive with InPlace.
	Out string
	// InPlace commits back over Path.
	InPlace bool
	// Backup, when set with InPlace, is the literal path the original is copied to
	// before the in-place commit.
	Backup string
	// NoValidate skips validate-by-default on commit.
	NoValidate bool
	// DryRun accepts ops/inspect/validate but commit writes nothing.
	DryRun bool
}

// OpenResult is returned from Open.
type OpenResult struct {
	SessionID string `json:"sessionId"`
	Type      string `json:"type"`
}

// SessionEngine is the backend-agnostic spine both `serve` and (later) an MCP
// adapter ride on. The working-copy backend is the only implementation today.
type SessionEngine interface {
	// Open stages a working copy of the input and returns a session id.
	Open(p OpenParams) (OpenResult, error)
	// Op runs ONE mutation op against the session's working copy, advancing it
	// only on success. The returned AppliedOp carries the op's readback.
	Op(sessionID string, op apply.Operation) (apply.AppliedOp, error)
	// Inspect runs a read-only command against the current working state and
	// returns its JSON output verbatim.
	Inspect(sessionID string, command string, args map[string]apply.Arg) (json.RawMessage, error)
	// Validate validates the current working state and returns its diagnostics.
	Validate(sessionID string) ([]result.Diagnostic, error)
	// Plan returns the would-apply plan for the buffered ops without committing.
	Plan(sessionID string) (apply.Plan, error)
	// Commit validates (unless NoValidate) then atomically writes the working copy
	// to the target. Returns the apply.Result. A dry-run session writes nothing.
	Commit(sessionID string) (apply.Result, error)
	// Abort discards the working copy; nothing is written, the original untouched.
	Abort(sessionID string) error
	// Close reaps every still-open session, removing its working-copy scratch dir.
	// It is the graceful-shutdown reaper for sessions opened but never committed or
	// aborted before the host disconnects (the normal stdio EOF path). Idempotent
	// and safe to call with no live sessions.
	Close() error
}

// MultiSourceError reports that an op is a multi-source operation (it consumes a
// second package, e.g. clone/import/merge) which the working-copy MVP engine does
// not support.
type MultiSourceError struct {
	Command string
}

func (e *MultiSourceError) Error() string {
	return fmt.Sprintf("multi-source ops not yet supported in serve: %q", e.Command)
}

// AddressPositionalHandleAfterShiftError reports a session op that tries to use
// an XLSX address-positional handle after an earlier row/column structural shift
// in the same held session.
type AddressPositionalHandleAfterShiftError struct {
	Command    string
	ArgKey     string
	ArgValue   string
	ShiftIndex int
	ShiftCmd   string
}

func (e *AddressPositionalHandleAfterShiftError) Error() string {
	return fmt.Sprintf("address-positional XLSX handle %s=%q cannot be used after op %d (%s) shifted rows/columns earlier in this session; re-run inspect/find against the current session state to resolve a fresh handle, or target the cell/comment positionally with --sheet/--cell",
		e.ArgKey, e.ArgValue, e.ShiftIndex, e.ShiftCmd)
}

// SessionNotFoundError reports an unknown session id.
type SessionNotFoundError struct {
	SessionID string
}

func (e *SessionNotFoundError) Error() string {
	return fmt.Sprintf("session not found: %q", e.SessionID)
}

// session is one open working-copy session.
type session struct {
	mu sync.Mutex

	id      string
	ext     string
	working string // current working temp; advances on each successful op
	tempDir string // per-session scratch dir for rolling temps

	pkgType string
	opts    OpenParams
	ops     []apply.Operation // buffered for plan/result
	applied []apply.AppliedOp
	temps   []string // every temp created, for cleanup on abort/close

	shiftedAddressSpace bool
	shiftOpIndex        int
	shiftOpCommand      string

	committed bool
	aborted   bool
}

// Engine is the working-copy SessionEngine implementation.
type Engine struct {
	// Self is the path to the ooxml binary used to dispatch each op as a
	// subprocess (production: os.Executable(); tests: a freshly built binary).
	Self string
	// TempBase is the directory under which per-session scratch dirs are created.
	// Empty means the OS default temp dir.
	TempBase string

	mu       sync.Mutex
	sessions map[string]*session
	seq      int
}

// NewEngine returns a working-copy engine dispatching ops through self.
func NewEngine(self, tempBase string) *Engine {
	return &Engine{
		Self:     self,
		TempBase: tempBase,
		sessions: make(map[string]*session),
	}
}

func (e *Engine) nextID() string {
	e.seq++
	return fmt.Sprintf("s%d", e.seq)
}

// scratchBase returns the directory under which a session's per-session scratch
// dir is created. The --temp-dir override (TempBase) always wins. With no
// override, a non-dry-run session bases its scratch dir on the COMMIT TARGET's
// directory so the final commit MoveFile (rename) is intra-filesystem and thus
// atomic, instead of landing in /tmp (often a different filesystem) and degrading
// to copy+remove. Dry-run (which never writes) and an unknown/unusable target dir
// fall back to "" (the OS default temp dir).
func (e *Engine) scratchBase(p OpenParams) string {
	if e.TempBase != "" {
		return e.TempBase
	}
	if p.DryRun {
		return ""
	}
	target := p.Out
	if p.InPlace {
		target = p.Path
	}
	if target == "" {
		return ""
	}
	dir := filepath.Dir(target)
	if info, err := os.Stat(dir); err != nil || !info.IsDir() {
		return ""
	}
	return dir
}

func (e *Engine) get(sessionID string) (*session, error) {
	e.mu.Lock()
	defer e.mu.Unlock()
	s, ok := e.sessions[sessionID]
	if !ok {
		return nil, &SessionNotFoundError{SessionID: sessionID}
	}
	return s, nil
}

// Open stages a working copy and detects the package type.
func (e *Engine) Open(p OpenParams) (OpenResult, error) {
	if _, err := os.Stat(p.Path); err != nil {
		return OpenResult{}, fmt.Errorf("file not found: %s", p.Path)
	}
	if err := validateOpenParams(p); err != nil {
		return OpenResult{}, err
	}

	// Open once to assert the file is a readable OOXML package and detect type.
	pkg, err := opc.Open(p.Path)
	if err != nil {
		return OpenResult{}, fmt.Errorf("failed to open package: %w", err)
	}
	pkgType := opc.DetectType(pkg)
	pkg.Close()
	if pkgType == opc.PackageTypeUnknown {
		return OpenResult{}, fmt.Errorf("unsupported type: %s", pkgType.String())
	}

	tempDir, err := os.MkdirTemp(e.scratchBase(p), "ooxml-serve-*")
	if err != nil {
		return OpenResult{}, fmt.Errorf("failed to create session temp dir: %w", err)
	}

	ext := filepath.Ext(p.Path)
	working := filepath.Join(tempDir, "working-0"+ext)
	if err := apply.CopyFile(p.Path, working); err != nil {
		os.RemoveAll(tempDir)
		return OpenResult{}, fmt.Errorf("failed to stage working copy: %w", err)
	}

	e.mu.Lock()
	id := e.nextID()
	s := &session{
		id:      id,
		ext:     ext,
		working: working,
		tempDir: tempDir,
		pkgType: pkgType.String(),
		opts:    p,
		temps:   []string{working},
	}
	e.sessions[id] = s
	e.mu.Unlock()

	return OpenResult{SessionID: id, Type: pkgType.String()}, nil
}

func validateOpenParams(p OpenParams) error {
	if p.DryRun {
		if p.Out != "" || p.InPlace {
			return fmt.Errorf("--dry-run cannot be combined with out or in-place")
		}
		if p.Backup != "" {
			return fmt.Errorf("backup cannot be used with dry-run")
		}
		return nil
	}
	if p.Out == "" && !p.InPlace {
		return fmt.Errorf("must specify exactly one of out, in-place, or dry-run")
	}
	if p.Out != "" && p.InPlace {
		return fmt.Errorf("cannot specify both out and in-place")
	}
	if p.Backup != "" && !p.InPlace {
		return fmt.Errorf("backup can only be used with in-place")
	}
	return nil
}

// Op runs a single op against the working copy, advancing it only on success.
func (e *Engine) Op(sessionID string, op apply.Operation) (apply.AppliedOp, error) {
	s, err := e.get(sessionID)
	if err != nil {
		return apply.AppliedOp{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.usable(); err != nil {
		return apply.AppliedOp{}, err
	}
	op.Command = apply.NormalizeCommand(op.Command)
	if s.shiftedAddressSpace {
		if key, value, ok := apply.FirstAddressPositionalArg(op); ok {
			return apply.AppliedOp{}, &AddressPositionalHandleAfterShiftError{
				Command:    op.Command,
				ArgKey:     key,
				ArgValue:   value,
				ShiftIndex: s.shiftOpIndex,
				ShiftCmd:   s.shiftOpCommand,
			}
		}
	}
	if IsMultiSource(op.Command) {
		return apply.AppliedOp{}, &MultiSourceError{Command: op.Command}
	}

	current := s.working
	next := filepath.Join(s.tempDir, fmt.Sprintf("working-%d%s", len(s.ops)+1, s.ext))

	ao, runErr := apply.RunOp(e.Self, current, next, op)
	if runErr != nil {
		// Failed op must NOT corrupt the working copy or advance state: discard
		// the next temp; the session stays usable at its last-good working copy.
		os.Remove(next)
		if oe, ok := runErr.(*apply.OpError); ok {
			oe.FailedOpIndex = len(s.ops)
		}
		return apply.AppliedOp{}, runErr
	}

	s.temps = append(s.temps, next)
	ao.Index = len(s.ops)
	if s.opts.DryRun {
		ao = rewriteDryRunAppliedOp(ao, []string{current, next})
	}
	s.ops = append(s.ops, op)
	s.applied = append(s.applied, ao)
	s.working = next
	if apply.StructuralShiftCommand(op.Command) && !s.shiftedAddressSpace {
		s.shiftedAddressSpace = true
		s.shiftOpIndex = ao.Index
		s.shiftOpCommand = op.Command
	}
	return ao, nil
}

func rewriteDryRunAppliedOp(op apply.AppliedOp, scratchPaths []string) apply.AppliedOp {
	rewritten := apply.RewriteAppliedReadbacks([]apply.AppliedOp{op}, scratchPaths, "<dry-run-output>")
	if len(rewritten) == 0 || rewritten[0].Readback == nil {
		return op
	}
	var value any
	if err := json.Unmarshal(rewritten[0].Readback, &value); err != nil {
		return rewritten[0]
	}
	value, changed := markDryRunReadback(value)
	if !changed {
		return rewritten[0]
	}
	data, err := json.Marshal(value)
	if err != nil {
		return rewritten[0]
	}
	rewritten[0].Readback = json.RawMessage(data)
	return rewritten[0]
}

func markDryRunReadback(value any) (any, bool) {
	switch v := value.(type) {
	case map[string]any:
		changed := false
		if b, ok := v["dryRun"].(bool); !ok || !b {
			v["dryRun"] = true
			changed = true
		}
		return v, changed
	default:
		return value, false
	}
}

// Inspect runs a read-only command against the current working state.
func (e *Engine) Inspect(sessionID string, command string, args map[string]apply.Arg) (json.RawMessage, error) {
	s, err := e.get(sessionID)
	if err != nil {
		return nil, err
	}
	s.mu.Lock()
	working := s.working
	usable := s.usable()
	s.mu.Unlock()
	if usable != nil {
		return nil, usable
	}
	return runReadCommand(e.Self, working, command, args)
}

// Validate validates the current working state via serialize-then-open so
// [Content_Types].xml regeneration is captured (apply.ValidateFile re-opens the
// working temp from disk, which already reflects every applied op).
func (e *Engine) Validate(sessionID string) ([]result.Diagnostic, error) {
	s, err := e.get(sessionID)
	if err != nil {
		return nil, err
	}
	s.mu.Lock()
	working := s.working
	usable := s.usable()
	s.mu.Unlock()
	if usable != nil {
		return nil, usable
	}
	pkg, err := opc.Open(working)
	if err != nil {
		return nil, fmt.Errorf("failed to open working copy for validation: %w", err)
	}
	defer pkg.Close()
	return validatePackage(pkg)
}

// Plan returns the would-apply plan for the buffered ops.
func (e *Engine) Plan(sessionID string) (apply.Plan, error) {
	s, err := e.get(sessionID)
	if err != nil {
		return apply.Plan{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	return apply.Plan{
		SchemaVersion: apply.SchemaVersion,
		File:          s.opts.Path,
		OpsCount:      len(s.ops),
		DryRun:        s.opts.DryRun,
		Plan:          apply.BuildPlan(s.ops, s.opts.Path),
	}, nil
}

// Commit validates (unless NoValidate) then atomically publishes the working copy.
//
// Working-copy semantics (by design): a session edits an isolated copy taken at
// Open; Commit publishes that copy to the target with a last-writer-wins atomic
// rename. It does NOT detect a concurrent EXTERNAL modification of the target
// between Open and Commit — if another process rewrote the output path meanwhile,
// the commit overwrites it. This matches the apply CLI (which likewise re-writes
// its --out target) and the single-author session model; callers that need
// optimistic concurrency should coordinate out of band or use distinct --out
// paths per session.
func (e *Engine) Commit(sessionID string) (apply.Result, error) {
	s, err := e.get(sessionID)
	if err != nil {
		return apply.Result{}, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.usable(); err != nil {
		return apply.Result{}, err
	}

	// Validate-by-default. A failure leaves the session OPEN and re-committable.
	if !s.opts.NoValidate {
		if err := apply.ValidateFile(s.working); err != nil {
			return apply.Result{}, err
		}
	}

	outputPath := s.opts.Out
	if s.opts.InPlace {
		outputPath = s.opts.Path
	}

	result := apply.Result{
		SchemaVersion: apply.SchemaVersion,
		File:          s.opts.Path,
		OpsCount:      len(s.ops),
		Applied:       s.applied,
		DryRun:        s.opts.DryRun,
	}

	// Dry-run: validate then discard without writing.
	if s.opts.DryRun {
		result.Applied = apply.RewriteAppliedReadbacks(result.Applied, s.temps, "<dry-run-output>")
		s.committed = true
		s.cleanup()
		e.drop(sessionID)
		return result, nil
	}

	// Backup the original before an in-place publish.
	if s.opts.InPlace && s.opts.Backup != "" {
		if _, statErr := os.Stat(outputPath); statErr == nil {
			if err := apply.CopyFile(outputPath, s.opts.Backup); err != nil {
				return apply.Result{}, fmt.Errorf("failed to create backup: %w", err)
			}
		}
	}

	scratchPaths := append([]string(nil), s.temps...)
	if err := apply.MoveFile(s.working, outputPath); err != nil {
		return apply.Result{}, fmt.Errorf("failed to write output file: %w", err)
	}
	// The working copy's mode derives from a scratch temp (0600); align the
	// published file with the original so commit never silently downgrades the
	// user's file permissions.
	if info, statErr := os.Stat(s.opts.Path); statErr == nil {
		_ = os.Chmod(outputPath, info.Mode().Perm())
	}
	// working was consumed by the move; drop it from cleanup tracking.
	s.dropTemp(s.working)

	result.Output = outputPath
	result.Applied = apply.RewriteAppliedReadbacks(result.Applied, scratchPaths, outputPath)
	result.ValidateCommand = apply.ShellCommand("ooxml", "validate", "--strict", outputPath)

	s.committed = true
	s.cleanup()
	e.drop(sessionID)
	return result, nil
}

// Abort discards the working copy; nothing is written.
func (e *Engine) Abort(sessionID string) error {
	s, err := e.get(sessionID)
	if err != nil {
		return err
	}
	s.mu.Lock()
	s.aborted = true
	s.cleanup()
	s.mu.Unlock()
	e.drop(sessionID)
	return nil
}

// Close reaps every still-open session so a graceful shutdown (stdio EOF) never
// leaks the per-session working-copy scratch dirs. It snapshots the live session
// ids under e.mu, releases the lock (Abort re-takes e.mu and the session mutex),
// then aborts each — Abort runs s.cleanup() (RemoveAll of the scratch dir) and
// drops the session. Committed/aborted sessions are already gone from e.sessions,
// so Close is idempotent and a no-op when there are no live sessions.
func (e *Engine) Close() error {
	e.mu.Lock()
	ids := make([]string, 0, len(e.sessions))
	for id := range e.sessions {
		ids = append(ids, id)
	}
	e.mu.Unlock()
	for _, id := range ids {
		_ = e.Abort(id)
	}
	return nil
}

func (e *Engine) drop(sessionID string) {
	e.mu.Lock()
	delete(e.sessions, sessionID)
	e.mu.Unlock()
}

// usable reports whether the session can still accept calls. Caller holds s.mu.
func (s *session) usable() error {
	if s.committed {
		return fmt.Errorf("session %q already committed", s.id)
	}
	if s.aborted {
		return fmt.Errorf("session %q already aborted", s.id)
	}
	return nil
}

// cleanup removes the per-session scratch dir. Caller holds s.mu.
func (s *session) cleanup() {
	for _, t := range s.temps {
		os.Remove(t)
	}
	s.temps = nil
	if s.tempDir != "" {
		os.RemoveAll(s.tempDir)
	}
}

// dropTemp removes path from the tracked temps (it was consumed by a move).
func (s *session) dropTemp(path string) {
	for i, t := range s.temps {
		if t == path {
			s.temps = append(s.temps[:i], s.temps[i+1:]...)
			return
		}
	}
}
