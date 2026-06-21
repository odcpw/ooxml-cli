package cli

import (
	"encoding/json"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
)

type xlsxOpsWorkbookGolden struct {
	Table           xlsxOpsTableGolden           `json:"table"`
	Export          xlsxOpsExportGolden          `json:"export"`
	Filters         xlsxOpsFiltersGolden         `json:"filters"`
	Freeze          xlsxOpsFreezeGolden          `json:"freeze"`
	DataValidations xlsxOpsDataValidationsGolden `json:"dataValidations"`
	Charts          xlsxOpsChartsGolden          `json:"charts"`
	Pivots          xlsxOpsPivotsGolden          `json:"pivots"`
}

type xlsxOpsTableGolden struct {
	Name         string `json:"name"`
	Range        string `json:"range"`
	DataRowCount int    `json:"dataRowCount"`
}

type xlsxOpsExportGolden struct {
	Range             string     `json:"range"`
	Rows              int        `json:"rows"`
	Cols              int        `json:"cols"`
	FormulaCount      int        `json:"formulaCount"`
	Values            [][]any    `json:"values"`
	Types             [][]string `json:"types"`
	Formulas          [][]any    `json:"formulas"`
	NumberFormatCodes [][]any    `json:"numberFormatCodes"`
}

type xlsxOpsFiltersGolden struct {
	AutoFilterRef string   `json:"autoFilterRef"`
	FilterValues  []string `json:"filterValues"`
	SortRef       string   `json:"sortRef"`
	SortColumns   []string `json:"sortColumns"`
}

type xlsxOpsFreezeGolden struct {
	Frozen      bool   `json:"frozen"`
	Rows        int    `json:"rows"`
	Cols        int    `json:"cols"`
	TopLeftCell string `json:"topLeftCell"`
}

type xlsxOpsDataValidationsGolden struct {
	Count int    `json:"count"`
	Range string `json:"range"`
	Type  string `json:"type"`
}

type xlsxOpsChartsGolden struct {
	Count         int      `json:"count"`
	Types         []string `json:"types"`
	Title         string   `json:"title"`
	SeriesCount   int      `json:"seriesCount"`
	CategoryRange string   `json:"categoryRange"`
	ValueRange    string   `json:"valueRange"`
	CategoryCount int      `json:"categoryCount"`
}

type xlsxOpsPivotsGolden struct {
	Count       int      `json:"count"`
	SourceRange string   `json:"sourceRange"`
	Rows        []string `json:"rows"`
	Values      []string `json:"values"`
}

