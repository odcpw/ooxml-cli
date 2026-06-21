package cli

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
	"github.com/spf13/cobra"
)

// MutationOptions holds the mutation-related flags
type MutationOptions struct {
	// Either OutPath or InPlace must be set, but not both
	OutPath    string // --out flag
	InPlace    bool   // --in-place flag
	Backup     string // --backup flag (only valid with --in-place)
	NoValidate bool   // --no-validate flag (skip post-write validation)
	DryRun     bool   // --dry-run flag (validate without writing output)
}

// ValidateMutationFlags checks that exactly one of --out or --in-place is set
func ValidateMutationFlags(opts *MutationOptions) error {
	if opts.DryRun {
		if opts.OutPath != "" || opts.InPlace {
			return InvalidArgsError("--dry-run cannot be combined with --out or --in-place")
		}
		if opts.Backup != "" {
			return InvalidArgsError("--backup cannot be used with --dry-run")
		}
		return nil
	}

	if opts.OutPath == "" && !opts.InPlace {
		return InvalidArgsError("must specify exactly one of --out, --in-place, or --dry-run")
	}

	if opts.OutPath != "" && opts.InPlace {
		return InvalidArgsError("cannot specify both --out and --in-place")
	}

	if opts.Backup != "" && !opts.InPlace {
		return InvalidArgsError("--backup can only be used with --in-place")
	}

	return nil
}

func GetValidatedMutationOptions(cmd *cobra.Command) (*MutationOptions, error) {
	opts, err := GetMutationOptions(cmd)
	if err != nil {
		return nil, err
	}
	if err := ValidateMutationFlags(opts); err != nil {
		return nil, err
	}
	return opts, nil
}

// MutationWriter handles safe writing of mutated packages
type MutationWriter struct {
	inputPath  string
	outputPath string
	backupPath string
	tempPath   string
	noValidate bool
	dryRun     bool
}

// NewMutationWriter creates a new mutation writer
func NewMutationWriter(inputPath string, opts *MutationOptions) (*MutationWriter, error) {
	return NewMutationWriterForType(inputPath, opts, opc.PackageTypePPTX)
}

// NewMutationWriterForType creates a mutation writer guarded for a specific package type.
func NewMutationWriterForType(inputPath string, opts *MutationOptions, expectedType opc.PackageType) (*MutationWriter, error) {
	if err := ValidateMutationFlags(opts); err != nil {
		return nil, err
	}

	guard, err := openPackageExpectType(inputPath, expectedType)
	if err != nil {
		return nil, err
	}
	_ = guard.Close()

	writer := &MutationWriter{
		inputPath:  inputPath,
		noValidate: opts.NoValidate,
		dryRun:     opts.DryRun,
	}

	// Determine output and backup paths
	if opts.DryRun {
		writer.outputPath = ""
	} else if opts.InPlace {
		writer.outputPath = inputPath
		if opts.Backup != "" {
			writer.backupPath = opts.Backup
		}
	} else {
		writer.outputPath = opts.OutPath
	}

	// Create temp file in the same directory as output to ensure same filesystem.
	// Dry-run uses the system temp dir because no output artifact will be written.
	outputDir := os.TempDir()
	if !opts.DryRun {
		outputDir = filepath.Dir(writer.outputPath)
	}
	tempFile, err := os.CreateTemp(outputDir, ".ooxml-mutate-*."+expectedType.String())
	if err != nil {
		return nil, fmt.Errorf("failed to create temporary file: %w", err)
	}
	writer.tempPath = tempFile.Name()
	tempFile.Close()

	// Clean up temp file (will be recreated by SaveAs)
	os.Remove(writer.tempPath)

	return writer, nil
}

