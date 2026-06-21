package cli

import "fmt"

// DOCXTableReadbackCommands is the additive readback contract attached to DOCX
// table mutation results. It mirrors the two-branch shape used elsewhere:
// concrete commands when a destination file exists, otherwise *Template fields
// keyed off outputPlaceholder() for --dry-run runs.
//
// DOCX has no `tables list` command, so TablesShowCommand targets the specific
// mutated table (`docx tables show --table N`) and TablesListCommand re-inspects
// every table (`docx tables show` with no selector).
type DOCXTableReadbackCommands struct {
	ValidateCommand           string `json:"validateCommand,omitempty"`
	TablesShowCommand         string `json:"tablesShowCommand,omitempty"`
	TablesListCommand         string `json:"tablesListCommand,omitempty"`
	ValidateCommandTemplate   string `json:"validateCommandTemplate,omitempty"`
	TablesShowCommandTemplate string `json:"tablesShowCommandTemplate,omitempty"`
	TablesListCommandTemplate string `json:"tablesListCommandTemplate,omitempty"`
}

// docxTableMutationReadbackCommands builds the generated follow-up commands for
// a table mutation. destinationFile is the resolved output path (empty when the
// mutation ran as --dry-run); table is the 1-based table index that was edited.
func docxTableMutationReadbackCommands(destinationFile string, table int) DOCXTableReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return DOCXTableReadbackCommands{
			ValidateCommandTemplate:   docxValidateStrictCommand(placeholder),
			TablesShowCommandTemplate: docxTablesShowReadbackCommand(placeholder, table),
			TablesListCommandTemplate: docxTablesListReadbackCommand(placeholder),
		}
	}
	return DOCXTableReadbackCommands{
		ValidateCommand:   docxValidateStrictCommand(destinationFile),
		TablesShowCommand: docxTablesShowReadbackCommand(destinationFile, table),
		TablesListCommand: docxTablesListReadbackCommand(destinationFile),
	}
}

// docxTablesShowReadbackCommand renders a follow-up `docx tables show` command
// scoped to one table.
func docxTablesShowReadbackCommand(filePath string, table int) string {
	return fmt.Sprintf("ooxml --json docx tables show %s --table %d", pptxXLSXCommandArg(filePath), table)
}

// docxTablesListReadbackCommand renders a follow-up `docx tables show` command
// covering every table (DOCX has no dedicated list verb).
func docxTablesListReadbackCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json docx tables show %s", pptxXLSXCommandArg(filePath))
}
