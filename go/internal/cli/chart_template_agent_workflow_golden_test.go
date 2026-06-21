package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
)

type chartTemplateAgentWorkflowGolden struct {
	Workflow string                      `json:"workflow"`
	Tokens   chartTemplateTokensGolden   `json:"tokens"`
	PPTX     chartTemplateFamilyGolden   `json:"pptx"`
	XLSX     chartTemplateFamilyGolden   `json:"xlsx"`
	Commands chartTemplateCommandsGolden `json:"commands"`
}

type chartTemplateTokensGolden struct {
	PPTXChartStyles int `json:"pptxChartStyles"`
	XLSXChartStyles int `json:"xlsxChartStyles"`
}

type chartTemplateFamilyGolden struct {
	TargetType             string                   `json:"targetType"`
	TotalUpdates           int                      `json:"totalUpdates"`
	AppliedCharts          []TemplateAppliedChart   `json:"appliedCharts"`
	Skipped                []string                 `json:"skipped"`
	Chart                  chartTemplateChartGolden `json:"chart"`
	ValidationStatus       string                   `json:"validationStatus"`
	ValidateCommandPresent bool                     `json:"validateCommandPresent"`
}

type chartTemplateChartGolden struct {
	PrimarySelector string                    `json:"primarySelector"`
	PartURI         string                    `json:"partUri"`
	Title           string                    `json:"title"`
	Types           []string                  `json:"types"`
	SeriesCount     int                       `json:"seriesCount"`
	FirstSeries     chartTemplateSeriesGolden `json:"firstSeries"`
}

type chartTemplateSeriesGolden struct {
	Number    int    `json:"number"`
	Name      string `json:"name"`
	FillColor string `json:"fillColor"`
	LineColor string `json:"lineColor"`
}

type chartTemplateCommandsGolden struct {
	PPTXShowCommandPresent  bool `json:"pptxShowCommandPresent"`
	PPTXShowCommandHasSlide bool `json:"pptxShowCommandHasSlide"`
	PPTXShowCommandHasChart bool `json:"pptxShowCommandHasChart"`
	XLSXShowCommandPresent  bool `json:"xlsxShowCommandPresent"`
	XLSXShowCommandHasSheet bool `json:"xlsxShowCommandHasSheet"`
	XLSXShowCommandHasChart bool `json:"xlsxShowCommandHasChart"`
}

