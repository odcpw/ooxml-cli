package cli

import (
	"strings"
	"testing"
)

func assertXLSXMutationSavedCommandsForTest(t *testing.T, commands XLSXMutationReadbackCommands, outPath, rangeRef string) (string, string) {
	t.Helper()

	if commands.ValidateCommand == "" || commands.CellsExtractCommand == "" || commands.RangesExportCommand == "" {
		t.Fatalf("saved XLSX mutation commands are incomplete: %+v", commands)
	}
	if commands.ValidateCommandTemplate != "" || commands.CellsExtractCommandTemplate != "" || commands.RangesExportCommandTemplate != "" {
		t.Fatalf("saved XLSX mutation result should not include templates: %+v", commands)
	}
	if !strings.Contains(commands.ValidateCommand, "validate --strict") || !strings.Contains(commands.ValidateCommand, outPath) {
		t.Fatalf("unexpected validateCommand: %s", commands.ValidateCommand)
	}
	if !strings.Contains(commands.CellsExtractCommand, "xlsx cells extract") || !strings.Contains(commands.CellsExtractCommand, outPath) || !strings.Contains(commands.CellsExtractCommand, rangeRef) || !strings.Contains(commands.CellsExtractCommand, "--include-empty") {
		t.Fatalf("unexpected cellsExtractCommand: %s", commands.CellsExtractCommand)
	}
	if !strings.Contains(commands.RangesExportCommand, "xlsx ranges export") || !strings.Contains(commands.RangesExportCommand, outPath) || !strings.Contains(commands.RangesExportCommand, rangeRef) || !strings.Contains(commands.RangesExportCommand, "--include-types") || !strings.Contains(commands.RangesExportCommand, "--include-formulas") || !strings.Contains(commands.RangesExportCommand, "--include-formats") {
		t.Fatalf("unexpected rangesExportCommand: %s", commands.RangesExportCommand)
	}

	executeGeneratedOOXMLCommandForXLSXTest(t, commands.ValidateCommand)
	cellsOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.CellsExtractCommand)
	if strings.TrimSpace(cellsOutput) == "" {
		t.Fatalf("generated cellsExtractCommand returned empty output: %s", commands.CellsExtractCommand)
	}
	rangesOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.RangesExportCommand)
	if strings.TrimSpace(rangesOutput) == "" {
		t.Fatalf("generated rangesExportCommand returned empty output: %s", commands.RangesExportCommand)
	}
	return cellsOutput, rangesOutput
}

func assertXLSXMutationDryRunTemplatesForTest(t *testing.T, commands XLSXMutationReadbackCommands, rangeRef string) {
	t.Helper()

	if commands.ValidateCommand != "" || commands.CellsExtractCommand != "" || commands.RangesExportCommand != "" {
		t.Fatalf("dry-run XLSX mutation result should not include saved-output commands: %+v", commands)
	}
	for label, command := range map[string]string{
		"validate":      commands.ValidateCommandTemplate,
		"cells extract": commands.CellsExtractCommandTemplate,
		"ranges export": commands.RangesExportCommandTemplate,
	} {
		if command == "" || !strings.Contains(command, "<out.xlsx>") {
			t.Fatalf("%s template missing output placeholder: %s", label, command)
		}
	}
	if !strings.Contains(commands.ValidateCommandTemplate, "validate --strict") {
		t.Fatalf("unexpected validateCommandTemplate: %s", commands.ValidateCommandTemplate)
	}
	if !strings.Contains(commands.CellsExtractCommandTemplate, "xlsx cells extract") || !strings.Contains(commands.CellsExtractCommandTemplate, "--include-empty") {
		t.Fatalf("unexpected cellsExtractCommandTemplate: %s", commands.CellsExtractCommandTemplate)
	}
	if !strings.Contains(commands.CellsExtractCommandTemplate, rangeRef) {
		t.Fatalf("cellsExtractCommandTemplate missing range %s: %s", rangeRef, commands.CellsExtractCommandTemplate)
	}
	if !strings.Contains(commands.RangesExportCommandTemplate, "xlsx ranges export") || !strings.Contains(commands.RangesExportCommandTemplate, "--include-types") || !strings.Contains(commands.RangesExportCommandTemplate, "--include-formulas") || !strings.Contains(commands.RangesExportCommandTemplate, "--include-formats") {
		t.Fatalf("unexpected rangesExportCommandTemplate: %s", commands.RangesExportCommandTemplate)
	}
	if !strings.Contains(commands.RangesExportCommandTemplate, rangeRef) {
		t.Fatalf("rangesExportCommandTemplate missing range %s: %s", rangeRef, commands.RangesExportCommandTemplate)
	}
}

