package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/spf13/cobra"
)

// Helper function to reset flags
func resetLayoutFlags() {
	layoutsListMasterFlag = 0
	layoutShowLayoutFlag = ""
	importLayoutSourcePath = ""
	importLayoutSelector = ""
	importLayoutThemePolicy = "reuse"
}

// TestLayoutsListMinimalTitle tests the layouts list command with minimal-title fixture
func TestLayoutsListMinimalTitle(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsListCmd()
	cmd.SetArgs([]string{filePath, "--format", "json"})

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

	// Validate JSON structure
	var result LayoutListOutput
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	// Basic validation
	if len(result.Layouts) == 0 {
		t.Errorf("expected layouts in output")
	}
	first := result.Layouts[0]
	if first.PrimarySelector == "" {
		t.Fatalf("first layout missing primarySelector: %+v", first)
	}
	if !containsString(first.Selectors, first.PrimarySelector) || !containsString(first.Selectors, first.Name) {
		t.Fatalf("first layout selectors should include primary selector and name: %+v", first)
	}

	// Compare with golden file
	goldenPath := "testdata/golden/layouts-list-minimal-title.json"
	compareWithGolden(t, output, goldenPath)
}

// TestLayoutsListMultiLayout tests the layouts list command with multi-layout fixture
func TestLayoutsListMultiLayout(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("multi-layout", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsListCmd()
	cmd.SetArgs([]string{filePath, "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	goldenPath := "testdata/golden/layouts-list-multi-layout.json"
	compareWithGolden(t, output, goldenPath)
}

// TestLayoutsListMasterFilter tests the --master flag
func TestLayoutsListMasterFilter(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("multi-layout", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsListCmd()
	cmd.SetArgs([]string{filePath, "--format", "json", "--master", "1"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	var result LayoutListOutput
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	// Check that only layouts from master-1 are returned
	for _, layout := range result.Layouts {
		if layout.MasterID != "" && layout.MasterID != "master-1" {
			t.Errorf("expected master-1, got %s", layout.MasterID)
		}
	}
}

// TestLayoutsShowByNumber tests the show command with layout number
func TestLayoutsShowByNumber(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsShowCmd()
	cmd.SetArgs([]string{filePath, "--layout", "1", "--format", "json"})

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

	// Validate JSON structure
	var result LayoutShowOutput
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	goldenPath := "testdata/golden/layouts-show-minimal-title-1.json"
	compareWithGolden(t, output, goldenPath)
}

// TestLayoutsShowByName tests the show command with layout name
func TestLayoutsShowByName(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("multi-layout", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsShowCmd()
	cmd.SetArgs([]string{filePath, "--layout", "Title Slide", "--format", "json"})

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

	// Validate JSON structure
	var result LayoutShowOutput
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	if result.Name != "Title Slide" {
		t.Errorf("expected layout name 'Title Slide', got '%s'", result.Name)
	}
}

// TestLayoutsShowMissingLayout tests error handling for missing layout
func TestLayoutsShowMissingLayout(t *testing.T) {
	resetLayoutFlags()

	cmd := getLayoutsShowCmd()
	cmd.SetArgs([]string{"testdata/pptx/minimal-title/presentation.pptx", "--layout", "999"})

	var errBuf bytes.Buffer
	cmd.SetErr(&errBuf)

	err := cmd.Execute()
	if err == nil {
		t.Errorf("expected error for missing layout")
	}
}

// TestLayoutsShowMissingRequired tests that --layout is required
func TestLayoutsShowMissingRequired(t *testing.T) {
	resetLayoutFlags()

	cmd := getLayoutsShowCmd()
	cmd.SetArgs([]string{"testdata/pptx/minimal-title/presentation.pptx"})

	var errBuf bytes.Buffer
	cmd.SetErr(&errBuf)

	err := cmd.Execute()
	if err == nil {
		t.Errorf("expected error for missing --layout flag")
	}
}

// TestLayoutsListText tests text output format
func TestLayoutsListText(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsListCmd()
	cmd.SetArgs([]string{filePath})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected text output")
	}

	// Check for expected format: [number] name
	if !bytes.Contains([]byte(output), []byte("[1]")) {
		t.Errorf("expected '[1]' in output")
	}

	if !bytes.Contains([]byte(output), []byte("placeholders:")) {
		t.Errorf("expected 'placeholders:' in output")
	}
}

// TestLayoutsShowText tests text output format
func TestLayoutsShowText(t *testing.T) {
	resetLayoutFlags()

	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := getLayoutsShowCmd()
	cmd.SetArgs([]string{filePath, "--layout", "1"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected text output")
	}

	// Check for expected format
	if !bytes.Contains([]byte(output), []byte("Layout:")) {
		t.Errorf("expected 'Layout:' in output")
	}

	if !bytes.Contains([]byte(output), []byte("Placeholders:")) {
		t.Errorf("expected 'Placeholders:' in output")
	}
}

// Helper functions

func getLayoutsListCmd() *cobra.Command {
	// Create a fresh command with proper setup
	cmd := &cobra.Command{
		Use:   "list",
		Short: "List layouts",
		RunE:  layoutsListCmd.RunE,
	}

	cmd.Flags().IntVarP(
		&layoutsListMasterFlag,
		"master",
		"",
		0,
		"filter layouts by master number",
	)
	cmd.Flags().StringVarP(
		&cmdFormat,
		"format",
		"f",
		"text",
		"output format",
	)

	return cmd
}

func getLayoutsShowCmd() *cobra.Command {
	// Create a fresh command with proper setup
	cmd := &cobra.Command{
		Use:   "show",
		Short: "Show layout",
		RunE:  layoutsShowCmd.RunE,
	}

	cmd.Flags().StringVarP(
		&layoutShowLayoutFlag,
		"layout",
		"l",
		"",
		"layout number or name",
	)
	cmd.Flags().StringVarP(
		&cmdFormat,
		"format",
		"f",
		"text",
		"output format",
	)

	return cmd
}

func compareWithGolden(t *testing.T, actual, goldenPath string) {
	// Get the project root directory using runtime.Caller
	_, currentFile, _, _ := runtime.Caller(0)
	projectRoot := filepath.Dir(filepath.Dir(filepath.Dir(currentFile)))

	// Construct the full path to the golden file
	fullGoldenPath := filepath.Join(projectRoot, goldenPath)

	var actualJSON interface{}
	if err := json.Unmarshal([]byte(actual), &actualJSON); err != nil {
		t.Fatalf("failed to parse actual JSON: %v", err)
	}
	actualJSON = normalizeGoldenPaths(actualJSON)
	actualBytes, err := json.MarshalIndent(actualJSON, "", "  ")
	if err != nil {
		t.Fatalf("failed to marshal actual JSON: %v", err)
	}
	actualBytes = append(actualBytes, '\n')

	if os.Getenv("UPDATE_GOLDENS") == "1" {
		if err := os.MkdirAll(filepath.Dir(fullGoldenPath), 0o755); err != nil {
			t.Fatalf("failed to create golden dir: %v", err)
		}
		if err := os.WriteFile(fullGoldenPath, actualBytes, 0o644); err != nil {
			t.Fatalf("failed to update golden file: %v", err)
		}
	}

	// Read expected output
	expected, err := os.ReadFile(fullGoldenPath)
	if err != nil {
		t.Fatalf("failed to read golden file: %v", err)
	}

	// Parse both as JSON to avoid formatting differences
	var expectedJSON interface{}

	err = json.Unmarshal(expected, &expectedJSON)
	if err != nil {
		t.Fatalf("failed to parse expected JSON: %v", err)
	}

	expectedJSON = normalizeGoldenPaths(expectedJSON)

	// Marshal both back to JSON for comparison
	expectedBytes, _ := json.MarshalIndent(expectedJSON, "", "  ")
	expectedBytes = append(expectedBytes, '\n')

	if !bytes.Equal(actualBytes, expectedBytes) {
		t.Errorf("output mismatch with golden file %s\nExpected:\n%s\n\nActual:\n%s\n",
			goldenPath, string(expectedBytes), string(actualBytes))
	}
}

func normalizeGoldenPaths(value interface{}) interface{} {
	switch typed := value.(type) {
	case map[string]interface{}:
		for key, child := range typed {
			typed[key] = normalizeGoldenPaths(child)
		}
		return typed
	case []interface{}:
		for i, child := range typed {
			typed[i] = normalizeGoldenPaths(child)
		}
		return typed
	case string:
		return scrubGoldenPath(typed)
	default:
		return value
	}
}

func scrubGoldenPath(path string) string {
	normalized := filepath.ToSlash(path)
	for {
		idx := strings.Index(normalized, "/testdata/")
		if idx < 0 {
			return normalizeQuotedGoldenCommandPaths(normalized)
		}
		tokenStart := idx
		for tokenStart > 0 && !isGoldenPathBoundary(normalized[tokenStart-1]) {
			tokenStart--
		}
		normalized = normalized[:tokenStart] + "testdata/" + normalized[idx+len("/testdata/"):]
	}
}

func normalizeQuotedGoldenCommandPaths(value string) string {
	value = strings.ReplaceAll(value, "'testdata/", "testdata/")
	for _, ext := range []string{".pptx", ".pptm", ".xlsx", ".xlsm", ".docx", ".docm"} {
		value = strings.ReplaceAll(value, ext+"'", ext)
	}
	return value
}

func isGoldenPathBoundary(b byte) bool {
	switch b {
	case ' ', '\t', '\n', '\r', '"', '\'':
		return true
	default:
		return false
	}
}

// Global variable for format flag (used by test commands)
var cmdFormat string
