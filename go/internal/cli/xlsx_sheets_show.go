package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/spf13/cobra"
)

type XLSXSheetsShowResult struct {
	File            string              `json:"file"`
	ValidateCommand string              `json:"validateCommand,omitempty"`
	Sheets          []XLSXSheetShowItem `json:"sheets"`
}

var xlsxSheetsShowSheet string

var xlsxSheetsShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show worksheet metadata and used ranges",
	Long:  "Show worksheet metadata, declared dimensions, computed used ranges, and row/cell counts.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
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
		ctx, err := xlsxsheet.LoadContext(pkg, workbook)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to load workbook context: %v", err)
		}

		sheets := workbook.Sheets
		if xlsxSheetsShowSheet != "" {
			selected, err := selectXLSXSheet(workbook.Sheets, xlsxSheetsShowSheet)
			if err != nil {
				return err
			}
			if err := requireXLSXWorksheetRef(selected); err != nil {
				return err
			}
			sheets = []model.SheetRef{selected}
		} else {
			sheets = filterXLSXWorksheetRefs(workbook.Sheets)
			if len(sheets) == 0 {
				return NewCLIErrorf(ExitInvalidArgs, "workbook has no worksheet sheets")
			}
		}

		reports := make([]*model.SheetReport, 0, len(sheets))
		for _, sheetRef := range sheets {
			report, err := xlsxsheet.Read(pkg, sheetRef, ctx, xlsxsheet.ReadOptions{})
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read sheet %q: %v", sheetRef.Name, err)
			}
			reports = append(reports, report)
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXSheetsShowJSON(cmd, filePath, reports)
		}
		return outputXLSXSheetsShowText(cmd, reports)
	},
}

func outputXLSXSheetsShowJSON(cmd *cobra.Command, filePath string, reports []*model.SheetReport) error {
	config := GetGlobalConfig(cmd)
	items := make([]XLSXSheetShowItem, 0, len(reports))
	for _, report := range reports {
		items = append(items, xlsxSheetShowItem(filePath, report))
	}
	result := XLSXSheetsShowResult{
		File:            filePath,
		ValidateCommand: xlsxValidateCommand(filePath),
		Sheets:          items,
	}

	var data []byte
	var err error
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal sheets show JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXSheetsShowText(cmd *cobra.Command, reports []*model.SheetReport) error {
	config := GetGlobalConfig(cmd)
	out, closer, err := xlsxOutputWriter(config, cmd)
	if err != nil {
		return err
	}
	if closer != nil {
		defer closer.Close()
	}

	for i, report := range reports {
		if i > 0 {
			fmt.Fprintln(out)
		}
		fmt.Fprintf(out, "[%d] %s\n", report.Number, report.Name)
		fmt.Fprintf(out, "  sheetId: %s\n", report.SheetID)
		fmt.Fprintf(out, "  state: %s\n", nonEmpty(report.State, model.SheetStateVisible))
		fmt.Fprintf(out, "  part: %s\n", report.PartURI)
		if report.DimensionDeclared != "" {
			fmt.Fprintf(out, "  declared: %s\n", report.DimensionDeclared)
		}
		if report.UsedRange.Empty {
			fmt.Fprintln(out, "  used: empty")
		} else {
			fmt.Fprintf(out, "  used: %s (%d rows x %d cols)\n", report.UsedRange.Ref, report.UsedRange.Rows, report.UsedRange.Cols)
		}
		fmt.Fprintf(out, "  rows: %d\n", report.RowCount)
		fmt.Fprintf(out, "  cells: %d\n", report.CellCount)
		fmt.Fprintf(out, "  mergedCells: %d\n", report.MergedCellCount)
	}
	return nil
}

func writeXLSXOutput(cmd *cobra.Command, data []byte) error {
	config := GetGlobalConfig(cmd)
	out, closer, err := xlsxOutputWriter(config, cmd)
	if err != nil {
		return err
	}
	if closer != nil {
		defer closer.Close()
	}
	fmt.Fprintf(out, "%s\n", string(data))
	return nil
}

func xlsxOutputWriter(config *GlobalConfig, cmd *cobra.Command) (io.Writer, io.Closer, error) {
	if config.Output == "" {
		return cmd.OutOrStdout(), nil, nil
	}
	file, err := os.Create(config.Output)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
	}
	return file, file, nil
}

func nonEmpty(value, fallback string) string {
	if value == "" {
		return fallback
	}
	return value
}

func filterXLSXWorksheetRefs(sheets []model.SheetRef) []model.SheetRef {
	filtered := make([]model.SheetRef, 0, len(sheets))
	for _, sheet := range sheets {
		if isXLSXWorksheetRef(sheet) {
			filtered = append(filtered, sheet)
		}
	}
	return filtered
}

func init() {
	xlsxSheetsShowCmd.Flags().StringVar(&xlsxSheetsShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxSheetsCmd.AddCommand(xlsxSheetsShowCmd)
}
