package cli

import (
	"encoding/json"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/conformance"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxchart "github.com/ooxml-cli/ooxml-cli/pkg/pptx/chart"
	xlsxns "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

type generatedRepairConformanceSummary struct {
	Scenario     string                            `json:"scenario"`
	Family       string                            `json:"family"`
	Status       string                            `json:"status"`
	Checks       []generatedRepairConformanceCheck `json:"checks"`
	PackageFacts []generatedRepairPackageFact      `json:"packageFacts"`
}

type generatedRepairConformanceCheck struct {
	Name        string `json:"name"`
	Status      string `json:"status"`
	Diagnostics int    `json:"diagnostics"`
}

type generatedRepairPackageFact struct {
	Name   string `json:"name"`
	Status string `json:"status"`
	Detail string `json:"detail,omitempty"`
}

type generatedRepairProofBundle struct {
	SchemaVersion       string                         `json:"schemaVersion"`
	Scope               string                         `json:"scope"`
	OfficeOpenMode      string                         `json:"officeOpenMode"`
	MicrosoftOfficeMode string                         `json:"microsoftOfficeMode"`
	Scenarios           []generatedRepairProofScenario `json:"scenarios"`
}

type generatedRepairProofScenario struct {
	Scenario                string                            `json:"scenario"`
	Family                  string                            `json:"family"`
	StrictValidation        string                            `json:"strictValidation"`
	RepairConformance       string                            `json:"repairConformance"`
	LocalOfficeOpen         string                            `json:"localOfficeOpen"`
	RequiresMicrosoftOffice bool                              `json:"requiresMicrosoftOffice"`
	Checks                  []generatedRepairConformanceCheck `json:"checks"`
	PackageFacts            []generatedRepairPackageFact      `json:"packageFacts"`
}

const generatedEmbeddedWorkbookContentType = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
const generatedRepairProofBundleSchemaVersion = "ooxml-cli.generated-repair-proof-bundle.v1"
const generatedRepairPassedStatus = "passed"

// TestGeneratedRepairConformanceGolden freezes repair-conformance evidence for
// packages authored by high-risk writer paths: charts, chart/range styling,
// chart source/type mutations, embedded chart workbooks, and pivots. These
// commands create or update several coordinated OOXML parts and relationships,
// so they are the most likely ordinary workflows to trigger Office repair if a
// writer regresses.
func TestGeneratedRepairConformanceGolden(t *testing.T) {
	dir := t.TempDir()
	var summaries []generatedRepairConformanceSummary

	xlsxChartData := stageChartData(t, `[["Region","Sales"],["North",42],["South",58],["East",30]]`)
	xlsxChart := filepath.Join(dir, "xlsx-chart-create-bar.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", xlsxChartData,
		"--type", "bar", "--sheet", "1", "--range", "A1:B4", "--title", "Sales", "--anchor", "D1", "--out", xlsxChart); err != nil {
		t.Fatalf("xlsx chart create failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-chart-create-bar", xlsxChart))

	xlsxChartUpdatedSource := filepath.Join(dir, "xlsx-chart-update-source.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "update-source", xlsxChart,
		"--sheet", "1", "--chart", "1", "--series", "1", "--role", "values",
		"--source-sheet", "Sheet1", "--source-range", "$B$2:$B$3",
		"--expect-source-range", "$B$2:$B$4",
		"--out", xlsxChartUpdatedSource); err != nil {
		t.Fatalf("xlsx chart update-source failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-chart-update-source", xlsxChartUpdatedSource))

	xlsxChartConverted := filepath.Join(dir, "xlsx-chart-convert-type.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "convert-type", xlsxChart,
		"--sheet", "1", "--chart", "1", "--to", "line", "--expect-type", "column",
		"--out", xlsxChartConverted); err != nil {
		t.Fatalf("xlsx chart convert-type failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-chart-convert-type", xlsxChartConverted))

	xlsxChartStyledTitle := filepath.Join(dir, "xlsx-chart-style-title.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-title", xlsxChart,
		"--sheet", "1", "--chart", "1", "--title", "Styled Sales",
		"--font-size", "16", "--font-color", "#1f77b4", "--font-bold",
		"--out", xlsxChartStyledTitle); err != nil {
		t.Fatalf("xlsx chart set-title failed: %v\n%s", err, out)
	}
	xlsxChartStyledLegend := filepath.Join(dir, "xlsx-chart-style-legend.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-legend", xlsxChartStyledTitle,
		"--sheet", "1", "--chart", "1", "--position", "bottom", "--out", xlsxChartStyledLegend); err != nil {
		t.Fatalf("xlsx chart set-legend failed: %v\n%s", err, out)
	}
	xlsxChartStyledAxis := filepath.Join(dir, "xlsx-chart-style-axis.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-axis", xlsxChartStyledLegend,
		"--sheet", "1", "--chart", "1", "--axis", "value", "--title", "Revenue",
		"--number-format", "#,##0", "--major-gridlines=true",
		"--out", xlsxChartStyledAxis); err != nil {
		t.Fatalf("xlsx chart set-axis failed: %v\n%s", err, out)
	}
	xlsxChartStyled := filepath.Join(dir, "xlsx-chart-style-chain.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-series-style", xlsxChartStyledAxis,
		"--sheet", "1", "--chart", "1", "--series", "1", "--fill-color", "#2ca02c",
		"--expect-series-count", "1", "--out", xlsxChartStyled); err != nil {
		t.Fatalf("xlsx chart set-series-style failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-chart-style-chain", xlsxChartStyled))

	xlsxRangeStyled := filepath.Join(dir, "xlsx-range-style.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "ranges", "set-style", getXLSXTestFilePath("minimal-workbook"),
		"--sheet", "1", "--range", "A1:A1",
		"--font-bold", "--font-color", "#ffffff", "--fill-color", "#1f4e79",
		"--border-style", "thin", "--border-color", "#d9eaf7",
		"--alignment-horizontal", "center", "--out", xlsxRangeStyled); err != nil {
		t.Fatalf("xlsx range set-style failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-range-style", xlsxRangeStyled))

	xlsxPivotData := stagePivotData(t)
	xlsxPivot := filepath.Join(dir, "xlsx-pivot-create.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "create", xlsxPivotData,
		"--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--cols", "Product", "--values", "Sales:sum", "--anchor", "F1", "--out", xlsxPivot); err != nil {
		t.Fatalf("xlsx pivot create failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-pivot-create", xlsxPivot))

	xlsxHyperlink := filepath.Join(dir, "xlsx-hyperlink-add.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "hyperlinks", "add", getXLSXTestFilePath("minimal-workbook"),
		"--sheet", "1", "--cell", "A1", "--url", "https://example.com", "--tooltip", "Visit", "--out", xlsxHyperlink); err != nil {
		t.Fatalf("xlsx hyperlink add failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "xlsx-hyperlink-add", xlsxHyperlink))

	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	pptxInline := filepath.Join(dir, "pptx-chart-create-inline.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar", "--title", "Sales",
		"--values-json", `[["Region","S1","S2"],["North",10,15],["South",20,25],["East",30,35]]`,
		"--out", pptxInline); err != nil {
		t.Fatalf("pptx inline chart create failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "pptx-chart-create-inline", pptxInline))

	pptxStyleTitle := filepath.Join(dir, "pptx-chart-style-title.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-title", getTestFilePath("chart-simple", "presentation.pptx"),
		"--chart", "chart:1", "--title", "Styled Revenue",
		"--font-size", "18", "--font-color", "#1f77b4", "--font-bold",
		"--out", pptxStyleTitle); err != nil {
		t.Fatalf("pptx chart set-title failed: %v\n%s", err, out)
	}
	pptxStyleLegend := filepath.Join(dir, "pptx-chart-style-legend.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-legend", pptxStyleTitle,
		"--chart", "chart:1", "--position", "top", "--overlay=false", "--out", pptxStyleLegend); err != nil {
		t.Fatalf("pptx chart set-legend failed: %v\n%s", err, out)
	}
	pptxStyleAxis := filepath.Join(dir, "pptx-chart-style-axis.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-axis", pptxStyleLegend,
		"--chart", "chart:1", "--axis", "value", "--title", "Revenue",
		"--number-format", "#,##0", "--out", pptxStyleAxis); err != nil {
		t.Fatalf("pptx chart set-axis failed: %v\n%s", err, out)
	}
	pptxStyled := filepath.Join(dir, "pptx-chart-style-chain.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-series-style", pptxStyleAxis,
		"--chart", "chart:1", "--series", "1", "--fill-color", "#2ca02c",
		"--expect-series-count", "1", "--out", pptxStyled); err != nil {
		t.Fatalf("pptx chart set-series-style failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "pptx-chart-style-chain", pptxStyled))

	pptxConverted := filepath.Join(dir, "pptx-chart-convert-type.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "convert-type", getTestFilePath("chart-simple", "presentation.pptx"),
		"--chart", "chart:1", "--to", "line", "--expect-type", "column", "--out", pptxConverted); err != nil {
		t.Fatalf("pptx chart convert-type failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "pptx-chart-convert-type", pptxConverted))

	sourceWorkbook := getTestFilePath("../xlsx/chart-workbook", "workbook.xlsx")
	pptxEmbedded := filepath.Join(dir, "pptx-chart-create-embedded-workbook.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar",
		"--source-file", sourceWorkbook, "--source-sheet", "Data", "--source-range", "A1:B4",
		"--expect-source-range", "A1:B4", "--embed-workbook",
		"--out", pptxEmbedded); err != nil {
		t.Fatalf("pptx embedded chart create failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "pptx-chart-create-embedded-workbook", pptxEmbedded))

	pptxUpdatedData := filepath.Join(dir, "pptx-chart-update-data.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "update-data", pptxEmbedded,
		"--chart", "chart:1", "--series", "1",
		"--values", "99,88,77", "--categories", "North,South,West",
		"--expect-point-count", "3", "--out", pptxUpdatedData); err != nil {
		t.Fatalf("pptx chart update-data failed: %v\n%s", err, out)
	}
	summaries = append(summaries, generatedConformanceSummary(t, "pptx-chart-update-data", pptxUpdatedData))

	assertGoldenJSONValue(t, "generated-repair-conformance-summary.json", summaries)
	assertGeneratedLocalOfficeOpenScenariosMatchSummaries(t, summaries)
	assertGoldenJSONValue(t, "generated-repair-proof-bundle.json", generatedProofBundleFromSummaries(summaries))
}

func TestGeneratedRepairConformanceOfficeOpenIfAvailable(t *testing.T) {
	requireGeneratedOfficeCheckBaseline(t)

	dir := t.TempDir()
	proof := newGeneratedOfficeOpenProofTracker(t)
	xlsxChartData := stageChartData(t, `[["Region","Sales"],["North",42],["South",58],["East",30]]`)
	xlsxChart := filepath.Join(dir, "xlsx-chart-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "create", xlsxChartData,
		"--type", "bar", "--sheet", "1", "--range", "A1:B4", "--title", "Sales", "--anchor", "D1", "--out", xlsxChart); err != nil {
		t.Fatalf("xlsx chart create failed: %v\n%s", err, out)
	}
	xlsxUpdated := filepath.Join(dir, "xlsx-chart-office-open-updated.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "update-source", xlsxChart,
		"--sheet", "1", "--chart", "1", "--series", "1", "--role", "values",
		"--source-sheet", "Sheet1", "--source-range", "$B$2:$B$3",
		"--expect-source-range", "$B$2:$B$4",
		"--out", xlsxUpdated); err != nil {
		t.Fatalf("xlsx chart update-source failed: %v\n%s", err, out)
	}
	xlsxConverted := filepath.Join(dir, "xlsx-chart-office-open-converted.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "convert-type", xlsxChart,
		"--sheet", "1", "--chart", "1", "--to", "line", "--expect-type", "column",
		"--out", xlsxConverted); err != nil {
		t.Fatalf("xlsx chart convert-type failed: %v\n%s", err, out)
	}
	proof.assertPassed("xlsx-chart-create-bar", xlsxChart)
	proof.assertPassed("xlsx-chart-update-source", xlsxUpdated)
	proof.assertPassed("xlsx-chart-convert-type", xlsxConverted)

	xlsxChartStyledTitle := filepath.Join(dir, "xlsx-chart-style-title-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-title", xlsxChart,
		"--sheet", "1", "--chart", "1", "--title", "Styled Sales",
		"--font-size", "16", "--font-color", "#1f77b4", "--font-bold",
		"--out", xlsxChartStyledTitle); err != nil {
		t.Fatalf("xlsx chart set-title failed: %v\n%s", err, out)
	}
	xlsxChartStyledLegend := filepath.Join(dir, "xlsx-chart-style-legend-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-legend", xlsxChartStyledTitle,
		"--sheet", "1", "--chart", "1", "--position", "bottom", "--out", xlsxChartStyledLegend); err != nil {
		t.Fatalf("xlsx chart set-legend failed: %v\n%s", err, out)
	}
	xlsxChartStyledAxis := filepath.Join(dir, "xlsx-chart-style-axis-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-axis", xlsxChartStyledLegend,
		"--sheet", "1", "--chart", "1", "--axis", "value", "--title", "Revenue",
		"--number-format", "#,##0", "--major-gridlines=true",
		"--out", xlsxChartStyledAxis); err != nil {
		t.Fatalf("xlsx chart set-axis failed: %v\n%s", err, out)
	}
	xlsxChartStyled := filepath.Join(dir, "xlsx-chart-style-chain-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "set-series-style", xlsxChartStyledAxis,
		"--sheet", "1", "--chart", "1", "--series", "1", "--fill-color", "#2ca02c",
		"--expect-series-count", "1", "--out", xlsxChartStyled); err != nil {
		t.Fatalf("xlsx chart set-series-style failed: %v\n%s", err, out)
	}
	proof.assertPassed("xlsx-chart-style-chain", xlsxChartStyled)

	xlsxRangeStyled := filepath.Join(dir, "xlsx-range-style-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "ranges", "set-style", getXLSXTestFilePath("minimal-workbook"),
		"--sheet", "1", "--range", "A1:A1",
		"--font-bold", "--font-color", "#ffffff", "--fill-color", "#1f4e79",
		"--border-style", "thin", "--border-color", "#d9eaf7",
		"--alignment-horizontal", "center", "--out", xlsxRangeStyled); err != nil {
		t.Fatalf("xlsx range set-style failed: %v\n%s", err, out)
	}
	proof.assertPassed("xlsx-range-style", xlsxRangeStyled)

	xlsxPivotData := stagePivotData(t)
	xlsxPivot := filepath.Join(dir, "xlsx-pivot-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "pivots", "create", xlsxPivotData,
		"--sheet", "1", "--range", "A1:C5", "--rows", "Region", "--cols", "Product", "--values", "Sales:sum", "--anchor", "F1", "--out", xlsxPivot); err != nil {
		t.Fatalf("xlsx pivot create failed: %v\n%s", err, out)
	}
	proof.assertPassed("xlsx-pivot-create", xlsxPivot)

	xlsxHyperlink := filepath.Join(dir, "xlsx-hyperlink-office-open.xlsx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "hyperlinks", "add", getXLSXTestFilePath("minimal-workbook"),
		"--sheet", "1", "--cell", "A1", "--url", "https://example.com", "--tooltip", "Visit", "--out", xlsxHyperlink); err != nil {
		t.Fatalf("xlsx hyperlink add failed: %v\n%s", err, out)
	}
	proof.assertPassed("xlsx-hyperlink-add", xlsxHyperlink)

	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	pptxInline := filepath.Join(dir, "pptx-chart-inline-office-open.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar", "--title", "Sales",
		"--values-json", `[["Region","S1","S2"],["North",10,15],["South",20,25],["East",30,35]]`,
		"--out", pptxInline); err != nil {
		t.Fatalf("pptx inline chart create failed: %v\n%s", err, out)
	}
	proof.assertPassed("pptx-chart-create-inline", pptxInline)

	sourceWorkbook := getTestFilePath("../xlsx/chart-workbook", "workbook.xlsx")
	pptxEmbedded := filepath.Join(dir, "pptx-chart-office-open-embedded.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar",
		"--source-file", sourceWorkbook, "--source-sheet", "Data", "--source-range", "A1:B4",
		"--expect-source-range", "A1:B4", "--embed-workbook",
		"--out", pptxEmbedded); err != nil {
		t.Fatalf("pptx embedded chart create failed: %v\n%s", err, out)
	}
	pptxUpdated := filepath.Join(dir, "pptx-chart-office-open-updated.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "update-data", pptxEmbedded,
		"--chart", "chart:1", "--series", "1",
		"--values", "99,88,77", "--categories", "North,South,West",
		"--expect-point-count", "3", "--out", pptxUpdated); err != nil {
		t.Fatalf("pptx chart update-data failed: %v\n%s", err, out)
	}
	proof.assertPassed("pptx-chart-create-embedded-workbook", pptxEmbedded)
	proof.assertPassed("pptx-chart-update-data", pptxUpdated)

	pptxConvertedFromFixture := filepath.Join(dir, "pptx-chart-convert-type-office-open.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "convert-type", getTestFilePath("chart-simple", "presentation.pptx"),
		"--chart", "chart:1", "--to", "line", "--expect-type", "column", "--out", pptxConvertedFromFixture); err != nil {
		t.Fatalf("pptx chart convert-type failed: %v\n%s", err, out)
	}
	proof.assertPassed("pptx-chart-convert-type", pptxConvertedFromFixture)

	pptxStyleTitle := filepath.Join(dir, "pptx-chart-style-title-office-open.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-title", getTestFilePath("chart-simple", "presentation.pptx"),
		"--chart", "chart:1", "--title", "Styled Revenue",
		"--font-size", "18", "--font-color", "#1f77b4", "--font-bold",
		"--out", pptxStyleTitle); err != nil {
		t.Fatalf("pptx chart set-title failed: %v\n%s", err, out)
	}
	pptxStyleLegend := filepath.Join(dir, "pptx-chart-style-legend-office-open.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-legend", pptxStyleTitle,
		"--chart", "chart:1", "--position", "top", "--overlay=false", "--out", pptxStyleLegend); err != nil {
		t.Fatalf("pptx chart set-legend failed: %v\n%s", err, out)
	}
	pptxStyleAxis := filepath.Join(dir, "pptx-chart-style-axis-office-open.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-axis", pptxStyleLegend,
		"--chart", "chart:1", "--axis", "value", "--title", "Revenue",
		"--number-format", "#,##0", "--out", pptxStyleAxis); err != nil {
		t.Fatalf("pptx chart set-axis failed: %v\n%s", err, out)
	}
	pptxStyled := filepath.Join(dir, "pptx-chart-style-chain-office-open.pptx")
	if out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "set-series-style", pptxStyleAxis,
		"--chart", "chart:1", "--series", "1", "--fill-color", "#2ca02c",
		"--expect-series-count", "1", "--out", pptxStyled); err != nil {
		t.Fatalf("pptx chart set-series-style failed: %v\n%s", err, out)
	}
	proof.assertPassed("pptx-chart-style-chain", pptxStyled)
	proof.assertComplete()
}

func generatedConformanceSummary(t *testing.T, scenario, path string) generatedRepairConformanceSummary {
	t.Helper()
	if out, err := executeRootForXLSXTest(t, "validate", "--strict", path); err != nil {
		t.Fatalf("validate --strict failed for %s: %v\n%s", scenario, err, out)
	}
	out, err := executeRootForXLSXTest(t, "--json", "conformance", "check", path)
	if err != nil {
		t.Fatalf("conformance check failed for %s: %v\n%s", scenario, err, out)
	}
	var report conformance.Report
	if err := json.Unmarshal([]byte(out), &report); err != nil {
		t.Fatalf("parse conformance report for %s: %v\n%s", scenario, err, out)
	}
	if report.Status != generatedRepairPassedStatus {
		t.Fatalf("conformance status for %s = %s, want %s\n%s", scenario, report.Status, generatedRepairPassedStatus, out)
	}

	summary := generatedRepairConformanceSummary{
		Scenario:     scenario,
		Family:       report.Family,
		Status:       report.Status,
		PackageFacts: generatedPackageFacts(t, scenario, path),
	}
	for _, check := range report.Checks {
		summary.Checks = append(summary.Checks, generatedRepairConformanceCheck{
			Name:        check.Name,
			Status:      check.Status,
			Diagnostics: len(check.Diagnostics),
		})
	}
	return summary
}

func generatedProofBundleFromSummaries(summaries []generatedRepairConformanceSummary) generatedRepairProofBundle {
	// Failed validation or conformance aborts the generating test, so this
	// manifest intentionally records the successful proof bundle only.
	bundle := generatedRepairProofBundle{
		SchemaVersion:       generatedRepairProofBundleSchemaVersion,
		Scope:               "generated-pptx-xlsx-business-workflows",
		OfficeOpenMode:      "not-run-in-golden; see TestGeneratedRepairConformanceOfficeOpenIfAvailable for optional local LibreOffice/soffice proof",
		MicrosoftOfficeMode: "external-oracle-required",
	}
	for _, summary := range summaries {
		bundle.Scenarios = append(bundle.Scenarios, generatedRepairProofScenario{
			Scenario:                summary.Scenario,
			Family:                  summary.Family,
			StrictValidation:        generatedRepairPassedStatus,
			RepairConformance:       summary.Status,
			LocalOfficeOpen:         generatedLocalOfficeOpenProof(summary.Scenario),
			RequiresMicrosoftOffice: true,
			Checks:                  summary.Checks,
			PackageFacts:            summary.PackageFacts,
		})
	}
	return bundle
}

func assertGeneratedLocalOfficeOpenScenariosMatchSummaries(t *testing.T, summaries []generatedRepairConformanceSummary) {
	t.Helper()
	generated := make(map[string]bool, len(summaries))
	for _, summary := range summaries {
		generated[summary.Scenario] = true
	}
	for scenario := range generatedLocalOfficeOpenScenarios() {
		if !generated[scenario] {
			t.Fatalf("local office-open scenario %q is not present in generated repair summaries", scenario)
		}
	}
}

func generatedLocalOfficeOpenProof(scenario string) string {
	if generatedLocalOfficeOpenScenarios()[scenario] {
		return "covered-by-optional-local-office-open-test"
	}
	return "not-run"
}

func generatedLocalOfficeOpenScenarios() map[string]bool {
	return map[string]bool{
		"xlsx-chart-create-bar":               true,
		"xlsx-chart-update-source":            true,
		"xlsx-chart-convert-type":             true,
		"xlsx-chart-style-chain":              true,
		"xlsx-range-style":                    true,
		"xlsx-pivot-create":                   true,
		"xlsx-hyperlink-add":                  true,
		"pptx-chart-create-inline":            true,
		"pptx-chart-style-chain":              true,
		"pptx-chart-convert-type":             true,
		"pptx-chart-create-embedded-workbook": true,
		"pptx-chart-update-data":              true,
	}
}

type generatedOfficeOpenProofTracker struct {
	t       *testing.T
	covered map[string]bool
}

func newGeneratedOfficeOpenProofTracker(t *testing.T) generatedOfficeOpenProofTracker {
	t.Helper()
	return generatedOfficeOpenProofTracker{t: t, covered: make(map[string]bool)}
}

func (p generatedOfficeOpenProofTracker) assertPassed(scenario, path string) {
	p.t.Helper()
	if !generatedLocalOfficeOpenScenarios()[scenario] {
		p.t.Fatalf("office-open scenario %q is not listed in generatedLocalOfficeOpenScenarios", scenario)
	}
	p.covered[scenario] = true
	assertGeneratedOfficeOpenPassed(p.t, scenario, path)
}

func (p generatedOfficeOpenProofTracker) assertComplete() {
	p.t.Helper()
	expected := generatedLocalOfficeOpenScenarios()
	for scenario := range expected {
		if !p.covered[scenario] {
			p.t.Fatalf("generated local office-open proof missing scenario %q", scenario)
		}
	}
	for scenario := range p.covered {
		if !expected[scenario] {
			p.t.Fatalf("generated local office-open proof covered unexpected scenario %q", scenario)
		}
	}
}

func requireGeneratedOfficeCheckBaseline(t *testing.T) {
	t.Helper()
	if _, err := exec.LookPath("soffice"); err != nil {
		if _, err := exec.LookPath("libreoffice"); err != nil {
			t.Skip("soffice/libreoffice not available; skipping generated office-open conformance proof")
		}
	}
	for label, path := range map[string]string{
		"xlsx baseline": getXLSXTestFilePath("minimal-workbook"),
		"pptx baseline": getTestFilePath("minimal-title", "presentation.pptx"),
	} {
		report, output, err := generatedOfficeCheckReport(t, path)
		if err != nil {
			t.Skipf("local office-check baseline %s failed; skipping generated office-open proof: %v\n%s", label, err, output)
		}
		if !generatedReportHasPassedOfficeOpen(report) {
			t.Skipf("local office-check baseline %s did not produce passed office-open proof: %+v", label, report.Checks)
		}
	}
}

func assertGeneratedOfficeOpenPassed(t *testing.T, scenario, path string) {
	t.Helper()
	report, output, err := generatedOfficeCheckReport(t, path)
	if err != nil {
		t.Fatalf("conformance check --office-check failed for %s: %v\n%s", scenario, err, output)
	}
	if report.Status != "passed" {
		t.Fatalf("conformance office-check status for %s = %s, want passed\n%s", scenario, report.Status, output)
	}
	if !generatedReportHasPassedOfficeOpen(report) {
		t.Fatalf("missing passed office-open proof for %s: %+v\n%s", scenario, report.Checks, output)
	}
}

func generatedOfficeCheckReport(t *testing.T, path string) (conformance.Report, string, error) {
	t.Helper()
	output, err := executeRootForXLSXTest(t, "--json", "conformance", "check", "--office-check", path)
	if err != nil {
		return conformance.Report{}, output, err
	}
	var report conformance.Report
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		return conformance.Report{}, output, err
	}
	return report, output, nil
}

func generatedReportHasPassedOfficeOpen(report conformance.Report) bool {
	for _, check := range report.Checks {
		if check.Name != "office-open" {
			continue
		}
		return check.Status == "passed" && check.OfficeCheck != nil && check.OfficeCheck.OfficeOpenVerified
	}
	return false
}

func generatedPackageFacts(t *testing.T, scenario, path string) []generatedRepairPackageFact {
	t.Helper()
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("open generated package for %s: %v", scenario, err)
	}
	defer pkg.Close()

	c := generatedPackageFactCollector{t: t, pkg: pkg}
	switch scenario {
	case "xlsx-chart-create-bar":
		c.requirePart("/xl/drawings/drawing1.xml", xlsxns.ContentTypeDrawing)
		c.requirePart("/xl/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/xl/worksheets/sheet1.xml", xlsxns.RelDrawing, "../drawings/drawing1.xml", "internal")
		c.requireRelationship("/xl/drawings/drawing1.xml", xlsxns.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/xl/worksheets/sheet1.xml", xlsxns.NsSpreadsheetML, "drawing")
		c.requireElement("/xl/drawings/drawing1.xml", xlsxns.NsChart, "chart")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "barChart")
	case "xlsx-chart-update-source":
		c.requirePart("/xl/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/xl/drawings/drawing1.xml", xlsxns.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "barChart")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "f")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "numCache")
	case "xlsx-chart-convert-type":
		c.requirePart("/xl/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/xl/drawings/drawing1.xml", xlsxns.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "lineChart")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "catAx")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "valAx")
	case "xlsx-chart-style-chain":
		c.requirePart("/xl/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/xl/drawings/drawing1.xml", xlsxns.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "title")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "legend")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "valAx")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "numFmt")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsChart, "spPr")
		c.requireElement("/xl/charts/chart1.xml", xlsxns.NsDrawingMain, "solidFill")
	case "xlsx-range-style":
		c.requirePart("/xl/styles.xml", xlsxns.ContentTypeStyles)
		c.requireRelationship("/xl/workbook.xml", xlsxns.RelStyles, "styles.xml", "internal")
		c.requireElement("/xl/styles.xml", xlsxns.NsSpreadsheetML, "fonts")
		c.requireElement("/xl/styles.xml", xlsxns.NsSpreadsheetML, "fills")
		c.requireElement("/xl/styles.xml", xlsxns.NsSpreadsheetML, "borders")
		c.requireElement("/xl/styles.xml", xlsxns.NsSpreadsheetML, "cellXfs")
		c.requireStyledCell("/xl/worksheets/sheet1.xml", "A1")
	case "xlsx-pivot-create":
		c.requirePart("/xl/pivotTables/pivotTable1.xml", xlsxns.ContentTypePivotTable)
		c.requirePart("/xl/pivotCache/pivotCacheDefinition1.xml", xlsxns.ContentTypePivotCache)
		c.requirePart("/xl/pivotCache/pivotCacheRecords1.xml", xlsxns.ContentTypePivotRecords)
		c.requireRelationship("/xl/worksheets/sheet1.xml", xlsxns.RelPivotTable, "../pivotTables/pivotTable1.xml", "internal")
		c.requireRelationship("/xl/workbook.xml", xlsxns.RelPivotCache, "pivotCache/pivotCacheDefinition1.xml", "internal")
		c.requireRelationship("/xl/pivotCache/pivotCacheDefinition1.xml", xlsxns.RelPivotRecords, "pivotCacheRecords1.xml", "internal")
		c.requireElement("/xl/pivotTables/pivotTable1.xml", xlsxns.NsSpreadsheetML, "pivotTableDefinition")
		c.requireElement("/xl/pivotCache/pivotCacheDefinition1.xml", xlsxns.NsSpreadsheetML, "pivotCacheDefinition")
		c.requireElement("/xl/pivotCache/pivotCacheRecords1.xml", xlsxns.NsSpreadsheetML, "pivotCacheRecords")
	case "xlsx-hyperlink-add":
		c.requirePart("/xl/worksheets/sheet1.xml", xlsxns.ContentTypeWorksheet)
		c.requireRelationship("/xl/worksheets/sheet1.xml", xlsxns.RelHyperlink, "https://example.com", "External")
		c.requireElement("/xl/worksheets/sheet1.xml", xlsxns.NsSpreadsheetML, "hyperlink")
		c.requireRelationshipBackedElement("/xl/worksheets/sheet1.xml", xlsxns.NsSpreadsheetML, "hyperlink", xlsxns.RelHyperlink, "External")
	case "pptx-chart-create-inline":
		c.requirePart("/ppt/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/ppt/slides/slide1.xml", pptxchart.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/ppt/slides/slide1.xml", xlsxns.NsChart, "chart")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "barChart")
	case "pptx-chart-style-chain":
		c.requirePart("/ppt/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/ppt/slides/slide1.xml", pptxchart.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "title")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "legend")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "valAx")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "numFmt")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "spPr")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsDrawingMain, "solidFill")
	case "pptx-chart-convert-type":
		c.requirePart("/ppt/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requireRelationship("/ppt/slides/slide1.xml", pptxchart.RelChart, "../charts/chart1.xml", "internal")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "lineChart")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "catAx")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "valAx")
	case "pptx-chart-create-embedded-workbook":
		c.requirePart("/ppt/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requirePart("/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx", generatedEmbeddedWorkbookContentType)
		c.requireRelationship("/ppt/slides/slide1.xml", pptxchart.RelChart, "../charts/chart1.xml", "internal")
		c.requireRelationship("/ppt/charts/chart1.xml", pptxchart.RelPackage, "../embeddings/Microsoft_Excel_Sheet1.xlsx", "internal")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "barChart")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "externalData")
	case "pptx-chart-update-data":
		c.requirePart("/ppt/charts/chart1.xml", xlsxns.ContentTypeChart)
		c.requirePart("/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx", generatedEmbeddedWorkbookContentType)
		c.requireRelationship("/ppt/slides/slide1.xml", pptxchart.RelChart, "../charts/chart1.xml", "internal")
		c.requireRelationship("/ppt/charts/chart1.xml", pptxchart.RelPackage, "../embeddings/Microsoft_Excel_Sheet1.xlsx", "internal")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "barChart")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "externalData")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "numCache")
		c.requireElement("/ppt/charts/chart1.xml", xlsxns.NsChart, "strCache")
	default:
		t.Fatalf("no generated package fact checks for scenario %s", scenario)
	}
	return c.facts
}