// TestXLSXOpsWorkbookGoldenWorkflow is the practical workbook capstone for
// agent use: append records to a table, add formulas/formats/styles, apply
// filters/sort/freeze/data validation, author a chart and pivot, validate, then
// compare semantic readback to a compact golden JSON snapshot.
func TestXLSXOpsWorkbookGoldenWorkflow(t *testing.T) {
	dir := t.TempDir()
	step := 0
	next := func(label string) string {
		step++
		return filepath.Join(dir, fmt.Sprintf("%02d-%s.xlsx", step, strings.ReplaceAll(label, " ", "-")))
	}
	run := func(args ...string) string {
		t.Helper()
		out, err := executeRootForXLSXTest(t, args...)
		if err != nil {
			t.Fatalf("command failed: %v\nargs=%v\n%s", err, args, out)
		}
		return out
	}
	validate := func(path string) {
		t.Helper()
		run("validate", "--strict", path)
	}

	cur := writeTestXLSXWithTable(t, "A1:B3", false, "")

	prev := cur
	cur = next("append-records")
	appendOut := run("--format", "json", "xlsx", "tables", "append-records", prev,
		"--table", "Sales",
		"--expect-range", "A1:B3",
		"--records", `[{"Region":"North","Amount":30},{"Region":"South","Amount":40},{"Region":"Central","Amount":50}]`,
		"--out", cur)
	var appendResult XLSXTablesAppendRecordsResult
	mustUnmarshalXLSXGolden(t, appendOut, &appendResult)
	if appendResult.Range != "A1:B6" || appendResult.RowsAppended != 3 {
		t.Fatalf("unexpected append-records result: %+v", appendResult)
	}
	validate(cur)

	prev = cur
	cur = next("formula-column")
	run("--format", "json", "xlsx", "ranges", "set", prev,
		"--sheet", "Data",
		"--values", `{"range":"C1:C6","values":[["RunRate"],[{"formula":"B2*1.1"}],[{"formula":"B3*1.1"}],[{"formula":"B4*1.1"}],[{"formula":"B5*1.1"}],[{"formula":"B6*1.1"}]]}`,
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("table-format")
	run("--format", "json", "xlsx", "tables", "set-column-format", prev,
		"--table", "Sales",
		"--column", "Amount",
		"--expect-column", "Amount",
		"--preset", "currency",
		"--decimals", "0",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("header-style")
	run("--format", "json", "xlsx", "ranges", "set-style", prev,
		"--sheet", "Data",
		"--range", "A1:C1",
		"--font-bold",
		"--fill-color", "#D9EAF7",
		"--border-style", "thin",
		"--alignment-horizontal", "center",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("table-filter")
	run("--format", "json", "xlsx", "filters-sorts", "set-autofilter", prev,
		"--sheet", "Data",
		"--range", "A1:C6",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("filter-values")
	run("--format", "json", "xlsx", "filters-sorts", "add-column-filter", prev,
		"--sheet", "Data",
		"--column", "0",
		"--values", "East,North,South",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("sort")
	run("--format", "json", "xlsx", "filters-sorts", "set-sort", prev,
		"--sheet", "Data",
		"--ref", "A1:C6",
		"--column", "B",
		"--descending",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("freeze")
	run("--format", "json", "xlsx", "freeze", "set", prev,
		"--sheet", "Data",
		"--rows", "1",
		"--expect-state", "none",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("validation")
	run("--format", "json", "xlsx", "data-validations", "create", prev,
		"--sheet", "Data",
		"--range", "A2:A6",
		"--type", "list",
		"--list-values", "East,West,North,South,Central",
		"--out", cur)
	validate(cur)

	prev = cur
	cur = next("chart")
	chartOut := run("--format", "json", "xlsx", "charts", "create", prev,
		"--type", "bar",
		"--table", "Sales",
		"--expect-source-range", "A1:B6",
		"--title", "Sales by Region",
		"--anchor", "E2",
		"--out", cur)
	var chartResult XLSXChartsCreateResult
	mustUnmarshalXLSXGolden(t, chartOut, &chartResult)
	if chartResult.SeriesCount != 1 || chartResult.Categories != 5 {
		t.Fatalf("unexpected chart result: %+v", chartResult)
	}
	validate(cur)

	prev = cur
	cur = next("pivot")
	pivotOut := run("--format", "json", "xlsx", "pivots", "create", prev,
		"--table", "Sales",
		"--expect-source-range", "A1:B6",
		"--rows", "Region",
		"--values", "Amount:sum",
		"--anchor", "E20",
		"--out", cur)
	var pivotResult XLSXPivotsCreateResult
	mustUnmarshalXLSXGolden(t, pivotOut, &pivotResult)
	if pivotResult.SourceRange != "A1:B6" || len(pivotResult.RowFields) != 1 || len(pivotResult.ValueFields) != 1 {
		t.Fatalf("unexpected pivot result: %+v", pivotResult)
	}
	validate(cur)
	openWithLibreOfficeIfAvailable(t, cur)

	actual := collectXLSXOpsWorkbookGolden(t, cur)
	assertGoldenJSONValue(t, "xlsx_ops_workbook_summary.json", actual)
}

func collectXLSXOpsWorkbookGolden(t *testing.T, workbookPath string) xlsxOpsWorkbookGolden {
	t.Helper()
	var tables XLSXTablesResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "tables", "show", workbookPath, "--table", "Sales"), &tables)
	if len(tables.Tables) != 1 {
		t.Fatalf("expected one table, got %+v", tables.Tables)
	}

	var exported XLSXRangesExportResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "ranges", "export", workbookPath, "--sheet", "Data", "--range", "A1:C6", "--include-types", "--include-formulas", "--include-formats"), &exported)

	var sheetFilters XLSXFiltersSortsShowResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "filters-sorts", "show", workbookPath, "--sheet", "Data"), &sheetFilters)

	var freeze XLSXFreezeShowResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "freeze", "show", workbookPath, "--sheet", "Data"), &freeze)

	var dvs XLSXDataValidationsListResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "data-validations", "list", workbookPath, "--sheet", "Data"), &dvs)

	var charts XLSXChartsResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "charts", "list", workbookPath), &charts)

	var pivots XLSXPivotsResult
	mustUnmarshalXLSXGolden(t, executeGeneratedOrRootXLSXGolden(t, "--format", "json", "xlsx", "pivots", "list", workbookPath), &pivots)

	filterValues := []string{}
	if sheetFilters.AutoFilter != nil && len(sheetFilters.AutoFilter.Columns) > 0 {
		filterValues = append(filterValues, sheetFilters.AutoFilter.Columns[0].Values...)
	}
	sortRef := ""
	sortColumns := []string{}
	if sheetFilters.SortState != nil {
		sortRef = sheetFilters.SortState.Ref
		for _, cond := range sheetFilters.SortState.Conditions {
			dir := "asc"
			if cond.Descending {
				dir = "desc"
			}
			sortColumns = append(sortColumns, cond.Ref+" "+dir)
		}
	}
	if sheetFilters.AutoFilter == nil {
		t.Fatalf("expected autoFilter readback, got %+v", sheetFilters)
	}
	if freeze.State == nil {
		t.Fatalf("expected freeze readback, got %+v", freeze)
	}

	validationRange := ""
	validationType := ""
	if dvs.Count > 0 {
		if len(dvs.DataValidations) == 0 {
			t.Fatalf("data validation count/readback mismatch: %+v", dvs)
		}
		validationRange = dvs.DataValidations[0].Sqref
		validationType = dvs.DataValidations[0].Type
	}

	chartTypes := []string{}
	chartTitle := ""
	chartSeries, chartCategories := 0, 0
	categoryRange, valueRange := "", ""
	if len(charts.Charts) > 0 {
		chartTypes = append(chartTypes, charts.Charts[0].Types...)
		chartTitle = charts.Charts[0].Title
		chartSeries = len(charts.Charts[0].Series)
		if chartSeries > 0 && charts.Charts[0].Series[0].Categories != nil {
			categoryRange = charts.Charts[0].Series[0].Categories.Range
			chartCategories = charts.Charts[0].Series[0].Categories.PointCount
		}
		if chartSeries > 0 && charts.Charts[0].Series[0].Values != nil {
			valueRange = charts.Charts[0].Series[0].Values.Range
		}
	}

	pivotSource := ""
	pivotRows := []string{}
	pivotValues := []string{}
	if len(pivots.Pivots) > 0 {
		if pivots.Pivots[0].Cache != nil {
			pivotSource = pivots.Pivots[0].Cache.Source.Range
		}
		for _, field := range pivots.Pivots[0].RowFields {
			pivotRows = append(pivotRows, field.Name)
		}
		for _, field := range pivots.Pivots[0].DataFields {
			pivotValues = append(pivotValues, field.Name)
		}
	}

	return xlsxOpsWorkbookGolden{
		Table: xlsxOpsTableGolden{
			Name:         tables.Tables[0].DisplayName,
			Range:        tables.Tables[0].Range,
			DataRowCount: tables.Tables[0].DataRowCount,
		},
		Export: xlsxOpsExportGolden{
			Range:             exported.Range,
			Rows:              exported.Rows,
			Cols:              exported.Cols,
			FormulaCount:      exported.FormulaCount,
			Values:            exported.Values,
			Types:             exported.Types,
			Formulas:          exported.Formulas,
			NumberFormatCodes: exported.NumberFormatCodes,
		},
		Filters: xlsxOpsFiltersGolden{
			AutoFilterRef: sheetFilters.AutoFilter.Ref,
			FilterValues:  filterValues,
			SortRef:       sortRef,
			SortColumns:   sortColumns,
		},
		Freeze: xlsxOpsFreezeGolden{
			Frozen:      freeze.State != nil && freeze.State.Frozen,
			Rows:        freeze.State.Rows,
			Cols:        freeze.State.Cols,
			TopLeftCell: freeze.State.TopLeftCell,
		},
		DataValidations: xlsxOpsDataValidationsGolden{
			Count: dvs.Count,
			Range: validationRange,
			Type:  validationType,
		},
		Charts: xlsxOpsChartsGolden{
			Count:         len(charts.Charts),
			Types:         chartTypes,
			Title:         chartTitle,
			SeriesCount:   chartSeries,
			CategoryRange: categoryRange,
			ValueRange:    valueRange,
			CategoryCount: chartCategories,
		},
		Pivots: xlsxOpsPivotsGolden{
			Count:       len(pivots.Pivots),
			SourceRange: pivotSource,
			Rows:        pivotRows,
			Values:      pivotValues,
		},
	}
}

func executeGeneratedOrRootXLSXGolden(t *testing.T, args ...string) string {
	t.Helper()
	out, err := executeRootForXLSXTest(t, args...)
	if err != nil {
		t.Fatalf("command failed: %v\nargs=%v\n%s", err, args, out)
	}
	return out
}

func mustUnmarshalXLSXGolden(t *testing.T, data string, v any) {
	t.Helper()
	if err := json.Unmarshal([]byte(data), v); err != nil {
		t.Fatalf("failed to unmarshal JSON: %v\n%s", err, data)
	}
}