// TestChartTemplateAgentWorkflowGolden freezes the practical corporate-design
// chart path for agents: inspect template chart tokens, apply a compact tokens
// profile to PPTX and XLSX charts, read back chart style, then strict-validate
// both edited files. It catches regressions in the chart/template workflow
// without snapshotting temp paths or full OOXML XML.
func TestChartTemplateAgentWorkflowGolden(t *testing.T) {
	dir := t.TempDir()
	tokensPath := filepath.Join(dir, "chart-style-tokens.json")
	if err := os.WriteFile(tokensPath, []byte(chartTemplateTokensProfileJSON), 0o644); err != nil {
		t.Fatalf("write tokens profile: %v", err)
	}

	pptxFixture := "../../testdata/pptx/chart-simple/presentation.pptx"
	xlsxFixture := "../../testdata/xlsx/chart-workbook/workbook.xlsx"
	pptxTokens := readTemplateTokensForChartGolden(t, pptxFixture)
	xlsxTokens := readTemplateTokensForChartGolden(t, xlsxFixture)

	pptxOut := filepath.Join(dir, "branded.pptx")
	pptxApply := applyChartTemplateTokensForGolden(t, pptxFixture, tokensPath, pptxOut)
	pptxCharts := readPPTXChartForGolden(t, pptxOut)

	xlsxOut := filepath.Join(dir, "branded.xlsx")
	xlsxApply := applyChartTemplateTokensForGolden(t, xlsxFixture, tokensPath, xlsxOut)
	xlsxCharts := readXLSXChartForGolden(t, xlsxOut)

	actual := chartTemplateAgentWorkflowGolden{
		Workflow: "chart-template-apply-readback-validate",
		Tokens: chartTemplateTokensGolden{
			PPTXChartStyles: len(pptxTokens.PPTX.ChartStyles),
			XLSXChartStyles: len(xlsxTokens.XLSX.ChartStyles),
		},
		PPTX: chartTemplateFamilyGolden{
			TargetType:             pptxApply.TargetType,
			TotalUpdates:           pptxApply.TotalUpdates,
			AppliedCharts:          pptxApply.Applied.Charts,
			Skipped:                pptxApply.Skipped,
			Chart:                  summarizePPTXChartTemplateGolden(t, pptxCharts),
			ValidationStatus:       validateStatusForChartTemplateGolden(t, pptxOut),
			ValidateCommandPresent: pptxCharts.ValidateCommand != "",
		},
		XLSX: chartTemplateFamilyGolden{
			TargetType:             xlsxApply.TargetType,
			TotalUpdates:           xlsxApply.TotalUpdates,
			AppliedCharts:          xlsxApply.Applied.Charts,
			Skipped:                xlsxApply.Skipped,
			Chart:                  summarizeXLSXChartTemplateGolden(t, xlsxCharts),
			ValidationStatus:       validateStatusForChartTemplateGolden(t, xlsxOut),
			ValidateCommandPresent: xlsxCharts.ValidateCommand != "",
		},
		Commands: chartTemplateCommandsGolden{
			PPTXShowCommandPresent:  pptxCharts.Charts[0].ShowCommand != "",
			PPTXShowCommandHasSlide: strings.Contains(pptxCharts.Charts[0].ShowCommand, " --slide "),
			PPTXShowCommandHasChart: strings.Contains(pptxCharts.Charts[0].ShowCommand, " --chart "),
			XLSXShowCommandPresent:  xlsxCharts.Charts[0].ShowCommand != "",
			XLSXShowCommandHasSheet: strings.Contains(xlsxCharts.Charts[0].ShowCommand, " --sheet "),
			XLSXShowCommandHasChart: strings.Contains(xlsxCharts.Charts[0].ShowCommand, " --chart "),
		},
	}
	assertGoldenJSONValue(t, "chart_template_agent_workflow_summary.json", actual)
}

const chartTemplateTokensProfileJSON = `{
  "schemaVersion": "1.0",
  "type": "pptx",
  "source": "chart-template-agent-workflow-golden",
  "pptx": {
    "theme": null,
    "defaultTextStyles": [],
    "tableStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  },
  "xlsx": {
    "theme": null,
    "namedCellStyles": [],
    "chartStyles": [
      {
        "partUri": "/template/chart.xml",
        "seriesFillColor": "FF0000",
        "seriesLineColor": "00FF00"
      }
    ]
  }
}
`

func readTemplateTokensForChartGolden(t *testing.T, path string) tmpl.TemplateTokens {
	t.Helper()
	out, err := runTemplateTokens(t, "--json", "template", "tokens", path)
	if err != nil {
		t.Fatalf("template tokens failed for %s: %v\n%s", path, err, out)
	}
	var tokens tmpl.TemplateTokens
	mustUnmarshalChartTemplateGolden(t, out, &tokens)
	if strings.HasSuffix(path, ".pptx") && (tokens.PPTX == nil || len(tokens.PPTX.ChartStyles) == 0) {
		t.Fatalf("PPTX tokens missing chart styles: %+v", tokens.PPTX)
	}
	if strings.HasSuffix(path, ".xlsx") && (tokens.XLSX == nil || len(tokens.XLSX.ChartStyles) == 0) {
		t.Fatalf("XLSX tokens missing chart styles: %+v", tokens.XLSX)
	}
	return tokens
}

