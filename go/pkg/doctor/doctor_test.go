package doctor

import (
	"fmt"
	"strings"
	"testing"
)

// healthyEnv returns an Environment where every detector should report OK.
func healthyEnv() Environment {
	paths := map[string]string{
		"ooxml":       "/usr/bin/ooxml",
		"libreoffice": "/usr/bin/libreoffice",
		"fc-list":     "/usr/bin/fc-list",
		"go":          "/usr/bin/go",
		"dotnet":      "/usr/bin/dotnet",
	}
	return Environment{
		Tool:           "ooxml",
		RunningVersion: "1.0",
		RunningExec:    "/usr/bin/ooxml",
		GOOS:           "linux",
		ProjectRoot:    "/work",
		TempDir:        "/tmp",
		WorkingDir:     "/work",
		LookPath: func(name string) (string, error) {
			if p, ok := paths[name]; ok {
				return p, nil
			}
			return "", fmt.Errorf("%s not found", name)
		},
		CommandOutput: func(name string, args ...string) (string, error) {
			switch {
			case strings.HasSuffix(name, "fc-list"):
				return "DejaVu Sans\nLiberation Serif\n", nil
			case strings.HasSuffix(name, "libreoffice"):
				return "LibreOffice 24.2", nil
			case strings.HasSuffix(name, "go"):
				return "go version go1.23 linux/amd64", nil
			case strings.HasSuffix(name, "dotnet"):
				return "8.0.100 [/usr/lib/dotnet/sdk]", nil
			case strings.HasSuffix(name, "ooxml"):
				return "1.0", nil
			}
			return "", nil
		},
		PathExists:    func(path string) bool { return true },
		ProbeWritable: func(dir string) error { return nil },
		SameFile:      func(a, b string) bool { return a == b },
	}
}

func find(report Report, id string) Check {
	for _, c := range report.Checks {
		if c.ID == id {
			return c
		}
	}
	return Check{}
}

func TestHealthyEnvironment(t *testing.T) {
	report := Run(healthyEnv(), nil)
	if !report.Healthy {
		t.Fatalf("expected healthy, got findings: %+v", report.Summary)
	}
	if report.Summary.Findings != 0 || report.Summary.Total != 10 {
		t.Fatalf("unexpected summary: %+v", report.Summary)
	}
	for _, c := range report.Checks {
		if c.IsFinding() {
			t.Fatalf("unexpected finding on healthy env: %s = %s", c.ID, c.Status)
		}
	}
}

func TestStaleBinaryWarns(t *testing.T) {
	env := healthyEnv()
	env.RunningExec = "/home/u/build/ooxml" // different file than the PATH copy
	env.SameFile = func(a, b string) bool { return false }
	env.CommandOutput = func(name string, args ...string) (string, error) {
		if strings.HasSuffix(name, "ooxml") {
			return "0.9", nil // PATH binary is an older version
		}
		return healthyEnv().CommandOutput(name, args...)
	}
	c := find(Run(env, nil), "binary")
	if c.Status != StatusWarn {
		t.Fatalf("expected stale binary warning, got %s (%s)", c.Status, c.Detail)
	}
	if c.Command == "" {
		t.Fatalf("stale binary finding must include a remediation command")
	}
}

func TestSameFileBinaryIsOK(t *testing.T) {
	env := healthyEnv()
	// Even with mismatched version strings, invoking the PATH binary is OK.
	env.RunningVersion = "deadbeef"
	env.SameFile = func(a, b string) bool { return true }
	c := find(Run(env, nil), "binary")
	if c.Status != StatusOK {
		t.Fatalf("expected OK when invoking the PATH binary itself, got %s (%s)", c.Status, c.Detail)
	}
}

func TestMissingBinaryWarns(t *testing.T) {
	env := healthyEnv()
	env.LookPath = func(name string) (string, error) { return "", fmt.Errorf("not found") }
	c := find(Run(env, []string{"binary"}), "binary")
	if c.Status != StatusWarn || c.Command == "" {
		t.Fatalf("expected missing-binary warning with command, got %+v", c)
	}
}

func TestMissingRenderEngineWarns(t *testing.T) {
	env := healthyEnv()
	env.LookPath = func(name string) (string, error) {
		if name == "libreoffice" || name == "soffice" {
			return "", fmt.Errorf("not found")
		}
		return "/usr/bin/" + name, nil
	}
	c := find(Run(env, []string{"render-engine"}), "render-engine")
	if c.Status != StatusWarn {
		t.Fatalf("expected render-engine warning, got %s", c.Status)
	}
	if !strings.Contains(c.Command, "libreoffice") {
		t.Fatalf("expected install hint mentioning libreoffice: %q", c.Command)
	}
}

