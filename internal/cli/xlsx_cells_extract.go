package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXCellsExtractResult struct {
	File  string             `json:"file"`
	Sheet *model.SheetReport `json:"sheet"`
}

var (
	xlsxCellsExtractSheet        string
	xlsxCellsExtractRange        string
	xlsxCellsExtractMaxRows      int
	xlsxCellsExtractMaxCells     int
	xlsxCellsExtractIncludeEmpty bool
)

var xlsxCellsExtractCmd = &cobra.Command{
	Use:   "extract <file>",
	Short: "Extract decoded cells from a worksheet",
	Long:  "Extract decoded worksheet cells with optional A1 range and output limits.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		var rangeRef *address.RangeRef
		if xlsxCellsExtractRange != "" {
			parsed, err := address.ParseRange(xlsxCellsExtractRange)
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
			}
			rangeRef = &parsed
		}
		if xlsxCellsExtractMaxRows < 0 {
			return NewCLIErrorf(ExitInvalidArgs, "--max-rows must be >= 0")
		}
		if xlsxCellsExtractMaxCells < 0 {
			return NewCLIErrorf(ExitInvalidArgs, "--max-cells must be >= 0")
		}

		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()

		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, xlsxCellsExtractSheet)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		ctx, err := xlsxsheet.LoadContext(pkg, workbook)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to load workbook context: %v", err)
		}

		report, err := xlsxsheet.Read(pkg, sheetRef, ctx, xlsxsheet.ReadOptions{
			Range:        rangeRef,
			MaxRows:      xlsxCellsExtractMaxRows,
			MaxCells:     xlsxCellsExtractMaxCells,
			IncludeEmpty: xlsxCellsExtractIncludeEmpty,
			IncludeData:  true,
		})
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read sheet %q: %v", sheetRef.Name, err)
		}

		// Surface a stable cell handle on each read cell (omitted for an absent or
		// duplicated sheetId). This is the read-path analog of the handle field on
		// find hits and the cells-set readback.
		counts := xlsxSheetIDCounts(workbook.Sheets)
		for ri := range report.Rows {
			for ci := range report.Rows[ri].Cells {
				cell := &report.Rows[ri].Cells[ci]
				cell.Handle = xlsxCellHandleString(sheetRef, cell.Ref, counts)
				cell.PrimarySelector = cell.Ref
				cell.Selectors = xlsxCellSelectors(cell.Ref)
			}
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXCellsExtractJSON(cmd, filePath, report)
		}
		return outputXLSXCellsExtractText(cmd, report)
	},
}

func xlsxCellSelectors(ref string) []string {
	if ref == "" {
		return nil
	}
	return []string{ref}
}

func outputXLSXCellsExtractJSON(cmd *cobra.Command, filePath string, report *model.SheetReport) error {
	config := GetGlobalConfig(cmd)
	result := XLSXCellsExtractResult{
		File:  filePath,
		Sheet: report,
	}

	var data []byte
	var err error
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal cells extract JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXCellsExtractText(cmd *cobra.Command, report *model.SheetReport) error {
	config := GetGlobalConfig(cmd)
	out, closer, err := xlsxOutputWriter(config, cmd)
	if err != nil {
		return err
	}
	if closer != nil {
		defer closer.Close()
	}

	fmt.Fprintf(out, "[%d] %s\n", report.Number, report.Name)
	if report.UsedRange.Empty {
		fmt.Fprintln(out, "used: empty")
	} else {
		fmt.Fprintf(out, "used: %s\n", report.UsedRange.Ref)
	}
	if report.Truncated {
		fmt.Fprintln(out, "truncated: true")
	}
	for _, row := range report.Rows {
		fmt.Fprintf(out, "row %d:", row.Number)
		for _, cell := range row.Cells {
			fmt.Fprintf(out, " %s=%s", cell.Ref, cell.Value)
			if cell.Formula != "" {
				fmt.Fprintf(out, " formula=%s", cell.Formula)
			}
		}
		fmt.Fprintln(out)
	}
	return nil
}

func init() {
	xlsxCellsExtractCmd.Flags().StringVar(&xlsxCellsExtractSheet, "sheet", "", "sheet number (1-based) or exact sheet name (default: first sheet)")
	xlsxCellsExtractCmd.Flags().StringVar(&xlsxCellsExtractRange, "range", "", "A1 range to extract")
	xlsxCellsExtractCmd.Flags().IntVar(&xlsxCellsExtractMaxRows, "max-rows", 1000, "maximum rows to emit (0 for unlimited)")
	xlsxCellsExtractCmd.Flags().IntVar(&xlsxCellsExtractMaxCells, "max-cells", 0, "maximum cells to emit (0 for unlimited)")
	xlsxCellsExtractCmd.Flags().BoolVar(&xlsxCellsExtractIncludeEmpty, "include-empty", false, "include empty cells inside the output range")
	xlsxCellsCmd.AddCommand(xlsxCellsExtractCmd)
}
