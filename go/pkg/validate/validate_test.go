package validate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestValidateValidFixture(t *testing.T) {
	// Test minimal-title (valid fixture)
	pptxPath := filepath.Join("../../testdata/pptx/minimal-title/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Valid fixture should have no errors (may have info/warnings)
	hasErrors := false
	for _, d := range diags {
		if d.Severity == result.Error {
			hasErrors = true
			t.Logf("unexpected error: %s: %s", d.Code, d.Message)
		}
	}
	if hasErrors {
		t.Errorf("valid fixture should not produce errors")
	}
}

func TestValidateMissingMediaFixture(t *testing.T) {
	// Test corrupted-missing-media fixture
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

	// Should detect missing media
	hasMissingMediaDiag := false
	for _, d := range diags {
		if d.Code == "PPTX_MISSING_MEDIA" || d.Code == "REL_DANGLING_TARGET" {
			hasMissingMediaDiag = true
			t.Logf("detected corruption: %s: %s", d.Code, d.Message)
		}
	}

	if !hasMissingMediaDiag {
		t.Errorf("should detect missing media in corrupted fixture")
	}
}

func TestValidateDanglingLayoutFixture(t *testing.T) {
	// Test corrupted-dangling-layout fixture
	pptxPath := filepath.Join("../../testdata/pptx/corrupted-dangling-layout/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Should detect dangling layout
	hasDanglingLayoutDiag := false
	for _, d := range diags {
		if d.Code == "PPTX_DANGLING_LAYOUT" || d.Code == "REL_DANGLING_TARGET" {
			hasDanglingLayoutDiag = true
			t.Logf("detected corruption: %s: %s", d.Code, d.Message)
		}
	}

	if !hasDanglingLayoutDiag {
		t.Errorf("should detect dangling layout in corrupted fixture")
	}
}

func TestValidateMultiLayoutFixture(t *testing.T) {
	// Test multi-layout (valid but complex)
	pptxPath := filepath.Join("../../testdata/pptx/multi-layout/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Valid fixture should have no errors
	hasErrors := false
	for _, d := range diags {
		if d.Severity == result.Error {
			hasErrors = true
			t.Logf("unexpected error: %s: %s", d.Code, d.Message)
		}
	}
	if hasErrors {
		t.Errorf("valid multi-layout fixture should not produce errors")
	}
}

func TestValidatePictureFixture(t *testing.T) {
	// Test picture-placeholder (has images)
	pptxPath := filepath.Join("../../testdata/pptx/picture-placeholder/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Valid picture fixture should have no errors
	hasErrors := false
	for _, d := range diags {
		if d.Severity == result.Error {
			hasErrors = true
			t.Logf("unexpected error: %s: %s", d.Code, d.Message)
		}
	}
	if hasErrors {
		t.Errorf("valid picture-placeholder fixture should not produce errors")
	}
}

func TestValidateDiagnosticStructure(t *testing.T) {
	pptxPath := filepath.Join("../../testdata/pptx/minimal-title/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Verify diagnostic structure
	for _, d := range diags {
		if d.Code == "" {
			t.Error("diagnostic missing code")
		}
		if d.Message == "" {
			t.Error("diagnostic missing message")
		}
		// Severity should be one of the valid values
		if d.Severity != result.Info && d.Severity != result.Warning && d.Severity != result.Error {
			t.Errorf("diagnostic has invalid severity: %v", d.Severity)
		}
	}
}

func TestValidateEmptyPackage(t *testing.T) {
	// This test is illustrative - we'd need a truly empty zip to test.
	// For now, we just verify that ValidatePackage handles gracefully.
	// A real empty zip would fail at the opc.Open stage.
}

func TestValidateDiagnosticFormatting(t *testing.T) {
	// Verify that diagnostics format correctly
	d := result.Diagnostic{
		Code:     "TEST_CODE",
		Severity: result.Error,
		Message:  "test message",
	}

	if d.Code != "TEST_CODE" {
		t.Error("diagnostic code not preserved")
	}
	if d.Severity != result.Error {
		t.Error("diagnostic severity not preserved")
	}
	if d.Message != "test message" {
		t.Error("diagnostic message not preserved")
	}
}

// M12-1 Tests: Validation Extensions

func TestValidateDuplicateShapeIDs(t *testing.T) {
	// Create a test to verify duplicate shape ID detection
	// For now, we test that valid fixtures don't have duplicate IDs
	pptxPath := filepath.Join("../../testdata/pptx/minimal-title/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Valid fixture should not have duplicate shape ID errors
	for _, d := range diags {
		if d.Code == "PPTX_DUPLICATE_SHAPE_ID" {
			t.Errorf("valid fixture should not have duplicate shape IDs: %s", d.Message)
		}
	}
}

func TestValidateTextBodyStructure(t *testing.T) {
	// Test that valid text bodies don't produce errors
	pptxPath := filepath.Join("../../testdata/pptx/minimal-title/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Valid fixture should have proper text body structure
	hasTextBodyErrors := false
	for _, d := range diags {
		if d.Code == "PPTX_TEXT_BODY_EMPTY" && d.Severity == result.Error {
			hasTextBodyErrors = true
			t.Logf("unexpected text body error: %s", d.Message)
		}
	}
	if hasTextBodyErrors {
		t.Errorf("valid fixture should have proper text body structure")
	}
}

func TestValidatePlaceholderStructure(t *testing.T) {
	// Test that placeholders have proper structure
	pptxPath := filepath.Join("../../testdata/pptx/minimal-title/presentation.pptx")
	session, err := opc.Open(pptxPath)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer session.Close()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("validation should not error: %v", err)
	}

	// Valid fixture should have proper placeholder structure
	hasPlaceholderErrors := false
	for _, d := range diags {
		if d.Code == "PPTX_PLACEHOLDER_NO_TEXT_BODY" && d.Severity == result.Error {
			hasPlaceholderErrors = true
			t.Logf("unexpected placeholder error: %s", d.Message)
		}
	}
	if hasPlaceholderErrors {
		t.Errorf("valid fixture should have proper placeholder structure")
	}
}
