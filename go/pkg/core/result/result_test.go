package result

import (
	"strings"
	"testing"
)

func TestSuccess(t *testing.T) {
	r := Success(42)
	if !r.IsSuccess() {
		t.Errorf("Expected success, got failure")
	}
	if r.Value != 42 {
		t.Errorf("Expected value 42, got %d", r.Value)
	}
	if len(r.Diagnostics) != 0 {
		t.Errorf("Expected no diagnostics, got %d", len(r.Diagnostics))
	}
}

func TestFailure(t *testing.T) {
	d1 := Diagnostic{Code: "ERR001", Severity: Error, Message: "test error"}
	d2 := Diagnostic{Code: "WARN001", Severity: Warning, Message: "test warning"}
	r := Failure[int](d1, d2)

	if !r.IsFailure() {
		t.Errorf("Expected failure, got success")
	}
	if r.Value != 0 {
		t.Errorf("Expected zero value, got %d", r.Value)
	}
	if len(r.Diagnostics) != 2 {
		t.Errorf("Expected 2 diagnostics, got %d", len(r.Diagnostics))
	}
}

func TestAddDiagnostic(t *testing.T) {
	r := Success(0)
	d := Diagnostic{Code: "INFO001", Severity: Info, Message: "test info"}
	r.AddDiagnostic(d)

	if len(r.Diagnostics) != 1 {
		t.Errorf("Expected 1 diagnostic, got %d", len(r.Diagnostics))
	}
	if r.Diagnostics[0].Code != "INFO001" {
		t.Errorf("Expected code INFO001, got %s", r.Diagnostics[0].Code)
	}
}

func TestSeverityString(t *testing.T) {
	tests := map[Severity]string{
		Info:    "info",
		Warning: "warning",
		Error:   "error",
	}

	for sev, expected := range tests {
		if sev.String() != expected {
			t.Errorf("Severity %d: expected %q, got %q", sev, expected, sev.String())
		}
	}
}

func TestResultString(t *testing.T) {
	// Test empty success result
	r := Success(0)
	if r.String() != "success" {
		t.Errorf("Expected 'success', got %q", r.String())
	}

	// Test empty failure result
	r2 := Failure[int]()
	if r2.String() != "failure (no diagnostics)" {
		t.Errorf("Expected 'failure (no diagnostics)', got %q", r2.String())
	}

	// Test result with multiple diagnostics
	d1 := Diagnostic{Code: "ERR001", Severity: Error, Message: "error message"}
	d2 := Diagnostic{Code: "WARN001", Severity: Warning, Message: "warning message"}
	r3 := Failure[int](d1, d2)
	s := r3.String()

	if !strings.Contains(s, "ERR001") {
		t.Errorf("Result string missing ERR001: %q", s)
	}
	if !strings.Contains(s, "error") {
		t.Errorf("Result string missing 'error': %q", s)
	}
	if !strings.Contains(s, "WARN001") {
		t.Errorf("Result string missing WARN001: %q", s)
	}
	if !strings.Contains(s, "warning") {
		t.Errorf("Result string missing 'warning': %q", s)
	}
}
