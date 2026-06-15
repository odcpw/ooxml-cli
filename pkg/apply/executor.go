package apply

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
)

// Executor runs an ops sequence using the rolling-temp-file pipeline. Self is
// the path to the ooxml binary to re-dispatch each op against (production:
// os.Executable(); tests: a freshly built ./cmd/ooxml). TempDir is the
// directory used for rolling temp files (production: a created temp dir).
type Executor struct {
	// Self is the path to the ooxml binary used to run each op as a subprocess.
	Self string
	// TempDir holds the rolling temp files. Must exist.
	TempDir string
}

// OpError reports a failed operation in the chain. The chain stops at the first
// failure; nothing is written to the user's output.
type OpError struct {
	FailedOpIndex int
	Command       string
	Stderr        string
	Err           error
}

func (e *OpError) Error() string {
	msg := strings.TrimSpace(e.Stderr)
	if msg == "" && e.Err != nil {
		msg = e.Err.Error()
	}
	return fmt.Sprintf("op %d (%s) failed: %s", e.FailedOpIndex, e.Command, msg)
}

func (e *OpError) Unwrap() error { return e.Err }

// ValidationError reports that the final package failed validation. It carries
// the diagnostics so the CLI layer can map them into the standard envelope. Err
// is set when the validator itself failed to run (vs. reporting diagnostics);
// either case is a validation failure and maps to ExitValidationFailed.
type ValidationError struct {
	Diagnostics []result.Diagnostic
	Err         error
}

func (e *ValidationError) Error() string {
	if e.Err != nil {
		return fmt.Sprintf("output validation failed: %v", e.Err)
	}
	errs := 0
	for _, d := range e.Diagnostics {
		if d.Severity == result.Error {
			errs++
		}
	}
	return fmt.Sprintf("output validation failed: package has %d error(s)", errs)
}

func (e *ValidationError) Unwrap() error { return e.Err }

// buildArgv builds the deterministic subprocess argv for one op. Arg keys are
// sorted so the argv (and therefore the dry-run plan) is stable. The positional
// input file is inserted right after the command words; per-op output goes to
// out via --out, and per-op validation is disabled (a single final validation
// runs over the whole package instead).
func buildArgv(op Operation, in, out string) []string {
	argv := append([]string{}, splitCommand(NormalizeCommand(op.Command))...)
	argv = append(argv, in)

	keys := make([]string, 0, len(op.Args))
	for k := range op.Args {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	for _, k := range keys {
		argv = AppendFlagArg(argv, k, op.Args[k])
	}

	argv = append(argv, "--out", out, "--json", "--no-validate")
	return argv
}

// splitCommand splits a command string like "xlsx cells set" into words.
func splitCommand(command string) []string {
	return strings.Fields(command)
}

// RunOp runs a single op as a subprocess of the ooxml binary (self), reading the
// package at in and writing the mutated package to out. It is the reusable per-op
// STEP shared by the apply batch pipeline and the long-lived serve engine: it
// builds the deterministic argv, runs the subprocess capturing stdout/stderr,
// and returns the op's readback (parsed from stdout JSON when present).
//
// On a non-zero subprocess exit it returns an *OpError. The returned AppliedOp
// and any *OpError carry Index == 0; the caller is responsible for stamping the
// op's position in the larger sequence (AppliedOp.Index / OpError.FailedOpIndex),
// since RunOp executes one op in isolation and has no notion of sequence index.
func RunOp(self, in, out string, op Operation) (AppliedOp, error) {
	op.Command = NormalizeCommand(op.Command)
	argv := buildArgv(op, in, out)

	cmd := exec.Command(self, argv...)
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if runErr := cmd.Run(); runErr != nil {
		return AppliedOp{}, &OpError{
			Command: op.Command,
			Stderr:  stderr.String(),
			Err:     runErr,
		}
	}
	if err := ensureSubprocessWrotePackage(out); err != nil {
		return AppliedOp{}, &OpError{
			Command: op.Command,
			Stderr:  stderr.String(),
			Err:     err,
		}
	}

	return AppliedOp{
		Command:  op.Command,
		Readback: captureReadback(stdout.Bytes()),
	}, nil
}

func ensureSubprocessWrotePackage(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("subprocess did not write output package %q: %w", path, err)
	}
	if info.Size() == 0 {
		return fmt.Errorf("subprocess did not write output package %q: file is empty", path)
	}
	pkg, err := opc.Open(path)
	if err != nil {
		return fmt.Errorf("subprocess wrote invalid output package %q: %w", path, err)
	}
	if closeErr := pkg.Close(); closeErr != nil {
		return fmt.Errorf("failed to close output package %q: %w", path, closeErr)
	}
	return nil
}

