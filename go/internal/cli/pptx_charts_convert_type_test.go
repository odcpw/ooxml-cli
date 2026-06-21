package cli

import (
	"encoding/json"
	"os/exec"
	"path/filepath"
	"testing"
)

func TestPPTXChartsConvertTypeColumnToLine(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "converted.pptx")
	// The fixture chart is a barChart with barDir=col (i.e. a column chart).
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "convert-type", deckPath,
		"--chart", "chart:1", "--to", "line", "--expect-type", "column", "--out", out)
	if err != nil {
		t.Fatalf("pptx convert-type failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.Action != "pptx.chart.convert-type" || res.Output != out {
		t.Fatalf("unexpected envelope: %+v", res)
	}
	if res.PreviousType != "column" || res.NewType != "line" {
		t.Fatalf("unexpected previous/new type: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("missing chart style readback: %+v", res)
	}
	if len(res.Chart.Style.Types) != 1 || res.Chart.Style.Types[0] != "lineChart" {
		t.Fatalf("expected lineChart element readback, got %v", res.Chart.Style.Types)
	}
	if res.RenderCommand == "" {
		t.Fatalf("pptx convert-type should generate a render command: %+v", res)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after pptx convert failed: %v", err)
	}
	// Generated chart show command reads the converted chart back.
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, res.ChartShowCommand)
	var showRes PPTXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showRes); err != nil {
		t.Fatalf("unmarshal pptx chart show: %v\n%s", err, showOutput)
	}
	if len(showRes.Charts) != 1 || showRes.Charts[0].Style == nil ||
		len(showRes.Charts[0].Style.Types) != 1 || showRes.Charts[0].Style.Types[0] != "lineChart" {
		t.Fatalf("pptx chart show readback mismatch: %s", showOutput)
	}
	openPPTXWithLibreOfficeIfAvailable(t, out)
}

// openPPTXWithLibreOfficeIfAvailable converts a presentation to PDF headlessly
// to confirm the converted deck is openable (the xlsx helper exports to CSV,
// which has no PPTX filter).
func openPPTXWithLibreOfficeIfAvailable(t *testing.T, path string) {
	t.Helper()
	bin := ""
	for _, name := range []string{"libreoffice", "soffice"} {
		if p, err := exec.LookPath(name); err == nil {
			bin = p
			break
		}
	}
	if bin == "" {
		t.Log("libreoffice not available; skipping headless-open check")
		return
	}
	baselineOut := t.TempDir()
	baselineCmd := exec.Command(bin, libreOfficeUserInstallationArg(t), "--headless", "--convert-to", "pdf", "--outdir", baselineOut, pptxShapesFixturePath(t, "minimal-title"))
	if out, err := baselineCmd.CombinedOutput(); err != nil {
		t.Logf("libreoffice cannot open known-good PPTX fixture; skipping headless-open check: %v\n%s", err, out)
		return
	}
	outDir := t.TempDir()
	cmd := exec.Command(bin, libreOfficeUserInstallationArg(t), "--headless", "--convert-to", "pdf", "--outdir", outDir, path)
	if out, err := cmd.CombinedOutput(); err != nil {
		t.Fatalf("libreoffice failed to open %s: %v\n%s", path, err, out)
	}
}

func TestPPTXChartsConvertTypeColumnToPieSingleSeries(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "pie.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "convert-type", deckPath,
		"--chart", "chart:1", "--to", "pie", "--out", out)
	if err != nil {
		t.Fatalf("pptx convert to pie failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.NewType != "pie" || res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("unexpected pie envelope: %+v", res)
	}
	if len(res.Chart.Style.Axes) != 0 {
		t.Fatalf("pie chart should report no axes: %+v", res.Chart.Style.Axes)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate pptx pie failed: %v", err)
	}
}

// TestPPTXChartsConvertTypeColumnToScatter converts the real-world fixture chart
// (rich CT_CatAx tail) to scatter, exercising the catAx->valAx rename + prune of
// CT_CatAx-only children that Office would otherwise reject.
func TestPPTXChartsConvertTypeColumnToScatter(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "scatter.pptx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "convert-type", deckPath,
		"--chart", "chart:1", "--to", "scatter", "--out", out)
	if err != nil {
		t.Fatalf("pptx convert to scatter failed: %v\n%s", err, output)
	}
	var res PPTXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.NewType != "scatter" || res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("unexpected scatter envelope: %+v", res)
	}
	if len(res.Chart.Style.Types) != 1 || res.Chart.Style.Types[0] != "scatterChart" {
		t.Fatalf("expected scatterChart readback, got %v", res.Chart.Style.Types)
	}
	// Both axes must read back as value axes.
	for _, ax := range res.Chart.Style.Axes {
		if ax.Kind != "value" {
			t.Fatalf("scatter axes should all be value axes, got %+v", res.Chart.Style.Axes)
		}
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate pptx scatter failed: %v", err)
	}
	openPPTXWithLibreOfficeIfAvailable(t, out)
}

func TestPPTXChartsConvertTypeGuards(t *testing.T) {
	deckPath := getTestFilePath("chart-simple", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "g.pptx")
	cases := [][]string{
		{"pptx", "charts", "convert-type", deckPath, "--chart", "chart:1", "--out", out},
		{"pptx", "charts", "convert-type", deckPath, "--chart", "chart:1", "--to", "nope", "--out", out},
		{"pptx", "charts", "convert-type", deckPath, "--chart", "chart:1", "--to", "line", "--expect-type", "pie", "--out", out},
		{"pptx", "charts", "convert-type", deckPath, "--chart", "chart:1", "--to", "column", "--out", out}, // identical (fixture is column)
	}
	for _, args := range cases {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}
