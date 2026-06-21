package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXChartsListShowJSONAndGeneratedCommands(t *testing.T) {
	workbookPath := getXLSXTestFilePath("chart-workbook")

	listOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx charts list failed: %v", err)
	}
	var listResult XLSXChartsResult
	if err := json.Unmarshal([]byte(listOutput), &listResult); err != nil {
		t.Fatalf("failed to unmarshal charts list JSON: %v\n%s", err, listOutput)
	}
	if len(listResult.Charts) != 1 {
		t.Fatalf("charts len = %d, want 1", len(listResult.Charts))
	}
	chart := listResult.Charts[0]
	if chart.Name != "Revenue Chart 1" || chart.Title != "Revenue by Region" || chart.Sheet != "Data" {
		t.Fatalf("unexpected chart identity: %+v", chart)
	}
	if chart.PrimarySelector != "chart:1" {
		t.Fatalf("chart primarySelector = %q, want chart:1", chart.PrimarySelector)
	}
	if chart.DrawingRelationshipID != "rIdDrawing1" || chart.RelationshipID != "rIdChart1" || chart.DrawingPartURI != "/xl/drawings/drawing1.xml" || chart.PartURI != "/xl/charts/chart1.xml" {
		t.Fatalf("unexpected chart part metadata: %+v", chart)
	}
	if len(chart.Types) != 1 || chart.Types[0] != "barChart" {
		t.Fatalf("unexpected chart types: %+v", chart.Types)
	}
	if chart.Anchor == nil || chart.Anchor.Type != "twoCellAnchor" || chart.Anchor.From == nil || chart.Anchor.From.Column != 3 || chart.Anchor.From.Row != 1 {
		t.Fatalf("unexpected chart anchor: %+v", chart.Anchor)
	}
	if len(chart.Series) != 1 {
		t.Fatalf("series len = %d, want 1", len(chart.Series))
	}
	series := chart.Series[0]
	if series.Number != 1 || series.Index != 0 || series.Order != 0 {
		t.Fatalf("unexpected series identity: %+v", series)
	}
	if series.Name == nil || series.Name.Formula != "Data!$B$1" || series.Name.Sheet != "Data" || series.Name.Range != "$B$1" || series.Name.CacheType != "strCache" || !containsString(series.Name.CachePreview, "Revenue") {
		t.Fatalf("unexpected series name source: %+v", series.Name)
	}
	if series.Categories == nil || series.Categories.Formula != "Data!$A$2:$A$4" || series.Categories.Range != "$A$2:$A$4" || series.Categories.PointCount != 3 || !containsString(series.Categories.CachePreview, "South") {
		t.Fatalf("unexpected category source: %+v", series.Categories)
	}
	if series.Values == nil || series.Values.Formula != "Data!$B$2:$B$4" || series.Values.Range != "$B$2:$B$4" || series.Values.CacheType != "numCache" || series.Values.PointCount != 3 || !containsString(series.Values.CachePreview, "120") {
		t.Fatalf("unexpected values source: %+v", series.Values)
	}
	if len(chart.SourceExportCommands) != 3 {
		t.Fatalf("source export commands len = %d, want 3: %+v", len(chart.SourceExportCommands), chart.SourceExportCommands)
	}
	for _, sourceCommand := range chart.SourceExportCommands {
		if sourceCommand.RangesExportCommand == "" {
			t.Fatalf("source command is empty: %+v", sourceCommand)
		}
		output := executeGeneratedOOXMLCommandForXLSXTest(t, sourceCommand.RangesExportCommand)
		if strings.TrimSpace(output) == "" {
			t.Fatalf("source generated command returned empty output: %s", sourceCommand.RangesExportCommand)
		}
	}
	for label, command := range map[string]string{
		"show":     chart.ShowCommand,
		"validate": listResult.ValidateCommand,
	} {
		if command == "" {
			t.Fatalf("%s command is empty: %+v", label, chart)
		}
		output := executeGeneratedOOXMLCommandForXLSXTest(t, command)
		if label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}
	for _, want := range []string{"chart:1", "#1", "chart:Revenue Chart 1", "name:Revenue Chart 1", "~Revenue Chart 1", "Revenue Chart 1", "part:/xl/charts/chart1.xml", "rid:rIdChart1", "rId:rIdChart1", "drawingRid:rIdDrawing1"} {
		if !containsString(chart.Selectors, want) {
			t.Fatalf("chart selectors missing %q: %+v", want, chart.Selectors)
		}
	}

	showOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "show", workbookPath, "--chart", "Revenue Chart 1")
	if err != nil {
		t.Fatalf("xlsx charts show failed: %v", err)
	}
	var showResult XLSXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal charts show JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Charts) != 1 || showResult.Charts[0].PartURI != "/xl/charts/chart1.xml" {
		t.Fatalf("unexpected show result: %+v", showResult.Charts)
	}

	inspectOutput, err := executeRootForXLSXTest(t, "--format", "json", "inspect", workbookPath)
	if err != nil {
		t.Fatalf("inspect chart workbook failed: %v", err)
	}
	var inspectResult XLSXInspectResult
	if err := json.Unmarshal([]byte(inspectOutput), &inspectResult); err != nil {
		t.Fatalf("failed to unmarshal inspect JSON: %v\n%s", err, inspectOutput)
	}
	if inspectResult.Summary.Charts != 1 {
		t.Fatalf("unexpected inspect chart count: %+v", inspectResult.Summary)
	}
}

