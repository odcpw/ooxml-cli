package cli

import (
	"bytes"
	"encoding/json"
	"errors"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestCLIErrorExitCodes(t *testing.T) {
	tests := []struct {
		name     string
		err      *CLIError
		expected int
	}{
		{
			name:     "FileNotFoundError",
			err:      FileNotFoundError("test.txt"),
			expected: ExitFileNotFound,
		},
		{
			name:     "UnsupportedTypeError",
			err:      UnsupportedTypeError("xyz"),
			expected: ExitUnsupportedType,
		},
		{
			name:     "ValidationFailedError",
			err:      ValidationFailedError("validation failed"),
			expected: ExitValidationFailed,
		},
		{
			name:     "InvalidArgsError",
			err:      InvalidArgsError("invalid args"),
			expected: ExitInvalidArgs,
		},
		{
			name:     "TargetNotFoundError",
			err:      TargetNotFoundError("target"),
			expected: ExitTargetNotFound,
		},
		{
			name:     "RenderFailedError",
			err:      RenderFailedError("render failed"),
			expected: ExitRenderFailed,
		},
		{
			name:     "DiffThresholdError",
			err:      DiffThresholdError("threshold exceeded"),
			expected: ExitDiffThreshold,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if tt.err.ExitCode != tt.expected {
				t.Errorf("expected exit code %d, got %d", tt.expected, tt.err.ExitCode)
			}
			if tt.err.Message == "" {
				t.Errorf("expected non-empty message")
			}
		})
	}
}

func TestNewCLIErrorf(t *testing.T) {
	err := NewCLIErrorf(ExitFileNotFound, "file not found: %s", "test.txt")
	if err.ExitCode != ExitFileNotFound {
		t.Errorf("expected exit code %d, got %d", ExitFileNotFound, err.ExitCode)
	}
	if err.Message != "file not found: test.txt" {
		t.Errorf("expected 'file not found: test.txt', got '%s'", err.Message)
	}
}

func TestExitCodeConstants(t *testing.T) {
	expectedCodes := map[string]int{
		"ExitSuccess":          0,
		"ExitUnexpected":       1,
		"ExitInvalidArgs":      2,
		"ExitFileNotFound":     3,
		"ExitUnsupportedType":  4,
		"ExitValidationFailed": 5,
		"ExitTargetNotFound":   6,
		"ExitRenderFailed":     7,
		"ExitDiffThreshold":    8,
		"ExitPartialSuccess":   9,
	}

	actualCodes := map[string]int{
		"ExitSuccess":          ExitSuccess,
		"ExitUnexpected":       ExitUnexpected,
		"ExitInvalidArgs":      ExitInvalidArgs,
		"ExitFileNotFound":     ExitFileNotFound,
		"ExitUnsupportedType":  ExitUnsupportedType,
		"ExitValidationFailed": ExitValidationFailed,
		"ExitTargetNotFound":   ExitTargetNotFound,
		"ExitRenderFailed":     ExitRenderFailed,
		"ExitDiffThreshold":    ExitDiffThreshold,
		"ExitPartialSuccess":   ExitPartialSuccess,
	}

	for name, expected := range expectedCodes {
		actual := actualCodes[name]
		if actual != expected {
			t.Errorf("%s: expected %d, got %d", name, expected, actual)
		}
	}
}

func TestRenderErrorTextModeUnchanged(t *testing.T) {
	var stderr bytes.Buffer

	exitCode := renderError(FileNotFoundError("missing.pptx"), "text", false, &stderr)

	assert.Equal(t, ExitFileNotFound, exitCode)
	assert.Equal(t, "Error: file not found: missing.pptx\n", stderr.String())
}

func TestRenderErrorJSONEnvelope(t *testing.T) {
	var stderr bytes.Buffer
	err := ValidationFailedErrorWithDiagnostics("output validation failed: package has 1 error(s)", []result.Diagnostic{
		{
			Code:     "REL_DANGLING_TARGET",
			Severity: result.Error,
			Message:  "relationship target is missing",
		},
	})

	exitCode := renderError(err, "json", false, &stderr)

	assert.Equal(t, ExitValidationFailed, exitCode)

	var envelope ErrorEnvelope
	require.NoError(t, json.Unmarshal(stderr.Bytes(), &envelope))
	assert.Equal(t, "validation_failed", envelope.Error.Code)
	assert.Equal(t, ExitValidationFailed, envelope.Error.ExitCode)
	assert.Equal(t, "output validation failed: package has 1 error(s)", envelope.Error.Message)
	require.Len(t, envelope.Error.Diagnostics, 1)
	assert.Equal(t, "REL_DANGLING_TARGET", envelope.Error.Diagnostics[0].Code)
	assert.Equal(t, "error", envelope.Error.Diagnostics[0].Severity)
}

func TestRenderErrorReportedJSONSuppressesEnvelope(t *testing.T) {
	var stderr bytes.Buffer
	err := NewCLIError(ExitValidationFailed, "already reported")
	err.Reported = true

	exitCode := renderError(err, "json", true, &stderr)

	assert.Equal(t, ExitValidationFailed, exitCode)
	assert.Empty(t, stderr.String())
}

func TestRenderErrorEmptyMessageSuppressesOutput(t *testing.T) {
	var stderr bytes.Buffer

	exitCode := renderError(NewCLIError(ExitDiffThreshold, ""), "json", false, &stderr)

	assert.Equal(t, ExitDiffThreshold, exitCode)
	assert.Empty(t, stderr.String())
}

func TestRenderErrorNonCLIErrorUsesInvalidArgs(t *testing.T) {
	var stderr bytes.Buffer

	exitCode := renderError(errors.New("unknown command"), "json", false, &stderr)

	assert.Equal(t, ExitInvalidArgs, exitCode)

	var envelope ErrorEnvelope
	require.NoError(t, json.Unmarshal(stderr.Bytes(), &envelope))
	assert.Equal(t, "invalid_args", envelope.Error.Code)
	assert.Equal(t, ExitInvalidArgs, envelope.Error.ExitCode)
	assert.Equal(t, "unknown command", envelope.Error.Message)
}

func TestValidationFailureErrorCarriesDiagnostics(t *testing.T) {
	err := validationFailureError([]result.Diagnostic{
		{Code: "INFO", Severity: result.Info, Message: "not fatal"},
		{Code: "BROKEN", Severity: result.Error, Message: "broken package"},
	})

	require.NotNil(t, err)
	assert.Equal(t, ExitValidationFailed, err.ExitCode)
	assert.Equal(t, "validation_failed", err.Code)
	assert.Equal(t, "output validation failed: package has 1 error(s)", err.Message)
	require.Len(t, err.Diagnostics, 2)
	assert.Equal(t, "BROKEN", err.Diagnostics[1].Code)
}
