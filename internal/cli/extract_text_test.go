package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

// TestExtractTextMinimalTitle tests the extract text command with the minimal-title fixture
func TestExtractTextMinimalTitle(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testExtractTextCommand(t, "minimal-title", []int{})
}

// TestExtractTextTitleContent tests the extract text command with the title-content fixture
func TestExtractTextTitleContent(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testExtractTextCommand(t, "title-content", []int{})
}

// TestExtractTextWithSlideFilter tests the extract text command with --slide filter
func TestExtractTextWithSlideFilter(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testExtractTextCommand(t, "title-content", []int{1, 2})
}

// TestExtractTextJSON tests the extract text command with JSON output
func TestExtractTextJSON(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "extract", "text", fixtureFile, "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Validate JSON structure
	var result ExtractTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}

	// Check that we have slides
	if len(result.Slides) == 0 {
		t.Errorf("expected slides in output, got %d", len(result.Slides))
	}

	// Check slide structure
	for _, slide := range result.Slides {
		if slide.Slide == 0 {
			t.Errorf("expected non-zero slide number")
		}
		// Shapes can be empty for slides without text
		_ = slide
	}
}

func TestExtractTextRichGoldens(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	for _, tc := range []struct {
		fixture string
		golden  string
	}{
		{fixture: "rich-alignment", golden: "extract-text-rich-alignment.json"},
		{fixture: "rich-bodypr", golden: "extract-text-rich-bodypr.json"},
		{fixture: "rich-formatting", golden: "extract-text-rich-formatting.json"},
		{fixture: "rich-numbered-lists", golden: "extract-text-rich-numbered-lists.json"},
	} {
		t.Run(tc.fixture, func(t *testing.T) {
			fixtureFile := filepath.Join(getTestdataPath(), "pptx", tc.fixture, "presentation.pptx")
			if _, err := os.Stat(fixtureFile); err != nil {
				t.Skipf("fixture not found: %v", err)
			}
			output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "extract", "text", fixtureFile)
			if err != nil {
				t.Fatalf("extract text failed: %v\n%s", err, output)
			}
			compareWithGolden(t, output, filepath.Join("testdata", "golden", tc.golden))
		})
	}
}

// TestExtractTextEmptySlides tests that empty slides produce empty items rather than errors
func TestExtractTextEmptySlides(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	// Use a fixture that might have empty slides
	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", "picture-placeholder")
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "extract", "text", fixtureFile, "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Should have valid JSON output, not an error
	var result ExtractTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}
}

// testExtractTextCommand runs the extract text command on a fixture
func testExtractTextCommand(t *testing.T, fixtureName string, slideFilter []int) {
	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", fixtureName)
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	args := []string{"pptx", "extract", "text", fixtureFile}

	// Add slide filter if specified
	for _, slideNum := range slideFilter {
		args = append(args, "--slide", strconv.Itoa(slideNum))
	}

	cmd.SetArgs(args)

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
