package cli

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
)

// VerifySchemaVersion pins the verify JSON contract.
const VerifySchemaVersion = "1.0"

var verifyBaseline string

// VerifyResult is the unified verify envelope returned to stdout.
type VerifyResult struct {
	SchemaVersion string                 `json:"schemaVersion"`
	File          string                 `json:"file"`
	Type          string                 `json:"type"`
	Valid         bool                   `json:"valid"`
	Validation    VerifyValidationResult `json:"validation"`
	Rendered      VerifyRenderResult     `json:"rendered"`
	Diff          *FamilyDiffResult      `json:"diff,omitempty"`
	Summary       VerifySummary          `json:"summary"`
}

// VerifyValidationResult mirrors the validate command's status/summary.
type VerifyValidationResult struct {
	Status      string           `json:"status"` // "valid", "warnings", "errors"
	Diagnostics []DiagnosticJSON `json:"diagnostics,omitempty"`
	Summary     ValidateSummary  `json:"summary"`
}

// VerifyRenderResult reports the render gate outcome.
type VerifyRenderResult struct {
	Enabled bool   `json:"enabled"`
	Status  string `json:"status"` // "ok", "skipped", "unavailable"
	Reason  string `json:"reason,omitempty"`
	PDFPath string `json:"pdfPath,omitempty"`
}

// VerifySummary is the at-a-glance outcome.
type VerifySummary struct {
	Valid    bool   `json:"valid"`
	Rendered bool   `json:"rendered"`
	Changes  int    `json:"changes"`
	Baseline string `json:"baseline,omitempty"`
}

var verifyCmd = &cobra.Command{
	Use:   "verify <file>",
	Short: "Validate, render-check, and optionally diff a package against a baseline",
	Long: `Run a unified verification of an OOXML package.

verify always validates the package. For PPTX it additionally attempts a render
check, gated on LibreOffice availability (skipped gracefully when absent). When
--baseline is given it computes a semantic diff baseline -> file so a caller can
confirm that a mutation changed only the intended fields.

Output is a single JSON object {valid, validation, rendered, diff, summary,
schemaVersion}.

Exit codes:
  0 = valid (no errors; warnings allowed unless --strict)
  5 = validation failed (errors, or warnings under --strict)`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pkg, err := opc.Open(filePath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()
		pkgType := opc.DetectType(pkg)

		config := GetGlobalConfig(cmd)

		// Validation (always).
		diags, err := validate.ValidatePackage(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "validation error: %v", err)
		}
		validation, valid := summarizeValidation(diags, config.Strict)

		result := &VerifyResult{
			SchemaVersion: VerifySchemaVersion,
			File:          filePath,
			Type:          pkgType.String(),
			Valid:         valid,
			Validation:    validation,
			Rendered:      runVerifyRender(filePath, pkgType, config.KeepTemp),
		}

		// Optional semantic diff baseline -> file (readback proof).
		if verifyBaseline != "" {
			if _, err := os.Stat(verifyBaseline); err != nil {
				return FileNotFoundError(verifyBaseline)
			}
			diff, changes, err := runVerifyDiff(cmd, pkgType, verifyBaseline, filePath)
			if err != nil {
				return err
			}
			result.Diff = diff
			result.Summary.Changes = changes
			result.Summary.Baseline = verifyBaseline
		}

		result.Summary.Valid = result.Valid
		result.Summary.Rendered = result.Rendered.Status == "ok"

		if err := writeGlobalJSON(cmd, result); err != nil {
			return err
		}
		if !valid {
			cliErr := NewCLIError(ExitValidationFailed, "")
			cliErr.Reported = true
			return cliErr
		}
		return nil
	},
}

// summarizeValidation collapses diagnostics into the verify validation block and
// reports overall validity under the strict setting.
func summarizeValidation(diags []result.Diagnostic, strict bool) (VerifyValidationResult, bool) {
	var errorCount, warningCount, infoCount int
	diagsJSON := make([]DiagnosticJSON, 0, len(diags))
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

	status := "valid"
	valid := true
	if errorCount > 0 {
		status = "errors"
		valid = false
	} else if warningCount > 0 {
		if strict {
			status = "errors"
			valid = false
		} else {
			status = "warnings"
		}
	}

	return VerifyValidationResult{
		Status:      status,
		Diagnostics: diagsJSON,
		Summary: ValidateSummary{
			ErrorCount:   errorCount,
			WarningCount: warningCount,
			InfoCount:    infoCount,
		},
	}, valid
}

// runVerifyRender attempts a PPTX render check, gated on LibreOffice. Non-PPTX
// types are skipped. Missing render tools degrade gracefully to "unavailable"
// without failing verify.
func runVerifyRender(filePath string, pkgType opc.PackageType, keepTemp bool) VerifyRenderResult {
	if pkgType != opc.PackageTypePPTX {
		return VerifyRenderResult{Enabled: false, Status: "skipped", Reason: "render check applies to PPTX only"}
	}

	outDir, err := os.MkdirTemp("", "ooxml-verify-*")
	if err != nil {
		return VerifyRenderResult{Enabled: true, Status: "unavailable", Reason: err.Error()}
	}
	if !keepTemp {
		defer os.RemoveAll(outDir)
	}

	pdfPath, err := renderToPDFFn(filePath, outDir)
	if err != nil {
		var missing *pkgrender.MissingDependencyError
		var toolFailure *pkgrender.ToolFailureError
		if errors.As(err, &missing) || errors.As(err, &toolFailure) {
			return VerifyRenderResult{Enabled: true, Status: "unavailable", Reason: err.Error()}
		}
		return VerifyRenderResult{Enabled: true, Status: "unavailable", Reason: fmt.Sprintf("render failed: %v", err)}
	}

	res := VerifyRenderResult{Enabled: true, Status: "ok"}
	if keepTemp {
		res.PDFPath = pdfPath
	} else {
		res.PDFPath = filepath.Base(pdfPath)
	}
	return res
}

// runVerifyDiff computes a semantic diff baseline -> file and returns the
// family-general result plus a flat change count for the summary.
func runVerifyDiff(cmd *cobra.Command, fileType opc.PackageType, baselinePath, filePath string) (*FamilyDiffResult, int, error) {
	baseline, baseType, err := openDetect(baselinePath)
	if err != nil {
		return nil, 0, err
	}
	defer baseline.Close()
	candidate, candType, err := openDetect(filePath)
	if err != nil {
		return nil, 0, err
	}
	defer candidate.Close()

	if baseType != candType {
		return nil, 0, NewCLIErrorf(ExitUnsupportedType,
			"cannot diff different package types (baseline: %s, file: %s)", baseType, candType)
	}

	// verify never renders inside the diff; reuse the semantic path only.
	prevRender := familyDiffRender
	familyDiffRender = false
	defer func() { familyDiffRender = prevRender }()

	diff, _, err := computeFamilyDiff(cmd, baseType, baseline, candidate, baselinePath, filePath)
	if err != nil {
		return nil, 0, err
	}
	return diff, familyDiffChangeCount(diff.Semantic), nil
}

func init() {
	verifyCmd.Flags().StringVar(&verifyBaseline, "baseline", "", "optional baseline package to semantic-diff against the file")
	GetRootCmd().AddCommand(verifyCmd)
}
