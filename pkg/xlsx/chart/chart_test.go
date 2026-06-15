package chart

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

func TestListWorksheetCharts(t *testing.T) {
	workbookPath := filepath.Join("..", "..", "..", "testdata", "xlsx", "chart-workbook", "workbook.xlsx")
	pkg, err := opc.Open(workbookPath)
	if err != nil {
		t.Fatalf("failed to open chart workbook: %v", err)
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("failed to parse workbook: %v", err)
	}
	charts, err := List(pkg, workbook, nil)
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(charts) != 1 {
		t.Fatalf("charts len = %d, want 1", len(charts))
	}
	chart := charts[0]
	if chart.Name != "Revenue Chart 1" || chart.Title != "Revenue by Region" || chart.Sheet != "Data" || chart.PartURI != "/xl/charts/chart1.xml" {
		t.Fatalf("unexpected chart metadata: %+v", chart)
	}
	if len(chart.Types) != 1 || chart.Types[0] != "barChart" {
		t.Fatalf("unexpected chart types: %+v", chart.Types)
	}
	if len(chart.Series) != 1 || chart.Series[0].Categories == nil || chart.Series[0].Values == nil {
		t.Fatalf("unexpected series: %+v", chart.Series)
	}
	if chart.Series[0].Categories.Range != "$A$2:$A$4" || chart.Series[0].Values.CacheType != "numCache" || chart.Series[0].Values.PointCount != 3 {
		t.Fatalf("unexpected series sources: %+v", chart.Series[0])
	}
}

func TestSetSeriesSourceUpdatesFormulaAndCache(t *testing.T) {
	workbookPath := filepath.Join("..", "..", "..", "testdata", "xlsx", "chart-workbook", "workbook.xlsx")
	pkg, err := opc.Open(workbookPath)
	if err != nil {
		t.Fatalf("failed to open chart workbook: %v", err)
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("failed to parse workbook: %v", err)
	}
	charts, err := List(pkg, workbook, nil)
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	if len(charts) != 1 {
		t.Fatalf("charts len = %d, want 1", len(charts))
	}
	result, err := SetSeriesSource(&SetSeriesSourceRequest{
		Package:           pkg,
		ChartURI:          charts[0].PartURI,
		SeriesNumber:      1,
		Role:              RoleValues,
		Formula:           "Data!$B$2:$B$3",
		CacheMode:         CacheModeAuto,
		CachePoints:       []CachePoint{{Index: 0, Value: "100"}, {Index: 1, Value: "120"}},
		ExpectSourceRange: "$B$2:$B$4",
	})
	if err != nil {
		t.Fatalf("SetSeriesSource failed: %v", err)
	}
	if result.PreviousFormula != "Data!$B$2:$B$4" || result.Formula != "Data!$B$2:$B$3" || result.RefKind != "numRef" || result.CacheType != "numCache" || result.CachePointCount != 2 {
		t.Fatalf("unexpected mutation result: %+v", result)
	}
	if len(result.Warnings) != 1 || !strings.Contains(result.Warnings[0], "categories has 3") {
		t.Fatalf("unexpected point-count warnings: %+v", result.Warnings)
	}

	updated, err := List(pkg, workbook, nil)
	if err != nil {
		t.Fatalf("List after mutation failed: %v", err)
	}
	values := updated[0].Series[0].Values
	if values == nil || values.Formula != "Data!$B$2:$B$3" || values.Range != "$B$2:$B$3" || values.PointCount != 2 || !containsChartTestString(values.CachePreview, "120") {
		t.Fatalf("updated values source not read back: %+v", values)
	}
	categories := updated[0].Series[0].Categories
	if categories == nil || categories.Range != "$A$2:$A$4" || categories.PointCount != 3 {
		t.Fatalf("categories source should be unchanged: %+v", categories)
	}
}

func TestSetSeriesSourceRejectsStaleSourceGuard(t *testing.T) {
	workbookPath := filepath.Join("..", "..", "..", "testdata", "xlsx", "chart-workbook", "workbook.xlsx")
	pkg, err := opc.Open(workbookPath)
	if err != nil {
		t.Fatalf("failed to open chart workbook: %v", err)
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("failed to parse workbook: %v", err)
	}
	charts, err := List(pkg, workbook, nil)
	if err != nil {
		t.Fatalf("List failed: %v", err)
	}
	_, err = SetSeriesSource(&SetSeriesSourceRequest{
		Package:           pkg,
		ChartURI:          charts[0].PartURI,
		SeriesNumber:      1,
		Role:              RoleValues,
		Formula:           "Data!$B$2:$B$3",
		CacheMode:         CacheModeClear,
		ExpectSourceRange: "$B$2:$B$99",
	})
	if err == nil || !strings.Contains(err.Error(), "chart source range mismatch") {
		t.Fatalf("expected stale range guard error, got %v", err)
	}
}

func containsChartTestString(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func TestSplitSheetRangeFormulaOnlyAcceptsLocalA1Ranges(t *testing.T) {
	tests := []struct {
		formula   string
		wantSheet string
		wantRange string
	}{
		{formula: "Data!$A$1:$B$2", wantSheet: "Data", wantRange: "$A$1:$B$2"},
		{formula: "'Sales Data'!B2:B4", wantSheet: "Sales Data", wantRange: "B2:B4"},
		{formula: "'O''Brien'!$C$3", wantSheet: "O'Brien", wantRange: "$C$3"},
		{formula: "Table1[Amount]"},
		{formula: "[Book.xlsx]Data!$A$1"},
		{formula: "Data!Table1[Amount]"},
	}
	for _, tt := range tests {
		t.Run(tt.formula, func(t *testing.T) {
			gotSheet, gotRange := splitSheetRangeFormula(tt.formula)
			if gotSheet != tt.wantSheet || gotRange != tt.wantRange {
				t.Fatalf("splitSheetRangeFormula(%q) = (%q, %q), want (%q, %q)", tt.formula, gotSheet, gotRange, tt.wantSheet, tt.wantRange)
			}
		})
	}
}
