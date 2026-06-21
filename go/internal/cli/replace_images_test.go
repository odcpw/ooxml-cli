package cli

import (
	"bytes"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

func TestGetImageContentTypeRejectsUnsupportedExtension(t *testing.T) {
	_, err := getImageContentType("payload.bin")
	if err == nil {
		t.Fatal("expected unsupported image type error")
	}
	var cliErr *CLIError
	if !errors.As(err, &cliErr) || cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("expected unsupported-type CLI error, got %#v", err)
	}
}

// TestReplaceImages_WithOut tests replacing an image and writing to a new file
func TestReplaceImages_WithOut(t *testing.T) {
	// Create a temporary output directory
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")

	// Create a simple test image (1x1 PNG)
	testImage := []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}

	// Write test image to a temp file
	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, testImage, 0644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}

	// Create command
	cmd := &cobra.Command{}
	cmd.SetOut(new(bytes.Buffer))
	cmd.SetErr(new(bytes.Buffer))

	// Set flags
	rootCmd := getRootCmdForTest()
	args := []string{
		"pptx", "replace", "images",
		"testdata/pptx/picture-placeholder/presentation.pptx",
		"--target", "shape:2",
		"--image", imagePath,
		"--out", outputPath,
		"--format", "json",
	}

	// Use absolute path for test fixture
	testFixturePath, _ := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	args[3] = testFixturePath

	rootCmd.SetArgs(args)

	// Execute command
	err := rootCmd.Execute()
	if err != nil {
		t.Fatalf("command execution failed: %v", err)
	}

	// Verify output file was created
	if _, err := os.Stat(outputPath); err != nil {
		t.Fatalf("output file not created: %v", err)
	}

	// Verify output file has content
	outputInfo, err := os.Stat(outputPath)
	if err != nil {
		t.Fatalf("failed to stat output file: %v", err)
	}

	if outputInfo.Size() == 0 {
		t.Fatal("output file is empty")
	}
}

// TestReplaceImages_JSONOutput tests JSON output format
func TestReplaceImages_JSONOutput(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")

	testImage := []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}

	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, testImage, 0644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}

	jsonOutputPath := filepath.Join(tmpDir, "output.json")

	rootCmd := getRootCmdForTest()
	testFixturePath, _ := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	args := []string{
		"pptx", "replace", "images",
		testFixturePath,
		"--target", "shape:2",
		"--image", imagePath,
		"--out", outputPath,
		"--format", "json",
		"-o", jsonOutputPath,
	}

	rootCmd.SetArgs(args)

	err := rootCmd.Execute()
	if err != nil {
		t.Fatalf("command execution failed: %v", err)
	}

	// Read and verify JSON output
	jsonData, err := os.ReadFile(jsonOutputPath)
	if err != nil {
		t.Fatalf("failed to read JSON output: %v", err)
	}

	var result replaceImageResult
	if err := json.Unmarshal(jsonData, &result); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v", err)
	}

	if result.File != testFixturePath || result.Output != outputPath || result.DryRun {
		t.Fatalf("unexpected file/output metadata: %+v", result)
	}
	if result.Target != "shape:2" || result.FitMode != "contain" || result.SlideNumber != 2 || result.ShapeID != 2 {
		t.Fatalf("unexpected replacement metadata: %+v", result)
	}
	if result.NewContentType != "image/png" {
		t.Fatalf("expected content type image/png, got %s", result.NewContentType)
	}
	if result.Destination == nil {
		t.Fatal("destination is nil")
	}
	if result.Destination.File != outputPath || result.Destination.Slide != 2 || result.Destination.PrimarySelector != "shape:2" {
		t.Fatalf("unexpected destination metadata: %+v", result.Destination)
	}
	if !containsString(result.Destination.Selectors, "shape:2") || !containsString(result.Destination.Selectors, "~Picture 1") {
		t.Fatalf("unexpected destination selectors: %+v", result.Destination.Selectors)
	}
	if result.Destination.ImageRef == nil {
		t.Fatal("destination imageRef is nil")
	}
	if result.Destination.ImageRef.RelID != result.RelID || result.Destination.ImageRef.TargetURI != result.NewTargetURI || result.Destination.ImageRef.ContentType != result.NewContentType {
		t.Fatalf("unexpected destination imageRef: %+v result=%+v", result.Destination.ImageRef, result)
	}
	if result.Destination.Bounds == nil {
		t.Fatal("destination bounds are nil")
	}

	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outputPath, "pptx shapes get")
	var shapes PPTXShapesResult
	if err := json.Unmarshal([]byte(readback), &shapes); err != nil {
		t.Fatalf("failed to unmarshal shapes readback: %v\n%s", err, readback)
	}
	if len(shapes.Shapes) != 1 || shapes.Shapes[0].ImageRef == nil {
		t.Fatalf("unexpected shapes readback: %+v", shapes.Shapes)
	}
	if shapes.Shapes[0].ImageRef.TargetURI != result.NewTargetURI || shapes.Shapes[0].ImageRef.ContentType != "image/png" {
		t.Fatalf("unexpected readback imageRef: %+v", shapes.Shapes[0].ImageRef)
	}
}

