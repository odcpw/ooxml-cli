package cli

import (
	"crypto/sha256"
	"encoding/csv"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxchart "github.com/ooxml-cli/ooxml-cli/pkg/pptx/chart"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxchart "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/chart"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type PPTXChartUpdateDataResult struct {
	File                       string                 `json:"file"`
	Output                     string                 `json:"output,omitempty"`
	DryRun                     bool                   `json:"dryRun"`
	Action                     string                 `json:"action"`
	Chart                      *PPTXChartResultItem   `json:"chart,omitempty"`
	Series                     int                    `json:"series"`
	UpdatedRoles               []PPTXChartUpdatedRole `json:"updatedRoles"`
	EmbeddedWorkbookPartURI    string                 `json:"embeddedWorkbookPartUri,omitempty"`
	EmbeddedWorkbookUpdated    bool                   `json:"embeddedWorkbookUpdated"`
	CacheVerified              bool                   `json:"cacheVerified"`
	Warnings                   []string               `json:"warnings,omitempty"`
	StoredCacheContract        string                 `json:"storedCacheContract"`
	ValidateCommand            string                 `json:"validateCommand,omitempty"`
	ChartShowCommand           string                 `json:"chartShowCommand,omitempty"`
	RenderCommand              string                 `json:"renderCommand,omitempty"`
	ValidateCommandTemplate    string                 `json:"validateCommandTemplate,omitempty"`
	ChartShowCommandTemplate   string                 `json:"chartShowCommandTemplate,omitempty"`
	RenderCommandTemplate      string                 `json:"renderCommandTemplate,omitempty"`
	CurrentValuesHash          string                 `json:"currentValuesHash,omitempty"`
	ExpectedValuesHashAccepted string                 `json:"expectedValuesHashAccepted,omitempty"`
}

type PPTXChartUpdatedRole struct {
	Role                         string   `json:"role"`
	Formula                      string   `json:"formula"`
	Sheet                        string   `json:"sheet,omitempty"`
	Range                        string   `json:"range,omitempty"`
	RefKind                      string   `json:"refKind"`
	PreviousCacheType            string   `json:"previousCacheType,omitempty"`
	PreviousCachePointCount      int      `json:"previousCachePointCount"`
	PreviousCachePreview         []string `json:"previousCachePreview,omitempty"`
	PreviousValuesHash           string   `json:"previousValuesHash,omitempty"`
	CacheType                    string   `json:"cacheType,omitempty"`
	CachePointCount              int      `json:"cachePointCount"`
	CachePreview                 []string `json:"cachePreview,omitempty"`
	EmbeddedWorkbookRangeUpdated bool     `json:"embeddedWorkbookRangeUpdated"`
}

var (
	pptxChartsUpdateSlide            int
	pptxChartsUpdateChart            string
	pptxChartsUpdateSeries           int
	pptxChartsUpdateValues           string
	pptxChartsUpdateValuesJSON       string
	pptxChartsUpdateCategories       string
	pptxChartsUpdateCategoriesJSON   string
	pptxChartsUpdateExpectPointCount int
	pptxChartsUpdateExpectValuesHash string
)

var pptxChartsUpdateDataCmd = &cobra.Command{
	Use:   "update-data <file>",
	Short: "Update data in an existing PPTX chart",
	Long: `Update values and/or categories for an existing slide chart series.

The command updates the chart's stored cache and, when the chart has an
embedded workbook, updates the matching source cells inside that embedded
workbook. It edits existing chart sources; it does not author new charts.`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if pptxChartsUpdateSeries < 1 {
			return InvalidArgsError("--series must be >= 1")
		}
		valuesChanged := cmd.Flags().Lookup("values").Changed
		valuesJSONChanged := cmd.Flags().Lookup("values-json").Changed
		categoriesChanged := cmd.Flags().Lookup("categories").Changed
		categoriesJSONChanged := cmd.Flags().Lookup("categories-json").Changed
		if !valuesChanged {
			pptxChartsUpdateValues = ""
		}
		if !valuesJSONChanged {
			pptxChartsUpdateValuesJSON = ""
		}
		if !categoriesChanged {
			pptxChartsUpdateCategories = ""
		}
		if !categoriesJSONChanged {
			pptxChartsUpdateCategoriesJSON = ""
		}
		if valuesChanged && valuesJSONChanged {
			return InvalidArgsError("specify only one of --values or --values-json")
		}
		if categoriesChanged && categoriesJSONChanged {
			return InvalidArgsError("specify only one of --categories or --categories-json")
		}
		if !valuesChanged && !valuesJSONChanged && !categoriesChanged && !categoriesJSONChanged {
			return InvalidArgsError("must specify --values, --values-json, --categories, or --categories-json")
		}
		if pptxChartsUpdateExpectPointCount < 0 {
			return InvalidArgsError("--expect-point-count must be >= 0")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performPPTXChartsUpdateData(filePath, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXChartUpdateDataJSON(cmd, result)
		}
		return outputPPTXChartUpdateDataText(cmd, result)
	},
}

type pptxChartInputRole struct {
	role   string
	values []string
}

func performPPTXChartsUpdateData(filePath string, mutOpts *MutationOptions, wantReadback bool) (*PPTXChartUpdateDataResult, error) {
	inputRoles, err := resolvePPTXChartInputRoles()
	if err != nil {
		return nil, err
	}
	if len(inputRoles) == 0 {
		return nil, InvalidArgsError("at least one non-empty values or categories input is required")
	}
	if len(inputRoles) == 2 && len(inputRoles[0].values) != len(inputRoles[1].values) {
		return nil, NewCLIErrorf(ExitInvalidArgs, "values and categories must have the same point count when both are supplied (%d vs %d)", len(inputRoles[0].values), len(inputRoles[1].values))
	}

	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}

	var result *PPTXChartUpdateDataResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		charts, err := pptxchart.List(pkg, pptxChartsUpdateSlide)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list PPTX charts: %v", err)
		}
		selected, err := selectPPTXChart(charts, pptxChartsUpdateChart)
		if err != nil {
			return err
		}
		if pptxChartsUpdateSeries > len(selected.Series) {
			return NewCLIErrorf(ExitInvalidArgs, "series %d is out of range (1-%d)", pptxChartsUpdateSeries, len(selected.Series))
		}

		var warnings []string
		valuesSnapshot, err := xlsxchart.ReadSeriesSource(pkg, selected.PartURI, pptxChartsUpdateSeries, xlsxchart.RoleValues)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to inspect current values source: %v", err)
		}
		currentValuesHash := chartValuesHash(valuesSnapshot.Values)
		if strings.TrimSpace(pptxChartsUpdateExpectValuesHash) != "" {
			if !chartHashMatches(currentValuesHash, pptxChartsUpdateExpectValuesHash) {
				return NewCLIErrorf(ExitInvalidArgs, "--expect-values-hash mismatch: current %s", currentValuesHash)
			}
		}

		var embeddedPkg *opc.Package
		var embeddedWorkbook *model.Workbook
		embeddedUpdated := false
		if selected.EmbeddedWorkbookPartURI != "" {
			embeddedPkg, embeddedWorkbook, err = openEmbeddedWorkbook(pkg, selected.EmbeddedWorkbookPartURI)
			if err != nil {
				return err
			}
			defer embeddedPkg.Close()
		} else {
			warnings = append(warnings, "chart has no embedded workbook; updated chart cache only")
		}

		updatedRoles := make([]PPTXChartUpdatedRole, 0, len(inputRoles))
		for _, inputRole := range inputRoles {
			snapshot, err := xlsxchart.ReadSeriesSource(pkg, selected.PartURI, pptxChartsUpdateSeries, inputRole.role)
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to inspect current %s source: %v", inputRole.role, err)
			}
			if strings.TrimSpace(snapshot.Formula) == "" {
				return NewCLIErrorf(ExitInvalidArgs, "series %d %s source has no editable local source formula", pptxChartsUpdateSeries, inputRole.role)
			}
			if _, _, ok := xlsxchart.ParseLocalRangeFormula(snapshot.Formula); !ok {
				return NewCLIErrorf(ExitInvalidArgs, "series %d %s source formula %q is not a supported local A1 range", pptxChartsUpdateSeries, inputRole.role, snapshot.Formula)
			}
			if pptxChartsUpdateExpectPointCount > 0 && snapshot.PointCount != pptxChartsUpdateExpectPointCount {
				return NewCLIErrorf(ExitInvalidArgs, "--expect-point-count mismatch for %s: current %d", inputRole.role, snapshot.PointCount)
			}
			cachePoints, err := chartCachePointsForValues(inputRole.values, snapshot.RefKind)
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "%s input is invalid for chart source: %v", inputRole.role, err)
			}
			mutation, err := xlsxchart.SetSeriesSource(&xlsxchart.SetSeriesSourceRequest{
				Package:       pkg,
				ChartURI:      selected.PartURI,
				SeriesNumber:  pptxChartsUpdateSeries,
				Role:          inputRole.role,
				Formula:       snapshot.Formula,
				CacheMode:     xlsxchart.CacheModeAuto,
				CachePoints:   cachePoints,
				ExpectFormula: snapshot.Formula,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to update chart %s cache: %v", inputRole.role, err)
			}
			warnings = append(warnings, mutation.Warnings...)

			embeddedRoleUpdated := false
			if embeddedPkg != nil && embeddedWorkbook != nil {
				embeddedRoleUpdated, err = updateEmbeddedWorkbookChartRange(embeddedPkg, embeddedWorkbook, snapshot, inputRole.values)
				if err != nil {
					return err
				}
				if embeddedRoleUpdated {
					embeddedUpdated = true
				} else {
					warnings = append(warnings, fmt.Sprintf("could not update embedded workbook range for %s; updated chart cache only", inputRole.role))
				}
			}

			updatedRoles = append(updatedRoles, PPTXChartUpdatedRole{
				Role:                         inputRole.role,
				Formula:                      snapshot.Formula,
				Sheet:                        snapshot.Sheet,
				Range:                        snapshot.Range,
				RefKind:                      snapshot.RefKind,
				PreviousCacheType:            snapshot.CacheType,
				PreviousCachePointCount:      snapshot.PointCount,
				PreviousCachePreview:         previewStrings(snapshot.Values, 5),
				PreviousValuesHash:           chartValuesHash(snapshot.Values),
				CacheType:                    mutation.CacheType,
				CachePointCount:              mutation.CachePointCount,
				CachePreview:                 mutation.CachePreview,
				EmbeddedWorkbookRangeUpdated: embeddedRoleUpdated,
			})
		}

		if embeddedUpdated {
			embeddedBytes, err := embeddedPkg.WriteToBytes()
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to serialize embedded workbook: %v", err)
			}
			contentType := pkg.GetContentType(selected.EmbeddedWorkbookPartURI)
			if strings.TrimSpace(contentType) == "" {
				contentType = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
			}
			if err := pkg.ReplaceRawPart(selected.EmbeddedWorkbookPartURI, embeddedBytes, contentType); err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to replace embedded workbook %s: %v", selected.EmbeddedWorkbookPartURI, err)
			}
		}

		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var chartItem *PPTXChartResultItem
		if wantReadback {
			updatedCharts, err := pptxchart.List(pkg, pptxChartsUpdateSlide)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read back PPTX charts: %v", err)
			}
			updated, err := selectPPTXChart(updatedCharts, "part:"+selected.PartURI)
			if err != nil {
				return err
			}
			item := pptxChartItemForUpdate(destinationFile, updated)
			chartItem = &item
		}

		result = buildPPTXChartUpdateDataResult(filePath, destinationFile, mutOpts, chartItem, selected, updatedRoles, embeddedUpdated, currentValuesHash, warnings)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func resolvePPTXChartInputRoles() ([]pptxChartInputRole, error) {
	var roles []pptxChartInputRole
	values, changed, err := resolvePPTXChartInputValues(pptxChartsUpdateValues, pptxChartsUpdateValuesJSON)
	if err != nil {
		return nil, err
	}
	if changed {
		roles = append(roles, pptxChartInputRole{role: xlsxchart.RoleValues, values: values})
	}
	categories, changed, err := resolvePPTXChartInputValues(pptxChartsUpdateCategories, pptxChartsUpdateCategoriesJSON)
	if err != nil {
		return nil, err
	}
	if changed {
		roles = append(roles, pptxChartInputRole{role: xlsxchart.RoleCategories, values: categories})
	}
	return roles, nil
}