type generatedPackageFactCollector struct {
	t     *testing.T
	pkg   *opc.Package
	facts []generatedRepairPackageFact
}

func (c *generatedPackageFactCollector) requirePart(uri, contentType string) {
	c.t.Helper()
	for _, part := range c.pkg.ListParts() {
		if part.URI != uri {
			continue
		}
		got := c.pkg.GetContentType(uri)
		if got != contentType {
			c.t.Fatalf("part %s content type = %q, want %q", uri, got, contentType)
		}
		c.add("part:"+uri, "contentType="+got)
		return
	}
	c.t.Fatalf("generated package missing part %s", uri)
}

func (c *generatedPackageFactCollector) requireRelationship(sourceURI, relType, target, mode string) {
	c.t.Helper()
	target = strings.TrimSpace(target)
	mode = strings.TrimSpace(mode)
	if target == "" {
		c.t.Fatalf("requireRelationship for %s type %s must specify a concrete target", sourceURI, relType)
	}
	if mode == "" {
		c.t.Fatalf("requireRelationship for %s type %s target %s must specify a concrete target mode", sourceURI, relType, target)
	}
	var typeMatched []opc.RelationshipInfo
	for _, rel := range c.pkg.ListRelationships(sourceURI) {
		if rel.Type != relType {
			continue
		}
		typeMatched = append(typeMatched, rel)
		if rel.Target != target {
			continue
		}
		if !relationshipModeMatches(rel.TargetMode, mode) {
			continue
		}
		c.add("relationship:"+sourceURI+":"+relationshipShortName(relType), relationshipDetail(rel))
		return
	}
	if len(typeMatched) > 0 {
		c.t.Fatalf("relationships from %s include type %s but not target %q mode %q: %+v", sourceURI, relType, target, mode, typeMatched)
	}
	c.t.Fatalf("relationships from %s missing type %s", sourceURI, relType)
}

