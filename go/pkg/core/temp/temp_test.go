package temp

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestNew(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	// Verify the directory exists
	if _, err := os.Stat(d.Path); err != nil {
		t.Fatalf("Temp directory does not exist: %v", err)
	}

	// Verify it's a directory
	fi, err := os.Stat(d.Path)
	if err != nil || !fi.IsDir() {
		t.Errorf("Path is not a directory: %v", err)
	}
}

func TestNewWithPattern(t *testing.T) {
	d, err := NewWithPattern("custom-*")
	if err != nil {
		t.Fatalf("NewWithPattern() failed: %v", err)
	}
	defer d.Close()

	// Verify the directory exists and starts with "custom-"
	if _, err := os.Stat(d.Path); err != nil {
		t.Fatalf("Temp directory does not exist: %v", err)
	}

	baseName := filepath.Base(d.Path)
	if !bytes.HasPrefix([]byte(baseName), []byte("custom-")) {
		t.Errorf("Pattern not applied: expected 'custom-*', got %s", baseName)
	}
}

func TestClose(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}

	dirPath := d.Path

	// Verify directory exists
	if _, err := os.Stat(dirPath); err != nil {
		t.Fatalf("Temp directory does not exist: %v", err)
	}

	// Close the directory
	if err := d.Close(); err != nil {
		t.Fatalf("Close() failed: %v", err)
	}

	// Verify directory is removed
	if _, err := os.Stat(dirPath); err == nil {
		t.Errorf("Temp directory still exists after Close()")
	}
}

func TestCreate(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	// Create a subdirectory
	subdir := "subdir"
	if err := d.Create(subdir); err != nil {
		t.Fatalf("Create() failed: %v", err)
	}

	// Verify the subdirectory exists
	fullPath := filepath.Join(d.Path, subdir)
	if _, err := os.Stat(fullPath); err != nil {
		t.Fatalf("Subdirectory does not exist: %v", err)
	}
}

func TestCreateNested(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	// Create nested subdirectories
	nested := filepath.Join("a", "b", "c")
	if err := d.Create(nested); err != nil {
		t.Fatalf("Create() failed: %v", err)
	}

	// Verify the nested subdirectories exist
	fullPath := filepath.Join(d.Path, nested)
	if _, err := os.Stat(fullPath); err != nil {
		t.Fatalf("Nested subdirectory does not exist: %v", err)
	}
}

func TestJoin(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	result := d.Join("a", "b", "c.txt")
	expected := filepath.Join(d.Path, "a", "b", "c.txt")
	if result != expected {
		t.Errorf("Join() mismatch: expected %q, got %q", expected, result)
	}
}

func TestWriteFile(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	testData := []byte("Hello, Temp!")
	if err := d.WriteFile("test.txt", testData); err != nil {
		t.Fatalf("WriteFile() failed: %v", err)
	}

	// Verify the file exists and has correct content
	fullPath := filepath.Join(d.Path, "test.txt")
	content, err := os.ReadFile(fullPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}

func TestWriteFileNested(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	testData := []byte("Nested file content")
	nestedPath := filepath.Join("nested", "dir", "file.txt")
	if err := d.WriteFile(nestedPath, testData); err != nil {
		t.Fatalf("WriteFile() failed: %v", err)
	}

	// Verify the file exists and has correct content
	fullPath := filepath.Join(d.Path, nestedPath)
	content, err := os.ReadFile(fullPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}

func TestReadFile(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	testData := []byte("File content to read")
	if err := d.WriteFile("test.txt", testData); err != nil {
		t.Fatalf("WriteFile() failed: %v", err)
	}

	// Read the file back
	content, err := d.ReadFile("test.txt")
	if err != nil {
		t.Fatalf("ReadFile() failed: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}

func TestString(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	if d.String() != d.Path {
		t.Errorf("String() mismatch: expected %q, got %q", d.Path, d.String())
	}
}

func TestSubDir(t *testing.T) {
	d, err := New()
	if err != nil {
		t.Fatalf("New() failed: %v", err)
	}
	defer d.Close()

	subdir := "mysubdir"
	result, err := d.SubDir(subdir)
	if err != nil {
		t.Fatalf("SubDir() failed: %v", err)
	}

	// Verify the subdirectory exists
	if _, err := os.Stat(result); err != nil {
		t.Fatalf("SubDir does not exist: %v", err)
	}

	expected := filepath.Join(d.Path, subdir)
	if result != expected {
		t.Errorf("SubDir() path mismatch: expected %q, got %q", expected, result)
	}
}
