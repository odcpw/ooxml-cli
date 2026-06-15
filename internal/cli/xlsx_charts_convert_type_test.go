package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

// stageChartByType authors a single-series chart of the given create type.
func stageChartByType(t *testing.T, createType string) string {
	t.Helper()
	data := stageChartData(t, `[["Region","Sales"],["North",42],["South",58],["East",30]]`)
	out := filepath.Join(t.TempDir(), createType+"chart.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data,
		"--type", createType, "--sheet", "1", "--range", "A1:B4", "--title", "Original", "--anchor", "E1", "--out", out); err != nil {
		t.Fatalf("stage %s chart failed: %v", createType, err)
	}
	return out
}

func TestXLSXChartsConvertTypeLineToColumn(t *testing.T) {
	chartPath := stageChartByType(t, "line")
	out := filepath.Join(t.TempDir(), "converted.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "convert-type", chartPath,
		"--sheet", "1", "--chart", "1", "--to", "column", "--expect-type", "line", "--out", out)
	if err != nil {
		t.Fatalf("convert-type failed: %v\n%s", err, output)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.Action != "xlsx.chart.convert-type" || res.Output != out || res.DryRun {
		t.Fatalf("unexpected envelope: %+v", res)
	}
	if res.PreviousType != "line" || res.NewType != "column" {
		t.Fatalf("unexpected previous/new type: %+v", res)
	}
	if res.Chart == nil || res.Chart.Style == nil {
		t.Fatalf("missing chart style readback: %+v", res)
	}
	// bar and column both render as the barChart element; assert via newType, not types.
	if len(res.Chart.Style.Types) != 1 || res.Chart.Style.Types[0] != "barChart" {
		t.Fatalf("expected barChart element readback, got %v", res.Chart.Style.Types)
	}
	if len(res.Chart.Style.Axes) != 2 {
		t.Fatalf("expected 2 axes after column conversion: %+v", res.Chart.Style.Axes)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate after convert failed: %v", err)
	}
	// Generated show command reads the converted chart back.
	showStyle := xlsxChartStyleFromShowCommand(t, res.ChartShowCommand)
	if len(showStyle.Types) != 1 || showStyle.Types[0] != "barChart" {
		t.Fatalf("show readback type mismatch: %v", showStyle.Types)
	}
	openWithLibreOfficeIfAvailable(t, out)
}

// TestXLSXChartsConvertTypeMatrix exercises every compatible pair plus the
// documented rejections from each compatible source type.
func TestXLSXChartsConvertTypeMatrix(t *testing.T) {
	type tc struct {
		from   string // create type
		to     string
		reject bool
	}
	cases := []tc{
		{"line", "column", false},
		{"line", "bar", false},
		{"line", "area", false},
		{"line", "scatter", false},
		{"line", "pie", false}, // single series, allowed
		{"bar", "line", false},
		{"bar", "area", false},
		{"bar", "scatter", false},
		{"area", "line", false},
		{"area", "column", false},
		{"scatter", "line", false},
		{"scatter", "bar", false},
		{"pie", "bar", true}, // from pie is rejected
		{"pie", "line", true},
		{"pie", "scatter", true},
	}
	for _, c := range cases {
		c := c
		t.Run(c.from+"_to_"+c.to, func(t *testing.T) {
			chartPath := stageChartByType(t, c.from)
			out := filepath.Join(t.TempDir(), "m.xlsx")
			args := []string{"--format", "json", "xlsx", "charts", "convert-type", chartPath,
				"--sheet", "1", "--chart", "1", "--to", c.to, "--out", out}
			output, err := executeRootForXLSXTest(t, args...)
			if c.reject {
				if err == nil {
					t.Fatalf("expected %s->%s to be rejected, got %s", c.from, c.to, output)
				}
				return
			}
			if err != nil {
				t.Fatalf("%s->%s failed: %v\n%s", c.from, c.to, err, output)
			}
			if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
				t.Fatalf("validate %s->%s failed: %v", c.from, c.to, err)
			}
		})
	}
}

