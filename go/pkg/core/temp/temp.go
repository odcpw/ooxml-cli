// Package temp provides utilities for managing temporary directories with deferred cleanup.
package temp

import (
	"fmt"
	"os"
	"path/filepath"
)

// Dir represents a temporary directory that will be cleaned up on Close.
type Dir struct {
	Path string
}

// New creates a new temporary directory and returns a Dir that will clean it up.
// The caller should defer cleanup via Close().
func New() (*Dir, error) {
	path, err := os.MkdirTemp("", "ooxml-*")
	if err != nil {
		return nil, fmt.Errorf("failed to create temporary directory: %w", err)
	}
	return &Dir{Path: path}, nil
}

// NewWithPattern creates a new temporary directory with a custom pattern.
func NewWithPattern(pattern string) (*Dir, error) {
	path, err := os.MkdirTemp("", pattern)
	if err != nil {
		return nil, fmt.Errorf("failed to create temporary directory: %w", err)
	}
	return &Dir{Path: path}, nil
}

// NewInDir creates a new temporary directory inside a parent directory.
func NewInDir(parentDir, pattern string) (*Dir, error) {
	path, err := os.MkdirTemp(parentDir, pattern)
	if err != nil {
		return nil, fmt.Errorf("failed to create temporary directory in %s: %w", parentDir, err)
	}
	return &Dir{Path: path}, nil
}

// Close removes the temporary directory and all its contents.
func (d *Dir) Close() error {
	if d.Path == "" {
		return nil
	}
	return os.RemoveAll(d.Path)
}

// String returns the path of the temporary directory.
func (d *Dir) String() string {
	return d.Path
}

// Join appends path elements to the temporary directory path.
func (d *Dir) Join(elem ...string) string {
	return filepath.Join(append([]string{d.Path}, elem...)...)
}

// Create creates a subdirectory within the temporary directory.
func (d *Dir) Create(subdir string) error {
	fullPath := d.Join(subdir)
	return os.MkdirAll(fullPath, 0755)
}

// SubDir creates and returns a subdirectory path within the temporary directory.
func (d *Dir) SubDir(subdir string) (string, error) {
	fullPath := d.Join(subdir)
	if err := os.MkdirAll(fullPath, 0755); err != nil {
		return "", fmt.Errorf("failed to create subdirectory %s: %w", fullPath, err)
	}
	return fullPath, nil
}

// WriteFile writes data to a file within the temporary directory.
func (d *Dir) WriteFile(filename string, data []byte) error {
	fullPath := d.Join(filename)
	dir := filepath.Dir(fullPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory %s: %w", dir, err)
	}
	return os.WriteFile(fullPath, data, 0644)
}

// ReadFile reads a file from within the temporary directory.
func (d *Dir) ReadFile(filename string) ([]byte, error) {
	fullPath := d.Join(filename)
	return os.ReadFile(fullPath)
}
