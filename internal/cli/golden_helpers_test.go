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
		if err := os.WriteFile(path, actualJSON, 0o644); err != nil {
			t.Fatalf("update golden %s: %v", path, err)
		}
	}
	expected, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden %s: %v", path, err)
	}
	if !bytes.Equal(normalizeGoldenLineEndings(expected), normalizeGoldenLineEndings(actualJSON)) {
		t.Fatalf("golden mismatch for %s\nexpected:\n%s\nactual:\n%s", path, expected, actualJSON)
	}
}

func normalizeGoldenLineEndings(data []byte) []byte {
	return bytes.ReplaceAll(data, []byte("\r\n"), []byte("\n"))
}
