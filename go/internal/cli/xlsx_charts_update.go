package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXChartUpdateSourceResult struct {
	File                           string         `json:"file"`
	Output                         string         `json:"output,omitempty"`
	DryRun                         bool           `json:"dryRun"`
	Action                         string         `json:"action"`
	Chart                          *XLSXChartItem `json:"chart,omitempty"`
	Series                         int            `json:"series"`
	Role                           string         `json:"role"`
	PreviousFormula                string         `json:"previousFormula,omitempty"`
	Formula                        string         `json:"formula"`
	Sheet                          string         `json:"sheet"`
	Range                          string         `json:"range"`
	RefKind                        string         `json:"refKind"`
	CacheType                      string         `json:"cacheType,omitempty"`
	CachePointCount                int            `json:"cachePointCount"`
	CachePreview                   []string       `json:"cachePreview,omitempty"`
	CacheSkipped                   int            `json:"cacheSkipped,omitempty"`
	CacheVerified                  bool           `json:"cacheVerified"`
	Warnings                       []string       `json:"warnings,omitempty"`
	ValidateCommand                string         `json:"validateCommand,omitempty"`
	ChartShowCommand               string         `json:"chartShowCommand,omitempty"`
	RangesExportCommand            string         `json:"rangesExportCommand,omitempty"`
	ValidateCommandTemplate        string         `json:"validateCommandTemplate,omitempty"`
	ChartShowCommandTemplate       string         `json:"chartShowCommandTemplate,omitempty"`
	RangesExportCommandTemplate    string         `json:"rangesExportCommandTemplate,omitempty"`
	StoredCacheContract            string         `json:"storedCacheContract"`
	SourceRangeExportCommand       string         `json:"sourceRangeExportCommand,omitempty"`
	SourceRangeExportCommandDryRun string         `json:"sourceRangeExportCommandDryRun,omitempty"`
}

var (
	xlsxChartsUpdateSheet             string
	xlsxChartsUpdateChart             string
	xlsxChartsUpdateSeries            int
	xlsxChartsUpdateRole              string
	xlsxChartsUpdateSourceSheet       string
	xlsxChartsUpdateSourceRange       string
	xlsxChartsUpdateFormula           string
	xlsxChartsUpdateCache             string
	xlsxChartsUpdateExpectSourceRange string
	xlsxChartsUpdateExpectFormula     string
	xlsxChartsUpdateMaxCells          int
)

