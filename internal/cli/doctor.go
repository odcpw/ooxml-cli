package cli

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/doctor"
	"github.com/spf13/cobra"
)

var (
	doctorOnly   string
	doctorOnline bool
)

var doctorCmd = &cobra.Command{
	Use:   "doctor",
	Short: "Diagnose the environment ooxml needs to run",
	Long: `Diagnose whether the local environment is ready to use ooxml.

ooxml is stateless — it edits the files you pass it and owns no persistent
state — so doctor is an advisory environment check, not a state repair tool. It
verifies the binary on PATH is not stale, that the rendering engine and fonts
are available, and that scratch/working directories are writable. Every finding
names the exact command that resolves it. doctor never modifies your files.

Exit codes: 0 = healthy, 1 = findings present.`,
	Args: cobra.NoArgs,
	// Findings return a non-nil (silent) error to set exit 1; keep usage off the
	// findings path so stdout stays pure report data.
	SilenceUsage:  true,
	SilenceErrors: true,
	RunE: func(cmd *cobra.Command, args []string) error {
		report := doctor.Run(buildDoctorEnvironment(), splitCommaList(doctorOnly))
		if GetGlobalConfig(cmd).Format == "json" {
			if err := writeGlobalJSON(cmd, report); err != nil {
				return err
			}
		} else {
			if err := writeGlobalOutput(cmd, []byte(renderDoctorText(report))); err != nil {
				return err
			}
		}
		if !report.Healthy {
			// Findings present: exit 1 with no error message so stdout stays the
			// report (data). renderError returns the code silently for empty msg.
			return &CLIError{ExitCode: doctorFindingsExitCode}
		}
		return nil
	},
}

// doctor findings reuse the conventional "issues found" exit code (1), mirroring
// tools like `git fsck`. It is documented in `doctor capabilities --json`.
const doctorFindingsExitCode = 1

var doctorHealthCmd = &cobra.Command{
	Use:           "health",
	Short:         "One-line environment health summary",
	Long:          "Print a single-line health summary and set an exit code (0 healthy, 1 findings) for CI/scheduling.",
	Args:          cobra.NoArgs,
	SilenceUsage:  true,
	SilenceErrors: true,
	RunE: func(cmd *cobra.Command, args []string) error {
		report := doctor.Run(buildDoctorEnvironment(), splitCommaList(doctorOnly))
		exitCode := 0
		if !report.Healthy {
			exitCode = doctorFindingsExitCode
		}
		if GetGlobalConfig(cmd).Format == "json" {
			if err := writeGlobalJSON(cmd, map[string]any{
				"schemaVersion":   report.SchemaVersion,
				"contractVersion": report.SchemaVersion,
				"tool":            report.Tool,
				"toolVersion":     report.ToolVersion,
				"doctorVersion":   report.DoctorVersion,
				"healthy":         report.Healthy,
				"summary":         report.Summary,
				"findings":        report.Summary.Findings,
				"exitCode":        exitCode,
			}); err != nil {
				return err
			}
		} else {
			state := "healthy"
			if !report.Healthy {
				state = "findings"
			}
			line := fmt.Sprintf("ooxml doctor: %s — %d ok, %d warn, %d fail (%d info)",
				state, report.Summary.OK, report.Summary.Warn, report.Summary.Fail, report.Summary.Info)
			if err := writeGlobalOutput(cmd, []byte(line)); err != nil {
				return err
			}
		}
		if exitCode != 0 {
			return &CLIError{ExitCode: doctorFindingsExitCode}
		}
		return nil
	},
}

