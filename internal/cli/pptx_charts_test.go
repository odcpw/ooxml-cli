package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestPPTXChartsListShowJSONAndGeneratedCommands(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")

	listOutput, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "list", deckPath)
	if err != nil {
		t.Fatalf("pptx charts list failed: %v", err)
	}
	var listResult PPTXChartsResult
	if err := json.Unmarshal([]byte(listOutput), &listResult); err != nil {
		t.Fatalf("failed to unmarshal PPTX charts JSON: %v\n%s", err, listOutput)
	}
	if len(listResult.Charts) != 2 {
		t.Fatalf("charts len = %d, want 2", len(listResult.Charts))
	}
	chart := listResult.Charts[0]
	if chart.PrimarySelector != "chart:1" || chart.Slide != 1 || chart.ShapeName != "Revenue Chart" || chart.Title != "Revenue by Region" {
		t.Fatalf("unexpected chart identity: %+v", chart)
	}
	if chart.PartURI != "/ppt/charts/chart1.xml" || chart.EmbeddedWorkbookPartURI != "/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx" {
		t.Fatalf("unexpected chart part metadata: %+v", chart)
	}
	if len(chart.Types) != 1 || chart.Types[0] != "barChart" {
		t.Fatalf("unexpected chart types: %+v", chart.Types)
	}
	if len(chart.Series) != 1 {
		t.Fatalf("series len = %d, want 1", len(chart.Series))
	}
	series := chart.Series[0]
	if series.Name == nil || series.Name.Formula != "Sheet1!$B$1" || !containsString(series.Name.CachePreview, "Revenue") {
		t.Fatalf("unexpected name source: %+v", series.Name)
	}
	if series.Categories == nil || series.Categories.Formula != "Sheet1!$A$2:$A$4" || series.Categories.PointCount != 3 || !containsString(series.Categories.CachePreview, "South") {
		t.Fatalf("unexpected category source: %+v", series.Categories)
	}
	if series.Values == nil || series.Values.Formula != "Sheet1!$B$2:$B$4" || series.Values.CacheType != "numCache" || series.Values.PointCount != 3 || !containsString(series.Values.CachePreview, "120") {
		t.Fatalf("unexpected values source: %+v", series.Values)
	}
	for _, want := range []string{"chart:1", "#1", "slide:1/chart:1", "shape:Revenue Chart", "name:Revenue Chart", "~Revenue Chart", "part:/ppt/charts/chart1.xml", "rid:rId2"} {
		if !containsString(chart.Selectors, want) {
			t.Fatalf("selectors missing %q: %+v", want, chart.Selectors)
		}
	}
	for label, command := range map[string]string{
		"show first":  chart.ShowCommand,
		"show second": listResult.Charts[1].ShowCommand,
		"validate":    listResult.ValidateCommand,
	} {
		if command == "" {
			t.Fatalf("%s command is empty: %+v", label, chart)
		}
		output := executeGeneratedOOXMLCommandForXLSXTest(t, command)
		if label != "validate" && strings.TrimSpace(output) == "" {
			t.Fatalf("%s generated command returned empty output: %s", label, command)
		}
	}
	if !strings.Contains(listResult.Charts[1].ShowCommand, "--chart part:/ppt/charts/chart2.xml") {
		t.Fatalf("second chart show command should use a part selector, got %q", listResult.Charts[1].ShowCommand)
	}

	showOutput, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "show", deckPath, "--slide", "1", "--chart", "Revenue Chart")
	if err != nil {
		t.Fatalf("pptx charts show failed: %v", err)
	}
	var showResult PPTXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal PPTX charts show JSON: %v\n%s", err, showOutput)
	}
	if len(showResult.Charts) != 1 || showResult.Charts[0].PartURI != "/ppt/charts/chart1.xml" {
		t.Fatalf("unexpected show result: %+v", showResult.Charts)
	}
}