var xlsxChartsUpdateSourceCmd = &cobra.Command{
	Use:   "update-source <file>",
	Short: "Update an existing chart source range",
	Long: `Update one source role on an existing worksheet chart series.

This command updates existing chart source formulas and stored chart caches for
simple local worksheet A1 ranges. It does not author new charts, edit pivot
charts, or recalculate formulas.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if xlsxChartsUpdateSeries < 1 {
			return InvalidArgsError("--series must be >= 1")
		}
		if _, err := xlsxchart.NormalizeSourceRole(xlsxChartsUpdateRole); err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		if strings.TrimSpace(xlsxChartsUpdateCache) == "" {
			xlsxChartsUpdateCache = xlsxchart.CacheModeAuto
		}
		if _, err := normalizeXLSXChartCacheMode(xlsxChartsUpdateCache); err != nil {
			return err
		}
		if xlsxChartsUpdateMaxCells < 0 {
			return InvalidArgsError("--max-cells must be >= 0")
		}
		formulaChanged := strings.TrimSpace(xlsxChartsUpdateFormula) != ""
		rangeChanged := strings.TrimSpace(xlsxChartsUpdateSourceRange) != ""
		if formulaChanged == rangeChanged {
			return InvalidArgsError("must specify exactly one of --formula or --source-range")
		}

		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXChartsUpdateSource(filePath, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXChartUpdateSourceJSON(cmd, result)
		}
		return outputXLSXChartUpdateSourceText(cmd, result)
	},
}

func performXLSXChartsUpdateSource(filePath string, mutOpts *MutationOptions, wantReadback bool) (*XLSXChartUpdateSourceResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXChartUpdateSourceResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheets := workbook.Sheets
		if strings.TrimSpace(xlsxChartsUpdateSheet) != "" {
			sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxChartsUpdateSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			sheets = []model.SheetRef{sheetRef}
		}
		charts, err := xlsxchart.List(pkg, workbook, sheets)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list charts: %v", err)
		}
		selected, err := selectXLSXChart(charts, xlsxChartsUpdateChart)
		if err != nil {
			return err
		}
		role, err := xlsxchart.NormalizeSourceRole(xlsxChartsUpdateRole)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		currentSource, err := chartSeriesSourceByRole(selected, xlsxChartsUpdateSeries, role)
		if err != nil {
			return err
		}
		sourceSheet, sourceRange, formula, rangeRef, err := resolveXLSXChartSourceFormula(workbook, currentSource)
		if err != nil {
			return err
		}
		rows, cols := xlsxRangeDimensions(rangeRef)
		if rows != 1 && cols != 1 {
			return NewCLIErrorf(ExitInvalidArgs, "chart source range %s is %dx%d; update-source currently requires a one-row or one-column series range", rangeRef.String(), rows, cols)
		}

		cacheMode, err := normalizeXLSXChartCacheMode(xlsxChartsUpdateCache)
		if err != nil {
			return err
		}
		var cachePoints []xlsxchart.CachePoint
		cacheSkipped := 0
		formatCode := ""
		var warnings []string
		if cacheMode == xlsxchart.CacheModeAuto {
			sourceSheetRef, err := selectXLSXSheet(workbook.Sheets, sourceSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sourceSheetRef); err != nil {
				return err
			}
			cachePoints, cacheSkipped, formatCode, warnings, err = collectXLSXChartCachePoints(pkg, workbook, sourceSheetRef, rangeRef, currentSource.RefKind)
			if err != nil {
				return err
			}
		}

		mutation, err := xlsxchart.SetSeriesSource(&xlsxchart.SetSeriesSourceRequest{
			Package:           pkg,
			ChartURI:          selected.PartURI,
			SeriesNumber:      xlsxChartsUpdateSeries,
			Role:              role,
			Formula:           formula,
			CacheMode:         cacheMode,
			CachePoints:       cachePoints,
			CacheSkipped:      cacheSkipped,
			FormatCode:        formatCode,
			ExpectFormula:     xlsxChartsUpdateExpectFormula,
			ExpectSourceRange: xlsxChartsUpdateExpectSourceRange,
		})
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to update chart source: %v", err)
		}
		warnings = append(warnings, mutation.Warnings...)
		if cacheMode == xlsxchart.CacheModeKeep {
			warnings = append(warnings, "stored chart cache was kept from the previous source and may not match the updated formula")
		}
		if cacheMode == xlsxchart.CacheModeClear {
			warnings = append(warnings, "stored chart cache was removed; spreadsheet applications may refresh it on open")
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var chartItem *XLSXChartItem
		if wantReadback {
			updatedCharts, err := xlsxchart.List(pkg, workbook, sheets)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read back charts: %v", err)
			}
			updated, err := selectXLSXChart(updatedCharts, "part:"+selected.PartURI)
			if err != nil {
				return err
			}
			item := xlsxChartItemForUpdate(destinationFile, updated)
			chartItem = &item
		}
		result = buildXLSXChartUpdateSourceResult(filePath, destinationFile, mutOpts, chartItem, mutation, sourceSheet, sourceRange, warnings)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func resolveXLSXChartSourceFormula(workbook *model.Workbook, currentSource *model.ChartDataSourceRef) (string, string, string, address.RangeRef, error) {
	if strings.TrimSpace(xlsxChartsUpdateFormula) != "" {
		sheetName, rangeText, ok := xlsxchart.ParseLocalRangeFormula(xlsxChartsUpdateFormula)
		if !ok {
			return "", "", "", address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "--formula must be a simple local worksheet A1 range such as Data!$B$2:$B$10")
		}
		rangeRef, err := address.ParseRange(rangeText)
		if err != nil {
			return "", "", "", address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "invalid --formula range: %v", err)
		}
		if _, err := selectXLSXSheet(workbook.Sheets, sheetName); err != nil {
			return "", "", "", address.RangeRef{}, err
		}
		return sheetName, rangeRef.String(), xlsxchart.LocalRangeFormula(sheetName, rangeRef), rangeRef, nil
	}

	rangeRef, err := address.ParseRange(xlsxChartsUpdateSourceRange)
	if err != nil {
		return "", "", "", address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "invalid --source-range: %v", err)
	}
	sheetName := strings.TrimSpace(xlsxChartsUpdateSourceSheet)
	if sheetName == "" && currentSource != nil {
		sheetName = currentSource.Sheet
	}
	if sheetName == "" {
		return "", "", "", address.RangeRef{}, InvalidArgsError("--source-sheet is required when the current chart source has no local worksheet sheet")
	}
	sourceSheetRef, err := selectXLSXSheet(workbook.Sheets, sheetName)
	if err != nil {
		return "", "", "", address.RangeRef{}, err
	}
	return sourceSheetRef.Name, rangeRef.String(), xlsxchart.LocalRangeFormula(sourceSheetRef.Name, rangeRef), rangeRef, nil
}

func chartSeriesSourceByRole(chart model.ChartRef, seriesNumber int, role string) (*model.ChartDataSourceRef, error) {
	if seriesNumber < 1 || seriesNumber > len(chart.Series) {
		return nil, NewCLIErrorf(ExitInvalidArgs, "series %d is out of range (1-%d)", seriesNumber, len(chart.Series))
	}
	series := chart.Series[seriesNumber-1]
	for _, source := range chartSeriesSources(series) {
		if source.Role == role {
			if source.Ref == nil {
				return nil, NewCLIErrorf(ExitInvalidArgs, "series %d has no %s source", seriesNumber, role)
			}
			if source.Ref.RefKind == "" {
				return nil, NewCLIErrorf(ExitInvalidArgs, "series %d %s source is not a cell reference", seriesNumber, role)
			}
			return source.Ref, nil
		}
	}
	return nil, NewCLIErrorf(ExitInvalidArgs, "unknown chart source role %q", role)
}

func collectXLSXChartCachePoints(pkg opc.PackageSession, workbook *model.Workbook, sheetRef model.SheetRef, rangeRef address.RangeRef, refKind string) ([]xlsxchart.CachePoint, int, string, []string, error) {
	if err := checkXLSXRangeMaxCells(rangeRef, xlsxChartsUpdateMaxCells); err != nil {
		return nil, 0, "", nil, err
	}
	ctx, err := xlsxsheet.LoadContext(pkg, workbook)
	if err != nil {
		return nil, 0, "", nil, NewCLIErrorf(ExitUnexpected, "failed to load workbook context: %v", err)
	}
	report, err := xlsxsheet.Read(pkg, sheetRef, ctx, xlsxsheet.ReadOptions{
		Range:        &rangeRef,
		MaxCells:     int(xlsxRangeCellCount(rangeRef)),
		IncludeEmpty: true,
		IncludeData:  true,
	})
	if err != nil {
		return nil, 0, "", nil, NewCLIErrorf(ExitUnexpected, "failed to read chart source range %q: %v", rangeRef.String(), err)
	}

	var points []xlsxchart.CachePoint
	skipped := 0
	formatCounts := map[string]int{}
	flatIndex := 0
	for _, row := range report.Rows {
		for _, cell := range row.Cells {
			value, ok := chartCacheValueFromCell(cell, refKind)
			if !ok {
				skipped++
				flatIndex++
				continue
			}
			points = append(points, xlsxchart.CachePoint{Index: flatIndex, Value: value})
			if refKind == "numRef" && cell.NumberFormatCode != "" {
				formatCounts[cell.NumberFormatCode]++
			}
			flatIndex++
		}
	}
	if refKind == "numRef" && len(points) == 0 {
		return nil, skipped, "", nil, NewCLIErrorf(ExitInvalidArgs, "source range %s has no numeric values for a numeric chart source", rangeRef.String())
	}
	warnings := []string{}
	if skipped > 0 {
		warnings = append(warnings, fmt.Sprintf("skipped %d source cell(s) that could not be represented in the %s chart cache", skipped, refKind))
	}
	return points, skipped, dominantFormatCode(formatCounts), warnings, nil
}

func chartCacheValueFromCell(cell model.Cell, refKind string) (string, bool) {
	switch refKind {
	case "numRef":
		if cell.Type != model.CellTypeNumber && cell.Type != model.CellTypeDate {
			return "", false
		}
		value := strings.TrimSpace(cell.RawValue)
		if value == "" {
			value = strings.TrimSpace(cell.Value)
		}
		if value == "" {
			return "", false
		}
		if _, err := strconv.ParseFloat(value, 64); err != nil {
			return "", false
		}
		return value, true
	default:
		if cell.Type == model.CellTypeError {
			return "", false
		}
		if cell.Value != "" {
			return cell.Value, true
		}
		return cell.RawValue, true
	}
}

func dominantFormatCode(counts map[string]int) string {
	best := ""
	bestCount := 0
	for format, count := range counts {
		if count > bestCount || count == bestCount && format < best {
			best = format
			bestCount = count
		}
	}
	return best
}

func normalizeXLSXChartCacheMode(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", xlsxchart.CacheModeAuto:
		return xlsxchart.CacheModeAuto, nil
	case xlsxchart.CacheModeClear:
		return xlsxchart.CacheModeClear, nil
	case xlsxchart.CacheModeKeep:
		return xlsxchart.CacheModeKeep, nil
	default:
		return "", InvalidArgsError("--cache must be auto, clear, or keep")
	}
}

func buildXLSXChartUpdateSourceResult(filePath, destinationFile string, mutOpts *MutationOptions, chartItem *XLSXChartItem, mutation *xlsxchart.SetSeriesSourceResult, sourceSheet, sourceRange string, warnings []string) *XLSXChartUpdateSourceResult {
	result := &XLSXChartUpdateSourceResult{
		File:                filePath,
		Output:              destinationFile,
		DryRun:              mutOpts != nil && mutOpts.DryRun,
		Action:              "xlsx.chart.update-source",
		Chart:               chartItem,
		Series:              xlsxChartsUpdateSeries,
		Role:                mutationRoleOrDefault(),
		PreviousFormula:     mutation.PreviousFormula,
		Formula:             mutation.Formula,
		Sheet:               sourceSheet,
		Range:               sourceRange,
		RefKind:             mutation.RefKind,
		CacheType:           mutation.CacheType,
		CachePointCount:     mutation.CachePointCount,
		CachePreview:        mutation.CachePreview,
		CacheSkipped:        mutation.CacheSkipped,
		CacheVerified:       false,
		Warnings:            uniqueSortedWarnings(warnings),
		StoredCacheContract: "stored chart cache values are written from worksheet cell values but not recalculated or verified by Excel",
	}
	if result.Output == "" {
		placeholder := xlsxOutputPlaceholder()
		result.ValidateCommandTemplate = xlsxValidateCommand(placeholder)
		result.ChartShowCommandTemplate = xlsxChartShowCommand(placeholder, xlsxChartsUpdateSheet, xlsxChartSelectorForTemplate(chartItem))
		result.RangesExportCommandTemplate = xlsxRangesExportCommand(placeholder, sourceSheet, sourceRange)
		result.SourceRangeExportCommandDryRun = xlsxRangesExportCommand(filePath, sourceSheet, sourceRange)
	} else {
		result.ValidateCommand = xlsxValidateCommand(result.Output)
		result.ChartShowCommand = xlsxChartShowCommand(result.Output, xlsxChartsUpdateSheet, xlsxChartSelectorForTemplate(chartItem))
		result.RangesExportCommand = xlsxRangesExportCommand(result.Output, sourceSheet, sourceRange)
		result.SourceRangeExportCommand = xlsxRangesExportCommand(filePath, sourceSheet, sourceRange)
	}
	return result
}

func mutationRoleOrDefault() string {
	role, err := xlsxchart.NormalizeSourceRole(xlsxChartsUpdateRole)
	if err != nil {
		return strings.TrimSpace(xlsxChartsUpdateRole)
	}
	return role
}

func xlsxChartItemForUpdate(filePath string, chart model.ChartRef) XLSXChartItem {
	if filePath == "" {
		return XLSXChartItem{ChartRef: model.WithChartSelectors(chart)}
	}
	return xlsxChartItem(filePath, chart)
}

func xlsxChartSelectorForTemplate(chartItem *XLSXChartItem) string {
	if chartItem != nil {
		return xlsxChartSelector(chartItem.ChartRef)
	}
	if strings.TrimSpace(xlsxChartsUpdateChart) != "" {
		return xlsxChartsUpdateChart
	}
	return "chart:1"
}

func uniqueSortedWarnings(warnings []string) []string {
	seen := map[string]struct{}{}
	var result []string
	for _, warning := range warnings {
		warning = strings.TrimSpace(warning)
		if warning == "" {
			continue
		}
		if _, ok := seen[warning]; ok {
			continue
		}
		seen[warning] = struct{}{}
		result = append(result, warning)
	}
	sort.Strings(result)
	return result
}

func outputXLSXChartUpdateSourceJSON(cmd *cobra.Command, result *XLSXChartUpdateSourceResult) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal chart update JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXChartUpdateSourceText(cmd *cobra.Command, result *XLSXChartUpdateSourceResult) error {
	action := "updated"
	if result.DryRun {
		action = "would update"
	}
	out := fmt.Sprintf("%s chart source: series %d %s -> %s!%s (%d cached point(s))", action, result.Series, result.Role, result.Sheet, result.Range, result.CachePointCount)
	if result.Output != "" {
		out += "\noutput: " + result.Output
	}
	if len(result.Warnings) > 0 {
		out += "\nwarnings: " + strings.Join(result.Warnings, "; ")
	}
	return writeXLSXOutput(cmd, []byte(out))
}

func init() {
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateSheet, "sheet", "", "sheet number (1-based) or exact sheet name for chart discovery")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateChart, "chart", "", "chart number, name, relationship id, drawing relationship id, or part selector")
	xlsxChartsUpdateSourceCmd.Flags().IntVar(&xlsxChartsUpdateSeries, "series", 1, "1-based chart series number")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateRole, "role", xlsxchart.RoleValues, "source role to update: name, categories, values, xValues, yValues, or bubbleSize")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateSourceSheet, "source-sheet", "", "worksheet for --source-range; defaults to the current chart source sheet when available")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateSourceRange, "source-range", "", "new local A1 source range")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateFormula, "formula", "", "new local chart source formula such as Data!$B$2:$B$10")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateCache, "cache", xlsxchart.CacheModeAuto, "stored chart cache mode: auto, clear, or keep")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateExpectSourceRange, "expect-source-range", "", "fail unless the current chart source range matches this A1 range")
	xlsxChartsUpdateSourceCmd.Flags().StringVar(&xlsxChartsUpdateExpectFormula, "expect-formula", "", "fail unless the current chart source formula matches this formula")
	xlsxChartsUpdateSourceCmd.Flags().IntVar(&xlsxChartsUpdateMaxCells, "max-cells", 100000, "maximum source cells to read for cache rebuild (0 for unlimited)")
	AddMutationFlags(xlsxChartsUpdateSourceCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsUpdateSourceCmd)
}