func resolvePPTXChartInputValues(csvValues, jsonValues string) ([]string, bool, error) {
	if strings.TrimSpace(jsonValues) != "" {
		var values []string
		if err := json.Unmarshal([]byte(jsonValues), &values); err != nil {
			return nil, false, NewCLIErrorf(ExitInvalidArgs, "invalid JSON values array: %v", err)
		}
		return normalizePPTXChartValues(values), true, nil
	}
	if strings.TrimSpace(csvValues) == "" {
		return nil, false, nil
	}
	reader := csv.NewReader(strings.NewReader(csvValues))
	reader.FieldsPerRecord = -1
	values, err := reader.Read()
	if err != nil {
		return nil, false, NewCLIErrorf(ExitInvalidArgs, "invalid comma-separated values: %v", err)
	}
	return normalizePPTXChartValues(values), true, nil
}

func normalizePPTXChartValues(values []string) []string {
	out := make([]string, 0, len(values))
	for _, value := range values {
		out = append(out, strings.TrimSpace(value))
	}
	return out
}

func chartCachePointsForValues(values []string, refKind string) ([]xlsxchart.CachePoint, error) {
	if len(values) == 0 {
		return nil, fmt.Errorf("at least one point is required")
	}
	points := make([]xlsxchart.CachePoint, 0, len(values))
	for idx, value := range values {
		if refKind == "numRef" {
			if strings.TrimSpace(value) == "" {
				return nil, fmt.Errorf("point %d is empty but numeric chart sources require numbers", idx+1)
			}
			if _, err := strconv.ParseFloat(value, 64); err != nil {
				return nil, fmt.Errorf("point %d value %q is not numeric", idx+1, value)
			}
		}
		points = append(points, xlsxchart.CachePoint{Index: idx, Value: value})
	}
	return points, nil
}

