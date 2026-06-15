package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXRangesSetResult struct {
	File           string                `json:"file"`
	Sheet          string                `json:"sheet"`
	SheetNumber    int                   `json:"sheetNumber"`
	Anchor         string                `json:"anchor"`
	Range          string                `json:"range"`
	Rows           int                   `json:"rows"`
	Cols           int                   `json:"cols"`
	Updated        int                   `json:"updated"`
	Created        int                   `json:"created"`
	Cleared        int                   `json:"cleared"`
	Skipped        int                   `json:"skipped"`
	FormulaCount   int                   `json:"formulaCount"`
	DataFormat     string                `json:"dataFormat"`
	NullPolicy     string                `json:"nullPolicy"`
	MajorDimension string                `json:"majorDimension"`
	Output         string                `json:"output,omitempty"`
	DryRun         bool                  `json:"dryRun"`
	Destination    *XLSXRangeDestination `json:"destination,omitempty"`
	XLSXMutationReadbackCommands
}

type XLSXRangeDestination struct {
	File                 string     `json:"file,omitempty"`
	Sheet                string     `json:"sheet"`
	SheetNumber          int        `json:"sheetNumber"`
	SheetPrimarySelector string     `json:"sheetPrimarySelector,omitempty"`
	SheetSelectors       []string   `json:"sheetSelectors,omitempty"`
	Range                string     `json:"range"`
	Rows                 int        `json:"rows"`
	Cols                 int        `json:"cols"`
	Values               [][]any    `json:"values,omitempty"`
	Types                [][]string `json:"types,omitempty"`
	Formulas             [][]any    `json:"formulas,omitempty"`
	StyleIndexes         [][]any    `json:"styleIndexes,omitempty"`
	NumberFormatIDs      [][]any    `json:"numberFormatIds,omitempty"`
	NumberFormatCodes    [][]any    `json:"numberFormatCodes,omitempty"`
	FormulaCount         int        `json:"formulaCount"`
	Truncated            bool       `json:"truncated"`
}

var (
	xlsxRangesSetSheet             string
	xlsxRangesSetAnchor            string
	xlsxRangesSetRange             string
	xlsxRangesSetValues            string
	xlsxRangesSetValuesFile        string
	xlsxRangesSetDataFormat        string
	xlsxRangesSetNullPolicy        string
	xlsxRangesSetRagged            string
	xlsxRangesSetMaxCells          int
	xlsxRangesSetOverwriteFormulas bool
)

var xlsxRangesSetCmd = &cobra.Command{
	Use:   "set <file>",
	Short: "Set a worksheet range from a rectangular matrix",
	Long:  "Set a worksheet range from JSON, CSV, or TSV matrix data with guarded formula and merged-cell handling.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXRangeSheet(strings.TrimSpace(xlsxRangesSetSheet)); err != nil {
			return err
		}

		dataFormat, err := rangeio.NormalizeDataFormat(xlsxRangesSetDataFormat)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		matrix, err := resolveXLSXRangesSetMatrix(cmd, dataFormat)
		if err != nil {
			return err
		}
		rows, rowCount, colCount, err := rangeio.Rectangularize(matrix.Values, xlsxRangesSetRagged)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		rangeRef, err := resolveXLSXRangesSetTarget(cmd, matrix, rowCount, colCount)
		if err != nil {
			return err
		}
		if err := checkXLSXRangeMaxCells(rangeRef, xlsxRangesSetMaxCells); err != nil {
			return err
		}
		nullPolicyText := xlsxRangesSetNullPolicy
		if !cmd.Flags().Lookup("null-policy").Changed && matrix.NullPolicy != "" {
			nullPolicyText = matrix.NullPolicy
		}
		nullPolicy, err := mutate.NormalizeRangeNullPolicy(nullPolicyText)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		mutRows, err := xlsxRangeCellsToMutate(rows)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		wantReadback := GetGlobalConfig(cmd).Format == "json"
		result, err := performXLSXRangesSet(filePath, rangeRef, mutRows, nullPolicy, dataFormat, matrix.MajorDimension, mutOpts, wantReadback)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXRangesSetJSON(cmd, result)
		}
		return outputXLSXRangesSetText(cmd, result)
	},
}

