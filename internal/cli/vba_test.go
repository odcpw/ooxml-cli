package cli

import (
	"bytes"
	"context"
	"encoding/binary"
	"encoding/json"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"
	"unicode/utf16"

	"github.com/ooxml-cli/ooxml-cli/pkg/officecheck"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	vbapkg "github.com/ooxml-cli/ooxml-cli/pkg/vba"
)

func TestVBACLIAddExtractRemovePPTX(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	projectData := []byte("opaque cli macro project")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}

	attachedPath := filepath.Join(t.TempDir(), "attached.pptm")
	attachOutput := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)
	attachResult := parseVBAMutationResult(t, attachOutput)
	assertVBAMutationReadback(t, attachResult, inputPath, attachedPath, "attach", true)
	assertExecutableVBAMutationCommands(t, attachResult)
	assertVBAState(t, attachedPath, true)
	runVBACommand(t, "validate", "--strict", attachedPath)
	runVBACommand(t, "pptx", "slides", "list", attachedPath)

	extractedPath := filepath.Join(t.TempDir(), "extracted.bin")
	extractOutput := runVBACommand(t, "--format", "json", "vba", "extract-bin", attachedPath, "--out", extractedPath)
	extractResult := parseVBAExtractResult(t, extractOutput)
	assertVBAExtractCommands(t, extractResult, attachedPath, extractedPath)
	extracted, err := os.ReadFile(extractedPath)
	if err != nil {
		t.Fatalf("failed to read extracted bin: %v", err)
	}
	if !bytes.Equal(extracted, projectData) {
		t.Fatalf("extracted = %q, want %q", extracted, projectData)
	}

	removedPath := filepath.Join(t.TempDir(), "removed.pptx")
	removeOutput := runVBACommand(t, "--format", "json", "vba", "remove", attachedPath, "--out", removedPath)
	removeResult := parseVBAMutationResult(t, removeOutput)
	assertVBAMutationReadback(t, removeResult, attachedPath, removedPath, "remove", false)
	assertExecutableVBAMutationCommands(t, removeResult)
	assertVBAState(t, removedPath, false)
	runVBACommand(t, "validate", "--strict", removedPath)
	runVBACommand(t, "pptx", "slides", "list", removedPath)
}

func TestVBACLIAddRemoveXLSXJSONReadbackAndCommandPaths(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	projectData := []byte("opaque xlsx cli macro project")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}

	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	attachOutput := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)
	attachResult := parseVBAMutationResult(t, attachOutput)
	assertVBAMutationReadback(t, attachResult, inputPath, attachedPath, "attach", true)
	assertExecutableVBAMutationCommands(t, attachResult)
	assertVBAState(t, attachedPath, true)
	runVBACommand(t, "validate", "--strict", attachedPath)
	runVBACommand(t, "xlsx", "sheets", "list", attachedPath)

	removedPath := filepath.Join(t.TempDir(), "removed.xlsx")
	removeOutput := runVBACommand(t, "--format", "json", "vba", "remove", attachedPath, "--out", removedPath)
	removeResult := parseVBAMutationResult(t, removeOutput)
	assertVBAMutationReadback(t, removeResult, attachedPath, removedPath, "remove", false)
	assertExecutableVBAMutationCommands(t, removeResult)
	assertVBAState(t, removedPath, false)
	runVBACommand(t, "validate", "--strict", removedPath)
	runVBACommand(t, "xlsx", "sheets", "list", removedPath)
}

func TestVBAAttachDryRunJSONIncludesReadback(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	projectData := []byte("opaque dry-run macro project")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}

	output := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--dry-run")
	result := parseVBAMutationResult(t, output)
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}
	assertVBAMutationReadback(t, result, inputPath, "", "attach", true)
	if result.InspectCommand != "" || result.ValidateCommand != "" || result.PackageReadbackCommand != "" {
		t.Fatalf("dry-run should not emit executable output commands: %+v", result)
	}
	for label, command := range map[string]string{
		"inspectTemplate":  result.InspectCommandTemplate,
		"validateTemplate": result.ValidateCommandTemplate,
		"packageTemplate":  result.PackageReadbackCommandTemplate,
	} {
		if !strings.Contains(command, "<out.xlsm>") {
			t.Fatalf("%s missing output placeholder: %q", label, command)
		}
	}
	assertVBAState(t, inputPath, false)
}

func TestVBACreateUsesOfficeScriptRunner(t *testing.T) {
	dir := t.TempDir()
	modulePath := filepath.Join(dir, "Module1.bas")
	classPath := filepath.Join(dir, "Worker.cls")
	scriptPath := filepath.Join(dir, "windows-office-vba-create.ps1")
	outPath := filepath.Join(dir, "created.xlsm")
	binPath := filepath.Join(dir, "vbaProject.bin")
	writeVBASourceForTest(t, modulePath, "Attribute VB_Name = \"Module1\"\r\nPublic Sub Main()\r\nEnd Sub\r\n")
	writeVBASourceForTest(t, classPath, "Attribute VB_Name = \"Worker\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n")
	if err := os.WriteFile(scriptPath, []byte("# fake helper for unit test\r\n"), 0o644); err != nil {
		t.Fatalf("write fake helper: %v", err)
	}

	previous := vbaCreateScriptRunner
	t.Cleanup(func() { vbaCreateScriptRunner = previous })
	var got VBACreateOptions
	vbaCreateScriptRunner = func(opts VBACreateOptions) (*VBACreateResult, error) {
		got = opts
		return &VBACreateResult{
			Family:              opts.Family,
			Output:              opts.OutputPath,
			OutputSHA256:        strings.Repeat("a", 64),
			VBAProjectBin:       opts.ExtractBinPath,
			VBAProjectBinSHA256: strings.Repeat("b", 64),
			Sources:             append([]string{}, opts.SourcePaths...),
			ImportedModules: []VBACreateImportedModule{
				{Source: opts.SourcePaths[0], Name: "Module1", Type: "1"},
				{Source: opts.SourcePaths[1], Name: "Worker", Type: "2"},
			},
			ProofLevel: "microsoft-office-authored",
			NextCommands: VBACreateNextCommands{
				Inspect: "powershell-helper-should-not-own-followup-commands",
			},
		}, nil
	}

	output := runVBACommand(t,
		"--format", "json",
		"vba", "create", outPath,
		"--source", modulePath,
		"--source", classPath,
		"--extract-bin", binPath,
		"--office-create-script", scriptPath,
		"--enable-vba-object-model-access",
		"--force",
	)
	result := parseVBACreateResult(t, output)
	if got.Family != "xlsx" || !got.InferredFamilyFromExtension || got.OutputPath != outPath {
		t.Fatalf("unexpected create options: %+v", got)
	}
	if !got.EnableVBOMAccess || !got.Force || got.Backend != "windows-office-com" {
		t.Fatalf("missing create option booleans/backend: %+v", got)
	}
	if len(got.SourcePaths) != 2 || got.SourcePaths[0] != modulePath || got.SourcePaths[1] != classPath {
		t.Fatalf("unexpected source paths: %+v", got.SourcePaths)
	}
	if result.Backend != "windows-office-com" || result.ProofLevel != "microsoft-office-authored" {
		t.Fatalf("unexpected result proof/backend: %+v", result)
	}
	for label, command := range map[string]string{
		"inspect":     result.NextCommands.Inspect,
		"list":        result.NextCommands.List,
		"validate":    result.NextCommands.Validate,
		"officeCheck": result.NextCommands.OfficeCheck,
		"readback":    result.NextCommands.Readback,
		"attachSeed":  result.NextCommands.AttachSeed,
	} {
		if command == "" {
			t.Fatalf("missing %s command in result: %+v", label, result)
		}
	}
	if !strings.Contains(result.NextCommands.Readback, "xlsx sheets list") {
		t.Fatalf("expected XLSX readback command, got %q", result.NextCommands.Readback)
	}
	if strings.Contains(result.NextCommands.Inspect, "powershell-helper") || !strings.HasPrefix(result.NextCommands.Inspect, "ooxml --json vba inspect") {
		t.Fatalf("expected Go-owned inspect command, got %q", result.NextCommands.Inspect)
	}
	legacyOpenProof := "authored and " + "opened"
	if len(result.Limitations) == 0 || !strings.Contains(result.Limitations[0], "authored and saved") || strings.Contains(result.Limitations[0], legacyOpenProof) {
		t.Fatalf("unexpected proof limitation wording: %+v", result.Limitations)
	}
	if len(result.ImportedModules) != 2 {
		t.Fatalf("unexpected imported modules: %+v", result.ImportedModules)
	}
}

func TestVBACreateScriptResultAcceptsPowerShellSingletonShapes(t *testing.T) {
	result, err := parseVBACreateScriptResult([]byte(`{
		"family": "pptx",
		"output": "created.pptm",
		"outputSha256": "abc",
		"vbaProjectBin": "",
		"vbaProjectBinSha256": "",
		"sources": "Module1.bas",
		"importedModules": {
			"source": "Module1.bas",
			"name": "Module1",
			"type": "1"
		},
		"proofLevel": "microsoft-office-authored",
		"nextCommands": {
			"inspect": "ooxml --json vba inspect created.pptm"
		}
	}`))
	if err != nil {
		t.Fatalf("parse singleton create result: %v", err)
	}
	if len(result.Sources) != 1 || result.Sources[0] != "Module1.bas" {
		t.Fatalf("unexpected sources: %+v", result.Sources)
	}
	if len(result.ImportedModules) != 1 || result.ImportedModules[0].Name != "Module1" {
		t.Fatalf("unexpected imported modules: %+v", result.ImportedModules)
	}
}

func TestVBACreateSourceNormalizationPreservesCommaPaths(t *testing.T) {
	dir := t.TempDir()
	commaPath := filepath.Join(dir, "Module,One.bas")
	plainPath := filepath.Join(dir, "ModuleOne.bas")
	secondPath := filepath.Join(dir, "Module2.bas")
	writeVBASourceForTest(t, commaPath, "Attribute VB_Name = \"ModuleOne\"\r\n")
	writeVBASourceForTest(t, plainPath, "Attribute VB_Name = \"ModuleOne\"\r\n")
	writeVBASourceForTest(t, secondPath, "Attribute VB_Name = \"Module2\"\r\n")

	oneSource, err := normalizeVBACreateSources([]string{commaPath})
	if err != nil {
		t.Fatalf("normalize comma path: %v", err)
	}
	if len(oneSource) != 1 || oneSource[0] != commaPath {
		t.Fatalf("comma-containing existing path should stay intact: %+v", oneSource)
	}

	twoSources, err := normalizeVBACreateSources([]string{plainPath + "," + secondPath})
	if err != nil {
		t.Fatalf("normalize comma list: %v", err)
	}
	if len(twoSources) != 2 || twoSources[0] != plainPath || twoSources[1] != secondPath {
		t.Fatalf("comma-separated source list should still split: %+v", twoSources)
	}
}

