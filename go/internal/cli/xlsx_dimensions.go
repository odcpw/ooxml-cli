package cli

import (
	"encoding/json"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxColWidthsCmd = &cobra.Command{
	Use:     "colwidths",
	Aliases: []string{"col-widths", "column-widths"},
	Short:   "Inspect and set worksheet column widths",
	Long:    "Commands for reading and setting worksheet column widths (in character units, excluding freeze panes).",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var xlsxRowHeightsCmd = &cobra.Command{
	Use:     "rowheights",
	Aliases: []string{"row-heights"},
	Short:   "Inspect and set worksheet row heights",
	Long:    "Commands for reading and setting worksheet row heights (in points, excluding freeze panes).",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

// ---- shared types ----

type xlsxDimensionEntry struct {
	Width    *float64 `json:"width,omitempty"`
	Height   *float64 `json:"height,omitempty"`
	Explicit bool     `json:"explicit"`
	Custom   bool     `json:"custom"`
	Hidden   bool     `json:"hidden"`
}

type XLSXColWidthsShowResult struct {
	File         string                        `json:"file"`
	Sheet        string                        `json:"sheet"`
	SheetNumber  int                           `json:"sheetNumber"`
	Range        string                        `json:"range"`
	MinColumn    string                        `json:"minColumn"`
	MaxColumn    string                        `json:"maxColumn"`
	Count        int                           `json:"count"`
	DefaultWidth float64                       `json:"defaultWidth"`
	Uniform      bool                          `json:"uniform"`
	Columns      map[string]xlsxDimensionEntry `json:"columns"`
	ColWidthsSet string                        `json:"colwidthsSetCommandTemplate,omitempty"`
}

type XLSXRowHeightsShowResult struct {
	File          string                        `json:"file"`
	Sheet         string                        `json:"sheet"`
	SheetNumber   int                           `json:"sheetNumber"`
	Range         string                        `json:"range"`
	MinRow        int                           `json:"minRow"`
	MaxRow        int                           `json:"maxRow"`
	Count         int                           `json:"count"`
	DefaultHeight float64                       `json:"defaultHeight"`
	Uniform       bool                          `json:"uniform"`
	Rows          map[string]xlsxDimensionEntry `json:"rows"`
	RowHeightsSet string                        `json:"rowheightsSetCommandTemplate,omitempty"`
}

type XLSXColWidthsSetResult struct {
	File                 string  `json:"file"`
	Sheet                string  `json:"sheet"`
	SheetNumber          int     `json:"sheetNumber"`
	Range                string  `json:"range"`
	MinColumn            string  `json:"minColumn"`
	MaxColumn            string  `json:"maxColumn"`
	Columns              int     `json:"columns"`
	Width                float64 `json:"width"`
	Output               string  `json:"output,omitempty"`
	DryRun               bool    `json:"dryRun"`
	ValidateCommand      string  `json:"validateCommand,omitempty"`
	ColWidthsShowCommand string  `json:"colwidthsShowCommand,omitempty"`
}

type XLSXRowHeightsSetResult struct {
	File                  string  `json:"file"`
	Sheet                 string  `json:"sheet"`
	SheetNumber           int     `json:"sheetNumber"`
	Range                 string  `json:"range"`
	MinRow                int     `json:"minRow"`
	MaxRow                int     `json:"maxRow"`
	Rows                  int     `json:"rows"`
	Created               int     `json:"created"`
	Height                float64 `json:"height"`
	Output                string  `json:"output,omitempty"`
	DryRun                bool    `json:"dryRun"`
	ValidateCommand       string  `json:"validateCommand,omitempty"`
	RowHeightsShowCommand string  `json:"rowheightsShowCommand,omitempty"`
}

// ---- flags ----

var (
	xlsxColWidthsShowSheet string
	xlsxColWidthsShowRange string

	xlsxColWidthsSetSheet  string
	xlsxColWidthsSetRange  string
	xlsxColWidthsSetWidth  float64
	xlsxColWidthsSetExpect float64

	xlsxRowHeightsShowSheet string
	xlsxRowHeightsShowRange string

	xlsxRowHeightsSetSheet  string
	xlsxRowHeightsSetRange  string
	xlsxRowHeightsSetHeight float64
	xlsxRowHeightsSetExpect float64
)

const xlsxDimensionTolerance = 1e-6

// ---- range parsing ----

func parseColumnSpan(value string) (int, int, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return 0, 0, InvalidArgsError("--range is required (e.g. B or B:D)")
	}
	parts := strings.Split(value, ":")
	if len(parts) > 2 {
		return 0, 0, InvalidArgsError("invalid column range: " + value)
	}
	minCol, err := address.ParseColumn(parts[0])
	if err != nil {
		return 0, 0, NewCLIErrorf(ExitInvalidArgs, "invalid column %q: %v", parts[0], err)
	}
	maxCol := minCol
	if len(parts) == 2 {
		maxCol, err = address.ParseColumn(parts[1])
		if err != nil {
			return 0, 0, NewCLIErrorf(ExitInvalidArgs, "invalid column %q: %v", parts[1], err)
		}
	}
	if maxCol < minCol {
		minCol, maxCol = maxCol, minCol
	}
	return minCol, maxCol, nil
}

func parseRowSpan(value string) (int, int, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return 0, 0, InvalidArgsError("--range is required (e.g. 2 or 2:5)")
	}
	parts := strings.Split(value, ":")
	if len(parts) > 2 {
		return 0, 0, InvalidArgsError("invalid row range: " + value)
	}
	minRow, err := strconv.Atoi(strings.TrimSpace(parts[0]))
	if err != nil || minRow < 1 {
		return 0, 0, NewCLIErrorf(ExitInvalidArgs, "invalid row %q", parts[0])
	}
	maxRow := minRow
	if len(parts) == 2 {
		maxRow, err = strconv.Atoi(strings.TrimSpace(parts[1]))
		if err != nil || maxRow < 1 {
			return 0, 0, NewCLIErrorf(ExitInvalidArgs, "invalid row %q", parts[1])
		}
	}
	if maxRow < minRow {
		minRow, maxRow = maxRow, minRow
	}
	return minRow, maxRow, nil
}

func resolveXLSXWorksheet(filePath, selector string) (opc.PackageSession, *model.Workbook, model.SheetRef, func(), error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
	if err != nil {
		return nil, nil, model.SheetRef{}, nil, err
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		pkg.Close()
		return nil, nil, model.SheetRef{}, nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, selector)
	if err != nil {
		pkg.Close()
		return nil, nil, model.SheetRef{}, nil, err
	}
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		pkg.Close()
		return nil, nil, model.SheetRef{}, nil, err
	}
	return pkg, workbook, sheetRef, func() { pkg.Close() }, nil
}