func TestXLSXChartsAcceptStableSelectors(t *testing.T) {
	workbookPath := getXLSXTestFilePath("chart-workbook")

	for _, selector := range []string{
		"chart:1",
		"#1",
		"chart:Revenue Chart 1",
		"name:Revenue Chart 1",
		"~Revenue Chart 1",
		"Revenue Chart 1",
		"part:/xl/charts/chart1.xml",
		"rid:rIdChart1",
		"rId:rIdChart1",
		"drawingRid:rIdDrawing1",
		"1",
	} {
		t.Run(selector, func(t *testing.T) {
			output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "show", workbookPath, "--chart", selector)
			if err != nil {
				t.Fatalf("xlsx charts show --chart %q failed: %v", selector, err)
			}
			var result XLSXChartsResult
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Fatalf("failed to unmarshal charts show JSON: %v\n%s", err, output)
			}
			if len(result.Charts) != 1 || result.Charts[0].Name != "Revenue Chart 1" {
				t.Fatalf("selector %q resolved to %+v", selector, result.Charts)
			}
		})
	}
}

func TestXLSXChartsUpdateSourceWritesReadbackAndCommands(t *testing.T) {
	workbookPath := getXLSXTestFilePath("chart-workbook")
	outPath := filepath.Join(t.TempDir(), "chart-updated.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "charts", "update-source", workbookPath,
		"--chart", "chart:1",
		"--series", "1",
		"--role", "values",
		"--source-sheet", "Data",
		"--source-range", "$B$2:$B$3",
		"--expect-source-range", "$B$2:$B$4",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx charts update-source failed: %v\n%s", err, output)
	}
	var result XLSXChartUpdateSourceResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal update-source JSON: %v\n%s", err, output)
	}
	if result.Output != outPath || result.DryRun || result.Action != "xlsx.chart.update-source" {
		t.Fatalf("unexpected mutation envelope: %+v", result)
	}
	if result.PreviousFormula != "Data!$B$2:$B$4" || result.Formula != "Data!$B$2:$B$3" || result.Range != "$B$2:$B$3" || result.RefKind != "numRef" || result.CacheType != "numCache" || result.CachePointCount != 2 || result.CacheVerified {
		t.Fatalf("unexpected update result: %+v", result)
	}
	if !containsString(result.CachePreview, "100") || !containsString(result.CachePreview, "120") {
		t.Fatalf("cache preview was not rebuilt from source cells: %+v", result.CachePreview)
	}
	if len(result.Warnings) != 1 || !strings.Contains(result.Warnings[0], "categories has 3") {
		t.Fatalf("unexpected warnings: %+v", result.Warnings)
	}
	if result.Chart == nil || len(result.Chart.Series) != 1 || result.Chart.Series[0].Values == nil || result.Chart.Series[0].Values.Range != "$B$2:$B$3" {
		t.Fatalf("missing changed-chart readback: %+v", result.Chart)
	}
	for label, command := range map[string]string{
		"validate":      result.ValidateCommand,
		"chart show":    result.ChartShowCommand,
		"ranges export": result.RangesExportCommand,
	} {
		if command == "" {
			t.Fatalf("%s command is empty: %+v", label, result)
		}
		output := executeGeneratedOOXMLCommandForXLSXTest(t, command)
		if label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}

	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, result.ChartShowCommand)
	var showResult XLSXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal generated chart show output: %v\n%s", err, showOutput)
	}
	if len(showResult.Charts) != 1 || showResult.Charts[0].Series[0].Values.Range != "$B$2:$B$3" {
		t.Fatalf("generated chart show did not read back update: %+v", showResult.Charts)
	}
}

