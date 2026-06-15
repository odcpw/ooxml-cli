package officecheck

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"testing"
)

type fakeRunner struct {
	paths map[string]string
	run   func(name string, args []string) (*RunResult, error)
}

func (r fakeRunner) LookPath(name string) (string, error) {
	if p, ok := r.paths[name]; ok {
		return p, nil
	}
	return "", errors.New("missing")
}

func (r fakeRunner) Run(_ context.Context, name string, args []string) (*RunResult, error) {
	if r.run != nil {
		return r.run(name, args)
	}
	return &RunResult{}, nil
}

func TestCheckUsesLibreOfficeConversionAndReportsProof(t *testing.T) {
	input := filepath.Join(t.TempDir(), "macro.xlsm")
	if err := os.WriteFile(input, []byte("placeholder"), 0o644); err != nil {
		t.Fatalf("failed to write input: %v", err)
	}
	outDir := t.TempDir()
	tools := &Tools{
		Runner: fakeRunner{
			paths: map[string]string{"soffice": "/usr/bin/soffice"},
			run: func(name string, args []string) (*RunResult, error) {
				if name != "soffice" {
					t.Fatalf("engine = %q, want soffice", name)
				}
				gotOutDir := flagValueForTest(args, "--outdir")
				if gotOutDir != outDir {
					t.Fatalf("--outdir = %q, want %q; args=%v", gotOutDir, outDir, args)
				}
				if gotFormat := flagValueForTest(args, "--convert-to"); gotFormat != "csv" {
					t.Fatalf("--convert-to = %q, want csv; args=%v", gotFormat, args)
				}
				if err := os.WriteFile(filepath.Join(outDir, "macro.csv"), []byte("ok\n"), 0o644); err != nil {
					t.Fatalf("failed to write fake conversion output: %v", err)
				}
				return &RunResult{}, nil
			},
		},
	}

	result, err := tools.Check(input, Options{Family: "xlsx", OutDir: outDir})
	if err != nil {
		t.Fatalf("Check failed: %v", err)
	}
	if result.Status != "passed" || !result.Checked || !result.OfficeOpenVerified {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.Engine != "soffice" || result.Method != "libreoffice-headless-convert" || result.ConversionFormat != "csv" {
		t.Fatalf("unexpected engine metadata: %+v", result)
	}
	if result.OutputPath == "" || result.OutputBytes <= 0 {
		t.Fatalf("missing output proof: %+v", result)
	}
	if result.MicrosoftOfficeVerified || result.MacroExecutionVerified {
		t.Fatalf("open check must not claim Microsoft Office or macro execution proof: %+v", result)
	}
}

func TestCheckReportsMissingEngineAsSkipped(t *testing.T) {
	tools := &Tools{Runner: fakeRunner{}}
	result, err := tools.Check("macro.xlsm", Options{Family: "xlsx"})
	if err == nil {
		t.Fatal("expected missing dependency error")
	}
	if result == nil || result.Status != "skipped" || result.Checked || result.ErrorCode != "missing_engine" {
		t.Fatalf("unexpected missing-engine result: %+v err=%v", result, err)
	}
}

func TestCheckReportsEngineFailure(t *testing.T) {
	input := filepath.Join(t.TempDir(), "deck.pptx")
	if err := os.WriteFile(input, []byte("placeholder"), 0o644); err != nil {
		t.Fatalf("failed to write input: %v", err)
	}
	tools := &Tools{
		Runner: fakeRunner{
			paths: map[string]string{"soffice": "/usr/bin/soffice"},
			run: func(name string, args []string) (*RunResult, error) {
				return &RunResult{Stderr: "conversion failed"}, errors.New("exit status 1")
			},
		},
	}
	result, err := tools.Check(input, Options{Family: "pptx", OutDir: t.TempDir()})
	if err == nil {
		t.Fatal("expected engine failure")
	}
	if result == nil || result.Status != "failed" || result.ErrorCode != "engine_failed" || !result.Checked {
		t.Fatalf("unexpected engine-failure result: %+v err=%v", result, err)
	}
	if result.Engine != "soffice" || result.ConversionFormat != "pdf" {
		t.Fatalf("unexpected engine metadata: %+v", result)
	}
}

func TestCheckReportsMissingConversionOutput(t *testing.T) {
	input := filepath.Join(t.TempDir(), "workbook.xlsx")
	if err := os.WriteFile(input, []byte("placeholder"), 0o644); err != nil {
		t.Fatalf("failed to write input: %v", err)
	}
	tools := &Tools{
		Runner: fakeRunner{
			paths: map[string]string{"soffice": "/usr/bin/soffice"},
			run: func(name string, args []string) (*RunResult, error) {
				return &RunResult{}, nil
			},
		},
	}
	result, err := tools.Check(input, Options{Family: "xlsx", OutDir: t.TempDir()})
	if err == nil {
		t.Fatal("expected missing conversion output error")
	}
	if result == nil || result.Status != "failed" || result.ErrorCode != "conversion_output_missing" || !result.Checked {
		t.Fatalf("unexpected missing-output result: %+v err=%v", result, err)
	}
	if result.Engine != "soffice" || result.ConversionFormat != "csv" {
		t.Fatalf("unexpected engine metadata: %+v", result)
	}
}

func flagValueForTest(args []string, flag string) string {
	for i, arg := range args {
		if arg == flag && i+1 < len(args) {
			return args[i+1]
		}
	}
	return ""
}
