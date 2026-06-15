package cli

import (
	"archive/zip"
	"bytes"
	"encoding/csv"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

func TestXLSXCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()

	xlsx := findSubcommand(cmd, "xlsx")
	if xlsx == nil {
		t.Fatal("xlsx command is not registered")
	}

	sheets := findSubcommand(xlsx, "sheets")
	if sheets == nil {
		t.Fatal("xlsx sheets command is not registered")
	}

	if list := findSubcommand(sheets, "list"); list == nil {
		t.Fatal("xlsx sheets list command is not registered")
	}
	if show := findSubcommand(sheets, "show"); show == nil {
		t.Fatal("xlsx sheets show command is not registered")
	}
	if add := findSubcommand(sheets, "add"); add == nil {
		t.Fatal("xlsx sheets add command is not registered")
	}
	if rename := findSubcommand(sheets, "rename"); rename == nil {
		t.Fatal("xlsx sheets rename command is not registered")
	}
	if move := findSubcommand(sheets, "move"); move == nil {
		t.Fatal("xlsx sheets move command is not registered")
	}
	if deleteCmd := findSubcommand(sheets, "delete"); deleteCmd == nil {
		t.Fatal("xlsx sheets delete command is not registered")
	}

	cells := findSubcommand(xlsx, "cells")
	if cells == nil {
		t.Fatal("xlsx cells command is not registered")
	}
	if extract := findSubcommand(cells, "extract"); extract == nil {
		t.Fatal("xlsx cells extract command is not registered")
	}
	if set := findSubcommand(cells, "set"); set == nil {
		t.Fatal("xlsx cells set command is not registered")
	}
	if clear := findSubcommand(cells, "clear"); clear == nil {
		t.Fatal("xlsx cells clear command is not registered")
	}
	if setBatch := findSubcommand(cells, "set-batch"); setBatch == nil {
		t.Fatal("xlsx cells set-batch command is not registered")
	}

	names := findSubcommand(xlsx, "names")
	if names == nil {
		t.Fatal("xlsx names command is not registered")
	}
	for _, name := range []string{"list", "show", "add", "update", "rename", "delete"} {
		if sub := findSubcommand(names, name); sub == nil {
			t.Fatalf("xlsx names %s command is not registered", name)
		}
	}

	ranges := findSubcommand(xlsx, "ranges")
	if ranges == nil {
		t.Fatal("xlsx ranges command is not registered")
	}
	if export := findSubcommand(ranges, "export"); export == nil {
		t.Fatal("xlsx ranges export command is not registered")
	}
	if set := findSubcommand(ranges, "set"); set == nil {
		t.Fatal("xlsx ranges set command is not registered")
	}
	if setFormat := findSubcommand(ranges, "set-format"); setFormat == nil {
		t.Fatal("xlsx ranges set-format command is not registered")
	}

	tables := findSubcommand(xlsx, "tables")
	if tables == nil {
		t.Fatal("xlsx tables command is not registered")
	}
	for _, name := range []string{"list", "show", "export", "append-rows", "append-records"} {
		if sub := findSubcommand(tables, name); sub == nil {
			t.Fatalf("xlsx tables %s command is not registered", name)
		}
	}

	pivots := findSubcommand(xlsx, "pivots")
	if pivots == nil {
		t.Fatal("xlsx pivots command is not registered")
	}
	for _, name := range []string{"list", "show"} {
		if sub := findSubcommand(pivots, name); sub == nil {
			t.Fatalf("xlsx pivots %s command is not registered", name)
		}
	}

	charts := findSubcommand(xlsx, "charts")
	if charts == nil {
		t.Fatal("xlsx charts command is not registered")
	}
	for _, name := range []string{"list", "show", "update-source"} {
		if sub := findSubcommand(charts, name); sub == nil {
			t.Fatalf("xlsx charts %s command is not registered", name)
		}
	}

	rows := findSubcommand(xlsx, "rows")
	if rows == nil {
		t.Fatal("xlsx rows command is not registered")
	}
	if insert := findSubcommand(rows, "insert"); insert == nil {
		t.Fatal("xlsx rows insert command is not registered")
	}
	if deleteCmd := findSubcommand(rows, "delete"); deleteCmd == nil {
		t.Fatal("xlsx rows delete command is not registered")
	}

	cols := findSubcommand(xlsx, "cols")
	if cols == nil {
		t.Fatal("xlsx cols command is not registered")
	}
	if insert := findSubcommand(cols, "insert"); insert == nil {
		t.Fatal("xlsx cols insert command is not registered")
	}
	if deleteCmd := findSubcommand(cols, "delete"); deleteCmd == nil {
		t.Fatal("xlsx cols delete command is not registered")
	}
}

func TestXLSXSheetsListJSON(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2", State: "hidden"},
	})

	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets list failed: %v", err)
	}

	var result XLSXSheetsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
	}

	if result.File != workbookPath {
		t.Fatalf("file = %q, want %q", result.File, workbookPath)
	}
	if len(result.Sheets) != 2 {
		t.Fatalf("sheet count = %d, want 2", len(result.Sheets))
	}
	if result.Sheets[0].Number != 1 || result.Sheets[0].Name != "Summary" || result.Sheets[0].PartURI != "/xl/worksheets/sheet1.xml" {
		t.Fatalf("unexpected first sheet: %+v", result.Sheets[0])
	}
	first := result.Sheets[0]
	if first.PrimarySelector != "sheetId:1" {
		t.Fatalf("first sheet primarySelector = %q, want sheetId:1", first.PrimarySelector)
	}
	if result.ValidateCommand == "" {
		t.Fatalf("validateCommand is empty: %+v", result)
	}
	if first.ShowCommand == "" || first.TablesListCommand == "" {
		t.Fatalf("generated sheet commands are incomplete: %+v", first)
	}
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, first.ShowCommand)
	var showResult XLSXSheetsShowResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal generated show command JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Sheets) != 1 || showResult.Sheets[0].Name != "Summary" {
		t.Fatalf("generated show command returned %+v", showResult.Sheets)
	}
	executeGeneratedOOXMLCommandForXLSXTest(t, first.TablesListCommand)
	executeGeneratedOOXMLCommandForXLSXTest(t, result.ValidateCommand)
	for _, want := range []string{"sheetId:1", "sheet:1", "#1", "rid:rId1", "rId:rId1", "part:/xl/worksheets/sheet1.xml", "name:Summary", "~Summary", "Summary"} {
		if !containsString(first.Selectors, want) {
			t.Fatalf("first sheet selectors missing %q: %+v", want, first.Selectors)
		}
	}
	if result.Sheets[1].State != "hidden" {
		t.Fatalf("second sheet state = %q, want hidden", result.Sheets[1].State)
	}
}

func TestXLSXSheetsListText(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2", State: "hidden"},
	})

	output, err := executeRootForXLSXTest(t, "xlsx", "sheets", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets list failed: %v", err)
	}

	for _, want := range []string{
		"[N]",
		"Name",
		"sheetId",
		"PartURI",
		"[1 ] Summary",
		"visible",
		"[2 ] Data",
		"hidden",
		"/xl/worksheets/sheet2.xml",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("text output missing %q:\n%s", want, output)
		}
	}
}

func TestXLSXSheetsShowJSON(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")

	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "show", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets show failed: %v", err)
	}

	var result XLSXSheetsShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath {
		t.Fatalf("file = %q, want %q", result.File, workbookPath)
	}
	if len(result.Sheets) != 1 {
		t.Fatalf("sheet count = %d, want 1", len(result.Sheets))
	}
	report := result.Sheets[0]
	if report.Name != "Sheet1" || report.UsedRange.Ref != "A1:C1" || report.RowCount != 1 || report.CellCount != 3 {
		t.Fatalf("unexpected sheet report: %+v", report)
	}
	if result.ValidateCommand == "" {
		t.Fatalf("validateCommand is empty: %+v", result)
	}
	for label, command := range map[string]string{
		"cells extract": report.CellsExtractCommand,
		"ranges export": report.RangesExportCommand,
		"tables list":   report.TablesListCommand,
		"set cell":      report.SetCellCommandTemplate,
		"set range":     report.SetRangeCommandTemplate,
		"validate":      result.ValidateCommand,
	} {
		if command == "" {
			t.Fatalf("%s command is empty: %+v", label, report)
		}
	}
	for label, command := range map[string]string{
		"cells extract": report.CellsExtractCommand,
		"ranges export": report.RangesExportCommand,
		"tables list":   report.TablesListCommand,
		"validate":      result.ValidateCommand,
	} {
		if output := executeGeneratedOOXMLCommandForXLSXTest(t, command); label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}
	if report.PrimarySelector != "sheetId:1" || !containsString(report.Selectors, "sheet:1") || !containsString(report.Selectors, "part:/xl/worksheets/sheet1.xml") {
		t.Fatalf("unexpected sheet report selectors: primary=%q selectors=%+v", report.PrimarySelector, report.Selectors)
	}
}

func TestXLSXSheetsShowAcceptsStableSelectors(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
	})

	for _, selector := range []string{
		"sheetId:1",
		"sheet:1",
		"#1",
		"rid:rId1",
		"rId:rId1",
		"part:/xl/worksheets/sheet1.xml",
		"name:Summary",
		"~Summary",
		"Summary",
		"1",
	} {
		t.Run(selector, func(t *testing.T) {
			output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "show", workbookPath, "--sheet", selector)
			if err != nil {
				t.Fatalf("xlsx sheets show --sheet %q failed: %v", selector, err)
			}
			var result XLSXSheetsShowResult
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
			}
			if len(result.Sheets) != 1 || result.Sheets[0].Name != "Summary" {
				t.Fatalf("selector %q resolved to %+v", selector, result.Sheets)
			}
		})
	}
}

func TestXLSXSheetsAddJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Tail", SheetID: "2"},
	})
	outPath := filepath.Join(t.TempDir(), "added.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "add", workbookPath,
		"--name", "Data",
		"--after", "Summary",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets add failed: %v", err)
	}
	var result XLSXSheetsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || result.Number != 2 || result.Name != "Data" || result.RelationshipID != "rId3" || result.PartURI != "/xl/worksheets/sheet3.xml" {
		t.Fatalf("unexpected add result: %+v", result)
	}
	// sheetId is randomly allocated (never reused after a delete), within the
	// Open XML SDK-enforced ST_SheetId range.
	if addedID, convErr := strconv.Atoi(result.SheetID); convErr != nil || addedID < 1 || addedID > 65534 || result.SheetID == "1" || result.SheetID == "2" {
		t.Fatalf("unexpected new sheetId %q", result.SheetID)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected add mutation metadata: %+v", result)
	}
	assertXLSXSheetsDestinationForTest(t, result.Destination, outPath, "Summary,Data,Tail", "Data")
	readback, show := assertXLSXSheetsMutationSavedCommandsForTest(t, result.XLSXSheetsMutationReadbackCommands, outPath, true)
	var listResult XLSXSheetsListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, readback)
	}
	if len(listResult.Sheets) != 3 || sheetListItemNamesForCLI(listResult.Sheets) != "Summary,Data,Tail" {
		t.Fatalf("unexpected sheet readback: %+v", listResult.Sheets)
	}
	var showResult XLSXSheetsShowResult
	if err := json.Unmarshal([]byte(show), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, show)
	}
	if len(showResult.Sheets) != 1 || !showResult.Sheets[0].UsedRange.Empty {
		t.Fatalf("added sheet report = %+v", showResult.Sheets)
	}
}

func TestXLSXSheetsAddDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "add", workbookPath,
		"--name", "DryRun",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx sheets add dry-run failed: %v", err)
	}
	var result XLSXSheetsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal add dry-run JSON: %v\n%s", err, output)
	}
	if result.Number != 2 || result.Name != "DryRun" {
		t.Fatalf("unexpected add dry-run result: %+v", result)
	}
	if result.Output != "" || !result.DryRun {
		t.Fatalf("unexpected add dry-run metadata: %+v", result)
	}
	assertXLSXSheetsDestinationForTest(t, result.Destination, "", "Sheet1,DryRun", "DryRun")
	assertXLSXSheetsMutationDryRunTemplatesForTest(t, result.XLSXSheetsMutationReadbackCommands, true)

	readback, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets list readback failed: %v", err)
	}
	var listResult XLSXSheetsListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, readback)
	}
	if len(listResult.Sheets) != 1 || listResult.Sheets[0].Name != "Sheet1" {
		t.Fatalf("dry-run wrote to workbook: %+v", listResult.Sheets)
	}
}

func TestXLSXSheetsAddInPlaceJSONReadback(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "add", workbookPath,
		"--name", "InPlace",
		"--in-place",
	)
	if err != nil {
		t.Fatalf("xlsx sheets add in-place failed: %v", err)
	}
	var result XLSXSheetsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal add in-place JSON: %v\n%s", err, output)
	}
	if result.Output != workbookPath || result.DryRun {
		t.Fatalf("unexpected add in-place metadata: %+v", result)
	}
	assertXLSXSheetsDestinationForTest(t, result.Destination, workbookPath, "Sheet1,InPlace", "InPlace")
	assertXLSXSheetsMutationSavedCommandsForTest(t, result.XLSXSheetsMutationReadbackCommands, workbookPath, true)
	list := readXLSXSheetsListForTest(t, workbookPath)
	if sheetListItemNamesForCLI(list.Sheets) != "Sheet1,InPlace" {
		t.Fatalf("unexpected in-place sheet order: %+v", list.Sheets)
	}
}

func TestXLSXSheetsRenameJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
	})
	outPath := filepath.Join(t.TempDir(), "renamed.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "rename", workbookPath,
		"--sheet", "2",
		"--name", "Facts",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets rename failed: %v", err)
	}
	var result XLSXSheetsRenameResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal rename JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || result.Number != 2 || result.PreviousName != "Data" || result.Name != "Facts" || result.RelationshipID != "rId2" || result.PartURI != "/xl/worksheets/sheet2.xml" {
		t.Fatalf("unexpected rename result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected rename mutation metadata: %+v", result)
	}
	assertXLSXSheetsDestinationForTest(t, result.Destination, outPath, "Summary,Facts", "Facts")
	readback, _ := assertXLSXSheetsMutationSavedCommandsForTest(t, result.XLSXSheetsMutationReadbackCommands, outPath, true)
	var listResult XLSXSheetsListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, readback)
	}
	if len(listResult.Sheets) != 2 || sheetListItemNamesForCLI(listResult.Sheets) != "Summary,Facts" {
		t.Fatalf("unexpected sheet readback: %+v", listResult.Sheets)
	}
}

func TestXLSXSheetsRenameDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "rename", workbookPath,
		"--sheet", "Sheet1",
		"--name", "DryName",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx sheets rename dry-run failed: %v", err)
	}
	var result XLSXSheetsRenameResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal rename dry-run JSON: %v\n%s", err, output)
	}
	if result.Output != "" || !result.DryRun || result.Name != "DryName" || result.PreviousName != "Sheet1" {
		t.Fatalf("unexpected rename dry-run result: %+v", result)
	}
	assertXLSXSheetsDestinationForTest(t, result.Destination, "", "DryName", "DryName")
	assertXLSXSheetsMutationDryRunTemplatesForTest(t, result.XLSXSheetsMutationReadbackCommands, true)

	readback, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets list readback failed: %v", err)
	}
	var listResult XLSXSheetsListResult
	if err := json.Unmarshal([]byte(readback), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, readback)
	}
	if len(listResult.Sheets) != 1 || listResult.Sheets[0].Name != "Sheet1" {
		t.Fatalf("dry-run wrote to workbook: %+v", listResult.Sheets)
	}
}

func TestXLSXSheetsMoveJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithWorkbookMetadata(t, []testSheet{
		{Name: "Summary", SheetID: "10"},
		{Name: "Data", SheetID: "20"},
		{Name: "Tail", SheetID: "30"},
	}, true)
	outPath := filepath.Join(t.TempDir(), "moved.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "move", workbookPath,
		"--sheet", "Data",
		"--to", "1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets move failed: %v", err)
	}
	var result XLSXSheetsMoveResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal move JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || result.Name != "Data" || result.FromPosition != 2 || result.ToPosition != 1 || result.SheetID != "20" || result.RelationshipID != "rId2" || result.PartURI != "/xl/worksheets/sheet2.xml" || result.IsNoOp {
		t.Fatalf("unexpected move result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected move mutation metadata: %+v", result)
	}
	assertXLSXSheetsDestinationForTest(t, result.Destination, outPath, "Data,Summary,Tail", "Data")
	assertXLSXSheetsMutationSavedCommandsForTest(t, result.XLSXSheetsMutationReadbackCommands, outPath, true)
	list := readXLSXSheetsListForTest(t, outPath)
	if sheetListItemNamesForCLI(list.Sheets) != "Data,Summary,Tail" {
		t.Fatalf("unexpected moved sheet order: %+v", list.Sheets)
	}
	workbookXML := readZipEntryForTest(t, outPath, "xl/workbook.xml")
	for _, want := range []string{
		`activeTab="2"`,
		`firstSheet="1"`,
		`name="LocalSummary" localSheetId="1"`,
		`name="LocalTail" localSheetId="2"`,
		`name="GlobalName"`,
	} {
		if !strings.Contains(workbookXML, want) {
			t.Fatalf("moved workbook.xml missing %q:\n%s", want, workbookXML)
		}
	}
}

