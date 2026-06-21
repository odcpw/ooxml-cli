package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXTablesSetCellResult struct {
	File         string `json:"file"`
	Table        int    `json:"table"`
	Block        int    `json:"block"`
	Row          int    `json:"row"`
	Col          int    `json:"col"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
	Text         string `json:"text"`
	PreviousText string `json:"previousText"`
	Flattened    bool   `json:"flattened"`
	Output       string `json:"output,omitempty"`
	DryRun       bool   `json:"dryRun"`
	DOCXTableReadbackCommands
}

var (
	docxTablesSetCellTable    int
	docxTablesSetCellRow      int
	docxTablesSetCellCol      int
	docxTablesSetCellText     string
	docxTablesSetCellTextFile string
	docxTablesSetCellHash     string
)

var docxTablesSetCellCmd = &cobra.Command{
	Use:   "set-cell <file>",
	Short: "Set text in one DOCX table cell",
	Long:  "Set one main-document table cell's plain text by 1-based table, row, and column.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"table": docxTablesSetCellTable,
			"row":   docxTablesSetCellRow,
			"col":   docxTablesSetCellCol,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		text, err := resolveRequiredDOCXTableText(cmd, "text", "text-file", docxTablesSetCellText, docxTablesSetCellTextFile)
		if err != nil {
			return err
		}
		if err := requireDOCXBlockHash(docxTablesSetCellHash); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXTablesSetCell(filePath, docxTablesSetCellTable, docxTablesSetCellRow, docxTablesSetCellCol, docxTablesSetCellHash, text, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "tables set-cell")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("set table %d cell R%dC%d = %q", result.Table, result.Row, result.Col, result.Text)))
	},
}

func performDOCXTablesSetCell(filePath string, table, row, col int, expectedHash, text string, mutOpts *MutationOptions) (*DOCXTablesSetCellResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXTablesSetCellResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		setResult, err := docxmutate.SetTableCellText(&docxmutate.SetTableCellTextRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			TableIndex:   table,
			ExpectedHash: expectedHash,
			RowIndex:     row,
			ColumnIndex:  col,
			Text:         text,
		})
		if err != nil {
			return mapDOCXTableMutationError(fmt.Sprintf("table %d cell R%dC%d", table, row, col), err)
		}
		result = &DOCXTablesSetCellResult{
			File:         filePath,
			Table:        setResult.TableIndex,
			Block:        setResult.BlockIndex,
			Row:          setResult.RowIndex,
			Col:          setResult.ColumnIndex,
			ContentHash:  setResult.ContentHash,
			PreviousHash: setResult.PreviousHash,
			Text:         setResult.Text,
			PreviousText: setResult.PreviousText,
			Flattened:    setResult.Flattened,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
		}
		result.DOCXTableReadbackCommands = docxTableMutationReadbackCommands(destinationFile, setResult.TableIndex)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxTablesSetCellCmd.Flags().IntVar(&docxTablesSetCellTable, "table", 0, "1-based table number")
	docxTablesSetCellCmd.Flags().IntVar(&docxTablesSetCellRow, "row", 0, "1-based table row")
	docxTablesSetCellCmd.Flags().IntVar(&docxTablesSetCellCol, "col", 0, "1-based table column")
	docxTablesSetCellCmd.Flags().StringVar(&docxTablesSetCellHash, "expect-hash", "", "expected sha256: table block hash from docx tables show or docx blocks")
	docxTablesSetCellCmd.Flags().StringVar(&docxTablesSetCellText, "text", "", "replacement cell text; empty string clears the cell")
	docxTablesSetCellCmd.Flags().StringVar(&docxTablesSetCellTextFile, "text-file", "", "path to replacement cell text; empty files clear the cell")
	AddMutationFlags(docxTablesSetCellCmd)
	docxTablesCmd.AddCommand(docxTablesSetCellCmd)
}
