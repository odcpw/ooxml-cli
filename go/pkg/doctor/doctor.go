// Package doctor implements ooxml-cli's environment/readiness self-diagnosis.
//
// ooxml-cli is a stateless tool: it operates on files the caller passes and
// owns almost no persistent state of its own. Its doctor is therefore an
// advisory environment doctor rather than a state-repairing one. Every detector
// here is a pure function of an injected Environment so it is deterministic and
// testable; detectors never mutate anything. The CLI layer decides how to render
// the report and what exit code to return. Each finding names the exact command
// that resolves it, so an agent can act without guessing.
package doctor

import (
	"fmt"
	"path/filepath"
	"sort"
	"strings"
)

// SchemaVersion is the stable contract version of the doctor report JSON.
const SchemaVersion = 1

// DoctorVersion tracks the doctor implementation; bump on detector changes.
const DoctorVersion = "1.3.0"

// Status is a per-check outcome.
type Status string

const (
	StatusOK   Status = "ok"   // healthy
	StatusWarn Status = "warn" // a finding that should be addressed but is non-fatal
	StatusFail Status = "fail" // a finding that blocks a core workflow
	StatusInfo Status = "info" // informational, never a finding
)

// Severity classifies how much a finding matters.
type Severity string

const (
	SeverityInfo    Severity = "info"
	SeverityWarning Severity = "warning"
	SeverityError   Severity = "error"
)

// Check is the result of one detector. It is data only — no behavior.
type Check struct {
	ID          string   `json:"id"`
	Title       string   `json:"title"`
	Status      Status   `json:"status"`
	Severity    Severity `json:"severity"`
	Detail      string   `json:"detail"`
	Remediation string   `json:"remediation,omitempty"`
	Command     string   `json:"remediationCommand,omitempty"`
}

// IsFinding reports whether the check represents something to act on.
func (c Check) IsFinding() bool { return c.Status == StatusWarn || c.Status == StatusFail }

// Summary aggregates check statuses.
type Summary struct {
	Total    int `json:"total"`
	OK       int `json:"ok"`
	Warn     int `json:"warn"`
	Fail     int `json:"fail"`
	Info     int `json:"info"`
	Findings int `json:"findings"`
}

// Report is the full doctor output.
type Report struct {
	SchemaVersion int     `json:"schemaVersion"`
	Tool          string  `json:"tool"`
	ToolVersion   string  `json:"toolVersion"`
	DoctorVersion string  `json:"doctorVersion"`
	Healthy       bool    `json:"healthy"`
	Summary       Summary `json:"summary"`
	Checks        []Check `json:"checks"`
}

// Environment injects every external dependency a detector needs, so detectors
// stay pure and the CLI (or a test) supplies the real or fake world.
type Environment struct {
	Tool           string                                            // binary name, e.g. "ooxml"
	RunningVersion string                                            // version of the executing binary
	RunningExec    string                                            // absolute path of the executing binary
	GOOS           string                                            // runtime.GOOS
	ProjectRoot    string                                            // source checkout root when available
	TempDir        string                                            // os.TempDir()
	WorkingDir     string                                            // cwd
	LookPath       func(name string) (string, error)                 // exec.LookPath
	CommandOutput  func(name string, args ...string) (string, error) // run a command, return trimmed stdout
	PathExists     func(path string) bool                            // true if path exists
	ProbeWritable  func(dir string) error                            // nil if a probe file can be created+written+removed
	SameFile       func(a, b string) bool                            // true if two paths are the same on-disk file
}

// detector is a pure check over the environment.
type detector struct {
	id string
	fn func(Environment) Check
}

func allDetectors() []detector {
	return []detector{
		{"binary", checkBinary},
		{"render-engine", checkRenderEngine},
		{"fonts", checkFonts},
		{"tempdir", checkTempDir},
		{"workdir", checkWorkingDir},
		{"go-toolchain", checkGoToolchain},
		{"openxml-sdk-validator", checkOpenXMLSDKValidator},
		{"microsoft-office-com", checkMicrosoftOfficeCOM},
		{"office-edit-smoke", checkOfficeEditSmoke},
		{"office-vba-smoke", checkOfficeVBASmoke},
	}
}

