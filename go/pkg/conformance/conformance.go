// Package conformance combines repo validation, Office-sensitive XML
// invariants, and optional local Office-compatible open checks.
package conformance

import (
	"errors"
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/officecheck"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
)

const SchemaVersion = "ooxml-cli.conformance.v1"

// OfficeChecker is the subset of officecheck.Tools used by this package.
type OfficeChecker interface {
	Check(filePath string, opts officecheck.Options) (*officecheck.Result, error)
}

// Options controls a conformance run.
type Options struct {
	RunOfficeCheck    bool
	OfficeCheckOutDir string
	OfficeChecker     OfficeChecker
}

// Report is the machine-readable conformance result.
type Report struct {
	SchemaVersion string        `json:"schemaVersion"`
	File          string        `json:"file,omitempty"`
	Family        string        `json:"family"`
	Status        string        `json:"status"`
	Checks        []CheckResult `json:"checks"`
	Summary       Summary       `json:"summary"`
}

// CheckResult captures one harness stage.
type CheckResult struct {
	Name        string              `json:"name"`
	Status      string              `json:"status"`
	Diagnostics []Diagnostic        `json:"diagnostics,omitempty"`
	OfficeCheck *officecheck.Result `json:"officeCheck,omitempty"`
}

// Diagnostic is a stable JSON representation of result.Diagnostic.
type Diagnostic struct {
	Code     string `json:"code"`
	Severity string `json:"severity"`
	Message  string `json:"message"`
}

// Summary rolls up all checks.
type Summary struct {
	Passed   int `json:"passed"`
	Failed   int `json:"failed"`
	Warnings int `json:"warnings"`
	Skipped  int `json:"skipped"`
	Errors   int `json:"errors"`
}

// CheckPackage opens path and runs the conformance harness.
func CheckPackage(path string, opts Options) (*Report, error) {
	report := newReport(path)

	pkg, err := opc.Open(path)
	if err != nil {
		report.addCheck(CheckResult{
			Name:   "package-open",
			Status: "failed",
			Diagnostics: diagnosticsJSON([]result.Diagnostic{
				diag.Error("OOXML_OPEN_FAILED", err.Error()),
			}),
		})
		report.finish()
		return report, err
	}
	defer pkg.Close()

	report.Family = opc.DetectType(pkg).String()
	report.addCheck(CheckResult{Name: "package-open", Status: "passed"})
	runSessionChecks(report, pkg)

	if opts.RunOfficeCheck {
		runOfficeCheck(report, path, opts)
	}

	report.finish()
	return report, nil
}

// CheckSession runs conformance checks over an already-open package session.
// It intentionally omits local Office open checks because those need a file path.
func CheckSession(session opc.PackageSession) (*Report, error) {
	report := newReport("")
	report.Family = opc.DetectType(session).String()
	runSessionChecks(report, session)
	report.finish()
	return report, nil
}

func newReport(path string) *Report {
	return &Report{
		SchemaVersion: SchemaVersion,
		File:          path,
		Family:        opc.PackageTypeUnknown.String(),
		Status:        "passed",
	}
}

func runSessionChecks(report *Report, session opc.PackageSession) {
	validationDiags, err := validate.ValidatePackage(session)
	if err != nil {
		report.addCheck(CheckResult{
			Name:   "repo-validation",
			Status: "failed",
			Diagnostics: diagnosticsJSON([]result.Diagnostic{
				diag.Error("OOXML_VALIDATE_FAILED", err.Error()),
			}),
		})
	} else {
		report.addDiagnosticCheck("repo-validation", validationDiags)
	}

	invariantDiags, err := CheckRepairInvariants(session)
	if err != nil {
		report.addCheck(CheckResult{
			Name:   "repair-invariants",
			Status: "failed",
			Diagnostics: diagnosticsJSON([]result.Diagnostic{
				diag.Error("OOXML_REPAIR_INVARIANT_FAILED", err.Error()),
			}),
		})
	} else {
		report.addDiagnosticCheck("repair-invariants", invariantDiags)
	}
}

func runOfficeCheck(report *Report, path string, opts Options) {
	family := report.Family
	if family != opc.PackageTypePPTX.String() && family != opc.PackageTypeXLSX.String() {
		report.addCheck(CheckResult{
			Name:   "office-open",
			Status: "skipped",
			Diagnostics: diagnosticsJSON([]result.Diagnostic{
				diag.Info("OOXML_OFFICE_CHECK_UNSUPPORTED", fmt.Sprintf("office open-check supports pptx/xlsx only, got %s", family)),
			}),
		})
		return
	}

	checker := opts.OfficeChecker
	if checker == nil {
		checker = officecheck.NewTools()
	}
	res, err := checker.Check(path, officecheck.Options{Family: family, OutDir: opts.OfficeCheckOutDir})
	check := CheckResult{Name: "office-open", OfficeCheck: res}
	if err == nil {
		check.Status = "passed"
		report.addCheck(check)
		return
	}

	var missing *officecheck.MissingDependencyError
	if errors.As(err, &missing) {
		check.Status = "skipped"
		check.Diagnostics = diagnosticsJSON([]result.Diagnostic{
			diag.Info("OOXML_OFFICE_CHECK_SKIPPED", err.Error()),
		})
		report.addCheck(check)
		return
	}

	check.Status = "failed"
	check.Diagnostics = diagnosticsJSON([]result.Diagnostic{
		diag.Error("OOXML_OFFICE_CHECK_FAILED", err.Error()),
	})
	report.addCheck(check)
}

func (r *Report) addDiagnosticCheck(name string, diags []result.Diagnostic) {
	status := "passed"
	if hasSeverity(diags, result.Error) {
		status = "failed"
	} else if hasSeverity(diags, result.Warning) {
		status = "warning"
	}
	r.addCheck(CheckResult{Name: name, Status: status, Diagnostics: diagnosticsJSON(diags)})
}

func (r *Report) addCheck(check CheckResult) {
	r.Checks = append(r.Checks, check)
}

func (r *Report) finish() {
	var hasFailed, hasWarnings bool
	for _, check := range r.Checks {
		switch check.Status {
		case "failed":
			r.Summary.Failed++
			hasFailed = true
		case "warning":
			r.Summary.Warnings++
			hasWarnings = true
		case "skipped":
			r.Summary.Skipped++
		default:
			r.Summary.Passed++
		}
		for _, d := range check.Diagnostics {
			if d.Severity == result.Error.String() {
				r.Summary.Errors++
			}
		}
	}
	switch {
	case hasFailed:
		r.Status = "failed"
	case hasWarnings:
		r.Status = "warning"
	default:
		r.Status = "passed"
	}
}

func (r *Report) HasFailures() bool {
	return r != nil && r.Status == "failed"
}

func hasSeverity(diags []result.Diagnostic, severity result.Severity) bool {
	for _, d := range diags {
		if d.Severity == severity {
			return true
		}
	}
	return false
}

func diagnosticsJSON(diags []result.Diagnostic) []Diagnostic {
	if len(diags) == 0 {
		return nil
	}
	out := make([]Diagnostic, 0, len(diags))
	for _, d := range diags {
		out = append(out, Diagnostic{
			Code:     d.Code,
			Severity: d.Severity.String(),
			Message:  d.Message,
		})
	}
	return out
}