type doctorCapabilities struct {
	Tool            string              `json:"tool"`
	DoctorVersion   string              `json:"doctorVersion"`
	ContractVersion int                 `json:"contractVersion"`
	SchemaVersion   int                 `json:"schemaVersion"`
	ReadOnly        bool                `json:"readOnly"`
	Checks          []doctorCapCheck    `json:"checks"`
	ProofLevels     []doctorProofLevel  `json:"proofLevels"`
	ReleaseGates    []doctorReleaseGate `json:"releaseGates"`
	ExitCodes       []doctorCapExitCode `json:"exitCodes"`
	Flags           []string            `json:"flags"`
	Notes           []string            `json:"notes"`
}

type doctorCapCheck struct {
	ID    string `json:"id"`
	Title string `json:"title"`
}

type doctorCapExitCode struct {
	Code    int    `json:"code"`
	Meaning string `json:"meaning"`
}

type doctorProofLevel struct {
	ID             string   `json:"id"`
	Title          string   `json:"title"`
	RequiredChecks []string `json:"requiredChecks,omitempty"`
	Command        string   `json:"command"`
	Meaning        string   `json:"meaning"`
}

type doctorReleaseGate struct {
	ID             string   `json:"id"`
	Title          string   `json:"title"`
	Command        string   `json:"command"`
	PowerShell     string   `json:"powershell"`
	ProofLevel     string   `json:"proofLevel"`
	RequiredChecks []string `json:"requiredChecks,omitempty"`
	RequiresOffice bool     `json:"requiresOffice"`
	Meaning        string   `json:"meaning"`
}