func resolveXLSXRangesSetMatrix(cmd *cobra.Command, dataFormat string) (*rangeio.Matrix, error) {
	valuesChanged := cmd.Flags().Lookup("values").Changed
	valuesFileChanged := cmd.Flags().Lookup("values-file").Changed
	if valuesChanged == valuesFileChanged {
		return nil, InvalidArgsError("must specify exactly one of --values or --values-file")
	}

	var (
		data []byte
		err  error
	)
	if valuesChanged {
		data = []byte(xlsxRangesSetValues)
	} else if xlsxRangesSetValuesFile == "-" {
		data, err = io.ReadAll(cmd.InOrStdin())
	} else {
		data, err = os.ReadFile(xlsxRangesSetValuesFile)
	}
	if err != nil {
		return nil, FileNotFoundError(xlsxRangesSetValuesFile)
	}
	matrix, err := rangeio.Decode(data, dataFormat)
	if err != nil {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid %s values: %v", dataFormat, err)
	}
	return matrix, nil
}

func resolveXLSXRangesSetTarget(cmd *cobra.Command, matrix *rangeio.Matrix, rows, cols int) (address.RangeRef, error) {
	anchorChanged := cmd.Flags().Lookup("anchor").Changed
	rangeChanged := cmd.Flags().Lookup("range").Changed
	inputRange := strings.TrimSpace(matrix.Range)

	sourceCount := 0
	if anchorChanged && strings.TrimSpace(xlsxRangesSetAnchor) != "" {
		sourceCount++
	}
	if rangeChanged && strings.TrimSpace(xlsxRangesSetRange) != "" {
		sourceCount++
	}
	if inputRange != "" {
		sourceCount++
	}
	if sourceCount != 1 {
		return address.RangeRef{}, InvalidArgsError("must specify exactly one of --anchor, --range, or JSON input range")
	}

	if anchorChanged {
		anchor, err := address.ParseCell(xlsxRangesSetAnchor)
		if err != nil {
			return address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "invalid --anchor: %v", err)
		}
		rangeRef, err := xlsxRangeFromAnchor(anchor, rows, cols)
		if err != nil {
			return address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "invalid --anchor: %v", err)
		}
		return rangeRef, nil
	}

	rangeText := xlsxRangesSetRange
	if inputRange != "" {
		rangeText = inputRange
	}
	rangeRef, err := address.ParseRange(rangeText)
	if err != nil {
		return address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
	}
	rangeRows, rangeCols := xlsxRangeDimensions(rangeRef)
	if rangeRows != rows || rangeCols != cols {
		return address.RangeRef{}, NewCLIErrorf(ExitInvalidArgs, "range %s is %dx%d but values matrix is %dx%d", rangeRef.String(), rangeRows, rangeCols, rows, cols)
	}
	return rangeRef, nil
}

func performXLSXRangesSet(filePath string, rangeRef address.RangeRef, rows [][]mutate.RangeCell, nullPolicy mutate.RangeNullPolicy, dataFormat, majorDimension string, mutOpts *MutationOptions, wantReadback bool) (*XLSXRangesSetResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}

	var result *XLSXRangesSetResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxRangesSetSheet)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}

		setResult, err := mutate.SetRange(&mutate.SetRangeRequest{
			Package:           pkg,
			WorkbookURI:       workbook.PartURI,
			SheetRef:          sheetRef,
			Range:             rangeRef,
			Rows:              rows,
			NullPolicy:        nullPolicy,
			OverwriteFormulas: xlsxRangesSetOverwriteFormulas,
		})
		if err != nil {
			return mapXLSXRangesSetMutationError(err)
		}
		rangeRows, rangeCols := xlsxRangeDimensions(rangeRef)
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		var destination *XLSXRangeDestination
		if wantReadback {
			destination, err = collectXLSXRangeDestination(pkg, workbook, sheetRef, rangeRef, destinationFile)
			if err != nil {
				return err
			}
		}
		result = &XLSXRangesSetResult{
			File:           filePath,
			Sheet:          sheetRef.Name,
			SheetNumber:    sheetRef.Number,
			Anchor:         rangeRef.Start.String(),
			Range:          setResult.Range,
			Rows:           rangeRows,
			Cols:           rangeCols,
			Updated:        setResult.Updated,
			Created:        setResult.Created,
			Cleared:        setResult.Cleared,
			Skipped:        setResult.Skipped,
			FormulaCount:   setResult.FormulaCount,
			DataFormat:     dataFormat,
			NullPolicy:     string(nullPolicy),
			MajorDimension: majorDimension,
			Output:         destinationFile,
			DryRun:         mutOpts != nil && mutOpts.DryRun,
			Destination:    destination,
		}
		result.XLSXMutationReadbackCommands = xlsxMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func collectXLSXRangeDestination(pkg opc.PackageSession, workbook *model.Workbook, sheetRef model.SheetRef, rangeRef address.RangeRef, destinationFile string) (*XLSXRangeDestination, error) {
	return collectXLSXRangeDestinationWithMaxCells(pkg, workbook, sheetRef, rangeRef, destinationFile, 0)
}