func TestVBACreateOfficeScriptArgsPassSourcesAsJSON(t *testing.T) {
	opts := VBACreateOptions{
		Family:                 "xlsx",
		OutputPath:             `C:\tmp\out.xlsm`,
		OfficeCreateScriptPath: `C:\repo\tools\windows-office-vba-create.ps1`,
		SourcePaths: []string{
			`C:\tmp\Module,One.bas`,
			`C:\tmp\Worker.cls`,
		},
		ExtractBinPath:   `C:\tmp\vbaProject.bin`,
		EnableVBOMAccess: true,
		Force:            true,
	}

	args := buildVBACreateOfficeScriptArgs(opts)
	sourceJSONFlag := -1
	for i, arg := range args {
		if arg == "-SourcePathJson" {
			sourceJSONFlag = i
			break
		}
	}
	if sourceJSONFlag < 0 || sourceJSONFlag+1 >= len(args) {
		t.Fatalf("missing source JSON argument: %+v", args)
	}
	var sources []string
	if err := json.Unmarshal([]byte(args[sourceJSONFlag+1]), &sources); err != nil {
		t.Fatalf("source JSON should be parseable: %v; args=%+v", err, args)
	}
	if len(sources) != 2 || sources[0] != opts.SourcePaths[0] || sources[1] != opts.SourcePaths[1] {
		t.Fatalf("unexpected source JSON: %+v", sources)
	}
	for _, arg := range args {
		if arg == opts.SourcePaths[0] || arg == opts.SourcePaths[1] {
			t.Fatalf("raw source path should not be passed as a positional PowerShell argument: %+v", args)
		}
	}
}

func TestVBACreateExplicitOfficeScriptPathDoesNotFallback(t *testing.T) {
	dir := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dir, "tools"), 0o755); err != nil {
		t.Fatalf("mkdir tools: %v", err)
	}
	defaultScript := filepath.Join(dir, "tools", "windows-office-vba-create.ps1")
	if err := os.WriteFile(defaultScript, []byte("# should not be used\r\n"), 0o644); err != nil {
		t.Fatalf("write default helper: %v", err)
	}
	modulePath := filepath.Join(dir, "Module1.bas")
	writeVBASourceForTest(t, modulePath, "Attribute VB_Name = \"Module1\"\r\n")
	cwd, err := os.Getwd()
	if err != nil {
		t.Fatalf("get cwd: %v", err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatalf("chdir: %v", err)
	}
	t.Cleanup(func() {
		if err := os.Chdir(cwd); err != nil {
			t.Fatalf("restore cwd: %v", err)
		}
	})

	previous := vbaCreateScriptRunner
	t.Cleanup(func() { vbaCreateScriptRunner = previous })
	vbaCreateScriptRunner = func(opts VBACreateOptions) (*VBACreateResult, error) {
		t.Fatalf("Office runner should not be called when explicit helper path is missing: %+v", opts)
		return nil, nil
	}

	err = executeVBACommandExpectError(t,
		"vba", "create", filepath.Join(dir, "created.xlsm"),
		"--source", modulePath,
		"--office-create-script", filepath.Join(dir, "missing.ps1"),
	)
	if cliErr, ok := AsCLIError(err); !ok || cliErr.ExitCode != ExitFileNotFound {
		t.Fatalf("expected file-not-found CLI error for explicit helper path, got %#v", err)
	}
}

func TestVBACreateValidatesArgumentsBeforeOfficeRunner(t *testing.T) {
	dir := t.TempDir()
	scriptPath := filepath.Join(dir, "windows-office-vba-create.ps1")
	modulePath := filepath.Join(dir, "Module1.bas")
	writeVBASourceForTest(t, modulePath, "Attribute VB_Name = \"Module1\"\r\n")
	if err := os.WriteFile(scriptPath, []byte("# fake helper\r\n"), 0o644); err != nil {
		t.Fatalf("write fake helper: %v", err)
	}
	previous := vbaCreateScriptRunner
	t.Cleanup(func() { vbaCreateScriptRunner = previous })
	vbaCreateScriptRunner = func(opts VBACreateOptions) (*VBACreateResult, error) {
		t.Fatalf("Office runner should not be called for invalid args: %+v", opts)
		return nil, nil
	}

	cases := [][]string{
		{"vba", "create", filepath.Join(dir, "bad.xlsx"), "--source", modulePath, "--office-create-script", scriptPath},
		{"vba", "create", filepath.Join(dir, "bad.pptm"), "--family", "xlsx", "--source", modulePath, "--office-create-script", scriptPath},
		{"vba", "create", filepath.Join(dir, "created.xlsm"), "--office-create-script", scriptPath},
		{"vba", "create", filepath.Join(dir, "created.xlsm"), "--source", filepath.Join(dir, "missing.bas"), "--office-create-script", scriptPath},
	}
	for _, args := range cases {
		err := executeVBACommandExpectError(t, args...)
		if _, ok := AsCLIError(err); !ok {
			t.Fatalf("%v: expected CLIError, got %#v", args, err)
		}
	}
}

func TestVBAInPlaceJSONReadbackUsesWrittenFile(t *testing.T) {
	sourcePath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	inputPath := filepath.Join(t.TempDir(), "workbook.xlsx")
	data, err := os.ReadFile(sourcePath)
	if err != nil {
		t.Fatalf("failed to read source workbook: %v", err)
	}
	if err := os.WriteFile(inputPath, data, 0o644); err != nil {
		t.Fatalf("failed to write temp workbook: %v", err)
	}
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, []byte("opaque in-place macro project"), 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}

	attachOutput := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--in-place")
	attachResult := parseVBAMutationResult(t, attachOutput)
	assertVBAMutationReadback(t, attachResult, inputPath, inputPath, "attach", true)
	assertExecutableVBAMutationCommands(t, attachResult)
	assertVBAState(t, inputPath, true)

	removeOutput := runVBACommand(t, "--format", "json", "vba", "remove", inputPath, "--in-place")
	removeResult := parseVBAMutationResult(t, removeOutput)
	assertVBAMutationReadback(t, removeResult, inputPath, inputPath, "remove", false)
	assertExecutableVBAMutationCommands(t, removeResult)
	assertVBAState(t, inputPath, false)
}

func TestVBAInspectJSONIncludesNextCommands(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	projectData := []byte("opaque inspect command project")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	output := runVBACommand(t, "--format", "json", "vba", "inspect", attachedPath)
	result := parseVBAInspectResult(t, output)
	if result.File != attachedPath || result.VBA == nil || !result.VBA.HasVBAProject {
		t.Fatalf("unexpected inspect result: %+v", result)
	}
	for label, command := range map[string]string{
		"validate": result.ValidateCommand,
		"package":  result.PackageReadbackCommand,
	} {
		if command == "" {
			t.Fatalf("missing %s command: %+v", label, result)
		}
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}
	if result.ExtractBinCommand == "" || !strings.Contains(result.ExtractBinCommand, "vba extract-bin") {
		t.Fatalf("missing extract-bin command: %+v", result)
	}
	if !strings.Contains(result.NextMutationTemplate, "vba remove") || !strings.Contains(result.NextMutationTemplate, "<out.xlsx>") {
		t.Fatalf("unexpected next mutation template: %q", result.NextMutationTemplate)
	}
}

func TestVBAOfficeCheckJSONUsesLocalEngineProof(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	projectData := []byte("opaque office-check project")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	attachOutput := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)
	attachResult := parseVBAMutationResult(t, attachOutput)
	if attachResult.OfficeCheckCommand == "" || !strings.Contains(attachResult.OfficeCheckCommand, "vba office-check") {
		t.Fatalf("mutation output missing office-check command: %+v", attachResult)
	}

	outDir := t.TempDir()
	restore := stubVBAOfficeCheckTools(t, map[string]string{"soffice": "/usr/bin/soffice"}, func(name string, args []string) (*officecheck.RunResult, error) {
		if name != "soffice" {
			t.Fatalf("engine = %q, want soffice", name)
		}
		if got := flagValueForVBATest(args, "--convert-to"); got != "csv" {
			t.Fatalf("--convert-to = %q, want csv; args=%v", got, args)
		}
		if got := flagValueForVBATest(args, "--outdir"); got != outDir {
			t.Fatalf("--outdir = %q, want %q; args=%v", got, outDir, args)
		}
		if err := os.WriteFile(filepath.Join(outDir, "attached.csv"), []byte("ok\n"), 0o644); err != nil {
			t.Fatalf("failed to write conversion output: %v", err)
		}
		return &officecheck.RunResult{}, nil
	})
	defer restore()

	output := runVBACommand(t, "--format", "json", "vba", "office-check", attachedPath, "--out-dir", outDir)
	result := parseVBAOfficeCheckResult(t, output)
	if result.File != attachedPath || result.Family != "xlsx" || !result.PackageValid {
		t.Fatalf("unexpected office-check file metadata: %+v", result)
	}
	if !result.OverallVerified || result.OverallStatus != "passed" || result.OpenCheck == nil || result.OpenCheck.Status != "passed" || !result.OpenCheck.OfficeOpenVerified {
		t.Fatalf("unexpected office-check result: %+v", result)
	}
	if result.MicrosoftOffice || result.MacroExecution || result.MacroCompilation {
		t.Fatalf("office-check must not overclaim runtime proof: %+v", result)
	}
	if result.OpenCheck.OutputPath == "" || result.OpenCheck.OutputBytes <= 0 {
		t.Fatalf("missing conversion output proof: %+v", result.OpenCheck)
	}
}

func TestVBAOfficeCheckMissingEngineReturnsJSONFailure(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	projectData := []byte("opaque office-check project")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	restore := stubVBAOfficeCheckTools(t, nil, nil)
	defer restore()

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"--format", "json", "vba", "office-check", attachedPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitRenderFailed || !cliErr.Reported {
		t.Fatalf("expected reported render_failed error, got %#v", err)
	}
	result := parseVBAOfficeCheckResult(t, stdout.String())
	if result.OverallVerified || result.OpenCheck == nil || result.OpenCheck.Status != "skipped" || result.OpenCheck.ErrorCode != "missing_engine" {
		t.Fatalf("unexpected missing-engine JSON: %+v", result)
	}
}

func TestVBAOfficeCheckRejectsMacroFreePackage(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	restore := stubVBAOfficeCheckTools(t, map[string]string{"soffice": "/usr/bin/soffice"}, func(name string, args []string) (*officecheck.RunResult, error) {
		t.Fatalf("office-check engine should not run for macro-free packages: %s %v", name, args)
		return nil, nil
	})
	defer restore()

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"--format", "json", "vba", "office-check", inputPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitUnsupportedType || !cliErr.Reported {
		t.Fatalf("expected reported unsupported_type error, got %#v", err)
	}
	result := parseVBAOfficeCheckResult(t, stdout.String())
	if result.PackageValid || result.OverallVerified || result.OpenCheck == nil || result.OpenCheck.ErrorCode != "missing_vba_project" {
		t.Fatalf("unexpected macro-free office-check result: %+v", result)
	}
}

