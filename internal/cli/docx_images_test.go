package cli

import (
	"bytes"
	"encoding/json"
	"image"
	"image/jpeg"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestDOCXImagesCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	docx := findSubcommand(cmd, "docx")
	if docx == nil {
		t.Fatal("docx command is not registered")
	}
	images := findSubcommand(docx, "images")
	if images == nil {
		t.Fatal("docx images command is not registered")
	}
	for _, name := range []string{"list", "replace", "insert"} {
		if command := findSubcommand(images, name); command == nil {
			t.Fatalf("docx images %s command is not registered", name)
		}
	}
}

func TestDOCXImagesListJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "images", "list", documentPath)
	if err != nil {
		t.Fatalf("docx images list failed: %v", err)
	}

	var result DOCXImagesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal images list JSON: %v\n%s", err, output)
	}
	if result.DocumentPartURI != "/word/document.xml" || len(result.Images) != 1 {
		t.Fatalf("unexpected list result: %+v", result)
	}
	image := result.Images[0]
	if image.Index != 1 || image.ID != "rId10" || image.MediaURI != "/word/media/image1.png" || image.ContentType != "image/png" {
		t.Fatalf("unexpected image report: %+v", image)
	}
	if image.Width != 914400 || image.Height != 914400 {
		t.Fatalf("unexpected EMU extent: %dx%d", image.Width, image.Height)
	}
	if image.BlockIndex != 2 || image.BlockID != "body.b2" || image.BlockHash == "" {
		t.Fatalf("unexpected block anchoring: %+v", image)
	}
	if image.PrimarySelector != "1" || !containsString(image.Selectors, "1") {
		t.Fatalf("unexpected image selectors: primary=%q selectors=%+v", image.PrimarySelector, image.Selectors)
	}
}

func TestDOCXImagesListText(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")

	output, err := executeRootForXLSXTest(t, "docx", "images", "list", documentPath)
	if err != nil {
		t.Fatalf("docx images list failed: %v", err)
	}
	if want := "image 1: /word/media/image1.png (914400x914400)"; !strings.Contains(output, want) {
		t.Fatalf("list text = %q, want %q", output, want)
	}
}

func TestDOCXImagesListRejectsNonDOCX(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	_, err := executeRootForXLSXTest(t, "docx", "images", "list", workbookPath)
	if err == nil {
		t.Fatal("expected unsupported type error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitUnsupportedType)
	}
}

func TestDOCXImageContentTypeRejectsUnsupportedExtension(t *testing.T) {
	_, err := docxImageContentType("payload.bin")
	if cliErr, ok := AsCLIError(err); !ok || cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("expected unsupported type CLI error, got %#v", err)
	}
}

func TestDOCXImagesReplaceHashGuardedReadbackAndValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")
	outPath := filepath.Join(t.TempDir(), "replaced.docx")
	newImage := writeTestPNG(t)
	// The inline image lives in body block 2; replace anchors to that block.
	hash := docxImageBlockHashForTest(t, documentPath, 2)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "images", "replace", documentPath,
		"--image", "1",
		"--file", newImage,
		"--expect-hash", hash,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx images replace failed: %v", err)
	}
	var result DOCXImagesReplaceResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal replace JSON: %v\n%s", err, output)
	}
	if result.Index != 1 || result.ID != "rId10" || result.BlockIndex != 2 || result.PreviousURI != "/word/media/image1.png" || result.NewURI != "/word/media/image1.png" {
		t.Fatalf("unexpected replace result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "images", "list", outPath)
	if err != nil {
		t.Fatalf("readback failed: %v", err)
	}
	var listResult DOCXImagesListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal readback: %v\n%s", err, readback)
	}
	if len(listResult.Images) != 1 || listResult.Images[0].MediaURI != "/word/media/image1.png" {
		t.Fatalf("unexpected readback: %+v", listResult.Images)
	}
}

func TestDOCXImagesReplaceResizeAndContentType(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")
	outPath := filepath.Join(t.TempDir(), "resized.docx")
	newImage := writeTestJPEG(t)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "images", "replace", documentPath,
		"--image", "1",
		"--file", newImage,
		"--width", "1828800",
		"--height", "914400",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx images replace failed: %v", err)
	}
	var result DOCXImagesReplaceResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal replace JSON: %v\n%s", err, output)
	}
	if result.NewContentType != "image/jpeg" || result.NewURI != "/word/media/image1.jpeg" || result.PreviousURI != "/word/media/image1.png" {
		t.Fatalf("unexpected content-type swap: %+v", result)
	}
	if result.Width != 1828800 || result.Height != 914400 {
		t.Fatalf("unexpected resize: %dx%d", result.Width, result.Height)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "images", "list", outPath)
	if err != nil {
		t.Fatalf("readback failed: %v", err)
	}
	var listResult DOCXImagesListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal readback: %v\n%s", err, readback)
	}
	if len(listResult.Images) != 1 {
		t.Fatalf("unexpected image count: %+v", listResult.Images)
	}
	img := listResult.Images[0]
	if img.MediaURI != "/word/media/image1.jpeg" || img.ContentType != "image/jpeg" || img.Width != 1828800 || img.Height != 914400 {
		t.Fatalf("unexpected readback image: %+v", img)
	}
}