func TestXLSXSheetsMoveBeforeAfterAndDryRun(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
		{Name: "Tail", SheetID: "3"},
	})
	outPath := filepath.Join(t.TempDir(), "after.xlsx")

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "sheets", "move", workbookPath,
		"--sheet", "Summary",
		"--after", "Tail",
		"--out", outPath,
	); err != nil {
		t.Fatalf("xlsx sheets move --after failed: %v", err)
	}
	list := readXLSXSheetsListForTest(t, outPath)
	if sheetListItemNamesForCLI(list.Sheets) != "Data,Tail,Summary" {
		t.Fatalf("unexpected --after order: %+v", list.Sheets)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "move", workbookPath,
		"--sheet", "Tail",
		"--before", "Summary",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx sheets move dry-run failed: %v", err)
	}
	var moveResult XLSXSheetsMoveResult
	if err := json.Unmarshal([]byte(output), &moveResult); err != nil {
		t.Fatalf("failed to unmarshal move dry-run JSON: %v\n%s", err, output)
	}
	if moveResult.Output != "" || !moveResult.DryRun || moveResult.Name != "Tail" || moveResult.FromPosition != 3 || moveResult.ToPosition != 1 {
		t.Fatalf("unexpected move dry-run result: %+v", moveResult)
	}
	assertXLSXSheetsDestinationForTest(t, moveResult.Destination, "", "Tail,Summary,Data", "Tail")
	assertXLSXSheetsMutationDryRunTemplatesForTest(t, moveResult.XLSXSheetsMutationReadbackCommands, true)
	original := readXLSXSheetsListForTest(t, workbookPath)
	if sheetListItemNamesForCLI(original.Sheets) != "Summary,Data,Tail" {
		t.Fatalf("dry-run wrote to workbook: %+v", original.Sheets)
	}
}

func TestXLSXSheetsDeleteJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithWorkbookMetadata(t, []testSheet{
		{Name: "Summary", SheetID: "10"},
		{Name: "Data", SheetID: "20"},
		{Name: "Tail", SheetID: "30"},
	}, true)
	outPath := filepath.Join(t.TempDir(), "deleted.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "delete", workbookPath,
		"--sheet", "Data",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets delete failed: %v", err)
	}
	var result XLSXSheetsDeleteResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal delete JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || result.Name != "Data" || result.Number != 2 || result.RemainingSheets != 2 || result.RemovedRelationshipID != "rId2" {
		t.Fatalf("unexpected delete result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected delete mutation metadata: %+v", result)
	}
	if result.Deleted == nil {
		t.Fatalf("delete result missing deleted sheet ref: %+v", result)
	}
	assertXLSXSheetRefForTest(t, *result.Deleted, "Data")
	assertXLSXSheetsDestinationForTest(t, result.Destination, outPath, "Summary,Tail", "")
	if !containsString(result.RemovedParts, "/xl/worksheets/sheet2.xml") || !containsString(result.RemovedParts, "/xl/worksheets/_rels/sheet2.xml.rels") || !containsString(result.RemovedParts, "/xl/calcChain.xml") {
		t.Fatalf("removed parts missing expected cleanup: %+v", result.RemovedParts)
	}
	assertXLSXSheetsMutationSavedCommandsForTest(t, result.XLSXSheetsMutationReadbackCommands, outPath, false)
	list := readXLSXSheetsListForTest(t, outPath)
	if sheetListItemNamesForCLI(list.Sheets) != "Summary,Tail" {
		t.Fatalf("unexpected delete readback: %+v", list.Sheets)
	}
	if _, ok := tryReadZipEntryForTest(t, outPath, "xl/worksheets/sheet2.xml"); ok {
		t.Fatal("deleted worksheet part still exists")
	}
	if _, ok := tryReadZipEntryForTest(t, outPath, "xl/worksheets/_rels/sheet2.xml.rels"); ok {
		t.Fatal("deleted worksheet rels part still exists")
	}
	if _, ok := tryReadZipEntryForTest(t, outPath, "xl/calcChain.xml"); ok {
		t.Fatal("calcChain part still exists")
	}
	workbookRels := readZipEntryForTest(t, outPath, "xl/_rels/workbook.xml.rels")
	if strings.Contains(workbookRels, `Id="rId2"`) || strings.Contains(workbookRels, `calcChain`) {
		t.Fatalf("workbook rels still contain deleted relationship:\n%s", workbookRels)
	}
	contentTypes := readZipEntryForTest(t, outPath, "[Content_Types].xml")
	if strings.Contains(contentTypes, `/xl/worksheets/sheet2.xml`) || strings.Contains(contentTypes, `/xl/calcChain.xml`) {
		t.Fatalf("content types still contain removed parts:\n%s", contentTypes)
	}
	workbookXML := readZipEntryForTest(t, outPath, "xl/workbook.xml")
	for _, want := range []string{
		`activeTab="1"`,
		`firstSheet="0"`,
		`name="LocalSummary" localSheetId="0"`,
		`name="LocalTail" localSheetId="1"`,
		`name="GlobalName"`,
	} {
		if !strings.Contains(workbookXML, want) {
			t.Fatalf("deleted workbook.xml missing %q:\n%s", want, workbookXML)
		}
	}
	if strings.Contains(workbookXML, "LocalData") {
		t.Fatalf("deleted sheet scoped definedName remained:\n%s", workbookXML)
	}
}

func TestXLSXSheetsDeleteDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
	})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "sheets", "delete", workbookPath,
		"--sheet", "Data",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx sheets delete dry-run failed: %v", err)
	}
	var result XLSXSheetsDeleteResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal delete dry-run JSON: %v\n%s", err, output)
	}
	if result.Output != "" || !result.DryRun || result.Name != "Data" || result.RemainingSheets != 1 {
		t.Fatalf("unexpected delete dry-run result: %+v", result)
	}
	if result.Deleted == nil {
		t.Fatalf("delete dry-run missing deleted sheet ref: %+v", result)
	}
	assertXLSXSheetRefForTest(t, *result.Deleted, "Data")
	assertXLSXSheetsDestinationForTest(t, result.Destination, "", "Summary", "")
	assertXLSXSheetsMutationDryRunTemplatesForTest(t, result.XLSXSheetsMutationReadbackCommands, false)
	list := readXLSXSheetsListForTest(t, workbookPath)
	if sheetListItemNamesForCLI(list.Sheets) != "Summary,Data" {
		t.Fatalf("dry-run wrote to workbook: %+v", list.Sheets)
	}
}

func TestXLSXSheetsMutationTextOutput(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
	})
	tmpDir := t.TempDir()

	addPath := filepath.Join(tmpDir, "added.xlsx")
	output, err := executeRootForXLSXTest(t,
		"xlsx", "sheets", "add", workbookPath,
		"--name", "Tail",
		"--out", addPath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets add text failed: %v", err)
	}
	if output != "added sheet 3 \"Tail\"\n" {
		t.Fatalf("unexpected add text output: %q", output)
	}

	renamePath := filepath.Join(tmpDir, "renamed.xlsx")
	output, err = executeRootForXLSXTest(t,
		"xlsx", "sheets", "rename", addPath,
		"--sheet", "Tail",
		"--name", "Facts",
		"--out", renamePath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets rename text failed: %v", err)
	}
	if output != "renamed sheet 3 \"Tail\" -> \"Facts\"\n" {
		t.Fatalf("unexpected rename text output: %q", output)
	}

	movePath := filepath.Join(tmpDir, "moved.xlsx")
	output, err = executeRootForXLSXTest(t,
		"xlsx", "sheets", "move", renamePath,
		"--sheet", "Facts",
		"--to", "1",
		"--out", movePath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets move text failed: %v", err)
	}
	if output != "moved sheet \"Facts\" from 3 to 1\n" {
		t.Fatalf("unexpected move text output: %q", output)
	}

	deletePath := filepath.Join(tmpDir, "deleted.xlsx")
	output, err = executeRootForXLSXTest(t,
		"xlsx", "sheets", "delete", movePath,
		"--sheet", "Facts",
		"--out", deletePath,
	)
	if err != nil {
		t.Fatalf("xlsx sheets delete text failed: %v", err)
	}
	if output != "deleted sheet 1 \"Facts\"\n" {
		t.Fatalf("unexpected delete text output: %q", output)
	}
}

func TestXLSXSheetsMutationsRejectBadArgs(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
	})

	tests := []struct {
		args []string
		code int
	}{
		{[]string{"xlsx", "sheets", "add", workbookPath, "--name", "Bad/Name", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "add", workbookPath, "--name", "History", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "add", workbookPath, "--name", "data", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "rename", workbookPath, "--sheet", "Summary", "--name", "Data", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "rename", workbookPath, "--sheet", "Missing", "--name", "Ok", "--dry-run"}, ExitTargetNotFound},
		{[]string{"xlsx", "sheets", "move", workbookPath, "--sheet", "Summary", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "move", workbookPath, "--sheet", "Summary", "--to", "1", "--after", "Data", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "move", workbookPath, "--sheet", "Summary", "--to", "0", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "move", workbookPath, "--sheet", "Missing", "--to", "1", "--dry-run"}, ExitTargetNotFound},
		{[]string{"xlsx", "sheets", "delete", workbookPath, "--sheet", "Missing", "--dry-run"}, ExitTargetNotFound},
		{[]string{"xlsx", "sheets", "delete", writeTestXLSX(t, []testSheet{{Name: "Only", SheetID: "1"}}), "--sheet", "Only", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "sheets", "delete", writeTestXLSX(t, []testSheet{{Name: "Visible", SheetID: "1"}, {Name: "Hidden", SheetID: "2", State: "hidden"}}), "--sheet", "Visible", "--dry-run"}, ExitInvalidArgs},
	}
	for _, tt := range tests {
		_, err := executeRootForXLSXTest(t, tt.args...)
		if err == nil {
			t.Fatalf("%v: expected error", tt.args)
		}
		cliErr, ok := err.(*CLIError)
		if !ok {
			t.Fatalf("%v: error type = %T, want *CLIError", tt.args, err)
		}
		if cliErr.ExitCode != tt.code {
			t.Fatalf("%v: exit code = %d, want %d", tt.args, cliErr.ExitCode, tt.code)
		}
	}
}

func TestXLSXCellsExtractJSON(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")

	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "cells", "extract", workbookPath)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}

	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
	}
	if result.Sheet == nil {
		t.Fatal("sheet is nil")
	}
	if result.Sheet.UsedRange.Ref != "A1:C1" {
		t.Fatalf("used range = %q, want A1:C1", result.Sheet.UsedRange.Ref)
	}
	cells := result.Sheet.Rows[0].Cells
	if len(cells) != 3 {
		t.Fatalf("cell count = %d, want 3", len(cells))
	}
	if cells[0].Value != "Hello" || cells[1].Value != "42" || cells[2].Value != "true" {
		t.Fatalf("unexpected cell values: %+v", cells)
	}
	if cells[0].PrimarySelector != "A1" || !containsString(cells[0].Selectors, "A1") {
		t.Fatalf("first cell missing selectors: %+v", cells[0])
	}
}

func TestXLSXCellsExtractRangeAndEmptyCells(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", workbookPath,
		"--range", "B1:D2",
		"--include-empty",
		"--max-rows", "2",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}

	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
	}
	if result.Sheet.CellCount != 2 {
		t.Fatalf("filtered cell count = %d, want 2", result.Sheet.CellCount)
	}
	if len(result.Sheet.Rows) != 2 {
		t.Fatalf("emitted rows = %d, want 2", len(result.Sheet.Rows))
	}
	first := result.Sheet.Rows[0].Cells
	second := result.Sheet.Rows[1].Cells
	if len(first) != 3 || first[0].Ref != "B1" || first[0].Value != "42" || first[2].Ref != "D1" || first[2].Type != "empty" {
		t.Fatalf("unexpected first dense row: %+v", first)
	}
	if len(second) != 3 || second[0].Ref != "B2" || second[0].Type != "empty" {
		t.Fatalf("unexpected second dense row: %+v", second)
	}
}

func TestXLSXCellsExtractTypesFormulasAndDates(t *testing.T) {
	workbookPath := getXLSXTestFilePath("types-and-formulas")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", workbookPath,
		"--sheet", "Types",
		"--range", "E2:H2",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}

	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
	}
	cells := result.Sheet.Rows[0].Cells
	if len(cells) != 4 {
		t.Fatalf("cell count = %d, want 4", len(cells))
	}
	if cells[0].Ref != "E2" || cells[0].Value != "2469" || cells[0].Formula != "B2*2" {
		t.Fatalf("unexpected formula cell: %+v", cells[0])
	}
	if cells[1].Ref != "F2" || cells[1].Value != "North region" || cells[1].Type != "string" {
		t.Fatalf("unexpected formula string cell: %+v", cells[1])
	}
	if cells[2].Ref != "G2" || cells[2].Value != "Inline value" {
		t.Fatalf("unexpected inline cell: %+v", cells[2])
	}
	if cells[3].Ref != "H2" || cells[3].Value != "45292" || cells[3].Type != "date" || !cells[3].DateStyle {
		t.Fatalf("unexpected date-style cell: %+v", cells[3])
	}
}

func TestXLSXCellsSetJSONAndReadback(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "set.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set", workbookPath,
		"--cell", "b2",
		"--value", "42.50",
		"--type", "number",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells set failed: %v", err)
	}

	var result XLSXCellsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || result.Sheet != "Sheet1" || result.Ref != "B2" || result.Type != "number" || result.Value != "42.50" || !result.Created {
		t.Fatalf("unexpected set result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected set mutation metadata: %+v", result)
	}
	if result.Destination == nil {
		t.Fatal("cells set result missing destination readback")
	}
	if result.Destination.File != outPath || result.Destination.Sheet != "Sheet1" || result.Destination.Range != "B2" || result.Destination.Rows != 1 || result.Destination.Cols != 1 {
		t.Fatalf("unexpected set destination metadata: %+v", result.Destination)
	}
	if result.Destination.SheetPrimarySelector == "" || !containsString(result.Destination.SheetSelectors, "name:Sheet1") {
		t.Fatalf("set destination missing sheet selectors: %+v", result.Destination)
	}
	if len(result.Destination.Values) != 1 || len(result.Destination.Values[0]) != 1 || fmt.Sprint(result.Destination.Values[0][0]) != "42.5" {
		t.Fatalf("unexpected set destination values: %+v", result.Destination.Values)
	}
	if len(result.Destination.Types) != 1 || result.Destination.Types[0][0] != "number" || result.Destination.FormulaCount != 0 {
		t.Fatalf("unexpected set destination types/formulas: %+v", result.Destination)
	}

	extracted, _ := assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "B2")
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := extractResult.Sheet.Rows[0].Cells
	if len(cells) != 1 || cells[0].Ref != "B2" || cells[0].Type != "number" || cells[0].Value != "42.50" {
		t.Fatalf("unexpected readback cells: %+v", cells)
	}
}

func TestXLSXCellsSetDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set", workbookPath,
		"--cell", "B2",
		"--value", "dry",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx cells set dry-run failed: %v", err)
	}
	var result XLSXCellsSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run set JSON: %v\n%s", err, output)
	}
	if result.Ref != "B2" || result.Value != "dry" || !result.Created {
		t.Fatalf("unexpected dry-run result: %+v", result)
	}
	if !result.DryRun || result.Output != "" || result.Destination == nil {
		t.Fatalf("unexpected dry-run mutation metadata: %+v", result)
	}
	if result.Destination.File != "" || result.Destination.Range != "B2" || len(result.Destination.Values) != 1 || result.Destination.Values[0][0] != "dry" {
		t.Fatalf("unexpected dry-run destination readback: %+v", result.Destination)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "B2")

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", workbookPath,
		"--range", "B2",
		"--include-empty",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := extractResult.Sheet.Rows[0].Cells
	if len(cells) != 1 || cells[0].Type != "empty" {
		t.Fatalf("dry-run wrote to workbook, readback cells: %+v", cells)
	}
}

func TestXLSXCellsSetFormulaAndValidate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "formula.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set", workbookPath,
		"--cell", "C3",
		"--formula", "=SUM(A1:B1)",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells set formula failed: %v", err)
	}
	var setResult XLSXCellsSetResult
	if err := json.Unmarshal([]byte(output), &setResult); err != nil {
		t.Fatalf("failed to unmarshal formula set JSON: %v\n%s", err, output)
	}
	if setResult.Output != outPath || setResult.DryRun || setResult.Destination == nil {
		t.Fatalf("unexpected formula set metadata: %+v", setResult)
	}
	if setResult.Destination.Range != "C3" || setResult.Destination.FormulaCount != 1 || setResult.Destination.Formulas[0][0] != "SUM(A1:B1)" {
		t.Fatalf("unexpected formula destination formulas: %+v", setResult.Destination)
	}
	if setResult.Destination.Values[0][0] != nil || setResult.Destination.Types[0][0] != "number" {
		t.Fatalf("unexpected formula destination values/types: %+v", setResult.Destination)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate failed: %v", err)
	}
	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", outPath,
		"--range", "C3",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &result); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := result.Sheet.Rows[0].Cells
	if len(cells) != 1 || cells[0].Ref != "C3" || cells[0].Formula != "SUM(A1:B1)" {
		t.Fatalf("unexpected formula readback: %+v", cells)
	}
}

