package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestSlidesListMinimalTitle tests the slides list command with the minimal-title fixture
func TestSlidesListMinimalTitle(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesListCommand(t, "minimal-title")
}

// TestSlidesListTitleContent tests the slides list command with the title-content fixture
func TestSlidesListTitleContent(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesListCommand(t, "title-content")
}

// TestSlidesListTableSlide tests the slides list command with the table-slide fixture
func TestSlidesListTableSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesListCommand(t, "table-slide")

	fixtureFile := filepath.Join(getTestdataPath(), "pptx", "table-slide", "presentation.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "slides", "list", fixtureFile)
	if err != nil {
		t.Fatalf("slides list table-slide failed: %v", err)
	}
	var result SlidesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal slides list JSON: %v\n%s", err, output)
	}
	if len(result.Slides) < 2 {
		t.Fatalf("slide count = %d, want at least 2", len(result.Slides))
	}
	tableSlide := result.Slides[1]
	if tableSlide.Tables != 1 {
		t.Fatalf("slide 2 tables = %d, want 1; slide=%+v", tableSlide.Tables, tableSlide)
	}
	if tableSlide.SlideID == 0 {
		t.Fatalf("slide 2 missing slideId: %+v", tableSlide)
	}
	if tableSlide.RelationshipID == "" {
		t.Fatalf("slide 2 missing relationshipId: %+v", tableSlide)
	}
	if tableSlide.PrimarySelector != "2" {
		t.Fatalf("slide 2 primarySelector = %q, want 2", tableSlide.PrimarySelector)
	}
	for _, want := range []string{"2", "part:" + tableSlide.PartURI} {
		if !containsStringForSlidesTest(tableSlide.Selectors, want) {
			t.Fatalf("slide 2 selectors missing %q: %+v", want, tableSlide.Selectors)
		}
	}
	if tableSlide.LayoutNumber == 0 || tableSlide.LayoutPartURI == "" || tableSlide.LayoutReadbackCommand == "" {
		t.Fatalf("slide 2 missing layout handles: %+v", tableSlide)
	}
	for label, command := range map[string]string{
		"readback":  tableSlide.ReadbackCommand,
		"selectors": tableSlide.SelectorsCommand,
		"shapes":    tableSlide.ShapesCommand,
		"tables":    tableSlide.TablesCommand,
	} {
		if command == "" {
			t.Fatalf("slide 2 missing %s command: %+v", label, tableSlide)
		}
		out := executeGeneratedOOXMLCommandForSlidesTest(t, command)
		var payload map[string]any
		if err := json.Unmarshal([]byte(out), &payload); err != nil {
			t.Fatalf("%s command did not return JSON: %v\ncommand=%s\n%s", label, err, command, out)
		}
	}
}

// TestSlidesListNotesSlide tests the slides list command with the notes-slide fixture
func TestSlidesListNotesSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesListCommand(t, "notes-slide")

	fixtureFile := filepath.Join(getTestdataPath(), "pptx", "notes-slide", "presentation.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "slides", "list", fixtureFile)
	if err != nil {
		t.Fatalf("slides list notes-slide failed: %v", err)
	}
	var result SlidesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal notes slide list JSON: %v\n%s", err, output)
	}
	foundNotes := false
	for _, slide := range result.Slides {
		if slide.Notes {
			foundNotes = true
			if slide.NotesPartURI == "" {
				t.Fatalf("slide reports notes but missing notesPartUri: %+v", slide)
			}
		}
	}
	if !foundNotes {
		t.Fatalf("expected at least one slide with notes: %+v", result.Slides)
	}
}

// TestSlidesListPictureSlide tests the slides list command with the picture-placeholder fixture
func TestSlidesListPictureSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesListCommand(t, "picture-placeholder")
}

// TestSlidesShowMinimalTitle tests the slides show command with the minimal-title fixture
func TestSlidesShowMinimalTitle(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesShowCommand(t, "minimal-title", 1)
}

// TestSlidesShowTableSlide tests the slides show command with the table-slide fixture
func TestSlidesShowTableSlide(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	testSlidesShowCommand(t, "table-slide", 2)

	fixtureFile := filepath.Join(getTestdataPath(), "pptx", "table-slide", "presentation.pptx")
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "slides", "show", fixtureFile,
		"--slide", "2",
		"--include-text",
		"--include-bounds",
	)
	if err != nil {
		t.Fatalf("slides show table-slide failed: %v", err)
	}
	var result SlidesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal slides show JSON: %v\n%s", err, output)
	}
	if len(result.Slides) != 1 {
		t.Fatalf("slide result count = %d, want 1", len(result.Slides))
	}
	report := result.Slides[0]
	if report.SlideID == 0 || report.RelationshipID == "" {
		t.Fatalf("slide show missing durable handles: %+v", report)
	}
	if report.PrimarySelector != "2" {
		t.Fatalf("slide show primarySelector = %q, want 2", report.PrimarySelector)
	}
	for _, want := range []string{"2", "part:" + report.PartURI} {
		if !containsStringForSlidesTest(report.Selectors, want) {
			t.Fatalf("slide show selectors missing %q: %+v", want, report.Selectors)
		}
	}
	if report.LayoutPartURI == "" || report.LayoutNumber == 0 || report.LayoutReadbackCommand == "" {
		t.Fatalf("slide show missing layout handles: %+v", report)
	}
	if report.ReadbackCommand == "" || report.SelectorsCommand == "" || report.ShapesCommand == "" || report.TablesCommand == "" {
		t.Fatalf("slide show missing generated commands: %+v", report)
	}
	var tableFound, boundsFound bool
	for _, shape := range report.Shapes {
		if shape.Bounds != nil {
			boundsFound = true
		}
		if shape.TableInfo != nil {
			tableFound = true
			if shape.TableInfo.Rows != 3 || shape.TableInfo.Cols != 3 {
				t.Fatalf("table dimensions = %dx%d, want 3x3", shape.TableInfo.Rows, shape.TableInfo.Cols)
			}
		}
	}
	if !tableFound {
		t.Fatalf("slides show did not report table shape: %+v", report.Shapes)
	}
	if !boundsFound {
		t.Fatalf("slides show did not report any bounds: %+v", report.Shapes)
	}
}

