package validate

import (
	"encoding/json"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// TestIntegrationValidationReport tests the full validation pipeline and
// verifies that diagnostics have the correct structure for CLI reporting.
func TestIntegrationValidationReport(t *testing.T) {
	tests := []struct {
		name        string
		fixture     string
		shouldError bool
		checkDiag   func([]result.Diagnostic) bool
	}{
		{
			name:        "minimal-title valid",
			fixture:     "minimal-title/presentation.pptx",
			shouldError: false,
			checkDiag: func(diags []result.Diagnostic) bool {
				// Should have no errors
				for _, d := range diags {
					if d.Severity == result.Error {
						return false
					}
				}
				return true
			},
		},
		{
			name:        "corrupted-missing-media",
			fixture:     "corrupted-missing-media/presentation.pptx",
			shouldError: false, // ValidatePackage doesn't error, just returns diags
			checkDiag: func(diags []result.Diagnostic) bool {
				// Should detect missing media
				hasMedia := false
				for _, d := range diags {
					if d.Code == "PPTX_MISSING_MEDIA" || d.Code == "REL_DANGLING_TARGET" {
						hasMedia = true
						if d.Severity != result.Warning && d.Severity != result.Error {
							return false
						}
					}
				}
				return hasMedia
			},
		},
		{
			name:        "corrupted-dangling-layout",
			fixture:     "corrupted-dangling-layout/presentation.pptx",
			shouldError: false,
			checkDiag: func(diags []result.Diagnostic) bool {
				// Should detect dangling layout
				hasLayout := false
				for _, d := range diags {
					if d.Code == "PPTX_DANGLING_LAYOUT" || d.Code == "REL_DANGLING_TARGET" {
						hasLayout = true
						if d.Severity != result.Error {
							return false
						}
					}
				}
				return hasLayout
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			pptxPath := filepath.Join("../../testdata/pptx", tc.fixture)
			session, err := opc.Open(pptxPath)
			if err != nil {
				t.Fatalf("failed to open fixture: %v", err)
			}
			defer session.Close()

			diags, err := ValidatePackage(session)
			if err != nil != tc.shouldError {
				t.Fatalf("expected error=%v, got error=%v: %v", tc.shouldError, err != nil, err)
			}

			if !tc.checkDiag(diags) {
				t.Errorf("diagnostic check failed")
				for _, d := range diags {
					t.Logf("  %s [%s]: %s", d.Code, d.Severity, d.Message)
				}
			}
		})
	}
}

// TestValidationResultSerialization ensures diagnostics can be serialized to JSON
// for CLI output.
func TestValidationResultSerialization(t *testing.T) {
	pptxPath := filepath.Join("../../testdata/pptx/corrupted-missing-media/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Try to serialize to JSON
	data, err := json.MarshalIndent(diags, "", "  ")
	if err != nil {
		t.Fatalf("failed to marshal diagnostics to JSON: %v", err)
	}

	if len(data) == 0 {
		t.Fatal("serialized JSON is empty")
	}

	// Verify JSON structure
	var parsed []result.Diagnostic
	err = json.Unmarshal(data, &parsed)
	if err != nil {
		t.Fatalf("failed to unmarshal diagnostics: %v", err)
	}

	if len(parsed) != len(diags) {
		t.Errorf("expected %d diagnostics after unmarshal, got %d", len(diags), len(parsed))
	}

	for i, d := range parsed {
		if d.Code != diags[i].Code {
			t.Errorf("diagnostic %d: expected code %s, got %s", i, diags[i].Code, d.Code)
		}
		if d.Message != diags[i].Message {
			t.Errorf("diagnostic %d: expected message %s, got %s", i, diags[i].Message, d.Message)
		}
	}
}

// TestValidationDiagnosticCodes verifies that all diagnostic codes are
// properly namespaced and meaningful.
func TestValidationDiagnosticCodes(t *testing.T) {
	fixtures := []string{
		"minimal-title/presentation.pptx",
		"corrupted-missing-media/presentation.pptx",
		"corrupted-dangling-layout/presentation.pptx",
		"picture-placeholder/presentation.pptx",
	}

	validCodePrefixes := map[string]bool{
		"PKG":  true, // Package integrity
		"REL":  true, // Relationship integrity
		"PPTX": true, // PPTX semantics
		"VBA":  true, // VBA package consistency
		"XML":  true, // XML well-formedness
	}

	for _, fixture := range fixtures {
		pptxPath := filepath.Join("../../testdata/pptx", fixture)
		session, err := opc.Open(pptxPath)
		if err != nil {
			t.Logf("skipping %s: %v", fixture, err)
			continue
		}

		diags, _ := ValidatePackage(session)
		session.Close()

		for _, d := range diags {
			hasValidPrefix := false
			for prefix := range validCodePrefixes {
				if len(d.Code) > len(prefix) && d.Code[:len(prefix)] == prefix {
					hasValidPrefix = true
					break
				}
			}

			if !hasValidPrefix {
				t.Errorf("diagnostic code %s from %s has invalid prefix", d.Code, fixture)
			}
		}
	}
}
