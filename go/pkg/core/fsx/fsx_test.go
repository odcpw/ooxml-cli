package fsx

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"testing"
)

func TestWriteAtomic(t *testing.T) {
	// Create a temporary directory for the test
	tmpDir, err := os.MkdirTemp("", "fsx-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp directory: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	targetPath := filepath.Join(tmpDir, "test.txt")
	testData := []byte("Hello, World!")

	// Write the file atomically
	err = WriteAtomic(targetPath, testData)
	if err != nil {
		t.Fatalf("WriteAtomic failed: %v", err)
	}

	// Verify the file was created
	if _, err := os.Stat(targetPath); err != nil {
		t.Fatalf("File was not created: %v", err)
	}

	// Verify the content
	content, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}

func TestWriteAtomicOverwrite(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "fsx-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp directory: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	targetPath := filepath.Join(tmpDir, "test.txt")

	// Write initial content
	initialData := []byte("Initial content")
	if err := WriteAtomic(targetPath, initialData); err != nil {
		t.Fatalf("Initial write failed: %v", err)
	}

	// Overwrite with new content
	newData := []byte("Overwritten content")
	if err := WriteAtomic(targetPath, newData); err != nil {
		t.Fatalf("Overwrite failed: %v", err)
	}

	// Verify the new content
	content, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, newData) {
		t.Errorf("Content mismatch: expected %q, got %q", newData, content)
	}
}

func TestWriteAtomicCreateDir(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "fsx-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp directory: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	// Create a path with non-existent subdirectories
	targetPath := filepath.Join(tmpDir, "a", "b", "c", "test.txt")
	testData := []byte("Nested directory test")

	err = WriteAtomic(targetPath, testData)
	if err != nil {
		t.Fatalf("WriteAtomic failed: %v", err)
	}

	// Verify the file was created
	content, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}

func TestWriteAtomicWithPerm(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "fsx-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp directory: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	targetPath := filepath.Join(tmpDir, "test.txt")
	testData := []byte("Permission test")
	perm := os.FileMode(0644)

	err = WriteAtomicWithPerm(targetPath, testData, perm)
	if err != nil {
		t.Fatalf("WriteAtomicWithPerm failed: %v", err)
	}

	// Verify permissions
	fi, err := os.Stat(targetPath)
	if err != nil {
		t.Fatalf("Failed to stat file: %v", err)
	}
	if got := fi.Mode().Perm(); got != perm {
		if runtime.GOOS == "windows" && got&0o600 == 0o600 {
			return
		}
		t.Errorf("Permission mismatch: expected %o, got %o", perm, got)
	}

	// Verify content
	content, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}

func TestWriteAtomicFromReader(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "fsx-test-*")
	if err != nil {
		t.Fatalf("Failed to create temp directory: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	targetPath := filepath.Join(tmpDir, "test.txt")
	testData := []byte("Data from reader")
	reader := bytes.NewReader(testData)

	err = WriteAtomicFromReader(targetPath, reader)
	if err != nil {
		t.Fatalf("WriteAtomicFromReader failed: %v", err)
	}

	// Verify content
	content, err := os.ReadFile(targetPath)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}
	if !bytes.Equal(content, testData) {
		t.Errorf("Content mismatch: expected %q, got %q", testData, content)
	}
}