// ---- colwidths show ----

var xlsxColWidthsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show column widths for a column range",
	Long:  "Show resolved column widths (in character units) for a column range such as B:D.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		minCol, maxCol, err := parseColumnSpan(xlsxColWidthsShowRange)
		if err != nil {
			return err
		}
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxColWidthsShowSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		widths, fallback, err := mutate.ReadColumnWidths(pkg, sheetRef, minCol, maxCol)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read column widths: %v", err)
		}
		minLetters, _ := address.ColumnIndexToLetters(minCol)
		maxLetters, _ := address.ColumnIndexToLetters(maxCol)
		result := &XLSXColWidthsShowResult{
			File:         filePath,
			Sheet:        sheetRef.Name,
			SheetNumber:  sheetRef.Number,
			Range:        minLetters + ":" + maxLetters,
			MinColumn:    minLetters,
			MaxColumn:    maxLetters,
			Count:        maxCol - minCol + 1,
			DefaultWidth: fallback,
			Columns:      map[string]xlsxDimensionEntry{},
		}
		var distinct []float64
		for col := minCol; col <= maxCol; col++ {
			info := widths[col]
			letters, _ := address.ColumnIndexToLetters(col)
			w := info.Width
			result.Columns[letters] = xlsxDimensionEntry{Width: &w, Explicit: info.Explicit, Custom: info.Custom, Hidden: info.Hidden}
			distinct = appendDistinctFloat(distinct, info.Width)
		}
		result.Uniform = len(distinct) <= 1
		result.ColWidthsSet = fmt.Sprintf("ooxml xlsx colwidths set %s --sheet %s --range %s --width <width> --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheetRef)), pptxXLSXCommandArg(result.Range))
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "colwidths show")
		}
		return writeXLSXOutput(cmd, []byte(formatDimensionTable(result.Range, result.Columns, "width")))
	},
}

// ---- rowheights show ----

var xlsxRowHeightsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show row heights for a row range",
	Long:  "Show resolved row heights (in points) for a row range such as 2:5.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		minRow, maxRow, err := parseRowSpan(xlsxRowHeightsShowRange)
		if err != nil {
			return err
		}
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxRowHeightsShowSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		heights, fallback, err := mutate.ReadRowHeights(pkg, sheetRef, minRow, maxRow)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read row heights: %v", err)
		}
		result := &XLSXRowHeightsShowResult{
			File:          filePath,
			Sheet:         sheetRef.Name,
			SheetNumber:   sheetRef.Number,
			Range:         fmt.Sprintf("%d:%d", minRow, maxRow),
			MinRow:        minRow,
			MaxRow:        maxRow,
			Count:         maxRow - minRow + 1,
			DefaultHeight: fallback,
			Rows:          map[string]xlsxDimensionEntry{},
		}
		var distinct []float64
		for r := minRow; r <= maxRow; r++ {
			info := heights[r]
			h := info.Height
			result.Rows[strconv.Itoa(r)] = xlsxDimensionEntry{Height: &h, Explicit: info.Explicit, Custom: info.Custom, Hidden: info.Hidden}
			distinct = appendDistinctFloat(distinct, info.Height)
		}
		result.Uniform = len(distinct) <= 1
		result.RowHeightsSet = fmt.Sprintf("ooxml xlsx rowheights set %s --sheet %s --range %s --height <height> --in-place", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheetRef)), pptxXLSXCommandArg(result.Range))
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "rowheights show")
		}
		return writeXLSXOutput(cmd, []byte(formatDimensionTable(result.Range, result.Rows, "height")))
	},
}

// ---- colwidths set ----

var xlsxColWidthsSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set a uniform column width for a column range",
	Long:  "Set a uniform column width (in character units) across a column range such as B:D.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if !cmd.Flags().Changed("width") {
			return InvalidArgsError("--width is required")
		}
		minCol, maxCol, err := parseColumnSpan(xlsxColWidthsSetRange)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXColWidthsSetResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxColWidthsSetSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			if cmd.Flags().Changed("expect-width") {
				current, _, err := mutate.ReadColumnWidths(pkg, sheetRef, minCol, minCol)
				if err != nil {
					return NewCLIErrorf(ExitUnexpected, "failed to read column widths: %v", err)
				}
				if math.Abs(current[minCol].Width-xlsxColWidthsSetExpect) > xlsxDimensionTolerance {
					return NewCLIErrorf(ExitInvalidArgs, "expected width %.4g but found %.4g", xlsxColWidthsSetExpect, current[minCol].Width)
				}
			}
			setResult, err := mutate.SetColumnWidths(&mutate.ColumnWidthRequest{
				Package:   pkg,
				SheetRef:  sheetRef,
				MinColumn: minCol,
				MaxColumn: maxCol,
				Width:     xlsxColWidthsSetWidth,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to set column widths: %v", err)
			}
			minLetters, _ := address.ColumnIndexToLetters(minCol)
			maxLetters, _ := address.ColumnIndexToLetters(maxCol)
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			result = &XLSXColWidthsSetResult{
				File:        filePath,
				Sheet:       sheetRef.Name,
				SheetNumber: sheetRef.Number,
				Range:       minLetters + ":" + maxLetters,
				MinColumn:   minLetters,
				MaxColumn:   maxLetters,
				Columns:     setResult.Columns,
				Width:       setResult.Width,
				Output:      destinationFile,
				DryRun:      mutOpts != nil && mutOpts.DryRun,
			}
			selector := xlsxSheetSelectorForRef(sheetRef)
			if destinationFile != "" {
				result.ValidateCommand = xlsxValidateCommand(destinationFile)
				result.ColWidthsShowCommand = fmt.Sprintf("ooxml --json xlsx colwidths show %s --sheet %s --range %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(result.Range))
			}
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "colwidths set")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("set width %.4g for %s!%s (%d columns)", result.Width, result.Sheet, result.Range, result.Columns)))
	},
}

// ---- rowheights set ----

var xlsxRowHeightsSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set a uniform row height for a row range",
	Long:  "Set a uniform row height (in points) across a row range such as 2:5.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if !cmd.Flags().Changed("height") {
			return InvalidArgsError("--height is required")
		}
		minRow, maxRow, err := parseRowSpan(xlsxRowHeightsSetRange)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXRowHeightsSetResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxRowHeightsSetSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			if cmd.Flags().Changed("expect-height") {
				current, _, err := mutate.ReadRowHeights(pkg, sheetRef, minRow, minRow)
				if err != nil {
					return NewCLIErrorf(ExitUnexpected, "failed to read row heights: %v", err)
				}
				if math.Abs(current[minRow].Height-xlsxRowHeightsSetExpect) > xlsxDimensionTolerance {
					return NewCLIErrorf(ExitInvalidArgs, "expected height %.4g but found %.4g", xlsxRowHeightsSetExpect, current[minRow].Height)
				}
			}
			setResult, err := mutate.SetRowHeights(&mutate.RowHeightRequest{
				Package:  pkg,
				SheetRef: sheetRef,
				MinRow:   minRow,
				MaxRow:   maxRow,
				Height:   xlsxRowHeightsSetHeight,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to set row heights: %v", err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			result = &XLSXRowHeightsSetResult{
				File:        filePath,
				Sheet:       sheetRef.Name,
				SheetNumber: sheetRef.Number,
				Range:       fmt.Sprintf("%d:%d", minRow, maxRow),
				MinRow:      minRow,
				MaxRow:      maxRow,
				Rows:        setResult.Rows,
				Created:     setResult.Created,
				Height:      setResult.Height,
				Output:      destinationFile,
				DryRun:      mutOpts != nil && mutOpts.DryRun,
			}
			selector := xlsxSheetSelectorForRef(sheetRef)
			if destinationFile != "" {
				result.ValidateCommand = xlsxValidateCommand(destinationFile)
				result.RowHeightsShowCommand = fmt.Sprintf("ooxml --json xlsx rowheights show %s --sheet %s --range %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(result.Range))
			}
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "rowheights set")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("set height %.4g for %s!%s (%d rows)", result.Height, result.Sheet, result.Range, result.Rows)))
	},
}

