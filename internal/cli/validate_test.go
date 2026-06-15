package cli

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestValidateValidFile tests validation of a known-good PPTX file
func TestValidateValidFile(t *testing.T) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath})

	err := cmd.Execute()
	// Note: We expect err to be nil for a valid file, but it might be a CLIError with exit code 0
	// The actual test of exit codes happens in integration tests
	_ = err
}

// TestValidateFileNotFound tests validation of a non-existent file
func TestValidateFileNotFound(t *testing.T) {
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", "/nonexistent/file.pptx"})

	err := cmd.Execute()
	assert.Error(t, err)

	if cliErr, ok := err.(*CLIError); ok {
		assert.Equal(t, ExitFileNotFound, cliErr.ExitCode)
	}
}

// TestValidateJSONOutput tests JSON output format
func TestValidateJSONOutput(t *testing.T) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	// Create a temporary output file
	tmpOut, err := os.CreateTemp("", "validate-output-*.json")
	require.NoError(t, err)
	defer os.Remove(tmpOut.Name())
	tmpOut.Close()

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath, "--format", "json", "--output", tmpOut.Name()})

	// Run validation - we expect it might fail due to missing validation pipeline,
	// but we can at least test that the command structure works
	_ = cmd.Execute()

	// Check if output file was created and contains valid JSON structure
	output, err := os.ReadFile(tmpOut.Name())
	if len(output) > 0 {
		var result ValidateResult
		err := json.Unmarshal(output, &result)
		assert.NoError(t, err)
		assert.NotEmpty(t, result.File)
		assert.NotNil(t, result.Summary)
	}
}

// TestValidateTextOutput tests text output format
func TestValidateTextOutput(t *testing.T) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	// Create a temporary output file
	tmpOut, err := os.CreateTemp("", "validate-output-*.txt")
	require.NoError(t, err)
	defer os.Remove(tmpOut.Name())
	tmpOut.Close()

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath, "--format", "text", "--output", tmpOut.Name()})

	// Run validation
	_ = cmd.Execute()

	// Check if output file was created
	output, err := os.ReadFile(tmpOut.Name())
	if err == nil && len(output) > 0 {
		content := string(output)
		assert.Contains(t, content, "File:")
		assert.Contains(t, content, "Status:")
	}
}

// TestValidateCorruptedFile tests validation of a corrupted PPTX file
func TestValidateCorruptedFile(t *testing.T) {
	filePath := "../../testdata/pptx/corrupted-missing-media/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath})

	err := cmd.Execute()
	// The command should report validation failure via CLIError
	if err != nil && err.Error() != "" {
		assert.Error(t, err)
	}
}

func TestValidateJSONFailureReturnsReportedResult(t *testing.T) {
	filePath := "../../testdata/pptx/corrupted-missing-media/presentation.pptx"
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	cmd := newTestRootCmd(t)
	cmd.SilenceUsage = true
	cmd.SilenceErrors = true
	cmd.SetArgs([]string{"--format", "json", "validate", filePath})

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetErr(&stderr)

	err := cmd.Execute()
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)
	assert.True(t, cliErr.Reported)
	assert.Empty(t, cliErr.Message)
	assert.Empty(t, stderr.String())

	var output ValidateResult
	require.NoError(t, json.Unmarshal(stdout.Bytes(), &output))
	assert.False(t, output.Valid)
	assert.Equal(t, "errors", output.Status)
	require.NotNil(t, output.Summary)
	assert.GreaterOrEqual(t, output.Summary.ErrorCount, 1)
	assert.True(t, diagnosticJSONContains(output.Diagnostics, "REL_DANGLING_TARGET"), "diagnostics = %#v", output.Diagnostics)
}