func openEmbeddedWorkbook(pkg opc.PackageSession, embeddedURI string) (*opc.Package, *model.Workbook, error) {
	raw, err := pkg.ReadRawPart(embeddedURI)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to read embedded workbook %s: %v", embeddedURI, err)
	}
	embeddedPkg, err := opc.OpenBytes(raw)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to open embedded workbook %s: %v", embeddedURI, err)
	}
	workbook, err := xlsxinspect.ParseWorkbook(embeddedPkg)
	if err != nil {
		embeddedPkg.Close()
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to parse embedded workbook %s: %v", embeddedURI, err)
	}
	return embeddedPkg, workbook, nil
}

func updateEmbeddedWorkbookChartRange(pkg opc.PackageSession, workbook *model.Workbook, snapshot *xlsxchart.SeriesSourceSnapshot, values []string) (bool, error) {
	if snapshot == nil || snapshot.Sheet == "" || snapshot.Range == "" {
		return false, nil
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, snapshot.Sheet)
	if err != nil {
		return false, NewCLIErrorf(ExitInvalidArgs, "embedded workbook sheet %q not found for chart source %s: %v", snapshot.Sheet, snapshot.Role, err)
	}
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		return false, err
	}
	rangeRef, err := address.ParseRange(snapshot.Range)
	if err != nil {
		return false, NewCLIErrorf(ExitInvalidArgs, "invalid embedded workbook source range %q: %v", snapshot.Range, err)
	}
	rows, cols := xlsxRangeDimensions(rangeRef)
	if rows != 1 && cols != 1 {
		return false, NewCLIErrorf(ExitInvalidArgs, "embedded workbook source range %s is %dx%d; update-data currently requires a one-row or one-column series range", rangeRef.String(), rows, cols)
	}
	if rows*cols != len(values) {
		return false, NewCLIErrorf(ExitInvalidArgs, "embedded workbook source range %s has %d cell(s) but %s input has %d point(s)", rangeRef.String(), rows*cols, snapshot.Role, len(values))
	}
	mutRows, err := chartValuesToRangeCells(values, rows, cols, snapshot.RefKind)
	if err != nil {
		return false, err
	}
	if _, err := xlsxmutate.SetRange(&xlsxmutate.SetRangeRequest{
		Package:           pkg,
		WorkbookURI:       workbook.PartURI,
		SheetRef:          sheetRef,
		Range:             rangeRef,
		Rows:              mutRows,
		NullPolicy:        xlsxmutate.RangeNullEmptyString,
		OverwriteFormulas: true,
	}); err != nil {
		return false, NewCLIErrorf(ExitInvalidArgs, "failed to update embedded workbook range %s!%s: %v", snapshot.Sheet, snapshot.Range, err)
	}
	return true, nil
}