func TestVBAOfficeCheckTreatsValidationWarningsAsBlocking(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	projectData := syntheticCLIVBAProjectBinForModulesTest(t, []syntheticCLIVBAModule{
		{
			Name:       "ThisWorkbook",
			StreamName: "ThisWorkbook",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"ThisWorkbook\"\r\nPrivate Sub Workbook_Open()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub Hello()\r\nEnd Sub\r\n",
		},
	})
	binPath := filepath.Join(t.TempDir(), "excel-shaped-vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached-risky.pptm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--allow-host-family-risk", "--out", attachedPath)

	restore := stubVBAOfficeCheckTools(t, map[string]string{"soffice": "/usr/bin/soffice"}, func(name string, args []string) (*officecheck.RunResult, error) {
		t.Fatalf("office-check engine should not run when strict validation has warnings: %s %v", name, args)
		return nil, nil
	})
	defer restore()

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"--format", "json", "vba", "office-check", attachedPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitValidationFailed || !cliErr.Reported {
		t.Fatalf("expected reported validation_failed error, got %#v", err)
	}
	result := parseVBAOfficeCheckResult(t, stdout.String())
	if result.PackageValid || result.OverallVerified || result.OpenCheck == nil || result.OpenCheck.ErrorCode != "package_validation_failed" {
		t.Fatalf("warning-only package should not pass office-check validation: %+v", result)
	}
	if !containsDiagnosticCodeForVBATest(result.Validation.Diagnostics, "VBA_HOST_EXCEL_MODULES_IN_PPTM") {
		t.Fatalf("expected host-family warning diagnostic in strict validation: %+v", result.Validation)
	}
}

func TestVBACLIListExtractSyntheticModules(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, syntheticCLIVBAProjectBinForTest(t), 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", attachedPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	if listResult.File != attachedPath || listResult.VBA == nil || !listResult.VBA.HasVBAProject || listResult.Project == nil {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	if listResult.Project.ModuleCount != 2 || len(listResult.Project.Modules) != 2 {
		t.Fatalf("unexpected module list: %+v", listResult.Project)
	}
	if listResult.Project.Modules[0].Name != "Module1" || listResult.Project.Modules[0].Source != "" {
		t.Fatalf("list should summarize Module1 without embedding source: %+v", listResult.Project.Modules[0])
	}
	if listResult.Project.Modules[0].LineEnding != "crlf" || !listResult.Project.Modules[0].TrailingNewline || listResult.Project.Modules[0].SHA256Basis != "decoded-source-utf8" {
		t.Fatalf("list should report source normalization metadata: %+v", listResult.Project.Modules[0])
	}
	if !strings.Contains(listResult.ExtractCommandTemplate, "vba extract") || !strings.Contains(listResult.ExtractCommandTemplate, "--out-dir macros") {
		t.Fatalf("unexpected extract template: %q", listResult.ExtractCommandTemplate)
	}
	for _, command := range []string{listResult.InspectCommand, listResult.ValidateCommand, listResult.PackageReadbackCommand} {
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}

	outDir := filepath.Join(t.TempDir(), "macros")
	extractOutput := runVBACommand(t, "--format", "json", "vba", "extract", attachedPath, "--out-dir", outDir)
	extractResult := parseVBAModuleExtractResult(t, extractOutput)
	if extractResult.OutputDir != outDir || len(extractResult.Modules) != 2 || extractResult.ListCommand == "" {
		t.Fatalf("unexpected extract result: %+v", extractResult)
	}
	if extractResult.Modules[0].LineEnding != "crlf" || !extractResult.Modules[0].TrailingNewline || extractResult.Modules[0].SHA256Basis != "decoded-source-utf8" {
		t.Fatalf("extract should report source normalization metadata: %+v", extractResult.Modules[0])
	}
	moduleSource := readTextFileForVBATest(t, filepath.Join(outDir, "Module1.bas"))
	if !strings.Contains(moduleSource, "Public Sub HelloWorld()") {
		t.Fatalf("unexpected Module1.bas:\n%s", moduleSource)
	}
	classSource := readTextFileForVBATest(t, filepath.Join(outDir, "Class1.cls"))
	if !strings.Contains(classSource, "Public Function Answer()") {
		t.Fatalf("unexpected Class1.cls:\n%s", classSource)
	}
	executeGeneratedOOXMLCommandForVBATest(t, extractResult.ListCommand)

	oneDir := filepath.Join(t.TempDir(), "one")
	oneOutput := runVBACommand(t, "--format", "json", "vba", "extract", attachedPath, "--out-dir", oneDir, "--module", "module:Class1")
	oneResult := parseVBAModuleExtractResult(t, oneOutput)
	if len(oneResult.Modules) != 1 || oneResult.Modules[0].Name != "Class1" {
		t.Fatalf("unexpected single-module extract result: %+v", oneResult.Modules)
	}
	if _, err := os.Stat(filepath.Join(oneDir, "Module1.bas")); !os.IsNotExist(err) {
		t.Fatalf("single-module extract should not write Module1.bas, stat err=%v", err)
	}
}

func TestVBAListAndValidateWarnForExcelDocumentModulesInPPTM(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	projectData := syntheticCLIVBAProjectBinForModulesTest(t, []syntheticCLIVBAModule{
		{
			Name:       "ThisWorkbook",
			StreamName: "ThisWorkbook",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"ThisWorkbook\"\r\nPrivate Sub Workbook_Open()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Sheet1",
			StreamName: "Sheet1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Sheet1\"\r\nPrivate Sub Worksheet_Activate()\r\nEnd Sub\r\n",
		},
		{
			Name:       "DeckMacro",
			StreamName: "DeckMacro",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"DeckMacro\"\r\nPublic Sub RefreshDeck()\r\nEnd Sub\r\n",
		},
	})
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.pptm")
	attachOutput := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--allow-host-family-risk", "--out", attachedPath)
	attachResult := parseVBAMutationResult(t, attachOutput)
	if attachResult.ListCommand == "" || !strings.Contains(attachResult.ListCommand, "vba list") {
		t.Fatalf("parseable macro attach should include listCommand: %+v", attachResult)
	}
	executeGeneratedOOXMLCommandForVBATest(t, attachResult.ListCommand)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", attachedPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	if listResult.Project == nil ||
		listResult.Project.OfficeCompatibility == nil ||
		listResult.Project.OfficeCompatibility.OfficeLoadVerified ||
		listResult.Project.OfficeCompatibility.Status != "risk" ||
		!containsWarningForVBATest(listResult.Project.Warnings, "PowerPoint macro package contains Excel document module") ||
		!containsHostCompatibilityCodeForVBATest(listResult.Project.HostCompatibilityWarnings, "VBA_HOST_EXCEL_MODULES_IN_PPTM") {
		t.Fatalf("expected host warning in vba list result: %+v", listResult.Project)
	}

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"--format", "json", "validate", "--strict", attachedPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	assertCLIExitCodeForXLSXTest(t, []string{"validate", "--strict", attachedPath}, err, ExitValidationFailed)
	var validateResult ValidateResult
	if jsonErr := json.Unmarshal(stdout.Bytes(), &validateResult); jsonErr != nil {
		t.Fatalf("failed to unmarshal validate result: %v\n%s", jsonErr, stdout.String())
	}
	if !containsDiagnosticCodeForVBATest(validateResult.Diagnostics, "VBA_HOST_EXCEL_MODULES_IN_PPTM") {
		t.Fatalf("expected host diagnostic in validate result: %+v", validateResult.Diagnostics)
	}
}

func TestVBAInspectBinWarnsBeforeAttachingWrongHostSeed(t *testing.T) {
	projectData := syntheticCLIVBAProjectBinForModulesTest(t, []syntheticCLIVBAModule{
		{
			Name:       "ThisWorkbook",
			StreamName: "ThisWorkbook",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"ThisWorkbook\"\r\nPrivate Sub Workbook_Open()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Sheet1",
			StreamName: "Sheet1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Sheet1\"\r\nPrivate Sub Worksheet_Activate()\r\nEnd Sub\r\n",
		},
		{
			Name:       "DeckMacro",
			StreamName: "DeckMacro",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"DeckMacro\"\r\nPublic Sub RefreshDeck()\r\nEnd Sub\r\n",
		},
	})
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}

	output := runVBACommand(t, "--format", "json", "vba", "inspect-bin", binPath, "--family", "pptx")
	result := parseVBAInspectBinResult(t, output)
	if result.File != binPath || result.SizeBytes != len(projectData) || result.SHA256 == "" || result.Project == nil {
		t.Fatalf("unexpected inspect-bin result: %+v", result)
	}
	if result.Project.Family != "pptx" ||
		result.Project.OfficeCompatibility == nil ||
		result.Project.OfficeCompatibility.Status != "risk" ||
		!containsHostCompatibilityCodeForVBATest(result.Project.HostCompatibilityWarnings, "VBA_HOST_EXCEL_MODULES_IN_PPTM") {
		t.Fatalf("expected PowerPoint host-family risk, got %+v", result.Project)
	}
	if !strings.Contains(result.AttachCommandTemplate, "vba attach deck.pptx") || !strings.Contains(result.AttachCommandTemplate, "--out deck.pptm") {
		t.Fatalf("unexpected attach template: %q", result.AttachCommandTemplate)
	}
	for _, module := range result.Project.Modules {
		if module.Source != "" {
			t.Fatalf("inspect-bin JSON should summarize modules without embedding source: %+v", module)
		}
	}

	xlsxOutput := runVBACommand(t, "--format", "json", "vba", "inspect-bin", binPath, "--family", "xlsx")
	xlsxResult := parseVBAInspectBinResult(t, xlsxOutput)
	if xlsxResult.Project == nil || xlsxResult.Project.OfficeCompatibility == nil || xlsxResult.Project.OfficeCompatibility.Status != "unverified" {
		t.Fatalf("Excel seed inspection should not warn for Excel document modules: %+v", xlsxResult.Project)
	}

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"--format", "json", "vba", "inspect-bin", binPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	assertCLIExitCodeForXLSXTest(t, []string{"vba", "inspect-bin", binPath}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "--family is required") {
		t.Fatalf("missing helpful family error: %v", err)
	}
}

