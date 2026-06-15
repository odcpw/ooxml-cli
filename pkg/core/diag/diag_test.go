package diag

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
)

func TestErrorDiagnostic(t *testing.T) {
	d := Error(CodeIOError, "file not found")
	if d.Code != CodeIOError {
		t.Errorf("Expected code %s, got %s", CodeIOError, d.Code)
	}
	if d.Severity != result.Error {
		t.Errorf("Expected Error severity, got %v", d.Severity)
	}
	if d.Message != "file not found" {
		t.Errorf("Expected message 'file not found', got %q", d.Message)
	}
}

func TestWarningDiagnostic(t *testing.T) {
	d := Warning(CodeValidation, "missing attribute")
	if d.Code != CodeValidation {
		t.Errorf("Expected code %s, got %s", CodeValidation, d.Code)
	}
	if d.Severity != result.Warning {
		t.Errorf("Expected Warning severity, got %v", d.Severity)
	}
	if d.Message != "missing attribute" {
		t.Errorf("Expected message 'missing attribute', got %q", d.Message)
	}
}

func TestInfoDiagnostic(t *testing.T) {
	d := Info("INFO", "processing complete")
	if d.Code != "INFO" {
		t.Errorf("Expected code INFO, got %s", d.Code)
	}
	if d.Severity != result.Info {
		t.Errorf("Expected Info severity, got %v", d.Severity)
	}
	if d.Message != "processing complete" {
		t.Errorf("Expected message 'processing complete', got %q", d.Message)
	}
}

func TestErrorf(t *testing.T) {
	d := Errorf(CodeXMLError, "line %d: %s", 42, "invalid element")
	if d.Code != CodeXMLError {
		t.Errorf("Expected code %s, got %s", CodeXMLError, d.Code)
	}
	if d.Severity != result.Error {
		t.Errorf("Expected Error severity, got %v", d.Severity)
	}
	if d.Message != "line 42: invalid element" {
		t.Errorf("Expected message 'line 42: invalid element', got %q", d.Message)
	}
}

func TestWarningf(t *testing.T) {
	d := Warningf(CodeValidation, "missing %d attributes", 3)
	if d.Code != CodeValidation {
		t.Errorf("Expected code %s, got %s", CodeValidation, d.Code)
	}
	if d.Severity != result.Warning {
		t.Errorf("Expected Warning severity, got %v", d.Severity)
	}
	if d.Message != "missing 3 attributes" {
		t.Errorf("Expected message 'missing 3 attributes', got %q", d.Message)
	}
}

func TestInfof(t *testing.T) {
	d := Infof("TIMING", "processed in %dms", 250)
	if d.Code != "TIMING" {
		t.Errorf("Expected code TIMING, got %s", d.Code)
	}
	if d.Severity != result.Info {
		t.Errorf("Expected Info severity, got %v", d.Severity)
	}
	if d.Message != "processed in 250ms" {
		t.Errorf("Expected message 'processed in 250ms', got %q", d.Message)
	}
}

func TestFormat(t *testing.T) {
	d := Error(CodeIOError, "disk full")
	formatted := Format(d)

	if !strings.Contains(formatted, "error") {
		t.Errorf("Format should contain severity 'error', got %q", formatted)
	}
	if !strings.Contains(formatted, CodeIOError) {
		t.Errorf("Format should contain code %s, got %q", CodeIOError, formatted)
	}
	if !strings.Contains(formatted, "disk full") {
		t.Errorf("Format should contain message 'disk full', got %q", formatted)
	}

	// Check exact format: [severity] code: message
	expected := "[error] IO_ERROR: disk full"
	if formatted != expected {
		t.Errorf("Expected format %q, got %q", expected, formatted)
	}
}

func TestFormatWithWarning(t *testing.T) {
	d := Warning(CodeXMLNamespace, "unknown namespace prefix")
	formatted := Format(d)

	expected := "[warning] XML_NAMESPACE: unknown namespace prefix"
	if formatted != expected {
		t.Errorf("Expected format %q, got %q", expected, formatted)
	}
}

func TestFormatWithInfo(t *testing.T) {
	d := Info("STAT", "5 slides found")
	formatted := Format(d)

	expected := "[info] STAT: 5 slides found"
	if formatted != expected {
		t.Errorf("Expected format %q, got %q", expected, formatted)
	}
}