// CheckIDs returns the stable ordered list of detector IDs (for capabilities).
func CheckIDs() []string {
	ds := allDetectors()
	ids := make([]string, len(ds))
	for i, d := range ds {
		ids[i] = d.id
	}
	return ids
}

// Run executes the detectors (optionally filtered to only) and builds a report.
func Run(env Environment, only []string) Report {
	wanted := map[string]bool{}
	for _, id := range only {
		wanted[strings.TrimSpace(id)] = true
	}
	report := Report{
		SchemaVersion: SchemaVersion,
		Tool:          env.Tool,
		ToolVersion:   env.RunningVersion,
		DoctorVersion: DoctorVersion,
	}
	for _, d := range allDetectors() {
		if len(wanted) > 0 && !wanted[d.id] {
			continue
		}
		check := d.fn(env)
		check.ID = d.id
		report.Checks = append(report.Checks, check)
	}
	report.Summary = summarize(report.Checks)
	report.Healthy = report.Summary.Findings == 0
	return report
}

func summarize(checks []Check) Summary {
	s := Summary{Total: len(checks)}
	for _, c := range checks {
		switch c.Status {
		case StatusOK:
			s.OK++
		case StatusWarn:
			s.Warn++
		case StatusFail:
			s.Fail++
		case StatusInfo:
			s.Info++
		}
		if c.IsFinding() {
			s.Findings++
		}
	}
	return s
}

// ---- detectors (pure) ----