func TestXLSXCellsClearJSONAndReadback(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "clear.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "clear", workbookPath,
		"--range", "A1:B1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells clear failed: %v", err)
	}

	var result XLSXCellsClearResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal clear JSON: %v\n%s", err, output)
	}
	if result.Sheet != "Sheet1" || result.Range != "A1:B1" || result.Cleared != 2 || strings.Join(result.Refs, ",") != "A1,B1" {
		t.Fatalf("unexpected clear result: %+v", result)
	}
	if result.Output != outPath || result.DryRun || result.Destination == nil {
		t.Fatalf("unexpected clear mutation metadata: %+v", result)
	}
	if result.Destination.File != outPath || result.Destination.Sheet != "Sheet1" || result.Destination.Range != "A1:B1" || result.Destination.Rows != 1 || result.Destination.Cols != 2 {
		t.Fatalf("unexpected clear destination metadata: %+v", result.Destination)
	}
	if result.Destination.SheetPrimarySelector == "" || !containsString(result.Destination.SheetSelectors, "name:Sheet1") {
		t.Fatalf("clear destination missing sheet selectors: %+v", result.Destination)
	}
	if len(result.Destination.Values) != 1 || len(result.Destination.Values[0]) != 2 || result.Destination.Values[0][0] != nil || result.Destination.Values[0][1] != nil {
		t.Fatalf("unexpected clear destination values: %+v", result.Destination.Values)
	}
	if result.Destination.Types[0][0] != "empty" || result.Destination.Types[0][1] != "empty" || result.Destination.FormulaCount != 0 {
		t.Fatalf("unexpected clear destination types/formulas: %+v", result.Destination)
	}
	assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "A1:B1")

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", outPath,
		"--range", "A1:C1",
		"--include-empty",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := extractResult.Sheet.Rows[0].Cells
	if len(cells) != 3 || cells[0].Type != "empty" || cells[1].Type != "empty" || cells[2].Ref != "C1" || cells[2].Value != "true" {
		t.Fatalf("unexpected clear readback: %+v", cells)
	}
}

func TestXLSXCellsClearDryRunIncludesDestinationReadback(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "clear", workbookPath,
		"--range", "A1:B1",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx cells clear dry-run failed: %v", err)
	}

	var result XLSXCellsClearResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal clear dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Destination == nil {
		t.Fatalf("unexpected clear dry-run metadata: %+v", result)
	}
	if result.Destination.File != "" || result.Destination.Range != "A1:B1" || len(result.Destination.Values) != 1 || len(result.Destination.Values[0]) != 2 {
		t.Fatalf("unexpected clear dry-run destination metadata: %+v", result.Destination)
	}
	if result.Destination.Values[0][0] != nil || result.Destination.Values[0][1] != nil || result.Destination.Types[0][0] != "empty" || result.Destination.Types[0][1] != "empty" {
		t.Fatalf("unexpected clear dry-run destination readback: %+v", result.Destination)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "A1:B1")

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", workbookPath,
		"--range", "A1:B1",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := extractResult.Sheet.Rows[0].Cells
	if len(cells) != 2 || cells[0].Type == "empty" || cells[1].Type == "empty" {
		t.Fatalf("dry-run clear wrote to workbook, readback cells: %+v", cells)
	}
}

func TestXLSXCellsClearReadbackMaxCellsTruncatesDestination(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "clear", workbookPath,
		"--range", "A1:Z1000",
		"--readback-max-cells", "3",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx cells clear capped dry-run failed: %v", err)
	}

	var result XLSXCellsClearResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal capped clear JSON: %v\n%s", err, output)
	}
	if result.Destination == nil || !result.Destination.Truncated {
		t.Fatalf("expected truncated destination readback: %+v", result.Destination)
	}
	if result.Destination.Range != "A1:Z1000" || result.Destination.Rows != 1000 || result.Destination.Cols != 26 {
		t.Fatalf("unexpected capped destination metadata: %+v", result.Destination)
	}
	if len(result.Destination.Values) != 1 || len(result.Destination.Values[0]) != 3 {
		t.Fatalf("unexpected capped destination shape: %+v", result.Destination.Values)
	}
}

func TestXLSXCellsSetAndClearTextOutputUnchanged(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	setPath := filepath.Join(t.TempDir(), "text-set.xlsx")
	clearPath := filepath.Join(t.TempDir(), "text-clear.xlsx")

	setOutput, err := executeRootForXLSXTest(t,
		"xlsx", "cells", "set", workbookPath,
		"--cell", "A1",
		"--value", "text",
		"--out", setPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells set text failed: %v", err)
	}
	if strings.TrimSpace(setOutput) != "set Sheet1!A1 = text (string)" {
		t.Fatalf("unexpected set text output: %q", setOutput)
	}

	clearOutput, err := executeRootForXLSXTest(t,
		"xlsx", "cells", "clear", setPath,
		"--range", "A1",
		"--out", clearPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells clear text failed: %v", err)
	}
	if strings.TrimSpace(clearOutput) != "cleared 1 cells in Sheet1!A1" {
		t.Fatalf("unexpected clear text output: %q", clearOutput)
	}
}

func TestXLSXCellsSetBatchJSONAndReadback(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "batch.xlsx")
	cellsJSON := `[{"ref":"A1","type":"string","value":"Name"},{"ref":"B1","type":"number","value":"42.50"},{"ref":"C1","type":"bool","value":"true"},{"ref":"D1","formula":"SUM(B1:B1)"}]`

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set-batch", workbookPath,
		"--cells", cellsJSON,
		"--details",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells set-batch failed: %v", err)
	}

	var result XLSXCellsSetBatchResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal set-batch JSON: %v\n%s", err, output)
	}
	if result.Updated != 4 || result.Created != 4 || result.FormulaCount != 1 || result.Range != "A1:D1" || len(result.Cells) != 4 {
		t.Fatalf("unexpected set-batch result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected set-batch mutation metadata: %+v", result)
	}
	if result.Destination == nil {
		t.Fatal("cells set-batch result missing destination readback")
	}
	if result.Destination.File != outPath || result.Destination.Sheet != "Sheet1" || result.Destination.Range != "A1:D1" || result.Destination.Rows != 1 || result.Destination.Cols != 4 {
		t.Fatalf("unexpected set-batch destination metadata: %+v", result.Destination)
	}
	if result.Destination.SheetPrimarySelector == "" || !containsString(result.Destination.SheetSelectors, "name:Sheet1") {
		t.Fatalf("set-batch destination missing sheet selectors: %+v", result.Destination)
	}
	if len(result.Destination.Values) != 1 || len(result.Destination.Values[0]) != 4 {
		t.Fatalf("unexpected set-batch destination shape: %+v", result.Destination)
	}
	if result.Destination.Values[0][0] != "Name" || fmt.Sprint(result.Destination.Values[0][1]) != "42.5" || result.Destination.Values[0][2] != true || result.Destination.Values[0][3] != nil {
		t.Fatalf("unexpected set-batch destination values: %+v", result.Destination.Values)
	}
	if result.Destination.Types[0][0] != "string" || result.Destination.Types[0][1] != "number" || result.Destination.Types[0][2] != "boolean" || result.Destination.Types[0][3] != "number" {
		t.Fatalf("unexpected set-batch destination types: %+v", result.Destination.Types)
	}
	if result.Destination.FormulaCount != 1 || result.Destination.Formulas[0][3] != "SUM(B1:B1)" {
		t.Fatalf("unexpected set-batch destination formulas: %+v", result.Destination)
	}
	assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "A1:D1")

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", outPath,
		"--range", "A1:D1",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := extractResult.Sheet.Rows[0].Cells
	if len(cells) != 4 || cells[0].Value != "Name" || cells[1].Value != "42.50" || cells[2].Value != "true" || cells[3].Formula != "SUM(B1:B1)" {
		t.Fatalf("unexpected batch readback: %+v", cells)
	}
}

func TestXLSXCellsSetBatchDryRunIncludesDestinationReadback(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	cellsJSON := `[{"ref":"A1","value":"top"},{"ref":"C2","type":"number","value":"9"}]`

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set-batch", workbookPath,
		"--cells", cellsJSON,
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx cells set-batch dry-run failed: %v", err)
	}

	var result XLSXCellsSetBatchResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run set-batch JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Destination == nil || len(result.Cells) != 0 {
		t.Fatalf("unexpected set-batch dry-run metadata: %+v", result)
	}
	if result.Destination.File != "" || result.Destination.Range != "A1:C2" || result.Destination.Rows != 2 || result.Destination.Cols != 3 {
		t.Fatalf("unexpected set-batch dry-run destination metadata: %+v", result.Destination)
	}
	if result.Destination.Values[0][0] != "top" || result.Destination.Values[0][1] != nil || result.Destination.Values[1][2] == nil || fmt.Sprint(result.Destination.Values[1][2]) != "9" {
		t.Fatalf("unexpected set-batch dry-run destination values: %+v", result.Destination.Values)
	}
	if result.Destination.Types[0][1] != "empty" || result.Destination.Types[1][2] != "number" {
		t.Fatalf("unexpected set-batch dry-run destination types: %+v", result.Destination.Types)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "A1:C2")

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", workbookPath,
		"--range", "A1:C2",
		"--include-empty",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	for _, row := range extractResult.Sheet.Rows {
		for _, cell := range row.Cells {
			if cell.Type != "empty" {
				t.Fatalf("dry-run set-batch wrote to workbook, readback rows: %+v", extractResult.Sheet.Rows)
			}
		}
	}
}

func TestXLSXCellsSetBatchReadbackMaxCellsTruncatesDestination(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	cellsJSON := `[{"ref":"A1","value":"start"},{"ref":"Z1000","value":"end"}]`

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "set-batch", workbookPath,
		"--cells", cellsJSON,
		"--readback-max-cells", "3",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx cells set-batch capped dry-run failed: %v", err)
	}

	var result XLSXCellsSetBatchResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal capped set-batch JSON: %v\n%s", err, output)
	}
	if result.Destination == nil || !result.Destination.Truncated {
		t.Fatalf("expected truncated set-batch destination readback: %+v", result.Destination)
	}
	if result.Destination.Range != "A1:Z1000" || result.Destination.Rows != 1000 || result.Destination.Cols != 26 {
		t.Fatalf("unexpected capped set-batch destination metadata: %+v", result.Destination)
	}
	if len(result.Destination.Values) != 1 || len(result.Destination.Values[0]) != 3 || result.Destination.Values[0][0] != "start" {
		t.Fatalf("unexpected capped set-batch destination shape: %+v", result.Destination.Values)
	}
}

func TestXLSXCellsSetBatchTextOutputUnchanged(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "batch-text.xlsx")
	cellsJSON := `[{"ref":"A1","value":"left"},{"ref":"B1","formula":"SUM(A1:A1)"}]`

	output, err := executeRootForXLSXTest(t,
		"xlsx", "cells", "set-batch", workbookPath,
		"--cells", cellsJSON,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx cells set-batch text failed: %v", err)
	}
	if strings.TrimSpace(output) != "set 2 cells in Sheet1 (A1:B1); formulas: 1" {
		t.Fatalf("unexpected set-batch text output: %q", output)
	}
}

func TestXLSXCellsSetBatchCellsFileAndStdin(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	fileOutPath := filepath.Join(t.TempDir(), "batch-file.xlsx")
	stdinOutPath := filepath.Join(t.TempDir(), "batch-stdin.xlsx")
	cellsPath := filepath.Join(t.TempDir(), "cells.json")
	if err := os.WriteFile(cellsPath, []byte(`[{"ref":"A2","value":"from file"}]`), 0o644); err != nil {
		t.Fatalf("failed to write cells file: %v", err)
	}

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "cells", "set-batch", workbookPath,
		"--cells-file", cellsPath,
		"--out", fileOutPath,
	); err != nil {
		t.Fatalf("xlsx cells set-batch --cells-file failed: %v", err)
	}

	if _, err := executeRootForXLSXTestWithInput(t,
		strings.NewReader(`[{"ref":"B2","value":"from stdin"}]`),
		"xlsx", "cells", "set-batch", fileOutPath,
		"--cells-file", "-",
		"--out", stdinOutPath,
	); err != nil {
		t.Fatalf("xlsx cells set-batch stdin failed: %v", err)
	}

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", stdinOutPath,
		"--range", "A2:B2",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &result); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := result.Sheet.Rows[0].Cells
	if len(cells) != 2 || cells[0].Value != "from file" || cells[1].Value != "from stdin" {
		t.Fatalf("unexpected file/stdin readback: %+v", cells)
	}
}

func TestXLSXRangesExportJSONMatrix(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B1"><v>42</v></c>
      <c r="C1" t="b"><v>1</v></c>
    </row>
    <row r="2">
      <c r="A2"><f>SUM(B1:B1)</f><v>42</v></c>
      <c r="C2" t="inlineStr"><is><t>tail</t></is></c>
    </row>
  </sheetData>
</worksheet>`)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "export", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:C2",
		"--include-types",
		"--include-formulas",
	)
	if err != nil {
		t.Fatalf("xlsx ranges export failed: %v", err)
	}

	var result XLSXRangesExportResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal ranges export JSON: %v\n%s", err, output)
	}
	if result.Range != "A1:C2" || result.Rows != 2 || result.Cols != 3 || result.FormulaCount != 1 {
		t.Fatalf("unexpected export result: %+v", result)
	}
	if result.PrimarySelector != "A1:C2" || !containsString(result.Selectors, "A1:C2") {
		t.Fatalf("range export missing selectors: %+v", result)
	}
	if result.Values[0][0] != "Name" || fmt.Sprint(result.Values[0][1]) != "42" || result.Values[0][2] != true {
		t.Fatalf("unexpected first row values: %+v", result.Values[0])
	}
	if result.Values[1][1] != nil || result.Types[1][1] != "empty" || result.Formulas[1][0] != "SUM(B1:B1)" {
		t.Fatalf("unexpected second row metadata: values=%+v types=%+v formulas=%+v", result.Values[1], result.Types[1], result.Formulas[1])
	}
	if result.ValidateCommand == "" || result.CellsExtractCommand == "" {
		t.Fatalf("range export missing generated readback commands: %+v", result)
	}
	for label, command := range map[string]string{
		"cells extract": result.CellsExtractCommand,
		"validate":      result.ValidateCommand,
	} {
		if output := executeGeneratedOOXMLCommandForXLSXTest(t, command); label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}
	if result.PPTXUpdateTableCommandTemplate == "" || result.PPTXPlaceTableCommandTemplate == "" || result.PPTXReplaceTextCommandTemplate == "" {
		t.Fatalf("range export missing bridge templates: %+v", result)
	}
	if !strings.Contains(result.PPTXUpdateTableCommandTemplate, "--expect-source-range A1:C2") {
		t.Fatalf("pptx update template missing source guard: %s", result.PPTXUpdateTableCommandTemplate)
	}
	if strings.Contains(result.PPTXReplaceTextCommandTemplate, "--expect-source-range") {
		t.Fatalf("pptx text template used unsupported source guard: %s", result.PPTXReplaceTextCommandTemplate)
	}
}

func TestXLSXRangesExportCSVAndTSV(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>north,west</t></is></c>
      <c r="B1" t="inlineStr"><is><t>quoted "value"</t></is></c>
    </row>
    <row r="2">
      <c r="A2" t="inlineStr"><is><t>line
break</t></is></c>
      <c r="B2" t="inlineStr"><is><t>tab	value</t></is></c>
    </row>
  </sheetData>
</worksheet>`)
	csvPath := filepath.Join(t.TempDir(), "range.csv")
	tsvPath := filepath.Join(t.TempDir(), "range.tsv")

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "ranges", "export", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B2",
		"--data-format", "csv",
		"--data-out", csvPath,
	); err != nil {
		t.Fatalf("xlsx ranges export csv failed: %v", err)
	}
	if _, err := executeRootForXLSXTest(t,
		"xlsx", "ranges", "export", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B2",
		"--data-format", "tsv",
		"--data-out", tsvPath,
	); err != nil {
		t.Fatalf("xlsx ranges export tsv failed: %v", err)
	}

	csvRecords := readDelimitedRecordsForTest(t, csvPath, ',')
	if len(csvRecords) != 2 || csvRecords[0][0] != "north,west" || csvRecords[0][1] != `quoted "value"` || csvRecords[1][0] != "line\nbreak" {
		t.Fatalf("unexpected csv records: %+v", csvRecords)
	}
	tsvRecords := readDelimitedRecordsForTest(t, tsvPath, '\t')
	if len(tsvRecords) != 2 || tsvRecords[1][1] != "tab\tvalue" {
		t.Fatalf("unexpected tsv records: %+v", tsvRecords)
	}
}

func TestXLSXRangesSetJSONInlineReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "ranges-json.xlsx")
	values := `{"range":"A1:C2","values":[["Name",{"value":"42.5","type":"number"},{"formula":"SUM(B1:B1)"}],[null,true,"tail"]],"nullPolicy":"skip"}`

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "set", workbookPath,
		"--sheet", "Sheet1",
		"--values", values,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx ranges set failed: %v", err)
	}
	var result XLSXRangesSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal ranges set JSON: %v\n%s", err, output)
	}
	if result.Range != "A1:C2" || result.Updated != 5 || result.Skipped != 1 || result.FormulaCount != 1 {
		t.Fatalf("unexpected ranges set result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected mutation destination metadata: %+v", result)
	}
	if result.Destination == nil {
		t.Fatal("ranges set result missing destination readback")
	}
	if result.Destination.File != outPath || result.Destination.Sheet != "Sheet1" || result.Destination.Range != "A1:C2" || result.Destination.Rows != 2 || result.Destination.Cols != 3 {
		t.Fatalf("unexpected destination metadata: %+v", result.Destination)
	}
	if result.Destination.SheetPrimarySelector == "" || !containsString(result.Destination.SheetSelectors, "name:Sheet1") {
		t.Fatalf("destination missing sheet selectors: %+v", result.Destination)
	}
	if result.Destination.FormulaCount != 1 || len(result.Destination.Values) != 2 || len(result.Destination.Values[0]) != 3 {
		t.Fatalf("unexpected destination matrix shape: %+v", result.Destination)
	}
	if result.Destination.Values[0][0] != "Name" || fmt.Sprint(result.Destination.Values[0][1]) != "42.5" || result.Destination.Values[1][0] != nil || result.Destination.Values[1][1] != true || result.Destination.Values[1][2] != "tail" {
		t.Fatalf("unexpected destination values: %+v", result.Destination.Values)
	}
	if result.Destination.Values[0][2] != nil {
		t.Fatalf("newly written formula should not have a cached value: %+v", result.Destination.Values)
	}
	if result.Destination.Formulas[0][2] != "SUM(B1:B1)" {
		t.Fatalf("unexpected destination formulas: %+v", result.Destination.Formulas)
	}
	if result.Destination.Types[0][2] != "number" || result.Destination.Types[1][0] != "empty" || result.Destination.Types[1][1] != "boolean" {
		t.Fatalf("unexpected destination types: %+v", result.Destination.Types)
	}

	extracted, _ := assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "A1:C2")
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	var formula string
	valuesByRef := map[string]string{}
	for _, row := range extractResult.Sheet.Rows {
		for _, cell := range row.Cells {
			valuesByRef[cell.Ref] = cell.Value
			if cell.Ref == "C1" {
				formula = cell.Formula
			}
		}
	}
	if valuesByRef["A1"] != "Name" || valuesByRef["B1"] != "42.5" || valuesByRef["B2"] != "true" || valuesByRef["C2"] != "tail" || formula != "SUM(B1:B1)" {
		t.Fatalf("unexpected ranges readback: values=%+v formula=%q", valuesByRef, formula)
	}
}