// BuildPlan returns the resolved argv for each op without executing anything.
// The rolling temp paths are shown as <temp.in>/<temp.out> placeholders so the
// plan is stable and readable.
func BuildPlan(ops []Operation, file string) []PlanEntry {
	plan := make([]PlanEntry, 0, len(ops))
	for i, op := range ops {
		in := "<input>"
		if i == 0 {
			in = file
		} else {
			in = fmt.Sprintf("<temp.%d>", i-1)
		}
		out := fmt.Sprintf("<temp.%d>", i)
		plan = append(plan, PlanEntry{
			Index:   i,
			Command: op.Command,
			Argv:    buildArgv(op, in, out),
		})
	}
	return plan
}

// Execute runs the ops all-or-nothing. inputPath is the source file; ops are
// applied in order via rolling temps. If noValidate is false, the final temp is
// validated in-process before any output is written. On success the final temp
// is moved to outputPath (atomically), creating backupPath first if non-empty.
// The collected per-op readbacks are returned for the caller's Result.
//
// On any op failure, Execute returns an *OpError and writes nothing to
// outputPath. On final validation failure it returns a *ValidationError.
func (e *Executor) Execute(inputPath string, ops []Operation, outputPath, backupPath string, noValidate bool) ([]AppliedOp, error) {
	ext := filepath.Ext(inputPath)

	// Track rolling temps for cleanup.
	var temps []string
	cleanup := func() {
		for _, t := range temps {
			os.Remove(t)
		}
	}
	defer cleanup()

	newTemp := func(stage int) (string, error) {
		f, err := os.CreateTemp(e.TempDir, fmt.Sprintf(".ooxml-apply-%d-*%s", stage, ext))
		if err != nil {
			return "", fmt.Errorf("failed to create temp file: %w", err)
		}
		name := f.Name()
		f.Close()
		temps = append(temps, name)
		return name, nil
	}

	// Stage 0 input: copy the source into the first rolling temp so the original
	// is never touched until the final atomic write.
	current, err := newTemp(0)
	if err != nil {
		return nil, err
	}
	if err := copyFile(inputPath, current); err != nil {
		return nil, fmt.Errorf("failed to stage input: %w", err)
	}

	applied := make([]AppliedOp, 0, len(ops))
	for i, op := range ops {
		next, err := newTemp(i + 1)
		if err != nil {
			return nil, err
		}

		ao, runErr := RunOp(e.Self, current, next, op)
		if runErr != nil {
			if oe, ok := runErr.(*OpError); ok {
				oe.FailedOpIndex = i
			}
			return nil, runErr
		}

		ao.Index = i
		applied = append(applied, ao)
		current = next
	}

	// Single final validation over the whole package.
	if !noValidate {
		if err := validateFinal(current); err != nil {
			return nil, err
		}
	}

	// Atomic write to the user's target (only reached after everything passed).
	if backupPath != "" {
		if _, statErr := os.Stat(outputPath); statErr == nil {
			if err := copyFile(outputPath, backupPath); err != nil {
				return nil, fmt.Errorf("failed to create backup: %w", err)
			}
		}
	}
	scratchPaths := append([]string(nil), temps...)
	if err := moveFile(current, outputPath); err != nil {
		return nil, fmt.Errorf("failed to write output file: %w", err)
	}
	// The published file's mode comes from a rolling temp (CreateTemp's 0600) or
	// the cross-FS copy; align it with the source so an in-place or --out commit
	// never silently downgrades the user's file permissions.
	if info, statErr := os.Stat(inputPath); statErr == nil {
		_ = os.Chmod(outputPath, info.Mode().Perm())
	}
	// current was consumed by the move; drop it from cleanup tracking.
	for idx, t := range temps {
		if t == current {
			temps = append(temps[:idx], temps[idx+1:]...)
			break
		}
	}

	return RewriteAppliedReadbacks(applied, scratchPaths, outputPath), nil
}