func TestVBAAttachRefusesWrongHostSeedByDefault(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	projectData := syntheticCLIVBAProjectBinForModulesTest(t, []syntheticCLIVBAModule{
		{
			Name:       "ThisWorkbook",
			StreamName: "ThisWorkbook",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"ThisWorkbook\"\r\nPrivate Sub Workbook_Open()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Sheet1",
			StreamName: "Sheet1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Sheet1\"\r\nPrivate Sub Worksheet_Activate()\r\nEnd Sub\r\n",
		},
	})
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, projectData, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}

	blockedPath := filepath.Join(t.TempDir(), "blocked.pptm")
	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", blockedPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	assertCLIExitCodeForXLSXTest(t, []string{"vba", "attach", inputPath}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "VBA host-family risk refused") {
		t.Fatalf("missing host-family refusal detail: %v", err)
	}
	if _, statErr := os.Stat(blockedPath); !os.IsNotExist(statErr) {
		t.Fatalf("refused attach should not write output, stat error = %v", statErr)
	}

	allowedPath := filepath.Join(t.TempDir(), "allowed.pptm")
	output := runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--allow-host-family-risk", "--out", allowedPath)
	result := parseVBAMutationResult(t, output)
	if result.Result == nil ||
		result.Result.SourceProject == nil ||
		result.Result.SourceProject.OfficeCompatibility == nil ||
		result.Result.SourceProject.OfficeCompatibility.Status != "risk" ||
		!containsHostCompatibilityCodeForVBATest(result.Result.SourceProject.HostCompatibilityWarnings, "VBA_HOST_EXCEL_MODULES_IN_PPTM") {
		t.Fatalf("allowed risky attach should report summarized source project: %+v", result.Result)
	}
	for _, module := range result.Result.SourceProject.Modules {
		if module.Source != "" {
			t.Fatalf("attach result should summarize source project without embedding source: %+v", module)
		}
	}
	assertVBAState(t, allowedPath, true)
}

func TestVBACLIReplaceModuleSyntheticProject(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, syntheticCLIVBAProjectBinForTest(t), 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", attachedPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	currentHash := listResult.Project.Modules[0].SHA256
	if currentHash == "" {
		t.Fatalf("module list did not include source hash: %+v", listResult.Project.Modules[0])
	}

	sourcePath := filepath.Join(t.TempDir(), "Module1.bas")
	replacement := "Attribute VB_Name = \"Module1\"\nPublic Sub Replaced()\nDebug.Print \"ok\"\nEnd Sub"
	if err := os.WriteFile(sourcePath, []byte(replacement), 0o644); err != nil {
		t.Fatalf("failed to write replacement source: %v", err)
	}
	replacedPath := filepath.Join(t.TempDir(), "replaced.xlsm")
	output := runVBACommand(t,
		"--format", "json",
		"vba", "replace-module", attachedPath,
		"--module", "module:Module1",
		"--source", sourcePath,
		"--expect-sha256", currentHash,
		"--out", replacedPath,
	)
	result := parseVBAModuleReplaceResult(t, output)
	if result.Output != replacedPath || result.DryRun || result.Result == nil || result.Result.Module.Name != "Module1" {
		t.Fatalf("unexpected replace-module result: %+v", result)
	}
	if result.Result.PreviousSHA256 != currentHash || result.Result.SHA256 == "" || result.Result.SHA256 == currentHash {
		t.Fatalf("unexpected source hashes: %+v", result.Result)
	}
	if result.Result.OfficeLoadVerified || result.Result.CompatibilityStatus != "experimental" {
		t.Fatalf("source rewrites must report experimental Office-load status: %+v", result.Result)
	}
	if result.Project == nil || result.Project.Modules[0].Source != "" {
		t.Fatalf("replace result should summarize project without embedding source: %+v", result.Project)
	}
	if result.Project.OfficeCompatibility == nil || result.Project.OfficeCompatibility.OfficeLoadVerified || result.Project.OfficeCompatibility.Status != "unverified" {
		t.Fatalf("replace result should include unverified Office compatibility: %+v", result.Project)
	}
	for label, command := range map[string]string{
		"inspect": result.InspectCommand,
		"list":    result.ListCommand,
		"package": result.PackageReadbackCommand,
	} {
		if command == "" {
			t.Fatalf("%s command missing: %+v", label, result)
		}
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}
	runVBACommand(t, "validate", "--strict", replacedPath)

	outDir := filepath.Join(t.TempDir(), "macros")
	runVBACommand(t, "--format", "json", "vba", "extract", replacedPath, "--out-dir", outDir, "--module", "Module1")
	moduleSource := readTextFileForVBATest(t, filepath.Join(outDir, "Module1.bas"))
	if !strings.Contains(moduleSource, "Public Sub Replaced()") || !strings.HasSuffix(moduleSource, "\r\n") {
		t.Fatalf("unexpected replaced source:\n%q", moduleSource)
	}
	classOutDir := filepath.Join(t.TempDir(), "class")
	runVBACommand(t, "--format", "json", "vba", "extract", replacedPath, "--out-dir", classOutDir, "--module", "Class1")
	classSource := readTextFileForVBATest(t, filepath.Join(classOutDir, "Class1.cls"))
	if !strings.Contains(classSource, "Public Function Answer()") {
		t.Fatalf("Class1 should be unchanged:\n%s", classSource)
	}

	dryOutput := runVBACommand(t,
		"--format", "json",
		"vba", "replace-module", attachedPath,
		"--module", "Module1",
		"--source", sourcePath,
		"--expect-sha256", currentHash,
		"--dry-run",
	)
	dryResult := parseVBAModuleReplaceResult(t, dryOutput)
	if !dryResult.DryRun || dryResult.Output != "" {
		t.Fatalf("unexpected dry-run result: %+v", dryResult)
	}
	for label, command := range map[string]string{
		"inspect": dryResult.InspectCommandTemplate,
		"list":    dryResult.ListCommandTemplate,
		"extract": dryResult.ExtractCommandTemplate,
	} {
		if !strings.Contains(command, "<out.xlsm>") {
			t.Fatalf("%s template missing output placeholder: %q", label, command)
		}
	}

	guardPath := filepath.Join(t.TempDir(), "guard.xlsm")
	args := []string{
		"vba", "replace-module", attachedPath,
		"--module", "Module1",
		"--source", sourcePath,
		"--expect-sha256", "deadbeef",
		"--out", guardPath,
	}
	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs(args)
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	if err == nil {
		t.Fatal("expected stale hash guard to fail")
	}
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("unexpected guard error: %#v", err)
	}
	if _, statErr := os.Stat(guardPath); !os.IsNotExist(statErr) {
		t.Fatalf("guarded failed replace should not write output, stat error = %v", statErr)
	}

	mismatchSourcePath := filepath.Join(t.TempDir(), "Other.bas")
	if err := os.WriteFile(mismatchSourcePath, []byte("Attribute VB_Name = \"Other\"\r\nPublic Sub Wrong()\r\nEnd Sub\r\n"), 0o644); err != nil {
		t.Fatalf("failed to write mismatched replacement source: %v", err)
	}
	mismatchPath := filepath.Join(t.TempDir(), "mismatch.xlsm")
	args = []string{
		"vba", "replace-module", attachedPath,
		"--module", "Module1",
		"--source", mismatchSourcePath,
		"--out", mismatchPath,
	}
	rootCmd = newTestRootCmd(t)
	rootCmd.SetArgs(args)
	rootCmd.SetOut(&stdout)
	err = rootCmd.Execute()
	if err == nil {
		t.Fatal("expected VB_Name mismatch to fail")
	}
	cliErr, ok = AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitInvalidArgs || !strings.Contains(cliErr.Message, "Attribute VB_Name") {
		t.Fatalf("unexpected mismatch error: %#v", err)
	}
	if _, statErr := os.Stat(mismatchPath); !os.IsNotExist(statErr) {
		t.Fatalf("mismatched failed replace should not write output, stat error = %v", statErr)
	}
}

func TestVBACLINoopReplacePreservesVBAProjectBytes(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	originalProject := syntheticCLIVBAProjectBinForTest(t)
	if err := os.WriteFile(binPath, originalProject, 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", attachedPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	current := listResult.Project.Modules[0]
	if current.Name != "Module1" || current.SHA256 == "" {
		t.Fatalf("unexpected module for no-op replace: %+v", current)
	}

	sourceDir := filepath.Join(t.TempDir(), "source")
	runVBACommand(t, "--format", "json", "vba", "extract", attachedPath, "--module", "Module1", "--out-dir", sourceDir)
	sourcePath := filepath.Join(sourceDir, "Module1.bas")
	noOpPath := filepath.Join(t.TempDir(), "noop.xlsm")
	output := runVBACommand(t,
		"--format", "json",
		"vba", "replace-module", attachedPath,
		"--module", "Module1",
		"--source", sourcePath,
		"--expect-sha256", current.SHA256,
		"--out", noOpPath,
	)
	result := parseVBAModuleReplaceResult(t, output)
	if result.Result == nil || result.Result.Action != "replace-module" || result.Result.SHA256 != current.SHA256 {
		t.Fatalf("unexpected no-op replace result: %+v", result)
	}
	if result.Result.CompatibilityStatus == "experimental" {
		t.Fatalf("exact no-op replace should not be reported as an experimental rewrite: %+v", result.Result)
	}
	if !strings.Contains(output, `"purgedCaches":false`) || !strings.Contains(output, `"recompilesOnOpen":false`) {
		t.Fatalf("no-op replace JSON must explicitly report false cache/recompile flags:\n%s", output)
	}

	extractedPath := filepath.Join(t.TempDir(), "extracted.bin")
	runVBACommand(t, "--format", "json", "vba", "extract-bin", noOpPath, "--out", extractedPath)
	extracted, err := os.ReadFile(extractedPath)
	if err != nil {
		t.Fatalf("failed to read extracted VBA bin: %v", err)
	}
	if !bytes.Equal(extracted, originalProject) {
		t.Fatal("exact no-op replace changed vbaProject.bin bytes")
	}
}

func TestVBAModuleSelectorMissListsCandidatesAndDiscovery(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, syntheticCLIVBAProjectBinForTest(t), 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	sourcePath := filepath.Join(t.TempDir(), "replacement.bas")
	if err := os.WriteFile(sourcePath, []byte("Public Sub Replacement()\r\nEnd Sub\r\n"), 0o644); err != nil {
		t.Fatalf("failed to write replacement source: %v", err)
	}

	tests := []struct {
		name string
		args []string
	}{
		{
			name: "extract",
			args: []string{"--format", "json", "vba", "extract", attachedPath,
				"--module", "Modul", "--out-dir", filepath.Join(t.TempDir(), "extract")},
		},
		{
			name: "remove",
			args: []string{"--format", "json", "vba", "remove-module", attachedPath,
				"--module", "Modul", "--out", filepath.Join(t.TempDir(), "removed.xlsm")},
		},
		{
			name: "replace",
			args: []string{"--format", "json", "vba", "replace-module", attachedPath,
				"--module", "Modul", "--source", sourcePath, "--out", filepath.Join(t.TempDir(), "replaced.xlsm")},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := executeVBACommandExpectError(t, tt.args...)
			cliErr, ok := AsCLIError(err)
			if !ok {
				t.Fatalf("expected CLIError, got %T", err)
			}
			if cliErr.ExitCode != ExitTargetNotFound {
				t.Fatalf("exit code = %d, want %d: %v", cliErr.ExitCode, ExitTargetNotFound, cliErr)
			}
			for _, want := range []string{
				"VBA module not found: Modul",
				"did you mean: module:Module1",
				"discover with `ooxml --json vba list",
				attachedPath,
			} {
				if !strings.Contains(cliErr.Message, want) {
					t.Fatalf("message %q missing %q", cliErr.Message, want)
				}
			}
		})
	}
}