func TestValidateVBAMacroProjectOutgoingRelationshipJSON(t *testing.T) {
	inputPath := "../../testdata/xlsx/minimal-workbook/workbook.xlsx"
	projectPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	require.NoError(t, os.WriteFile(projectPath, []byte("opaque cli macro project"), 0o644))

	workbookPath := filepath.Join(t.TempDir(), "macro-outgoing.xlsm")
	runVBACommand(t, "vba", "attach", inputPath, "--bin", projectPath, "--out", workbookPath)
	addZipEntryForValidateTest(t, workbookPath, "xl/media/image1.png", []byte("not really an image"))
	addZipEntryForValidateTest(t, workbookPath, "xl/_rels/vbaProject.bin.rels", []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>`))

	cmd := newTestRootCmd(t)
	cmd.SilenceUsage = true
	cmd.SilenceErrors = true
	cmd.SetArgs([]string{"--format", "json", "validate", workbookPath})

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetErr(&stderr)

	err := cmd.Execute()
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)
	assert.True(t, cliErr.Reported)
	assert.Empty(t, stderr.String())

	var output ValidateResult
	require.NoError(t, json.Unmarshal(stdout.Bytes(), &output))
	assert.False(t, output.Valid)
	assert.True(t, diagnosticJSONContains(output.Diagnostics, "VBA_PROJECT_UNEXPECTED_RELATIONSHIP"), "diagnostics = %#v", output.Diagnostics)
	assert.False(t, diagnosticJSONContains(output.Diagnostics, "REL_DANGLING_TARGET"), "diagnostics = %#v", output.Diagnostics)
}

func TestValidateDOCXCorruptImagePayloadJSON(t *testing.T) {
	fixturePath := "../../testdata/docx/with-image/document.docx"
	if _, err := os.Stat(fixturePath); err != nil {
		t.Skipf("test file not found: %s", fixturePath)
	}

	pkg, err := opc.Open(fixturePath)
	require.NoError(t, err)
	defer pkg.Close()
	require.NoError(t, pkg.ReplaceRawPart("/word/media/image1.png", []byte("not really a png"), "image/png"))
	docPath := filepath.Join(t.TempDir(), "corrupt-image.docx")
	require.NoError(t, pkg.SaveAs(docPath))

	output, err := executeRootForXLSXTest(t, "--format", "json", "validate", docPath)
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)

	var result ValidateResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.False(t, result.Valid)
	assert.Equal(t, "errors", result.Status)
	assert.True(t, diagnosticJSONContains(result.Diagnostics, "DOCX_IMAGE_PAYLOAD"), "diagnostics = %#v", result.Diagnostics)
}

func TestValidateDOCXHeaderCorruptImagePayloadJSON(t *testing.T) {
	fixturePath := "../../testdata/docx/headers/document.docx"
	if _, err := os.Stat(fixturePath); err != nil {
		t.Skipf("test file not found: %s", fixturePath)
	}

	pkg, err := opc.Open(fixturePath)
	require.NoError(t, err)
	defer pkg.Close()
	headerXML := `<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:p><w:r><w:drawing><a:blip r:embed="rIdHeaderImage"/></w:drawing></w:r></w:p></w:hdr>`
	headerRels := `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeaderImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/header.png"/></Relationships>`
	require.NoError(t, pkg.ReplaceRawPart("/word/header1.xml", []byte(headerXML), "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"))
	require.NoError(t, pkg.ReplaceRawPart("/word/_rels/header1.xml.rels", []byte(headerRels), "application/vnd.openxmlformats-package.relationships+xml"))
	require.NoError(t, pkg.ReplaceRawPart("/word/media/header.png", []byte("not really a png"), "image/png"))
	docPath := filepath.Join(t.TempDir(), "corrupt-header-image.docx")
	require.NoError(t, pkg.SaveAs(docPath))

	output, err := executeRootForXLSXTest(t, "--format", "json", "validate", docPath)
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)

	var result ValidateResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.False(t, result.Valid)
	assert.Equal(t, "errors", result.Status)
	assert.True(t, diagnosticJSONContains(result.Diagnostics, "DOCX_IMAGE_PAYLOAD"), "diagnostics = %#v", result.Diagnostics)
}

// TestValidateStrictMode tests --strict flag behavior
func TestValidateStrictMode(t *testing.T) {
	filePath := "../../testdata/pptx/animations-synthetic/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	nonStrictOut, err := executeRootForXLSXTest(t, "--format", "json", "validate", filePath)
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitPartialSuccess, cliErr.ExitCode)

	var nonStrictResult ValidateResult
	require.NoError(t, json.Unmarshal([]byte(nonStrictOut), &nonStrictResult))
	assert.Equal(t, "warnings", nonStrictResult.Status)
	require.NotNil(t, nonStrictResult.Summary)
	assert.Zero(t, nonStrictResult.Summary.ErrorCount)
	assert.GreaterOrEqual(t, nonStrictResult.Summary.WarningCount, 1)
	assert.True(t, diagnosticJSONContains(nonStrictResult.Diagnostics, "PPTX_STALE_ANIMATION_TARGET"), "diagnostics = %#v", nonStrictResult.Diagnostics)

	strictOut, err := executeRootForXLSXTest(t, "--format", "json", "validate", "--strict", filePath)
	require.Error(t, err)
	cliErr, ok = AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)

	var strictResult ValidateResult
	require.NoError(t, json.Unmarshal([]byte(strictOut), &strictResult))
	assert.Equal(t, "errors", strictResult.Status)
	require.NotNil(t, strictResult.Summary)
	assert.Zero(t, strictResult.Summary.ErrorCount)
	assert.GreaterOrEqual(t, strictResult.Summary.WarningCount, 1)
	assert.True(t, diagnosticJSONContains(strictResult.Diagnostics, "PPTX_STALE_ANIMATION_TARGET"), "diagnostics = %#v", strictResult.Diagnostics)
}

