package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXRangesExportResult struct {
	File                           string     `json:"file"`
	Sheet                          string     `json:"sheet"`
	SheetNumber                    int        `json:"sheetNumber"`
	Range                          string     `json:"range"`
	PrimarySelector                string     `json:"primarySelector"`
	Selectors                      []string   `json:"selectors"`
	Rows                           int        `json:"rows"`
	Cols                           int        `json:"cols"`
	Values                         [][]any    `json:"values,omitempty"`
	Types                          [][]string `json:"types,omitempty"`
	Formulas                       [][]any    `json:"formulas,omitempty"`
	StyleIndexes                   [][]any    `json:"styleIndexes,omitempty"`
	NumberFormatIDs                [][]any    `json:"numberFormatIds,omitempty"`
	NumberFormatCodes              [][]any    `json:"numberFormatCodes,omitempty"`
	FormulaCount                   int        `json:"formulaCount"`
	DataFormat                     string     `json:"dataFormat"`
	DataOut                        string     `json:"dataOut,omitempty"`
	Truncated                      bool       `json:"truncated"`
	MajorDimension                 string     `json:"majorDimension"`
	ValidateCommand                string     `json:"validateCommand,omitempty"`
	CellsExtractCommand            string     `json:"cellsExtractCommand,omitempty"`
	PPTXUpdateTableCommandTemplate string     `json:"pptxUpdateTableCommandTemplate,omitempty"`
	PPTXPlaceTableCommandTemplate  string     `json:"pptxPlaceTableCommandTemplate,omitempty"`
	PPTXReplaceTextCommandTemplate string     `json:"pptxReplaceTextCommandTemplate,omitempty"`
}

var (
	xlsxRangesExportSheet           string
	xlsxRangesExportRange           string
	xlsxRangesExportDataFormat      string
	xlsxRangesExportDataOut         string
	xlsxRangesExportIncludeTypes    bool
	xlsxRangesExportIncludeFormulas bool
	xlsxRangesExportIncludeFormats  bool
	xlsxRangesExportMaxCells        int
)

var xlsxRangesExportCmd = &cobra.Command{
	Use:   "export <file>",
	Short: "Export a worksheet range as a rectangular matrix",
	Long:  "Export a worksheet A1 range as JSON, CSV, or TSV while preserving rectangular empty cells.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requireXLSXRangeSheet(strings.TrimSpace(xlsxRangesExportSheet)); err != nil {
			return err
		}
		if strings.TrimSpace(xlsxRangesExportRange) == "" {
			return InvalidArgsError("--range is required")
		}
		rangeRef, err := address.ParseRange(xlsxRangesExportRange)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
		}
		if err := checkXLSXRangeMaxCells(rangeRef, xlsxRangesExportMaxCells); err != nil {
			return err
		}
		dataFormat, err := rangeio.NormalizeDataFormat(xlsxRangesExportDataFormat)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}

		result, matrix, err := performXLSXRangesExport(filePath, rangeRef, dataFormat)
		if err != nil {
			return err
		}
		result.DataOut = xlsxRangesExportDataOut
		result.Values = rangeio.PrimitiveValues(matrix)
		if xlsxRangesExportIncludeTypes {
			result.Types = rangeio.Types(matrix)
		}
		if xlsxRangesExportIncludeFormulas {
			result.Formulas = rangeio.Formulas(matrix)
		}
		if !xlsxRangesExportIncludeFormats {
			result.StyleIndexes = nil
			result.NumberFormatIDs = nil
			result.NumberFormatCodes = nil
		}

		switch dataFormat {
		case rangeio.FormatJSON:
			return outputXLSXRangesExportJSON(cmd, result)
		case rangeio.FormatCSV, rangeio.FormatTSV:
			data, err := rangeio.EncodeDelimited(matrix, dataFormat)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to encode %s: %v", dataFormat, err)
			}
			return outputXLSXRangesDelimited(cmd, data, result)
		default:
			return NewCLIErrorf(ExitInvalidArgs, "unsupported data format %q", dataFormat)
		}
	},
}