// Write performs the mutation by writing to the temp file
func (w *MutationWriter) Write(fn func(opc.PackageSession) error) error {
	// Open input package
	inputPkg, err := opc.Open(w.inputPath)
	if err != nil {
		w.cleanup()
		return fmt.Errorf("failed to open input package: %w", err)
	}
	defer inputPkg.Close()

	// Apply mutations
	if err := fn(inputPkg); err != nil {
		w.cleanup()
		return err
	}

	// Validate output if not disabled
	if !w.noValidate {
		// Save to temp first for validation
		if err := inputPkg.SaveAs(w.tempPath); err != nil {
			w.cleanup()
			return fmt.Errorf("failed to save to temp file: %w", err)
		}

		// Validate
		validPkg, err := opc.Open(w.tempPath)
		if err != nil {
			w.cleanup()
			return fmt.Errorf("failed to open temp file for validation: %w", err)
		}

		diags, err := validate.ValidatePackage(validPkg)
		validPkg.Close()
		if err != nil {
			w.cleanup()
			return fmt.Errorf("validation error: %w", err)
		}

		if err := validationFailureError(diags); err != nil {
			w.cleanup()
			return err
		}
	} else if !w.dryRun {
		// Save directly to temp file
		if err := inputPkg.SaveAs(w.tempPath); err != nil {
			w.cleanup()
			return fmt.Errorf("failed to save to temp file: %w", err)
		}
	}

	if w.dryRun {
		w.cleanup()
		return nil
	}

	// Atomic replace: if in-place, backup original and move temp to output
	if w.backupPath != "" {
		// Create backup of original
		if err := copyFile(w.inputPath, w.backupPath); err != nil {
			w.cleanup()
			return fmt.Errorf("failed to create backup: %w", err)
		}
	}

	// Move temp to output (atomically replaces if it exists)
	if err := os.Rename(w.tempPath, w.outputPath); err != nil {
		w.cleanup()
		return fmt.Errorf("failed to write output file: %w", err)
	}

	return nil
}

// cleanup removes the temporary file if it still exists
func (w *MutationWriter) cleanup() {
	if w.tempPath != "" {
		os.Remove(w.tempPath)
	}
}

func mutationWriteError(err error, message string) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	return NewCLIErrorf(ExitUnexpected, "%s: %v", message, err)
}

// copyFile copies a file
func copyFile(src, dst string) error {
	srcFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer srcFile.Close()

	dstFile, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer dstFile.Close()

	if _, err := io.Copy(dstFile, srcFile); err != nil {
		return err
	}

	// Preserve timestamps
	srcInfo, err := os.Stat(src)
	if err == nil {
		os.Chtimes(dst, time.Now(), srcInfo.ModTime())
	}

	return nil
}

// AddMutationFlags adds common mutation flags to a command
func AddMutationFlags(cmd *cobra.Command) {
	cmd.Flags().String("out", "", "output file path (mutually exclusive with --in-place)")
	cmd.Flags().Bool("in-place", false, "modify the input file in place (mutually exclusive with --out)")
	cmd.Flags().String("backup", "", "backup file path for --in-place (optional)")
	cmd.Flags().Bool("no-validate", false, "skip validation after mutation")
	cmd.Flags().Bool("dry-run", false, "validate mutation without writing an output file")
}

// GetMutationOptions extracts mutation options from command flags
func GetMutationOptions(cmd *cobra.Command) (*MutationOptions, error) {
	outPath, err := cmd.Flags().GetString("out")
	if err != nil {
		return nil, err
	}

	inPlace, err := cmd.Flags().GetBool("in-place")
	if err != nil {
		return nil, err
	}

	backup, err := cmd.Flags().GetString("backup")
	if err != nil {
		return nil, err
	}

	noValidate, err := cmd.Flags().GetBool("no-validate")
	if err != nil {
		return nil, err
	}

	dryRun, err := cmd.Flags().GetBool("dry-run")
	if err != nil {
		return nil, err
	}

	return &MutationOptions{
		OutPath:    outPath,
		InPlace:    inPlace,
		Backup:     backup,
		NoValidate: noValidate,
		DryRun:     dryRun,
	}, nil
}