func (c *generatedPackageFactCollector) requireElement(partURI, ns, localName string) {
	c.t.Helper()
	if !c.hasElement(partURI, ns, localName) {
		c.t.Fatalf("part %s missing element {%s}%s", partURI, ns, localName)
	}
	c.add("xml:"+partURI+":"+localName, "namespace="+ns)
}

func (c *generatedPackageFactCollector) requireRelationshipBackedElement(partURI, ns, localName, relType, mode string) {
	c.t.Helper()
	doc, err := c.pkg.ReadXMLPart(partURI)
	if err != nil {
		c.t.Fatalf("read XML part %s: %v", partURI, err)
	}
	for _, elem := range xlsxns.FindDescendants(doc.Root(), ns, localName) {
		rid := generatedRelationshipIDAttr(elem)
		if rid == "" {
			continue
		}
		for _, rel := range c.pkg.ListRelationships(partURI) {
			if rel.ID == rid && rel.Type == relType && relationshipModeMatches(rel.TargetMode, mode) {
				c.add("xml-relationship:"+partURI+":"+localName, "rid="+rid+" "+relationshipDetail(rel))
				return
			}
		}
		c.t.Fatalf("element %s in %s references relationship %s, but no matching %s relationship with mode %s exists", localName, partURI, rid, relType, mode)
	}
	c.t.Fatalf("part %s has no %s element with r:id", partURI, localName)
}