func TestOpenXMLSDKValidatorWarnsWithoutSDK(t *testing.T) {
	env := healthyEnv()
	env.CommandOutput = func(name string, args ...string) (string, error) {
		if strings.HasSuffix(name, "dotnet") {
			return "", nil
		}
		return healthyEnv().CommandOutput(name, args...)
	}
	c := find(Run(env, []string{"openxml-sdk-validator"}), "openxml-sdk-validator")
	if c.Status != StatusWarn || !strings.Contains(c.Command, "dotnet") {
		t.Fatalf("expected Open XML SDK warning with dotnet remediation, got %+v", c)
	}
}

func TestMicrosoftOfficeCOMCheckOnWindows(t *testing.T) {
	env := healthyEnv()
	env.GOOS = "windows"
	env.LookPath = func(name string) (string, error) {
		if name == "powershell.exe" {
			return `C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`, nil
		}
		return healthyEnv().LookPath(name)
	}
	env.CommandOutput = func(name string, args ...string) (string, error) {
		if strings.HasSuffix(strings.ToLower(name), "powershell.exe") {
			return "Excel.Application\nPowerPoint.Application\nWord.Application\n", nil
		}
		return healthyEnv().CommandOutput(name, args...)
	}
	c := find(Run(env, []string{"microsoft-office-com"}), "microsoft-office-com")
	if c.Status != StatusOK || !strings.Contains(c.Detail, "Excel.Application") {
		t.Fatalf("expected Office COM OK, got %+v", c)
	}
}

func TestOfficeEditSmokeCheckOnWindows(t *testing.T) {
	env := healthyEnv()
	env.GOOS = "windows"
	env.LookPath = func(name string) (string, error) {
		if name == "powershell.exe" {
			return `C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe`, nil
		}
		return healthyEnv().LookPath(name)
	}
	c := find(Run(env, []string{"office-edit-smoke"}), "office-edit-smoke")
	if c.Status != StatusOK ||
		!strings.Contains(c.Command, "windows-office-edit-smoke.ps1") ||
		!strings.Contains(c.Command, "-MutationParallelism 4") ||
		!strings.Contains(c.Command, "-RequireOpenXmlSdk") ||
		!strings.Contains(c.Command, "-RunConformance") ||
		!strings.Contains(c.Detail, "check-release-fast") ||
		!strings.Contains(c.Detail, "check-release-slow") {
		t.Fatalf("expected Office edit smoke OK with runnable command, got %+v", c)
	}
}

func TestNoFontsWarns(t *testing.T) {
	env := healthyEnv()
	env.CommandOutput = func(name string, args ...string) (string, error) {
		if strings.HasSuffix(name, "fc-list") {
			return "", nil // installed but zero fonts
		}
		return healthyEnv().CommandOutput(name, args...)
	}
	c := find(Run(env, []string{"fonts"}), "fonts")
	if c.Status != StatusWarn {
		t.Fatalf("expected fonts warning when zero fonts, got %s", c.Status)
	}
}

func TestTempDirUnwritableFails(t *testing.T) {
	env := healthyEnv()
	env.ProbeWritable = func(dir string) error {
		if dir == "/tmp" {
			return fmt.Errorf("read-only file system")
		}
		return nil
	}
	c := find(Run(env, []string{"tempdir"}), "tempdir")
	if c.Status != StatusFail || c.Severity != SeverityError {
		t.Fatalf("expected tempdir fail/error, got %s/%s", c.Status, c.Severity)
	}
}

func TestOnlyFilter(t *testing.T) {
	report := Run(healthyEnv(), []string{"binary", "tempdir"})
	if len(report.Checks) != 2 {
		t.Fatalf("expected 2 checks with --only, got %d", len(report.Checks))
	}
}

func TestEveryFindingNamesAFix(t *testing.T) {
	// Break everything and assert each finding carries a remediation command.
	env := healthyEnv()
	env.LookPath = func(name string) (string, error) { return "", fmt.Errorf("not found") }
	env.ProbeWritable = func(dir string) error { return fmt.Errorf("denied") }
	for _, c := range Run(env, nil).Checks {
		if c.IsFinding() && strings.TrimSpace(c.Command) == "" {
			t.Fatalf("finding %s has no remediation command", c.ID)
		}
	}
}

func TestCheckIDsStable(t *testing.T) {
	got := strings.Join(CheckIDs(), ",")
	want := "binary,render-engine,fonts,tempdir,workdir,go-toolchain,openxml-sdk-validator,microsoft-office-com,office-edit-smoke,office-vba-smoke"
	if got != want {
		t.Fatalf("check IDs drifted: %q", got)
	}
}
