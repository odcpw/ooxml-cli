package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXConditionalFormatsCommandRegistration(t *testing.T) {
	xlsx := findSubcommand(GetRootCmd(), "xlsx")
	if xlsx == nil {
		t.Fatal("xlsx command is not registered")
	}
	cf := findSubcommand(xlsx, "conditional-formats")
	if cf == nil {
		t.Fatal("xlsx conditional-formats command is not registered")
	}
	for _, name := range []string{"list", "show", "add", "delete", "reorder"} {
		if sub := findSubcommand(cf, name); sub == nil {
			t.Fatalf("xlsx conditional-formats %s command is not registered", name)
		}
	}
}

func TestXLSXConditionalFormatsAddListShowDelete(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf.xlsx")

	addOut, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "A1:A5",
		"--type", "expression",
		"--formula", "A1>0",
		"--priority", "7",
		"--stop-if-true",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats add failed: %v", err)
	}
	var addResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(addOut), &addResult); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, addOut)
	}
	if addResult.Action != "add" || addResult.Range != "A1:A5" || addResult.Rule == nil {
		t.Fatalf("unexpected add result: %+v", addResult)
	}
	if addResult.Rule.Type != "expression" || addResult.Rule.Formula != "A1>0" || addResult.Rule.Priority == nil || *addResult.Rule.Priority != 7 || !addResult.Rule.StopIfTrue {
		t.Fatalf("unexpected added rule: %+v", addResult.Rule)
	}
	if addResult.CellsAffected != 5 {
		t.Fatalf("cellsAffected = %d, want 5", addResult.CellsAffected)
	}
	if addResult.ConditionalFormatsListCommand == "" || !strings.Contains(addResult.ConditionalFormatsListCommand, "--json") {
		t.Fatalf("missing list readback command: %+v", addResult)
	}
	if addResult.ConditionalFormatsShowCommand == "" {
		t.Fatalf("missing show readback command: %+v", addResult)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsListCommand)
	var listResult XLSXConditionalFormatsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || len(listResult.ConditionalFormats) != 1 || listResult.Rules[0].PrimarySelector != "cfRule:1" {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	if !containsString(listResult.Rules[0].Selectors, "priority:7") {
		t.Fatalf("missing priority selector: %+v", listResult.Rules[0])
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsShowCommand)
	var showResult XLSXConditionalFormatRuleJSON
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOut)
	}
	if showResult.Sqref != "A1:A5" || showResult.Formula != "A1>0" {
		t.Fatalf("unexpected show result: %+v", showResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}

	deletedPath := filepath.Join(t.TempDir(), "cf-deleted.xlsx")
	deleteOut, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "conditional-formats", "delete", outPath,
		"--sheet", "1",
		"--rule", "priority:7",
		"--out", deletedPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats delete failed: %v", err)
	}
	var deleteResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(deleteOut), &deleteResult); err != nil {
		t.Fatalf("failed to unmarshal delete JSON: %v\n%s", err, deleteOut)
	}
	if deleteResult.Action != "delete" || deleteResult.Rule == nil || deleteResult.Rule.Formula != "A1>0" {
		t.Fatalf("unexpected delete result: %+v", deleteResult)
	}
	listOut, err = executeRootForXLSXTest(t, "--format", "json", "xlsx", "conditional-formats", "list", deletedPath, "--sheet", "1")
	if err != nil {
		t.Fatalf("conditional-formats list after delete failed: %v", err)
	}
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal post-delete list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 0 || len(listResult.ConditionalFormats) != 0 {
		t.Fatalf("expected no conditional formats after delete: %+v", listResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", deletedPath); err != nil {
		t.Fatalf("validate --strict after delete failed: %v", err)
	}
}