func (c *generatedPackageFactCollector) requireStyledCell(partURI, cellRef string) {
	c.t.Helper()
	doc, err := c.pkg.ReadXMLPart(partURI)
	if err != nil {
		c.t.Fatalf("read XML part %s: %v", partURI, err)
	}
	for _, cell := range xlsxns.FindDescendants(doc.Root(), xlsxns.NsSpreadsheetML, "c") {
		if strings.TrimSpace(cell.SelectAttrValue("r", "")) != cellRef {
			continue
		}
		styleIndex := strings.TrimSpace(cell.SelectAttrValue("s", ""))
		if styleIndex == "" || styleIndex == "0" {
			c.t.Fatalf("cell %s in %s has style index %q, want non-default style", cellRef, partURI, styleIndex)
		}
		c.add("xml:"+partURI+":cell-style", "cell="+cellRef+" styleIndex="+styleIndex)
		return
	}
	c.t.Fatalf("part %s has no cell %s", partURI, cellRef)
}

func (c *generatedPackageFactCollector) hasElement(partURI, ns, localName string) bool {
	c.t.Helper()
	doc, err := c.pkg.ReadXMLPart(partURI)
	if err != nil {
		c.t.Fatalf("read XML part %s: %v", partURI, err)
	}
	root := doc.Root()
	if root == nil {
		return false
	}
	if xlsxns.IsElement(root, ns, localName) {
		return true
	}
	return len(xlsxns.FindDescendants(root, ns, localName)) > 0
}

