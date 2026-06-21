package cli

import "fmt"

// XLSXStructureReadbackCommands is the additive readback contract attached to
// structural row/column mutation results. It mirrors the two-branch shape of
// pptxBridgeReadbackCommands: concrete commands when a destination file exists,
// otherwise *Template fields keyed off outputPlaceholder() for --dry-run runs.
//
// SheetShowCommand re-inspects the mutated worksheet (declared dimension and
// computed used range), which is what actually confirms an inserted or deleted
// row/column. SheetsListCommand only re-lists the worksheet inventory, so it can
// confirm sheet identity but not the structural change; both are emitted.
type XLSXStructureReadbackCommands struct {
	ValidateCommand           string `json:"validateCommand,omitempty"`
	SheetShowCommand          string `json:"sheetShowCommand,omitempty"`
	SheetsListCommand         string `json:"sheetsListCommand,omitempty"`
	ValidateCommandTemplate   string `json:"validateCommandTemplate,omitempty"`
	SheetShowCommandTemplate  string `json:"sheetShowCommandTemplate,omitempty"`
	SheetsListCommandTemplate string `json:"sheetsListCommandTemplate,omitempty"`
}

// xlsxStructureMutationReadbackCommands builds the generated follow-up commands
// for a structural mutation. destinationFile is the resolved output path (empty
// when the mutation ran as --dry-run). sheetSelector identifies the mutated
// worksheet so the sheets-show readback re-inspects the sheet whose structure
// changed rather than emitting an identity-only inventory listing.
func xlsxStructureMutationReadbackCommands(destinationFile, sheetSelector string) XLSXStructureReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return XLSXStructureReadbackCommands{
			ValidateCommandTemplate:   xlsxValidateCommand(placeholder),
			SheetShowCommandTemplate:  xlsxSheetShowCommand(placeholder, sheetSelector),
			SheetsListCommandTemplate: xlsxSheetsListReadbackCommand(placeholder),
		}
	}
	return XLSXStructureReadbackCommands{
		ValidateCommand:   xlsxValidateCommand(destinationFile),
		SheetShowCommand:  xlsxSheetShowCommand(destinationFile, sheetSelector),
		SheetsListCommand: xlsxSheetsListReadbackCommand(destinationFile),
	}
}

// xlsxSheetsListReadbackCommand renders a follow-up `xlsx sheets list` command
// that re-inspects the post-mutation worksheet inventory.
func xlsxSheetsListReadbackCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json xlsx sheets list %s", pptxXLSXCommandArg(filePath))
}
