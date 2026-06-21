package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestPPTXChartsCreateInlineBarValidatesAndReadsBack(t *testing.T) {
	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "created.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar", "--title", "Sales",
		"--values-json", `[["Region","S1","S2"],["North",10,15],["South",20,25],["East",30,35]]`,
		"--out", out)
	if err != nil {
		t.Fatalf("pptx charts create failed: %v\n%s", err, output)
	}

	var res PPTXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal create result: %v\n%s", err, output)
	}
	if res.Action != "pptx.chart.create" || res.Output != out {
		t.Fatalf("unexpected envelope: %+v", res)
	}
	if res.ChartType != "bar" || res.SeriesCount != 2 || res.Categories != 3 {
		t.Fatalf("unexpected chart shape: type=%s series=%d cats=%d", res.ChartType, res.SeriesCount, res.Categories)
	}
	if res.SourceMode != "inline" || res.ChartPartURI != "/ppt/charts/chart1.xml" {
		t.Fatalf("unexpected source/part: mode=%s part=%s", res.SourceMode, res.ChartPartURI)
	}
	if res.ChartRelationshipID == "" || res.ShapeID == 0 || res.ShapeName == "" {
		t.Fatalf("missing wiring identity: %+v", res)
	}
	if res.CX <= 0 || res.CY <= 0 {
		t.Fatalf("expected positive geometry: %+v", res)
	}
	if res.Chart == nil || res.Chart.Title != "Sales" || len(res.Chart.Types) != 1 || res.Chart.Types[0] != "barChart" {
		t.Fatalf("unexpected chart readback: %+v", res.Chart)
	}
	if len(res.Chart.Series) != 2 {
		t.Fatalf("expected 2 series in readback, got %d", len(res.Chart.Series))
	}
	cat := res.Chart.Series[0].Categories
	if cat == nil || cat.PointCount != 3 || !containsString(cat.CachePreview, "South") {
		t.Fatalf("unexpected category cache: %+v", cat)
	}

	// Strict-validate the output.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after create failed: %v", err)
	}

	// Generated readback commands resolve the new chart.
	if res.ValidateCommand == "" || res.RenderCommand == "" || res.ChartsListCommand == "" || res.ChartShowCommand == "" {
		t.Fatalf("missing generated commands: %+v", res)
	}
	if !strings.Contains(res.ChartShowCommand, "--chart part:/ppt/charts/chart1.xml") {
		t.Fatalf("chart show command should use a part selector: %q", res.ChartShowCommand)
	}
	showOutput := executeGeneratedOOXMLCommandForXLSXTest(t, res.ChartShowCommand)
	var showRes PPTXChartsResult
	if err := json.Unmarshal([]byte(showOutput), &showRes); err != nil {
		t.Fatalf("unmarshal chart show: %v\n%s", err, showOutput)
	}
	if len(showRes.Charts) != 1 || showRes.Charts[0].PartURI != "/ppt/charts/chart1.xml" {
		t.Fatalf("chart show readback mismatch: %s", showOutput)
	}

	listOutput := executeGeneratedOOXMLCommandForXLSXTest(t, res.ChartsListCommand)
	var listRes PPTXChartsResult
	if err := json.Unmarshal([]byte(listOutput), &listRes); err != nil {
		t.Fatalf("unmarshal charts list: %v\n%s", err, listOutput)
	}
	if len(listRes.Charts) != 1 {
		t.Fatalf("expected 1 chart on slide 1, got %d", len(listRes.Charts))
	}

	openPPTXWithLibreOfficeIfAvailable(t, out)
}

func TestPPTXChartsCreateExplicitZeroOffsetAnchorsAtOrigin(t *testing.T) {
	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "anchored.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar",
		"--values-json", `[["Region","S1"],["North",10],["South",20]]`,
		"--x", "0", "--y", "0",
		"--out", out)
	if err != nil {
		t.Fatalf("pptx charts create with --x 0 --y 0 failed: %v\n%s", err, output)
	}

	var res PPTXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal create result: %v\n%s", err, output)
	}
	// Explicit --x 0 / --y 0 must anchor at the slide's top/left edge, not be
	// silently recentred.
	if res.X != 0 || res.Y != 0 {
		t.Fatalf("expected chart anchored at offset (0,0), got x=%d y=%d", res.X, res.Y)
	}
	// Width/height stay defaulted (positive) since they were not supplied.
	if res.CX <= 0 || res.CY <= 0 {
		t.Fatalf("expected positive default size: cx=%d cy=%d", res.CX, res.CY)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after anchored create failed: %v", err)
	}
}

func TestPPTXChartsCreateOmittedOffsetRecentres(t *testing.T) {
	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "centred.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar",
		"--values-json", `[["Region","S1"],["North",10],["South",20]]`,
		"--out", out)
	if err != nil {
		t.Fatalf("pptx charts create without offsets failed: %v\n%s", err, output)
	}

	var res PPTXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal create result: %v\n%s", err, output)
	}
	// When --x/--y are omitted the chart is recentred, so the offset is positive.
	if res.X <= 0 || res.Y <= 0 {
		t.Fatalf("expected omitted offsets to recentre with positive x/y, got x=%d y=%d", res.X, res.Y)
	}
}

