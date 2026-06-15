package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/conformance"
	"github.com/ooxml-cli/ooxml-cli/pkg/officecheck"
)

func TestConformanceCheckJSON(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	output, err := executeRootForXLSXTest(t, "--json", "conformance", "check", workbookPath)
	if err != nil {
		t.Fatalf("conformance check failed: %v\n%s", err, output)
	}

	var report conformance.Report
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		t.Fatalf("parse conformance report: %v\n%s", err, output)
	}
	if report.SchemaVersion != conformance.SchemaVersion {
		t.Fatalf("schemaVersion = %q, want %q", report.SchemaVersion, conformance.SchemaVersion)
	}
	if report.Family != "xlsx" || report.Status != "passed" {
		t.Fatalf("unexpected family/status: %s/%s", report.Family, report.Status)
	}
	want := map[string]bool{
		"package-open":      false,
		"repo-validation":   false,
		"repair-invariants": false,
	}
	for _, check := range report.Checks {
		if _, ok := want[check.Name]; ok {
			want[check.Name] = check.Status == "passed"
		}
	}
	for name, passed := range want {
		if !passed {
			t.Fatalf("missing passed check %q in %+v", name, report.Checks)
		}
	}
}

func TestConformanceCheckOfficeCheckJSON(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outDir := filepath.Join(t.TempDir(), "office-open")
	previous := conformanceCheckPackage
	t.Cleanup(func() { conformanceCheckPackage = previous })
	conformanceCheckPackage = func(path string, opts conformance.Options) (*conformance.Report, error) {
		if path != workbookPath {
			t.Fatalf("path = %q, want %q", path, workbookPath)
		}
		if !opts.RunOfficeCheck {
			t.Fatal("RunOfficeCheck = false, want true")
		}
		if opts.OfficeCheckOutDir != outDir {
			t.Fatalf("OfficeCheckOutDir = %q, want %q", opts.OfficeCheckOutDir, outDir)
		}
		return &conformance.Report{
			SchemaVersion: conformance.SchemaVersion,
			File:          path,
			Family:        "xlsx",
			Status:        "passed",
			Checks: []conformance.CheckResult{
				{Name: "package-open", Status: "passed"},
				{
					Name:   "office-open",
					Status: "skipped",
					OfficeCheck: &officecheck.Result{
						Status:    "skipped",
						ErrorCode: "missing_engine",
						Error:     "required Office-compatible tool not available: soffice",
					},
				},
			},
			Summary: conformance.Summary{Passed: 1, Skipped: 1},
		}, nil
	}

	output, err := executeRootForXLSXTest(t, "--json", "conformance", "check", "--office-check", "--office-check-out-dir", outDir, workbookPath)
	if err != nil {
		t.Fatalf("conformance check --office-check failed: %v\n%s", err, output)
	}
	var report conformance.Report
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		t.Fatalf("parse conformance report: %v\n%s", err, output)
	}
	if report.Status != "passed" || report.Summary.Skipped != 1 {
		t.Fatalf("unexpected status/summary: %s %+v", report.Status, report.Summary)
	}
	foundOfficeOpen := false
	for _, check := range report.Checks {
		if check.Name != "office-open" {
			continue
		}
		foundOfficeOpen = true
		if check.Status != "skipped" || check.OfficeCheck == nil || check.OfficeCheck.ErrorCode != "missing_engine" {
			t.Fatalf("unexpected office-open check: %+v", check)
		}
	}
	if !foundOfficeOpen {
		t.Fatalf("missing office-open check in %+v", report.Checks)
	}
}

func TestConformanceCheckTextShowsOfficeCheckOutputPath(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outputPath := filepath.Join(t.TempDir(), "workbook.csv")
	previous := conformanceCheckPackage
	t.Cleanup(func() { conformanceCheckPackage = previous })
	conformanceCheckPackage = func(path string, opts conformance.Options) (*conformance.Report, error) {
		if path != workbookPath {
			t.Fatalf("path = %q, want %q", path, workbookPath)
		}
		if !opts.RunOfficeCheck {
			t.Fatal("RunOfficeCheck = false, want true")
		}
		return &conformance.Report{
			SchemaVersion: conformance.SchemaVersion,
			File:          path,
			Family:        "xlsx",
			Status:        "passed",
			Checks: []conformance.CheckResult{
				{Name: "package-open", Status: "passed"},
				{
					Name:   "office-open",
					Status: "passed",
					OfficeCheck: &officecheck.Result{
						Status:             "passed",
						Checked:            true,
						Engine:             "soffice",
						Method:             "libreoffice-headless-convert",
						ConversionFormat:   "csv",
						OutputPath:         outputPath,
						OutputBytes:        12,
						OfficeOpenVerified: true,
					},
				},
			},
			Summary: conformance.Summary{Passed: 2},
		}, nil
	}

	output, err := executeRootForXLSXTest(t, "conformance", "check", "--office-check", workbookPath)
	if err != nil {
		t.Fatalf("conformance check --office-check text failed: %v\n%s", err, output)
	}
	if !strings.Contains(output, "office-check-output: "+outputPath) {
		t.Fatalf("text output missing retained office-check output path %q\n%s", outputPath, output)
	}
}

func TestConformanceCoverageJSON(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "--json", "conformance", "coverage")
	if err != nil {
		t.Fatalf("conformance coverage failed: %v\n%s", err, output)
	}
	var report conformance.CoverageReport
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		t.Fatalf("parse conformance coverage report: %v\n%s", err, output)
	}
	if report.SchemaVersion != conformance.CoverageSchemaVersion {
		t.Fatalf("schemaVersion = %q, want %q", report.SchemaVersion, conformance.CoverageSchemaVersion)
	}
	if report.Scope != "pptx-xlsx-office-repair-plus-docx-targeted-invariants" || report.Status != "active" {
		t.Fatalf("unexpected scope/status: %s/%s", report.Scope, report.Status)
	}
	var hasOfficeOpen, hasContentTypes bool
	for _, stage := range report.HarnessStages {
		if stage.Name == "office-open" {
			hasOfficeOpen = true
		}
	}
	for _, class := range report.RepairClasses {
		if class.ID == "content-types" {
			hasContentTypes = true
		}
	}
	if !hasOfficeOpen || !hasContentTypes {
		t.Fatalf("coverage missing office-open stage or content-types class: %+v", report)
	}
}

func TestConformanceCoverageTextIncludesProvenance(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "conformance", "coverage")
	if err != nil {
		t.Fatalf("conformance coverage text failed: %v\n%s", err, output)
	}
	for _, want := range []string{
		"Fixture sets:",
		"OOXML_CONTENT_TYPES_ROOT",
		"TestConformanceGoldenSummary",
		"Real Microsoft Office repair prompts",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("coverage text missing %q\n%s", want, output)
		}
	}
}