func TestXLSXRangesSetFormatCreatesStylesReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1"><v>1234.5</v></c>
      <c r="B1"><f>A1*2</f><v>2469</v></c>
    </row>
  </sheetData>
</worksheet>`)
	outPath := filepath.Join(t.TempDir(), "ranges-format.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "set-format", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B2",
		"--preset", "currency",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx ranges set-format failed: %v", err)
	}

	var result XLSXRangesSetFormatResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal ranges set-format JSON: %v\n%s", err, output)
	}
	if result.Range != "A1:B2" || result.Updated != 4 || result.Created != 2 || result.NumberFormatID < 164 || result.FormatCode != `"$"#,##0.00` {
		t.Fatalf("unexpected set-format result: %+v", result)
	}
	if result.Output != outPath || result.DryRun || result.CreatedStyles != 1 || len(result.StyleIndexes) != 1 {
		t.Fatalf("unexpected set-format mutation metadata: %+v", result)
	}
	if result.Destination == nil {
		t.Fatal("set-format result missing destination readback")
	}
	if fmt.Sprint(result.Destination.Values[0][0]) != "1234.5" || fmt.Sprint(result.Destination.Values[0][1]) != "2469" || result.Destination.Values[1][0] != nil || result.Destination.Formulas[0][1] != "A1*2" {
		t.Fatalf("set-format did not preserve values/formulas: values=%+v formulas=%+v", result.Destination.Values, result.Destination.Formulas)
	}
	if result.Destination.NumberFormatCodes == nil || result.Destination.NumberFormatIDs == nil || result.Destination.StyleIndexes == nil {
		t.Fatalf("destination missing format readback: %+v", result.Destination)
	}
	for rowIdx, row := range result.Destination.NumberFormatCodes {
		for colIdx, code := range row {
			if code != `"$"#,##0.00` {
				t.Fatalf("format code[%d][%d] = %v, want currency; matrix=%+v", rowIdx, colIdx, code, result.Destination.NumberFormatCodes)
			}
		}
	}

	extracted, exported := assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "A1:B2")
	var extractResult XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &extractResult); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	if extractResult.Sheet.Rows[0].Cells[0].NumberFormatCode != `"$"#,##0.00` || extractResult.Sheet.Rows[0].Cells[1].Formula != "A1*2" {
		t.Fatalf("unexpected cells extract format readback: %+v", extractResult.Sheet.Rows[0].Cells)
	}
	var exportResult XLSXRangesExportResult
	if err := json.Unmarshal([]byte(exported), &exportResult); err != nil {
		t.Fatalf("failed to unmarshal export JSON: %v\n%s", err, exported)
	}
	if exportResult.NumberFormatCodes == nil || exportResult.NumberFormatCodes[0][0] != `"$"#,##0.00` || exportResult.StyleIndexes == nil {
		t.Fatalf("ranges export missing format readback: %+v", exportResult)
	}
	stylesXML := readZipEntryForTest(t, outPath, "xl/styles.xml")
	if !strings.Contains(stylesXML, `formatCode="&quot;$&quot;#,##0.00"`) || !strings.Contains(stylesXML, `applyNumberFormat="1"`) {
		t.Fatalf("styles.xml missing currency format:\n%s", stylesXML)
	}
	relsXML := readZipEntryForTest(t, outPath, "xl/_rels/workbook.xml.rels")
	if !strings.Contains(relsXML, `relationships/styles`) || !strings.Contains(relsXML, `styles.xml`) {
		t.Fatalf("workbook relationships missing styles relationship:\n%s", relsXML)
	}
}

func TestXLSXRangesSetFormatDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "set-format", workbookPath,
		"--sheet", "Sheet1",
		"--range", "C3",
		"--preset", "percent",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx ranges set-format dry-run failed: %v", err)
	}
	var result XLSXRangesSetFormatResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run set-format JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Destination == nil || result.Destination.File != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}
	if result.NumberFormatID != 10 || result.Destination.NumberFormatCodes[0][0] != "0.00%" || result.Destination.Values[0][0] != nil {
		t.Fatalf("unexpected dry-run destination: %+v", result)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "C3")
	if _, ok := tryReadZipEntryForTest(t, workbookPath, "xl/styles.xml"); ok {
		t.Fatal("dry-run wrote styles.xml into the input workbook")
	}
}

func TestXLSXRangesSetValuesFileStdinAndDelimitedReadback(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	stdinOut := filepath.Join(t.TempDir(), "ranges-stdin.xlsx")
	csvOut := filepath.Join(t.TempDir(), "ranges-csv.xlsx")
	csvPath := filepath.Join(t.TempDir(), "values.csv")
	if err := os.WriteFile(csvPath, []byte("left,right\n1,=not formula\n"), 0o644); err != nil {
		t.Fatalf("failed to write csv values: %v", err)
	}

	if _, err := executeRootForXLSXTestWithInput(t,
		strings.NewReader(`[["from stdin"]]`),
		"xlsx", "ranges", "set", workbookPath,
		"--sheet", "Sheet1",
		"--anchor", "B2",
		"--values-file", "-",
		"--out", stdinOut,
	); err != nil {
		t.Fatalf("xlsx ranges set stdin failed: %v", err)
	}
	if _, err := executeRootForXLSXTest(t,
		"xlsx", "ranges", "set", stdinOut,
		"--sheet", "Sheet1",
		"--anchor", "A1",
		"--data-format", "csv",
		"--values-file", csvPath,
		"--out", csvOut,
	); err != nil {
		t.Fatalf("xlsx ranges set csv failed: %v", err)
	}

	extracted, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", csvOut,
		"--range", "A1:B2",
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(extracted), &result); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, extracted)
	}
	cells := result.Sheet.Rows[1].Cells
	if len(cells) != 2 || cells[0].Value != "1" || cells[1].Value != "=not formula" || cells[1].Formula != "" {
		t.Fatalf("unexpected csv readback: %+v", cells)
	}
}

func TestXLSXRangesSetNullPolicySkipAndClear(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>old</t></is></c>
      <c r="B1" t="inlineStr"><is><t>old</t></is></c>
    </row>
  </sheetData>
</worksheet>`)
	skipPath := filepath.Join(t.TempDir(), "skip.xlsx")
	clearPath := filepath.Join(t.TempDir(), "clear.xlsx")

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "ranges", "set", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1:B1",
		"--values", `[[null,"new"]]`,
		"--null-policy", "skip",
		"--out", skipPath,
	); err != nil {
		t.Fatalf("xlsx ranges set skip failed: %v", err)
	}
	values := readXLSXCellValuesForTest(t, skipPath, "A1:B1")
	assertXLSXCellValue(t, values, "A1", "old")
	assertXLSXCellValue(t, values, "B1", "new")

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "ranges", "set", skipPath,
		"--sheet", "Sheet1",
		"--range", "A1:B1",
		"--values", `[[null,"newer"]]`,
		"--null-policy", "clear",
		"--out", clearPath,
	); err != nil {
		t.Fatalf("xlsx ranges set clear failed: %v", err)
	}
	values = readXLSXCellValuesForTest(t, clearPath, "A1:B1")
	assertXLSXNoCell(t, values, "A1")
	assertXLSXCellValue(t, values, "B1", "newer")
}

func TestXLSXRangesSetDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "set", workbookPath,
		"--sheet", "Sheet1",
		"--anchor", "A1",
		"--values", `[["dry"]]`,
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx ranges set dry-run failed: %v", err)
	}
	var result XLSXRangesSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run ranges set JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Destination == nil {
		t.Fatalf("unexpected dry-run result metadata: %+v", result)
	}
	if result.Destination.File != "" || result.Destination.Range != "A1" || len(result.Destination.Values) != 1 || result.Destination.Values[0][0] != "dry" {
		t.Fatalf("unexpected dry-run destination readback: %+v", result.Destination)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "A1")
	values := readXLSXCellValuesForTest(t, workbookPath, "A1")
	assertXLSXNoCell(t, values, "A1")
}

func TestXLSXRangesSetRejectsFormulaOverwriteAndMergedCells(t *testing.T) {
	formulaWorkbook := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1"/>
  <sheetData><row r="1"><c r="A1"><f>SUM(B1:B1)</f><v>1</v></c></row></sheetData>
</worksheet>`)
	args := []string{
		"xlsx", "ranges", "set", formulaWorkbook,
		"--sheet", "Sheet1",
		"--anchor", "A1",
		"--values", `[["replace"]]`,
		"--dry-run",
	}
	_, err := executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)

	if _, err := executeRootForXLSXTest(t, append(args, "--overwrite-formulas")...); err != nil {
		t.Fatalf("xlsx ranges set --overwrite-formulas failed: %v", err)
	}

	mergedWorkbook := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B1"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>
  <mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>
