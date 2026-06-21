package render

import (
	"context"
	"errors"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const defaultTimeout = 2 * time.Minute

// MissingDependencyError indicates that an external render tool is unavailable.
type MissingDependencyError struct {
	Tool string
}

func (e *MissingDependencyError) Error() string {
	return fmt.Sprintf("required render tool not available: %s", e.Tool)
}

// ToolFailureError indicates that an external render tool returned an error.
type ToolFailureError struct {
	Tool   string
	Args   []string
	Stdout string
	Stderr string
	Cause  error
}

func (e *ToolFailureError) Error() string {
	if e.Stderr != "" {
		return fmt.Sprintf("%s failed: %s", e.Tool, strings.TrimSpace(e.Stderr))
	}
	if e.Stdout != "" {
		return fmt.Sprintf("%s failed: %s", e.Tool, strings.TrimSpace(e.Stdout))
	}
	return fmt.Sprintf("%s failed: %v", e.Tool, e.Cause)
}

func (e *ToolFailureError) Unwrap() error {
	return e.Cause
}

// Tools wraps the external Linux render toolchain.
type Tools struct {
	Runner  Runner
	Timeout time.Duration
}

// NewTools constructs a render tool wrapper with production defaults.
func NewTools() *Tools {
	return &Tools{Runner: ExecRunner{}, Timeout: defaultTimeout}
}

// RenderToPDF converts a PPTX file to PDF via headless LibreOffice/soffice.
func RenderToPDF(pptxPath string, outDir string) (string, error) {
	return NewTools().RenderToPDF(pptxPath, outDir)
}

// RenderToPDF converts a PPTX file to PDF via headless LibreOffice/soffice.
func (t *Tools) RenderToPDF(pptxPath string, outDir string) (string, error) {
	if t == nil {
		t = NewTools()
	}
	if t.Runner == nil {
		t.Runner = ExecRunner{}
	}
	if t.Timeout <= 0 {
		t.Timeout = defaultTimeout
	}
	if pptxPath == "" {
		return "", fmt.Errorf("pptx path cannot be empty")
	}
	if outDir == "" {
		return "", fmt.Errorf("output directory cannot be empty")
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return "", fmt.Errorf("failed to create output directory: %w", err)
	}

	binary, err := t.findLibreOfficeBinary()
	if err != nil {
		return "", err
	}

	ctx, cancel := context.WithTimeout(context.Background(), t.Timeout)
	defer cancel()

	profileDir, err := os.MkdirTemp("", "ooxml-render-profile-*")
	if err != nil {
		return "", fmt.Errorf("failed to create LibreOffice profile directory: %w", err)
	}
	defer os.RemoveAll(profileDir)

	args := []string{libreOfficeUserInstallationArg(profileDir), "--headless", "--convert-to", "pdf", "--outdir", outDir, pptxPath}
	result, runErr := t.Runner.Run(ctx, binary, args)
	if runErr != nil {
		if errors.Is(ctx.Err(), context.DeadlineExceeded) {
			return "", fmt.Errorf("%s render timed out", binary)
		}
		toolErr := &ToolFailureError{Tool: binary, Args: args, Cause: runErr}
		if result != nil {
			toolErr.Stdout = result.Stdout
			toolErr.Stderr = result.Stderr
		}
		return "", toolErr
	}

	pdfPath := filepath.Join(outDir, strings.TrimSuffix(filepath.Base(pptxPath), filepath.Ext(pptxPath))+".pdf")
	if _, err := os.Stat(pdfPath); err != nil {
		toolErr := &ToolFailureError{Tool: binary, Args: args, Cause: err}
		if result != nil {
			toolErr.Stdout = result.Stdout
			toolErr.Stderr = result.Stderr
		}
		return "", toolErr
	}
	return pdfPath, nil
}

func libreOfficeUserInstallationArg(profileDir string) string {
	return "-env:UserInstallation=" + (&url.URL{Scheme: "file", Path: profileDir}).String()
}

func (t *Tools) findLibreOfficeBinary() (string, error) {
	candidates := []string{"soffice", "libreoffice"}
	for _, candidate := range candidates {
		if _, err := t.Runner.LookPath(candidate); err == nil {
			return candidate, nil
		}
	}
	return "", &MissingDependencyError{Tool: "soffice"}
}
