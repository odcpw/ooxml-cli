package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

type XLSXTableItem struct {
	model.TableRef
	ShowCommand                    string `json:"showCommand,omitempty"`
	ExportCommand                  string `json:"exportCommand,omitempty"`
	AppendRowsCommandTemplate      string `json:"appendRowsCommandTemplate,omitempty"`
	AppendRecordsCommandTemplate   string `json:"appendRecordsCommandTemplate,omitempty"`
	PPTXUpdateTableCommandTemplate string `json:"pptxUpdateTableCommandTemplate,omitempty"`
	PPTXPlaceTableCommandTemplate  string `json:"pptxPlaceTableCommandTemplate,omitempty"`
	PPTXReplaceTextCommandTemplate string `json:"pptxReplaceTextCommandTemplate,omitempty"`
}

func xlsxTableItem(filePath string, table model.TableRef) XLSXTableItem {
	table = model.WithTableSelectors(table)
	tableSelector := xlsxTableSelector(table.PrimarySelector, table.DisplayName, table.Number)
	sheetSelector := xlsxTableSheetSelector(table)
	item := XLSXTableItem{
		TableRef:                       table,
		ShowCommand:                    xlsxTableShowCommand(filePath, sheetSelector, tableSelector),
		ExportCommand:                  xlsxTableExportCommand(filePath, sheetSelector, tableSelector),
		AppendRowsCommandTemplate:      xlsxTableAppendRowsCommandTemplate(filePath, sheetSelector, tableSelector),
		AppendRecordsCommandTemplate:   xlsxTableAppendRecordsCommandTemplate(filePath, sheetSelector, tableSelector, table.Range),
		PPTXUpdateTableCommandTemplate: xlsxPPTXUpdateTableFromXLSXTableTemplate(filePath, sheetSelector, tableSelector, table.Range),
		PPTXPlaceTableCommandTemplate:  xlsxPPTXPlaceTableFromXLSXTableTemplate(filePath, sheetSelector, tableSelector, table.Range),
	}
	if table.Sheet != "" && table.Range != "" {
		item.PPTXReplaceTextCommandTemplate = xlsxPPTXReplaceTextFromXLSXRangeTemplate(filePath, table.Sheet, table.Range)
	}
	return item
}

func addXLSXRangeBridgeCommands(result *XLSXRangesExportResult) {
	if result == nil || result.File == "" || result.Sheet == "" || result.Range == "" {
		return
	}
	result.ValidateCommand = xlsxValidateCommand(result.File)
	result.CellsExtractCommand = xlsxCellsExtractCommand(result.File, result.Sheet, result.Range)
	result.PPTXUpdateTableCommandTemplate = xlsxPPTXUpdateTableFromXLSXRangeTemplate(result.File, result.Sheet, result.Range)
	result.PPTXPlaceTableCommandTemplate = xlsxPPTXPlaceTableFromXLSXRangeTemplate(result.File, result.Sheet, result.Range)
	result.PPTXReplaceTextCommandTemplate = xlsxPPTXReplaceTextFromXLSXRangeTemplate(result.File, result.Sheet, result.Range)
}

func xlsxTableSelector(primary, displayName string, number int) string {
	if primary != "" {
		return primary
	}
	if displayName != "" {
		return displayName
	}
	if number > 0 {
		return fmt.Sprintf("table:%d", number)
	}
	return "1"
}

func xlsxTableSheetSelector(table model.TableRef) string {
	if table.Sheet != "" {
		return table.Sheet
	}
	if table.SheetNumber > 0 {
		return fmt.Sprintf("sheet:%d", table.SheetNumber)
	}
	return ""
}