func TestPPTXChartsCreateTypesValidate(t *testing.T) {
	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	for _, chartType := range []string{"line", "area", "pie", "scatter"} {
		out := filepath.Join(t.TempDir(), chartType+".pptx")
		output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
			"--slide", "1", "--type", chartType,
			"--values-json", `[["Region","S1","S2"],["North",10,15],["South",20,25]]`,
			"--out", out)
		if err != nil {
			t.Fatalf("create %s failed: %v\n%s", chartType, err, output)
		}
		var res PPTXChartsCreateResult
		if err := json.Unmarshal([]byte(output), &res); err != nil {
			t.Fatalf("unmarshal %s: %v\n%s", chartType, err, output)
		}
		if res.ChartType != chartType {
			t.Fatalf("expected chart type %s, got %s", chartType, res.ChartType)
		}
		// Pie coerces to a single series.
		if chartType == "pie" {
			if res.SeriesCount != 1 {
				t.Fatalf("pie should coerce to 1 series, got %d", res.SeriesCount)
			}
			if !containsString(res.Warnings, "pie chart uses only the first series") {
				t.Fatalf("expected pie coercion warning: %+v", res.Warnings)
			}
		}
		if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
			t.Fatalf("validate %s failed: %v", chartType, err)
		}
	}
}

func TestPPTXChartsCreateFromXLSXRangeEmbedsWorkbook(t *testing.T) {
	deckPath := getTestFilePath("multi-layout", "presentation.pptx")
	sourcePath := getTestFilePath("../xlsx/chart-workbook", "workbook.xlsx")
	out := filepath.Join(t.TempDir(), "external.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar",
		"--source-file", sourcePath, "--source-sheet", "Data", "--source-range", "A1:B4",
		"--expect-source-range", "A1:B4", "--embed-workbook",
		"--out", out)
	if err != nil {
		t.Fatalf("create from xlsx range failed: %v\n%s", err, output)
	}
	var res PPTXChartsCreateResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal external create: %v\n%s", err, output)
	}
	if res.SourceMode != "external" || res.SourceSheet != "Data" || res.SourceRange != "A1:B4" {
		t.Fatalf("unexpected external source metadata: %+v", res)
	}
	if res.EmbeddedWorkbookPartURI == "" {
		t.Fatalf("expected an embedded workbook part: %+v", res)
	}
	if res.Chart == nil || res.Chart.EmbeddedWorkbookPartURI != res.EmbeddedWorkbookPartURI {
		t.Fatalf("embedded workbook not visible in readback: %+v", res.Chart)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate external create failed: %v", err)
	}

	// The embedded workbook makes the chart editable: update-data should update
	// the embedded cells, not just the cache.
	updated := filepath.Join(t.TempDir(), "external-updated.pptx")
	updateOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "update-data", out,
		"--chart", "part:"+res.ChartPartURI, "--series", "1", "--values", "99,88,77", "--out", updated)
	if err != nil {
		t.Fatalf("update-data on created chart failed: %v\n%s", err, updateOut)
	}
	var updRes PPTXChartUpdateDataResult
	if err := json.Unmarshal([]byte(updateOut), &updRes); err != nil {
		t.Fatalf("unmarshal update-data: %v\n%s", err, updateOut)
	}
	if !updRes.EmbeddedWorkbookUpdated {
		t.Fatalf("expected embedded workbook to be updated: %+v", updRes)
	}

	openPPTXWithLibreOfficeIfAvailable(t, out)
}

func TestPPTXChartsCreateDryRunAndGuards(t *testing.T) {
	deckPath := getTestFilePath("multi-layout", "presentation.pptx")

	// Dry-run produces templates and no output file.
	dryOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar",
		"--values-json", `[["R","S"],["A",1]]`, "--dry-run")
	if err != nil {
		t.Fatalf("dry-run create failed: %v\n%s", err, dryOut)
	}
	var dryRes PPTXChartsCreateResult
	if err := json.Unmarshal([]byte(dryOut), &dryRes); err != nil {
		t.Fatalf("unmarshal dry-run: %v\n%s", err, dryOut)
	}
	if !dryRes.DryRun || dryRes.Output != "" {
		t.Fatalf("dry-run should not write output: %+v", dryRes)
	}
	if dryRes.ValidateCommandTpl == "" || dryRes.ChartShowCommandTpl == "" {
		t.Fatalf("dry-run should emit command templates: %+v", dryRes)
	}

	// Invalid chart type is rejected.
	if _, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bogus",
		"--values-json", `[["R","S"],["A",1]]`, "--dry-run"); err == nil {
		t.Fatalf("expected invalid chart type to be rejected")
	}

	// Out-of-range slide is rejected.
	if _, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "999", "--type", "bar",
		"--values-json", `[["R","S"],["A",1]]`, "--dry-run"); err == nil {
		t.Fatalf("expected out-of-range slide to be rejected")
	}

	// Missing source is rejected.
	if _, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "charts", "create", deckPath,
		"--slide", "1", "--type", "bar", "--dry-run"); err == nil {
		t.Fatalf("expected missing source to be rejected")
	}
}