func TestXLSXConditionalFormatsReorder(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	dir := t.TempDir()
	step1 := filepath.Join(dir, "cf-1.xlsx")
	step2 := filepath.Join(dir, "cf-2.xlsx")
	step3 := filepath.Join(dir, "cf-3.xlsx")

	if _, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "A1:A5",
		"--type", "expression",
		"--formula", "A1>0",
		"--priority", "3",
		"--out", step1,
	); err != nil {
		t.Fatalf("conditional-formats add step1 failed: %v", err)
	}
	if _, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "add", step1,
		"--sheet", "1",
		"--range", "B1:B5",
		"--type", "expression",
		"--formula", "B1>0",
		"--priority", "1",
		"--out", step2,
	); err != nil {
		t.Fatalf("conditional-formats add step2 failed: %v", err)
	}
	if _, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "add", step2,
		"--sheet", "1",
		"--range", "C1:C5",
		"--type", "expression",
		"--formula", "C1>0",
		"--priority", "2",
		"--out", step3,
	); err != nil {
		t.Fatalf("conditional-formats add step3 failed: %v", err)
	}

	reorderedPath := filepath.Join(dir, "cf-reordered.xlsx")
	reorderOut, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "reorder", step3,
		"--sheet", "1",
		"--rule", "priority:2",
		"--priority", "1",
		"--out", reorderedPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats reorder failed: %v", err)
	}
	var reorderResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(reorderOut), &reorderResult); err != nil {
		t.Fatalf("failed to unmarshal reorder JSON: %v\n%s", err, reorderOut)
	}
	if reorderResult.Action != "reorder" || reorderResult.Rule == nil {
		t.Fatalf("unexpected reorder result: %+v", reorderResult)
	}
	if reorderResult.OldPriority == nil || *reorderResult.OldPriority != 2 || reorderResult.NewPriority == nil || *reorderResult.NewPriority != 1 {
		t.Fatalf("unexpected old/new priority readback: %+v", reorderResult)
	}
	if reorderResult.Rule.Formula != "C1>0" || reorderResult.Rule.Priority == nil || *reorderResult.Rule.Priority != 1 {
		t.Fatalf("unexpected reordered rule: %+v", reorderResult.Rule)
	}
	if reorderResult.ConditionalFormatsListCommand == "" || reorderResult.ConditionalFormatsShowCommand == "" || reorderResult.ValidateCommand == "" {
		t.Fatalf("missing readback commands: %+v", reorderResult)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, reorderResult.ConditionalFormatsListCommand)
	var listResult XLSXConditionalFormatsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 3 {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	prioritiesByFormula := map[string]int{}
	for _, rule := range listResult.Rules {
		if rule.Priority == nil {
			t.Fatalf("rule missing priority after reorder: %+v", rule)
		}
		prioritiesByFormula[rule.Formula] = *rule.Priority
	}
	if prioritiesByFormula["C1>0"] != 1 || prioritiesByFormula["B1>0"] != 2 || prioritiesByFormula["A1>0"] != 3 {
		t.Fatalf("unexpected priorities by formula: %+v", prioritiesByFormula)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, reorderResult.ConditionalFormatsShowCommand)
	var showResult XLSXConditionalFormatRuleJSON
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOut)
	}
	if showResult.Formula != "C1>0" || showResult.Priority == nil || *showResult.Priority != 1 {
		t.Fatalf("unexpected show result after reorder: %+v", showResult)
	}
	executeGeneratedOOXMLCommandForXLSXTest(t, reorderResult.ValidateCommand)
}

func TestXLSXConditionalFormatsReorderPriorityValidation(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	dir := t.TempDir()
	seededPath := filepath.Join(dir, "cf-seeded.xlsx")
	if _, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "A1:A5",
		"--type", "expression",
		"--formula", "A1>0",
		"--priority", "1",
		"--out", seededPath,
	); err != nil {
		t.Fatalf("conditional-formats add seed failed: %v", err)
	}

	_, err := executeRootForXLSXTest(t,
		"xlsx", "conditional-formats", "reorder", seededPath,
		"--sheet", "1",
		"--rule", "cfRule:1",
		"--out", filepath.Join(dir, "missing-priority.xlsx"),
	)
	if err == nil || !strings.Contains(err.Error(), "--priority is required") {
		t.Fatalf("expected missing priority error, got: %v", err)
	}

	_, err = executeRootForXLSXTest(t,
		"xlsx", "conditional-formats", "reorder", seededPath,
		"--sheet", "1",
		"--rule", "cfRule:1",
		"--priority", "0",
		"--out", filepath.Join(dir, "zero-priority.xlsx"),
	)
	if err == nil || !strings.Contains(err.Error(), "--priority must be greater than zero") {
		t.Fatalf("expected zero priority error, got: %v", err)
	}

	_, err = executeRootForXLSXTest(t,
		"xlsx", "conditional-formats", "reorder", seededPath,
		"--sheet", "1",
		"--rule", "cfRule:1",
		"--priority", "2",
		"--out", filepath.Join(dir, "too-large-priority.xlsx"),
	)
	if err == nil || !strings.Contains(err.Error(), "--priority must be between 1 and 1") {
		t.Fatalf("expected too-large priority error, got: %v", err)
	}
}

