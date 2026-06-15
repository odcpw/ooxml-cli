package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxpivot "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/pivot"
	"github.com/spf13/cobra"
)

type XLSXPivotsResult struct {
	File            string          `json:"file"`
	ValidateCommand string          `json:"validateCommand,omitempty"`
	Pivots          []XLSXPivotItem `json:"pivots"`
}

type XLSXPivotItem struct {
	model.PivotRef
	ShowCommand         string `json:"showCommand,omitempty"`
	SourceExportCommand string `json:"sourceExportCommand,omitempty"`
}

var (
	xlsxPivotsListSheet string
	xlsxPivotsShowSheet string
	xlsxPivotsShowPivot string
)

var xlsxPivotsListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List workbook PivotTables",
	Long:  "List existing XLSX PivotTables discovered from worksheet pivotTableDefinition relationships.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pivots, err := loadXLSXPivotsForCLI(filePath, xlsxPivotsListSheet)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXPivotsJSON(cmd, filePath, pivots)
		}
		return outputXLSXPivotsText(cmd, pivots)
	},
}

var xlsxPivotsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show PivotTable metadata",
	Long:  "Show one XLSX PivotTable, including worksheet location, cache source, fields, and part metadata.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		pivots, err := loadXLSXPivotsForCLI(filePath, xlsxPivotsShowSheet)
		if err != nil {
			return err
		}
		selected, err := selectXLSXPivot(pivots, xlsxPivotsShowPivot)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXPivotsJSON(cmd, filePath, []model.PivotRef{selected})
		}
		return outputXLSXPivotsText(cmd, []model.PivotRef{selected})
	},
}

func loadXLSXPivotsForCLI(filePath, sheetSelector string) ([]model.PivotRef, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheets := workbook.Sheets
	if sheetSelector != "" {
		selected, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return nil, err
		}
		if err := requireXLSXWorksheetRef(selected); err != nil {
			return nil, err
		}
		sheets = []model.SheetRef{selected}
	}
	pivots, err := xlsxpivot.List(pkg, workbook, sheets)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to list pivots: %v", err)
	}
	return pivots, nil
}

func selectXLSXPivot(pivots []model.PivotRef, selector string) (model.PivotRef, error) {
	if len(pivots) == 0 {
		return model.PivotRef{}, NewCLIErrorf(ExitInvalidArgs, "workbook has no pivots")
	}
	selector = strings.TrimSpace(selector)
	if selector == "" {
		if len(pivots) == 1 {
			return pivots[0], nil
		}
		return model.PivotRef{}, InvalidArgsError("--pivot is required when workbook has multiple pivots")
	}
	var matches []model.PivotRef
	for _, pivotRef := range pivots {
		withSelectors := model.WithPivotSelectors(pivotRef)
		if model.SelectorMatches(withSelectors.Selectors, selector) {
			matches = append(matches, withSelectors)
		}
	}
	if len(matches) == 1 {
		return matches[0], nil
	}
	if len(matches) > 1 {
		candidates := make([]string, 0, len(matches))
		for _, match := range matches {
			candidates = append(candidates, match.PrimarySelector)
		}
		return model.PivotRef{}, NewCLIErrorf(ExitInvalidArgs, "pivot selector %q matched multiple pivots (%s); use a more specific selector", selector, strings.Join(candidates, ", "))
	}
	if number, err := strconv.Atoi(selector); err == nil {
		if number < 1 || number > len(pivots) {
			return model.PivotRef{}, NewCLIErrorf(ExitTargetNotFound, "pivot %d is out of range (1-%d)", number, len(pivots))
		}
		return model.WithPivotSelectors(pivots[number-1]), nil
	}
	candidates := pivotSelectorCandidates(pivots)
	return model.PivotRef{}, SelectorNotFoundError("pivot", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), "ooxml --json xlsx pivots list <file>")
}

func pivotSelectorCandidates(pivots []model.PivotRef) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(pivots))
	for _, pivotRef := range pivots {
		withSelectors := model.WithPivotSelectors(pivotRef)
		out = append(out, SelectorCandidate{Primary: withSelectors.PrimarySelector, Selectors: withSelectors.Selectors})
	}
	return out
}