func chartValuesToRangeCells(values []string, rows, cols int, refKind string) ([][]xlsxmutate.RangeCell, error) {
	valueType := xlsxmutate.CellValueString
	if refKind == "numRef" {
		valueType = xlsxmutate.CellValueNumber
	}
	out := make([][]xlsxmutate.RangeCell, rows)
	idx := 0
	for row := 0; row < rows; row++ {
		out[row] = make([]xlsxmutate.RangeCell, cols)
		for col := 0; col < cols; col++ {
			value := values[idx]
			if valueType == xlsxmutate.CellValueNumber {
				if _, err := strconv.ParseFloat(value, 64); err != nil {
					return nil, NewCLIErrorf(ExitInvalidArgs, "point %d value %q is not numeric", idx+1, value)
				}
			}
			out[row][col] = xlsxmutate.RangeCell{Type: valueType, Value: value}
			idx++
		}
	}
	return out, nil
}

func buildPPTXChartUpdateDataResult(filePath, destinationFile string, mutOpts *MutationOptions, chartItem *PPTXChartResultItem, selected pptxchart.ChartRef, updatedRoles []PPTXChartUpdatedRole, embeddedUpdated bool, currentValuesHash string, warnings []string) *PPTXChartUpdateDataResult {
	result := &PPTXChartUpdateDataResult{
		File:                       filePath,
		Output:                     destinationFile,
		DryRun:                     mutOpts != nil && mutOpts.DryRun,
		Action:                     "pptx.chart.update-data",
		Chart:                      chartItem,
		Series:                     pptxChartsUpdateSeries,
		UpdatedRoles:               updatedRoles,
		EmbeddedWorkbookPartURI:    selected.EmbeddedWorkbookPartURI,
		EmbeddedWorkbookUpdated:    embeddedUpdated,
		CacheVerified:              false,
		Warnings:                   uniqueSortedWarnings(warnings),
		StoredCacheContract:        "stored chart cache values and embedded workbook cells are updated, but chart rendering is not recalculated by PowerPoint until validation/render/open",
		CurrentValuesHash:          currentValuesHash,
		ExpectedValuesHashAccepted: strings.TrimSpace(pptxChartsUpdateExpectValuesHash),
	}
	if result.Output == "" {
		placeholder := outputPlaceholder()
		result.ValidateCommandTemplate = pptxValidateCommand(placeholder)
		result.ChartShowCommandTemplate = pptxChartShowCommand(placeholder, pptxChartsUpdateSlide, pptxChartSelectorForUpdate(chartItem, selected))
		result.RenderCommandTemplate = pptxRenderCommand(placeholder)
	} else {
		result.ValidateCommand = pptxValidateCommand(result.Output)
		result.ChartShowCommand = pptxChartShowCommand(result.Output, pptxChartsUpdateSlide, pptxChartSelectorForUpdate(chartItem, selected))
		result.RenderCommand = pptxRenderCommand(result.Output)
	}
	return result
}