// captureReadback returns the op's stdout as raw JSON if it parses as JSON;
// otherwise nil (some commands may not emit JSON readback).
func captureReadback(stdout []byte) json.RawMessage {
	trimmed := bytes.TrimSpace(stdout)
	if len(trimmed) == 0 || !json.Valid(trimmed) {
		return nil
	}
	return json.RawMessage(trimmed)
}

// RewriteAppliedReadbacks rewrites per-op readback JSON captured while the op
// wrote to scratch files so final apply/serve results point at the committed
// output. It updates both path fields and command strings containing scratch
// paths. If a readback somehow stops being JSON, it is left untouched.
func RewriteAppliedReadbacks(applied []AppliedOp, scratchPaths []string, finalPath string) []AppliedOp {
	if len(applied) == 0 || len(scratchPaths) == 0 || strings.TrimSpace(finalPath) == "" {
		return applied
	}
	cleaned := make([]string, 0, len(scratchPaths))
	for _, path := range scratchPaths {
		path = strings.TrimSpace(path)
		if path != "" {
			cleaned = append(cleaned, path)
		}
	}
	if len(cleaned) == 0 {
		return applied
	}
	sort.SliceStable(cleaned, func(i, j int) bool { return len(cleaned[i]) > len(cleaned[j]) })
	out := make([]AppliedOp, len(applied))
	copy(out, applied)
	for i := range out {
		if out[i].Readback == nil {
			continue
		}
		var value any
		if err := json.Unmarshal(out[i].Readback, &value); err != nil {
			continue
		}
		value, changed := rewriteScratchValue(value, cleaned, finalPath)
		if !changed {
			continue
		}
		data, err := json.Marshal(value)
		if err != nil {
			continue
		}
		out[i].Readback = json.RawMessage(data)
	}
	return out
}

func rewriteScratchValue(value any, scratchPaths []string, finalPath string) (any, bool) {
	switch v := value.(type) {
	case string:
		rewritten := v
		for _, scratch := range scratchPaths {
			rewritten = strings.ReplaceAll(rewritten, scratch, finalPath)
		}
		return rewritten, rewritten != v
	case []any:
		changed := false
		for i := range v {
			var itemChanged bool
			v[i], itemChanged = rewriteScratchValue(v[i], scratchPaths, finalPath)
			changed = changed || itemChanged
		}
		return v, changed
	case map[string]any:
		changed := false
		for k := range v {
			var itemChanged bool
			v[k], itemChanged = rewriteScratchValue(v[k], scratchPaths, finalPath)
			changed = changed || itemChanged
		}
		return v, changed
	default:
		return value, false
	}
}

// ShellCommand renders a short follow-up command that is safe to paste into a
// POSIX shell. It is for human/agent command strings only; subprocess execution
// still uses argv slices.
func ShellCommand(args ...string) string {
	quoted := make([]string, 0, len(args))
	for _, arg := range args {
		quoted = append(quoted, shellQuoteArg(arg))
	}
	return strings.Join(quoted, " ")
}