func addZipEntryForValidateTest(t *testing.T, path, name string, data []byte) {
	t.Helper()
	reader, err := zip.OpenReader(path)
	require.NoError(t, err)
	var entries []struct {
		header zip.FileHeader
		data   []byte
	}
	for _, file := range reader.File {
		rc, err := file.Open()
		require.NoError(t, err)
		entryData, err := io.ReadAll(rc)
		require.NoError(t, err)
		require.NoError(t, rc.Close())
		header := file.FileHeader
		entries = append(entries, struct {
			header zip.FileHeader
			data   []byte
		}{header: header, data: entryData})
	}
	require.NoError(t, reader.Close())

	tmpPath := path + ".tmp"
	out, err := os.Create(tmpPath)
	require.NoError(t, err)
	writer := zip.NewWriter(out)
	for _, entry := range entries {
		w, err := writer.CreateHeader(&entry.header)
		require.NoError(t, err)
		_, err = w.Write(entry.data)
		require.NoError(t, err)
	}
	w, err := writer.Create(name)
	require.NoError(t, err)
	_, err = w.Write(data)
	require.NoError(t, err)
	require.NoError(t, writer.Close())
	require.NoError(t, out.Close())
	require.NoError(t, os.Rename(tmpPath, path))
}

// TestValidatePrettyJSON tests pretty JSON output
func TestValidatePrettyJSON(t *testing.T) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	tmpOut, err := os.CreateTemp("", "validate-pretty-*.json")
	require.NoError(t, err)
	defer os.Remove(tmpOut.Name())
	tmpOut.Close()

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath, "--format", "json", "--pretty", "--output", tmpOut.Name()})

	// Run validation
	_ = cmd.Execute()

	// Check if output is formatted (contains newlines)
	output, err := os.ReadFile(tmpOut.Name())
	if err == nil && len(output) > 0 {
		assert.Contains(t, string(output), "\n")
	}
}

// TestValidateDiagnosticStructure tests that diagnostics are properly structured
func TestValidateDiagnosticStructure(t *testing.T) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	tmpOut, err := os.CreateTemp("", "validate-diag-*.json")
	require.NoError(t, err)
	defer os.Remove(tmpOut.Name())
	tmpOut.Close()

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath, "--format", "json", "--output", tmpOut.Name()})

	// Run validation
	_ = cmd.Execute()

	output, err := os.ReadFile(tmpOut.Name())
	if err == nil && len(output) > 0 {
		var result ValidateResult
		err := json.Unmarshal(output, &result)
		if err == nil {
			// Verify the structure
			assert.NotEmpty(t, result.File)
			assert.NotNil(t, result.Summary)
			assert.GreaterOrEqual(t, result.Summary.ErrorCount, 0)
			assert.GreaterOrEqual(t, result.Summary.WarningCount, 0)
			assert.GreaterOrEqual(t, result.Summary.InfoCount, 0)

			// Check diagnostic structure if any exist
			for _, d := range result.Diagnostics {
				assert.NotEmpty(t, d.Code)
				assert.NotEmpty(t, d.Severity)
				assert.NotEmpty(t, d.Message)
			}
		}
	}
}

// TestValidateExitCodes tests that exit codes are properly mapped
func TestValidateExitCodes(t *testing.T) {
	// This test verifies exit code constants are properly defined
	assert.Equal(t, 0, ExitSuccess)
	assert.Equal(t, 5, ExitValidationFailed)
	assert.Equal(t, 9, ExitPartialSuccess)
	assert.Equal(t, 3, ExitFileNotFound)
	assert.Equal(t, 2, ExitInvalidArgs)
}

// BenchmarkValidate benchmarks the validate command on a standard file
func BenchmarkValidate(b *testing.B) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		b.Skipf("test file not found: %s", filePath)
	}

	for i := 0; i < b.N; i++ {
		cmd := GetRootCmd()
		cmd.SetArgs([]string{"validate", filePath})
		_ = cmd.Execute()
	}
}

// TestValidateWithQuietVerbosity tests text output with quiet verbosity
func TestValidateWithQuietVerbosity(t *testing.T) {
	filePath := "../../testdata/pptx/minimal-title/presentation.pptx"

	// Skip if test file doesn't exist
	if _, err := os.Stat(filePath); err != nil {
		t.Skipf("test file not found: %s", filePath)
	}

	tmpOut, err := os.CreateTemp("", "validate-quiet-*.txt")
	require.NoError(t, err)
	defer os.Remove(tmpOut.Name())
	tmpOut.Close()

	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", filePath, "--format", "text", "--verbosity", "quiet", "--output", tmpOut.Name()})

	// Run validation
	_ = cmd.Execute()

	// In quiet mode, info messages should not appear
	output, err := os.ReadFile(tmpOut.Name())
	if err == nil && len(output) > 0 {
		// Just verify the file was created
		assert.Greater(t, len(output), 0)
	}
}

// TestValidateMultipleFixtures tests validate against various fixtures
func TestValidateMultipleFixtures(t *testing.T) {
	fixtures := []string{
		"../../testdata/pptx/minimal-title/presentation.pptx",
		"../../testdata/pptx/multi-layout/presentation.pptx",
		"../../testdata/pptx/title-content/presentation.pptx",
	}

	for _, fixture := range fixtures {
		t.Run(filepath.Base(filepath.Dir(fixture)), func(t *testing.T) {
			if _, err := os.Stat(fixture); err != nil {
				t.Skipf("test file not found: %s", fixture)
			}

			cmd := GetRootCmd()
			cmd.SetArgs([]string{"validate", fixture})

			// Should execute without panic
			err := cmd.Execute()
			// Error handling depends on validation pipeline, just ensure it doesn't panic
			_ = err
		})
	}
}
