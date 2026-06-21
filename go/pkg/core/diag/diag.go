// Package diag defines diagnostic codes and severity-aware formatting.
package diag

import (
	"fmt"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
)

// Common diagnostic codes
const (
	CodeUnknown       = "UNKNOWN"
	CodeIOError       = "IO_ERROR"
	CodeXMLError      = "XML_ERROR"
	CodeXMLNamespace  = "XML_NAMESPACE"
	CodeParseError    = "PARSE_ERROR"
	CodeValidation    = "VALIDATION"
	CodeNotFound      = "NOT_FOUND"
	CodeAlreadyExists = "ALREADY_EXISTS"
)

// Error creates an error-level diagnostic.
func Error(code, message string) result.Diagnostic {
	return result.Diagnostic{
		Code:     code,
		Severity: result.Error,
		Message:  message,
	}
}

// Warning creates a warning-level diagnostic.
func Warning(code, message string) result.Diagnostic {
	return result.Diagnostic{
		Code:     code,
		Severity: result.Warning,
		Message:  message,
	}
}

// Info creates an info-level diagnostic.
func Info(code, message string) result.Diagnostic {
	return result.Diagnostic{
		Code:     code,
		Severity: result.Info,
		Message:  message,
	}
}

// Errorf creates an error-level diagnostic with formatted message.
func Errorf(code, format string, args ...interface{}) result.Diagnostic {
	return result.Diagnostic{
		Code:     code,
		Severity: result.Error,
		Message:  fmt.Sprintf(format, args...),
	}
}

// Warningf creates a warning-level diagnostic with formatted message.
func Warningf(code, format string, args ...interface{}) result.Diagnostic {
	return result.Diagnostic{
		Code:     code,
		Severity: result.Warning,
		Message:  fmt.Sprintf(format, args...),
	}
}

// Infof creates an info-level diagnostic with formatted message.
func Infof(code, format string, args ...interface{}) result.Diagnostic {
	return result.Diagnostic{
		Code:     code,
		Severity: result.Info,
		Message:  fmt.Sprintf(format, args...),
	}
}

// Format returns the fully formatted diagnostic string: [severity] code: message
func Format(d result.Diagnostic) string {
	return fmt.Sprintf("[%s] %s: %s", d.Severity, d.Code, d.Message)
}