func shellQuoteArg(arg string) string {
	if arg == "" {
		return "''"
	}
	if strings.IndexFunc(arg, func(r rune) bool {
		return !(r >= 'A' && r <= 'Z') &&
			!(r >= 'a' && r <= 'z') &&
			!(r >= '0' && r <= '9') &&
			!strings.ContainsRune("@%_+=:,./-", r)
	}) == -1 {
		return arg
	}
	return "'" + strings.ReplaceAll(arg, "'", "'\"'\"'") + "'"
}

func validateFinal(path string) error {
	pkg, err := opc.Open(path)
	if err != nil {
		return &ValidationError{Err: fmt.Errorf("failed to open final package for validation: %w", err)}
	}
	diags, err := validate.ValidatePackage(pkg)
	pkg.Close()
	if err != nil {
		// A validator failure is still a validation outcome → exit 5, not 1.
		return &ValidationError{Err: err}
	}
	for _, d := range diags {
		if d.Severity == result.Error {
			return &ValidationError{Diagnostics: diags}
		}
	}
	return nil
}

// copyFile copies src to dst crash-safely: it writes to a sibling temp in dst's
// directory, fsyncs and closes it, then atomically renames it onto dst. A
// mid-copy failure (write error, disk-full, crash) therefore NEVER truncates or
// destroys an existing dst — the partial bytes live only in the sibling temp,
// which is removed on error and never replaces dst. The final rename is
// intra-directory (and thus intra-filesystem), so it is atomic.
func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	tmp, err := os.CreateTemp(filepath.Dir(dst), ".ooxml-copy-*")
	if err != nil {
		return err
	}
	tmpName := tmp.Name()
	// Preserve src's permissions: CreateTemp makes 0600, but the sibling temp is
	// renamed onto dst, so without this dst would be silently downgraded to 0600
	// (e.g. a user's 0644 file copied for staging/backup or published cross-FS).
	if info, statErr := os.Stat(src); statErr == nil {
		_ = tmp.Chmod(info.Mode().Perm())
	}
	// On any failure before the successful rename, remove the sibling temp so a
	// partial copy never lingers and dst is left untouched.
	committed := false
	defer func() {
		if !committed {
			tmp.Close()
			os.Remove(tmpName)
		}
	}()

	if _, err := io.Copy(tmp, in); err != nil {
		return err
	}
	if err := tmp.Sync(); err != nil {
		return err
	}
	if err := tmp.Close(); err != nil {
		return err
	}
	if err := os.Rename(tmpName, dst); err != nil {
		return err
	}
	committed = true
	return nil
}

// moveFile renames src to dst, falling back to copy+remove across filesystems.
// Once dst is fully written the move is successful: failing to remove the temp
// src is best-effort cleanup, not an output-integrity failure, so it must not
// surface as an error (that would falsely signal "nothing was written" while the
// output is in fact complete, breaking the all-or-nothing contract).
// MoveFile atomically moves src to dst (rename, with a cross-filesystem
// copy+remove fallback). It is the publish primitive shared by the apply
// pipeline's final write and the serve engine's commit.
func MoveFile(src, dst string) error { return moveFile(src, dst) }

// CopyFile copies src to dst, truncating dst. It is exported for reuse by the
// serve engine (staging the working copy on open and creating commit backups).
func CopyFile(src, dst string) error { return copyFile(src, dst) }

// ValidateFile opens the package at path and returns a *ValidationError if it
// has any error-severity diagnostics (or if the validator itself fails to run).
// It is the commit-time validation primitive shared by apply and serve.
func ValidateFile(path string) error { return validateFinal(path) }

// osRename is a seam over os.Rename so tests can force the cross-filesystem
// (EXDEV) path that single-filesystem CI never exercises naturally.
var osRename = os.Rename

func moveFile(src, dst string) error {
	if err := osRename(src, dst); err == nil {
		return nil
	}
	if err := copyFile(src, dst); err != nil {
		return err
	}
	_ = os.Remove(src) // best-effort; dst is already complete.
	return nil
}
