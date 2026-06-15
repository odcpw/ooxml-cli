package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

type XLSXSheetListItem struct {
	model.SheetRef
	// Handle is the stable, paste-safe sheet handle (H:xlsx/ws:<sheetId>),
	// omitted when the sheet has no sheetId or carries a duplicated sheetId.
	Handle            string `json:"handle,omitempty"`
	ShowCommand       string `json:"showCommand,omitempty"`
	TablesListCommand string `json:"tablesListCommand,omitempty"`
}

type XLSXSheetShowItem struct {
	model.SheetReport
	CellsExtractCommand     string `json:"cellsExtractCommand,omitempty"`
	RangesExportCommand     string `json:"rangesExportCommand,omitempty"`
	TablesListCommand       string `json:"tablesListCommand,omitempty"`
	SetCellCommandTemplate  string `json:"setCellCommandTemplate,omitempty"`
	SetRangeCommandTemplate string `json:"setRangeCommandTemplate,omitempty"`
}

func xlsxSheetListItem(filePath string, sheet model.SheetRef) XLSXSheetListItem {
	return xlsxSheetListItemWithCounts(filePath, sheet, nil)
}

// xlsxSheetListItemWithCounts builds a list item and mints a sheet handle,
// omitting it for a non-unique sheetId per the supplied counts map (nil skips
// the uniqueness check).
func xlsxSheetListItemWithCounts(filePath string, sheet model.SheetRef, counts map[string]int) XLSXSheetListItem {
	selector := xlsxSheetSelector(sheet.PrimarySelector, sheet.Name, sheet.Number)
	return XLSXSheetListItem{
		SheetRef:          sheet,
		Handle:            xlsxSheetHandleString(sheet, counts),
		ShowCommand:       xlsxSheetShowCommand(filePath, selector),
		TablesListCommand: xlsxTablesListCommand(filePath, selector),
	}
}

func xlsxSheetShowItem(filePath string, report *model.SheetReport) XLSXSheetShowItem {
	item := XLSXSheetShowItem{SheetReport: *report}
	selector := xlsxSheetSelector(report.PrimarySelector, report.Name, report.Number)
	item.TablesListCommand = xlsxTablesListCommand(filePath, selector)
	item.SetCellCommandTemplate = xlsxSetCellCommandTemplate(filePath, selector)
	if !report.UsedRange.Empty && report.UsedRange.Ref != "" {
		item.CellsExtractCommand = xlsxCellsExtractCommand(filePath, selector, report.UsedRange.Ref)
		item.RangesExportCommand = xlsxRangesExportCommand(filePath, selector, report.UsedRange.Ref)
		item.SetRangeCommandTemplate = xlsxSetRangeCommandTemplate(filePath, selector, report.UsedRange.Ref)
	}
	return item
}

func xlsxSheetSelector(primary, name string, number int) string {
	if primary != "" {
		return primary
	}
	if name != "" {
		return name
	}
	if number > 0 {
		return fmt.Sprintf("sheet:%d", number)
	}
	return "1"
}

func xlsxValidateCommand(filePath string) string {
	return fmt.Sprintf("ooxml validate --strict %s", pptxXLSXCommandArg(filePath))
}

func xlsxSheetsListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json xlsx sheets list %s", pptxXLSXCommandArg(filePath))
}

func xlsxSheetShowCommand(filePath, selector string) string {
	return fmt.Sprintf("ooxml --json xlsx sheets show %s --sheet %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector))
}

func xlsxTablesListCommand(filePath, selector string) string {
	return fmt.Sprintf("ooxml --json xlsx tables list %s --sheet %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector))
}

func xlsxCellsExtractCommand(filePath, selector, rangeRef string) string {
	return fmt.Sprintf("ooxml --json xlsx cells extract %s --sheet %s --range %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(rangeRef))
}

func xlsxRangesExportCommand(filePath, selector, rangeRef string) string {
	return fmt.Sprintf("ooxml --json xlsx ranges export %s --sheet %s --range %s --include-types", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(rangeRef))
}

func xlsxSetCellCommandTemplate(filePath, selector string) string {
	return fmt.Sprintf("ooxml --json xlsx cells set %s --sheet %s --cell A1 --value VALUE --out out.xlsx", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector))
}

func xlsxSetRangeCommandTemplate(filePath, selector, rangeRef string) string {
	return fmt.Sprintf("ooxml --json xlsx ranges set %s --sheet %s --range %s --data-format json --values-file values.json --out out.xlsx", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(rangeRef))
}