var doctorCapabilitiesCmd = &cobra.Command{
	Use:   "capabilities",
	Short: "Print the machine-readable doctor contract",
	Args:  cobra.NoArgs,
	RunE: func(cmd *cobra.Command, args []string) error {
		caps := doctorCapabilities{
			Tool:            "ooxml",
			DoctorVersion:   doctor.DoctorVersion,
			ContractVersion: doctor.SchemaVersion,
			SchemaVersion:   doctor.SchemaVersion,
			ReadOnly:        true,
			ProofLevels: []doctorProofLevel{
				{
					ID:      "strict-validation",
					Title:   "Strict ooxml validation",
					Command: "ooxml validate --strict <file>",
					Meaning: "Fast package, relationship, XML, VBA, and semantic validation. Necessary, but not Microsoft Office-open proof.",
				},
				{
					ID:             "repair-conformance",
					Title:          "Repair-sensitive conformance invariants",
					RequiredChecks: []string{"office-edit-smoke"},
					Command:        "ooxml --json conformance check <file>",
					Meaning:        "Repo conformance checks for package and XML invariants known to trigger Office repair, beyond strict validation alone.",
				},
				{
					ID:             "openxml-sdk-schema",
					Title:          "Open XML SDK schema validation",
					RequiredChecks: []string{"openxml-sdk-validator"},
					Command:        "dotnet run --project tools/openxml-validator -- <file>",
					Meaning:        "Optional schema-tier validation when a .NET SDK is available.",
				},
				{
					ID:             "libreoffice-open-render",
					Title:          "LibreOffice open/render evidence",
					RequiredChecks: []string{"render-engine"},
					Command:        "ooxml --json conformance check --office-check <file>",
					Meaning:        "Headless compatibility evidence through LibreOffice/soffice, not Microsoft Office proof.",
				},
				{
					ID:             "microsoft-office-com-open",
					Title:          "Desktop Microsoft Office open proof",
					RequiredChecks: []string{"openxml-sdk-validator", "microsoft-office-com", "office-edit-smoke"},
					Command:        `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance`,
					Meaning:        "Ground-truth Windows proof: verify, edit outputs, strict/Open XML SDK validation, conformance checks, then open in Word, Excel, and PowerPoint through COM.",
				},
				{
					ID:             "microsoft-office-vba-com-open",
					Title:          "Desktop Microsoft Office VBA open proof",
					RequiredChecks: []string{"openxml-sdk-validator", "microsoft-office-com", "office-vba-smoke"},
					Command:        `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120`,
					Meaning:        "Ground-truth Windows macro proof: generate Office-native XLSM/PPTM seeds, validate package attach/remove and existing-module replacement, assert real Office-shaped add/remove guards, then open macro-enabled outputs in Excel and PowerPoint through COM with macro execution disabled.",
				},
			},
			ReleaseGates: []doctorReleaseGate{
				{
					ID:             "check-release-fast",
					Title:          "Release gate without desktop Office COM",
					Command:        "make check-release-fast",
					PowerShell:     `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice`,
					ProofLevel:     "openxml-sdk-schema",
					RequiredChecks: []string{"openxml-sdk-validator", "office-edit-smoke"},
					RequiresOffice: false,
					Meaning:        "Runs verify plus the generated DOCX/XLSX/PPTX edit smoke with strict validation, Open XML SDK schema validation, and conformance coverage; skips desktop Office COM.",
				},
				{
					ID:             "check-release-slow",
					Title:          "Release gate with desktop Office COM",
					Command:        "make check-release-slow",
					PowerShell:     `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance`,
					ProofLevel:     "microsoft-office-com-open",
					RequiredChecks: []string{"openxml-sdk-validator", "microsoft-office-com", "office-edit-smoke"},
					RequiresOffice: true,
					Meaning:        "Runs verify plus generated DOCX/XLSX/PPTX edit smoke, Open XML SDK schema validation, conformance coverage, and Word/Excel/PowerPoint COM open proof.",
				},
				{
					ID:             "check-office-vba-schema",
					Title:          "VBA macro gate without final Office open oracle",
					Command:        "make check-office-vba-schema",
					PowerShell:     `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess`,
					ProofLevel:     "openxml-sdk-schema",
					RequiredChecks: []string{"openxml-sdk-validator", "microsoft-office-com", "office-vba-smoke"},
					RequiresOffice: true,
					Meaning:        "Generates Office-native XLSM/PPTM macro seeds through Excel/PowerPoint COM, then runs strict/Open XML SDK validation and guard checks while skipping the final Office-open oracle.",
				},
				{
					ID:             "check-office-vba-com",
					Title:          "VBA macro gate with desktop Office COM",
					Command:        "make check-office-vba-com",
					PowerShell:     `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120`,
					ProofLevel:     "microsoft-office-vba-com-open",
					RequiredChecks: []string{"openxml-sdk-validator", "microsoft-office-com", "office-vba-smoke"},
					RequiresOffice: true,
					Meaning:        "Runs the full XLSM/PPTM VBA smoke: Office-native seed generation, package attach/remove, existing-module replacement, strict/Open XML SDK validation, add/remove guard checks, and Excel/PowerPoint COM open proof.",
				},
			},
			ExitCodes: []doctorCapExitCode{
				{0, "healthy: no findings"},
				{1, "findings present (advisory; see each finding's remediationCommand)"},
				{2, "invalid arguments"},
			},
			Flags: []string{"--json", "--only", "--online", "--pretty"},
			Notes: []string{
				"doctor is advisory and read-only; it never modifies your files or system.",
				"Each finding includes a remediationCommand you can run to resolve it.",
				"--online is reserved for future network probes; no network calls are made today.",
				"`conformance check --office-check` is LibreOffice evidence; Windows Office COM is the Microsoft Office-open proof tier.",
				"`check-release-fast` is the schema/conformance release gate; `check-release-slow` adds desktop Office COM open proof and the VBA smoke gate.",
			},
		}
		for _, id := range doctor.CheckIDs() {
			caps.Checks = append(caps.Checks, doctorCapCheck{ID: id, Title: doctorCheckTitle(id)})
		}
		return writeGlobalJSON(cmd, caps)
	},
}

var doctorRobotDocsCmd = &cobra.Command{
	Use:   "robot-docs",
	Short: "Print a paste-ready agent handbook for ooxml doctor",
	Args:  cobra.NoArgs,
	RunE: func(cmd *cobra.Command, args []string) error {
		return writeGlobalOutput(cmd, []byte(doctorRobotDocs()))
	},
}