func pptxChartItemForUpdate(filePath string, chart pptxchart.ChartRef) PPTXChartResultItem {
	if filePath == "" {
		return PPTXChartResultItem{ChartRef: pptxchart.WithSelectors(chart)}
	}
	return pptxChartItem(filePath, chart)
}

func pptxChartSelectorForUpdate(chartItem *PPTXChartResultItem, fallback pptxchart.ChartRef) string {
	if chartItem != nil {
		return pptxChartGeneratedSelector(chartItem.ChartRef)
	}
	if strings.TrimSpace(pptxChartsUpdateChart) != "" {
		return pptxChartsUpdateChart
	}
	return pptxChartGeneratedSelector(fallback)
}

func chartValuesHash(values []string) string {
	data, _ := json.Marshal(values)
	sum := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(sum[:])
}

func chartHashMatches(current, expected string) bool {
	expected = strings.TrimSpace(expected)
	if expected == "" {
		return true
	}
	if current == expected {
		return true
	}
	if !strings.HasPrefix(expected, "sha256:") && strings.TrimPrefix(current, "sha256:") == expected {
		return true
	}
	return false
}

func previewStrings(values []string, max int) []string {
	if len(values) == 0 || max <= 0 {
		return nil
	}
	if len(values) < max {
		max = len(values)
	}
	return append([]string(nil), values[:max]...)
}

