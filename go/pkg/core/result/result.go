// Package result defines the Result type for error handling with diagnostics.
package result

import (
	"fmt"
	"strings"
)

// Severity represents the severity level of a diagnostic.
type Severity int

const (
	Info Severity = iota
	Warning
	Error
)

// String returns the string representation of the severity.
func (s Severity) String() string {
	switch s {
	case Info:
		return "info"
	case Warning:
		return "warning"
	case Error:
		return "error"
	default:
		return "unknown"
	}
}

// Diagnostic represents a diagnostic message with code, severity, and text.
type Diagnostic struct {
	Code     string
	Severity Severity
	Message  string
}

// Result[T] represents either a success value or a failure with diagnostics.
type Result[T any] struct {
	Value       T
	Diagnostics []Diagnostic
	isSuccess   bool
}

// Success creates a successful result with the given value.
func Success[T any](value T) Result[T] {
	return Result[T]{
		Value:       value,
		Diagnostics: nil,
		isSuccess:   true,
	}
}

// Failure creates a failed result with diagnostics.
func Failure[T any](diags ...Diagnostic) Result[T] {
	var zero T
	return Result[T]{
		Value:       zero,
		Diagnostics: diags,
		isSuccess:   false,
	}
}

// IsSuccess returns true if the result is a success.
func (r Result[T]) IsSuccess() bool {
	return r.isSuccess
}

// IsFailure returns true if the result is a failure.
func (r Result[T]) IsFailure() bool {
	return !r.isSuccess
}

// AddDiagnostic adds a diagnostic to the result.
func (r *Result[T]) AddDiagnostic(diag Diagnostic) {
	r.Diagnostics = append(r.Diagnostics, diag)
}

// String returns a string representation of all diagnostics.
func (r Result[T]) String() string {
	if len(r.Diagnostics) == 0 {
		if r.isSuccess {
			return "success"
		}
		return "failure (no diagnostics)"
	}

	var buf strings.Builder
	for i, d := range r.Diagnostics {
		if i > 0 {
			buf.WriteString("\n")
		}
		buf.WriteString(fmt.Sprintf("[%s] %s: %s", d.Severity, d.Code, d.Message))
	}
	return buf.String()
}