// TestSlidesShowWithText tests the slides show command with --include-text flag
func TestSlidesShowWithText(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "slides", "show", fixtureFile, "--slide", "1", "--include-text", "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected output, got empty string")
	}

	// Validate JSON structure
	var result SlidesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}

	if len(result.Slides) == 0 || len(result.Slides[0].Shapes) == 0 {
		t.Fatalf("expected slide shapes, got %+v", result.Slides)
	}
	foundText := false
	for _, shape := range result.Slides[0].Shapes {
		if shape.TextContent != "" {
			foundText = true
			break
		}
	}
	if !foundText {
		t.Fatalf("expected at least one shape with text content: %+v", result.Slides[0].Shapes)
	}
}

// TestSlidesShowWithBounds tests the slides show command with --include-bounds flag
func TestSlidesShowWithBounds(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "slides", "show", fixtureFile, "--slide", "1", "--include-bounds"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("command failed: %v", err)
	}

	output := outBuf.String()
	if output == "" {
		t.Errorf("expected output, got empty string")
	}
}

// getTestdataPath returns the path to the testdata directory

// testSlidesListCommand runs the slides list command on a fixture
func testSlidesListCommand(t *testing.T, fixtureName string) {
	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", fixtureName)
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "slides", "list", fixtureFile})

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

// testSlidesShowCommand runs the slides show command on a fixture
func testSlidesShowCommand(t *testing.T, fixtureName string, slideNum int) {
	testdataDir := getTestdataPath()
	fixtureDir := filepath.Join(testdataDir, "pptx", fixtureName)
	fixtureFile := filepath.Join(fixtureDir, "presentation.pptx")

	if _, err := os.Stat(fixtureFile); err != nil {
		t.Skipf("fixture not found: %v", err)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "slides", "show", fixtureFile, "--slide", "1"})

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

// TestSlidesListJSON tests the slides list command with JSON output
func TestSlidesListJSON(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "slides", "list", fixtureFile, "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Validate JSON structure
	var result SlidesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}

	// Check that we have slides
	if len(result.Slides) == 0 {
		t.Errorf("expected slides in output, got %d", len(result.Slides))
	}

	// Check slide structure
	for i, slide := range result.Slides {
		if slide.Number == 0 {
			t.Errorf("slide %d: expected non-zero number", i)
		}
		if slide.PartURI == "" {
			t.Errorf("slide %d: expected non-empty PartURI", i)
		}
	}
}

// TestSlidesShowJSON tests the slides show command with JSON output
func TestSlidesShowJSON(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "slides", "show", fixtureFile, "--slide", "1", "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Validate JSON structure
	var result SlidesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}

	// Check that we have at least one slide
	if len(result.Slides) == 0 {
		t.Errorf("expected slides in output, got %d", len(result.Slides))
	}
}

// TestSlidesShowWithAllFlags tests the slides show command with all flags
func TestSlidesShowWithAllFlags(t *testing.T) {
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
	cmd.SetArgs([]string{"pptx", "slides", "show", fixtureFile, "--slide", "1", "--include-text", "--include-bounds", "--format", "json"})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()

	// Validate JSON structure
	var result SlidesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("expected valid JSON output, got: %v", err)
	}

	// Check that we got shapes
	if len(result.Slides) > 0 && len(result.Slides[0].Shapes) > 0 {
		for _, shape := range result.Slides[0].Shapes {
			_ = shape // Just check it deserializes correctly
		}
	}
}

func containsStringForSlidesTest(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func executeGeneratedOOXMLCommandForSlidesTest(t *testing.T, command string) string {
	t.Helper()
	if !strings.HasPrefix(command, "ooxml ") {
		t.Fatalf("generated command must start with ooxml: %s", command)
	}
	args := splitGeneratedOOXMLCommandForXLSXTest(t, command)[1:]
	output, err := executeRootForXLSXTest(t, args...)
	if err != nil {
		t.Fatalf("generated command failed: %v\ncommand=%s\noutput=%s", err, command, output)
	}
	return output
}