func applyChartTemplateTokensForGolden(t *testing.T, inputPath, tokensPath, outputPath string) TemplateApplyResult {
	t.Helper()
	out, err := runTemplateApply(t, "--json", "template", "apply", inputPath, "--tokens", tokensPath, "--target-charts", "--out", outputPath)
	if err != nil {
		t.Fatalf("template apply failed for %s: %v\n%s", inputPath, err, out)
	}
	var result TemplateApplyResult
	mustUnmarshalChartTemplateGolden(t, out, &result)
	if result.Output != outputPath || result.TotalUpdates == 0 || len(result.Applied.Charts) == 0 {
		t.Fatalf("unexpected template apply result: %+v", result)
	}
	return result
}

func readPPTXChartForGolden(t *testing.T, path string) PPTXChartsResult {
	t.Helper()
	out, err := executeRootForXLSXTest(t, "--json", "pptx", "charts", "show", path, "--chart", "chart:1")
	if err != nil {
		t.Fatalf("pptx charts show failed: %v\n%s", err, out)
	}
	var result PPTXChartsResult
	mustUnmarshalChartTemplateGolden(t, out, &result)
	if len(result.Charts) != 1 {
		t.Fatalf("expected one PPTX chart readback, got %+v", result.Charts)
	}
	return result
}

func readXLSXChartForGolden(t *testing.T, path string) XLSXChartsResult {
	t.Helper()
	out, err := executeRootForXLSXTest(t, "--json", "xlsx", "charts", "show", path, "--chart", "chart:1")
	if err != nil {
		t.Fatalf("xlsx charts show failed: %v\n%s", err, out)
	}
	var result XLSXChartsResult
	mustUnmarshalChartTemplateGolden(t, out, &result)
	if len(result.Charts) != 1 {
		t.Fatalf("expected one XLSX chart readback, got %+v", result.Charts)
	}
	return result
}

func summarizePPTXChartTemplateGolden(t *testing.T, result PPTXChartsResult) chartTemplateChartGolden {
	t.Helper()
	chart := result.Charts[0]
	return chartTemplateChartGolden{
		PrimarySelector: chart.PrimarySelector,
		PartURI:         chart.PartURI,
		Title:           chart.Title,
		Types:           append([]string{}, chart.Types...),
		SeriesCount:     len(chart.Series),
		FirstSeries:     summarizeChartStyleSeriesForGolden(t, chart.Style),
	}
}

func summarizeXLSXChartTemplateGolden(t *testing.T, result XLSXChartsResult) chartTemplateChartGolden {
	t.Helper()
	chart := result.Charts[0]
	return chartTemplateChartGolden{
		PrimarySelector: chart.PrimarySelector,
		PartURI:         chart.PartURI,
		Title:           chart.Title,
		Types:           append([]string{}, chart.Types...),
		SeriesCount:     len(chart.Series),
		FirstSeries:     summarizeChartStyleSeriesForGolden(t, chart.Style),
	}
}

func summarizeChartStyleSeriesForGolden(t *testing.T, style *xlsxchart.ChartStyle) chartTemplateSeriesGolden {
	t.Helper()
	if style == nil || len(style.Series) == 0 {
		t.Fatalf("chart style missing series readback: %+v", style)
	}
	series := style.Series[0]
	return chartTemplateSeriesGolden{
		Number:    series.Number,
		Name:      series.Name,
		FillColor: series.FillColor,
		LineColor: series.LineColor,
	}
}

func validateStatusForChartTemplateGolden(t *testing.T, path string) string {
	t.Helper()
	out, err := executeRootForXLSXTest(t, "validate", "--strict", path)
	if err != nil {
		t.Fatalf("validate failed for %s: %v\n%s", path, err, out)
	}
	if strings.Contains(out, "valid") {
		return "valid"
	}
	return strings.TrimSpace(out)
}

func mustUnmarshalChartTemplateGolden(t *testing.T, data string, target any) {
	t.Helper()
	if err := json.Unmarshal([]byte(data), target); err != nil {
		t.Fatalf("unmarshal JSON: %v\n%s", err, data)
	}
}