func performXLSXRangesExport(filePath string, rangeRef address.RangeRef, dataFormat string) (*XLSXRangesExportResult, [][]rangeio.Cell, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
	if err != nil {
		return nil, nil, err
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxRangesExportSheet)
	if err != nil {
		return nil, nil, err
	}
	if err := requireXLSXWorksheetRef(sheetRef); err != nil {
		return nil, nil, err
	}
	ctx, err := xlsxsheet.LoadContext(pkg, workbook)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to load workbook context: %v", err)
	}

	totalCells := xlsxRangeCellCount(rangeRef)
	if totalCells > int64(^uint(0)>>1) {
		return nil, nil, NewCLIErrorf(ExitInvalidArgs, "range %s is too large to export on this platform", rangeRef.String())
	}
	report, err := xlsxsheet.Read(pkg, sheetRef, ctx, xlsxsheet.ReadOptions{
		Range:        &rangeRef,
		MaxRows:      0,
		MaxCells:     int(totalCells),
		IncludeEmpty: true,
		IncludeData:  true,
	})
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to read sheet %q: %v", sheetRef.Name, err)
	}

	rows, cols := xlsxRangeDimensions(rangeRef)
	matrix := xlsxRangeCellsFromRows(report.Rows)
	result := &XLSXRangesExportResult{
		File:              filePath,
		Sheet:             sheetRef.Name,
		SheetNumber:       sheetRef.Number,
		Range:             rangeRef.String(),
		PrimarySelector:   rangeRef.String(),
		Selectors:         xlsxRangeSelectors(rangeRef.String()),
		Rows:              rows,
		Cols:              cols,
		StyleIndexes:      xlsxRangeStyleIndexesFromRows(report.Rows),
		NumberFormatIDs:   xlsxRangeNumberFormatIDsFromRows(report.Rows),
		NumberFormatCodes: xlsxRangeNumberFormatCodesFromRows(report.Rows),
		FormulaCount:      rangeio.FormulaCount(matrix),
		DataFormat:        dataFormat,
		Truncated:         report.Truncated,
		MajorDimension:    "rows",
	}
	addXLSXRangeBridgeCommands(result)
	return result, matrix, nil
}

func xlsxRangeSelectors(ref string) []string {
	if ref == "" {
		return nil
	}
	return []string{ref}
}

func outputXLSXRangesExportJSON(cmd *cobra.Command, result *XLSXRangesExportResult) error {
	data, err := marshalXLSXRangesExportResult(cmd, result)
	if err != nil {
		return err
	}
	if result.DataOut != "" {
		if err := os.WriteFile(result.DataOut, append(data, '\n'), 0o644); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to write --data-out: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			result.Values = nil
			result.Types = nil
			result.Formulas = nil
			result.StyleIndexes = nil
			result.NumberFormatIDs = nil
			result.NumberFormatCodes = nil
			summaryData, err := marshalXLSXRangesExportResult(cmd, result)
			if err != nil {
				return err
			}
			return writeXLSXOutput(cmd, summaryData)
		}
		summary := fmt.Sprintf("exported %s!%s to %s (%dx%d)", result.Sheet, result.Range, result.DataOut, result.Rows, result.Cols)
		return writeXLSXOutput(cmd, []byte(summary))
	}
	return writeXLSXOutput(cmd, data)
}

func marshalXLSXRangesExportResult(cmd *cobra.Command, result *XLSXRangesExportResult) ([]byte, error) {
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
		return nil, NewCLIErrorf(ExitUnexpected, "failed to marshal ranges export JSON: %v", err)
	}
	return data, nil
}

func outputXLSXRangesDelimited(cmd *cobra.Command, data []byte, result *XLSXRangesExportResult) error {
	if result.DataOut != "" {
		if err := os.WriteFile(result.DataOut, data, 0o644); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to write --data-out: %v", err)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			result.Values = nil
			result.Types = nil
			result.Formulas = nil
			result.StyleIndexes = nil
			result.NumberFormatIDs = nil
			result.NumberFormatCodes = nil
			data, err := marshalXLSXRangesExportResult(cmd, result)
			if err != nil {
				return err
			}
			return writeXLSXOutput(cmd, data)
		}
		summary := fmt.Sprintf("exported %s!%s to %s (%dx%d)", result.Sheet, result.Range, result.DataOut, result.Rows, result.Cols)
		return writeXLSXOutput(cmd, []byte(summary))
	}

	config := GetGlobalConfig(cmd)
	out, closer, err := xlsxOutputWriter(config, cmd)
	if err != nil {
		return err
	}
	if closer != nil {
		defer closer.Close()
	}
	if _, err := out.Write(data); err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to write range data: %v", err)
	}
	return nil
}

func init() {
	xlsxRangesExportCmd.Flags().StringVar(&xlsxRangesExportSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxRangesExportCmd.Flags().StringVar(&xlsxRangesExportRange, "range", "", "A1 range to export")
	xlsxRangesExportCmd.Flags().StringVar(&xlsxRangesExportDataFormat, "data-format", "json", "matrix data format: json, csv, or tsv")
	xlsxRangesExportCmd.Flags().StringVar(&xlsxRangesExportDataOut, "data-out", "", "write matrix data to this file instead of stdout")
	xlsxRangesExportCmd.Flags().BoolVar(&xlsxRangesExportIncludeTypes, "include-types", false, "include a parallel JSON matrix of decoded cell types")
	xlsxRangesExportCmd.Flags().BoolVar(&xlsxRangesExportIncludeFormulas, "include-formulas", false, "include a parallel JSON matrix of formulas")
	xlsxRangesExportCmd.Flags().BoolVar(&xlsxRangesExportIncludeFormats, "include-formats", false, "include parallel JSON matrices of style indexes and number formats")
	xlsxRangesExportCmd.Flags().IntVar(&xlsxRangesExportMaxCells, "max-cells", 100000, "maximum cells to export (0 for unlimited)")
	xlsxRangesCmd.AddCommand(xlsxRangesExportCmd)
}