func TestSelectVBAModulesAmbiguousListsDiscovery(t *testing.T) {
	modules := []vbapkg.SourceModule{
		{Number: 1, PrimarySelector: "module:Duplicate", Selectors: []string{"module:Duplicate", "Duplicate"}},
		{Number: 2, PrimarySelector: "module:Duplicate", Selectors: []string{"module:Duplicate", "Duplicate"}},
	}
	_, err := selectVBAModules("macro.xlsm", modules, "Duplicate")
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T", err)
	}
	if cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("exit code = %d, want %d", cliErr.ExitCode, ExitInvalidArgs)
	}
	for _, want := range []string{
		`VBA module selector "Duplicate" matched multiple modules`,
		"module:1, module:2",
		"discover with `ooxml --json vba list macro.xlsm`",
	} {
		if !strings.Contains(cliErr.Message, want) {
			t.Fatalf("message %q missing %q", cliErr.Message, want)
		}
	}
}

func TestVBACLIAddModuleSyntheticProject(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, syntheticCLIVBAProjectBinForTest(t), 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	modulePath := filepath.Join(t.TempDir(), "Module2.bas")
	if err := os.WriteFile(modulePath, []byte("Public Sub Added()\r\nDebug.Print \"added\"\r\nEnd Sub\r\n"), 0o644); err != nil {
		t.Fatalf("failed to write module source: %v", err)
	}
	addedPath := filepath.Join(t.TempDir(), "added.xlsm")
	output := runVBACommand(t,
		"--format", "json",
		"vba", "add-module", attachedPath,
		"--source", modulePath,
		"--expect-module-count", "2",
		"--out", addedPath,
	)
	result := parseVBAModuleReplaceResult(t, output)
	if result.Output != addedPath || result.DryRun || result.Result == nil || result.Result.Action != "add-module" {
		t.Fatalf("unexpected add-module result: %+v", result)
	}
	if result.Result.Module.Name != "Module2" || result.Result.Module.Kind != "standard" || result.Result.PreviousCount != 2 || result.Result.ModuleCount != 3 || result.Result.SHA256 == "" {
		t.Fatalf("unexpected add result metadata: %+v", result.Result)
	}
	if result.Project == nil || result.Project.ModuleCount != 3 || len(result.Project.Modules) != 3 {
		t.Fatalf("unexpected project readback: %+v", result.Project)
	}
	for label, command := range map[string]string{
		"inspect":  result.InspectCommand,
		"validate": result.ValidateCommand,
		"list":     result.ListCommand,
		"extract":  result.ExtractCommand,
		"package":  result.PackageReadbackCommand,
	} {
		if command == "" {
			t.Fatalf("%s command missing: %+v", label, result)
		}
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}
	runVBACommand(t, "validate", "--strict", addedPath)

	outDir := filepath.Join(t.TempDir(), "macros")
	runVBACommand(t, "--format", "json", "vba", "extract", addedPath, "--out-dir", outDir, "--module", "Module2")
	moduleSource := readTextFileForVBATest(t, filepath.Join(outDir, "Module2.bas"))
	if !strings.Contains(moduleSource, "Attribute VB_Name = \"Module2\"") || !strings.Contains(moduleSource, "Public Sub Added()") {
		t.Fatalf("unexpected added module source:\n%s", moduleSource)
	}

	classPath := filepath.Join(t.TempDir(), "Class2.cls")
	if err := os.WriteFile(classPath, []byte("Attribute VB_Name = \"Class2\"\r\nPublic Function AddedClass()\r\nAddedClass = 9\r\nEnd Function\r\n"), 0o644); err != nil {
		t.Fatalf("failed to write class source: %v", err)
	}
	addedClassPath := filepath.Join(t.TempDir(), "added-class.xlsm")
	classOutput := runVBACommand(t,
		"--format", "json",
		"vba", "add-module", addedPath,
		"--source", classPath,
		"--expect-module-count", "3",
		"--out", addedClassPath,
	)
	classResult := parseVBAModuleReplaceResult(t, classOutput)
	if classResult.Result == nil || classResult.Result.Module.Name != "Class2" || classResult.Result.Module.Kind != "class" || classResult.Result.ModuleCount != 4 {
		t.Fatalf("unexpected class add result: %+v", classResult)
	}
	classOutDir := filepath.Join(t.TempDir(), "classes")
	runVBACommand(t, "--format", "json", "vba", "extract", addedClassPath, "--out-dir", classOutDir, "--module", "Class2")
	classSource := readTextFileForVBATest(t, filepath.Join(classOutDir, "Class2.cls"))
	if !strings.Contains(classSource, "Public Function AddedClass()") {
		t.Fatalf("unexpected added class source:\n%s", classSource)
	}

	dryOutput := runVBACommand(t,
		"--format", "json",
		"vba", "add-module", attachedPath,
		"--source", modulePath,
		"--expect-module-count", "2",
		"--dry-run",
	)
	dryResult := parseVBAModuleReplaceResult(t, dryOutput)
	if !dryResult.DryRun || dryResult.Output != "" {
		t.Fatalf("unexpected dry-run add result: %+v", dryResult)
	}
	for label, command := range map[string]string{
		"inspect": dryResult.InspectCommandTemplate,
		"list":    dryResult.ListCommandTemplate,
		"extract": dryResult.ExtractCommandTemplate,
	} {
		if !strings.Contains(command, "<out.xlsm>") {
			t.Fatalf("%s template missing output placeholder: %q", label, command)
		}
	}

	guardPath := filepath.Join(t.TempDir(), "guard.xlsm")
	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"vba", "add-module", attachedPath, "--source", modulePath, "--expect-module-count", "99", "--out", guardPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	if err == nil {
		t.Fatal("expected stale module count guard to fail")
	}
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("unexpected guard error: %#v", err)
	}
	if _, statErr := os.Stat(guardPath); !os.IsNotExist(statErr) {
		t.Fatalf("guarded failed add should not write output, stat error = %v", statErr)
	}
}

func TestVBACLIRemoveModuleSyntheticProject(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(t.TempDir(), "vbaProject.bin")
	if err := os.WriteFile(binPath, syntheticCLIVBAProjectBinForTest(t), 0o644); err != nil {
		t.Fatalf("failed to write VBA bin: %v", err)
	}
	attachedPath := filepath.Join(t.TempDir(), "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", attachedPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	currentHash := listResult.Project.Modules[0].SHA256
	if currentHash == "" {
		t.Fatalf("module list did not include source hash: %+v", listResult.Project.Modules[0])
	}

	removedPath := filepath.Join(t.TempDir(), "removed.xlsm")
	output := runVBACommand(t,
		"--format", "json",
		"vba", "remove-module", attachedPath,
		"--module", "module:Module1",
		"--expect-sha256", currentHash,
		"--out", removedPath,
	)
	result := parseVBAModuleReplaceResult(t, output)
	if result.Output != removedPath || result.DryRun || result.Result == nil || result.Result.Action != "remove-module" {
		t.Fatalf("unexpected remove-module result: %+v", result)
	}
	if result.Result.Module.Name != "Module1" || result.Result.PreviousSHA256 != currentHash || result.Result.SHA256 != "" {
		t.Fatalf("unexpected removed module metadata: %+v", result.Result)
	}
	if result.ExtractCommand != "" || result.ExtractCommandTemplate != "" {
		t.Fatalf("remove-module should not emit extract command for deleted module: %+v", result)
	}
	if result.Project == nil || result.Project.ModuleCount != 1 || len(result.Project.Modules) != 1 || result.Project.Modules[0].Name != "Class1" {
		t.Fatalf("unexpected post-remove project: %+v", result.Project)
	}
	for label, command := range map[string]string{
		"inspect":  result.InspectCommand,
		"validate": result.ValidateCommand,
		"list":     result.ListCommand,
		"package":  result.PackageReadbackCommand,
	} {
		if command == "" {
			t.Fatalf("%s command missing: %+v", label, result)
		}
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}
	runVBACommand(t, "validate", "--strict", removedPath)

	classOutDir := filepath.Join(t.TempDir(), "class")
	runVBACommand(t, "--format", "json", "vba", "extract", removedPath, "--out-dir", classOutDir, "--module", "Class1")
	classSource := readTextFileForVBATest(t, filepath.Join(classOutDir, "Class1.cls"))
	if !strings.Contains(classSource, "Public Function Answer()") {
		t.Fatalf("Class1 should remain extractable:\n%s", classSource)
	}

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"vba", "extract", removedPath, "--out-dir", filepath.Join(t.TempDir(), "gone"), "--module", "Module1"})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	if err == nil {
		t.Fatal("expected extracting removed Module1 to fail")
	}
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("unexpected removed-module extract error: %#v", err)
	}

	dryOutput := runVBACommand(t,
		"--format", "json",
		"vba", "remove-module", attachedPath,
		"--module", "Module1",
		"--expect-sha256", currentHash,
		"--dry-run",
	)
	dryResult := parseVBAModuleReplaceResult(t, dryOutput)
	if !dryResult.DryRun || dryResult.Output != "" || dryResult.ExtractCommandTemplate != "" {
		t.Fatalf("unexpected dry-run remove result: %+v", dryResult)
	}
	for label, command := range map[string]string{
		"inspect": dryResult.InspectCommandTemplate,
		"list":    dryResult.ListCommandTemplate,
	} {
		if !strings.Contains(command, "<out.xlsm>") {
			t.Fatalf("%s template missing output placeholder: %q", label, command)
		}
	}
}

