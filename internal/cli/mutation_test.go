package cli

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestValidateMutationFlags tests the mutation flags validation
func TestValidateMutationFlags(t *testing.T) {
	tests := []struct {
		name    string
		opts    *MutationOptions
		wantErr bool
		errMsg  string
	}{
		{
			name: "neither out nor in-place specified",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: true,
			errMsg:  "must specify exactly one",
		},
		{
			name: "dry-run without output",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
				DryRun:     true,
			},
			wantErr: false,
		},
		{
			name: "dry-run with out",
			opts: &MutationOptions{
				OutPath:    "output.pptx",
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
				DryRun:     true,
			},
			wantErr: true,
			errMsg:  "cannot be combined",
		},
		{
			name: "dry-run with in-place",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    true,
				Backup:     "",
				NoValidate: false,
				DryRun:     true,
			},
			wantErr: true,
			errMsg:  "cannot be combined",
		},
		{
			name: "dry-run with backup",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    false,
				Backup:     "input.bak",
				NoValidate: false,
				DryRun:     true,
			},
			wantErr: true,
			errMsg:  "cannot be used with --dry-run",
		},
		{
			name: "both out and in-place specified",
			opts: &MutationOptions{
				OutPath:    "output.pptx",
				InPlace:    true,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: true,
			errMsg:  "cannot specify both",
		},
		{
			name: "only out specified",
			opts: &MutationOptions{
				OutPath:    "output.pptx",
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: false,
		},
		{
			name: "only in-place specified",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    true,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: false,
		},
		{
			name: "backup without in-place",
			opts: &MutationOptions{
				OutPath:    "output.pptx",
				InPlace:    false,
				Backup:     "output.bak",
				NoValidate: false,
			},
			wantErr: true,
			errMsg:  "can only be used with --in-place",
		},
		{
			name: "in-place with backup",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    true,
				Backup:     "input.bak",
				NoValidate: false,
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateMutationFlags(tt.opts)
			if tt.wantErr {
				assert.Error(t, err)
				if err != nil && tt.errMsg != "" {
					assert.Contains(t, err.Error(), tt.errMsg)
				}
			} else {
				assert.NoError(t, err)
			}
		})
	}
}

// TestNewMutationWriter tests the mutation writer creation
func TestNewMutationWriter(t *testing.T) {
	tempDir := t.TempDir()
	inputPath := filepath.Join(tempDir, "input.pptx")
	outputPath := filepath.Join(tempDir, "output.pptx")

	require.NoError(t, copyFile(getTestFilePath("minimal-title", "presentation.pptx"), inputPath))

	tests := []struct {
		name    string
		opts    *MutationOptions
		wantErr bool
	}{
		{
			name: "valid out option",
			opts: &MutationOptions{
				OutPath:    outputPath,
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: false,
		},
		{
			name: "valid in-place option",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    true,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: false,
		},
		{
			name: "invalid options",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
			},
			wantErr: true,
		},
		{
			name: "valid dry-run option",
			opts: &MutationOptions{
				OutPath:    "",
				InPlace:    false,
				Backup:     "",
				NoValidate: false,
				DryRun:     true,
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			writer, err := NewMutationWriter(inputPath, tt.opts)
			if tt.wantErr {
				assert.Error(t, err)
			} else {
				assert.NoError(t, err)
				assert.NotNil(t, writer)
			}
		})
	}
}

func TestNewMutationWriterForTypeAcceptsXLSX(t *testing.T) {
	inputPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outputPath := filepath.Join(t.TempDir(), "output.xlsx")

	writer, err := NewMutationWriterForType(inputPath, &MutationOptions{
		OutPath: outputPath,
	}, opc.PackageTypeXLSX)
	require.NoError(t, err)
	require.NotNil(t, writer)
}

// TestMutationOptionsStruct tests the MutationOptions struct construction
func TestMutationOptionsStruct(t *testing.T) {
	opts := &MutationOptions{
		OutPath:    "output.pptx",
		InPlace:    false,
		Backup:     "",
		NoValidate: false,
	}

	assert.Equal(t, "output.pptx", opts.OutPath)
	assert.False(t, opts.InPlace)
	assert.Equal(t, "", opts.Backup)
	assert.False(t, opts.NoValidate)
	assert.False(t, opts.DryRun)
}

