package output

import (
	"bytes"
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestNewWriter_Text(t *testing.T) {
	buf := &bytes.Buffer{}
	cfg := Config{
		Format: FormatText,
		Stdout: buf,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	if err := w.WriteString("hello"); err != nil {
		t.Fatalf("WriteString() error: %v", err)
	}

	if err := w.Close(); err != nil {
		t.Fatalf("Close() error: %v", err)
	}

	got := strings.TrimSpace(buf.String())
	if got != "hello" {
		t.Errorf("WriteString() got %q, want %q", got, "hello")
	}
}

func TestNewWriter_JSON(t *testing.T) {
	buf := &bytes.Buffer{}
	cfg := Config{
		Format: FormatJSON,
		Pretty: false,
		Stdout: buf,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	obj := map[string]interface{}{"key": "value"}
	if err := w.WriteObject(obj); err != nil {
		t.Fatalf("WriteObject() error: %v", err)
	}

	if err := w.Close(); err != nil {
		t.Fatalf("Close() error: %v", err)
	}

	output := strings.TrimSpace(buf.String())
	var result map[string]interface{}
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Errorf("Output is not valid JSON: %v", err)
	}
}

func TestNewWriter_FileOutput(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.txt")

	cfg := Config{
		Format:     FormatText,
		OutputPath: outputPath,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	if err := w.WriteString("test content"); err != nil {
		t.Fatalf("WriteString() error: %v", err)
	}

	if err := w.Close(); err != nil {
		t.Fatalf("Close() error: %v", err)
	}

	// Verify file was created and contains the expected content
	content, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("Failed to read output file: %v", err)
	}

	got := strings.TrimSpace(string(content))
	if got != "test content" {
		t.Errorf("File content mismatch: got %q, want %q", got, "test content")
	}
}

func TestNewWriter_FileOutput_CreatesDirectory(t *testing.T) {
	tmpDir := t.TempDir()

	// Use a nested path that doesn't exist yet
	outputPath := filepath.Join(tmpDir, "subdir", "nested", "output.txt")
	cfg := Config{
		Format:     FormatText,
		OutputPath: outputPath,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	if err := w.WriteString("test"); err != nil {
		t.Fatalf("WriteString() error: %v", err)
	}

	if err := w.Close(); err != nil {
		t.Fatalf("Close() error: %v", err)
	}

	// Verify file was created
	if _, err := os.Stat(outputPath); err != nil {
		t.Errorf("Output file not created: %v", err)
	}
}

func TestNewWriter_InvalidFormat(t *testing.T) {
	buf := &bytes.Buffer{}
	cfg := Config{
		Format: Format("invalid"),
		Stdout: buf,
	}

	_, err := NewWriter(cfg)
	if err == nil {
		t.Errorf("NewWriter() should fail with invalid format")
	}
}

func TestNewWriter_InvalidFormatClosesOutputFile(t *testing.T) {
	outputPath := filepath.Join(t.TempDir(), "output.txt")
	before := openFDCountForPath(t, outputPath)

	_, err := NewWriter(Config{
		Format:     Format("invalid"),
		OutputPath: outputPath,
	})
	if err == nil {
		t.Fatal("NewWriter() should fail with invalid format")
	}

	after := openFDCountForPath(t, outputPath)
	if after != before {
		t.Fatalf("open fd count for %s = %d, want %d", outputPath, after, before)
	}
}

func openFDCountForPath(t *testing.T, path string) int {
	t.Helper()
	entries, err := os.ReadDir("/proc/self/fd")
	if err != nil {
		t.Skipf("/proc/self/fd unavailable: %v", err)
	}
	want := filepath.Clean(path)
	count := 0
	for _, entry := range entries {
		target, err := os.Readlink(filepath.Join("/proc/self/fd", entry.Name()))
		if err != nil {
			continue
		}
		if filepath.Clean(target) == want {
			count++
		}
	}
	return count
}

func TestNewWriter_DefaultFormat(t *testing.T) {
	buf := &bytes.Buffer{}
	cfg := Config{
		Stdout: buf,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	if err := w.WriteString("test"); err != nil {
		t.Fatalf("WriteString() error: %v", err)
	}

	// Should succeed with default text format
}

func TestWriter_WriteTable(t *testing.T) {
	buf := &bytes.Buffer{}
	cfg := Config{
		Format: FormatText,
		Stdout: buf,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	rows := []map[string]interface{}{
		{"Name": "Alice", "Age": 30},
		{"Name": "Bob", "Age": 25},
	}

	if err := w.WriteTable(rows); err != nil {
		t.Fatalf("WriteTable() error: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "Alice") || !strings.Contains(output, "Bob") {
		t.Errorf("Table output missing expected content")
	}
}

func TestWriter_WriteRaw(t *testing.T) {
	buf := &bytes.Buffer{}
	cfg := Config{
		Format: FormatJSON,
		Stdout: buf,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	data := []byte("raw content")
	if err := w.WriteRaw(data); err != nil {
		t.Fatalf("WriteRaw() error: %v", err)
	}

	got := strings.TrimSpace(buf.String())
	if got != "raw content" {
		t.Errorf("WriteRaw() got %q, want %q", got, "raw content")
	}
}

func TestGetWriterFromContext_NoConfig(t *testing.T) {
	ctx := context.Background()

	w, err := GetWriterFromContext(ctx)
	if err != nil {
		t.Fatalf("GetWriterFromContext() error: %v", err)
	}

	// Should return a writer with defaults (text format)
	if w == nil {
		t.Errorf("GetWriterFromContext() returned nil writer")
	}
}

func TestGetWriterFromContext_WithConfig(t *testing.T) {
	cfg := &GlobalConfig{
		Format: string(FormatJSON),
		Pretty: true,
	}

	ctx := context.WithValue(context.Background(), "config", cfg)

	w, err := GetWriterFromContext(ctx)
	if err != nil {
		t.Fatalf("GetWriterFromContext() error: %v", err)
	}

	if w == nil {
		t.Errorf("GetWriterFromContext() returned nil writer")
	}
}

func TestGetWriterFromContext_InvalidConfig(t *testing.T) {
	// Store a non-GlobalConfig value
	ctx := context.WithValue(context.Background(), "config", "not a config")

	w, err := GetWriterFromContext(ctx)
	if err != nil {
		t.Fatalf("GetWriterFromContext() error: %v", err)
	}

	// Should fallback to defaults
	if w == nil {
		t.Errorf("GetWriterFromContext() returned nil writer")
	}
}

func TestConfig_TextAndJSON(t *testing.T) {
	tests := []struct {
		name   string
		format Format
		pretty bool
		check  func(t *testing.T, output string)
	}{
		{
			name:   "text format",
			format: FormatText,
			check: func(t *testing.T, output string) {
				// Text format should produce simple output
				if output == "" {
					t.Errorf("Text output should not be empty")
				}
			},
		},
		{
			name:   "json compact",
			format: FormatJSON,
			pretty: false,
			check: func(t *testing.T, output string) {
				var result interface{}
				if err := json.Unmarshal([]byte(output), &result); err != nil {
					t.Errorf("Output should be valid JSON: %v", err)
				}
			},
		},
		{
			name:   "json pretty",
			format: FormatJSON,
			pretty: true,
			check: func(t *testing.T, output string) {
				var result interface{}
				if err := json.Unmarshal([]byte(output), &result); err != nil {
					t.Errorf("Output should be valid JSON: %v", err)
				}
				if !strings.Contains(output, "  ") {
					t.Errorf("Pretty JSON should have indentation")
				}
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buf := &bytes.Buffer{}
			cfg := Config{
				Format: tt.format,
				Pretty: tt.pretty,
				Stdout: buf,
			}

			w, err := NewWriter(cfg)
			if err != nil {
				t.Fatalf("NewWriter() error: %v", err)
			}

			obj := map[string]interface{}{"test": "data"}
			if err := w.WriteObject(obj); err != nil {
				t.Fatalf("WriteObject() error: %v", err)
			}

			output := strings.TrimSpace(buf.String())
			tt.check(t, output)
		})
	}
}

func TestWriter_Integration(t *testing.T) {
	// Test a complete workflow
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "report.json")

	cfg := Config{
		Format:     FormatJSON,
		Pretty:     true,
		OutputPath: outputPath,
	}

	w, err := NewWriter(cfg)
	if err != nil {
		t.Fatalf("NewWriter() error: %v", err)
	}

	// Write some data
	rows := []map[string]interface{}{
		{"ID": 1, "Name": "Item A"},
		{"ID": 2, "Name": "Item B"},
	}

	if err := w.WriteTable(rows); err != nil {
		t.Fatalf("WriteTable() error: %v", err)
	}

	if err := w.Close(); err != nil {
		t.Fatalf("Close() error: %v", err)
	}

	// Read and verify
	content, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("Failed to read output file: %v", err)
	}

	var result []interface{}
	if err := json.Unmarshal(content, &result); err != nil {
		t.Fatalf("Output is not valid JSON: %v", err)
	}

	if len(result) != 2 {
		t.Errorf("Expected 2 rows, got %d", len(result))
	}
}

func TestJSONPrettyVsCompact(t *testing.T) {
	obj := map[string]interface{}{
		"name":  "test",
		"value": 123,
	}

	// Compact
	compactBuf := &bytes.Buffer{}
	compactCfg := Config{
		Format: FormatJSON,
		Pretty: false,
		Stdout: compactBuf,
	}
	compactW, _ := NewWriter(compactCfg)
	compactW.WriteObject(obj)
	compactW.Close()
	compactOutput := strings.TrimSpace(compactBuf.String())

	// Pretty
	prettyBuf := &bytes.Buffer{}
	prettyCfg := Config{
		Format: FormatJSON,
		Pretty: true,
		Stdout: prettyBuf,
	}
	prettyW, _ := NewWriter(prettyCfg)
	prettyW.WriteObject(obj)
	prettyW.Close()
	prettyOutput := strings.TrimSpace(prettyBuf.String())

	// Verify both are valid JSON
	var compactResult, prettyResult map[string]interface{}
	if err := json.Unmarshal([]byte(compactOutput), &compactResult); err != nil {
		t.Errorf("Compact JSON invalid: %v", err)
	}
	if err := json.Unmarshal([]byte(prettyOutput), &prettyResult); err != nil {
		t.Errorf("Pretty JSON invalid: %v", err)
	}

	// Pretty should have more newlines
	if strings.Count(prettyOutput, "\n") <= strings.Count(compactOutput, "\n") {
		t.Errorf("Pretty output should have more newlines than compact")
	}
}
