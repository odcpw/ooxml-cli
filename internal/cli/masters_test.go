package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/stretchr/testify/require"
)

// TestMastersListMinimalTitle tests the masters list command with minimal-title fixture
func TestMastersListMinimalTitle(t *testing.T) {
	resetFlags()
	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--format", "json", "pptx", "masters", "list", filePath})

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
	var result MasterListResult
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	if len(result.Masters) == 0 {
		t.Errorf("expected masters in output")
	}
	first := result.Masters[0]
	if first.PrimarySelector == "" || !containsString(first.Selectors, first.PrimarySelector) {
		t.Fatalf("first master missing selector fields: %+v", first)
	}

	// Compare with golden file
	goldenPath := "testdata/golden/masters-list-minimal-title.json"
	compareWithGolden(t, output, goldenPath)
}

// TestMastersListMultiLayout tests the masters list command with multi-layout fixture
func TestMastersListMultiLayout(t *testing.T) {
	resetFlags()
	filePath := getTestFilePath("multi-layout", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--format", "json", "pptx", "masters", "list", filePath})

	var outBuf bytes.Buffer
	cmd.SetOut(&outBuf)

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("command failed: %v", err)
	}

	output := outBuf.String()
	goldenPath := "testdata/golden/masters-list-multi-layout.json"
	compareWithGolden(t, output, goldenPath)
}

// TestMastersShowMinimalTitle tests the show command with minimal-title fixture
func TestMastersShowMinimalTitle(t *testing.T) {
	resetFlags()
	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--format", "json", "pptx", "masters", "show", filePath, "--master", "1"})

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
	var result MasterDetail
	err = json.Unmarshal([]byte(output), &result)
	if err != nil {
		t.Fatalf("failed to parse JSON output: %v", err)
	}

	goldenPath := "testdata/golden/masters-show-minimal-title-1.json"
	compareWithGolden(t, output, goldenPath)
}

// TestMastersShowMultiLayout tests the show command with multi-layout fixture
func TestMastersShowMultiLayout(t *testing.T) {
	resetFlags()
	filePath := getTestFilePath("multi-layout", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--format", "json", "pptx", "masters", "show", filePath, "--master", "1"})

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

	goldenPath := "testdata/golden/masters-show-multi-layout-1.json"
	compareWithGolden(t, output, goldenPath)
}

func TestMastersImportCLI(t *testing.T) {
	resetFlags()
	targetPath := getTestFilePath("minimal-title", "presentation.pptx")
	sourceFixture := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(targetPath); err != nil {
		t.Skipf("target fixture not found: %v", err)
	}
	if _, err := os.Stat(sourceFixture); err != nil {
		t.Skipf("source fixture not found: %v", err)
	}
	sourcePath := filepath.Join(t.TempDir(), "source-import-master.pptx")
	sourcePkg, err := opc.Open(sourceFixture)
	require.NoError(t, err)
	_, err = mutate.RenameLayout(&mutate.RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	require.NoError(t, sourcePkg.SaveAs(sourcePath))
	require.NoError(t, sourcePkg.Close())

	outPath := filepath.Join(t.TempDir(), "imported-master.pptx")

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "masters", "import", targetPath, "--source", sourcePath, "--master", "1", "--theme-policy", "import", "--out", outPath})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	masters, err := ParsePresentationMasters(pkg)
	require.NoError(t, err)
	if len(masters) < 2 {
		t.Fatalf("expected imported master to be registered, got %d masters", len(masters))
	}
}

// TestMastersShowInvalidMaster tests error handling for invalid master number
func TestMastersShowInvalidMaster(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "masters", "show", "testdata/pptx/minimal-title/presentation.pptx", "--master", "999"})

	var errBuf bytes.Buffer
	cmd.SetErr(&errBuf)

	err := cmd.Execute()
	if err == nil {
		t.Errorf("expected error for invalid master number")
	}
}

// TestMastersListText tests text output format
func TestMastersListText(t *testing.T) {
	resetFlags()
	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "masters", "list", filePath})

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

	// Check for expected format: [number] uri
	if !bytes.Contains([]byte(output), []byte("[1]")) {
		t.Errorf("expected '[1]' in output")
	}

	if !bytes.Contains([]byte(output), []byte("layouts:")) {
		t.Errorf("expected 'layouts:' in output")
	}
}

// TestMastersShowText tests text output format for show
func TestMastersShowText(t *testing.T) {
	resetFlags()
	filePath := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("fixture not found: %v", err)
	}
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "masters", "show", filePath, "--master", "1"})

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
	if !bytes.Contains([]byte(output), []byte("Master:")) {
		t.Errorf("expected 'Master:' in output")
	}

	if !bytes.Contains([]byte(output), []byte("Layouts:")) {
		t.Errorf("expected 'Layouts:' in output")
	}

	if !bytes.Contains([]byte(output), []byte("Shapes:")) {
		t.Errorf("expected 'Shapes:' in output")
	}
}

// TestMastersListFileNotFound tests error handling for missing file
func TestMastersListFileNotFound(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "masters", "list", "nonexistent.pptx"})

	var errBuf bytes.Buffer
	cmd.SetErr(&errBuf)

	err := cmd.Execute()
	if err == nil {
		t.Errorf("expected error for missing file")
	}
}