func assertXLSXTableAppendSavedCommandsForTest(t *testing.T, commands XLSXTableAppendReadbackCommands, outPath string) (string, string) {
	t.Helper()

	if commands.TableShowCommand == "" || commands.TableExportCommand == "" {
		t.Fatalf("saved XLSX table append commands are incomplete: %+v", commands)
	}
	if commands.TableShowCommandTemplate != "" || commands.TableExportCommandTemplate != "" {
		t.Fatalf("saved XLSX table append result should not include templates: %+v", commands)
	}
	if !strings.Contains(commands.TableShowCommand, "xlsx tables show") || !strings.Contains(commands.TableShowCommand, outPath) || !strings.Contains(commands.TableShowCommand, "tableId:1") {
		t.Fatalf("unexpected tableShowCommand: %s", commands.TableShowCommand)
	}
	if !strings.Contains(commands.TableExportCommand, "xlsx tables export") || !strings.Contains(commands.TableExportCommand, outPath) || !strings.Contains(commands.TableExportCommand, "tableId:1") || !strings.Contains(commands.TableExportCommand, "--include-types") || !strings.Contains(commands.TableExportCommand, "--include-formulas") {
		t.Fatalf("unexpected tableExportCommand: %s", commands.TableExportCommand)
	}
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.TableShowCommand)
	if strings.TrimSpace(showOutput) == "" {
		t.Fatalf("generated tableShowCommand returned empty output: %s", commands.TableShowCommand)
	}
	exportOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.TableExportCommand)
	if strings.TrimSpace(exportOutput) == "" {
		t.Fatalf("generated tableExportCommand returned empty output: %s", commands.TableExportCommand)
	}
	return showOutput, exportOutput
}

func assertXLSXTableAppendDryRunTemplatesForTest(t *testing.T, commands XLSXTableAppendReadbackCommands) {
	t.Helper()

	if commands.TableShowCommand != "" || commands.TableExportCommand != "" {
		t.Fatalf("dry-run XLSX table append result should not include saved-output commands: %+v", commands)
	}
	if commands.TableShowCommandTemplate == "" || commands.TableExportCommandTemplate == "" {
		t.Fatalf("dry-run XLSX table append templates are incomplete: %+v", commands)
	}
	if !strings.Contains(commands.TableShowCommandTemplate, "<out.xlsx>") || !strings.Contains(commands.TableShowCommandTemplate, "xlsx tables show") || !strings.Contains(commands.TableShowCommandTemplate, "tableId:1") {
		t.Fatalf("unexpected tableShowCommandTemplate: %s", commands.TableShowCommandTemplate)
	}
	if !strings.Contains(commands.TableExportCommandTemplate, "<out.xlsx>") || !strings.Contains(commands.TableExportCommandTemplate, "xlsx tables export") || !strings.Contains(commands.TableExportCommandTemplate, "tableId:1") || !strings.Contains(commands.TableExportCommandTemplate, "--include-types") || !strings.Contains(commands.TableExportCommandTemplate, "--include-formulas") {
		t.Fatalf("unexpected tableExportCommandTemplate: %s", commands.TableExportCommandTemplate)
	}
}

func assertXLSXNameSavedCommandsForTest(t *testing.T, commands XLSXNameMutationReadbackCommands, outPath, name string) (string, string) {
	t.Helper()

	if commands.ValidateCommand == "" || commands.NamesListCommand == "" || commands.NameShowCommand == "" {
		t.Fatalf("saved XLSX name mutation commands are incomplete: %+v", commands)
	}
	if commands.ValidateCommandTemplate != "" || commands.NamesListCommandTemplate != "" || commands.NameShowCommandTemplate != "" {
		t.Fatalf("saved XLSX name mutation result should not include templates: %+v", commands)
	}
	if !strings.Contains(commands.ValidateCommand, "validate --strict") || !strings.Contains(commands.ValidateCommand, outPath) {
		t.Fatalf("unexpected validateCommand: %s", commands.ValidateCommand)
	}
	if !strings.Contains(commands.NamesListCommand, "xlsx names list") || !strings.Contains(commands.NamesListCommand, outPath) {
		t.Fatalf("unexpected namesListCommand: %s", commands.NamesListCommand)
	}
	if !strings.Contains(commands.NameShowCommand, "xlsx names show") || !strings.Contains(commands.NameShowCommand, outPath) || !strings.Contains(commands.NameShowCommand, name) {
		t.Fatalf("unexpected nameShowCommand: %s", commands.NameShowCommand)
	}

	executeGeneratedOOXMLCommandForXLSXTest(t, commands.ValidateCommand)
	listOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.NamesListCommand)
	if strings.TrimSpace(listOutput) == "" {
		t.Fatalf("generated namesListCommand returned empty output: %s", commands.NamesListCommand)
	}
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.NameShowCommand)
	if strings.TrimSpace(showOutput) == "" {
		t.Fatalf("generated nameShowCommand returned empty output: %s", commands.NameShowCommand)
	}
	return listOutput, showOutput
}

