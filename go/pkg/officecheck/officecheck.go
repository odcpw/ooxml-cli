// Package officecheck runs best-available local Office-compatible open checks.
package officecheck

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const defaultTimeout = 2 * time.Minute

// MissingDependencyError indicates that no supported Office-compatible engine is
// available on PATH.
type MissingDependencyError struct {
	Tool string
}

func (e *MissingDependencyError) Error() string {
	return fmt.Sprintf("required Office-compatible tool not available: %s", e.Tool)
}

// ToolFailureError indicates that the external open-check engine failed.
type ToolFailureError struct {
	Tool   string
	Args   []string
	Stdout string
	Stderr string
	Cause  error
}

func (e *ToolFailureError) Error() string {
	if e.Stderr != "" {
		return fmt.Sprintf("%s failed: %s", e.Tool, strings.TrimSpace(e.Stderr))
	}
	if e.Stdout != "" {
		return fmt.Sprintf("%s failed: %s", e.Tool, strings.TrimSpace(e.Stdout))
	}
	return fmt.Sprintf("%s failed: %v", e.Tool, e.Cause)
}

func (e *ToolFailureError) Unwrap() error {
	return e.Cause
}

// Runner abstracts command discovery and execution for deterministic tests.
type Runner interface {
	LookPath(name string) (string, error)
	Run(ctx context.Context, name string, args []string) (*RunResult, error)
}

// RunResult captures subprocess output.
type RunResult struct {
	Stdout string
	Stderr string
}

// ExecRunner is the production subprocess runner.
type ExecRunner struct{}

func (ExecRunner) LookPath(name string) (string, error) {
	return exec.LookPath(name)
}

func (ExecRunner) Run(ctx context.Context, name string, args []string) (*RunResult, error) {
	cmd := exec.CommandContext(ctx, name, args...)
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	return &RunResult{Stdout: stdout.String(), Stderr: stderr.String()}, err
}

// Tools wraps the local Office-compatible engine toolchain.
type Tools struct {
	Runner  Runner
	Timeout time.Duration
}

// NewTools constructs a checker with production defaults.
func NewTools() *Tools {
	return &Tools{Runner: ExecRunner{}, Timeout: defaultTimeout}
}

// Options controls one open-check run.
type Options struct {
	// Family is the OOXML family: pptx or xlsx. Macro variants use the same
	// family names because the conversion engine accepts .pptm/.xlsm paths.
	Family string
	// OutDir keeps the conversion artifact in a caller-chosen directory. When
	// empty, a temporary directory is used and removed before returning.
	OutDir string
}

// Result is the machine-readable open-check proof.
type Result struct {
	Status                  string   `json:"status"` // passed, failed, skipped
	Checked                 bool     `json:"checked"`
	Engine                  string   `json:"engine,omitempty"`
	Method                  string   `json:"method,omitempty"`
	ConversionFormat        string   `json:"conversionFormat,omitempty"`
	OutputPath              string   `json:"outputPath,omitempty"`
	OutputBytes             int64    `json:"outputBytes,omitempty"`
	OfficeOpenVerified      bool     `json:"officeOpenVerified"`
	MicrosoftOfficeVerified bool     `json:"microsoftOfficeVerified"`
	MacroExecutionVerified  bool     `json:"macroExecutionVerified"`
	ErrorCode               string   `json:"errorCode,omitempty"`
	Error                   string   `json:"error,omitempty"`
	Limitations             []string `json:"limitations,omitempty"`
}