// TestMutationOptionsInPlace tests in-place mutation options
func TestMutationOptionsInPlace(t *testing.T) {
	opts := &MutationOptions{
		OutPath:    "",
		InPlace:    true,
		Backup:     "input.bak",
		NoValidate: false,
	}

	assert.Equal(t, "", opts.OutPath)
	assert.True(t, opts.InPlace)
	assert.Equal(t, "input.bak", opts.Backup)
	assert.False(t, opts.NoValidate)
	assert.False(t, opts.DryRun)
}

func TestMutationWriterDryRunDoesNotWriteOutput(t *testing.T) {
	tempDir := t.TempDir()
	inputPath := filepath.Join(tempDir, "input.pptx")
	require.NoError(t, copyFile(getTestFilePath("minimal-title", "presentation.pptx"), inputPath))

	writer, err := NewMutationWriter(inputPath, &MutationOptions{DryRun: true})
	require.NoError(t, err)
	require.NotNil(t, writer)
	tempPath := writer.tempPath

	called := false
	err = writer.Write(func(pkg opc.PackageSession) error {
		called = true
		return nil
	})
	require.NoError(t, err)
	assert.True(t, called)
	_, err = os.Stat(tempPath)
	assert.True(t, os.IsNotExist(err), "dry-run temp file should be removed, stat error: %v", err)
}

func TestMutationWriterPostWriteValidationFailureCarriesDiagnostics(t *testing.T) {
	tempDir := t.TempDir()
	inputPath := filepath.Join(tempDir, "corrupted.pptx")
	outputPath := filepath.Join(tempDir, "output.pptx")
	require.NoError(t, copyFile("../../testdata/pptx/corrupted-missing-media/presentation.pptx", inputPath))

	writer, err := NewMutationWriter(inputPath, &MutationOptions{OutPath: outputPath})
	require.NoError(t, err)

	err = writer.Write(func(pkg opc.PackageSession) error {
		return nil
	})

	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok, "expected CLIError, got %T", err)
	assert.Equal(t, ExitValidationFailed, cliErr.ExitCode)
	assert.Equal(t, "validation_failed", cliErr.Code)
	assert.Contains(t, cliErr.Message, "output validation failed")
	assert.True(t, diagnosticJSONContains(cliErr.Diagnostics, "REL_DANGLING_TARGET"), "expected REL_DANGLING_TARGET, got %#v", cliErr.Diagnostics)
	assert.True(t, diagnosticJSONContains(cliErr.Diagnostics, "PPTX_MISSING_MEDIA"), "expected PPTX_MISSING_MEDIA, got %#v", cliErr.Diagnostics)

	_, statErr := os.Stat(outputPath)
	assert.True(t, os.IsNotExist(statErr), "failed validation should not write output, stat error: %v", statErr)
}

func TestMutationWriterNoValidateSkipsValidationDiagnostics(t *testing.T) {
	tempDir := t.TempDir()
	inputPath := filepath.Join(tempDir, "corrupted.pptx")
	outputPath := filepath.Join(tempDir, "output.pptx")
	require.NoError(t, copyFile("../../testdata/pptx/corrupted-missing-media/presentation.pptx", inputPath))

	writer, err := NewMutationWriter(inputPath, &MutationOptions{OutPath: outputPath, NoValidate: true})
	require.NoError(t, err)

	err = writer.Write(func(pkg opc.PackageSession) error {
		return nil
	})

	require.NoError(t, err)
	assert.FileExists(t, outputPath)
}

func diagnosticJSONContains(diags []DiagnosticJSON, code string) bool {
	for _, diag := range diags {
		if diag.Code == code {
			return true
		}
	}
	return false
}

// TestCopyFile tests the copyFile helper function
func TestCopyFileHelper(t *testing.T) {
	tempDir := t.TempDir()
	srcFile := filepath.Join(tempDir, "source.txt")
	dstFile := filepath.Join(tempDir, "dest.txt")

	// Create source file
	testContent := []byte("test content")
	err := os.WriteFile(srcFile, testContent, 0644)
	require.NoError(t, err)

	// Copy file
	err = copyFile(srcFile, dstFile)
	require.NoError(t, err)

	// Verify destination exists and has same content
	content, err := os.ReadFile(dstFile)
	require.NoError(t, err)
	assert.Equal(t, testContent, content)
}