</worksheet>`)
	args = []string{
		"xlsx", "ranges", "set", mergedWorkbook,
		"--sheet", "Sheet1",
		"--range", "A1:B1",
		"--values", `[["x","y"]]`,
		"--dry-run",
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
}

func TestXLSXTablesListShowExport(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	listOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "tables", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx tables list failed: %v", err)
	}
	var listResult XLSXTablesResult
	if err := json.Unmarshal([]byte(listOutput), &listResult); err != nil {
		t.Fatalf("failed to unmarshal tables list JSON: %v\n%s", err, listOutput)
	}
	if len(listResult.Tables) != 1 {
		t.Fatalf("tables len = %d, want 1", len(listResult.Tables))
	}
	tableRef := listResult.Tables[0]
	if tableRef.DisplayName != "Sales" || tableRef.Sheet != "Data" || tableRef.Range != "A1:B3" || tableRef.DataRowCount != 2 {
		t.Fatalf("unexpected table metadata: %+v", tableRef)
	}
	if tableRef.PrimarySelector != "tableId:1" {
		t.Fatalf("table primarySelector = %q, want tableId:1", tableRef.PrimarySelector)
	}
	if listResult.ValidateCommand == "" {
		t.Fatalf("tables list validateCommand is empty: %+v", listResult)
	}
	for label, command := range map[string]string{
		"show":           tableRef.ShowCommand,
		"export":         tableRef.ExportCommand,
		"append rows":    tableRef.AppendRowsCommandTemplate,
		"append records": tableRef.AppendRecordsCommandTemplate,
		"pptx update":    tableRef.PPTXUpdateTableCommandTemplate,
		"pptx place":     tableRef.PPTXPlaceTableCommandTemplate,
		"pptx text":      tableRef.PPTXReplaceTextCommandTemplate,
	} {
		if command == "" {
			t.Fatalf("%s command is empty: %+v", label, tableRef)
		}
	}
	for label, command := range map[string]string{
		"show":     tableRef.ShowCommand,
		"export":   tableRef.ExportCommand,
		"validate": listResult.ValidateCommand,
	} {
		if output := executeGeneratedOOXMLCommandForXLSXTest(t, command); label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}
	if !strings.Contains(tableRef.PPTXUpdateTableCommandTemplate, "--expect-source-range A1:B3") {
		t.Fatalf("pptx update template missing source guard: %s", tableRef.PPTXUpdateTableCommandTemplate)
	}
	if strings.Contains(tableRef.PPTXReplaceTextCommandTemplate, "--expect-source-range") {
		t.Fatalf("pptx text template used unsupported source guard: %s", tableRef.PPTXReplaceTextCommandTemplate)
	}
	for _, want := range []string{"tableId:1", "id:1", "table:1", "#1", "part:/xl/tables/table1.xml", "rid:rId1", "rId:rId1", "table:Sales", "displayName:Sales", "name:Sales", "Sales"} {
		if !containsString(tableRef.Selectors, want) {
			t.Fatalf("table selectors missing %q: %+v", want, tableRef.Selectors)
		}
	}
	if len(tableRef.Columns) != 2 || tableRef.Columns[0].Name != "Region" || tableRef.Columns[1].Name != "Amount" {
		t.Fatalf("unexpected table columns: %+v", tableRef.Columns)
	}

	showOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "tables", "show", workbookPath, "--table", "Sales")
	if err != nil {
		t.Fatalf("xlsx tables show failed: %v", err)
	}
	var showResult XLSXTablesResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal tables show JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Tables) != 1 || showResult.Tables[0].PartURI != "/xl/tables/table1.xml" {
		t.Fatalf("unexpected show result: %+v", showResult.Tables)
	}
	if showResult.Tables[0].ExportCommand == "" || showResult.Tables[0].PPTXPlaceTableCommandTemplate == "" {
		t.Fatalf("show result missing generated commands: %+v", showResult.Tables[0])
	}

	exportOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "tables", "export", workbookPath, "--table", "Sales", "--include-types")
	if err != nil {
		t.Fatalf("xlsx tables export failed: %v", err)
	}
	var exportResult XLSXRangesExportResult
	if err := json.Unmarshal([]byte(exportOutput), &exportResult); err != nil {
		t.Fatalf("failed to unmarshal table export JSON: %v\n%s", err, exportOutput)
	}
	if exportResult.Range != "A1:B3" || exportResult.Rows != 3 || exportResult.Cols != 2 {
		t.Fatalf("unexpected export dimensions: %+v", exportResult)
	}
	if got := fmt.Sprint(exportResult.Values); !strings.Contains(got, "Region") || !strings.Contains(got, "20") {
		t.Fatalf("export values missing expected cells: %#v", exportResult.Values)
	}
	if exportResult.PPTXUpdateTableCommandTemplate == "" || exportResult.PPTXPlaceTableCommandTemplate == "" || exportResult.PPTXReplaceTextCommandTemplate == "" {
		t.Fatalf("table export missing bridge templates: %+v", exportResult)
	}
}

func TestXLSXTablesAcceptStableSelectors(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	for _, selector := range []string{
		"tableId:1",
		"id:1",
		"table:1",
		"#1",
		"part:/xl/tables/table1.xml",
		"rid:rId1",
		"rId:rId1",
		"table:Sales",
		"displayName:Sales",
		"name:Sales",
		"Sales",
		"1",
	} {
		t.Run(selector, func(t *testing.T) {
			output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "tables", "show", workbookPath, "--table", selector)
			if err != nil {
				t.Fatalf("xlsx tables show --table %q failed: %v", selector, err)
			}
			var result XLSXTablesResult
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Fatalf("failed to unmarshal tables show JSON: %v\n%s", err, output)
			}
			if len(result.Tables) != 1 || result.Tables[0].DisplayName != "Sales" {
				t.Fatalf("selector %q resolved to %+v", selector, result.Tables)
			}
		})
	}
}

func TestXLSXPivotsListShowJSONAndGeneratedCommands(t *testing.T) {
	workbookPath := writeTestXLSXWithPivot(t, false)

	listOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx pivots list failed: %v", err)
	}
	var listResult XLSXPivotsResult
	if err := json.Unmarshal([]byte(listOutput), &listResult); err != nil {
		t.Fatalf("failed to unmarshal pivots list JSON: %v\n%s", err, listOutput)
	}
	if len(listResult.Pivots) != 1 {
		t.Fatalf("pivots len = %d, want 1", len(listResult.Pivots))
	}
	pivot := listResult.Pivots[0]
	if pivot.Name != "SalesPivot" || pivot.Sheet != "Data" || pivot.Location != "D3:E6" || pivot.Rows != 4 || pivot.Cols != 2 {
		t.Fatalf("unexpected pivot metadata: %+v", pivot)
	}
	if pivot.PrimarySelector != "pivot:1" {
		t.Fatalf("pivot primarySelector = %q, want pivot:1", pivot.PrimarySelector)
	}
	if pivot.Cache == nil {
		t.Fatalf("pivot cache is nil: %+v", pivot)
	}
	if pivot.Cache.CacheID != 1 || pivot.Cache.Source.Type != "worksheet" || pivot.Cache.Source.Sheet != "Data" || pivot.Cache.Source.Range != "A1:D3" || !pivot.Cache.RefreshOnLoad {
		t.Fatalf("unexpected pivot cache metadata: %+v", pivot.Cache)
	}
	if len(pivot.Cache.Fields) != 4 || pivot.Cache.Fields[0].Name != "Region" || pivot.Cache.Fields[2].Name != "Amount" {
		t.Fatalf("unexpected cache fields: %+v", pivot.Cache.Fields)
	}
	if len(pivot.RowFields) != 1 || pivot.RowFields[0].Name != "Region" {
		t.Fatalf("unexpected row fields: %+v", pivot.RowFields)
	}
	if len(pivot.ColumnFields) != 1 || pivot.ColumnFields[0].Name != "Quarter" {
		t.Fatalf("unexpected column fields: %+v", pivot.ColumnFields)
	}
	if len(pivot.DataFields) != 1 || pivot.DataFields[0].Name != "Amount" || pivot.DataFields[0].Caption != "Sum of Amount" || pivot.DataFields[0].Subtotal != "sum" {
		t.Fatalf("unexpected data fields: %+v", pivot.DataFields)
	}
	if len(pivot.FilterFields) != 1 || pivot.FilterFields[0].Name != "Segment" {
		t.Fatalf("unexpected filter fields: %+v", pivot.FilterFields)
	}
	for label, command := range map[string]string{
		"show":     pivot.ShowCommand,
		"source":   pivot.SourceExportCommand,
		"validate": listResult.ValidateCommand,
	} {
		if command == "" {
			t.Fatalf("%s command is empty: %+v", label, pivot)
		}
		output := executeGeneratedOOXMLCommandForXLSXTest(t, command)
		if label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}
	for _, want := range []string{"pivot:1", "#1", "pivot:SalesPivot", "name:SalesPivot", "~SalesPivot", "SalesPivot", "part:/xl/pivotTables/pivotTable1.xml", "rid:rIdPivot1", "rId:rIdPivot1", "cacheId:1"} {
		if !containsString(pivot.Selectors, want) {
			t.Fatalf("pivot selectors missing %q: %+v", want, pivot.Selectors)
		}
	}

	showOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "show", workbookPath, "--pivot", "SalesPivot")
	if err != nil {
		t.Fatalf("xlsx pivots show failed: %v", err)
	}
	var showResult XLSXPivotsResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal pivots show JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Pivots) != 1 || showResult.Pivots[0].PartURI != "/xl/pivotTables/pivotTable1.xml" {
		t.Fatalf("unexpected show result: %+v", showResult.Pivots)
	}

	inspectOutput, err := executeRootForXLSXTest(t, "--format", "json", "inspect", workbookPath)
	if err != nil {
		t.Fatalf("inspect pivot workbook failed: %v", err)
	}
	var inspectResult XLSXInspectResult
	if err := json.Unmarshal([]byte(inspectOutput), &inspectResult); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, inspectOutput)
	}
	if inspectResult.Summary.Pivots != 1 || inspectResult.Summary.PivotCaches != 1 {
		t.Fatalf("unexpected inspect pivot counts: %+v", inspectResult.Summary)
	}
}

func TestXLSXPivotsAcceptStableSelectors(t *testing.T) {
	workbookPath := writeTestXLSXWithPivot(t, false)

	for _, selector := range []string{
		"pivot:1",
		"#1",
		"pivot:SalesPivot",
		"name:SalesPivot",
		"~SalesPivot",
		"SalesPivot",
		"part:/xl/pivotTables/pivotTable1.xml",
		"rid:rIdPivot1",
		"rId:rIdPivot1",
		"cacheId:1",
		"1",
	} {
		t.Run(selector, func(t *testing.T) {
			output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "show", workbookPath, "--pivot", selector)
			if err != nil {
				t.Fatalf("xlsx pivots show --pivot %q failed: %v", selector, err)
			}
			var result XLSXPivotsResult
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Fatalf("failed to unmarshal pivots show JSON: %v\n%s", err, output)
			}
			if len(result.Pivots) != 1 || result.Pivots[0].Name != "SalesPivot" {
				t.Fatalf("selector %q resolved to %+v", selector, result.Pivots)
			}
		})
	}
}

func TestXLSXPivotsTextAndBadArguments(t *testing.T) {
	workbookPath := writeTestXLSXWithPivot(t, true)

	output, err := executeRootForXLSXTest(t, "xlsx", "pivots", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx pivots list text failed: %v", err)
	}
	for _, want := range []string{"SalesPivot", "SalesPivot2", "rows: Region", "values: Sum of Amount (Amount)"} {
		if !strings.Contains(output, want) {
			t.Fatalf("text output missing %q:\n%s", want, output)
		}
	}

	ambiguousArgs := []string{"xlsx", "pivots", "show", workbookPath, "--pivot", "cacheId:1"}
	_, err = executeRootForXLSXTest(t, ambiguousArgs...)
	assertCLIExitCodeForXLSXTest(t, ambiguousArgs, err, ExitInvalidArgs)

	missingArgs := []string{"xlsx", "pivots", "show", workbookPath, "--pivot", "Missing"}
	_, err = executeRootForXLSXTest(t, missingArgs...)
	assertCLIExitCodeForXLSXTest(t, missingArgs, err, ExitTargetNotFound)

	singleWorkbook := writeTestXLSXWithPivot(t, false)
	noPivotSheetArgs := []string{"xlsx", "pivots", "show", singleWorkbook, "--sheet", "Missing"}
	_, err = executeRootForXLSXTest(t, noPivotSheetArgs...)
	assertCLIExitCodeForXLSXTest(t, noPivotSheetArgs, err, ExitTargetNotFound)
}

func TestXLSXTablesAppendRowsJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")
	outPath := filepath.Join(t.TempDir(), "table-append.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "append-rows", workbookPath,
		"--table", "Sales",
		"--values", `[["North",30],["South",40]]`,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx tables append-rows failed: %v", err)
	}
	var result XLSXTablesAppendRowsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal append JSON: %v\n%s", err, output)
	}
	if result.PreviousRange != "A1:B3" || result.Range != "A1:B5" || result.AppendRange != "A4:B5" || result.RowsAppended != 2 {
		t.Fatalf("unexpected append result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected append mutation metadata: %+v", result)
	}
	assertXLSXTableAppendDestinationForTest(t, result.Destination, outPath, "A1:B3", "A1:B5", "A4:B5", 5, 4)
	if result.Destination.Appended.Values[0][0] != "North" || fmt.Sprint(result.Destination.Appended.Values[0][1]) != "30" || result.Destination.Appended.Values[1][0] != "South" || fmt.Sprint(result.Destination.Appended.Values[1][1]) != "40" {
		t.Fatalf("unexpected appended values: %+v", result.Destination.Appended.Values)
	}
	if result.Destination.Appended.Types[0][0] != "string" || result.Destination.Appended.Types[0][1] != "number" {
		t.Fatalf("unexpected appended types: %+v", result.Destination.Appended.Types)
	}
	assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "A4:B5")

	tableXML := readZipEntryForTest(t, outPath, "xl/tables/table1.xml")
	if !strings.Contains(tableXML, `ref="A1:B5"`) {
		t.Fatalf("table ref was not expanded:\n%s", tableXML)
	}
	sheetXML := readZipEntryForTest(t, outPath, "xl/worksheets/sheet1.xml")
	for _, want := range []string{`r="A4"`, `North`, `r="B5"`, `<v>40</v>`} {
		if !strings.Contains(sheetXML, want) {
			t.Fatalf("worksheet missing %q after append:\n%s", want, sheetXML)
		}
	}

	showOutput, _ := assertXLSXTableAppendSavedCommandsForTest(t, result.XLSXTableAppendReadbackCommands, outPath)
	var showResult XLSXTablesResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show readback JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Tables) != 1 || showResult.Tables[0].Range != "A1:B5" || showResult.Tables[0].DataRowCount != 4 {
		t.Fatalf("unexpected table readback: %+v", showResult.Tables)
	}
}

func TestXLSXTablesAppendRecordsJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")
	outPath := filepath.Join(t.TempDir(), "table-append-records.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "append-records", workbookPath,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Amount":30,"Region":"North"},{"Region":"South","Amount":{"value":"40","type":"number"}}]`,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx tables append-records failed: %v", err)
	}
	var result XLSXTablesAppendRecordsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal append-records JSON: %v\n%s", err, output)
	}
	if result.PreviousRange != "A1:B3" || result.Range != "A1:B5" || result.AppendRange != "A4:B5" || result.RowsAppended != 2 {
		t.Fatalf("unexpected append-records result: %+v", result)
	}
	if len(result.Columns) != 2 || result.Columns[0] != "Region" || result.Columns[1] != "Amount" || result.NullPolicy != "skip" || result.MissingPolicy != "reject" {
		t.Fatalf("unexpected append-records metadata: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected append-records mutation metadata: %+v", result)
	}
	assertXLSXTableAppendDestinationForTest(t, result.Destination, outPath, "A1:B3", "A1:B5", "A4:B5", 5, 4)
	if result.Destination.Appended.Values[0][0] != "North" || fmt.Sprint(result.Destination.Appended.Values[0][1]) != "30" || result.Destination.Appended.Values[1][0] != "South" || fmt.Sprint(result.Destination.Appended.Values[1][1]) != "40" {
		t.Fatalf("unexpected append-records destination values: %+v", result.Destination.Appended.Values)
	}
	assertXLSXMutationSavedCommandsForTest(t, result.XLSXMutationReadbackCommands, outPath, "A4:B5")

	showOutput, _ := assertXLSXTableAppendSavedCommandsForTest(t, result.XLSXTableAppendReadbackCommands, outPath)
	var showResult XLSXTablesResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show readback JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Tables) != 1 || showResult.Tables[0].Range != "A1:B5" || showResult.Tables[0].DataRowCount != 4 {
		t.Fatalf("unexpected table readback: %+v", showResult.Tables)
	}
	sheetXML := readZipEntryForTest(t, outPath, "xl/worksheets/sheet1.xml")
	for _, want := range []string{`r="A4"`, `North`, `r="B5"`, `<v>40</v>`} {
		if !strings.Contains(sheetXML, want) {
			t.Fatalf("worksheet missing %q after append-records:\n%s", want, sheetXML)
		}
	}
}

func TestXLSXTablesAppendRowsDryRunIncludesDestinationReadback(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "append-rows", workbookPath,
		"--table", "Sales",
		"--values", `[[{"formula":"SUM(B2:B3)"},"dry"]]`,
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx tables append-rows dry-run failed: %v", err)
	}
	var result XLSXTablesAppendRowsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal append-rows dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected append-rows dry-run metadata: %+v", result)
	}
	assertXLSXTableAppendDestinationForTest(t, result.Destination, "", "A1:B3", "A1:B4", "A4:B4", 4, 3)
	if result.Destination.Appended.Values[0][0] != nil || result.Destination.Appended.Formulas[0][0] != "SUM(B2:B3)" || result.Destination.Appended.Values[0][1] != "dry" {
		t.Fatalf("unexpected append-rows dry-run destination data: %+v", result.Destination.Appended)
	}
	if result.Destination.Appended.Types[0][0] != "number" || result.Destination.Appended.Types[0][1] != "string" {
		t.Fatalf("unexpected append-rows dry-run destination types: %+v", result.Destination.Appended.Types)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "A4:B4")
	assertXLSXTableAppendDryRunTemplatesForTest(t, result.XLSXTableAppendReadbackCommands)

	showOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "tables", "show", workbookPath, "--table", "Sales")
	if err != nil {
		t.Fatalf("xlsx tables show failed: %v", err)
	}
	var showResult XLSXTablesResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal show JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Tables) != 1 || showResult.Tables[0].Range != "A1:B3" {
		t.Fatalf("dry-run append wrote table range: %+v", showResult.Tables)
	}
}

func TestXLSXTablesAppendTextOutputUnchanged(t *testing.T) {
	rowsWorkbook := writeTestXLSXWithTable(t, "A1:B3", false, "")
	rowsPath := filepath.Join(t.TempDir(), "append-rows-text.xlsx")
	rowsOutput, err := executeRootForXLSXTest(t,
		"xlsx", "tables", "append-rows", rowsWorkbook,
		"--table", "Sales",
		"--values", `[["North",30]]`,
		"--out", rowsPath,
	)
	if err != nil {
		t.Fatalf("xlsx tables append-rows text failed: %v", err)
	}
	if strings.TrimSpace(rowsOutput) != "appended 1 rows to Data!Sales: A1:B3 -> A1:B4" {
		t.Fatalf("unexpected append-rows text output: %q", rowsOutput)
	}

	recordsWorkbook := writeTestXLSXWithTable(t, "A1:B3", false, "")
	recordsPath := filepath.Join(t.TempDir(), "append-records-text.xlsx")
	recordsOutput, err := executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", recordsWorkbook,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Region":"North","Amount":30}]`,
		"--out", recordsPath,
	)
	if err != nil {
		t.Fatalf("xlsx tables append-records text failed: %v", err)
	}
	if strings.TrimSpace(recordsOutput) != "appended 1 records to Data!Sales: A1:B3 -> A1:B4" {
		t.Fatalf("unexpected append-records text output: %q", recordsOutput)
	}
}

func assertXLSXTableAppendDestinationForTest(t *testing.T, destination *XLSXTableAppendDestination, filePath, previousRange, tableRange, appendRange string, rows, dataRows int) {
	t.Helper()

	if destination == nil {
		t.Fatal("append result missing destination readback")
	}
	if destination.File != filePath || destination.Table != "Sales" || destination.TablePartURI != "/xl/tables/table1.xml" || destination.RelationshipID != "rId1" {
		t.Fatalf("unexpected table destination identity: %+v", destination)
	}
	if destination.TablePrimarySelector != "tableId:1" || !containsString(destination.TableSelectors, "part:/xl/tables/table1.xml") || !containsString(destination.TableSelectors, "table:Sales") {
		t.Fatalf("destination missing table selectors: %+v", destination)
	}
	if destination.Sheet != "Data" || destination.SheetNumber != 1 || destination.SheetPrimarySelector == "" || !containsString(destination.SheetSelectors, "name:Data") {
		t.Fatalf("destination missing sheet selectors: %+v", destination)
	}
	if destination.PreviousRange != previousRange || destination.Range != tableRange || destination.AppendRange != appendRange {
		t.Fatalf("unexpected table destination ranges: %+v", destination)
	}
	if destination.Rows != rows || destination.Cols != 2 || destination.DataRows != dataRows {
		t.Fatalf("unexpected table destination dimensions: %+v", destination)
	}
	if len(destination.Columns) != 2 || destination.Columns[0] != "Region" || destination.Columns[1] != "Amount" {
		t.Fatalf("unexpected table destination columns: %+v", destination.Columns)
	}
	if destination.Appended == nil {
		t.Fatal("append destination missing appended range readback")
	}
	if destination.Appended.File != filePath || destination.Appended.Sheet != "Data" || destination.Appended.Range != appendRange {
		t.Fatalf("unexpected appended range metadata: %+v", destination.Appended)
	}
	if destination.Appended.SheetPrimarySelector == "" || !containsString(destination.Appended.SheetSelectors, "name:Data") {
		t.Fatalf("appended range missing sheet selectors: %+v", destination.Appended)
	}
	if destination.Appended.Cols != 2 || destination.Appended.FormulaCount < 0 {
		t.Fatalf("unexpected appended range shape: %+v", destination.Appended)
	}
}

func TestXLSXTablesAppendRowsRejectsBadArguments(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	_, err := executeRootForXLSXTest(t,
		"xlsx", "tables", "append-rows", workbookPath,
		"--table", "Sales",
		"--values", `[["Only one column"]]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-rows"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "column count") {
		t.Fatalf("column mismatch error = %v", err)
	}

	overwriteWorkbook := writeTestXLSXWithTable(t, "A1:B3", false, `<row r="4"><c r="A4" t="inlineStr"><is><t>occupied</t></is></c></row>`)
	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-rows", overwriteWorkbook,
		"--table", "Sales",
		"--values", `[["North",30]]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-rows"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "overwrite") {
		t.Fatalf("overwrite error = %v", err)
	}

	totalsWorkbook := writeTestXLSXWithTable(t, "A1:B4", true, "")
	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-rows", totalsWorkbook,
		"--table", "Sales",
		"--values", `[["North",30]]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-rows"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "totals") {
		t.Fatalf("totals error = %v", err)
	}
}

func TestXLSXTablesAppendRecordsRejectsBadArguments(t *testing.T) {
	workbookPath := writeTestXLSXWithTable(t, "A1:B3", false, "")

	_, err := executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", workbookPath,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Region":"North"}]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-records", "missing"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "missing required field") {
		t.Fatalf("missing field error = %v", err)
	}

	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", workbookPath,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Region":"North","Amount":30,"Extra":"x"}]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-records", "extra"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "unknown field") {
		t.Fatalf("extra field error = %v", err)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "tables", "append-records", workbookPath,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Region":"North","Amount":30,"Extra":"ignored"},{"Region":"South","Extra":"ignored"}]`,
		"--ignore-extra-fields",
		"--missing", "skip",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("append-records ignore-extra/missing-skip dry-run failed: %v", err)
	}
	var result XLSXTablesAppendRecordsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal append-records dry-run JSON: %v\n%s", err, output)
	}
	if !result.IgnoredExtraFields || result.MissingPolicy != "skip" || result.Skipped == 0 {
		t.Fatalf("unexpected ignore-extra/missing-skip result: %+v", result)
	}
	assertXLSXMutationDryRunTemplatesForTest(t, result.XLSXMutationReadbackCommands, "A4:B5")
	assertXLSXTableAppendDryRunTemplatesForTest(t, result.XLSXTableAppendReadbackCommands)

	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", workbookPath,
		"--table", "Sales",
		"--expect-range", "A1:B2",
		"--records", `[{"Region":"North","Amount":30}]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-records", "range-mismatch"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "range mismatch") {
		t.Fatalf("range mismatch error = %v", err)
	}

	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", workbookPath,
		"--table", "Sales",
		"--records", `[{"Region":"North","Amount":30}]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-records", "missing-expect-range"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "--expect-range is required") {
		t.Fatalf("missing expect range error = %v", err)
	}

	blankWorkbook := writeTestXLSXWithTableColumns(t, "A1:B3", false, "", []string{"", "Amount"})
	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", blankWorkbook,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Amount":30}]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-records", "blank-column"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "blank name") {
		t.Fatalf("blank column error = %v", err)
	}

	duplicateWorkbook := writeTestXLSXWithTableColumns(t, "A1:B3", false, "", []string{"Region", "Region"})
	_, err = executeRootForXLSXTest(t,
		"xlsx", "tables", "append-records", duplicateWorkbook,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Region":"North"}]`,
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"xlsx", "tables", "append-records", "duplicate-column"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "duplicate table column name") {
		t.Fatalf("duplicate column error = %v", err)
	}
}