func outputPPTXChartUpdateDataJSON(cmd *cobra.Command, result *PPTXChartUpdateDataResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal PPTX chart update JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputPPTXChartUpdateDataText(cmd *cobra.Command, result *PPTXChartUpdateDataResult) error {
	action := "updated"
	if result.DryRun {
		action = "would update"
	}
	roles := make([]string, 0, len(result.UpdatedRoles))
	for _, role := range result.UpdatedRoles {
		roles = append(roles, fmt.Sprintf("%s(%d)", role.Role, role.CachePointCount))
	}
	text := fmt.Sprintf("%s chart data: series %d %s", action, result.Series, strings.Join(roles, ", "))
	if result.Output != "" {
		text += "\noutput: " + result.Output
	}
	if len(result.Warnings) > 0 {
		text += "\nwarnings: " + strings.Join(result.Warnings, "; ")
	}
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	pptxChartsUpdateDataCmd.Flags().IntVar(&pptxChartsUpdateSlide, "slide", 0, "1-based slide number to search")
	pptxChartsUpdateDataCmd.Flags().StringVar(&pptxChartsUpdateChart, "chart", "", "chart selector from pptx charts list")
	pptxChartsUpdateDataCmd.Flags().IntVar(&pptxChartsUpdateSeries, "series", 1, "1-based chart series number")
	pptxChartsUpdateDataCmd.Flags().StringVar(&pptxChartsUpdateValues, "values", "", "comma-separated values for the series values source")
	pptxChartsUpdateDataCmd.Flags().StringVar(&pptxChartsUpdateValuesJSON, "values-json", "", "JSON string array for the series values source")
	pptxChartsUpdateDataCmd.Flags().StringVar(&pptxChartsUpdateCategories, "categories", "", "comma-separated labels for the series category source")
	pptxChartsUpdateDataCmd.Flags().StringVar(&pptxChartsUpdateCategoriesJSON, "categories-json", "", "JSON string array for the series category source")
	pptxChartsUpdateDataCmd.Flags().IntVar(&pptxChartsUpdateExpectPointCount, "expect-point-count", 0, "fail unless each updated current chart source has this stored cache point count")
	pptxChartsUpdateDataCmd.Flags().StringVar(&pptxChartsUpdateExpectValuesHash, "expect-values-hash", "", "fail unless current values cache has this sha256 hash")
	AddMutationFlags(pptxChartsUpdateDataCmd)
	chartsCmd.AddCommand(pptxChartsUpdateDataCmd)
}
