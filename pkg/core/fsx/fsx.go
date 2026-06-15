// Package fsx provides file system utilities, including atomic write operations.
package fsx

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
)

// WriteAtomic writes data to a file atomically using a write-to-temp-then-rename pattern.
// This ensures that if the process crashes during writing, the target file remains unchanged.
//
// The function:
// 1. Creates a temporary file in the same directory as the target
// 2. Writes all data to the temp file
// 3. Syncs the temp file to disk
// 4. Renames the temp file to the target path (atomic on most filesystems)
func WriteAtomic(path string, data []byte) error {
	// Get the directory and create temp file in the same directory
	dir := filepath.Dir(path)
	if dir == "" {
		dir = "."
	}

	// Create the directory if it doesn't exist
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory %s: %w", dir, err)
	}

	// Create a temporary file in the same directory
	tmpFile, err := os.CreateTemp(dir, ".tmp-*")
	if err != nil {
		return fmt.Errorf("failed to create temporary file in %s: %w", dir, err)
	}
	tmpPath := tmpFile.Name()
	defer os.Remove(tmpPath) // Clean up temp file if something goes wrong

	// Write all data to the temporary file
	if _, err := tmpFile.Write(data); err != nil {
		tmpFile.Close()
		return fmt.Errorf("failed to write to temporary file %s: %w", tmpPath, err)
	}

	// Sync to ensure data is written to disk
	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
		return fmt.Errorf("failed to sync temporary file %s: %w", tmpPath, err)
	}

	// Close the file before renaming
	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("failed to close temporary file %s: %w", tmpPath, err)
	}

	// Atomically rename the temp file to the target path
	if err := os.Rename(tmpPath, path); err != nil {
		return fmt.Errorf("failed to rename %s to %s: %w", tmpPath, path, err)
	}

	return nil
}

// WriteAtomicWithPerm writes data to a file atomically and sets the file permissions.
func WriteAtomicWithPerm(path string, data []byte, perm os.FileMode) error {
	if err := WriteAtomic(path, data); err != nil {
		return err
	}
	return os.Chmod(path, perm)
}

// WriteAtomicFromReader writes data from a reader to a file atomically.
func WriteAtomicFromReader(path string, reader io.Reader) error {
	dir := filepath.Dir(path)
	if dir == "" {
		dir = "."
	}

	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory %s: %w", dir, err)
	}

	tmpFile, err := os.CreateTemp(dir, ".tmp-*")
	if err != nil {
		return fmt.Errorf("failed to create temporary file in %s: %w", dir, err)
	}
	tmpPath := tmpFile.Name()
	defer os.Remove(tmpPath)

	if _, err := io.Copy(tmpFile, reader); err != nil {
		tmpFile.Close()
		return fmt.Errorf("failed to write to temporary file %s: %w", tmpPath, err)
	}

	if err := tmpFile.Sync(); err != nil {
		tmpFile.Close()
		return fmt.Errorf("failed to sync temporary file %s: %w", tmpPath, err)
	}

	if err := tmpFile.Close(); err != nil {
		return fmt.Errorf("failed to close temporary file %s: %w", tmpPath, err)
	}

	if err := os.Rename(tmpPath, path); err != nil {
		return fmt.Errorf("failed to rename %s to %s: %w", tmpPath, path, err)
	}

	return nil
}