func TestReplaceImagesSlideRestrictsSingleSlideSearch(t *testing.T) {
	tmpDir := t.TempDir()
	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, replaceImagesTestPNG(), 0o644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}
	fixturePath := filepath.Join(getTestdataPath(), "pptx", "picture-placeholder", "presentation.pptx")

	_, err := executeRootForReplaceImagesTest(t,
		"pptx", "replace", "images", fixturePath,
		"--slide", "1",
		"--target", "shape:2",
		"--image", imagePath,
		"--dry-run",
	)
	if err == nil {
		t.Fatal("expected --slide 1 to reject shape:2 because it is not a picture on that slide")
	}

	output, err := executeRootForReplaceImagesTest(t,
		"--format", "json",
		"pptx", "replace", "images", fixturePath,
		"--slide", "2",
		"--target", "shape:2",
		"--image", imagePath,
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("replace images on slide 2 failed: %v", err)
	}
	var result replaceImageResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal replace JSON: %v\n%s", err, output)
	}
	if result.SlideNumber != 2 || result.ShapeID != 2 {
		t.Fatalf("unexpected replace result: %+v", result)
	}
}

func TestReplaceImages_DryRunJSONIncludesDestinationReadback(t *testing.T) {
	tmpDir := t.TempDir()
	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, replaceImagesTestPNG(), 0o644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}
	reportPath := filepath.Join(tmpDir, "replace-images-dry-run.json")
	fixturePath := filepath.Join(getTestdataPath(), "pptx", "picture-placeholder", "presentation.pptx")

	cmd := getRootCmdForTest()
	var stdout bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetArgs([]string{
		"--format", "json",
		"-o", reportPath,
		"pptx", "replace", "images", fixturePath,
		"--slide", "2",
		"--target", "shape:2",
		"--image", imagePath,
		"--dry-run",
	})
	if err := cmd.Execute(); err != nil {
		t.Fatalf("replace images dry-run failed: %v", err)
	}
	if strings.TrimSpace(stdout.String()) != "" {
		t.Fatalf("stdout = %q, want empty because -o was used", stdout.String())
	}

	jsonData, err := os.ReadFile(reportPath)
	if err != nil {
		t.Fatalf("failed to read dry-run report: %v", err)
	}
	var result replaceImageResult
	if err := json.Unmarshal(jsonData, &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, jsonData)
	}
	if result.File != fixturePath || result.Output != "" || !result.DryRun {
		t.Fatalf("unexpected dry-run file/output metadata: %+v", result)
	}
	if result.Destination == nil || result.Destination.ImageRef == nil {
		t.Fatalf("missing dry-run destination imageRef: %+v", result.Destination)
	}
	if result.Destination.ImageRef.TargetURI != result.NewTargetURI || result.Destination.ImageRef.ContentType != result.NewContentType {
		t.Fatalf("unexpected dry-run destination imageRef: %+v result=%+v", result.Destination.ImageRef, result)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx shapes get")

	pkg, err := opc.Open(fixturePath)
	if err != nil {
		t.Fatalf("failed to open source fixture: %v", err)
	}
	defer pkg.Close()
	sourceBytes, err := pkg.ReadRawPart(result.NewTargetURI)
	if err != nil {
		t.Fatalf("failed to read source image part: %v", err)
	}
	if bytes.Equal(sourceBytes, replaceImagesTestPNG()) {
		t.Fatal("source image part unexpectedly equals dry-run replacement bytes")
	}
}

func TestReplaceImagesBatchUsesCommandOutputConfig(t *testing.T) {
	tmpDir := t.TempDir()
	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, replaceImagesTestPNG(), 0o644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}
	fixturePath := filepath.Join(getTestdataPath(), "pptx", "picture-placeholder", "presentation.pptx")

	output, err := executeRootForReplaceImagesTest(t,
		"--format", "json",
		"pptx", "replace", "images", fixturePath,
		"--for-slides", "1-2",
		"--target", "shape:2",
		"--image", imagePath,
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("batch replace images failed: %v", err)
	}
	var result struct {
		Target        string `json:"target"`
		TotalSlides   int    `json:"totalSlides"`
		SuccessCount  int    `json:"successCount"`
		NotFoundCount int    `json:"notFoundCount"`
	}
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal batch replace JSON: %v\n%s", err, output)
	}
	if result.Target != "shape:2" || result.TotalSlides != 2 || result.SuccessCount != 1 || result.NotFoundCount != 1 {
		t.Fatalf("unexpected batch result: %+v", result)
	}
}