func TestVBARealXLSMSmokeListExtractReplaceAndGuardModuleSetChanges(t *testing.T) {
	inputPath := realXLSMSmokeFixtureForTest(t)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", inputPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	if listResult.Project == nil || listResult.Project.ModuleCount < 2 {
		t.Fatalf("real XLSM fixture did not expose a useful VBA project: %+v", listResult.Project)
	}
	standard := firstVBAModuleOfKindForTest(t, listResult.Project.Modules, "standard")
	classModule := firstVBAModuleOfKindForTest(t, listResult.Project.Modules, "class")
	if standard.SourceOffset == 0 {
		t.Fatalf("real Office standard module should carry a non-zero source offset before rewrite: %+v", standard)
	}
	if classModule.SourceOffset == 0 {
		t.Fatalf("real Office class module should carry a non-zero source offset before rewrite: %+v", classModule)
	}
	officeCheckUsable := vbaOfficeCheckPassesIfEngineAvailable(t, inputPath)

	extractDir := filepath.Join(t.TempDir(), "extract")
	runVBACommand(t, "--format", "json", "vba", "extract", inputPath, "--out-dir", extractDir, "--module", standard.PrimarySelector)
	standardSource := readTextFileForVBATest(t, filepath.Join(extractDir, standard.Name+standard.Extension))
	if !strings.Contains(standardSource, `Attribute VB_Name = "`+standard.Name+`"`) {
		t.Fatalf("real standard module extract missed VB_Name for %s:\n%s", standard.Name, standardSource)
	}
	runVBACommand(t, "--format", "json", "vba", "extract", inputPath, "--out-dir", extractDir, "--module", classModule.PrimarySelector)
	classSource := readTextFileForVBATest(t, filepath.Join(extractDir, classModule.Name+classModule.Extension))
	if !strings.Contains(classSource, `Attribute VB_Name = "`+classModule.Name+`"`) {
		t.Fatalf("real class module extract missed VB_Name for %s:\n%s", classModule.Name, classSource)
	}

	standardPath := filepath.Join(t.TempDir(), standard.Name+".bas")
	writeVBASourceForTest(t, standardPath,
		"Attribute VB_Name = \""+standard.Name+"\"\r\n"+
			"Public Sub "+standard.Name+"Run()\r\n"+
			"    Debug.Print \"agent smoke replaced\"\r\n"+
			"End Sub\r\n",
	)
	replacedPath := filepath.Join(t.TempDir(), "replaced-standard.xlsm")
	replaceOutput := runVBACommand(t,
		"--format", "json",
		"vba", "replace-module", inputPath,
		"--module", standard.PrimarySelector,
		"--source", standardPath,
		"--expect-sha256", standard.SHA256,
		"--allow-experimental-vba-source-rewrite",
		"--out", replacedPath,
	)
	replaceResult := parseVBAModuleReplaceResult(t, replaceOutput)
	if replaceResult.Result == nil || replaceResult.Result.Module.Name != standard.Name || replaceResult.Result.PreviousSHA256 != standard.SHA256 || replaceResult.Result.SHA256 == standard.SHA256 {
		t.Fatalf("unexpected real standard replace result: %+v", replaceResult)
	}
	runVBACommand(t, "validate", "--strict", replacedPath)
	replacedExtractDir := filepath.Join(t.TempDir(), "replaced-extract")
	runVBACommand(t, "--format", "json", "vba", "extract", replacedPath, "--out-dir", replacedExtractDir, "--module", standard.Name)
	replacedSource := readTextFileForVBATest(t, filepath.Join(replacedExtractDir, standard.Name+".bas"))
	if !strings.Contains(replacedSource, "agent smoke replaced") {
		t.Fatalf("real standard replace was not readable after extraction:\n%s", replacedSource)
	}
	if officeCheckUsable {
		assertVBAOfficeCheckPasses(t, replacedPath)
	}

	newModuleName := unusedVBAModuleNameForTest(listResult.Project.Modules, "AgentSmoke")
	newModulePath := filepath.Join(t.TempDir(), newModuleName+".bas")
	writeVBASourceForTest(t, newModulePath,
		"Attribute VB_Name = \""+newModuleName+"\"\r\n"+
			"Public Sub "+newModuleName+"Run()\r\n"+
			"    Debug.Print \"blocked\"\r\n"+
			"End Sub\r\n",
	)
	addBlockedPath := filepath.Join(t.TempDir(), "add-blocked.xlsm")
	addErr := executeVBACommandExpectError(t,
		"--format", "json",
		"vba", "add-module", inputPath,
		"--source", newModulePath,
		"--expect-module-count", strconv.Itoa(listResult.Project.ModuleCount),
		"--allow-experimental-vba-source-rewrite",
		"--out", addBlockedPath,
	)
	assertVersionDependentProjectGuardForVBATest(t, addErr, addBlockedPath)

	removeBlockedPath := filepath.Join(t.TempDir(), "remove-blocked.xlsm")
	removeErr := executeVBACommandExpectError(t,
		"--format", "json",
		"vba", "remove-module", inputPath,
		"--module", classModule.PrimarySelector,
		"--expect-sha256", classModule.SHA256,
		"--allow-experimental-vba-source-rewrite",
		"--out", removeBlockedPath,
	)
	assertVersionDependentProjectGuardForVBATest(t, removeErr, removeBlockedPath)
}

func TestVBARealPPTMSmokeListExtractAndOfficeCheck(t *testing.T) {
	inputPath := realPPTMSmokeFixtureForTest(t)

	listOutput := runVBACommand(t, "--format", "json", "vba", "list", inputPath)
	listResult := parseVBAModuleListResult(t, listOutput)
	if listResult.VBA == nil || listResult.VBA.Family != "pptx" || !listResult.VBA.HasVBAProject {
		t.Fatalf("real PPTM fixture did not expose PPTM VBA state: %+v", listResult.VBA)
	}
	if listResult.Project == nil || listResult.Project.ModuleCount == 0 {
		t.Fatalf("real PPTM fixture did not expose source modules: %+v", listResult.Project)
	}
	module := firstExtractableVBAModuleForTest(t, listResult.Project.Modules)
	extractDir := filepath.Join(t.TempDir(), "pptm-extract")
	extractOutput := runVBACommand(t, "--format", "json", "vba", "extract", inputPath, "--out-dir", extractDir, "--module", module.PrimarySelector)
	extractResult := parseVBAModuleExtractResult(t, extractOutput)
	if len(extractResult.Modules) != 1 || extractResult.Modules[0].Name != module.Name {
		t.Fatalf("unexpected PPTM extract result: %+v", extractResult.Modules)
	}
	source := readTextFileForVBATest(t, filepath.Join(extractDir, module.Name+module.Extension))
	if !strings.Contains(source, `Attribute VB_Name = "`+module.Name+`"`) {
		t.Fatalf("PPTM module extract missed VB_Name for %s:\n%s", module.Name, source)
	}

	if listResult.Project.OfficeCompatibility != nil && listResult.Project.OfficeCompatibility.Status == "risk" {
		if len(listResult.Project.HostCompatibilityWarnings) == 0 {
			t.Fatalf("PPTM risk status should include hostCompatibilityWarnings: %+v", listResult.Project)
		}
		assertVBAOfficeCheckBlockedByValidation(t, inputPath)
		return
	}
	assertVBAOfficeCheckPassesIfEngineAvailable(t, inputPath)
}

func TestVBAExtractBinMissingMacroReturnsTargetNotFound(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	outPath := filepath.Join(t.TempDir(), "vbaProject.bin")

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{"vba", "extract-bin", inputPath, "--out", outPath})
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	err := rootCmd.Execute()
	if err == nil {
		t.Fatal("expected extract-bin to fail for workbook without macros")
	}
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("unexpected error: %#v", err)
	}
	if !strings.Contains(err.Error(), "no vbaProject.bin") {
		t.Fatalf("unexpected error message: %v", err)
	}
}

func TestVBAModuleListMissingMacroSuggestsInspectAndAttach(t *testing.T) {
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	err := executeVBACommandExpectError(t, "--format", "json", "vba", "list", inputPath)
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("unexpected error: %#v", err)
	}
	for _, want := range []string{
		"package has no vbaProject.bin part",
		"ooxml --json vba inspect",
		"ooxml --json vba attach",
		"<out.xlsm>",
	} {
		if !strings.Contains(err.Error(), want) {
			t.Fatalf("missing %q in error: %v", want, err)
		}
	}
}

func runVBACommand(t *testing.T, args ...string) string {
	t.Helper()
	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs(args)
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	if err := rootCmd.Execute(); err != nil {
		t.Fatalf("command %v failed: %v", args, err)
	}
	return stdout.String()
}

func executeVBACommandExpectError(t *testing.T, args ...string) error {
	t.Helper()
	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs(args)
	rootCmd.SetOut(&bytes.Buffer{})
	rootCmd.SetErr(&bytes.Buffer{})
	err := rootCmd.Execute()
	if err == nil {
		t.Fatalf("command %v succeeded, want error", args)
	}
	return err
}

func assertVersionDependentProjectGuardForVBATest(t *testing.T, err error, outputPath string) {
	t.Helper()
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("unexpected version-dependent project guard error: %#v", err)
	}
	if !strings.Contains(cliErr.Message, "version-dependent _VBA_PROJECT metadata") {
		t.Fatalf("guard error did not explain _VBA_PROJECT metadata: %v", cliErr.Message)
	}
	if _, statErr := os.Stat(outputPath); !os.IsNotExist(statErr) {
		t.Fatalf("guarded failed module-set mutation should not write output, stat error = %v", statErr)
	}
}

func runVBACommandAllowError(t *testing.T, args ...string) (string, error) {
	t.Helper()
	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs(args)
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	rootCmd.SetErr(&bytes.Buffer{})
	err := rootCmd.Execute()
	return stdout.String(), err
}

func localOfficeCheckEngineAvailableForTest() bool {
	if _, err := exec.LookPath("soffice"); err == nil {
		return true
	}
	if _, err := exec.LookPath("libreoffice"); err == nil {
		return true
	}
	return false
}

func assertVBAOfficeCheckPassesIfEngineAvailable(t *testing.T, path string) {
	t.Helper()
	_ = vbaOfficeCheckPassesIfEngineAvailable(t, path)
}

func vbaOfficeCheckPassesIfEngineAvailable(t *testing.T, path string) bool {
	t.Helper()
	if !localOfficeCheckEngineAvailableForTest() {
		t.Log("skipping local VBA office-check proof: soffice/libreoffice not on PATH")
		return false
	}
	output, err := runVBACommandAllowError(t, "--format", "json", "vba", "office-check", path, "--out-dir", t.TempDir())
	if err != nil {
		result := parseVBAOfficeCheckResult(t, output)
		if result.OpenCheck != nil && result.OpenCheck.ErrorCode == "engine_failed" {
			t.Logf("skipping local VBA office-check proof: local engine failed on %s: %s", path, result.OpenCheck.Error)
			return false
		}
		t.Fatalf("office-check failed for %s: %v\n%s", path, err, output)
	}
	assertVBAOfficeCheckResultPassed(t, path, output)
	return true
}

func assertVBAOfficeCheckPasses(t *testing.T, path string) {
	t.Helper()
	output := runVBACommand(t, "--format", "json", "vba", "office-check", path, "--out-dir", t.TempDir())
	assertVBAOfficeCheckResultPassed(t, path, output)
}

func assertVBAOfficeCheckResultPassed(t *testing.T, path string, output string) {
	t.Helper()
	result := parseVBAOfficeCheckResult(t, output)
	if !result.PackageValid || !result.OverallVerified || result.OpenCheck == nil || result.OpenCheck.Status != "passed" || !result.OpenCheck.OfficeOpenVerified {
		t.Fatalf("office-check did not pass for %s: %+v", path, result)
	}
	if result.MicrosoftOffice || result.MacroExecution || result.MacroCompilation {
		t.Fatalf("office-check must not overclaim Microsoft Office/runtime proof: %+v", result)
	}
}