func TestXLSXRangesExportJSONDataOutStillPrintsJSON(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1"/>
  <sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Hello</t></is></c></row></sheetData>
</worksheet>`)
	dataOut := filepath.Join(t.TempDir(), "range.json")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "ranges", "export", workbookPath,
		"--sheet", "Sheet1",
		"--range", "A1",
		"--data-format", "json",
		"--data-out", dataOut,
	)
	if err != nil {
		t.Fatalf("xlsx ranges export with data-out failed: %v", err)
	}
	var result XLSXRangesExportResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("stdout was not JSON: %v\n%s", err, output)
	}
	if result.DataOut != dataOut || result.Range != "A1" || result.Values != nil {
		t.Fatalf("unexpected stdout metadata: %+v", result)
	}
	dataOutBody := readFileForXLSXTest(t, dataOut)
	var dataOutResult XLSXRangesExportResult
	if err := json.Unmarshal([]byte(dataOutBody), &dataOutResult); err != nil {
		t.Fatalf("data-out was not JSON: %v\n%s", err, dataOutBody)
	}
	if got := fmt.Sprint(dataOutResult.Values); !strings.Contains(got, "Hello") {
		t.Fatalf("data-out values missing Hello: %#v", dataOutResult.Values)
	}
}

func TestXLSXRangesRejectBadArguments(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	for _, args := range [][]string{
		{"xlsx", "ranges", "export", workbookPath, "--range", "A1"},
		{"xlsx", "ranges", "export", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--max-cells", "1"},
		{"xlsx", "ranges", "set", workbookPath, "--anchor", "A1", "--values", `[["x"]]`, "--dry-run"},
		{"xlsx", "ranges", "set", workbookPath, "--sheet", "Sheet1", "--anchor", "A1", "--range", "A1", "--values", `[["x"]]`, "--dry-run"},
		{"xlsx", "ranges", "set", workbookPath, "--sheet", "Sheet1", "--anchor", "A1", "--values", `[["x"],["y","z"]]`, "--dry-run"},
		{"xlsx", "ranges", "set", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--values", `[["x"]]`, "--dry-run"},
		{"xlsx", "ranges", "set", workbookPath, "--sheet", "Sheet1", "--anchor", "A1", "--values", `[["x","y"]]`, "--max-cells", "1", "--dry-run"},
		{"xlsx", "ranges", "set-format", workbookPath, "--range", "A1", "--preset", "number", "--dry-run"},
		{"xlsx", "ranges", "set-format", workbookPath, "--sheet", "Sheet1", "--preset", "number", "--dry-run"},
		{"xlsx", "ranges", "set-format", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--dry-run"},
		{"xlsx", "ranges", "set-format", workbookPath, "--sheet", "Sheet1", "--range", "A1", "--preset", "number", "--format-code", "0.0", "--dry-run"},
		{"xlsx", "ranges", "set-format", workbookPath, "--sheet", "Sheet1", "--range", "A1:B2", "--preset", "number", "--max-cells", "1", "--dry-run"},
	} {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}

func TestXLSXRowsInsertDeleteJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="2"><c r="A2"><v>2</v></c><c r="B2"><v>3</v></c></row>
    <row r="3"><c r="C3"><v>4</v></c></row>
  </sheetData>
</worksheet>`)
	insertPath := filepath.Join(t.TempDir(), "rows-insert.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rows", "insert", workbookPath,
		"--sheet", "Sheet1",
		"--at", "2",
		"--count", "1",
		"--out", insertPath,
	)
	if err != nil {
		t.Fatalf("xlsx rows insert failed: %v", err)
	}
	var insertResult XLSXStructureMutationResult
	if err := json.Unmarshal([]byte(output), &insertResult); err != nil {
		t.Fatalf("failed to unmarshal rows insert JSON: %v\n%s", err, output)
	}
	if insertResult.File != workbookPath || insertResult.Sheet != "Sheet1" || insertResult.Axis != "rows" || insertResult.Operation != "insert" || insertResult.Start != 2 || insertResult.Count != 1 || insertResult.ShiftedRows != 2 || insertResult.ShiftedCells != 3 || insertResult.NewUsedRange != "A1:C4" {
		t.Fatalf("unexpected rows insert result: %+v", insertResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", insertPath); err != nil {
		t.Fatalf("validate rows insert output failed: %v", err)
	}
	values := readXLSXCellValuesForTest(t, insertPath, "A1:C4")
	assertXLSXCellValue(t, values, "A1", "1")
	assertXLSXCellValue(t, values, "A3", "2")
	assertXLSXCellValue(t, values, "B3", "3")
	assertXLSXCellValue(t, values, "C4", "4")
	assertXLSXNoCell(t, values, "A2")

	deletePath := filepath.Join(t.TempDir(), "rows-delete.xlsx")
	output, err = executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rows", "delete", insertPath,
		"--sheet", "1",
		"--row", "3",
		"--count", "1",
		"--out", deletePath,
	)
	if err != nil {
		t.Fatalf("xlsx rows delete failed: %v", err)
	}
	var deleteResult XLSXStructureMutationResult
	if err := json.Unmarshal([]byte(output), &deleteResult); err != nil {
		t.Fatalf("failed to unmarshal rows delete JSON: %v\n%s", err, output)
	}
	if deleteResult.Axis != "rows" || deleteResult.Operation != "delete" || deleteResult.RemovedRows != 1 || deleteResult.RemovedCells != 2 || deleteResult.ShiftedRows != 1 || deleteResult.ShiftedCells != 1 || deleteResult.NewUsedRange != "A1:C3" {
		t.Fatalf("unexpected rows delete result: %+v", deleteResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", deletePath); err != nil {
		t.Fatalf("validate rows delete output failed: %v", err)
	}
	values = readXLSXCellValuesForTest(t, deletePath, "A1:C3")
	assertXLSXCellValue(t, values, "A1", "1")
	assertXLSXCellValue(t, values, "C3", "4")
	assertXLSXNoCell(t, values, "A3")
}

func TestXLSXColsInsertDeleteJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D2"/>
  <sheetData>
    <row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c><c r="D1"><v>4</v></c></row>
    <row r="2"><c r="C2"><v>3</v></c></row>
  </sheetData>
</worksheet>`)
	insertPath := filepath.Join(t.TempDir(), "cols-insert.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cols", "insert", workbookPath,
		"--sheet", "Sheet1",
		"--at", "b",
		"--count", "1",
		"--out", insertPath,
	)
	if err != nil {
		t.Fatalf("xlsx cols insert failed: %v", err)
	}
	var insertResult XLSXStructureMutationResult
	if err := json.Unmarshal([]byte(output), &insertResult); err != nil {
		t.Fatalf("failed to unmarshal cols insert JSON: %v\n%s", err, output)
	}
	if insertResult.File != workbookPath || insertResult.Axis != "cols" || insertResult.Operation != "insert" || insertResult.Start != 2 || insertResult.StartColumn != "B" || insertResult.Count != 1 || insertResult.ShiftedCells != 3 || insertResult.NewUsedRange != "A1:E2" {
		t.Fatalf("unexpected cols insert result: %+v", insertResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", insertPath); err != nil {
		t.Fatalf("validate cols insert output failed: %v", err)
	}
	values := readXLSXCellValuesForTest(t, insertPath, "A1:E2")
	assertXLSXCellValue(t, values, "A1", "1")
	assertXLSXCellValue(t, values, "C1", "2")
	assertXLSXCellValue(t, values, "E1", "4")
	assertXLSXCellValue(t, values, "D2", "3")
	assertXLSXNoCell(t, values, "B1")

	deletePath := filepath.Join(t.TempDir(), "cols-delete.xlsx")
	output, err = executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cols", "delete", insertPath,
		"--sheet", "1",
		"--col", "C",
		"--count", "1",
		"--out", deletePath,
	)
	if err != nil {
		t.Fatalf("xlsx cols delete failed: %v", err)
	}
	var deleteResult XLSXStructureMutationResult
	if err := json.Unmarshal([]byte(output), &deleteResult); err != nil {
		t.Fatalf("failed to unmarshal cols delete JSON: %v\n%s", err, output)
	}
	if deleteResult.Axis != "cols" || deleteResult.Operation != "delete" || deleteResult.StartColumn != "C" || deleteResult.RemovedCells != 1 || deleteResult.ShiftedCells != 2 || deleteResult.NewUsedRange != "A1:D2" {
		t.Fatalf("unexpected cols delete result: %+v", deleteResult)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", deletePath); err != nil {
		t.Fatalf("validate cols delete output failed: %v", err)
	}
	values = readXLSXCellValuesForTest(t, deletePath, "A1:D2")
	assertXLSXCellValue(t, values, "A1", "1")
	assertXLSXCellValue(t, values, "D1", "4")
	assertXLSXCellValue(t, values, "C2", "3")
	assertXLSXNoCell(t, values, "C1")
}

func TestXLSXStructureDryRunDoesNotWrite(t *testing.T) {
	worksheet := `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c></row></sheetData>
