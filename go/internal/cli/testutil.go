package cli

import (
	"os"
	"path/filepath"
	"runtime"
)

// getTestdataPath returns the path to the testdata directory.
func getTestdataPath() string {
	_, currentFile, _, _ := runtime.Caller(0)
	// runtime.Caller gives us this file; walk up to the repo root from there.
	projectRoot := filepath.Dir(filepath.Dir(filepath.Dir(currentFile)))
	return filepath.Join(projectRoot, "testdata")
}

// getTestFilePath returns the full path to a test fixture file
func getTestFilePath(fixtureDir, filename string) string {
	return filepath.Join(getTestdataPath(), "pptx", fixtureDir, filename)
}

// getProducerFixturePath returns the full path to a producer fixture file
func getProducerFixturePath(producer string) string {
	return filepath.Join(getTestdataPath(), "pptx", "producers", producer, "presentation.pptx")
}

// fileExists checks if a file exists
func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}
