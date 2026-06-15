package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
)

// Exit codes mapping to error conditions
const (
	ExitSuccess          = 0 // Success
	ExitUnexpected       = 1 // Unexpected error
	ExitInvalidArgs      = 2 // Invalid arguments
	ExitFileNotFound     = 3 // File not found
	ExitUnsupportedType  = 4 // Unsupported type
	ExitValidationFailed = 5 // Validation failed
	ExitTargetNotFound   = 6 // Target not found
	ExitRenderFailed     = 7 // Render failed
	ExitDiffThreshold    = 8 // Diff threshold exceeded
	ExitPartialSuccess   = 9 // Partial success
)

// CLIError is a typed error that maps to an exit code
type CLIError struct {
	ExitCode    int
	Message     string
	Code        string
	Diagnostics []DiagnosticJSON
	Reported    bool
}

// Error implements the error interface
func (e *CLIError) Error() string {
	return e.Message
}

// NewCLIError creates a new CLIError with the given exit code and message
func NewCLIError(exitCode int, message string) *CLIError {
	return &CLIError{
		ExitCode: exitCode,
		Message:  message,
	}
}

// NewCLIErrorf is like NewCLIError but with printf-style formatting
func NewCLIErrorf(exitCode int, format string, args ...interface{}) *CLIError {
	return &CLIError{
		ExitCode: exitCode,
		Message:  fmt.Sprintf(format, args...),
	}
}

type ErrorEnvelope struct {
	Error ErrorBody `json:"error"`
}

type ErrorBody struct {
	Code        string           `json:"code"`
	ExitCode    int              `json:"exitCode"`
	Message     string           `json:"message"`
	Diagnostics []DiagnosticJSON `json:"diagnostics,omitempty"`
}

func codeForExit(code int) string {
	switch code {
	case ExitUnexpected:
		return "unexpected"
	case ExitInvalidArgs:
		return "invalid_args"
	case ExitFileNotFound:
		return "file_not_found"
	case ExitUnsupportedType:
		return "unsupported_type"
	case ExitValidationFailed:
		return "validation_failed"
	case ExitTargetNotFound:
		return "target_not_found"
	case ExitRenderFailed:
		return "render_failed"
	case ExitDiffThreshold:
		return "diff_threshold"
	case ExitPartialSuccess:
		return "partial_success"
	default:
		return "unexpected"
	}
}

func renderError(err error, format string, pretty bool, stderr io.Writer) int {
	if err == nil {
		return ExitSuccess
	}

	cliErr, ok := AsCLIError(err)
	if !ok {
		cliErr = &CLIError{ExitCode: ExitInvalidArgs, Message: err.Error()}
	}
	if cliErr.Message == "" {
		return cliErr.ExitCode
	}
	if format == "json" {
		if cliErr.Reported {
			return cliErr.ExitCode
		}
		code := cliErr.Code
		if code == "" {
			code = codeForExit(cliErr.ExitCode)
		}
		envelope := ErrorEnvelope{
			Error: ErrorBody{
				Code:        code,
				ExitCode:    cliErr.ExitCode,
				Message:     cliErr.Message,
				Diagnostics: cliErr.Diagnostics,
			},
		}
		var (
			data []byte
			e    error
		)
		if pretty {
			data, e = json.MarshalIndent(envelope, "", "  ")
		} else {
			data, e = json.Marshal(envelope)
		}
		if e == nil {
			fmt.Fprintf(stderr, "%s\n", data)
			return cliErr.ExitCode
		}
	}
	fmt.Fprintf(stderr, "Error: %s\n", cliErr.Message)
	return cliErr.ExitCode
}

// Exit writes an error message to stderr and exits with the given code
func Exit(code int, message string) {
	if message != "" {
		fmt.Fprintf(os.Stderr, "Error: %s\n", message)
	}
	os.Exit(code)
}

// ExitWithError exits with a CLIError's exit code and message
func ExitWithError(err *CLIError) {
	Exit(err.ExitCode, err.Message)
}

func AsCLIError(err error) (*CLIError, bool) {
	var cliErr *CLIError
	if errors.As(err, &cliErr) {
		return cliErr, true
	}
	return nil, false
}

// HandleError handles a CLIError and exits appropriately.
// If err is not a CLIError, it treats it as an unexpected error.
func HandleError(err error) {
	if err == nil {
		return
	}

	var cliErr *CLIError
	if errors.As(err, &cliErr) {
		ExitWithError(cliErr)
	}

	// Unexpected error
	Exit(ExitUnexpected, err.Error())
}

// FileNotFoundError creates a CLIError for file not found
func FileNotFoundError(path string) *CLIError {
	return NewCLIErrorf(ExitFileNotFound, "file not found: %s", path)
}

// UnsupportedTypeError creates a CLIError for unsupported type
func UnsupportedTypeError(typeName string) *CLIError {
	return NewCLIErrorf(ExitUnsupportedType, "unsupported type: %s", typeName)
}

// ValidationFailedError creates a CLIError for validation failure
func ValidationFailedError(message string) *CLIError {
	return NewCLIError(ExitValidationFailed, message)
}

func ValidationFailedErrorWithDiagnostics(message string, diags []result.Diagnostic) *CLIError {
	return &CLIError{
		ExitCode:    ExitValidationFailed,
		Code:        codeForExit(ExitValidationFailed),
		Message:     message,
		Diagnostics: diagnosticsJSON(diags),
	}
}

func validationFailureError(diags []result.Diagnostic) *CLIError {
	errorCount := 0
	for _, diag := range diags {
		if diag.Severity == result.Error {
			errorCount++
		}
	}
	if errorCount == 0 {
		return nil
	}
	return ValidationFailedErrorWithDiagnostics(fmt.Sprintf("output validation failed: package has %d error(s)", errorCount), diags)
}

func diagnosticsJSON(diags []result.Diagnostic) []DiagnosticJSON {
	if len(diags) == 0 {
		return nil
	}
	out := make([]DiagnosticJSON, 0, len(diags))
	for _, diag := range diags {
		out = append(out, DiagnosticJSON{
			Code:     diag.Code,
			Severity: diag.Severity.String(),
			Message:  diag.Message,
		})
	}
	return out
}

// InvalidArgsError creates a CLIError for invalid arguments
func InvalidArgsError(message string) *CLIError {
	return NewCLIError(ExitInvalidArgs, message)
}

// TargetNotFoundError creates a CLIError for target not found
func TargetNotFoundError(target string) *CLIError {
	return NewCLIErrorf(ExitTargetNotFound, "target not found: %s", target)
}

// RenderFailedError creates a CLIError for render failure
func RenderFailedError(message string) *CLIError {
	return NewCLIError(ExitRenderFailed, message)
}

// DiffThresholdError creates a CLIError for diff threshold exceeded
func DiffThresholdError(message string) *CLIError {
	return NewCLIError(ExitDiffThreshold, message)
}