func (c *generatedPackageFactCollector) add(name, detail string) {
	c.facts = append(c.facts, generatedRepairPackageFact{
		Name:   name,
		Status: "passed",
		Detail: detail,
	})
}

func relationshipModeMatches(actual, expected string) bool {
	switch expected {
	case "internal":
		return !strings.EqualFold(strings.TrimSpace(actual), "External")
	case "External":
		return strings.EqualFold(strings.TrimSpace(actual), "External")
	default:
		return strings.TrimSpace(actual) == expected
	}
}

func relationshipDetail(rel opc.RelationshipInfo) string {
	mode := strings.TrimSpace(rel.TargetMode)
	if mode == "" {
		mode = "internal"
	}
	return "target=" + rel.Target + " mode=" + mode
}

func relationshipShortName(relType string) string {
	if idx := strings.LastIndex(relType, "/"); idx >= 0 && idx+1 < len(relType) {
		return relType[idx+1:]
	}
	return relType
}

func generatedRelationshipIDAttr(elem *etree.Element) string {
	if attr := elem.SelectAttr("r:id"); attr != nil {
		return strings.TrimSpace(attr.Value)
	}
	for _, attr := range elem.Attr {
		if attr.Key == "id" && attr.Space == "r" {
			return strings.TrimSpace(attr.Value)
		}
	}
	return ""
}
