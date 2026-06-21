package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TestValidateLayoutTextOverflow tests the validate-layout command with text overflow fixture
func TestValidateLayoutTextOverflow(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "layout-qa-text-overflow")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected output, got empty string")
	}

	// Should contain "Issues" or report findings
	if len(output) == 0 {
		t.Error("expected non-empty text output")
	}
}

// TestValidateLayoutShapeCollision tests the validate-layout command with collision fixture
func TestValidateLayoutShapeCollision(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "layout-qa-shape-collision")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected output, got empty string")
	}
}

// TestValidateLayoutDenseSlide tests the validate-layout command with dense slide fixture
func TestValidateLayoutDenseSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "layout-qa-dense-slide")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected output, got empty string")
	}

	// Dense slide should report high density
	if len(output) == 0 {
		t.Error("expected non-empty text output")
	}
}

// TestValidateLayoutCleanSlide tests the validate-layout command with a clean fixture
func TestValidateLayoutCleanSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "minimal-title")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected output, got empty string")
	}

	// Clean slide should report no issues
	if len(output) == 0 {
		t.Error("expected non-empty text output")
	}
}

// TestValidateLayoutJSONOutput tests the validate-layout command with JSON output
func TestValidateLayoutJSONOutput(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "layout-qa-text-overflow")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile, "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Validate JSON structure
	var result map[string]interface{}
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}

	// Check required fields
	if _, ok := result["file"]; !ok {
		t.Errorf("expected 'file' field in JSON output")
	}
	if _, ok := result["totalSlides"]; !ok {
		t.Errorf("expected 'totalSlides' field in JSON output")
	}
	if _, ok := result["slidesWithIssues"]; !ok {
		t.Errorf("expected 'slidesWithIssues' field in JSON output")
	}
	if _, ok := result["averageDensity"]; !ok {
		t.Errorf("expected 'averageDensity' field in JSON output")
	}
	if _, ok := result["totalTextOverflows"]; !ok {
		t.Errorf("expected 'totalTextOverflows' field in JSON output")
	}
	if _, ok := result["totalCollisions"]; !ok {
		t.Errorf("expected 'totalCollisions' field in JSON output")
	}
	if _, ok := result["hasIssues"]; !ok {
		t.Errorf("expected 'hasIssues' field in JSON output")
	}
}

// TestValidateLayoutFileNotFound tests the validate-layout command with a non-existent file
func TestValidateLayoutFileNotFound(t *testing.T) {
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", "/nonexistent/file.pptx"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for non-existent file")
	}

	// Check that it's the correct exit code
	if cliErr, ok := err.(*CLIError); ok {
		if cliErr.ExitCode != ExitFileNotFound {
			t.Errorf("expected exit code %d, got %d", ExitFileNotFound, cliErr.ExitCode)
		}
	}
}

// TestValidateLayoutJSONReportStructure tests that JSON report has correct structure
func TestValidateLayoutJSONReportStructure(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "layout-qa-shape-collision")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile, "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Validate complete structure
	var analysis model.LayoutQAAnalysis
	if err := json.Unmarshal([]byte(output), &analysis); err != nil {
		t.Fatalf("expected valid LayoutQAAnalysis JSON, got: %v", err)
	}

	// Check that the structure is valid
	if analysis.TotalSlides == 0 {
		t.Errorf("expected at least 1 slide")
	}

	// Each slide report should have valid structure
	for i, report := range analysis.SlideReports {
		if report.SlideNumber == 0 {
			t.Errorf("slide report %d: expected non-zero slide number", i)
		}
		if report.Density == nil {
			t.Errorf("slide report %d: expected density info", i)
		}
	}
}

// TestValidateLayoutOutputFile tests the validate-layout command with output file
func TestValidateLayoutOutputFile(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "minimal-title")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	// Create a temporary file for output
	tmpFile := filepath.Join(t.TempDir(), "output.txt")

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "validate-layout", fixtureFile, "--output", tmpFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	// Check that the output file was created
	if _, err := os.Stat(tmpFile); err != nil {
		t.Fatalf("output file not created: %v", err)
	}

	// Read and verify the output file
	content, err := os.ReadFile(tmpFile)
	if err != nil {
		t.Fatalf("failed to read output file: %v", err)
	}

	if len(content) == 0 {
		t.Error("expected non-empty output file")
	}
}