func TestXLSXChartsConvertTypeMultiSeriesToPieRejected(t *testing.T) {
	data := stageChartData(t, `[["Region","Q1","Q2"],["North",42,10],["South",58,20],["East",30,15]]`)
	chartPath := filepath.Join(t.TempDir(), "multi.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "create", data,
		"--type", "line", "--sheet", "1", "--range", "A1:C4", "--title", "Multi", "--anchor", "E1", "--out", chartPath); err != nil {
		t.Fatalf("stage multi-series chart failed: %v", err)
	}
	out := filepath.Join(t.TempDir(), "x.xlsx")
	args := []string{"--format", "json", "xlsx", "charts", "convert-type", chartPath, "--sheet", "1", "--chart", "1", "--to", "pie", "--out", out}
	output, err := executeRootForXLSXTest(t, args...)
	if err == nil {
		t.Fatalf("expected multi-series->pie rejection, got %s", output)
	}
	assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	if !strings.Contains(err.Error(), "single series") {
		t.Fatalf("rejection should name the single-series constraint: %v", err)
	}
}

func TestXLSXChartsConvertTypeScatterWarning(t *testing.T) {
	chartPath := stageChartByType(t, "line")
	out := filepath.Join(t.TempDir(), "sc.xlsx")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "convert-type", chartPath,
		"--sheet", "1", "--chart", "1", "--to", "scatter", "--out", out)
	if err != nil {
		t.Fatalf("convert to scatter failed: %v\n%s", err, output)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	found := false
	for _, w := range res.Warnings {
		if strings.Contains(w, "numeric x-values") {
			found = true
		}
	}
	if !found {
		t.Fatalf("expected non-numeric x warning, got %v", res.Warnings)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("validate scatter failed: %v", err)
	}
}

func TestXLSXChartsConvertTypeDryRun(t *testing.T) {
	chartPath := stageChartByType(t, "line")
	output, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "charts", "convert-type", chartPath,
		"--sheet", "1", "--chart", "1", "--to", "column", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run failed: %v\n%s", err, output)
	}
	var res XLSXChartStyleResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal dry-run: %v\n%s", err, output)
	}
	if !res.DryRun || res.Output != "" {
		t.Fatalf("unexpected dry-run envelope: %+v", res)
	}
	if res.NewType != "column" {
		t.Fatalf("dry-run should report the target type: %+v", res)
	}
}

func TestXLSXChartsConvertTypeInPlace(t *testing.T) {
	chartPath := stageChartByType(t, "line")
	if _, err := executeRootForXLSXTest(t, "xlsx", "charts", "convert-type", chartPath,
		"--sheet", "1", "--chart", "1", "--to", "area", "--in-place"); err != nil {
		t.Fatalf("in-place convert failed: %v", err)
	}
	showStyle := xlsxChartStyleFromShowCommand(t, "ooxml --json xlsx charts show "+chartPath+" --chart 1")
	if len(showStyle.Types) != 1 || showStyle.Types[0] != "areaChart" {
		t.Fatalf("in-place output not converted: %v", showStyle.Types)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", chartPath); err != nil {
		t.Fatalf("validate in-place failed: %v", err)
	}
}

func TestXLSXChartsConvertTypeGuards(t *testing.T) {
	chartPath := stageChartByType(t, "line")
	out := filepath.Join(t.TempDir(), "g.xlsx")
	cases := [][]string{
		// missing --to
		{"xlsx", "charts", "convert-type", chartPath, "--sheet", "1", "--chart", "1", "--out", out},
		// invalid --to
		{"xlsx", "charts", "convert-type", chartPath, "--sheet", "1", "--chart", "1", "--to", "bubble", "--out", out},
		// expect-type mismatch
		{"xlsx", "charts", "convert-type", chartPath, "--sheet", "1", "--chart", "1", "--to", "column", "--expect-type", "pie", "--out", out},
		// identical type
		{"xlsx", "charts", "convert-type", chartPath, "--sheet", "1", "--chart", "1", "--to", "line", "--out", out},
	}
	for _, args := range cases {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}
