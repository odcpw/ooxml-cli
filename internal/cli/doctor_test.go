package cli

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/doctor"
)

func TestDoctorJSONReport(t *testing.T) {
	out, _ := executeRootForXLSXTest(t, "--json", "doctor")
	// Exit status is environment-dependent (a stale PATH binary, missing
	// LibreOffice, etc. are legitimate findings), so we assert structure, not
	// the exit code, here.
	var report doctor.Report
	if err := json.Unmarshal([]byte(out), &report); err != nil {
		t.Fatalf("doctor --json did not emit valid JSON: %v\n%s", err, out)
	}
	if report.SchemaVersion != doctor.SchemaVersion {
		t.Fatalf("unexpected schemaVersion: %d", report.SchemaVersion)
	}
	if len(report.Checks) != len(doctor.CheckIDs()) {
		t.Fatalf("expected %d checks, got %d", len(doctor.CheckIDs()), len(report.Checks))
	}
	if report.Summary.Total != len(report.Checks) {
		t.Fatalf("summary total %d != checks %d", report.Summary.Total, len(report.Checks))
	}
	for _, c := range report.Checks {
		if c.ID == "" || c.Status == "" {
			t.Fatalf("check missing id/status: %+v", c)
		}
		if c.IsFinding() && strings.TrimSpace(c.Command) == "" {
			t.Fatalf("finding %s has no remediation command", c.ID)
		}
	}
}

func TestDoctorTempdirHealthyExitsZero(t *testing.T) {
	// The temp dir is writable in the test environment, so scoping to it yields
	// a healthy report and a nil (exit 0) result deterministically.
	out, err := executeRootForXLSXTest(t, "--json", "doctor", "--only", "tempdir")
	if err != nil {
		t.Fatalf("doctor --only tempdir should be healthy: %v\n%s", err, out)
	}
	var report doctor.Report
	if err := json.Unmarshal([]byte(out), &report); err != nil {
		t.Fatalf("invalid JSON: %v\n%s", err, out)
	}
	if len(report.Checks) != 1 || report.Checks[0].ID != "tempdir" {
		t.Fatalf("expected only the tempdir check: %+v", report.Checks)
	}
	if !report.Healthy {
		t.Fatalf("expected healthy tempdir report: %+v", report)
	}
}

