package cli

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPPTXReadCommandsRejectNonPPTXPackages(t *testing.T) {
	xlsxPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	unknownPath := writeUnknownOPCPackage(t)

	inputs := []struct {
		name string
		path string
	}{
		{name: "xlsx", path: xlsxPath},
		{name: "unknown-opc", path: unknownPath},
	}

	for _, input := range inputs {
		for _, tt := range []struct {
			name string
			args []string
		}{
			{name: "extract text", args: []string{"pptx", "extract", "text", input.path}},
			{name: "extract notes", args: []string{"pptx", "extract", "notes", input.path}},
			{name: "extract images", args: []string{"pptx", "extract", "images", input.path, "--out", t.TempDir()}},
			{name: "extract xml", args: []string{"pptx", "extract", "xml", input.path, "--out", t.TempDir()}},
			{name: "slides selectors", args: []string{"pptx", "slides", "selectors", input.path, "--slide", "1"}},
			{name: "translate export", args: []string{"pptx", "translate", "export", input.path}},
			{name: "masters list", args: []string{"pptx", "masters", "list", input.path}},
			{name: "masters show", args: []string{"pptx", "masters", "show", input.path, "--master", "1"}},
			{name: "validate layout", args: []string{"pptx", "validate-layout", input.path}},
			{name: "template capture", args: []string{"pptx", "template", "capture", input.path}},
			{name: "diff", args: []string{"pptx", "diff", input.path, input.path}},
		} {
			t.Run(input.name+"/"+tt.name, func(t *testing.T) {
				_, err := executeRootForXLSXTest(t, tt.args...)
				assertUnsupportedPPTXPackageError(t, err)
			})
		}
	}
}

func TestPPTXMutationWriterRejectsNonPPTXInput(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "out.pptx")

	_, err := executeRootForXLSXTest(t,
		"pptx", "clone-slide", workbookPath,
		"--slide", "1",
		"--out", outPath,
	)
	assertUnsupportedPPTXPackageError(t, err)
}

func TestPPTXImportCommandsRejectNonPPTXSource(t *testing.T) {
	targetPath := getTestFilePath("minimal-title", "presentation.pptx")
	sourcePath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	for _, tt := range []struct {
		name string
		args []string
	}{
		{
			name: "slides import-slide",
			args: []string{"pptx", "slides", "import-slide", targetPath, "--source", sourcePath, "--slide", "1", "--out", filepath.Join(t.TempDir(), "imported-slide.pptx")},
		},
		{
			name: "slides merge",
			args: []string{"pptx", "slides", "merge", targetPath, sourcePath, "--out", filepath.Join(t.TempDir(), "merged.pptx")},
		},
		{
			name: "layouts import",
			args: []string{"pptx", "layouts", "import", targetPath, "--source", sourcePath, "--layout", "1", "--out", filepath.Join(t.TempDir(), "imported-layout.pptx")},
		},
		{
			name: "masters import",
			args: []string{"pptx", "masters", "import", targetPath, "--source", sourcePath, "--master", "1", "--out", filepath.Join(t.TempDir(), "imported-master.pptx")},
		},
	} {
		t.Run(tt.name, func(t *testing.T) {
			_, err := executeRootForXLSXTest(t, tt.args...)
			assertUnsupportedPPTXPackageError(t, err)
		})
	}
}

func TestPPTXGuardPreservesMissingFileError(t *testing.T) {
	missingPath := filepath.Join(t.TempDir(), "missing.pptx")

	_, err := executeRootForXLSXTest(t, "pptx", "extract", "text", missingPath)
	if err == nil {
		t.Fatal("expected file-not-found error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitFileNotFound {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitFileNotFound)
	}
}

func TestPPTXTranslateApplyRejectsNonPPTXAfterManifestRead(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	manifestPath := filepath.Join(t.TempDir(), "manifest.json")
	if err := os.WriteFile(manifestPath, []byte(`{"metadata":{"version":"1.0"},"entries":[]}`), 0644); err != nil {
		t.Fatalf("failed to write manifest: %v", err)
	}

	_, err := executeRootForXLSXTest(t, "pptx", "translate", "apply", workbookPath, manifestPath)
	assertUnsupportedPPTXPackageError(t, err)
}

func assertUnsupportedPPTXPackageError(t *testing.T, err error) {
	t.Helper()

	if err == nil {
		t.Fatal("expected unsupported package error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("error type = %T, want *CLIError", err)
	}
	if cliErr.ExitCode != ExitUnsupportedType {
		t.Fatalf("exit code = %d, want %d: %v", cliErr.ExitCode, ExitUnsupportedType, err)
	}
	if !strings.Contains(cliErr.Message, "PPTX") {
		t.Fatalf("error message %q does not mention PPTX", cliErr.Message)
	}
}
