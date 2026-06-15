package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
)

// ValidateResult is the JSON output structure for the validate command
type ValidateResult struct {
	File        string           `json:"file"`
	Valid       bool             `json:"valid"`
	Status      string           `json:"status"` // "valid", "warnings", "errors"
	Diagnostics []DiagnosticJSON `json:"diagnostics,omitempty"`
	Summary     *ValidateSummary `json:"summary"`
	Error       string           `json:"error,omitempty"`
}

// DiagnosticJSON represents a diagnostic in JSON format
type DiagnosticJSON struct {
	Code     string `json:"code"`
	Severity string `json:"severity"`
	Message  string `json:"message"`
}

// ValidateSummary contains summary statistics
type ValidateSummary struct {
	ErrorCount   int `json:"errors"`
	WarningCount int `json:"warnings"`
	InfoCount    int `json:"info"`
}

var validateCmd = &cobra.Command{
	Use:           "validate <file>",
	Short:         "Validate OOXML package integrity and structure",
	SilenceUsage:  true,
	SilenceErrors: true,
	Long: `Validate an OOXML package (PPTX, DOCX, or XLSX) for integrity and correct structure.

Performs package validation including:
  - Package structure and archive integrity
  - Relationship graph validity
  - VBA package consistency for PPTX/PPTM and XLSX/XLSM
  - PPTX, DOCX, and XLSX semantic validation where implemented
  - XML well-formedness

By default, warnings do not cause validation failure. Use --strict to treat warnings as errors.

Exit codes:
  0 = validation passed
  5 = validation failed (errors present)
  9 = validation passed with warnings (only if --strict is not set)`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Open the package
		pkg, err := opc.Open(filePath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		// Get global config
		config := GetGlobalConfig(cmd)

		// Run validation
		diags, err := validate.ValidatePackage(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "validation error: %v", err)
		}

		// Separate diagnostics by severity
		var errorDiags, warningDiags, infoDiags []result.Diagnostic
		for _, diag := range diags {
			switch diag.Severity {
			case result.Error:
				errorDiags = append(errorDiags, diag)
			case result.Warning:
				warningDiags = append(warningDiags, diag)
			case result.Info:
				infoDiags = append(infoDiags, diag)
			}
		}

		// Determine validation status and exit code
		hasErrors := len(errorDiags) > 0
		hasWarnings := len(warningDiags) > 0
		var exitCode int
		var status string

		if hasErrors {
			exitCode = ExitValidationFailed
			status = "errors"
		} else if hasWarnings {
			if config.Strict {
				exitCode = ExitValidationFailed
				status = "errors"
			} else {
				exitCode = ExitPartialSuccess
				status = "warnings"
			}
		} else {
			exitCode = ExitSuccess
			status = "valid"
		}

		// Format and output results
		if config.Format == "json" {
			return outputValidateJSON(cmd, filePath, status, diags, exitCode)
		}

		// Default to text output
		return outputValidateText(cmd, filePath, status, errorDiags, warningDiags, infoDiags, exitCode)
	},
}

// outputValidateJSON outputs the validation result in JSON format
func outputValidateJSON(cmd *cobra.Command, filePath string, status string, diags []result.Diagnostic, exitCode int) error {
	config := GetGlobalConfig(cmd)

	// Count diagnostics by severity
	var errorCount, warningCount, infoCount int
	var diagsJSON []DiagnosticJSON

	for _, d := range diags {
		switch d.Severity {
		case result.Error:
			errorCount++
		case result.Warning:
			warningCount++
		case result.Info:
			infoCount++
		}

		diagsJSON = append(diagsJSON, DiagnosticJSON{
			Code:     d.Code,
			Severity: d.Severity.String(),
			Message:  d.Message,
		})
	}

	// Build the result
	result := ValidateResult{
		File:        filePath,
		Valid:       exitCode == ExitSuccess,
		Status:      status,
		Diagnostics: diagsJSON,
		Summary: &ValidateSummary{
			ErrorCount:   errorCount,
			WarningCount: warningCount,
			InfoCount:    infoCount,
		},
	}

	// Marshal to JSON
	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(result, "", "  ")
	} else {
		jsonData, err = json.Marshal(result)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	// Write to output
	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "%s\n", string(jsonData))

	// Return CLIError with appropriate exit code
	if exitCode != ExitSuccess {
		err := NewCLIError(exitCode, "")
		err.Reported = true
		return err
	}

	return nil
}

// outputValidateText outputs the validation result in human-readable text format
func outputValidateText(cmd *cobra.Command, filePath string, status string, errorDiags, warningDiags, infoDiags []result.Diagnostic, exitCode int) error {
	config := GetGlobalConfig(cmd)

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	// Format text output
	lines := []string{}

	// Header
	lines = append(lines, fmt.Sprintf("File: %s", filePath))

	// Status
	statusIcon := "✓"
	if exitCode == ExitValidationFailed {
		statusIcon = "✗"
	} else if exitCode == ExitPartialSuccess {
		statusIcon = "⚠"
	}
	lines = append(lines, fmt.Sprintf("Status: %s %s", statusIcon, status))

	// Summary
	lines = append(lines, "")
	lines = append(lines, fmt.Sprintf("Errors:   %d", len(errorDiags)))
	lines = append(lines, fmt.Sprintf("Warnings: %d", len(warningDiags)))
	if len(infoDiags) > 0 {
		lines = append(lines, fmt.Sprintf("Info:     %d", len(infoDiags)))
	}

	// Diagnostics
	if len(errorDiags) > 0 {
		lines = append(lines, "")
		lines = append(lines, "Errors:")
		for _, d := range errorDiags {
			lines = append(lines, fmt.Sprintf("  [%s] %s", d.Code, d.Message))
		}
	}

	if len(warningDiags) > 0 {
		lines = append(lines, "")
		lines = append(lines, "Warnings:")
		for _, d := range warningDiags {
			lines = append(lines, fmt.Sprintf("  [%s] %s", d.Code, d.Message))
		}
	}

	if len(infoDiags) > 0 && config.Verbosity != "quiet" {
		lines = append(lines, "")
		lines = append(lines, "Info:")
		for _, d := range infoDiags {
			lines = append(lines, fmt.Sprintf("  [%s] %s", d.Code, d.Message))
		}
	}

	// Write output
	for _, line := range lines {
		fmt.Fprintf(outFile, "%s\n", line)
	}

	// Return CLIError with appropriate exit code
	if exitCode != ExitSuccess {
		err := NewCLIError(exitCode, "")
		err.Reported = true
		return err
	}

	return nil
}

// init registers the validate command with the root command
func init() {
	GetRootCmd().AddCommand(validateCmd)
}