func xlsxTableShowCommand(filePath, sheetSelector, tableSelector string) string {
	args := []string{"ooxml", "--json", "xlsx", "tables", "show", pptxXLSXCommandArg(filePath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--table", tableSelector)
	return strings.Join(args, " ")
}

func xlsxTableExportCommand(filePath, sheetSelector, tableSelector string) string {
	args := []string{"ooxml", "--json", "xlsx", "tables", "export", pptxXLSXCommandArg(filePath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--table", tableSelector)
	args = append(args, "--include-types")
	return strings.Join(args, " ")
}

func xlsxTableAppendRowsCommandTemplate(filePath, sheetSelector, tableSelector string) string {
	args := []string{"ooxml", "--json", "xlsx", "tables", "append-rows", pptxXLSXCommandArg(filePath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--table", tableSelector)
	args = append(args, "--values-file", "rows.json", "--out", "out.xlsx")
	return strings.Join(args, " ")
}

func xlsxTableAppendRecordsCommandTemplate(filePath, sheetSelector, tableSelector, expectRange string) string {
	args := []string{"ooxml", "--json", "xlsx", "tables", "append-records", pptxXLSXCommandArg(filePath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--table", tableSelector)
	args = appendXLSXSourceFlag(args, "--expect-range", expectRange)
	args = append(args, "--records-file", "records.json", "--out", "out.xlsx")
	return strings.Join(args, " ")
}

func xlsxPPTXUpdateTableFromXLSXRangeTemplate(workbookPath, sheetSelector, rangeRef string) string {
	args := []string{"ooxml", "--json", "pptx", "tables", "update-from-xlsx", "deck.pptx"}
	args = append(args, xlsxRangeSourceArgs(workbookPath, sheetSelector, rangeRef)...)
	args = append(args, "--slide", "1", "--target", "table:1", "--out", "out.pptx")
	return strings.Join(args, " ")
}

func xlsxPPTXPlaceTableFromXLSXRangeTemplate(workbookPath, sheetSelector, rangeRef string) string {
	args := []string{"ooxml", "--json", "pptx", "place", "table-from-xlsx", "deck.pptx"}
	args = append(args, xlsxRangeSourceArgs(workbookPath, sheetSelector, rangeRef)...)
	args = append(args, "--slide", "1", "--x", "0", "--y", "0", "--cx", "4000000", "--out", "out.pptx")
	return strings.Join(args, " ")
}

func xlsxPPTXReplaceTextFromXLSXRangeTemplate(workbookPath, sheetSelector, rangeRef string) string {
	args := []string{"ooxml", "--json", "pptx", "replace", "text-from-xlsx", "deck.pptx"}
	args = append(args, xlsxRangeSourceArgsWithoutExpect(workbookPath, sheetSelector, rangeRef)...)
	args = append(args, "--slide", "1", "--target", "title", "--out", "out.pptx")
	return strings.Join(args, " ")
}

func xlsxPPTXUpdateTableFromXLSXTableTemplate(workbookPath, sheetSelector, tableSelector, expectRange string) string {
	args := []string{"ooxml", "--json", "pptx", "tables", "update-from-xlsx", "deck.pptx"}
	args = append(args, xlsxTableSourceArgs(workbookPath, sheetSelector, tableSelector, expectRange)...)
	args = append(args, "--slide", "1", "--target", "table:1", "--out", "out.pptx")
	return strings.Join(args, " ")
}

func xlsxPPTXPlaceTableFromXLSXTableTemplate(workbookPath, sheetSelector, tableSelector, expectRange string) string {
	args := []string{"ooxml", "--json", "pptx", "place", "table-from-xlsx", "deck.pptx"}
	args = append(args, xlsxTableSourceArgs(workbookPath, sheetSelector, tableSelector, expectRange)...)
	args = append(args, "--slide", "1", "--x", "0", "--y", "0", "--cx", "4000000", "--out", "out.pptx")
	return strings.Join(args, " ")
}

func xlsxRangeSourceArgs(workbookPath, sheetSelector, rangeRef string) []string {
	args := []string{"--workbook", pptxXLSXCommandArg(workbookPath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--range", rangeRef)
	args = appendXLSXSourceFlag(args, "--expect-source-range", rangeRef)
	return args
}

func xlsxRangeSourceArgsWithoutExpect(workbookPath, sheetSelector, rangeRef string) []string {
	args := []string{"--workbook", pptxXLSXCommandArg(workbookPath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--range", rangeRef)
	return args
}

func xlsxTableSourceArgs(workbookPath, sheetSelector, tableSelector, expectRange string) []string {
	args := []string{"--workbook", pptxXLSXCommandArg(workbookPath)}
	args = appendXLSXSourceFlag(args, "--sheet", sheetSelector)
	args = appendXLSXSourceFlag(args, "--table", tableSelector)
	args = appendXLSXSourceFlag(args, "--expect-source-range", expectRange)
	return args
}

func appendXLSXSourceFlag(args []string, name, value string) []string {
	if strings.TrimSpace(value) == "" {
		return args
	}
	return append(args, name, pptxXLSXCommandArg(value))
}