func TestReplaceImagesDoesNotSwallowNonSearchErrors(t *testing.T) {
	tmpDir := t.TempDir()
	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, replaceImagesTestPNG(), 0o644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}
	fixturePath := filepath.Join(getTestdataPath(), "pptx", "picture-placeholder", "presentation.pptx")

	_, err := executeRootForReplaceImagesTest(t,
		"pptx", "replace", "images", fixturePath,
		"--target", "body",
		"--image", imagePath,
		"--dry-run",
	)
	if err == nil {
		t.Fatal("expected unsupported image selector error")
	}
	if !strings.Contains(err.Error(), "not supported for image replacement") {
		t.Fatalf("error = %v, want unsupported selector message", err)
	}
}

// TestReplaceImages_MissingTarget tests error handling for missing target
func TestReplaceImages_MissingTarget(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")

	testImage := []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}

	imagePath := filepath.Join(tmpDir, "test.png")
	if err := os.WriteFile(imagePath, testImage, 0644); err != nil {
		t.Fatalf("failed to write test image: %v", err)
	}

	rootCmd := getRootCmdForTest()
	testFixturePath, _ := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	args := []string{
		"pptx", "replace", "images",
		testFixturePath,
		"--target", "shape:9999", // Non-existent shape
		"--image", imagePath,
		"--out", outputPath,
	}

	rootCmd.SetArgs(args)

	// Command should fail
	err := rootCmd.Execute()
	if err == nil {
		t.Fatal("expected error for non-existent shape, got nil")
	}
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("error = %v, want target_not_found CLI error", err)
	}
	for _, want := range []string{
		"picture shape not found: shape:9999",
		"did you mean:",
		"shape:2",
		"ooxml --json pptx slides show <file> --include-bounds",
	} {
		if !strings.Contains(err.Error(), want) {
			t.Fatalf("error = %v, want substring %q", err, want)
		}
	}
}

// TestReplaceImages_MissingFlags tests error handling for missing flags
func TestReplaceImages_MissingFlags(t *testing.T) {
	rootCmd := getRootCmdForTest()
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")

	// Missing --image flag
	testFixturePath, _ := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	args := []string{
		"pptx", "replace", "images",
		testFixturePath,
		"--target", "shape:2",
		"--out", outputPath,
	}

	rootCmd.SetArgs(args)

	err := rootCmd.Execute()
	if err == nil {
		t.Fatal("expected error for missing --image flag, got nil")
	}
}

// getRootCmdForTest returns a root command for testing
func getRootCmdForTest() *cobra.Command {
	resetRootFlagsForReplaceImagesTest()
	resetCommandFlagsForReplaceImagesTest(replaceImagesCmd)
	cmd := GetRootCmd()
	cmd.SetOut(new(bytes.Buffer))
	cmd.SetErr(new(bytes.Buffer))
	return cmd
}

func executeRootForReplaceImagesTest(t *testing.T, args ...string) (string, error) {
	t.Helper()
	cmd := getRootCmdForTest()
	cmd.SetArgs(args)
	var output bytes.Buffer
	cmd.SetOut(&output)
	cmd.SetErr(new(bytes.Buffer))
	err := cmd.Execute()
	return output.String(), err
}

func resetRootFlagsForReplaceImagesTest() {
	resetFlags()
	flagOut = ""
	flagInPlace = ""
	flagBackup = ""

	cmd := GetRootCmd()
	for name, value := range map[string]string{
		"format":    "text",
		"verbosity": "normal",
		"output":    "",
		"temp-dir":  "",
		"out":       "",
		"in-place":  "",
		"backup":    "",
	} {
		_ = cmd.PersistentFlags().Set(name, value)
	}
	for _, name := range []string{"no-color", "pretty", "keep-temp", "strict"} {
		_ = cmd.PersistentFlags().Set(name, "false")
	}
}

func resetCommandFlagsForReplaceImagesTest(cmd *cobra.Command) {
	cmd.Flags().VisitAll(func(flag *pflag.Flag) {
		_ = cmd.Flags().Set(flag.Name, flag.DefValue)
		flag.Changed = false
	})
	for _, child := range cmd.Commands() {
		resetCommandFlagsForReplaceImagesTest(child)
	}
}

func replaceImagesTestPNG() []byte {
	return []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}
}