// checkBinary detects a stale or missing on-PATH binary — the first failure an
// agent hits when an old build shadows a freshly built one.
func checkBinary(env Environment) Check {
	c := Check{Title: "Installed binary matches this build"}
	path, err := env.LookPath(env.Tool)
	if err != nil || path == "" {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("%q is not on PATH (running build is %s)", env.Tool, env.RunningVersion)
		c.Remediation = "Build and install the binary onto your PATH."
		c.Command = "go build -o \"$(go env GOPATH)/bin/" + env.Tool + "\" ./cmd/" + env.Tool
		return c
	}
	// If you are invoking the very binary that's on PATH, it cannot be stale
	// relative to itself — report OK without comparing version strings (install
	// schemes vary, e.g. git-sha vs semver).
	if env.RunningExec != "" && env.SameFile != nil && env.SameFile(path, env.RunningExec) {
		c.Status = StatusOK
		c.Severity = SeverityInfo
		c.Detail = fmt.Sprintf("invoking the on-PATH %s at %s (%s)", env.Tool, path, env.RunningVersion)
		return c
	}
	version, verr := env.CommandOutput(path, "version")
	version = strings.TrimSpace(version)
	if verr != nil || version == "" {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("%s on PATH at %s did not report a version", env.Tool, path)
		c.Remediation = "Rebuild and reinstall the binary; the PATH copy may predate the version command."
		c.Command = "go build -o " + path + " ./cmd/" + env.Tool
		return c
	}
	if version != env.RunningVersion {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("PATH %s at %s is %s, but you are invoking %s from %s — the PATH copy may be stale", env.Tool, path, version, env.RunningVersion, env.RunningExec)
		c.Remediation = "Reinstall the freshly built binary over the stale PATH copy."
		c.Command = "go build -o " + path + " ./cmd/" + env.Tool
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("%s %s on PATH at %s matches the version you are invoking", env.Tool, version, path)
	return c
}

// checkRenderEngine detects LibreOffice, required by `render` and PDF output.
func checkRenderEngine(env Environment) Check {
	c := Check{Title: "Rendering engine (LibreOffice) available"}
	for _, bin := range []string{"libreoffice", "soffice"} {
		if path, err := env.LookPath(bin); err == nil && path != "" {
			version, _ := env.CommandOutput(path, "--version")
			version = firstLine(version)
			c.Status = StatusOK
			c.Severity = SeverityInfo
			if version != "" {
				c.Detail = fmt.Sprintf("%s found at %s (%s)", bin, path, version)
			} else {
				c.Detail = fmt.Sprintf("%s found at %s", bin, path)
			}
			return c
		}
	}
	c.Status = StatusWarn
	c.Severity = SeverityWarning
	c.Detail = "no libreoffice/soffice on PATH; `render` and PDF output will be unavailable"
	c.Remediation = "Install LibreOffice to enable rendering/visual verification."
	c.Command = installHint(env.GOOS, "libreoffice")
	return c
}

// checkFonts detects fontconfig, which affects render text fidelity.
func checkFonts(env Environment) Check {
	c := Check{Title: "Fonts available for rendering"}
	path, err := env.LookPath("fc-list")
	if err != nil || path == "" {
		c.Status = StatusInfo
		c.Severity = SeverityInfo
		c.Detail = "fontconfig (fc-list) not found; cannot verify font availability — rendered text may use fallback fonts"
		c.Remediation = "Install fontconfig and common fonts for higher-fidelity rendering."
		c.Command = installHint(env.GOOS, "fontconfig")
		return c
	}
	out, _ := env.CommandOutput(path)
	count := countNonEmptyLines(out)
	if count == 0 {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = "fontconfig is installed but reports no fonts; rendered documents may be blank or use fallbacks"
		c.Remediation = "Install a base font set (e.g. DejaVu, Liberation)."
		c.Command = installHint(env.GOOS, "fonts")
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("%d fonts available via fontconfig", count)
	return c
}

// checkTempDir detects whether scratch space (used during mutation/render) works.
func checkTempDir(env Environment) Check {
	c := Check{Title: "Temp directory is writable"}
	if err := env.ProbeWritable(env.TempDir); err != nil {
		c.Status = StatusFail
		c.Severity = SeverityError
		c.Detail = fmt.Sprintf("temp dir %s is not writable: %v", env.TempDir, err)
		c.Remediation = "Point TMPDIR at a writable directory."
		c.Command = "export TMPDIR=\"$HOME/.cache/ooxml-tmp\" && mkdir -p \"$TMPDIR\""
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("temp dir %s is writable", env.TempDir)
	return c
}

// checkWorkingDir warns when the cwd is read-only (mutations need an --out target).
func checkWorkingDir(env Environment) Check {
	c := Check{Title: "Working directory is writable"}
	if err := env.ProbeWritable(env.WorkingDir); err != nil {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("working directory %s is not writable: %v", env.WorkingDir, err)
		c.Remediation = "Write outputs elsewhere with an absolute --out path."
		c.Command = env.Tool + " <cmd> <file> --out /tmp/out.xlsx"
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("working directory %s is writable", env.WorkingDir)
	return c
}

// checkGoToolchain notes whether the binary can be rebuilt from source.
func checkGoToolchain(env Environment) Check {
	c := Check{Title: "Go toolchain available (for rebuilds)"}
	if path, err := env.LookPath("go"); err == nil && path != "" {
		version, _ := env.CommandOutput(path, "version")
		c.Status = StatusOK
		c.Severity = SeverityInfo
		c.Detail = strings.TrimSpace("go found at " + path + " " + firstLine(version))
		return c
	}
	c.Status = StatusInfo
	c.Severity = SeverityInfo
	c.Detail = "go not found on PATH; a stale binary cannot be rebuilt from source here"
	c.Remediation = "Install Go to rebuild the binary from source."
	c.Command = installHint(env.GOOS, "go")
	return c
}

// checkOpenXMLSDKValidator detects whether the optional .NET/Open XML SDK
// validation tier can be used from this checkout.
func checkOpenXMLSDKValidator(env Environment) Check {
	c := Check{Title: "Open XML SDK validator available"}
	root := checkoutRoot(env)
	project := filepath.Join(root, "tools", "openxml-validator", "openxml-validator.csproj")
	if !pathExists(env, project) {
		c.Status = StatusInfo
		c.Severity = SeverityInfo
		c.Detail = fmt.Sprintf("Open XML SDK validator project not found under %s; schema-tier validation is unavailable from this location", root)
		c.Remediation = "Run doctor from a source checkout that includes tools/openxml-validator, or skip this optional proof tier."
		return c
	}
	dotnet, err := env.LookPath("dotnet")
	if err != nil || dotnet == "" {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = "tools/openxml-validator exists, but dotnet is not on PATH"
		c.Remediation = "Install a .NET SDK to enable Open XML SDK schema validation."
		c.Command = installHint(env.GOOS, "dotnet-sdk")
		return c
	}
	out, err := env.CommandOutput(dotnet, "--list-sdks")
	if err != nil || countNonEmptyLines(out) == 0 {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("dotnet found at %s, but no .NET SDK is available; runtimes alone cannot build tools/openxml-validator", dotnet)
		c.Remediation = "Install a .NET SDK, then run the validator project."
		c.Command = installHint(env.GOOS, "dotnet-sdk")
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("dotnet SDK available at %s (%s); validator project is %s", dotnet, firstLine(out), project)
	return c
}

// checkMicrosoftOfficeCOM verifies the lightweight prerequisite for the Windows
// ground-truth oracle: Word, Excel, and PowerPoint COM classes are registered.
func checkMicrosoftOfficeCOM(env Environment) Check {
	c := Check{Title: "Microsoft Office COM automation available"}
	if env.GOOS != "windows" {
		c.Status = StatusInfo
		c.Severity = SeverityInfo
		c.Detail = "desktop Microsoft Office COM proof is Windows-only; use the Windows oracle on a machine with Office installed"
		return c
	}
	powershell, err := lookAny(env, "powershell.exe", "pwsh")
	if err != nil || powershell == "" {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = "PowerShell is not on PATH, so Office COM registration cannot be probed"
		c.Remediation = "Install PowerShell or use a normal Windows shell with powershell.exe available."
		c.Command = installHint(env.GOOS, "powershell")
		return c
	}
	script := `$ErrorActionPreference='Stop'; $apps=@('Excel.Application','PowerPoint.Application','Word.Application'); foreach ($app in $apps) { if ($null -eq [type]::GetTypeFromProgID($app)) { throw "$app not registered" }; Write-Output $app }`
	out, err := env.CommandOutput(powershell, "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script)
	if err != nil {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("Office COM probe failed via %s: %s", powershell, firstLine(out))
		c.Remediation = "Install Microsoft 365/desktop Office with Word, Excel, and PowerPoint, then re-run doctor."
		c.Command = "winget install Microsoft.Office"
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("Office COM classes registered via %s: %s", powershell, strings.Join(nonEmptyLines(out), ", "))
	return c
}

// checkOfficeEditSmoke detects the reusable edit->validate->Office-open gate.
func checkOfficeEditSmoke(env Environment) Check {
	c := Check{Title: "Windows Office edit smoke gate available"}
	if env.GOOS != "windows" {
		c.Status = StatusInfo
		c.Severity = SeverityInfo
		c.Detail = "tools/windows-office-edit-smoke.ps1 is a Windows-only Microsoft Office proof gate"
		return c
	}
	root := checkoutRoot(env)
	script := filepath.Join(root, "tools", "windows-office-edit-smoke.ps1")
	if !pathExists(env, script) {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("Office edit smoke script not found at %s", script)
		c.Remediation = "Run doctor from the ooxml-cli source checkout that includes tools/windows-office-edit-smoke.ps1."
		c.Command = "git checkout -- tools/windows-office-edit-smoke.ps1"
		return c
	}
	powershell, err := lookAny(env, "powershell.exe", "pwsh")
	if err != nil || powershell == "" {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = "PowerShell is not on PATH, so the Office edit smoke gate cannot run"
		c.Remediation = "Install PowerShell or use a normal Windows shell with powershell.exe available."
		c.Command = installHint(env.GOOS, "powershell")
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("Office edit smoke gate is available: %s (release gates: make check-release-fast, make check-release-slow)", script)
	c.Command = fmt.Sprintf("%s -NoProfile -ExecutionPolicy Bypass -File %s -RepoRoot %s -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance", powershell, quotePath(script), quotePath(root))
	return c
}

// checkOfficeVBASmoke detects the reusable XLSM/PPTM macro package proof gate.
func checkOfficeVBASmoke(env Environment) Check {
	c := Check{Title: "Windows Office VBA smoke gate available"}
	if env.GOOS != "windows" {
		c.Status = StatusInfo
		c.Severity = SeverityInfo
		c.Detail = "tools/windows-office-vba-smoke.ps1 is a Windows-only Microsoft Office VBA proof gate"
		return c
	}
	root := checkoutRoot(env)
	script := filepath.Join(root, "tools", "windows-office-vba-smoke.ps1")
	if !pathExists(env, script) {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = fmt.Sprintf("Office VBA smoke script not found at %s", script)
		c.Remediation = "Run doctor from the ooxml-cli source checkout that includes tools/windows-office-vba-smoke.ps1."
		c.Command = "git checkout -- tools/windows-office-vba-smoke.ps1"
		return c
	}
	powershell, err := lookAny(env, "powershell.exe", "pwsh")
	if err != nil || powershell == "" {
		c.Status = StatusWarn
		c.Severity = SeverityWarning
		c.Detail = "PowerShell is not on PATH, so the Office VBA smoke gate cannot run"
		c.Remediation = "Install PowerShell or use a normal Windows shell with powershell.exe available."
		c.Command = installHint(env.GOOS, "powershell")
		return c
	}
	c.Status = StatusOK
	c.Severity = SeverityInfo
	c.Detail = fmt.Sprintf("Office VBA smoke gate is available: %s (release gate: make check-office-vba-com; check-release-slow also runs it)", script)
	c.Command = fmt.Sprintf("%s -NoProfile -ExecutionPolicy Bypass -File %s -RepoRoot %s -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120", powershell, quotePath(script), quotePath(root))
	return c
}

// ---- helpers ----

func firstLine(s string) string {
	s = strings.TrimSpace(s)
	if i := strings.IndexByte(s, '\n'); i >= 0 {
		return strings.TrimSpace(s[:i])
	}
	return s
}

func countNonEmptyLines(s string) int {
	return len(nonEmptyLines(s))
}

func nonEmptyLines(s string) []string {
	lines := []string{}
	for _, line := range strings.Split(s, "\n") {
		line = strings.TrimSpace(line)
		if line != "" {
			lines = append(lines, line)
		}
	}
	return lines
}

func checkoutRoot(env Environment) string {
	if env.ProjectRoot != "" {
		return env.ProjectRoot
	}
	return env.WorkingDir
}

func pathExists(env Environment, path string) bool {
	if env.PathExists == nil {
		return false
	}
	return env.PathExists(path)
}

func lookAny(env Environment, names ...string) (string, error) {
	var last error
	for _, name := range names {
		path, err := env.LookPath(name)
		if err == nil && path != "" {
			return path, nil
		}
		last = err
	}
	if last == nil {
		last = fmt.Errorf("not found")
	}
	return "", last
}

func quotePath(path string) string {
	if strings.ContainsAny(path, " \t") {
		return `"` + strings.ReplaceAll(path, `"`, `\"`) + `"`
	}
	return path
}

// installHint returns a best-effort, platform-aware install command. It is a
// hint only — the doctor never runs it.
func installHint(goos, what string) string {
	pkgs := map[string]map[string]string{
		"linux": {
			"libreoffice": "sudo apt-get install libreoffice  # or: sudo pacman -S libreoffice-fresh",
			"fontconfig":  "sudo apt-get install fontconfig fonts-dejavu  # or your distro equivalent",
			"fonts":       "sudo apt-get install fonts-dejavu fonts-liberation",
			"go":          "see https://go.dev/dl/ or: sudo apt-get install golang",
		},
		"darwin": {
			"libreoffice": "brew install --cask libreoffice",
			"fontconfig":  "brew install fontconfig",
			"fonts":       "brew install --cask font-dejavu",
			"go":          "brew install go",
		},
		"windows": {
			"libreoffice": "winget install TheDocumentFoundation.LibreOffice",
			"fontconfig":  "(fontconfig is not used on Windows)",
			"fonts":       "install fonts via Settings > Personalization > Fonts",
			"go":          "winget install GoLang.Go",
			"dotnet-sdk":  "winget install Microsoft.DotNet.SDK.8",
			"powershell":  "winget install Microsoft.PowerShell",
		},
	}
	if byWhat, ok := pkgs[goos]; ok {
		if cmd, ok := byWhat[what]; ok {
			return cmd
		}
	}
	return "install " + what + " using your platform package manager"
}

// SortChecksByID returns checks sorted by ID (stable output for diffing).
func SortChecksByID(checks []Check) []Check {
	out := append([]Check(nil), checks...)
	sort.SliceStable(out, func(i, j int) bool { return out[i].ID < out[j].ID })
	return out
}