func doctorCheckTitle(id string) string {
	// Title is derived from a representative run so capabilities stays in sync
	// with the detectors without duplicating strings.
	for _, c := range doctor.Run(buildDoctorEnvironment(), []string{id}).Checks {
		if c.ID == id {
			return c.Title
		}
	}
	return id
}

// buildDoctorEnvironment wires the real world into the pure detectors.
func buildDoctorEnvironment() doctor.Environment {
	exe, _ := os.Executable()
	wd, _ := os.Getwd()
	return doctor.Environment{
		Tool:           "ooxml",
		RunningVersion: Version,
		RunningExec:    exe,
		GOOS:           runtime.GOOS,
		ProjectRoot:    wd,
		TempDir:        os.TempDir(),
		WorkingDir:     wd,
		LookPath:       exec.LookPath,
		CommandOutput:  runCommandOutput,
		PathExists:     pathExists,
		ProbeWritable:  probeWritable,
		SameFile:       sameFile,
	}
}

// sameFile reports whether two paths refer to the same on-disk file, falling
// back to cleaned-path equality when either cannot be stat'd.
func sameFile(a, b string) bool {
	if a == "" || b == "" {
		return false
	}
	ai, aerr := os.Stat(a)
	bi, berr := os.Stat(b)
	if aerr == nil && berr == nil {
		return os.SameFile(ai, bi)
	}
	ra, ea := resolvePath(a)
	rb, eb := resolvePath(b)
	if ea == nil && eb == nil {
		return ra == rb
	}
	return a == b
}

func resolvePath(p string) (string, error) {
	abs, err := filepath.Abs(p)
	if err != nil {
		return "", err
	}
	if resolved, err := filepath.EvalSymlinks(abs); err == nil {
		return resolved, nil
	}
	return abs, nil
}

// runCommandOutput runs a command and returns trimmed combined stdout. It never
// inherits stdin and is only used for read-only version/inventory probes.
func runCommandOutput(name string, args ...string) (string, error) {
	cmd := exec.Command(name, args...)
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = &out
	if err := cmd.Run(); err != nil {
		return out.String(), err
	}
	return strings.TrimSpace(out.String()), nil
}

// probeWritable confirms a directory exists and accepts a temp file, then removes
// it. It does not create the directory (doctor is read-only).
func probeWritable(dir string) error {
	if dir == "" {
		return fmt.Errorf("no directory configured")
	}
	f, err := os.CreateTemp(dir, ".ooxml-doctor-probe-*")
	if err != nil {
		return err
	}
	name := f.Name()
	_, werr := f.Write([]byte("ok"))
	closeErr := f.Close()
	_ = os.Remove(name)
	if werr != nil {
		return werr
	}
	return closeErr
}

func pathExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func renderDoctorText(report doctor.Report) string {
	var b strings.Builder
	state := "healthy"
	if !report.Healthy {
		state = fmt.Sprintf("%d finding(s)", report.Summary.Findings)
	}
	fmt.Fprintf(&b, "ooxml doctor (%s) — %s\n", report.ToolVersion, state)
	for _, c := range report.Checks {
		fmt.Fprintf(&b, "  [%s] %s: %s\n", doctorStatusGlyph(c.Status), c.ID, c.Detail)
		if c.IsFinding() && c.Command != "" {
			fmt.Fprintf(&b, "        fix: %s\n", c.Command)
		}
	}
	return strings.TrimRight(b.String(), "\n")
}

func doctorStatusGlyph(s doctor.Status) string {
	switch s {
	case doctor.StatusOK:
		return "ok"
	case doctor.StatusWarn:
		return "warn"
	case doctor.StatusFail:
		return "fail"
	default:
		return "info"
	}
}