func TestDOCXImagesReplaceRejectsBadHash(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")
	newImage := writeTestPNG(t)

	args := []string{
		"docx", "images", "replace", documentPath,
		"--image", "1",
		"--file", newImage,
		"--expect-hash", "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		"--dry-run",
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestDOCXImagesReplaceRejectsMissingImage(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")
	newImage := writeTestPNG(t)

	args := []string{
		"docx", "images", "replace", documentPath,
		"--image", "99",
		"--file", newImage,
		"--dry-run",
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitTargetNotFound)
}

func TestDOCXImagesInsertAfterBlockHashGuarded(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")
	outPath := filepath.Join(t.TempDir(), "inserted.docx")
	newImage := writeTestPNG(t)
	hash := docxImageBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "images", "insert", documentPath,
		"--after", "1",
		"--file", newImage,
		"--width", "914400",
		"--height", "914400",
		"--expect-hash", hash,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx images insert failed: %v", err)
	}
	var result DOCXImagesInsertResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal insert JSON: %v\n%s", err, output)
	}
	if result.Index != 2 || result.InsertAfter != 1 || result.AnchorHash != hash || result.MediaURI != "/word/media/image2.png" || result.Width != 914400 || result.Height != 914400 {
		t.Fatalf("unexpected insert result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "images", "list", outPath)
	if err != nil {
		t.Fatalf("readback failed: %v", err)
	}
	var listResult DOCXImagesListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal readback: %v\n%s", err, readback)
	}
	if len(listResult.Images) != 2 {
		t.Fatalf("expected 2 images after insert, got %+v", listResult.Images)
	}
	// New image becomes block 2; the pre-existing image shifts to block 3.
	if listResult.Images[0].MediaURI != "/word/media/image2.png" || listResult.Images[0].BlockIndex != 2 {
		t.Fatalf("unexpected first image after insert: %+v", listResult.Images[0])
	}
}

func TestDOCXImagesInsertDryRunDoesNotWrite(t *testing.T) {
	documentPath := filepath.Join(t.TempDir(), "document.docx")
	if err := copyFile(getDOCXTestFilePath("with-image"), documentPath); err != nil {
		t.Fatalf("failed to copy fixture: %v", err)
	}
	newImage := writeTestPNG(t)
	hash := docxImageBlockHashForTest(t, documentPath, 1)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "images", "insert", documentPath,
		"--after", "1",
		"--file", newImage,
		"--width", "914400",
		"--height", "914400",
		"--expect-hash", hash,
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx images insert dry-run failed: %v", err)
	}
	var result DOCXImagesInsertResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal insert dry-run JSON: %v\n%s", err, output)
	}
	if result.Index != 2 {
		t.Fatalf("unexpected dry-run result: %+v", result)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "images", "list", documentPath)
	if err != nil {
		t.Fatalf("readback failed: %v", err)
	}
	var listResult DOCXImagesListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal readback: %v\n%s", err, readback)
	}
	if len(listResult.Images) != 1 {
		t.Fatalf("dry-run wrote to document: %+v", listResult.Images)
	}
}

func TestDOCXImagesInsertRejectsOutOfBounds(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-image")
	newImage := writeTestPNG(t)

	args := []string{
		"docx", "images", "insert", documentPath,
		"--after", "99",
		"--file", newImage,
		"--width", "914400",
		"--height", "914400",
		"--expect-hash", "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		"--dry-run",
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitTargetNotFound)
}

func docxImageBlockHashForTest(t *testing.T, documentPath string, block int) string {
	t.Helper()
	return docxBlockHashForTest(t, documentPath, block)
}

func writeTestPNG(t *testing.T) string {
	t.Helper()
	// 1x1 PNG.
	data := []byte{
		0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
		0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00,
		0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
		0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB4, 0x00, 0x00,
		0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
	}
	path := filepath.Join(t.TempDir(), "new.png")
	if err := os.WriteFile(path, data, 0o644); err != nil {
		t.Fatalf("failed to write test PNG: %v", err)
	}
	return path
}

func writeTestJPEG(t *testing.T) string {
	t.Helper()
	var buf bytes.Buffer
	if err := jpeg.Encode(&buf, image.NewRGBA(image.Rect(0, 0, 1, 1)), nil); err != nil {
		t.Fatalf("failed to encode test JPEG: %v", err)
	}
	path := filepath.Join(t.TempDir(), "new.jpg")
	if err := os.WriteFile(path, buf.Bytes(), 0o644); err != nil {
		t.Fatalf("failed to write test JPEG: %v", err)
	}
	return path
}