func TestDoctorFindingsExitOne(t *testing.T) {
	// Scope to a check we can force into a finding: an unknown check id yields
	// zero checks -> healthy, so instead drive the binary check, which is a
	// finding whenever the test binary is not the on-PATH ooxml. To keep this
	// deterministic we assert the error type only when a finding is present.
	out, err := executeRootForXLSXTest(t, "--json", "doctor")
	var report doctor.Report
	if jerr := json.Unmarshal([]byte(out), &report); jerr != nil {
		t.Fatalf("invalid JSON: %v", jerr)
	}
	if report.Healthy {
		if err != nil {
			t.Fatalf("healthy report must exit 0, got %v", err)
		}
		return
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != 1 {
		t.Fatalf("findings present must return exit 1, got %v", err)
	}
}

func TestDoctorCapabilities(t *testing.T) {
	out, err := executeRootForXLSXTest(t, "--json", "doctor", "capabilities")
	if err != nil {
		t.Fatalf("doctor capabilities failed: %v", err)
	}
	var caps doctorCapabilities
	if err := json.Unmarshal([]byte(out), &caps); err != nil {
		t.Fatalf("invalid capabilities JSON: %v\n%s", err, out)
	}
	if !caps.ReadOnly {
		t.Fatalf("doctor must declare itself read-only")
	}
	if len(caps.Checks) != len(doctor.CheckIDs()) {
		t.Fatalf("capabilities checks out of sync: %d vs %d", len(caps.Checks), len(doctor.CheckIDs()))
	}
	var proofIDs []string
	var hasRepairConformance, hasOfficeProof, hasVBAOfficeProof bool
	for _, p := range caps.ProofLevels {
		proofIDs = append(proofIDs, p.ID)
		if p.ID == "repair-conformance" &&
			p.Command == "ooxml --json conformance check <file>" &&
			strings.Contains(p.Meaning, "Office repair") {
			hasRepairConformance = true
		}
		if p.ID == "microsoft-office-com-open" &&
			strings.Contains(p.Command, "windows-office-edit-smoke.ps1") &&
			strings.Contains(p.Command, "-MutationParallelism 4") &&
			strings.Contains(p.Command, "-RequireOpenXmlSdk") &&
			strings.Contains(p.Command, "-RunConformance") {
			hasOfficeProof = true
		}
		if p.ID == "microsoft-office-vba-com-open" &&
			strings.Contains(p.Command, "windows-office-vba-smoke.ps1") &&
			strings.Contains(p.Command, "-EnableVbaObjectModelAccess") &&
			strings.Contains(p.Meaning, "add/remove guards") {
			hasVBAOfficeProof = true
		}
	}
	if !hasRepairConformance {
		t.Fatalf("capabilities missing repair-conformance proof level: %+v", caps.ProofLevels)
	}
	if got, want := strings.Join(proofIDs, ","), "strict-validation,repair-conformance,openxml-sdk-schema,libreoffice-open-render,microsoft-office-com-open,microsoft-office-vba-com-open"; got != want {
		t.Fatalf("proof level order drifted: %s", got)
	}
	if !hasOfficeProof {
		t.Fatalf("capabilities missing Microsoft Office proof level: %+v", caps.ProofLevels)
	}
	if !hasVBAOfficeProof {
		t.Fatalf("capabilities missing Microsoft Office VBA proof level: %+v", caps.ProofLevels)
	}
	var hasFastGate, hasSlowGate, hasVBASchemaGate, hasVBAComGate bool
	for _, gate := range caps.ReleaseGates {
		switch gate.ID {
		case "check-release-fast":
			if gate.Command == "make check-release-fast" &&
				gate.ProofLevel == "openxml-sdk-schema" &&
				!gate.RequiresOffice &&
				strings.Contains(gate.PowerShell, "-RunConformance") &&
				strings.Contains(gate.PowerShell, "-SkipOffice") {
				hasFastGate = true
			}
		case "check-release-slow":
			if gate.Command == "make check-release-slow" &&
				gate.ProofLevel == "microsoft-office-com-open" &&
				gate.RequiresOffice &&
				strings.Contains(gate.PowerShell, "-RunConformance") &&
				!strings.Contains(gate.PowerShell, "-SkipOffice") {
				hasSlowGate = true
			}
		case "check-office-vba-schema":
			if gate.Command == "make check-office-vba-schema" &&
				gate.ProofLevel == "openxml-sdk-schema" &&
				gate.RequiresOffice &&
				strings.Contains(gate.PowerShell, "windows-office-vba-smoke.ps1") &&
				strings.Contains(gate.PowerShell, "-SkipOffice") &&
				strings.Contains(gate.PowerShell, "-EnableVbaObjectModelAccess") {
				hasVBASchemaGate = true
			}
		case "check-office-vba-com":
			if gate.Command == "make check-office-vba-com" &&
				gate.ProofLevel == "microsoft-office-vba-com-open" &&
				gate.RequiresOffice &&
				strings.Contains(gate.PowerShell, "windows-office-vba-smoke.ps1") &&
				strings.Contains(gate.PowerShell, "-EnableVbaObjectModelAccess") &&
				!strings.Contains(gate.PowerShell, "-SkipOffice") {
				hasVBAComGate = true
			}
		}
	}
	if !hasFastGate || !hasSlowGate || !hasVBASchemaGate || !hasVBAComGate {
		t.Fatalf("capabilities missing release gates: %+v", caps.ReleaseGates)
	}
	var hasHealthy, hasFindings bool
	for _, e := range caps.ExitCodes {
		if e.Code == 0 {
			hasHealthy = true
		}
		if e.Code == 1 {
			hasFindings = true
		}
	}
	if !hasHealthy || !hasFindings {
		t.Fatalf("capabilities must document exit codes 0 and 1: %+v", caps.ExitCodes)
	}
}

func TestDoctorRobotDocs(t *testing.T) {
	out, err := executeRootForXLSXTest(t, "doctor", "robot-docs")
	if err != nil {
		t.Fatalf("doctor robot-docs failed: %v", err)
	}
	for _, want := range []string{"agent handbook", "READ-ONLY", "remediationCommand", "binary", "-RequireOpenXmlSdk", "-RunConformance", "-SkipOffice", "-EnableVbaObjectModelAccess", "repair-conformance", "check-release-fast", "check-release-slow", "check-office-vba-schema", "check-office-vba-com"} {
		if !strings.Contains(out, want) {
			t.Fatalf("robot-docs missing %q:\n%s", want, out)
		}
	}
}

func TestDoctorHealth(t *testing.T) {
	out, err := executeRootForXLSXTest(t, "--json", "doctor", "health", "--only", "tempdir")
	if err != nil {
		t.Fatalf("doctor health --only tempdir should be healthy: %v", err)
	}
	var payload struct {
		SchemaVersion   int    `json:"schemaVersion"`
		ContractVersion int    `json:"contractVersion"`
		Tool            string `json:"tool"`
		ToolVersion     string `json:"toolVersion"`
		DoctorVersion   string `json:"doctorVersion"`
		Healthy         bool   `json:"healthy"`
		Findings        int    `json:"findings"`
		ExitCode        int    `json:"exitCode"`
	}
	if err := json.Unmarshal([]byte(out), &payload); err != nil {
		t.Fatalf("invalid health JSON: %v\n%s", err, out)
	}
	if payload.Tool != "ooxml" || !payload.Healthy {
		t.Fatalf("unexpected health payload: %+v", payload)
	}
	if payload.SchemaVersion != doctor.SchemaVersion ||
		payload.ContractVersion != doctor.SchemaVersion ||
		payload.ToolVersion != Version ||
		payload.DoctorVersion != doctor.DoctorVersion ||
		payload.ExitCode != 0 {
		t.Fatalf("health payload missing stable metadata: %+v", payload)
	}
}