func TestXLSXChartsUpdateSourceDryRunAndGuard(t *testing.T) {
	workbookPath := getXLSXTestFilePath("chart-workbook")

	dryOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "charts", "update-source", workbookPath,
		"--chart", "chart:1",
		"--series", "1",
		"--role", "values",
		"--source-sheet", "Data",
		"--source-range", "$B$2:$B$3",
		"--expect-source-range", "$B$2:$B$4",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx charts update-source dry-run failed: %v\n%s", err, dryOutput)
	}
	var dryResult XLSXChartUpdateSourceResult
	if err := json.Unmarshal([]byte(dryOutput), &dryResult); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, dryOutput)
	}
	if !dryResult.DryRun || dryResult.Output != "" {
		t.Fatalf("unexpected dry-run envelope: %+v", dryResult)
	}
	for label, command := range map[string]string{
		"validate template":      dryResult.ValidateCommandTemplate,
		"chart show template":    dryResult.ChartShowCommandTemplate,
		"ranges export template": dryResult.RangesExportCommandTemplate,
	} {
		if command == "" || !strings.Contains(command, "<out.xlsx>") {
			t.Fatalf("%s missing output placeholder: %s", label, command)
		}
	}
	if dryResult.SourceRangeExportCommandDryRun == "" {
		t.Fatalf("dry-run should include executable source range export command: %+v", dryResult)
	}
	if output := executeGeneratedOOXMLCommandForXLSXTest(t, dryResult.SourceRangeExportCommandDryRun); strings.TrimSpace(output) == "" {
		t.Fatalf("dry-run source export command returned empty output: %s", dryResult.SourceRangeExportCommandDryRun)
	}

	guardOut := filepath.Join(t.TempDir(), "guard-failed.xlsx")
	args := []string{
		"xlsx", "charts", "update-source", workbookPath,
		"--chart", "chart:1",
		"--series", "1",
		"--role", "values",
		"--source-sheet", "Data",
		"--source-range", "$B$2:$B$3",
		"--expect-source-range", "$B$2:$B$99",
		"--out", guardOut,
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if _, statErr := os.Stat(guardOut); !os.IsNotExist(statErr) {
		t.Fatalf("guarded failed mutation should not write output, stat error = %v", statErr)
	}
}

func TestXLSXChartsTextAndBadArguments(t *testing.T) {
	workbookPath := getXLSXTestFilePath("chart-workbook")

	output, err := executeRootForXLSXTest(t, "xlsx", "charts", "list", workbookPath)
	if err != nil {
		t.Fatalf("xlsx charts list text failed: %v", err)
	}
	for _, want := range []string{"Revenue Chart 1", "title: Revenue by Region", "types: barChart", "values=Data!$B$2:$B$4"} {
		if !strings.Contains(output, want) {
			t.Fatalf("text output missing %q:\n%s", want, output)
		}
	}

	missingArgs := []string{"xlsx", "charts", "show", workbookPath, "--chart", "Missing"}
	_, err = executeRootForXLSXTest(t, missingArgs...)
	assertCLIExitCodeForXLSXTest(t, missingArgs, err, ExitTargetNotFound)

	noChartWorkbook := getXLSXTestFilePath("minimal-workbook")
	noChartsOutput, err := executeRootForXLSXTest(t, "xlsx", "charts", "list", noChartWorkbook)
	if err != nil {
		t.Fatalf("xlsx charts list no-chart workbook failed: %v", err)
	}
	if strings.TrimSpace(noChartsOutput) != "no charts found" {
		t.Fatalf("unexpected no-chart output: %q", noChartsOutput)
	}
	noChartArgs := []string{"xlsx", "charts", "show", noChartWorkbook}
	_, err = executeRootForXLSXTest(t, noChartArgs...)
	assertCLIExitCodeForXLSXTest(t, noChartArgs, err, ExitInvalidArgs)
}