func outputXLSXPivotsJSON(cmd *cobra.Command, filePath string, pivots []model.PivotRef) error {
	config := GetGlobalConfig(cmd)
	items := make([]XLSXPivotItem, 0, len(pivots))
	for _, pivot := range pivots {
		items = append(items, xlsxPivotItem(filePath, pivot))
	}
	result := XLSXPivotsResult{File: filePath, ValidateCommand: xlsxValidateCommand(filePath), Pivots: items}
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal pivots JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXPivotsText(cmd *cobra.Command, pivots []model.PivotRef) error {
	if len(pivots) == 0 {
		return writeXLSXOutput(cmd, []byte("no pivots found"))
	}

	out := ""
	for i, pivotRef := range pivots {
		if i > 0 {
			out += "\n"
		}
		out += fmt.Sprintf("[%d] %s\n", pivotRef.Number, pivotDisplayName(pivotRef))
		out += fmt.Sprintf("  sheet: %s (%d)\n", pivotRef.Sheet, pivotRef.SheetNumber)
		if pivotRef.Location != "" {
			out += fmt.Sprintf("  location: %s", pivotRef.Location)
			if pivotRef.Rows > 0 && pivotRef.Cols > 0 {
				out += fmt.Sprintf(" (%d rows x %d cols)", pivotRef.Rows, pivotRef.Cols)
			}
			out += "\n"
		}
		if pivotRef.Cache != nil {
			out += fmt.Sprintf("  cache: %d %s\n", pivotRef.Cache.CacheID, pivotRef.Cache.PartURI)
			if pivotRef.Cache.Source.Type != "" || pivotRef.Cache.Source.Sheet != "" || pivotRef.Cache.Source.Range != "" || pivotRef.Cache.Source.Name != "" {
				out += fmt.Sprintf("  source: %s %s!%s %s\n", pivotRef.Cache.Source.Type, pivotRef.Cache.Source.Sheet, pivotRef.Cache.Source.Range, pivotRef.Cache.Source.Name)
			}
		}
		out += fmt.Sprintf("  rows: %s\n", fieldNamesText(pivotRef.RowFields))
		out += fmt.Sprintf("  columns: %s\n", fieldNamesText(pivotRef.ColumnFields))
		out += fmt.Sprintf("  values: %s\n", fieldNamesText(pivotRef.DataFields))
		out += fmt.Sprintf("  filters: %s\n", fieldNamesText(pivotRef.FilterFields))
		out += fmt.Sprintf("  part: %s\n", pivotRef.PartURI)
	}
	return writeXLSXOutput(cmd, []byte(out))
}

func xlsxPivotItem(filePath string, pivot model.PivotRef) XLSXPivotItem {
	pivot = model.WithPivotSelectors(pivot)
	pivotSelector := xlsxPivotSelector(pivot)
	sheetSelector := xlsxPivotSheetSelector(pivot)
	item := XLSXPivotItem{
		PivotRef:    pivot,
		ShowCommand: xlsxPivotShowCommand(filePath, sheetSelector, pivotSelector),
	}
	if pivot.Cache != nil && pivot.Cache.Source.Sheet != "" && pivot.Cache.Source.Range != "" {
		item.SourceExportCommand = xlsxRangesExportCommand(filePath, pivot.Cache.Source.Sheet, pivot.Cache.Source.Range)
	}
	return item
}

func xlsxPivotSelector(pivot model.PivotRef) string {
	if pivot.PrimarySelector != "" {
		return pivot.PrimarySelector
	}
	if pivot.Name != "" {
		return pivot.Name
	}
	if pivot.Number > 0 {
		return fmt.Sprintf("pivot:%d", pivot.Number)
	}
	return "1"
}

func xlsxPivotSheetSelector(pivot model.PivotRef) string {
	if pivot.Sheet != "" {
		return pivot.Sheet
	}
	if pivot.SheetNumber > 0 {
		return fmt.Sprintf("sheet:%d", pivot.SheetNumber)
	}
	return ""
}

func xlsxPivotShowCommand(filePath, sheetSelector, pivotSelector string) string {
	args := []string{"ooxml", "--json", "xlsx", "pivots", "show", pptxXLSXCommandArg(filePath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--pivot", pivotSelector)
	return strings.Join(args, " ")
}

func pivotDisplayName(pivot model.PivotRef) string {
	if strings.TrimSpace(pivot.Name) != "" {
		return pivot.Name
	}
	if pivot.Number > 0 {
		return fmt.Sprintf("pivot:%d", pivot.Number)
	}
	return "(unnamed)"
}

func fieldNamesText(fields []model.PivotFieldRef) string {
	if len(fields) == 0 {
		return "-"
	}
	names := make([]string, 0, len(fields))
	for _, field := range fields {
		name := field.Name
		if name == "" {
			name = fmt.Sprintf("#%d", field.Index)
		}
		if field.Caption != "" && field.Caption != name {
			name = field.Caption + " (" + name + ")"
		}
		names = append(names, name)
	}
	return strings.Join(names, ", ")
}

func init() {
	xlsxPivotsListCmd.Flags().StringVar(&xlsxPivotsListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxPivotsShowCmd.Flags().StringVar(&xlsxPivotsShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxPivotsShowCmd.Flags().StringVar(&xlsxPivotsShowPivot, "pivot", "", "pivot number, name, cache id, relationship id, or part selector")
	xlsxPivotsCmd.AddCommand(xlsxPivotsListCmd)
	xlsxPivotsCmd.AddCommand(xlsxPivotsShowCmd)
}