func assertVBAOfficeCheckBlockedByValidation(t *testing.T, path string) {
	t.Helper()
	output, err := runVBACommandAllowError(t, "--format", "json", "vba", "office-check", path, "--out-dir", t.TempDir())
	if err == nil {
		t.Fatalf("office-check should be blocked by strict validation warnings for %s", path)
	}
	cliErr, ok := AsCLIError(err)
	if !ok || cliErr.ExitCode != ExitValidationFailed || !cliErr.Reported {
		t.Fatalf("unexpected office-check error for validation-blocked package: %#v", err)
	}
	result := parseVBAOfficeCheckResult(t, output)
	if result.PackageValid || result.OverallVerified || result.OpenCheck == nil || result.OpenCheck.Status != "skipped" || result.OpenCheck.ErrorCode != "package_validation_failed" {
		t.Fatalf("unexpected validation-blocked office-check result for %s: %+v", path, result)
	}
}

func parseVBAMutationResult(t *testing.T, data string) VBAMutationCLIResult {
	t.Helper()
	var result VBAMutationCLIResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA mutation JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAInspectResult(t *testing.T, data string) VBAInspectResult {
	t.Helper()
	var result VBAInspectResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA inspect JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAExtractResult(t *testing.T, data string) VBAExtractResult {
	t.Helper()
	var result VBAExtractResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA extract JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAModuleListResult(t *testing.T, data string) VBAModuleListResult {
	t.Helper()
	var result VBAModuleListResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA module list JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAInspectBinResult(t *testing.T, data string) VBAInspectBinResult {
	t.Helper()
	var result VBAInspectBinResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA inspect-bin JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBACreateResult(t *testing.T, data string) VBACreateResult {
	t.Helper()
	var result VBACreateResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA create JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAModuleExtractResult(t *testing.T, data string) VBAModuleExtractResult {
	t.Helper()
	var result VBAModuleExtractResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA module extract JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAModuleReplaceResult(t *testing.T, data string) VBAModuleReplaceResult {
	t.Helper()
	var result VBAModuleReplaceResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA module replace JSON: %v\n%s", err, data)
	}
	return result
}

func parseVBAOfficeCheckResult(t *testing.T, data string) VBAOfficeCheckResult {
	t.Helper()
	var result VBAOfficeCheckResult
	if err := json.Unmarshal([]byte(data), &result); err != nil {
		t.Fatalf("failed to unmarshal VBA office-check JSON: %v\n%s", err, data)
	}
	return result
}

func realXLSMSmokeFixtureForTest(t *testing.T) string {
	t.Helper()
	if path := strings.TrimSpace(os.Getenv("OOXML_VBA_SMOKE_XLSM")); path != "" {
		if _, err := os.Stat(path); err != nil {
			t.Skipf("OOXML_VBA_SMOKE_XLSM=%s not found: %v", path, err)
		}
		return path
	}
	path := filepath.Join("..", "..", "testdata", "vba", "local", "2026-02-02 PBI Kostenrechner v2.xlsm")
	if _, err := os.Stat(path); err != nil {
		t.Skip("OOXML_VBA_SMOKE_XLSM not set and local ignored XLSM fixture is unavailable")
	}
	return path
}

func realPPTMSmokeFixtureForTest(t *testing.T) string {
	t.Helper()
	if path := strings.TrimSpace(os.Getenv("OOXML_VBA_SMOKE_PPTM")); path != "" {
		if _, err := os.Stat(path); err != nil {
			t.Skipf("OOXML_VBA_SMOKE_PPTM=%s not found: %v", path, err)
		}
		return path
	}
	localPath := filepath.Join(
		"..", "..", "testdata", "vba", "local",
		"PBI_Akquisition_DE_PPTM_Self_Update_EXPERIMENTAL.pptm",
	)
	if _, err := os.Stat(localPath); err == nil {
		return localPath
	}
	t.Skip("OOXML_VBA_SMOKE_PPTM not set and local ignored PPTM oracle is unavailable")
	return ""
}

func firstVBAModuleOfKindForTest(t *testing.T, modules []vbapkg.SourceModule, kind string) vbapkg.SourceModule {
	t.Helper()
	for _, module := range modules {
		if strings.EqualFold(module.Kind, kind) {
			return module
		}
	}
	t.Fatalf("no VBA module of kind %q in %+v", kind, modules)
	return vbapkg.SourceModule{}
}

func firstExtractableVBAModuleForTest(t *testing.T, modules []vbapkg.SourceModule) vbapkg.SourceModule {
	t.Helper()
	for _, module := range modules {
		if module.SourceBytes > 0 && module.PrimarySelector != "" && module.Extension != "" {
			return module
		}
	}
	t.Fatalf("no extractable VBA module in %+v", modules)
	return vbapkg.SourceModule{}
}

func unusedVBAModuleNameForTest(modules []vbapkg.SourceModule, preferred string) string {
	for i := 0; i < 100; i++ {
		candidate := preferred
		if i > 0 {
			candidate += strconv.Itoa(i + 1)
		}
		if !vbaModuleExistsForTest(modules, candidate) {
			return candidate
		}
	}
	return preferred + "Fallback"
}

func vbaModuleByNameForTest(t *testing.T, modules []vbapkg.SourceModule, name string) vbapkg.SourceModule {
	t.Helper()
	for _, module := range modules {
		if strings.EqualFold(module.Name, name) {
			return module
		}
	}
	t.Fatalf("VBA module %q not found in %+v", name, modules)
	return vbapkg.SourceModule{}
}

func vbaModuleExistsForTest(modules []vbapkg.SourceModule, name string) bool {
	for _, module := range modules {
		if strings.EqualFold(module.Name, name) {
			return true
		}
	}
	return false
}

func writeVBASourceForTest(t *testing.T, path string, source string) {
	t.Helper()
	if err := os.WriteFile(path, []byte(source), 0o644); err != nil {
		t.Fatalf("failed to write VBA source %s: %v", path, err)
	}
}

func assertVBAMutationReadback(t *testing.T, result VBAMutationCLIResult, inputPath, outputPath, action string, wantMacro bool) {
	t.Helper()
	if result.File != inputPath || result.Output != outputPath {
		t.Fatalf("unexpected mutation file metadata: %+v", result)
	}
	if result.Result == nil || result.Result.Action != action || result.Result.MacroEnabled != wantMacro {
		t.Fatalf("unexpected mutation result: %+v", result.Result)
	}
	if result.VBA == nil {
		t.Fatal("missing post-mutation VBA readback")
	}
	if result.VBA.MacroEnabled != wantMacro || result.VBA.HasVBAProject != wantMacro {
		t.Fatalf("unexpected VBA readback state: %+v", result.VBA)
	}
	if wantMacro {
		if result.VBA.VBAProject == nil || !result.VBA.VBAProject.Exists || result.VBA.VBAProject.PartURI == "" || result.VBA.VBAProject.RelationshipID == "" {
			t.Fatalf("missing VBA project readback details: %+v", result.VBA)
		}
	} else if result.VBA.VBAProject != nil && result.VBA.VBAProject.Exists {
		t.Fatalf("unexpected VBA project after remove: %+v", result.VBA.VBAProject)
	}
}

func assertExecutableVBAMutationCommands(t *testing.T, result VBAMutationCLIResult) {
	t.Helper()
	for label, command := range map[string]string{
		"inspect":  result.InspectCommand,
		"validate": result.ValidateCommand,
		"package":  result.PackageReadbackCommand,
	} {
		if command == "" {
			t.Fatalf("missing %s command: %+v", label, result)
		}
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}
	if result.VBA != nil && result.VBA.HasVBAProject {
		if result.ExtractBinCommand == "" || !strings.Contains(result.ExtractBinCommand, "vba extract-bin") {
			t.Fatalf("missing extract command for macro output: %+v", result)
		}
		if !strings.HasPrefix(result.ExtractBinCommand, "ooxml --json vba extract-bin") {
			t.Fatalf("extract-bin command should be JSON-first, got %q", result.ExtractBinCommand)
		}
	}
	if result.ListCommand != "" {
		executeGeneratedOOXMLCommandForVBATest(t, result.ListCommand)
	}
	if result.NextMutationTemplate == "" {
		t.Fatalf("missing next mutation template: %+v", result)
	}
}

func assertVBAExtractCommands(t *testing.T, result VBAExtractResult, inputPath, outputPath string) {
	t.Helper()
	if result.File != inputPath || result.Output != outputPath || result.BytesWritten == 0 {
		t.Fatalf("unexpected extract metadata: %+v", result)
	}
	for label, command := range map[string]string{
		"inspect":  result.InspectCommand,
		"validate": result.ValidateCommand,
		"package":  result.PackageReadbackCommand,
	} {
		if command == "" {
			t.Fatalf("missing %s command: %+v", label, result)
		}
		executeGeneratedOOXMLCommandForVBATest(t, command)
	}
	if !strings.Contains(result.AttachCommandTemplate, outputPath) || !strings.Contains(result.AttachCommandTemplate, "<out.pptm>") {
		t.Fatalf("unexpected attach command template: %q", result.AttachCommandTemplate)
	}
}

func assertVBAState(t *testing.T, path string, wantMacro bool) {
	t.Helper()
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open %s: %v", path, err)
	}
	defer pkg.Close()
	info, err := vbapkg.Inspect(pkg)
	if err != nil {
		t.Fatalf("failed to inspect %s: %v", path, err)
	}
	if info.MacroEnabled != wantMacro || info.HasVBAProject != wantMacro {
		t.Fatalf("macro state for %s = macroEnabled:%t hasVBA:%t, want %t", path, info.MacroEnabled, info.HasVBAProject, wantMacro)
	}
}

func executeGeneratedOOXMLCommandForVBATest(t *testing.T, command string) string {
	t.Helper()
	if !strings.HasPrefix(command, "ooxml ") {
		t.Fatalf("generated command must start with ooxml: %s", command)
	}
	args := splitGeneratedOOXMLCommandForXLSXTest(t, command)[1:]
	for i := 0; i < len(args)-1; i++ {
		if args[i] == "--out-dir" && !filepath.IsAbs(args[i+1]) {
			args[i+1] = filepath.Join(t.TempDir(), args[i+1])
		}
	}
	output := runVBACommand(t, args...)
	if strings.TrimSpace(output) == "" {
		t.Fatalf("generated command returned empty output: %s", command)
	}
	return output
}

func readTextFileForVBATest(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read %s: %v", path, err)
	}
	return string(data)
}

func containsWarningForVBATest(warnings []string, want string) bool {
	for _, warning := range warnings {
		if strings.Contains(warning, want) {
			return true
		}
	}
	return false
}

func containsDiagnosticCodeForVBATest(diags []DiagnosticJSON, want string) bool {
	for _, diag := range diags {
		if diag.Code == want {
			return true
		}
	}
	return false
}

func containsHostCompatibilityCodeForVBATest(warnings []vbapkg.HostCompatibilityWarning, want string) bool {
	for _, warning := range warnings {
		if warning.Code == want {
			return true
		}
	}
	return false
}

type vbaOfficeCheckFakeRunner struct {
	paths map[string]string
	run   func(name string, args []string) (*officecheck.RunResult, error)
}

func (r vbaOfficeCheckFakeRunner) LookPath(name string) (string, error) {
	if p, ok := r.paths[name]; ok {
		return p, nil
	}
	return "", errors.New("missing")
}

func (r vbaOfficeCheckFakeRunner) Run(_ context.Context, name string, args []string) (*officecheck.RunResult, error) {
	if r.run != nil {
		return r.run(name, args)
	}
	return &officecheck.RunResult{}, nil
}

func stubVBAOfficeCheckTools(t *testing.T, paths map[string]string, run func(name string, args []string) (*officecheck.RunResult, error)) func() {
	t.Helper()
	previous := vbaOfficeCheckToolsFactory
	vbaOfficeCheckToolsFactory = func() *officecheck.Tools {
		return &officecheck.Tools{Runner: vbaOfficeCheckFakeRunner{paths: paths, run: run}}
	}
	return func() {
		vbaOfficeCheckToolsFactory = previous
	}
}

func flagValueForVBATest(args []string, flag string) string {
	for i, arg := range args {
		if arg == flag && i+1 < len(args) {
			return args[i+1]
		}
	}
	return ""
}

func syntheticCLIVBAProjectBinForTest(t *testing.T) []byte {
	t.Helper()
	return syntheticCLIVBAProjectBinForModulesTest(t, []syntheticCLIVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	})
}

func syntheticCLIVBAProjectBinForModulesTest(t *testing.T, modules []syntheticCLIVBAModule) []byte {
	t.Helper()
	streams := map[string][]byte{
		"VBA/dir":          compressedCLILiteralsForTest(syntheticCLIDirStreamForTest(modules)),
		"VBA/_VBA_PROJECT": []byte{0xCC, 0x61},
	}
	for _, module := range modules {
		streams["VBA/"+module.StreamName] = compressedCLILiteralsForTest([]byte(module.Source))
	}
	return syntheticCLICFBForTest(t, streams)
}

type syntheticCLIVBAModule struct {
	Name       string
	StreamName string
	Kind       string
	Source     string
}

func syntheticCLIDirStreamForTest(modules []syntheticCLIVBAModule) []byte {
	var out []byte
	out = append(out, cliDirRecordForTest(0x0003, cliLE16ForTest(1252))...)
	out = append(out, cliDirRecordForTest(0x000F, cliLE16ForTest(uint16(len(modules))))...)
	for _, module := range modules {
		out = append(out, cliDirRecordForTest(0x0019, []byte(module.Name))...)
		out = append(out, cliDirRecordForTest(0x0047, cliUTF16BytesForTest(module.Name))...)
		out = append(out, cliDirRecordForTest(0x001A, []byte(module.StreamName))...)
		out = append(out, cliDirRecordForTest(0x0032, cliUTF16BytesForTest(module.StreamName))...)
		out = append(out, cliDirRecordForTest(0x0031, cliLE32ForTest(0))...)
		if module.Kind == "class" {
			out = append(out, cliDirRecordForTest(0x0022, nil)...)
		} else {
			out = append(out, cliDirRecordForTest(0x0021, nil)...)
		}
		out = append(out, cliDirRecordForTest(0x002B, nil)...)
	}
	out = append(out, cliDirRecordForTest(0x0010, nil)...)
	return out
}

func cliDirRecordForTest(id uint16, payload []byte) []byte {
	out := make([]byte, 6+len(payload))
	binary.LittleEndian.PutUint16(out[:2], id)
	binary.LittleEndian.PutUint32(out[2:6], uint32(len(payload)))
	copy(out[6:], payload)
	return out
}

func compressedCLILiteralsForTest(raw []byte) []byte {
	out := []byte{0x01}
	for len(raw) > 0 {
		chunk := raw
		if len(chunk) > 3600 {
			chunk = raw[:3600]
		}
		var chunkData []byte
		for offset := 0; offset < len(chunk); {
			n := len(chunk) - offset
			if n > 8 {
				n = 8
			}
			chunkData = append(chunkData, 0x00)
			chunkData = append(chunkData, chunk[offset:offset+n]...)
			offset += n
		}
		header := uint16(len(chunkData)-1) | 0x3000 | 0x8000
		out = binary.LittleEndian.AppendUint16(out, header)
		out = append(out, chunkData...)
		raw = raw[len(chunk):]
	}
	return out
}

type cliCFBEntryForTest struct {
	name        string
	objectType  byte
	left        uint32
	right       uint32
	child       uint32
	startSector uint32
	size        uint64
}

func syntheticCLICFBForTest(t *testing.T, streams map[string][]byte) []byte {
	t.Helper()
	const sectorSize = 512
	const noStream = uint32(0xFFFFFFFF)
	const endOfChain = uint32(0xFFFFFFFE)
	const fatSector = uint32(0xFFFFFFFD)

	names := []string{"dir", "_VBA_PROJECT"}
	var moduleNames []string
	for path := range streams {
		if !strings.HasPrefix(path, "VBA/") {
			continue
		}
		name := strings.TrimPrefix(path, "VBA/")
		if name == "dir" || name == "_VBA_PROJECT" {
			continue
		}
		moduleNames = append(moduleNames, name)
	}
	sort.Strings(moduleNames)
	names = append(names, moduleNames...)
	var sectors [][]byte
	sectors = append(sectors, make([]byte, sectorSize))
	entries := []cliCFBEntryForTest{
		{name: "Root Entry", objectType: 5, child: 1, left: noStream, right: noStream, startSector: endOfChain},
		{name: "VBA", objectType: 1, child: 2, left: noStream, right: noStream, startSector: endOfChain},
	}
	for idx, name := range names {
		data, ok := streams["VBA/"+name]
		if !ok {
			continue
		}
		start := uint32(len(sectors))
		padded := append([]byte{}, data...)
		for len(padded)%sectorSize != 0 {
			padded = append(padded, 0)
		}
		for len(padded) > 0 {
			sectors = append(sectors, append([]byte{}, padded[:sectorSize]...))
			padded = padded[sectorSize:]
		}
		right := noStream
		if idx < len(names)-1 {
			right = uint32(len(entries) + 1)
		}
		entries = append(entries, cliCFBEntryForTest{name: name, objectType: 2, left: noStream, right: right, child: noStream, startSector: start, size: uint64(len(data))})
	}
	dirStart := uint32(len(sectors))
	dirData := make([]byte, 0, ((len(entries)*128+sectorSize-1)/sectorSize)*sectorSize)
	for _, entry := range entries {
		dirData = append(dirData, cliDirectoryEntryForTest(entry)...)
	}
	for len(dirData)%sectorSize != 0 {
		dirData = append(dirData, 0)
	}
	for len(dirData) > 0 {
		sectors = append(sectors, append([]byte{}, dirData[:sectorSize]...))
		dirData = dirData[sectorSize:]
	}
	fat := make([]uint32, len(sectors))
	for i := range fat {
		fat[i] = endOfChain
	}
	fat[0] = fatSector
	for _, entry := range entries {
		if entry.objectType != 2 || entry.size == 0 {
			continue
		}
		count := int((entry.size + sectorSize - 1) / sectorSize)
		for i := 0; i < count-1; i++ {
			fat[int(entry.startSector)+i] = entry.startSector + uint32(i) + 1
		}
	}
	for i := 0; i < len(sectors)-int(dirStart)-1; i++ {
		fat[int(dirStart)+i] = dirStart + uint32(i) + 1
	}
	for i, value := range fat {
		binary.LittleEndian.PutUint32(sectors[0][i*4:i*4+4], value)
	}
	for i := len(fat); i < sectorSize/4; i++ {
		binary.LittleEndian.PutUint32(sectors[0][i*4:i*4+4], noStream)
	}
	header := make([]byte, 512)
	copy(header[:8], []byte{0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1})
	binary.LittleEndian.PutUint16(header[24:26], 0x003E)
	binary.LittleEndian.PutUint16(header[26:28], 0x0003)
	binary.LittleEndian.PutUint16(header[28:30], 0xFFFE)
	binary.LittleEndian.PutUint16(header[30:32], 9)
	binary.LittleEndian.PutUint16(header[32:34], 6)
	binary.LittleEndian.PutUint32(header[44:48], 1)
	binary.LittleEndian.PutUint32(header[48:52], dirStart)
	binary.LittleEndian.PutUint32(header[56:60], 0)
	binary.LittleEndian.PutUint32(header[60:64], endOfChain)
	binary.LittleEndian.PutUint32(header[68:72], endOfChain)
	binary.LittleEndian.PutUint32(header[76:80], 0)
	for offset := 80; offset < 512; offset += 4 {
		binary.LittleEndian.PutUint32(header[offset:offset+4], noStream)
	}
	out := append([]byte{}, header...)
	for _, sector := range sectors {
		out = append(out, sector...)
	}
	return out
}

func cliDirectoryEntryForTest(entry cliCFBEntryForTest) []byte {
	const noStream = uint32(0xFFFFFFFF)
	out := make([]byte, 128)
	if entry.left == 0 {
		entry.left = noStream
	}
	if entry.right == 0 {
		entry.right = noStream
	}
	if entry.child == 0 {
		entry.child = noStream
	}
	nameBytes := cliUTF16BytesForTest(entry.name + "\x00")
	copy(out[:64], nameBytes)
	binary.LittleEndian.PutUint16(out[64:66], uint16(len(nameBytes)))
	out[66] = entry.objectType
	out[67] = 1
	binary.LittleEndian.PutUint32(out[68:72], entry.left)
	binary.LittleEndian.PutUint32(out[72:76], entry.right)
	binary.LittleEndian.PutUint32(out[76:80], entry.child)
	binary.LittleEndian.PutUint32(out[116:120], entry.startSector)
	binary.LittleEndian.PutUint32(out[120:124], uint32(entry.size))
	return out
}

func cliUTF16BytesForTest(text string) []byte {
	units := utf16.Encode([]rune(text))
	out := make([]byte, len(units)*2)
	for i, unit := range units {
		binary.LittleEndian.PutUint16(out[i*2:i*2+2], unit)
	}
	return out
}

func cliLE16ForTest(value uint16) []byte {
	out := make([]byte, 2)
	binary.LittleEndian.PutUint16(out, value)
	return out
}

func cliLE32ForTest(value uint32) []byte {
	out := make([]byte, 4)
	binary.LittleEndian.PutUint32(out, value)
	return out
}