func TestXLSXConditionalFormatsAddCellIs(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-cell-is.xlsx")

	addOut, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "B1:B5",
		"--type", "cell-is",
		"--operator", "between",
		"--formula", "1",
		"--formula2", "10",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats add cell-is failed: %v", err)
	}
	var addResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(addOut), &addResult); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, addOut)
	}
	if addResult.Rule == nil || addResult.Rule.Type != "cellIs" || addResult.Rule.Operator != "between" {
		t.Fatalf("unexpected added rule: %+v", addResult.Rule)
	}
	if addResult.Rule.Formula != "1" || len(addResult.Rule.Formulas) != 2 || addResult.Rule.Formulas[1] != "10" {
		t.Fatalf("unexpected cellIs formulas: %+v", addResult.Rule)
	}

	listOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "conditional-formats", "list", outPath, "--sheet", "1")
	if err != nil {
		t.Fatalf("conditional-formats list failed: %v", err)
	}
	var listResult XLSXConditionalFormatsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || listResult.Rules[0].Type != "cellIs" || listResult.Rules[0].Operator != "between" {
		t.Fatalf("unexpected list result: %+v", listResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXConditionalFormatsAddColorScale(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-color-scale.xlsx")

	addOut, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "C1:C5",
		"--type", "color-scale",
		"--cfvo", "min",
		"--cfvo", "percentile:50",
		"--cfvo", "max",
		"--color", "F8696B",
		"--color", "FFEB84",
		"--color", "63BE7B",
		"--priority", "4",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats add color-scale failed: %v", err)
	}
	var addResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(addOut), &addResult); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, addOut)
	}
	if addResult.Rule == nil || addResult.Rule.Type != "colorScale" || addResult.Rule.ColorScale == nil {
		t.Fatalf("unexpected added rule: %+v", addResult.Rule)
	}
	scale := addResult.Rule.ColorScale
	if len(scale.CFVO) != 3 || scale.CFVO[1].Type != "percentile" || scale.CFVO[1].Value != "50" {
		t.Fatalf("unexpected color-scale cfvo readback: %+v", scale.CFVO)
	}
	if len(scale.Colors) != 3 || scale.Colors[0].RGB != "FFF8696B" || scale.Colors[1].RGB != "FFFFEB84" || scale.Colors[2].RGB != "FF63BE7B" {
		t.Fatalf("unexpected color-scale colors readback: %+v", scale.Colors)
	}
	if addResult.CellsAffected != 5 || addResult.ConditionalFormatsShowCommand == "" {
		t.Fatalf("unexpected mutation result: %+v", addResult)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsListCommand)
	var listResult XLSXConditionalFormatsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || listResult.Rules[0].Type != "colorScale" || listResult.Rules[0].ColorScale == nil {
		t.Fatalf("unexpected list result: %+v", listResult)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsShowCommand)
	var showResult XLSXConditionalFormatRuleJSON
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOut)
	}
	if showResult.ColorScale == nil || len(showResult.ColorScale.CFVO) != 3 || showResult.ColorScale.Colors[2].RGB != "FF63BE7B" {
		t.Fatalf("unexpected show result: %+v", showResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXConditionalFormatsAddDataBar(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-data-bar.xlsx")

	addOut, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "D1:D5",
		"--type", "data-bar",
		"--cfvo", "min",
		"--cfvo", "max",
		"--color", "638EC6",
		"--priority", "7",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats add data-bar failed: %v", err)
	}
	var addResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(addOut), &addResult); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, addOut)
	}
	if addResult.Rule == nil || addResult.Rule.Type != "dataBar" || addResult.Rule.DataBar == nil {
		t.Fatalf("unexpected added rule: %+v", addResult.Rule)
	}
	bar := addResult.Rule.DataBar
	if len(bar.CFVO) != 2 || bar.CFVO[0].Type != "min" || bar.CFVO[1].Type != "max" {
		t.Fatalf("unexpected data-bar cfvo readback: %+v", bar.CFVO)
	}
	if bar.Color.RGB != "FF638EC6" {
		t.Fatalf("unexpected data-bar color readback: %+v", bar.Color)
	}
	if addResult.CellsAffected != 5 || addResult.ConditionalFormatsShowCommand == "" {
		t.Fatalf("unexpected mutation result: %+v", addResult)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsListCommand)
	var listResult XLSXConditionalFormatsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || listResult.Rules[0].Type != "dataBar" || listResult.Rules[0].DataBar == nil {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	if listResult.Rules[0].DataBar.Color.RGB != "FF638EC6" {
		t.Fatalf("unexpected list dataBar color: %+v", listResult.Rules[0].DataBar)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsShowCommand)
	var showResult XLSXConditionalFormatRuleJSON
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOut)
	}
	if showResult.DataBar == nil || len(showResult.DataBar.CFVO) != 2 || showResult.DataBar.Color.RGB != "FF638EC6" {
		t.Fatalf("unexpected show result: %+v", showResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXConditionalFormatsAddIconSet(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-icon-set.xlsx")

	addOut, err := executeRootForXLSXTest(t,
		"--json",
		"xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1",
		"--range", "E1:E5",
		"--type", "icon-set",
		"--icon-set", "3TrafficLights1",
		"--cfvo", "percent:0",
		"--cfvo", "percent:33",
		"--cfvo", "percent:67",
		"--priority", "8",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("conditional-formats add icon-set failed: %v", err)
	}
	if strings.Contains(addOut, `"showValue"`) || strings.Contains(addOut, `"percent":`) || strings.Contains(addOut, `"reverse"`) {
		t.Fatalf("new icon-set JSON should omit optional booleans:\n%s", addOut)
	}
	var addResult XLSXConditionalFormatMutationResult
	if err := json.Unmarshal([]byte(addOut), &addResult); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, addOut)
	}
	if addResult.Rule == nil || addResult.Rule.Type != "iconSet" || addResult.Rule.IconSet == nil {
		t.Fatalf("unexpected added rule: %+v", addResult.Rule)
	}
	icons := addResult.Rule.IconSet
	if icons.IconSet != "3TrafficLights1" {
		t.Fatalf("iconSet = %q, want 3TrafficLights1", icons.IconSet)
	}
	if len(icons.CFVO) != 3 || icons.CFVO[0].Type != "percent" || icons.CFVO[0].Value != "0" || icons.CFVO[2].Value != "67" {
		t.Fatalf("unexpected icon-set cfvo readback: %+v", icons.CFVO)
	}
	if addResult.Rule.Priority == nil || *addResult.Rule.Priority != 8 {
		t.Fatalf("unexpected icon-set priority: %+v", addResult.Rule)
	}
	if addResult.CellsAffected != 5 || addResult.ConditionalFormatsShowCommand == "" {
		t.Fatalf("unexpected mutation result: %+v", addResult)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsListCommand)
	var listResult XLSXConditionalFormatsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || listResult.Rules[0].Type != "iconSet" || listResult.Rules[0].IconSet == nil {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	if listResult.Rules[0].IconSet.IconSet != "3TrafficLights1" || len(listResult.Rules[0].IconSet.CFVO) != 3 {
		t.Fatalf("unexpected list iconSet readback: %+v", listResult.Rules[0].IconSet)
	}

	showOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.ConditionalFormatsShowCommand)
	var showResult XLSXConditionalFormatRuleJSON
	if err := json.Unmarshal([]byte(showOut), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOut)
	}
	if showResult.IconSet == nil || showResult.IconSet.IconSet != "3TrafficLights1" || showResult.IconSet.CFVO[1].Value != "33" {
		t.Fatalf("unexpected show result: %+v", showResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}
}

