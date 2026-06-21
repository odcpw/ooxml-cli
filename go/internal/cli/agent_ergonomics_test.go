package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestGlobalJSONShortcutMatchesFormatJSON(t *testing.T) {
	workbookPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")

	formatOutput, err := executeRootForXLSXTest(t, "--format", "json", "inspect", workbookPath)
	if err != nil {
		t.Fatalf("--format json inspect failed: %v", err)
	}

	shortcutOutput, err := executeRootForXLSXTest(t, "--json", "inspect", workbookPath)
	if err != nil {
		t.Fatalf("--json inspect failed: %v", err)
	}

	if shortcutOutput != formatOutput {
		t.Fatalf("--json output differs from --format json\n--json:\n%s\n--format json:\n%s", shortcutOutput, formatOutput)
	}

	var decoded map[string]any
	if err := json.Unmarshal([]byte(shortcutOutput), &decoded); err != nil {
		t.Fatalf("--json output is not valid JSON: %v\n%s", err, shortcutOutput)
	}
}

func TestCapabilitiesJSONIsSelfDescribing(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}

	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("capabilities output is not valid JSON: %v\n%s", err, output)
	}

	if doc.Tool != "ooxml" {
		t.Fatalf("tool = %q, want ooxml", doc.Tool)
	}
	if doc.ContractVersion != capabilitiesContractVersion {
		t.Fatalf("contract version = %q, want %q", doc.ContractVersion, capabilitiesContractVersion)
	}
	if !hasCapabilityFlag(doc.GlobalFlags, "--json") {
		t.Fatalf("capabilities missing --json global flag")
	}
	if !hasCapabilityFlag(doc.GlobalFlags, "--format") {
		t.Fatalf("capabilities missing --format global flag")
	}
	for _, hidden := range []string{"--out", "--in-place", "--backup"} {
		if hasCapabilityFlag(doc.GlobalFlags, hidden) {
			t.Fatalf("capabilities should not expose hidden root mutation flag %s", hidden)
		}
	}
	for _, path := range []string{
		"ooxml pptx tables update-from-xlsx",
		"ooxml pptx charts update-data",
		"ooxml pptx xlsx-bindings plan",
		"ooxml pptx replace text-occurrences",
		"ooxml xlsx cells set",
		"ooxml xlsx names add",
		"ooxml xlsx charts list",
		"ooxml xlsx charts update-source",
		"ooxml xlsx pivots list",
		"ooxml vba list",
		"ooxml vba inspect",
		"ooxml vba create",
		"ooxml vba office-check",
		"ooxml robot-docs guide",
	} {
		if !hasCapabilityCommand(doc.Commands, path) {
			t.Fatalf("capabilities missing command path %q", path)
		}
	}
	if !hasExitCode(doc.ExitCodes, ExitInvalidArgs, "invalid_args") {
		t.Fatalf("capabilities missing invalid_args exit code")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json pptx tables update-from-xlsx") {
		t.Fatalf("capabilities missing PPTX table update workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json pptx clone-slide") {
		t.Fatalf("capabilities missing clone-slide workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json pptx replace text-occurrences") {
		t.Fatalf("capabilities missing deck-wide text occurrence workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json pptx charts update-data") {
		t.Fatalf("capabilities missing PPTX chart update workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json xlsx tables show workbook.xlsx --table Sales") {
		t.Fatalf("capabilities missing XLSX table discovery workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json xlsx names add") {
		t.Fatalf("capabilities missing XLSX defined-name mutation workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json xlsx charts list") {
		t.Fatalf("capabilities missing XLSX chart discovery workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json xlsx charts update-source") {
		t.Fatalf("capabilities missing XLSX chart update workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json xlsx pivots list") {
		t.Fatalf("capabilities missing XLSX pivot discovery workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json pptx xlsx-bindings apply") {
		t.Fatalf("capabilities missing PPTX/XLSX bindings workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json vba office-check") {
		t.Fatalf("capabilities missing VBA office-check workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "ooxml --json vba create workbook.xlsm") {
		t.Fatalf("capabilities missing VBA create workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "DeckImageBindings") {
		t.Fatalf("capabilities missing image binding workflow")
	}
	if !containsWorkflowCommand(doc.Workflows, "DeckBoundsBindings") {
		t.Fatalf("capabilities missing bounds binding workflow")
	}
	if containsWorkflowCommand(doc.Workflows, "--find") || containsWorkflowCommand(doc.Workflows, "--replace") {
		t.Fatalf("capabilities contains stale replace-text flags: %s", output)
	}
	notes := strings.Join(doc.Notes, "\n")
	for _, want := range []string{"vba create", "Windows desktop Office", "vba office-check", "local LibreOffice/soffice", "not Microsoft Office proof", "office-vba-smoke", "Office-shaped module-set changes are refused"} {
		if !strings.Contains(notes, want) {
			t.Fatalf("capabilities VBA notes missing %q: %s", want, notes)
		}
	}
	if strings.Contains(notes, "prove Office-load compatibility yet") {
		t.Fatalf("capabilities VBA notes contain stale office-check wording: %s", notes)
	}
}

func TestRobotDocsGuideTextMentionsCoreLoops(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "robot-docs", "guide")
	if err != nil {
		t.Fatalf("robot-docs guide failed: %v", err)
	}

	for _, want := range []string{
		"ooxml capabilities --json",
		"ooxml --json doctor",
		"ooxml --json doctor capabilities",
		"ooxml doctor robot-docs",
		"check-release-fast",
		"check-release-slow",
		"check-office-vba-schema",
		"check-office-vba-com",
		"-RunConformance",
		"-SkipOffice",
		"ooxml --json pptx tables show",
		"ooxml --json pptx clone-slide",
		"ooxml --json pptx replace text-occurrences",
		"ooxml --json pptx charts update-data",
		"ooxml --json xlsx tables show workbook.xlsx --table Sales",
		"ooxml --json pptx xlsx-bindings plan",
		"ooxml --json pptx place image",
		"ooxml --json pptx replace images",
		"DeckImageBindings",
		"ooxml --json pptx shapes set-bounds",
		"DeckBoundsBindings",
		"ooxml --json xlsx cells extract",
		"ooxml --json xlsx names add",
		"ooxml --json xlsx charts list",
		"ooxml --json xlsx pivots list",
		"ooxml --json vba inspect",
		"ooxml --json vba create workbook.xlsm",
		"ooxml --json vba extract workbook.xlsm --out-dir macros",
		"ooxml --json vba add-module source-only.xlsm --source macros/NewModule.bas",
		"ooxml --json vba replace-module workbook.xlsm --module Module1",
		"ooxml --json vba remove-module source-only-edited.xlsm --module Module1",
		"ooxml --json vba office-check target-with-vba.xlsm",
		"ooxml vba create can create fresh Office-authored XLSM/PPTM files",
		"local LibreOffice/soffice open-check evidence",
		"not Microsoft Office proof",
		"ooxml validate --strict",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("robot guide missing %q\n%s", want, output)
		}
	}
	for _, stale := range []string{"--find", "--replace", "prove Office-load compatibility yet"} {
		if strings.Contains(output, stale) {
			t.Fatalf("robot guide contains stale wording %q\n%s", stale, output)
		}
	}
}

func TestRobotDocsGuideJSON(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "robot-docs", "guide", "--json")
	if err != nil {
		t.Fatalf("robot-docs guide --json failed: %v", err)
	}
	aliasOutput, err := executeRootForXLSXTest(t, "agent", "guide", "--json")
	if err != nil {
		t.Fatalf("agent guide --json failed: %v", err)
	}
	if aliasOutput != output {
		t.Fatalf("agent guide alias output differs from robot-docs guide\nagent:\n%s\nrobot-docs:\n%s", aliasOutput, output)
	}

	var guide robotDocsGuide
	if err := json.Unmarshal([]byte(output), &guide); err != nil {
		t.Fatalf("robot guide output is not valid JSON: %v\n%s", err, output)
	}
	if guide.Tool != "ooxml" {
		t.Fatalf("guide tool = %q, want ooxml", guide.Tool)
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml agent guide") {
		t.Fatalf("robot guide missing agent guide alias command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json doctor capabilities") {
		t.Fatalf("robot guide missing doctor capabilities command")
	}
	if !containsRobotDocsCommand(guide.Sections, "make check-release-fast") {
		t.Fatalf("robot guide missing fast release gate")
	}
	if !containsRobotDocsCommand(guide.Sections, "make check-release-slow") {
		t.Fatalf("robot guide missing slow release gate")
	}
	if !containsRobotDocsCommand(guide.Sections, "make check-office-vba-schema") {
		t.Fatalf("robot guide missing VBA schema gate")
	}
	if !containsRobotDocsCommand(guide.Sections, "make check-office-vba-com") {
		t.Fatalf("robot guide missing VBA COM gate")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json xlsx tables append-rows") {
		t.Fatalf("robot guide missing xlsx append-rows command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json pptx replace text-occurrences") {
		t.Fatalf("robot guide missing PPTX text occurrences command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json pptx charts update-data") {
		t.Fatalf("robot guide missing PPTX chart update command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json xlsx names delete") {
		t.Fatalf("robot guide missing xlsx names delete command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json xlsx charts show") {
		t.Fatalf("robot guide missing xlsx charts show command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json xlsx pivots show") {
		t.Fatalf("robot guide missing xlsx pivots show command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json vba list") {
		t.Fatalf("robot guide missing vba list command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json vba create workbook.xlsm") {
		t.Fatalf("robot guide missing vba create command")
	}
	if !containsRobotDocsCommand(guide.Sections, "ooxml --json vba office-check") {
		t.Fatalf("robot guide missing vba office-check command")
	}
	if !containsRobotDocsCommand(guide.Sections, "DeckImageBindings") {
		t.Fatalf("robot guide missing image binding command")
	}
	if !containsRobotDocsCommand(guide.Sections, "DeckBoundsBindings") {
		t.Fatalf("robot guide missing bounds binding command")
	}
}

func TestSingularAliasesAndMisspelledSubcommands(t *testing.T) {
	presentationPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	workbookPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")

	for _, tc := range [][]string{
		{"--json", "pptx", "slide", "list", presentationPath},
		{"--json", "xlsx", "table", "list", workbookPath},
		{"--json", "xlsx", "pivot", "list", workbookPath},
	} {
		if _, err := executeRootForXLSXTest(t, tc...); err != nil {
			t.Fatalf("expected singular alias to work for %v: %v", tc, err)
		}
	}

	if _, err := executeRootForXLSXTest(t, "pptx", "replace", "image", "--help"); err != nil {
		t.Fatalf("expected singular image alias help to work: %v", err)
	}
	for _, tc := range [][]string{
		{"--json", "pptx", "slied", "list", presentationPath},
		{"--json", "xlsx", "taable", "list", workbookPath},
	} {
		if _, err := executeRootForXLSXTest(t, tc...); err == nil {
			t.Fatalf("expected misspelled command to fail for %v", tc)
		}
	}
}

func TestUnknownCommandErrorsSuggestCorrectedCommand(t *testing.T) {
	presentationPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	workbookPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")

	cases := []struct {
		name string
		args []string
		want []string
	}{
		{
			name: "pptx slides typo",
			args: []string{"--json", "pptx", "slied", "list", presentationPath},
			want: []string{
				`unknown command "slied" for "ooxml pptx"`,
				"did you mean: slides",
				"try: `ooxml pptx slides list",
				"discover with `ooxml pptx --help`",
			},
		},
		{
			name: "xlsx tables typo",
			args: []string{"--json", "xlsx", "taable", "list", workbookPath},
			want: []string{
				`unknown command "taable" for "ooxml xlsx"`,
				"did you mean: tables",
				"try: `ooxml xlsx tables list",
				"discover with `ooxml xlsx --help`",
			},
		},
		{
			name: "nested read command typo",
			args: []string{"--json", "xlsx", "cells", "extrct", workbookPath},
			want: []string{
				`unknown command "extrct" for "ooxml xlsx cells"`,
				"did you mean: extract",
				"try: `ooxml xlsx cells extract",
				"discover with `ooxml xlsx cells --help`",
			},
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := executeRootForXLSXTest(t, tc.args...)
			cliErr, ok := AsCLIError(err)
			if !ok {
				t.Fatalf("expected CLIError, got %T: %v", err, err)
			}
			if cliErr.ExitCode != ExitInvalidArgs {
				t.Fatalf("exitCode = %d, want %d", cliErr.ExitCode, ExitInvalidArgs)
			}
			for _, want := range tc.want {
				if !strings.Contains(cliErr.Message, want) {
					t.Fatalf("message missing %q:\n%s", want, cliErr.Message)
				}
			}
		})
	}
}

func TestUnknownFlagErrorsSuggestNearestFlag(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "xlsx", "types-and-formulas", "workbook.xlsx")

	_, err := executeRootForXLSXTest(t, "--json", "find", "Revenue", fixture, "--jsno")
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T: %v", err, err)
	}
	if cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("exitCode = %d, want %d", cliErr.ExitCode, ExitInvalidArgs)
	}
	for _, want := range []string{
		"unknown flag: --jsno",
		"did you mean: --json",
		"retry with `--json`",
		"discover with `ooxml find --help`",
	} {
		if !strings.Contains(cliErr.Message, want) {
			t.Fatalf("message missing %q:\n%s", want, cliErr.Message)
		}
	}
}

func TestPPTXHandleAwareHelpMentionsHandles(t *testing.T) {
	for _, tc := range []struct {
		name string
		args []string
		want []string
	}{
		{
			name: "replace text",
			args: []string{"pptx", "replace", "text", "--help"},
			want: []string{"H:pptx/s:<sldId>/shape:n:<id>", "supplies slide scope"},
		},
		{
			name: "replace images",
			args: []string{"pptx", "replace", "images", "--help"},
			want: []string{"H:pptx/s:<sldId>/shape:n:<id>", "Shape handles cannot be combined with --slide or --for-slides"},
		},
		{
			name: "animations add",
			args: []string{"pptx", "animations", "add", "--help"},
			want: []string{"H:pptx/s:<sldId>/shape:n:<id>", "required unless --shape is a stable shape handle"},
		},
	} {
		out, err := executeRootForXLSXTest(t, tc.args...)
		if err != nil {
			t.Fatalf("%s help failed: %v", tc.name, err)
		}
		for _, want := range tc.want {
			if !strings.Contains(out, want) {
				t.Fatalf("%s help missing %q:\n%s", tc.name, want, out)
			}
		}
	}
}

func TestErrorFormatDetectsJSONBeforePreRun(t *testing.T) {
	oldArgs := os.Args
	oldConfig := globalConfig
	t.Cleanup(func() {
		os.Args = oldArgs
		globalConfig = oldConfig
		resetFlags()
	})

	for _, tc := range [][]string{
		{"ooxml", "--json", "nope"},
		{"ooxml", "--format", "json", "nope"},
		{"ooxml", "-f", "json", "nope"},
		{"ooxml", "capabilities", "--format=json", "--bogus"},
	} {
		resetFlags()
		globalConfig = nil
		os.Args = tc
		if got := errorFormat(); got != "json" {
			t.Fatalf("errorFormat() = %q for args %v, want json", got, tc)
		}
	}
}

func TestVersionHonorsJSONShortcut(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "--json", "version")
	if err != nil {
		t.Fatalf("version --json failed: %v", err)
	}

	var version versionOutput
	if err := json.Unmarshal([]byte(output), &version); err != nil {
		t.Fatalf("version output is not valid JSON: %v\n%s", err, output)
	}
	if version.Tool != "ooxml" || version.Version != Version {
		t.Fatalf("version output = %+v, want tool=ooxml version=%s", version, Version)
	}
}

func TestPPTXTablesShowEmptyResultIsExplicit(t *testing.T) {
	presentationPath := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")

	textOutput, err := executeRootForXLSXTest(t, "pptx", "tables", "show", presentationPath, "--slide", "1")
	if err != nil {
		t.Fatalf("pptx tables show text failed: %v", err)
	}
	if !strings.Contains(textOutput, "No tables found on slide 1.") {
		t.Fatalf("empty table text output was not explicit:\n%s", textOutput)
	}

	jsonOutput, err := executeRootForXLSXTest(t, "--json", "pptx", "tables", "show", presentationPath, "--slide", "1")
	if err != nil {
		t.Fatalf("pptx tables show JSON failed: %v", err)
	}
	var result PPTXTablesShowResult
	if err := json.Unmarshal([]byte(jsonOutput), &result); err != nil {
		t.Fatalf("tables show output is not valid JSON: %v\n%s", err, jsonOutput)
	}
	if result.Tables == nil {
		t.Fatalf("empty JSON tables should be [], got nil in %s", jsonOutput)
	}
	if len(result.Tables) != 0 {
		t.Fatalf("tables length = %d, want 0", len(result.Tables))
	}
}

func hasCapabilityFlag(flags []capabilityFlag, name string) bool {
	for _, flag := range flags {
		if flag.Name == name {
			return true
		}
	}
	return false
}

func hasCapabilityCommand(commands []capabilityCommand, path string) bool {
	for _, command := range commands {
		if command.Path == path {
			return true
		}
	}
	return false
}

func hasExitCode(exitCodes []capabilityExitCode, code int, name string) bool {
	for _, exitCode := range exitCodes {
		if exitCode.Code == code && exitCode.Name == name {
			return true
		}
	}
	return false
}

func containsWorkflowCommand(workflows []capabilityWorkflow, fragment string) bool {
	for _, workflow := range workflows {
		for _, command := range workflow.Commands {
			if strings.Contains(command, fragment) {
				return true
			}
		}
	}
	return false
}

func containsRobotDocsCommand(sections []robotDocsSection, fragment string) bool {
	for _, section := range sections {
		for _, command := range section.Commands {
			if strings.Contains(command, fragment) {
				return true
			}
		}
	}
	return false
}