func doctorRobotDocs() string {
	ids := strings.Join(doctor.CheckIDs(), ", ")
	return strings.TrimSpace(`
ooxml doctor — agent handbook

PURPOSE
  Confirm the local environment can run ooxml before you rely on it. doctor is
  advisory and READ-ONLY: it never edits your files or installs anything.

COMMANDS
  ooxml --json doctor                  Full report. Exit 0 healthy, 1 if findings.
  ooxml --json doctor health           One-line summary + exit code (CI-friendly).
  ooxml --json doctor capabilities     Machine-readable contract (checks, exit codes).
  ooxml doctor robot-docs              This handbook.
  make check-release-fast              Verify + edit smoke + Open XML SDK + conformance; skips desktop Office COM.
  make check-release-slow              Verify + edit smoke + Open XML SDK + conformance + desktop Office COM + VBA smoke.
  make check-office-vba-schema         VBA smoke with Office-native seeds + strict/Open XML SDK; skips final open oracle.
  make check-office-vba-com            VBA smoke with Excel/PowerPoint COM open proof.

NO-MAKE WINDOWS COMMANDS
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice
                                       Same proof as check-release-fast.
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance
                                       Same proof as check-release-slow; requires desktop Word, Excel, and PowerPoint.
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess
                                       VBA schema proof: generates Office-native XLSM/PPTM seeds, validates, and checks add/remove guards; skips final open oracle.
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120
                                       VBA COM proof: also opens macro-enabled outputs in desktop Excel and PowerPoint with macro execution disabled.

FLAGS
  --json            Emit a stable JSON report (schemaVersion pinned).
  --only id,...     Run only the named checks (` + ids + `).
  --online          Reserved for future network probes; no network today.

PROOF LADDER
  strict-validation            ooxml validate --strict <file>
  repair-conformance          ooxml --json conformance check <file>
  openxml-sdk-schema           requires openxml-sdk-validator
  libreoffice-open-render      requires render-engine; compatibility evidence, not Microsoft Office proof
  microsoft-office-com-open    requires openxml-sdk-validator + microsoft-office-com + office-edit-smoke
  microsoft-office-vba-com-open requires openxml-sdk-validator + microsoft-office-com + office-vba-smoke

RELEASE GATES
  check-release-fast           proofLevel=openxml-sdk-schema; includes repair-conformance; requires openxml-sdk-validator + office-edit-smoke
  check-release-slow           proofLevel=microsoft-office-com-open; includes repair-conformance and VBA smoke; requires openxml-sdk-validator + microsoft-office-com + office-edit-smoke + office-vba-smoke
  check-office-vba-schema      proofLevel=openxml-sdk-schema; requires openxml-sdk-validator + microsoft-office-com + office-vba-smoke
  check-office-vba-com         proofLevel=microsoft-office-vba-com-open; requires openxml-sdk-validator + microsoft-office-com + office-vba-smoke

HOW TO USE THE JSON
  .healthy            bool — true when there are no findings.
  .summary            counts {ok,warn,fail,info,findings}.
  .checks[]           {id,title,status,severity,detail,remediation,remediationCommand}.
  status is one of ok|warn|fail|info; warn and fail are findings.
  For each finding, run .remediationCommand to resolve it, then re-run doctor.

TYPICAL FIRST STEP FOR AN AGENT
  ooxml --json doctor health   # if exit != 0, run 'ooxml --json doctor' and act on findings.
`)
}

func init() {
	doctorCmd.PersistentFlags().StringVar(&doctorOnly, "only", "", "run only the named checks (comma-separated)")
	doctorCmd.PersistentFlags().BoolVar(&doctorOnline, "online", false, "reserved: enable network probes (none today)")
	doctorCmd.AddCommand(doctorHealthCmd)
	doctorCmd.AddCommand(doctorCapabilitiesCmd)
	doctorCmd.AddCommand(doctorRobotDocsCmd)
	rootCmd.AddCommand(doctorCmd)
}
