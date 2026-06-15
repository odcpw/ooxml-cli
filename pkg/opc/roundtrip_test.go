package opc

import (
	"archive/zip"
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

// TestRoundtripPreservation tests that opening and saving a package preserves the original parts.
func TestRoundtripPreservation(t *testing.T) {
	testDataDir := "../../testdata/pptx"
	if _, err := os.Stat(testDataDir); err != nil {
		t.Skipf("testdata directory not found at %s, skipping roundtrip tests", testDataDir)
		return
	}

	err := filepath.Walk(testDataDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() || filepath.Ext(path) != ".pptx" {
			return nil
		}

		t.Run(filepath.Base(path), func(t *testing.T) {
			testRoundtripFile(t, path)
		})
		return nil
	})
	if err != nil {
		t.Fatalf("failed to walk testdata: %v", err)
	}
}

func testRoundtripFile(t *testing.T, inputPath string) {
	pkg, err := Open(inputPath)
	if err != nil {
		t.Fatalf("failed to open package: %v", err)
	}
	defer pkg.Close()

	tmpFile, err := os.CreateTemp("", "roundtrip-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}
	tmpPath := tmpFile.Name()
	if err := tmpFile.Close(); err != nil {
		t.Fatalf("failed to close temp file: %v", err)
	}
	defer os.Remove(tmpPath)

	if err := pkg.SaveAs(tmpPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	outputPkg, err := Open(tmpPath)
	if err != nil {
		t.Fatalf("failed to open output package (may be corrupted): %v", err)
	}
	defer outputPkg.Close()

	originalParts := pkg.ListParts()
	outputParts := outputPkg.ListParts()
	if len(originalParts) != len(outputParts) {
		t.Fatalf("part count mismatch: original=%d, output=%d", len(originalParts), len(outputParts))
	}

	outputPartMap := make(map[string]PartInfo, len(outputParts))
	for _, part := range outputParts {
		outputPartMap[part.URI] = part
	}

	for _, originalPart := range originalParts {
		if _, ok := outputPartMap[originalPart.URI]; !ok {
			t.Fatalf("missing part in output: %s", originalPart.URI)
		}

		originalBytes, err := pkg.ReadRawPart(originalPart.URI)
		if err != nil {
			t.Fatalf("failed to read original part %s: %v", originalPart.URI, err)
		}
		outputBytes, err := outputPkg.ReadRawPart(originalPart.URI)
		if err != nil {
			t.Fatalf("failed to read output part %s: %v", originalPart.URI, err)
		}
		if !bytes.Equal(originalBytes, outputBytes) {
			t.Fatalf("part %s differs between input and output", originalPart.URI)
		}
	}

	inputMethods, err := zipMethodsForRoundtrip(inputPath)
	if err != nil {
		t.Fatalf("failed to inspect input zip: %v", err)
	}
	outputMethods, err := zipMethodsForRoundtrip(tmpPath)
	if err != nil {
		t.Fatalf("failed to inspect output zip: %v", err)
	}
	for name, method := range inputMethods {
		if outputMethods[name] != method {
			t.Fatalf("compression method mismatch for %s: input=%d output=%d", name, method, outputMethods[name])
		}
	}

	if DetectType(pkg) != DetectType(outputPkg) {
		t.Fatalf("package type mismatch: original=%s, output=%s", DetectType(pkg), DetectType(outputPkg))
	}
}

func zipMethodsForRoundtrip(path string) (map[string]uint16, error) {
	zr, err := zip.OpenReader(path)
	if err != nil {
		return nil, err
	}
	defer zr.Close()

	methods := make(map[string]uint16, len(zr.File))
	for _, f := range zr.File {
		methods["/"+f.Name] = f.Method
	}
	return methods, nil
}

// BenchmarkRoundtrip benchmarks the roundtrip performance.
func BenchmarkRoundtrip(b *testing.B) {
	inputPath := "../../testdata/pptx/minimal-title/minimal-title.pptx"
	if _, err := os.Stat(inputPath); err != nil {
		b.Skipf("test file not found at %s", inputPath)
		return
	}

	b.ResetTimer()

	for i := 0; i < b.N; i++ {
		pkg, err := Open(inputPath)
		if err != nil {
			b.Fatalf("failed to open package: %v", err)
		}

		tmpFile, err := os.CreateTemp("", "bench-*.pptx")
		if err != nil {
			b.Fatalf("failed to create temp file: %v", err)
		}
		tmpPath := tmpFile.Name()
		_ = tmpFile.Close()

		err = pkg.SaveAs(tmpPath)
		_ = os.Remove(tmpPath)
		_ = pkg.Close()

		if err != nil {
			b.Fatalf("failed to save: %v", err)
		}
	}
}