// ---- helpers ----

func xlsxSheetSelectorForRef(sheet model.SheetRef) string {
	return xlsxSheetSelector(sheet.PrimarySelector, sheet.Name, sheet.Number)
}

func appendDistinctFloat(values []float64, v float64) []float64 {
	for _, existing := range values {
		if math.Abs(existing-v) <= xlsxDimensionTolerance {
			return values
		}
	}
	return append(values, v)
}

func formatDimensionTable(rangeRef string, entries map[string]xlsxDimensionEntry, kind string) string {
	keys := make([]string, 0, len(entries))
	for k := range entries {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	var b strings.Builder
	fmt.Fprintf(&b, "%s %s:\n", kind, rangeRef)
	for _, k := range keys {
		entry := entries[k]
		val := 0.0
		if entry.Width != nil {
			val = *entry.Width
		} else if entry.Height != nil {
			val = *entry.Height
		}
		marker := ""
		if entry.Explicit {
			marker = " (explicit)"
		}
		fmt.Fprintf(&b, "  %s: %.4g%s\n", k, val, marker)
	}
	return strings.TrimRight(b.String(), "\n")
}

func writeJSONResult(cmd *cobra.Command, result any, label string) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal %s JSON: %v", label, err)
	}
	return writeXLSXOutput(cmd, data)
}

func init() {
	xlsxColWidthsShowCmd.Flags().StringVar(&xlsxColWidthsShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxColWidthsShowCmd.Flags().StringVar(&xlsxColWidthsShowRange, "range", "", "column range such as B or B:D")
	xlsxColWidthsCmd.AddCommand(xlsxColWidthsShowCmd)

	xlsxColWidthsSetCmd.Flags().StringVar(&xlsxColWidthsSetSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxColWidthsSetCmd.Flags().StringVar(&xlsxColWidthsSetRange, "range", "", "column range such as B or B:D")
	xlsxColWidthsSetCmd.Flags().Float64Var(&xlsxColWidthsSetWidth, "width", 0, "column width in character units (0-255)")
	xlsxColWidthsSetCmd.Flags().Float64Var(&xlsxColWidthsSetExpect, "expect-width", 0, "guard: require the first column to currently have this width")
	AddMutationFlags(xlsxColWidthsSetCmd)
	xlsxColWidthsCmd.AddCommand(xlsxColWidthsSetCmd)

	xlsxRowHeightsShowCmd.Flags().StringVar(&xlsxRowHeightsShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRowHeightsShowCmd.Flags().StringVar(&xlsxRowHeightsShowRange, "range", "", "row range such as 2 or 2:5")
	xlsxRowHeightsCmd.AddCommand(xlsxRowHeightsShowCmd)

	xlsxRowHeightsSetCmd.Flags().StringVar(&xlsxRowHeightsSetSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRowHeightsSetCmd.Flags().StringVar(&xlsxRowHeightsSetRange, "range", "", "row range such as 2 or 2:5")
	xlsxRowHeightsSetCmd.Flags().Float64Var(&xlsxRowHeightsSetHeight, "height", 0, "row height in points (0-409)")
	xlsxRowHeightsSetCmd.Flags().Float64Var(&xlsxRowHeightsSetExpect, "expect-height", 0, "guard: require the first row to currently have this height")
	AddMutationFlags(xlsxRowHeightsSetCmd)
	xlsxRowHeightsCmd.AddCommand(xlsxRowHeightsSetCmd)

	xlsxCmd.AddCommand(xlsxColWidthsCmd)
	xlsxCmd.AddCommand(xlsxRowHeightsCmd)
}