</worksheet>`

	rowsWorkbook := writeTestXLSXWithSheetXML(t, worksheet)
	if _, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "rows", "insert", rowsWorkbook,
		"--sheet", "Sheet1",
		"--at", "1",
		"--dry-run",
	); err != nil {
		t.Fatalf("xlsx rows insert dry-run failed: %v", err)
	}
	values := readXLSXCellValuesForTest(t, rowsWorkbook, "A1:B2")
	assertXLSXCellValue(t, values, "A1", "1")
	assertXLSXCellValue(t, values, "B1", "2")
	assertXLSXNoCell(t, values, "A2")

	colsWorkbook := writeTestXLSXWithSheetXML(t, worksheet)
	if _, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cols", "insert", colsWorkbook,
		"--sheet", "Sheet1",
		"--at", "A",
		"--dry-run",
	); err != nil {
		t.Fatalf("xlsx cols insert dry-run failed: %v", err)
	}
	values = readXLSXCellValuesForTest(t, colsWorkbook, "A1:C1")
	assertXLSXCellValue(t, values, "A1", "1")
	assertXLSXCellValue(t, values, "B1", "2")
	assertXLSXNoCell(t, values, "C1")
}

func TestXLSXStructureRejectsBadArguments(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	for _, tt := range []struct {
		args []string
		code int
	}{
		{[]string{"xlsx", "rows", "insert", workbookPath, "--at", "1", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "rows", "insert", workbookPath, "--sheet", "Sheet1", "--at", "0", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "rows", "insert", workbookPath, "--sheet", "Sheet1", "--at", "1", "--count", "0", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "rows", "insert", workbookPath, "--sheet", "Missing", "--at", "1", "--dry-run"}, ExitTargetNotFound},
		{[]string{"xlsx", "rows", "insert", workbookPath, "--sheet", "Sheet1", "--at", "1048576", "--count", "2", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "cols", "insert", workbookPath, "--at", "A", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "cols", "insert", workbookPath, "--sheet", "Sheet1", "--at", "A1", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "cols", "delete", workbookPath, "--sheet", "Sheet1", "--col", "XFE", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "cols", "insert", workbookPath, "--sheet", "Sheet1", "--at", "XFD", "--count", "2", "--dry-run"}, ExitInvalidArgs},
	} {
		_, err := executeRootForXLSXTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
	}
}

func TestXLSXStructureRejectsHazards(t *testing.T) {
	formulaWorkbook := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><f>SUM(B1)</f></c></row></sheetData>
</worksheet>`)
	_, err := executeRootForXLSXTest(t,
		"xlsx", "rows", "insert", formulaWorkbook,
		"--sheet", "Sheet1",
		"--at", "1",
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"formula hazard"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "worksheet has formulas") {
		t.Fatalf("formula hazard error = %v, want formula message", err)
	}

	colsWorkbook := writeTestXLSXWithSheetXML(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cols><col min="1" max="1" width="12" customWidth="1"/></cols>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>`)
	_, err = executeRootForXLSXTest(t,
		"xlsx", "cols", "insert", colsWorkbook,
		"--sheet", "Sheet1",
		"--at", "A",
		"--dry-run",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"column metadata hazard"}, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "worksheet has column metadata") {
		t.Fatalf("column metadata hazard error = %v, want column metadata message", err)
	}
}

func TestXLSXCellsSetRejectsBadArguments(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "bad.xlsx")

	for _, args := range [][]string{
		{"xlsx", "cells", "set", workbookPath, "--value", "x", "--out", outPath},
		{"xlsx", "cells", "set", workbookPath, "--cell", "A1:B2", "--value", "x", "--out", outPath},
		{"xlsx", "cells", "set", workbookPath, "--cell", "A1", "--value", "x", "--type", "date", "--out", outPath},
		{"xlsx", "cells", "set", workbookPath, "--cell", "A1", "--value", "", "--out", outPath},
		{"xlsx", "cells", "set-batch", workbookPath, "--cells", "[]", "--out", outPath},
		{"xlsx", "cells", "set-batch", workbookPath, "--cells", `[{"ref":"A1","value":"x"}]`, "--cells-file", "cells.json", "--out", outPath},
	} {
		_, err := executeRootForXLSXTest(t, args...)
		if err == nil {
			t.Fatalf("%v: expected invalid args error", args)
		}
		cliErr, ok := err.(*CLIError)
		if !ok {
			t.Fatalf("%v: error type = %T, want *CLIError", args, err)
		}
		if cliErr.ExitCode != ExitInvalidArgs {
			t.Fatalf("%v: exit code = %d, want %d", args, cliErr.ExitCode, ExitInvalidArgs)
		}
	}
}

func TestXLSXSheetsShowComputesUsedRange(t *testing.T) {
	workbookPath := getXLSXTestFilePath("used-range")

	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "show", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets show failed: %v", err)
	}

	var result XLSXSheetsShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal output JSON: %v\n%s", err, output)
	}
	report := result.Sheets[0]
	if report.DimensionDeclared != "A1:Z100" || report.UsedRange.Ref != "B2:D5" {
		t.Fatalf("unexpected range report: %+v", report)
	}
}

func TestXLSXNamesListShowJSONAndGeneratedCommands(t *testing.T) {
	workbookPath := writeTestXLSXWithWorkbookMetadata(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
		{Name: "Tail", SheetID: "3"},
	}, false)

	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "names", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx names list failed: %v", err)
	}
	var result XLSXNamesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal names list JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || len(result.Names) != 4 || result.ValidateCommand == "" {
		t.Fatalf("unexpected names list result: %+v", result)
	}
	global := result.Names[0]
	if global.Name != "GlobalName" || global.Scope != "workbook" || global.Ref != "Summary!$A$1" || global.PrimarySelector != "name:GlobalName" {
		t.Fatalf("unexpected workbook defined name: %+v", global)
	}
	local := result.Names[2]
	if local.Name != "LocalData" || local.Scope != "sheet" || local.SheetNumber != 2 || local.SheetName != "Data" || local.Ref != "Data!$A$1" {
		t.Fatalf("unexpected local defined name: %+v", local)
	}
	if !containsString(local.Selectors, "sheet:2/name:LocalData") || local.ShowCommand == "" {
		t.Fatalf("local defined name missing selectors/show command: %+v", local)
	}
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, local.ShowCommand)
	var showResult XLSXNamesShowResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal generated names show JSON: %v\n%s", err, showOutput)
	}
	if showResult.Name.Name != "LocalData" || showResult.Name.Scope != "sheet" {
		t.Fatalf("unexpected generated show result: %+v", showResult)
	}
	executeGeneratedOOXMLCommandForXLSXTest(t, result.ValidateCommand)

	filteredOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "names", "list", workbookPath, "--scope-sheet", "Data")
	if err != nil {
		t.Fatalf("xlsx names list --scope-sheet failed: %v", err)
	}
	var filtered XLSXNamesListResult
	if err := json.Unmarshal([]byte(filteredOutput), &filtered); err != nil {
		t.Fatalf("failed to unmarshal filtered names list JSON: %v\n%s", err, filteredOutput)
	}
	if len(filtered.Names) != 1 || filtered.Names[0].Name != "LocalData" {
		t.Fatalf("unexpected scoped names list: %+v", filtered.Names)
	}
}

func TestXLSXNamesMutationWorkflowReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	addPath := filepath.Join(t.TempDir(), "names-add.xlsx")
	updatePath := filepath.Join(t.TempDir(), "names-update.xlsx")
	renamePath := filepath.Join(t.TempDir(), "names-rename.xlsx")
	deletePath := filepath.Join(t.TempDir(), "names-delete.xlsx")
	initialRef := "'Sheet1'!$A$1:$B$2"
	updatedRef := "SUM('Sheet1'!$B$1:$B$2)"

	addOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "names", "add", workbookPath,
		"--name", "SalesData",
		"--sheet", "Sheet1",
		"--range", "A1:B2",
		"--out", addPath,
	)
	if err != nil {
		t.Fatalf("xlsx names add failed: %v", err)
	}
	var addResult XLSXNameMutationResult
	if err := json.Unmarshal([]byte(addOutput), &addResult); err != nil {
		t.Fatalf("failed to unmarshal names add JSON: %v\n%s", err, addOutput)
	}
	if addResult.Output != addPath || addResult.DryRun || addResult.Name == nil || addResult.Name.Name != "SalesData" || addResult.Name.Ref != initialRef {
		t.Fatalf("unexpected add result: %+v", addResult)
	}
	assertXLSXNameSavedCommandsForTest(t, addResult.XLSXNameMutationReadbackCommands, addPath, "SalesData")
	if addResult.Name.ShowCommand == "" {
		t.Fatalf("add result missing name show command: %+v", addResult.Name)
	}

	updateOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "names", "update", addPath,
		"--name", "SalesData",
		"--ref", updatedRef,
		"--expect-ref", initialRef,
		"--out", updatePath,
	)
	if err != nil {
		t.Fatalf("xlsx names update failed: %v", err)
	}
	var updateResult XLSXNameMutationResult
	if err := json.Unmarshal([]byte(updateOutput), &updateResult); err != nil {
		t.Fatalf("failed to unmarshal names update JSON: %v\n%s", err, updateOutput)
	}
	if updateResult.PreviousRef != initialRef || updateResult.Name == nil || updateResult.Name.Ref != updatedRef {
		t.Fatalf("unexpected update result: %+v", updateResult)
	}
	assertXLSXNameSavedCommandsForTest(t, updateResult.XLSXNameMutationReadbackCommands, updatePath, "SalesData")

	renameOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "names", "rename", updatePath,
		"--name", "SalesData",
		"--new-name", "RevenueData",
		"--expect-ref", updatedRef,
		"--out", renamePath,
	)
	if err != nil {
		t.Fatalf("xlsx names rename failed: %v", err)
	}
	var renameResult XLSXNameMutationResult
	if err := json.Unmarshal([]byte(renameOutput), &renameResult); err != nil {
		t.Fatalf("failed to unmarshal names rename JSON: %v\n%s", err, renameOutput)
	}
	if renameResult.PreviousName != "SalesData" || renameResult.Name == nil || renameResult.Name.Name != "RevenueData" || renameResult.Name.Ref != updatedRef {
		t.Fatalf("unexpected rename result: %+v", renameResult)
	}
	assertXLSXNameSavedCommandsForTest(t, renameResult.XLSXNameMutationReadbackCommands, renamePath, "RevenueData")

	deleteOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "names", "delete", renamePath,
		"--name", "RevenueData",
		"--expect-ref", updatedRef,
		"--out", deletePath,
	)
	if err != nil {
		t.Fatalf("xlsx names delete failed: %v", err)
	}
	var deleteResult XLSXNameMutationResult
	if err := json.Unmarshal([]byte(deleteOutput), &deleteResult); err != nil {
		t.Fatalf("failed to unmarshal names delete JSON: %v\n%s", err, deleteOutput)
	}
	if deleteResult.Deleted == nil || deleteResult.Deleted.Name != "RevenueData" || deleteResult.Name != nil {
		t.Fatalf("unexpected delete result: %+v", deleteResult)
	}
	if deleteResult.NamesListCommand == "" || deleteResult.NameShowCommand != "" {
		t.Fatalf("unexpected delete readback commands: %+v", deleteResult.XLSXNameMutationReadbackCommands)
	}
	listAfterDelete := executeGeneratedOOXMLCommandForXLSXTest(t, deleteResult.NamesListCommand)
	var afterDelete XLSXNamesListResult
	if err := json.Unmarshal([]byte(listAfterDelete), &afterDelete); err != nil {
		t.Fatalf("failed to unmarshal post-delete names list JSON: %v\n%s", err, listAfterDelete)
	}
	if len(afterDelete.Names) != 0 {
		t.Fatalf("deleted name still present: %+v", afterDelete.Names)
	}
	if workbookXML := readZipEntryForTest(t, deletePath, "xl/workbook.xml"); strings.Contains(workbookXML, "definedNames") {
		t.Fatalf("empty definedNames element should be removed:\n%s", workbookXML)
	}
}

func TestXLSXNamesDryRunLocalNameDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "names", "add", workbookPath,
		"--name", "LocalInput",
		"--ref", "Sheet1!$A$1",
		"--scope-sheet", "Sheet1",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx names add dry-run failed: %v", err)
	}
	var result XLSXNameMutationResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal names add dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.Name == nil || result.Name.Scope != "sheet" || result.Name.SheetNumber != 1 {
		t.Fatalf("unexpected dry-run name result: %+v", result)
	}
	if result.ValidateCommandTemplate == "" || result.NamesListCommandTemplate == "" || result.NameShowCommandTemplate == "" {
		t.Fatalf("dry-run missing readback templates: %+v", result.XLSXNameMutationReadbackCommands)
	}
	if !strings.Contains(result.NameShowCommandTemplate, "<out.xlsx>") || !strings.Contains(result.NameShowCommandTemplate, "--scope-sheet sheet:1") {
		t.Fatalf("unexpected dry-run show template: %s", result.NameShowCommandTemplate)
	}
	if workbookXML := readZipEntryForTest(t, workbookPath, "xl/workbook.xml"); strings.Contains(workbookXML, "definedNames") {
		t.Fatalf("dry-run wrote definedNames into source workbook:\n%s", workbookXML)
	}
}

func TestXLSXNamesRejectBadArgumentsAndStaleGuards(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "names-bad.xlsx")

	for _, args := range [][]string{
		{"xlsx", "names", "show", workbookPath},
		{"xlsx", "names", "add", workbookPath, "--ref", "Sheet1!$A$1", "--dry-run"},
		{"xlsx", "names", "add", workbookPath, "--name", "A1", "--ref", "Sheet1!$A$1", "--dry-run"},
		{"xlsx", "names", "add", workbookPath, "--name", "Bad Name", "--ref", "Sheet1!$A$1", "--dry-run"},
		{"xlsx", "names", "add", workbookPath, "--name", "Input", "--ref", "Sheet1!$A$1", "--range", "A1", "--dry-run"},
		{"xlsx", "names", "add", workbookPath, "--name", "Input", "--range", "A1", "--dry-run"},
	} {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
	missingArgs := []string{"xlsx", "names", "update", workbookPath, "--name", "Missing", "--ref", "Sheet1!$A$1", "--dry-run"}
	_, err := executeRootForXLSXTest(t, missingArgs...)
	assertCLIExitCodeForXLSXTest(t, missingArgs, err, ExitTargetNotFound)

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "names", "add", workbookPath,
		"--name", "Input",
		"--ref", "Sheet1!$A$1",
		"--out", outPath,
	); err != nil {
		t.Fatalf("xlsx names add for stale guard setup failed: %v", err)
	}
	args := []string{
		"xlsx", "names", "update", outPath,
		"--name", "Input",
		"--ref", "Sheet1!$A$2",
		"--expect-ref", "Sheet1!$A$99",
		"--dry-run",
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "ref mismatch") {
		t.Fatalf("stale guard error = %v", err)
	}
}

func TestInspectDispatchesXLSXJSON(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{
		{Name: "Summary", SheetID: "1"},
		{Name: "Data", SheetID: "2"},
	})

	output, err := executeRootForXLSXTest(t, "--format", "json", "inspect", workbookPath)
	if err != nil {
		t.Fatalf("inspect failed: %v", err)
	}

	var result XLSXInspectResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, output)
	}

	if result.Type != "xlsx" {
		t.Fatalf("type = %q, want xlsx", result.Type)
	}
	if result.File != workbookPath {
		t.Fatalf("file = %q, want %q", result.File, workbookPath)
	}
	if result.Summary == nil {
		t.Fatal("summary is nil")
	}
	if result.Summary.Sheets != 2 || result.Summary.Worksheets != 2 {
		t.Fatalf("summary counts = sheets %d worksheets %d, want 2/2", result.Summary.Sheets, result.Summary.Worksheets)
	}
}

func TestXLSXCommandsRejectNonXLSX(t *testing.T) {
	packagePath := writeUnknownOPCPackage(t)

	for _, args := range [][]string{
		{"xlsx", "sheets", "list", packagePath},
		{"xlsx", "sheets", "show", packagePath},
		{"xlsx", "names", "list", packagePath},
		{"xlsx", "names", "show", packagePath, "--name", "Input"},
		{"xlsx", "names", "add", packagePath, "--name", "Input", "--ref", "Sheet1!$A$1", "--dry-run"},
		{"xlsx", "cells", "extract", packagePath},
		{"xlsx", "cells", "set", packagePath, "--cell", "A1", "--value", "x", "--out", filepath.Join(t.TempDir(), "out.xlsx")},
		{"xlsx", "cells", "set-batch", packagePath, "--cells", `[{"ref":"A1","value":"x"}]`, "--out", filepath.Join(t.TempDir(), "out.xlsx")},
		{"xlsx", "cells", "clear", packagePath, "--range", "A1", "--out", filepath.Join(t.TempDir(), "out.xlsx")},
		{"xlsx", "ranges", "export", packagePath, "--sheet", "Sheet1", "--range", "A1"},
		{"xlsx", "ranges", "set", packagePath, "--sheet", "Sheet1", "--anchor", "A1", "--values", `[["x"]]`, "--dry-run"},
		{"xlsx", "ranges", "set-format", packagePath, "--sheet", "Sheet1", "--range", "A1", "--preset", "number", "--dry-run"},
		{"xlsx", "rows", "insert", packagePath, "--sheet", "Sheet1", "--at", "1", "--dry-run"},
		{"xlsx", "rows", "delete", packagePath, "--sheet", "Sheet1", "--row", "1", "--dry-run"},
		{"xlsx", "cols", "insert", packagePath, "--sheet", "Sheet1", "--at", "A", "--dry-run"},
		{"xlsx", "cols", "delete", packagePath, "--sheet", "Sheet1", "--col", "A", "--dry-run"},
	} {
		_, err := executeRootForXLSXTest(t, args...)
		if err == nil {
			t.Fatalf("%v: expected unsupported type error", args)
		}

		cliErr, ok := err.(*CLIError)
		if !ok {
			t.Fatalf("%v: error type = %T, want *CLIError", args, err)
		}
		if cliErr.ExitCode != ExitUnsupportedType {
			t.Fatalf("%v: exit code = %d, want %d", args, cliErr.ExitCode, ExitUnsupportedType)
		}
	}
}

func findSubcommand(cmd *cobra.Command, name string) *cobra.Command {
	for _, sub := range cmd.Commands() {
		if sub.Name() == name {
			return sub
		}
	}
	return nil
}

type testSheet struct {
	Name    string
	SheetID string
	State   string
}

func executeRootForXLSXTest(t *testing.T, args ...string) (string, error) {
	return executeRootForXLSXTestWithInput(t, nil, args...)
}

func executeRootForXLSXTestWithInput(t *testing.T, input io.Reader, args ...string) (string, error) {
	t.Helper()
	resetRootFlagsForXLSXTest(t)
	ensureExampleMetadata()

	cmd := GetRootCmd()
	cmd.SetArgs(args)
	if input != nil {
		cmd.SetIn(input)
	}

	var output bytes.Buffer
	cmd.SetOut(&output)
	cmd.SetErr(&bytes.Buffer{})

	err := cmd.Execute()
	return output.String(), err
}

func resetRootFlagsForXLSXTest(t *testing.T) {
	t.Helper()

	resetFlags()
	flagOut = ""
	flagInPlace = ""
	flagBackup = ""

	cmd := GetRootCmd()
	values := map[string]string{
		"format":    "text",
		"verbosity": "normal",
		"output":    "",
		"temp-dir":  "",
		"out":       "",
		"in-place":  "",
		"backup":    "",
	}
	for name, value := range values {
		if err := cmd.PersistentFlags().Set(name, value); err != nil {
			t.Fatalf("failed to reset --%s: %v", name, err)
		}
	}

	for _, name := range []string{"json", "no-color", "pretty", "keep-temp", "strict"} {
		if err := cmd.PersistentFlags().Set(name, "false"); err != nil {
			t.Fatalf("failed to reset --%s: %v", name, err)
		}
	}
	resetCommandFlagsForXLSXTest(t, xlsxCmd)
	resetCommandFlagsForXLSXTest(t, docxCmd)
	resetCommandFlagsForXLSXTest(t, chartsCmd)
	resetCommandFlagsForXLSXTest(t, layoutsCmd)
	resetCommandFlagsForXLSXTest(t, mastersCmd)
	resetCommandFlagsForXLSXTest(t, slidesCmd)
	resetCommandFlagsForXLSXTest(t, cloneSlideCmd)
	resetCommandFlagsForXLSXTest(t, tablesCmd)
	resetCommandFlagsForXLSXTest(t, pptxAnimationsCmd)
	resetCommandFlagsForXLSXTest(t, pptxMediaCmd)
	resetCommandFlagsForXLSXTest(t, placeCmd)
	resetCommandFlagsForXLSXTest(t, replaceCmd)
	resetCommandFlagsForXLSXTest(t, pptxXLSXBindingsCmd)
	resetCommandFlagsForXLSXTest(t, applyCmd)
	resetCommandFlagsForXLSXTest(t, capabilitiesCmd)
	resetCommandFlagsForXLSXTest(t, conformanceCmd)
	resetCommandFlagsForXLSXTest(t, vbaCmd)
}

func resetCommandFlagsForXLSXTest(t *testing.T, cmd *cobra.Command) {
	t.Helper()

	cmd.Flags().VisitAll(func(flag *pflag.Flag) {
		if err := cmd.Flags().Set(flag.Name, flag.DefValue); err != nil {
			t.Fatalf("failed to reset %s --%s: %v", cmd.CommandPath(), flag.Name, err)
		}
		flag.Changed = false
	})
	for _, child := range cmd.Commands() {
		resetCommandFlagsForXLSXTest(t, child)
	}
}

func assertCLIExitCodeForXLSXTest(t *testing.T, args []string, err error, want int) {
	t.Helper()
	if err == nil {
		t.Fatalf("%v: expected exit code %d error", args, want)
	}
	cliErr, ok := err.(*CLIError)
	if !ok {
		t.Fatalf("%v: error type = %T, want *CLIError", args, err)
	}
	if cliErr.ExitCode != want {
		t.Fatalf("%v: exit code = %d, want %d", args, cliErr.ExitCode, want)
	}
}

func readDelimitedRecordsForTest(t *testing.T, path string, comma rune) [][]string {
	t.Helper()
	file, err := os.Open(path)
	if err != nil {
		t.Fatalf("failed to open delimited file: %v", err)
	}
	defer file.Close()
	reader := csv.NewReader(file)
	reader.Comma = comma
	reader.FieldsPerRecord = -1
	records, err := reader.ReadAll()
	if err != nil {
		t.Fatalf("failed to read delimited file: %v", err)
	}
	return records
}

func readFileForXLSXTest(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read %s: %v", path, err)
	}
	return string(data)
}

func writeTestXLSX(t *testing.T, sheets []testSheet) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "workbook.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", contentTypesXML(sheets))
	addZipFile(t, zw, "_rels/.rels", rootRelsXML())
	addZipFile(t, zw, "xl/workbook.xml", workbookXML(sheets))
	addZipFile(t, zw, "xl/_rels/workbook.xml.rels", workbookRelsXML(sheets))
	for i := range sheets {
		addZipFile(t, zw, fmt.Sprintf("xl/worksheets/sheet%d.xml", i+1), worksheetXML())
	}
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}

	return path
}

func writeTestXLSXWithSheetXML(t *testing.T, sheetXML string) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "workbook.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	sheets := []testSheet{{Name: "Sheet1", SheetID: "1"}}
	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", contentTypesXML(sheets))
	addZipFile(t, zw, "_rels/.rels", rootRelsXML())
	addZipFile(t, zw, "xl/workbook.xml", workbookXML(sheets))
	addZipFile(t, zw, "xl/_rels/workbook.xml.rels", workbookRelsXML(sheets))
	addZipFile(t, zw, "xl/worksheets/sheet1.xml", sheetXML)
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}

	return path
}

func writeTestXLSXWithTable(t *testing.T, tableRef string, totals bool, extraRows string) string {
	return writeTestXLSXWithTableColumns(t, tableRef, totals, extraRows, []string{"Region", "Amount"})
}

func writeTestXLSXWithTableColumns(t *testing.T, tableRef string, totals bool, extraRows string, tableColumns []string) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "table-workbook.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	totalsAttrs := `totalsRowShown="0"`
	totalsRow := ""
	if totals {
		totalsAttrs = `totalsRowShown="1" totalsRowCount="1"`
		totalsRow = `<row r="4"><c r="A4" t="inlineStr"><is><t>Total</t></is></c><c r="B4"><v>30</v></c></row>`
	}
	dimension := tableRef
	if extraRows != "" && !strings.Contains(dimension, "B4") {
		dimension = "A1:B4"
	}

	worksheet := fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="%s"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2"><v>10</v></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3"><v>20</v></c></row>
    %s
    %s
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>`, dimension, totalsRow, extraRows)
	tableXML := fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="%s" headerRowCount="1" %s>
  <autoFilter ref="%s"/>
  %s
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>`, tableRef, totalsAttrs, tableRef, testXLSXTableColumnsXML(tableColumns))

	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>`)
	addZipFile(t, zw, "_rels/.rels", rootRelsXML())
	addZipFile(t, zw, "xl/workbook.xml", workbookXML([]testSheet{{Name: "Data", SheetID: "1"}}))
	addZipFile(t, zw, "xl/_rels/workbook.xml.rels", workbookRelsXML([]testSheet{{Name: "Data", SheetID: "1"}}))
	addZipFile(t, zw, "xl/worksheets/sheet1.xml", worksheet)
	addZipFile(t, zw, "xl/worksheets/_rels/sheet1.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>`)
	addZipFile(t, zw, "xl/tables/table1.xml", tableXML)
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}

	return path
}

func testXLSXTableColumnsXML(columnNames []string) string {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf(`<tableColumns count="%d">`, len(columnNames)))
	for idx, name := range columnNames {
		builder.WriteString(fmt.Sprintf(`
    <tableColumn id="%d" name="%s"/>`, idx+1, name))
	}
	builder.WriteString(`
  </tableColumns>`)
	return builder.String()
}

