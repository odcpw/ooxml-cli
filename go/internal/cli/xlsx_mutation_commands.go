package cli

import "fmt"

type XLSXMutationReadbackCommands struct {
	ValidateCommand             string `json:"validateCommand,omitempty"`
	CellsExtractCommand         string `json:"cellsExtractCommand,omitempty"`
	RangesExportCommand         string `json:"rangesExportCommand,omitempty"`
	ValidateCommandTemplate     string `json:"validateCommandTemplate,omitempty"`
	CellsExtractCommandTemplate string `json:"cellsExtractCommandTemplate,omitempty"`
	RangesExportCommandTemplate string `json:"rangesExportCommandTemplate,omitempty"`
}

type XLSXTableAppendReadbackCommands struct {
	TableShowCommand           string `json:"tableShowCommand,omitempty"`
	TableExportCommand         string `json:"tableExportCommand,omitempty"`
	TableShowCommandTemplate   string `json:"tableShowCommandTemplate,omitempty"`
	TableExportCommandTemplate string `json:"tableExportCommandTemplate,omitempty"`
}

type XLSXSheetsMutationReadbackCommands struct {
	ValidateCommand           string `json:"validateCommand,omitempty"`
	SheetsListCommand         string `json:"sheetsListCommand,omitempty"`
	SheetShowCommand          string `json:"sheetShowCommand,omitempty"`
	ValidateCommandTemplate   string `json:"validateCommandTemplate,omitempty"`
	SheetsListCommandTemplate string `json:"sheetsListCommandTemplate,omitempty"`
	SheetShowCommandTemplate  string `json:"sheetShowCommandTemplate,omitempty"`
}

func xlsxMutationReadbackCommands(destination *XLSXRangeDestination) XLSXMutationReadbackCommands {
	if destination == nil {
		return XLSXMutationReadbackCommands{}
	}
	sheetSelector := xlsxSheetSelector(destination.SheetPrimarySelector, destination.Sheet, destination.SheetNumber)
	if destination.File == "" {
		placeholder := xlsxOutputPlaceholder()
		return XLSXMutationReadbackCommands{
			ValidateCommandTemplate:     xlsxValidateCommand(placeholder),
			CellsExtractCommandTemplate: xlsxCellsExtractReadbackCommand(placeholder, sheetSelector, destination.Range),
			RangesExportCommandTemplate: xlsxRangesExportReadbackCommand(placeholder, sheetSelector, destination.Range),
		}
	}
	return XLSXMutationReadbackCommands{
		ValidateCommand:     xlsxValidateCommand(destination.File),
		CellsExtractCommand: xlsxCellsExtractReadbackCommand(destination.File, sheetSelector, destination.Range),
		RangesExportCommand: xlsxRangesExportReadbackCommand(destination.File, sheetSelector, destination.Range),
	}
}

func xlsxTableAppendReadbackCommands(destination *XLSXTableAppendDestination) XLSXTableAppendReadbackCommands {
	if destination == nil {
		return XLSXTableAppendReadbackCommands{}
	}
	sheetSelector := xlsxSheetSelector(destination.SheetPrimarySelector, destination.Sheet, destination.SheetNumber)
	tableSelector := xlsxTableSelector(destination.TablePrimarySelector, destination.Table, 0)
	if destination.File == "" {
		placeholder := xlsxOutputPlaceholder()
		return XLSXTableAppendReadbackCommands{
			TableShowCommandTemplate:   xlsxTableShowCommand(placeholder, sheetSelector, tableSelector),
			TableExportCommandTemplate: xlsxTableExportReadbackCommand(placeholder, sheetSelector, tableSelector),
		}
	}
	return XLSXTableAppendReadbackCommands{
		TableShowCommand:   xlsxTableShowCommand(destination.File, sheetSelector, tableSelector),
		TableExportCommand: xlsxTableExportReadbackCommand(destination.File, sheetSelector, tableSelector),
	}
}

func xlsxSheetsMutationReadbackCommands(destination *XLSXSheetsMutationDestination) XLSXSheetsMutationReadbackCommands {
	if destination == nil {
		return XLSXSheetsMutationReadbackCommands{}
	}
	if destination.File == "" {
		placeholder := xlsxOutputPlaceholder()
		commands := XLSXSheetsMutationReadbackCommands{
			ValidateCommandTemplate:   xlsxValidateCommand(placeholder),
			SheetsListCommandTemplate: xlsxSheetsListCommand(placeholder),
		}
		if destination.Sheet != nil {
			commands.SheetShowCommandTemplate = xlsxSheetShowCommand(placeholder, xlsxSheetSelector(destination.Sheet.PrimarySelector, destination.Sheet.Name, destination.Sheet.Number))
		}
		return commands
	}
	commands := XLSXSheetsMutationReadbackCommands{
		ValidateCommand:   xlsxValidateCommand(destination.File),
		SheetsListCommand: xlsxSheetsListCommand(destination.File),
	}
	if destination.Sheet != nil {
		commands.SheetShowCommand = xlsxSheetShowCommand(destination.File, xlsxSheetSelector(destination.Sheet.PrimarySelector, destination.Sheet.Name, destination.Sheet.Number))
	}
	return commands
}

func xlsxCellsExtractReadbackCommand(filePath, selector, rangeRef string) string {
	return fmt.Sprintf("ooxml --json xlsx cells extract %s --sheet %s --range %s --include-empty", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(rangeRef))
}

func xlsxRangesExportReadbackCommand(filePath, selector, rangeRef string) string {
	return fmt.Sprintf("ooxml --json xlsx ranges export %s --sheet %s --range %s --include-types --include-formulas --include-formats", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(selector), pptxXLSXCommandArg(rangeRef))
}

func xlsxTableExportReadbackCommand(filePath, sheetSelector, tableSelector string) string {
	return fmt.Sprintf("ooxml --json xlsx tables export %s --sheet %s --table %s --include-types --include-formulas", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(sheetSelector), pptxXLSXCommandArg(tableSelector))
}

func xlsxOutputPlaceholder() string {
	return "<out.xlsx>"
}