func TestXLSXConditionalFormatsAddCellIsValidation(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-cell-is.xlsx")

	_, err := executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "cell-is", "--formula", "1", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--operator is required") {
		t.Fatalf("expected missing operator error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "cell-is", "--operator", "between", "--formula", "1", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "requires --formula2") {
		t.Fatalf("expected missing formula2 error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "cell-is", "--operator", "greaterThan", "--formula", "1", "--formula2", "10", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--formula2 is only valid") {
		t.Fatalf("expected non-between formula2 error, got: %v", err)
	}
}

func TestXLSXConditionalFormatsAddColorScaleValidation(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-color-scale.xlsx")

	_, err := executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "color-scale",
		"--color", "FF0000", "--color", "00FF00", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "exactly 2 or 3 --cfvo") {
		t.Fatalf("expected missing cfvo error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "color-scale",
		"--cfvo", "min", "--cfvo", "max", "--color", "FF0000", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "same number of --color and --cfvo") {
		t.Fatalf("expected mismatched color error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "color-scale",
		"--cfvo", "formula:A1", "--cfvo", "max", "--color", "FF0000", "--color", "00FF00", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "invalid --cfvo type") {
		t.Fatalf("expected invalid cfvo type error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--type", "color-scale",
		"--cfvo", "min", "--cfvo", "max", "--color", "FF0000", "--color", "00FF00", "--formula", "A1>0", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--formula and --formula2 are not valid") {
		t.Fatalf("expected formula rejected error, got: %v", err)
	}
}

func TestXLSXConditionalFormatsAddDataBarValidation(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-data-bar.xlsx")

	_, err := executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "D1:D5", "--type", "data-bar",
		"--cfvo", "min", "--color", "638EC6", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "exactly 2 --cfvo") {
		t.Fatalf("expected missing cfvo error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "D1:D5", "--type", "data-bar",
		"--cfvo", "min", "--cfvo", "max", "--color", "638EC6", "--color", "FF0000", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "exactly 1 --color") {
		t.Fatalf("expected extra color error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "D1:D5", "--type", "data-bar",
		"--cfvo", "min", "--cfvo", "max", "--color", "638EC6", "--formula", "D1>0", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--formula and --formula2 are not valid with --type data-bar") {
		t.Fatalf("expected formula rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "D1:D5", "--type", "data-bar",
		"--cfvo", "min", "--cfvo", "max", "--color", "638EC6", "--operator", "greaterThan", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--operator is only valid with --type cell-is") {
		t.Fatalf("expected operator rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "D1:D5", "--type", "data-bar",
		"--cfvo", "min", "--cfvo", "max", "--color", "638EC6", "--stop-if-true", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--stop-if-true is not valid with --type data-bar") {
		t.Fatalf("expected stop-if-true rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "D1:D5", "--type", "data-bar",
		"--cfvo", "min", "--cfvo", "max", "--color", "638EC6", "--dxf-id", "0", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--dxf-id is not valid with --type data-bar") {
		t.Fatalf("expected dxf-id rejected error, got: %v", err)
	}
}

func TestXLSXConditionalFormatsAddIconSetValidation(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf-icon-set.xlsx")

	_, err := executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--icon-set is required") {
		t.Fatalf("expected missing icon-set error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "4Arrows",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "exactly 4 --cfvo") {
		t.Fatalf("expected wrong cfvo count error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "2TrafficLights",
		"--cfvo", "percent:0", "--cfvo", "percent:50", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "must start with 3, 4, or 5") {
		t.Fatalf("expected icon-set name error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "3TrafficLights1",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--color", "FF0000", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--color is not valid with --type icon-set") {
		t.Fatalf("expected color rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "3TrafficLights1",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--formula", "E1>0", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--formula and --formula2 are not valid with --type icon-set") {
		t.Fatalf("expected formula rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "3TrafficLights1",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--operator", "greaterThan", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--operator is only valid with --type cell-is") {
		t.Fatalf("expected operator rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "3TrafficLights1",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--stop-if-true", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--stop-if-true is not valid with --type icon-set") {
		t.Fatalf("expected stop-if-true rejected error, got: %v", err)
	}
	_, err = executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "E1:E5", "--type", "icon-set", "--icon-set", "3TrafficLights1",
		"--cfvo", "percent:0", "--cfvo", "percent:33", "--cfvo", "percent:67", "--dxf-id", "0", "--out", outPath)
	if err == nil || !strings.Contains(err.Error(), "--dxf-id is not valid with --type icon-set") {
		t.Fatalf("expected dxf-id rejected error, got: %v", err)
	}
}

func TestXLSXConditionalFormatsShowNotFound(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "cf.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "conditional-formats", "add", workbookPath,
		"--sheet", "1", "--range", "A1:A5", "--formula", "A1>0", "--out", outPath); err != nil {
		t.Fatalf("conditional-formats add failed: %v", err)
	}
	_, err := executeRootForXLSXTest(t, "xlsx", "conditional-formats", "show", outPath, "--sheet", "1", "--rule", "cfRule:99")
	if err == nil {
		t.Fatalf("expected not-found error")
	}
	if !strings.Contains(err.Error(), "did you mean: cfRule:1") || !strings.Contains(err.Error(), "ooxml --json xlsx conditional-formats list <file> --sheet sheetId:1") {
		t.Fatalf("expected selector candidates and discovery command, got: %v", err)
	}
}