func assertXLSXSheetsMutationSavedCommandsForTest(t *testing.T, commands XLSXSheetsMutationReadbackCommands, outPath string, wantSheetShow bool) (string, string) {
	t.Helper()

	if commands.ValidateCommand == "" || commands.SheetsListCommand == "" {
		t.Fatalf("saved XLSX sheets mutation commands are incomplete: %+v", commands)
	}
	if commands.ValidateCommandTemplate != "" || commands.SheetsListCommandTemplate != "" || commands.SheetShowCommandTemplate != "" {
		t.Fatalf("saved XLSX sheets mutation result should not include templates: %+v", commands)
	}
	if !strings.Contains(commands.ValidateCommand, "validate --strict") || !strings.Contains(commands.ValidateCommand, outPath) {
		t.Fatalf("unexpected validateCommand: %s", commands.ValidateCommand)
	}
	if !strings.Contains(commands.SheetsListCommand, "xlsx sheets list") || !strings.Contains(commands.SheetsListCommand, outPath) {
		t.Fatalf("unexpected sheetsListCommand: %s", commands.SheetsListCommand)
	}
	if wantSheetShow {
		if commands.SheetShowCommand == "" || !strings.Contains(commands.SheetShowCommand, "xlsx sheets show") || !strings.Contains(commands.SheetShowCommand, outPath) {
			t.Fatalf("unexpected sheetShowCommand: %s", commands.SheetShowCommand)
		}
	} else if commands.SheetShowCommand != "" {
		t.Fatalf("sheetShowCommand should be empty: %s", commands.SheetShowCommand)
	}

	executeGeneratedOOXMLCommandForXLSXTest(t, commands.ValidateCommand)
	listOutput := executeGeneratedOOXMLCommandForXLSXTest(t, commands.SheetsListCommand)
	if strings.TrimSpace(listOutput) == "" {
		t.Fatalf("generated sheetsListCommand returned empty output: %s", commands.SheetsListCommand)
	}
	var showOutput string
	if wantSheetShow {
		showOutput = executeGeneratedOOXMLCommandForXLSXTest(t, commands.SheetShowCommand)
		if strings.TrimSpace(showOutput) == "" {
			t.Fatalf("generated sheetShowCommand returned empty output: %s", commands.SheetShowCommand)
		}
	}
	return listOutput, showOutput
}

func assertXLSXSheetsMutationDryRunTemplatesForTest(t *testing.T, commands XLSXSheetsMutationReadbackCommands, wantSheetShow bool) {
	t.Helper()

	if commands.ValidateCommand != "" || commands.SheetsListCommand != "" || commands.SheetShowCommand != "" {
		t.Fatalf("dry-run XLSX sheets mutation result should not include saved-output commands: %+v", commands)
	}
	if commands.ValidateCommandTemplate == "" || commands.SheetsListCommandTemplate == "" {
		t.Fatalf("dry-run XLSX sheets mutation templates are incomplete: %+v", commands)
	}
	if !strings.Contains(commands.ValidateCommandTemplate, "validate --strict") || !strings.Contains(commands.ValidateCommandTemplate, "<out.xlsx>") {
		t.Fatalf("unexpected validateCommandTemplate: %s", commands.ValidateCommandTemplate)
	}
	if !strings.Contains(commands.SheetsListCommandTemplate, "xlsx sheets list") || !strings.Contains(commands.SheetsListCommandTemplate, "<out.xlsx>") {
		t.Fatalf("unexpected sheetsListCommandTemplate: %s", commands.SheetsListCommandTemplate)
	}
	if wantSheetShow {
		if commands.SheetShowCommandTemplate == "" || !strings.Contains(commands.SheetShowCommandTemplate, "xlsx sheets show") || !strings.Contains(commands.SheetShowCommandTemplate, "<out.xlsx>") {
			t.Fatalf("unexpected sheetShowCommandTemplate: %s", commands.SheetShowCommandTemplate)
		}
	} else if commands.SheetShowCommandTemplate != "" {
		t.Fatalf("sheetShowCommandTemplate should be empty: %s", commands.SheetShowCommandTemplate)
	}
}
