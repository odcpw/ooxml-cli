package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestExtractNotesNotesSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "notes-slide")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "extract", "notes", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Error("expected output, got empty")
	}
}

func TestExtractNotesNotesHandout(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "notes-handout")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "extract", "notes", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Error("expected output, got empty")
	}
}

func TestExtractNotesMinimalTitle(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "extract", "notes", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Error("expected output, got empty")
	}
}

func TestExtractNotesJSONFormat(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "notes-slide")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "extract", "notes", "--format", "json", fixtureFile})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Error("expected JSON output, got empty")
	}

	// Try to parse JSON to verify format
	var result interface{}
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Errorf("invalid JSON output: %v", err)
	}
}