func writeTestXLSXWithPivot(t *testing.T, twoPivots bool) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "pivot-workbook.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	pivotSheetRefs := `<pivotTableDefinition r:id="rIdPivot1"/>`
	pivotSheetRels := `<Relationship Id="rIdPivot1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable1.xml"/>`
	pivotOverrides := `<Override PartName="/xl/pivotTables/pivotTable1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>`
	if twoPivots {
		pivotSheetRefs += `
  <pivotTableDefinition r:id="rIdPivot2"/>`
		pivotSheetRels += `
  <Relationship Id="rIdPivot2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable" Target="../pivotTables/pivotTable2.xml"/>`
		pivotOverrides += `
  <Override PartName="/xl/pivotTables/pivotTable2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml"/>`
	}

	worksheet := fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:E6"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Quarter</t></is></c><c r="C1" t="inlineStr"><is><t>Amount</t></is></c><c r="D1" t="inlineStr"><is><t>Segment</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2" t="inlineStr"><is><t>Q1</t></is></c><c r="C2"><v>10</v></c><c r="D2" t="inlineStr"><is><t>Enterprise</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3" t="inlineStr"><is><t>Q2</t></is></c><c r="C3"><v>20</v></c><c r="D3" t="inlineStr"><is><t>SMB</t></is></c></row>
  </sheetData>
  %s
</worksheet>`, pivotSheetRefs)

	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  %s
  <Override PartName="/xl/pivotCache/pivotCacheDefinition1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml"/>
  <Override PartName="/xl/pivotCache/pivotCacheRecords1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml"/>
</Types>`, pivotOverrides))
	addZipFile(t, zw, "_rels/.rels", rootRelsXML())
	addZipFile(t, zw, "xl/workbook.xml", `<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
  </pivotCaches>
</workbook>`)
	addZipFile(t, zw, "xl/_rels/workbook.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdCache1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition" Target="pivotCache/pivotCacheDefinition1.xml"/>
</Relationships>`)
	addZipFile(t, zw, "xl/worksheets/sheet1.xml", worksheet)
	addZipFile(t, zw, "xl/worksheets/_rels/sheet1.xml.rels", fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  %s
</Relationships>`, pivotSheetRels))
	addZipFile(t, zw, "xl/pivotTables/pivotTable1.xml", testPivotTableXML("SalesPivot", "D3:E6"))
	if twoPivots {
		addZipFile(t, zw, "xl/pivotTables/pivotTable2.xml", testPivotTableXML("SalesPivot2", "G3:H6"))
	}
	addZipFile(t, zw, "xl/pivotCache/pivotCacheDefinition1.xml", `<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" recordCount="2" createdVersion="6" refreshedVersion="6" refreshOnLoad="1" saveData="1">
  <cacheSource type="worksheet">
    <worksheetSource ref="A1:D3" sheet="Data"/>
  </cacheSource>
  <cacheFields count="4">
    <cacheField name="Region"><sharedItems count="2"/></cacheField>
    <cacheField name="Quarter"><sharedItems count="2"/></cacheField>
    <cacheField name="Amount" numFmtId="0"><sharedItems containsNumber="1" count="2"/></cacheField>
    <cacheField name="Segment"><sharedItems count="2"/></cacheField>
  </cacheFields>
</pivotCacheDefinition>`)
	addZipFile(t, zw, "xl/pivotCache/_rels/pivotCacheDefinition1.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdRecords1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords" Target="pivotCacheRecords1.xml"/>
</Relationships>`)
	addZipFile(t, zw, "xl/pivotCache/pivotCacheRecords1.xml", `<?xml version="1.0" encoding="UTF-8"?>
<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2">
  <r><s v="East"/><s v="Q1"/><n v="10"/><s v="Enterprise"/></r>
  <r><s v="West"/><s v="Q2"/><n v="20"/><s v="SMB"/></r>
</pivotCacheRecords>`)
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}

	return path
}

func testPivotTableXML(name, location string) string {
	return fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="%s" cacheId="1" dataCaption="Values" updatedVersion="6" minRefreshableVersion="3">
  <location ref="%s" firstHeaderRow="1" firstDataRow="2" firstDataCol="1"/>
  <pivotFields count="4">
    <pivotField axis="axisRow" showAll="0"/>
    <pivotField axis="axisCol" showAll="0"/>
    <pivotField dataField="1"/>
    <pivotField axis="axisPage" showAll="0"/>
  </pivotFields>
  <rowFields count="1"><field x="0"/></rowFields>
  <colFields count="1"><field x="1"/></colFields>
  <pageFields count="1"><pageField fld="3" hier="-1"/></pageFields>
  <dataFields count="1"><dataField name="Sum of Amount" fld="2" subtotal="sum"/></dataFields>
</pivotTableDefinition>`, name, location)
}

func writeTestXLSXWithWorkbookMetadata(t *testing.T, sheets []testSheet, includeCalcChain bool) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "workbook-rich.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", contentTypesXMLWithExtras(sheets, includeCalcChain))
	addZipFile(t, zw, "_rels/.rels", rootRelsXML())
	addZipFile(t, zw, "xl/workbook.xml", workbookXMLWithMetadata(sheets))
	addZipFile(t, zw, "xl/_rels/workbook.xml.rels", workbookRelsXMLWithExtras(sheets, includeCalcChain))
	for i := range sheets {
		addZipFile(t, zw, fmt.Sprintf("xl/worksheets/sheet%d.xml", i+1), worksheetXML())
	}
	addZipFile(t, zw, "xl/worksheets/_rels/sheet2.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`)
	if includeCalcChain {
		addZipFile(t, zw, "xl/calcChain.xml", `<?xml version="1.0" encoding="UTF-8"?>
<calcChain xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><c r="A1" i="2"/></calcChain>`)
	}
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}

	return path
}

func writeUnknownOPCPackage(t *testing.T) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "unknown.opc")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create package: %v", err)
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/custom/item.xml" ContentType="application/xml"/>
</Types>`)
	addZipFile(t, zw, "_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`)
	addZipFile(t, zw, "custom/item.xml", `<item/>`)
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close package: %v", err)
	}

	return path
}

func addZipFile(t *testing.T, zw *zip.Writer, name string, body string) {
	t.Helper()

	writer, err := zw.Create(name)
	if err != nil {
		t.Fatalf("failed to create zip entry %s: %v", name, err)
	}
	if _, err := writer.Write([]byte(body)); err != nil {
		t.Fatalf("failed to write zip entry %s: %v", name, err)
	}
}

func contentTypesXML(sheets []testSheet) string {
	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
`)
	for i := range sheets {
		builder.WriteString(fmt.Sprintf(`  <Override PartName="/xl/worksheets/sheet%d.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
`, i+1))
	}
	builder.WriteString(`</Types>`)
	return builder.String()
}

func contentTypesXMLWithExtras(sheets []testSheet, includeCalcChain bool) string {
	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
`)
	for i := range sheets {
		builder.WriteString(fmt.Sprintf(`  <Override PartName="/xl/worksheets/sheet%d.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
`, i+1))
	}
	if includeCalcChain {
		builder.WriteString(`  <Override PartName="/xl/calcChain.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"/>
`)
	}
	builder.WriteString(`</Types>`)
	return builder.String()
}

func rootRelsXML() string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>`
}

func workbookXML(sheets []testSheet) string {
	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
`)
	for i, sheet := range sheets {
		state := ""
		if sheet.State != "" {
			state = fmt.Sprintf(` state="%s"`, sheet.State)
		}
		builder.WriteString(fmt.Sprintf(`    <sheet name="%s" sheetId="%s"%s r:id="rId%d"/>
`, sheet.Name, sheet.SheetID, state, i+1))
	}
	builder.WriteString(`  </sheets>
</workbook>`)
	return builder.String()
}

func workbookXMLWithMetadata(sheets []testSheet) string {
	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <bookViews><workbookView activeTab="2" firstSheet="0"/></bookViews>
  <sheets>
`)
	for i, sheet := range sheets {
		state := ""
		if sheet.State != "" {
			state = fmt.Sprintf(` state="%s"`, sheet.State)
		}
		builder.WriteString(fmt.Sprintf(`    <sheet name="%s" sheetId="%s"%s r:id="rId%d"/>
`, sheet.Name, sheet.SheetID, state, i+1))
	}
	builder.WriteString(`  </sheets>
  <definedNames>
    <definedName name="GlobalName">Summary!$A$1</definedName>
    <definedName name="LocalSummary" localSheetId="0">Summary!$A$1</definedName>
    <definedName name="LocalData" localSheetId="1">Data!$A$1</definedName>
    <definedName name="LocalTail" localSheetId="2">Tail!$A$1</definedName>
  </definedNames>
</workbook>`)
	return builder.String()
}

func workbookRelsXML(sheets []testSheet) string {
	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
`)
	for i := range sheets {
		builder.WriteString(fmt.Sprintf(`  <Relationship Id="rId%d" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet%d.xml"/>
`, i+1, i+1))
	}
	builder.WriteString(`</Relationships>`)
	return builder.String()
}

func workbookRelsXMLWithExtras(sheets []testSheet, includeCalcChain bool) string {
	var builder strings.Builder
	builder.WriteString(`<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
`)
	for i := range sheets {
		builder.WriteString(fmt.Sprintf(`  <Relationship Id="rId%d" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet%d.xml"/>
`, i+1, i+1))
	}
	if includeCalcChain {
		builder.WriteString(`  <Relationship Id="rIdCalc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain" Target="calcChain.xml"/>
`)
	}
	builder.WriteString(`</Relationships>`)
	return builder.String()
}

func worksheetXML() string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
</worksheet>`
}

func sheetNamesForCLI(sheets []model.SheetRef) string {
	names := make([]string, 0, len(sheets))
	for _, sheet := range sheets {
		names = append(names, sheet.Name)
	}
	return strings.Join(names, ",")
}

func sheetListItemNamesForCLI(sheets []XLSXSheetListItem) string {
	names := make([]string, 0, len(sheets))
	for _, sheet := range sheets {
		names = append(names, sheet.Name)
	}
	return strings.Join(names, ",")
}

func assertXLSXSheetsDestinationForTest(t *testing.T, destination *XLSXSheetsMutationDestination, filePath, names, sheetName string) {
	t.Helper()
	if destination == nil {
		t.Fatal("expected sheet mutation destination readback")
	}
	if destination.File != filePath {
		t.Fatalf("destination file = %q, want %q", destination.File, filePath)
	}
	if got := sheetNamesForCLI(destination.Sheets); got != names {
		t.Fatalf("destination sheet order = %q, want %q: %+v", got, names, destination.Sheets)
	}
	if destination.SheetCount != len(destination.Sheets) {
		t.Fatalf("destination sheetCount = %d, len(sheets) = %d", destination.SheetCount, len(destination.Sheets))
	}
	for idx, sheet := range destination.Sheets {
		expectedPosition := idx + 1
		if sheet.Number != expectedPosition || sheet.Position != expectedPosition {
			t.Fatalf("destination sheet %q number/position = %d/%d, want %d/%d: %+v", sheet.Name, sheet.Number, sheet.Position, expectedPosition, expectedPosition, sheet)
		}
		assertXLSXSheetRefForTest(t, sheet, sheet.Name)
	}
	if sheetName == "" {
		if destination.Sheet != nil {
			t.Fatalf("destination sheet = %+v, want nil", destination.Sheet)
		}
		return
	}
	if destination.Sheet == nil {
		t.Fatalf("destination sheet missing, want %q", sheetName)
	}
	assertXLSXSheetRefForTest(t, *destination.Sheet, sheetName)
}

func assertXLSXSheetRefForTest(t *testing.T, sheet model.SheetRef, sheetName string) {
	t.Helper()
	if sheet.Name != sheetName {
		t.Fatalf("sheet name = %q, want %q: %+v", sheet.Name, sheetName, sheet)
	}
	if sheet.SheetID != "" && sheet.PrimarySelector != "sheetId:"+sheet.SheetID {
		t.Fatalf("sheet primary selector = %q, want sheetId:%s: %+v", sheet.PrimarySelector, sheet.SheetID, sheet)
	}
	if sheet.PrimarySelector == "" && sheet.SheetID == "" {
		t.Fatalf("sheet primary selector missing: %+v", sheet)
	}
	if sheet.Number > 0 {
		for _, want := range []string{fmt.Sprintf("sheet:%d", sheet.Number), fmt.Sprintf("#%d", sheet.Number)} {
			if !containsString(sheet.Selectors, want) {
				t.Fatalf("sheet selectors missing %s: %+v", want, sheet.Selectors)
			}
		}
	}
	if sheet.SheetID != "" && !containsString(sheet.Selectors, "sheetId:"+sheet.SheetID) {
		t.Fatalf("sheet selectors missing sheetId:%s: %+v", sheet.SheetID, sheet.Selectors)
	}
	if sheet.RelationshipID != "" {
		for _, want := range []string{"rid:" + sheet.RelationshipID, "rId:" + sheet.RelationshipID} {
			if !containsString(sheet.Selectors, want) {
				t.Fatalf("sheet selectors missing %s: %+v", want, sheet.Selectors)
			}
		}
	}
	if sheet.PartURI != "" && !containsString(sheet.Selectors, "part:"+sheet.PartURI) {
		t.Fatalf("sheet selectors missing part:%s: %+v", sheet.PartURI, sheet.Selectors)
	}
	if !containsString(sheet.Selectors, "name:"+sheetName) {
		t.Fatalf("sheet selectors missing name:%s: %+v", sheetName, sheet.Selectors)
	}
	if !containsString(sheet.Selectors, "~"+sheetName) || !containsString(sheet.Selectors, sheetName) {
		t.Fatalf("sheet selectors missing legacy name selectors for %s: %+v", sheetName, sheet.Selectors)
	}
}

func readXLSXSheetsListForTest(t *testing.T, workbookPath string) XLSXSheetsListResult {
	t.Helper()
	readback, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "sheets", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx sheets list readback failed: %v", err)
	}
	var result XLSXSheetsListResult
	if err := json.Unmarshal([]byte(readback), &result); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, readback)
	}
	return result
}

func executeGeneratedOOXMLCommandForXLSXTest(t *testing.T, command string) string {
	t.Helper()
	if !strings.HasPrefix(command, "ooxml ") {
		t.Fatalf("generated command must start with ooxml: %s", command)
	}
	args := splitGeneratedOOXMLCommandForXLSXTest(t, command)[1:]
	output, err := executeRootForXLSXTest(t, args...)
	if err != nil {
		t.Fatalf("generated command failed: %v\ncommand=%s\noutput=%s", err, command, output)
	}
	return output
}

func splitGeneratedOOXMLCommandForXLSXTest(t *testing.T, command string) []string {
	t.Helper()
	var args []string
	var current strings.Builder
	inSingle := false
	inDouble := false
	flush := func() {
		if current.Len() > 0 {
			args = append(args, current.String())
			current.Reset()
		}
	}
	for i := 0; i < len(command); i++ {
		ch := command[i]
		switch {
		case inSingle:
			if ch == '\'' {
				inSingle = false
			} else {
				current.WriteByte(ch)
			}
		case inDouble:
			if ch == '"' {
				inDouble = false
			} else if ch == '\\' && i+1 < len(command) {
				i++
				current.WriteByte(command[i])
			} else {
				current.WriteByte(ch)
			}
		case ch == '\'':
			inSingle = true
		case ch == '"':
			inDouble = true
		case ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n':
			flush()
		default:
			current.WriteByte(ch)
		}
	}
	if inSingle || inDouble {
		t.Fatalf("generated command has unterminated quote: %s", command)
	}
	flush()
	return args
}

func readZipEntryForTest(t *testing.T, path, name string) string {
	t.Helper()
	body, ok := tryReadZipEntryForTest(t, path, name)
	if !ok {
		t.Fatalf("zip entry %s not found in %s", name, path)
	}
	return body
}

func readXLSXCellValuesForTest(t *testing.T, workbookPath, rangeRef string) map[string]string {
	t.Helper()

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "cells", "extract", workbookPath,
		"--range", rangeRef,
	)
	if err != nil {
		t.Fatalf("xlsx cells extract failed: %v", err)
	}
	var result XLSXCellsExtractResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal extract JSON: %v\n%s", err, output)
	}
	values := make(map[string]string)
	if result.Sheet == nil {
		return values
	}
	for _, row := range result.Sheet.Rows {
		for _, cell := range row.Cells {
			values[cell.Ref] = cell.Value
		}
	}
	return values
}

func assertXLSXCellValue(t *testing.T, values map[string]string, ref, want string) {
	t.Helper()
	if got, ok := values[ref]; !ok || got != want {
		t.Fatalf("%s value = %q (exists %t), want %q; cells=%+v", ref, got, ok, want, values)
	}
}

func assertXLSXNoCell(t *testing.T, values map[string]string, ref string) {
	t.Helper()
	if got, ok := values[ref]; ok {
		t.Fatalf("%s exists with value %q, want absent; cells=%+v", ref, got, values)
	}
}

func tryReadZipEntryForTest(t *testing.T, path, name string) (string, bool) {
	t.Helper()
	zr, err := zip.OpenReader(path)
	if err != nil {
		t.Fatalf("failed to open zip %s: %v", path, err)
	}
	defer zr.Close()
	for _, file := range zr.File {
		if file.Name != name {
			continue
		}
		reader, err := file.Open()
		if err != nil {
			t.Fatalf("failed to open zip entry %s: %v", name, err)
		}
		defer reader.Close()
		data, err := io.ReadAll(reader)
		if err != nil {
			t.Fatalf("failed to read zip entry %s: %v", name, err)
		}
		return string(data), true
	}
	return "", false
}

func containsString(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func getXLSXTestFilePath(fixtureDir string) string {
	return filepath.Join(getTestdataPath(), "xlsx", fixtureDir, "workbook.xlsx")
}