// Check verifies that a local Office-compatible engine can open the package by
// converting it headlessly. It is compatibility evidence, not macro execution or
// Microsoft Office proof.
func (t *Tools) Check(filePath string, opts Options) (*Result, error) {
	if t == nil {
		t = NewTools()
	}
	if t.Runner == nil {
		t.Runner = ExecRunner{}
	}
	if t.Timeout <= 0 {
		t.Timeout = defaultTimeout
	}
	format, err := conversionFormat(opts.Family)
	if err != nil {
		return failedResult("", "", "", "invalid_family", err), err
	}
	result := &Result{
		Status:                  "skipped",
		ConversionFormat:        format,
		MicrosoftOfficeVerified: false,
		MacroExecutionVerified:  false,
		Limitations:             defaultLimitations(),
	}
	engine, err := t.findLibreOfficeBinary()
	if err != nil {
		result.ErrorCode = "missing_engine"
		result.Error = err.Error()
		return result, err
	}
	result.Engine = engine
	result.Method = "libreoffice-headless-convert"

	outDir, cleanup, err := prepareOutputDir(opts.OutDir)
	if err != nil {
		return failedResult(engine, result.Method, format, "output_dir_error", err), err
	}
	defer cleanup()
	profileDir, err := os.MkdirTemp("", "ooxml-office-profile-*")
	if err != nil {
		return failedResult(engine, result.Method, format, "profile_error", err), err
	}
	defer os.RemoveAll(profileDir)

	ctx, cancel := context.WithTimeout(context.Background(), t.Timeout)
	defer cancel()

	args := []string{
		libreOfficeUserInstallationArg(profileDir),
		"--headless",
		"--convert-to", format,
		"--outdir", outDir,
		filePath,
	}
	result.Checked = true
	runResult, runErr := t.Runner.Run(ctx, engine, args)
	if runErr != nil {
		if errors.Is(ctx.Err(), context.DeadlineExceeded) {
			err = fmt.Errorf("%s office open-check timed out", engine)
		} else {
			toolErr := &ToolFailureError{Tool: engine, Args: args, Cause: runErr}
			if runResult != nil {
				toolErr.Stdout = runResult.Stdout
				toolErr.Stderr = runResult.Stderr
			}
			err = toolErr
		}
		result.Status = "failed"
		result.ErrorCode = "engine_failed"
		result.Error = err.Error()
		return result, err
	}

	outputPath, size, err := findConvertedOutput(outDir, filePath, format)
	if err != nil {
		result.Status = "failed"
		result.ErrorCode = "conversion_output_missing"
		result.Error = err.Error()
		return result, err
	}
	result.Status = "passed"
	result.OfficeOpenVerified = true
	result.OutputBytes = size
	if opts.OutDir != "" {
		result.OutputPath = outputPath
	}
	return result, nil
}

func failedResult(engine, method, format, code string, err error) *Result {
	return &Result{
		Status:                  "failed",
		Engine:                  engine,
		Method:                  method,
		ConversionFormat:        format,
		MicrosoftOfficeVerified: false,
		MacroExecutionVerified:  false,
		ErrorCode:               code,
		Error:                   err.Error(),
		Limitations:             defaultLimitations(),
	}
}

func (t *Tools) findLibreOfficeBinary() (string, error) {
	for _, candidate := range []string{"soffice", "libreoffice"} {
		if _, err := t.Runner.LookPath(candidate); err == nil {
			return candidate, nil
		}
	}
	return "", &MissingDependencyError{Tool: "soffice"}
}

func conversionFormat(family string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(family)) {
	case "pptx", "pptm", "presentation", "powerpoint":
		return "pdf", nil
	case "xlsx", "xlsm", "spreadsheet", "excel":
		return "csv", nil
	default:
		return "", fmt.Errorf("office open-check supports pptx/pptm and xlsx/xlsm only (family %q)", family)
	}
}

func prepareOutputDir(outDir string) (string, func(), error) {
	if strings.TrimSpace(outDir) != "" {
		if err := os.MkdirAll(outDir, 0o755); err != nil {
			return "", func() {}, fmt.Errorf("failed to create output directory: %w", err)
		}
		return outDir, func() {}, nil
	}
	tempDir, err := os.MkdirTemp("", "ooxml-office-check-*")
	if err != nil {
		return "", func() {}, fmt.Errorf("failed to create temporary output directory: %w", err)
	}
	return tempDir, func() { _ = os.RemoveAll(tempDir) }, nil
}

func libreOfficeUserInstallationArg(profileDir string) string {
	return "-env:UserInstallation=" + (&url.URL{Scheme: "file", Path: profileDir}).String()
}

func findConvertedOutput(outDir, filePath, format string) (string, int64, error) {
	preferred := filepath.Join(outDir, strings.TrimSuffix(filepath.Base(filePath), filepath.Ext(filePath))+"."+format)
	if info, err := os.Stat(preferred); err == nil {
		if info.Size() <= 0 {
			return preferred, 0, fmt.Errorf("converted output is empty: %s", preferred)
		}
		return preferred, info.Size(), nil
	}
	matches, err := filepath.Glob(filepath.Join(outDir, "*."+format))
	if err != nil {
		return "", 0, fmt.Errorf("failed to inspect output directory: %w", err)
	}
	for _, match := range matches {
		info, err := os.Stat(match)
		if err != nil {
			continue
		}
		if info.Size() > 0 {
			return match, info.Size(), nil
		}
	}
	return "", 0, fmt.Errorf("no non-empty %s output produced in %s", format, outDir)
}

func defaultLimitations() []string {
	return []string{
		"LibreOffice/soffice load and conversion is compatibility evidence, not Microsoft Office proof.",
		"Macros are not executed or compiled by this check.",
	}
}
