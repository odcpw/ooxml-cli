package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXChartsCreateResult struct {
	File              string   `json:"file"`
	Sheet             string   `json:"sheet"`
	SheetNumber       int      `json:"sheetNumber"`
	ChartType         string   `json:"chartType"`
	Title             string   `json:"title,omitempty"`
	ChartPartURI      string   `json:"chartPartUri"`
	DrawingURI        string   `json:"drawingPartUri"`
	SeriesCount       int      `json:"seriesCount"`
	Categories        int      `json:"categories"`
	Anchor            string   `json:"anchor"`
	SourceSheet       string   `json:"sourceSheet"`
	SourceRange       string   `json:"sourceRange"`
	Warnings          []string `json:"warnings,omitempty"`
	Output            string   `json:"output,omitempty"`
	DryRun            bool     `json:"dryRun"`
	ValidateCommand   string   `json:"validateCommand,omitempty"`
	ChartsListCommand string   `json:"chartsListCommand,omitempty"`
}

var (
	xlsxChartsCreateType        string
	xlsxChartsCreateSheet       string
	xlsxChartsCreateRange       string
	xlsxChartsCreateTable       string
	xlsxChartsCreateTitle       string
	xlsxChartsCreateAnchor      string
	xlsxChartsCreateExpectRange string
	xlsxChartsCreateMaxCells    int
)

var xlsxChartsCreateCmd = &cobra.Command{
	Use:   "create <file>",
	Short: "Author a new worksheet chart from a table or range",
	Long:  "Create a bar, line, area, pie, or scatter chart embedded in a worksheet from a table or range source.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		chartType := strings.ToLower(strings.TrimSpace(xlsxChartsCreateType))
		if chartType == "" {
			return InvalidArgsError("--type is required (bar, line, area, pie, scatter)")
		}
		source, matrix, err := loadXLSXRangeOrTableSourceForCLI(filePath, xlsxChartsCreateSheet, xlsxChartsCreateRange, xlsxChartsCreateTable, xlsxChartsCreateMaxCells)
		if err != nil {
			return err
		}
		if xlsxChartsCreateExpectRange != "" && !strings.EqualFold(source.Range, xlsxChartsCreateExpectRange) {
			return NewCLIErrorf(ExitInvalidArgs, "source range mismatch: expected %s but found %s", xlsxChartsCreateExpectRange, source.Range)
		}
		srcRange, err := address.ParseRange(source.Range)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid source range: %v", err)
		}
		anchorFrom, anchorTo, err := resolveChartAnchor(xlsxChartsCreateAnchor, srcRange)
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
		var result *XLSXChartsCreateResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			workbook, err := xlsxinspect.ParseWorkbook(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
			}
			sheetRef, err := selectXLSXSheet(workbook.Sheets, source.Sheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(sheetRef); err != nil {
				return err
			}
			createResult, err := mutate.CreateChart(&mutate.CreateChartRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				SheetRef:    sheetRef,
				ChartType:   chartType,
				SourceSheet: source.Sheet,
				SourceRange: srcRange,
				SourceCells: matrix,
				Title:       xlsxChartsCreateTitle,
				AnchorFrom:  anchorFrom,
				AnchorTo:    anchorTo,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to create chart: %v", err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			result = &XLSXChartsCreateResult{
				File:         filePath,
				Sheet:        sheetRef.Name,
				SheetNumber:  sheetRef.Number,
				ChartType:    createResult.ChartType,
				Title:        createResult.Title,
				ChartPartURI: createResult.ChartURI,
				DrawingURI:   createResult.DrawingURI,
				SeriesCount:  createResult.SeriesCount,
				Categories:   createResult.Categories,
				Anchor:       createResult.Anchor,
				SourceSheet:  source.Sheet,
				SourceRange:  source.Range,
				Warnings:     createResult.Warnings,
				Output:       destinationFile,
				DryRun:       mutOpts != nil && mutOpts.DryRun,
			}
			if destinationFile != "" {
				result.ValidateCommand = xlsxValidateCommand(destinationFile)
				result.ChartsListCommand = fmt.Sprintf("ooxml --json xlsx charts list %s", pptxXLSXCommandArg(destinationFile))
			}
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "charts create")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("created %s chart on %s (%d series) at %s", result.ChartType, result.Sheet, result.SeriesCount, result.Anchor)))
	},
}

// resolveChartAnchor returns the from/to anchor cells. When --anchor is empty,
// the chart is placed two columns to the right of the source range.
func resolveChartAnchor(anchor string, src address.RangeRef) (address.CellRef, address.CellRef, error) {
	const widthCols, heightRows = 8, 15
	var from address.CellRef
	if strings.TrimSpace(anchor) == "" {
		_, _, maxCol, minRow := src.Bounds()
		from = address.CellRef{Column: maxCol + 2, Row: minRow}
	} else {
		c, err := address.ParseCell(anchor)
		if err != nil {
			return address.CellRef{}, address.CellRef{}, NewCLIErrorf(ExitInvalidArgs, "invalid --anchor: %v", err)
		}
		from = address.CellRef{Column: c.Column, Row: c.Row}
	}
	to := address.CellRef{Column: from.Column + widthCols, Row: from.Row + heightRows}
	return from, to, nil
}

func init() {
	f := xlsxChartsCreateCmd.Flags()
	f.StringVar(&xlsxChartsCreateType, "type", "", "chart type: bar, line, area, pie, or scatter")
	f.StringVar(&xlsxChartsCreateSheet, "sheet", "", "source sheet number (1-based) or exact name")
	f.StringVar(&xlsxChartsCreateRange, "range", "", "source A1 range (with a header row and a category column)")
	f.StringVar(&xlsxChartsCreateTable, "table", "", "source table name (alternative to --range)")
	f.StringVar(&xlsxChartsCreateTitle, "title", "", "chart title")
	f.StringVar(&xlsxChartsCreateAnchor, "anchor", "", "top-left anchor cell such as E2 (default: right of the source)")
	f.StringVar(&xlsxChartsCreateExpectRange, "expect-source-range", "", "guard: require the resolved source range to match")
	f.IntVar(&xlsxChartsCreateMaxCells, "max-cells", 100000, "maximum source cells to read (0 for unlimited)")
	AddMutationFlags(xlsxChartsCreateCmd)
	xlsxChartsCmd.AddCommand(xlsxChartsCreateCmd)
}