func TestPPTXChartsUpdateDataWritesCacheEmbeddedWorkbookAndCommands(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	outPath := filepath.Join(t.TempDir(), "chart-updated.pptx")
	currentHash := currentPPTXChartValuesHashForTest(t, deckPath)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "charts", "update-data", deckPath,
		"--slide", "1",
		"--chart", "chart:1",
		"--series", "1",
		"--values-json", `["150","175","210"]`,
		"--categories-json", `["North","South","West"]`,
		"--expect-point-count", "3",
		"--expect-values-hash", currentHash,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("pptx charts update-data failed: %v\n%s", err, output)
	}
	var result PPTXChartUpdateDataResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal update-data JSON: %v\n%s", err, output)
	}
	if result.Output != outPath || result.DryRun || result.Action != "pptx.chart.update-data" || !result.EmbeddedWorkbookUpdated {
		t.Fatalf("unexpected mutation envelope: %+v", result)
	}
	if result.CurrentValuesHash != currentHash || result.ExpectedValuesHashAccepted != currentHash {
		t.Fatalf("values hash guard not reflected: %+v", result)
	}
	if result.Chart == nil || len(result.Chart.Series) != 1 || result.Chart.Series[0].Values == nil || !containsString(result.Chart.Series[0].Values.CachePreview, "175") {
		t.Fatalf("missing updated chart cache readback: %+v", result.Chart)
	}
	if len(result.UpdatedRoles) != 2 {
		t.Fatalf("updated roles len = %d, want 2: %+v", len(result.UpdatedRoles), result.UpdatedRoles)
	}
	valuesRole := updatedPPTXChartRoleForTest(t, result.UpdatedRoles, "values")
	if valuesRole.PreviousValuesHash != currentHash || valuesRole.CachePointCount != 3 || !valuesRole.EmbeddedWorkbookRangeUpdated || !containsString(valuesRole.CachePreview, "210") {
		t.Fatalf("unexpected values role result: %+v", valuesRole)
	}
	categoriesRole := updatedPPTXChartRoleForTest(t, result.UpdatedRoles, "categories")
	if categoriesRole.CachePointCount != 3 || !categoriesRole.EmbeddedWorkbookRangeUpdated || !containsString(categoriesRole.CachePreview, "South") {
		t.Fatalf("unexpected categories role result: %+v", categoriesRole)
	}
	for label, command := range map[string]string{
		"validate":   result.ValidateCommand,
		"chart show": result.ChartShowCommand,
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
	var showResult PPTXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showResult); err != nil {
		t.Fatalf("failed to unmarshal generated chart show output: %v\n%s", err, showOutput)
	}
	if len(showResult.Charts) != 1 || showResult.Charts[0].Series[0].Values == nil || !containsString(showResult.Charts[0].Series[0].Values.CachePreview, "150") {
		t.Fatalf("generated chart show did not read back update: %+v", showResult.Charts)
	}

	embeddedPath := extractPPTXEmbeddedWorkbookForTest(t, outPath, result.EmbeddedWorkbookPartURI)
	rangeOutput, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "ranges", "export", embeddedPath, "--sheet", "Sheet1", "--range", "$A$1:$B$4", "--include-types")
	if err != nil {
		t.Fatalf("embedded workbook range export failed: %v\n%s", err, rangeOutput)
	}
	var rangeResult XLSXRangesExportResult
	if err := json.Unmarshal([]byte(rangeOutput), &rangeResult); err != nil {
		t.Fatalf("failed to unmarshal embedded range JSON: %v\n%s", err, rangeOutput)
	}
	if len(rangeResult.Values) != 4 || len(rangeResult.Values[3]) != 2 || rangeResult.Values[3][0] != "West" || rangeResult.Values[3][1] != float64(210) {
		t.Fatalf("embedded workbook did not sync chart values: %+v", rangeResult.Values)
	}
}

func TestPPTXChartsUpdateDataDryRunAndGuards(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")

	dryOutput, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "charts", "update-data", deckPath,
		"--slide", "1",
		"--chart", "chart:1",
		"--series", "1",
		"--values", "150,175,210",
		"--expect-point-count", "3",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("pptx charts update-data dry-run failed: %v\n%s", err, dryOutput)
	}
	var dryResult PPTXChartUpdateDataResult
	if err := json.Unmarshal([]byte(dryOutput), &dryResult); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, dryOutput)
	}
	if !dryResult.DryRun || dryResult.Output != "" || dryResult.EmbeddedWorkbookPartURI == "" {
		t.Fatalf("unexpected dry-run envelope: %+v", dryResult)
	}
	for label, command := range map[string]string{
		"validate template":   dryResult.ValidateCommandTemplate,
		"chart show template": dryResult.ChartShowCommandTemplate,
		"render template":     dryResult.RenderCommandTemplate,
	} {
		if command == "" || !strings.Contains(command, "<out.pptx>") {
			t.Fatalf("%s missing output placeholder: %s", label, command)
		}
	}

	guardOut := filepath.Join(t.TempDir(), "guard-failed.pptx")
	args := []string{
		"pptx", "charts", "update-data", deckPath,
		"--chart", "chart:1",
		"--series", "1",
		"--values", "150,175,210",
		"--expect-values-hash", "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		"--out", guardOut,
	}
	_, err = executeRootForXLSXTest(t, args...)
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if _, statErr := os.Stat(guardOut); !os.IsNotExist(statErr) {
		t.Fatalf("guarded failed mutation should not write output, stat error = %v", statErr)
	}
}

func currentPPTXChartValuesHashForTest(t *testing.T, deckPath string) string {
	t.Helper()
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "show", deckPath, "--chart", "chart:1")
	if err != nil {
		t.Fatalf("pptx charts show for hash failed: %v", err)
	}
	var result PPTXChartsResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal PPTX charts show JSON: %v\n%s", err, output)
	}
	if len(result.Charts) != 1 || len(result.Charts[0].Series) != 1 || result.Charts[0].Series[0].Values == nil {
		t.Fatalf("missing values source for hash: %+v", result.Charts)
	}
	return chartValuesHash(result.Charts[0].Series[0].Values.CachePreview)
}

func updatedPPTXChartRoleForTest(t *testing.T, roles []PPTXChartUpdatedRole, role string) PPTXChartUpdatedRole {
	t.Helper()
	for _, item := range roles {
		if item.Role == role {
			return item
		}
	}
	t.Fatalf("updated role %q not found: %+v", role, roles)
	return PPTXChartUpdatedRole{}
}

func extractPPTXEmbeddedWorkbookForTest(t *testing.T, deckPath, embeddedURI string) string {
	t.Helper()
	pkg, err := opc.Open(deckPath)
	if err != nil {
		t.Fatalf("failed to open deck: %v", err)
	}
	defer pkg.Close()
	raw, err := pkg.ReadRawPart(embeddedURI)
	if err != nil {
		t.Fatalf("failed to read embedded workbook %s: %v", embeddedURI, err)
	}
	path := filepath.Join(t.TempDir(), "embedded.xlsx")
	if err := os.WriteFile(path, raw, 0o644); err != nil {
		t.Fatalf("failed to write embedded workbook fixture: %v", err)
	}
	return path
}