func collectXLSXRangeDestinationWithMaxCells(pkg opc.PackageSession, workbook *model.Workbook, sheetRef model.SheetRef, rangeRef address.RangeRef, destinationFile string, maxCells int) (*XLSXRangeDestination, error) {
	if maxCells < 0 {
		return nil, InvalidArgsError("--readback-max-cells must be >= 0")
	}
	sheetRef = model.WithSheetSelectors(sheetRef)
	ctx, err := xlsxsheet.LoadContext(pkg, workbook)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to load workbook context: %v", err)
	}
	readMaxCells := maxCells
	if readMaxCells == 0 {
		totalCells := xlsxRangeCellCount(rangeRef)
		if totalCells > int64(^uint(0)>>1) {
			return nil, NewCLIErrorf(ExitInvalidArgs, "range %s is too large to read on this platform", rangeRef.String())
		}
		readMaxCells = int(totalCells)
	}
	report, err := xlsxsheet.Read(pkg, sheetRef, ctx, xlsxsheet.ReadOptions{
		Range:        &rangeRef,
		MaxRows:      0,
		MaxCells:     readMaxCells,
		IncludeEmpty: true,
		IncludeData:  true,
	})
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read sheet %q: %v", sheetRef.Name, err)
	}
	rows, cols := xlsxRangeDimensions(rangeRef)
	matrix := xlsxRangeCellsFromRows(report.Rows)
	return &XLSXRangeDestination{
		File:                 destinationFile,
		Sheet:                sheetRef.Name,
		SheetNumber:          sheetRef.Number,
		SheetPrimarySelector: sheetRef.PrimarySelector,
		SheetSelectors:       append([]string{}, sheetRef.Selectors...),
		Range:                rangeRef.String(),
		Rows:                 rows,
		Cols:                 cols,
		Values:               rangeio.PrimitiveValues(matrix),
		Types:                rangeio.Types(matrix),
		Formulas:             rangeio.Formulas(matrix),
		StyleIndexes:         xlsxRangeStyleIndexesFromRows(report.Rows),
		NumberFormatIDs:      xlsxRangeNumberFormatIDsFromRows(report.Rows),
		NumberFormatCodes:    xlsxRangeNumberFormatCodesFromRows(report.Rows),
		FormulaCount:         rangeio.FormulaCount(matrix),
		Truncated:            report.Truncated,
	}, nil
}

func mapXLSXRangesSetMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	if errors.Is(err, mutate.ErrRangeOverwritesFormula) ||
		errors.Is(err, mutate.ErrRangeIntersectsMergedCell) {
		return InvalidArgsError(err.Error())
	}
	return NewCLIErrorf(ExitInvalidArgs, "failed to set range: %v", err)
}

func outputXLSXRangesSetJSON(cmd *cobra.Command, result *XLSXRangesSetResult) error {
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
		return NewCLIErrorf(ExitUnexpected, "failed to marshal ranges set JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXRangesSetText(cmd *cobra.Command, result *XLSXRangesSetResult) error {
	text := fmt.Sprintf("set %s!%s (%dx%d): updated %d", result.Sheet, result.Range, result.Rows, result.Cols, result.Updated)
	if result.Cleared > 0 {
		text += fmt.Sprintf(", cleared %d", result.Cleared)
	}
	if result.Skipped > 0 {
		text += fmt.Sprintf(", skipped %d", result.Skipped)
	}
	if result.FormulaCount > 0 {
		text += fmt.Sprintf(", formulas %d", result.FormulaCount)
	}
	return writeXLSXOutput(cmd, []byte(text))
}

func init() {
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetAnchor, "anchor", "", "top-left A1 cell for the values matrix")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetRange, "range", "", "A1 target range; dimensions must match the values matrix")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetValues, "values", "", "inline JSON/CSV/TSV matrix data")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetValuesFile, "values-file", "", "path to JSON/CSV/TSV matrix data, or - for stdin")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetDataFormat, "data-format", "json", "matrix data format: json, csv, or tsv")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetNullPolicy, "null-policy", "skip", "null handling: skip, clear, or empty-string")
	xlsxRangesSetCmd.Flags().StringVar(&xlsxRangesSetRagged, "ragged", "reject", "ragged row handling: reject or fill-empty")
	xlsxRangesSetCmd.Flags().IntVar(&xlsxRangesSetMaxCells, "max-cells", 100000, "maximum cells to set (0 for unlimited)")
	xlsxRangesSetCmd.Flags().BoolVar(&xlsxRangesSetOverwriteFormulas, "overwrite-formulas", false, "allow writing over existing formula cells")
	AddMutationFlags(xlsxRangesSetCmd)
	xlsxRangesCmd.AddCommand(xlsxRangesSetCmd)
}
