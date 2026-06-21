package opc

import (
	"archive/zip"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestSaveAs_DoesNotUseDataDescriptorsForWrittenEntries(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	if _, err := os.Stat(inputPath); err != nil {
		t.Skipf("test fixture not found at %s", inputPath)
		return
	}

	pkg, err := Open(inputPath)
	if err != nil {
		t.Fatalf("failed to open package: %v", err)
	}
	defer pkg.Close()

	slideURI := "/ppt/slides/slide1.xml"
	raw, err := pkg.ReadRawPart(slideURI)
	if err != nil {
		t.Fatalf("failed to read slide part: %v", err)
	}
	if err := pkg.ReplaceRawPart(slideURI, raw, pkg.GetContentType(slideURI)); err != nil {
		t.Fatalf("failed to mark slide as dirty: %v", err)
	}

	tmpFile, err := os.CreateTemp("", "opc-saveas-compat-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	tmpPath := tmpFile.Name()
	tmpFile.Close()
	defer os.Remove(tmpPath)

	if err := pkg.SaveAs(tmpPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	zr, err := zip.OpenReader(tmpPath)
	if err != nil {
		t.Fatalf("failed to reopen saved package: %v", err)
	}
	defer zr.Close()

	flagsByName := map[string]uint16{}
	for _, f := range zr.File {
		flagsByName["/"+strings.TrimPrefix(f.Name, "/")] = f.Flags
	}

	for _, name := range []string{"/[Content_Types].xml", slideURI} {
		flags, ok := flagsByName[name]
		if !ok {
			t.Fatalf("missing zip entry %s", name)
		}
		if flags&0x8 != 0 {
			t.Fatalf("zip entry %s unexpectedly uses data descriptor flag: 0x%x", name, flags)
		}
	}
}

func TestSaveAs_WritesOfficeCompatibleZipVersions(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	if _, err := os.Stat(inputPath); err != nil {
		t.Skipf("test fixture not found at %s", inputPath)
		return
	}

	pkg, err := Open(inputPath)
	if err != nil {
		t.Fatalf("failed to open package: %v", err)
	}
	defer pkg.Close()

	sheetURI := "/xl/worksheets/sheet1.xml"
	raw, err := pkg.ReadRawPart(sheetURI)
	if err != nil {
		t.Fatalf("failed to read sheet part: %v", err)
	}
	if err := pkg.ReplaceRawPart(sheetURI, raw, pkg.GetContentType(sheetURI)); err != nil {
		t.Fatalf("failed to mark sheet as dirty: %v", err)
	}

	tmpFile, err := os.CreateTemp("", "opc-saveas-version-*.xlsx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	tmpPath := tmpFile.Name()
	tmpFile.Close()
	defer os.Remove(tmpPath)

	if err := pkg.SaveAs(tmpPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	zr, err := zip.OpenReader(tmpPath)
	if err != nil {
		t.Fatalf("failed to reopen saved package: %v", err)
	}
	defer zr.Close()

	for _, f := range zr.File {
		if f.Method == zip.Deflate && f.ReaderVersion < 20 {
			t.Fatalf("zip entry %s ReaderVersion = %d, want at least 20 for deflate", f.Name, f.ReaderVersion)
		}
		if f.CreatorVersion&0xff == 0 {
			t.Fatalf("zip entry %s CreatorVersion = %d, want non-zero ZIP version", f.Name, f.CreatorVersion)
		}
	}
}
