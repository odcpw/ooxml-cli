package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func assertGoldenJSONValue(t *testing.T, name string, actual any) {
	t.Helper()
	path := filepath.Join("..", "..", "testdata", "golden", name)
	actualJSON, err := json.MarshalIndent(actual, "", "  ")
	if err != nil {
		t.Fatalf("marshal actual golden: %v", err)
	}
	actualJSON = append(actualJSON, '\n')
	if os.Getenv("UPDATE_GOLDENS") == "1" {
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatalf("create golden dir %s: %v", filepath.Dir(path), err)
		}
		if err := os.WriteFile(path, actualJSON, 0o644); err != nil {
			t.Fatalf("update golden %s: %v", path, err)
		}
	}
	expected, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden %s: %v", path, err)
	}
	if !bytes.Equal(normalizeGoldenLineEndings(expected), normalizeGoldenLineEndings(actualJSON)) {
		actualPath := path + ".actual"
		if err := os.WriteFile(actualPath, actualJSON, 0o644); err != nil {
			t.Fatalf("golden mismatch for %s; failed to write actual %s: %v", path, actualPath, err)
		}
		t.Fatalf("golden mismatch for %s\nactual written to %s", path, actualPath)
	}
}

func normalizeGoldenLineEndings(data []byte) []byte {
	return bytes.ReplaceAll(data, []byte("\r\n"), []byte("\n"))
}
