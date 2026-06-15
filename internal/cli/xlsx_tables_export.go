package cli

import (
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
	"github.com/spf13/cobra"
)

var (
	xlsxTablesExportSheet           string
	xlsxTablesExportTable           string
	xlsxTablesExportDataFormat      string
	xlsxTablesExportDataOut         string
	xlsxTablesExportIncludeTypes    bool
	xlsxTablesExportIncludeFormulas bool
	xlsxTablesExportMaxCells        int
)

var xlsxTablesExportCmd = &cobra.Command{
	Use:   "export <file>",
	Short: "Export a table as a rectangular matrix",
	Long:  "Export an existing XLSX table range as JSON, CSV, or TSV.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		dataFormat, err := rangeio.NormalizeDataFormat(xlsxTablesExportDataFormat)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}

		tableRef, err := resolveXLSXTableForCLI(filePath, xlsxTablesExportSheet, xlsxTablesExportTable)
		if err != nil {
			return err
		}
		rangeRef, err := address.ParseRange(tableRef.Range)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "invalid table range %s: %v", tableRef.Range, err)
		}
		if err := checkXLSXRangeMaxCells(rangeRef, xlsxTablesExportMaxCells); err != nil {
			return err
		}

		previousSheet := xlsxRangesExportSheet
		previousRange := xlsxRangesExportRange
		defer func() {
			xlsxRangesExportSheet = previousSheet
			xlsxRangesExportRange = previousRange
		}()
		xlsxRangesExportSheet = tableRef.Sheet
		xlsxRangesExportRange = tableRef.Range

		result, matrix, err := performXLSXRangesExport(filePath, rangeRef, dataFormat)
		if err != nil {
			return err
		}
		result.DataOut = xlsxTablesExportDataOut
		result.Values = rangeio.PrimitiveValues(matrix)
		if xlsxTablesExportIncludeTypes {
			result.Types = rangeio.Types(matrix)
		}
		if xlsxTablesExportIncludeFormulas {
			result.Formulas = rangeio.Formulas(matrix)
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

func resolveXLSXTableForCLI(filePath, sheetSelector, tableSelector string) (model.TableRef, error) {
	pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
	if err != nil {
		return model.TableRef{}, err
	}
	defer pkg.Close()

	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return model.TableRef{}, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	sheets := workbook.Sheets
	if sheetSelector != "" {
		selected, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
		if err != nil {
			return model.TableRef{}, err
		}
		if err := requireXLSXWorksheetRef(selected); err != nil {
			return model.TableRef{}, err
		}
		sheets = []model.SheetRef{selected}
	}
	tables, err := xlsxtable.List(pkg, workbook, sheets)
	if err != nil {
		return model.TableRef{}, NewCLIErrorf(ExitUnexpected, "failed to list tables: %v", err)
	}
	return selectXLSXTable(tables, tableSelector)
}

func init() {
	xlsxTablesExportCmd.Flags().StringVar(&xlsxTablesExportSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxTablesExportCmd.Flags().StringVar(&xlsxTablesExportTable, "table", "", "table number, name, or displayName")
	xlsxTablesExportCmd.Flags().StringVar(&xlsxTablesExportDataFormat, "data-format", "json", "matrix data format: json, csv, or tsv")
	xlsxTablesExportCmd.Flags().StringVar(&xlsxTablesExportDataOut, "data-out", "", "write matrix data to this file instead of stdout")
	xlsxTablesExportCmd.Flags().BoolVar(&xlsxTablesExportIncludeTypes, "include-types", false, "include a parallel JSON matrix of decoded cell types")
	xlsxTablesExportCmd.Flags().BoolVar(&xlsxTablesExportIncludeFormulas, "include-formulas", false, "include a parallel JSON matrix of formulas")
	xlsxTablesExportCmd.Flags().IntVar(&xlsxTablesExportMaxCells, "max-cells", 100000, "maximum cells to export (0 for unlimited)")
	xlsxTablesCmd.AddCommand(xlsxTablesExportCmd)
}
