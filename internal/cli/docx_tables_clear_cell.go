package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXTablesClearCellResult struct {
	File         string `json:"file"`
	Table        int    `json:"table"`
	Block        int    `json:"block"`
	Row          int    `json:"row"`
	Col          int    `json:"col"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
	PreviousText string `json:"previousText"`
	Flattened    bool   `json:"flattened"`
	Output       string `json:"output,omitempty"`
	DryRun       bool   `json:"dryRun"`
	DOCXTableReadbackCommands
}

var (
	docxTablesClearCellTable int
	docxTablesClearCellRow   int
	docxTablesClearCellCol   int
	docxTablesClearCellHash  string
)

var docxTablesClearCellCmd = &cobra.Command{
	Use:   "clear-cell <file>",
	Short: "Clear text in one DOCX table cell",
	Long:  "Clear one main-document table cell's text by 1-based table, row, and column.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"table": docxTablesClearCellTable,
			"row":   docxTablesClearCellRow,
			"col":   docxTablesClearCellCol,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		if err := requireDOCXBlockHash(docxTablesClearCellHash); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXTablesClearCell(filePath, docxTablesClearCellTable, docxTablesClearCellRow, docxTablesClearCellCol, docxTablesClearCellHash, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "tables clear-cell")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("cleared table %d cell R%dC%d", result.Table, result.Row, result.Col)))
	},
}

func performDOCXTablesClearCell(filePath string, table, row, col int, expectedHash string, mutOpts *MutationOptions) (*DOCXTablesClearCellResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXTablesClearCellResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		clearResult, err := docxmutate.ClearTableCellText(&docxmutate.ClearTableCellTextRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			TableIndex:   table,
			ExpectedHash: expectedHash,
			RowIndex:     row,
			ColumnIndex:  col,
		})
		if err != nil {
			return mapDOCXTableMutationError(fmt.Sprintf("table %d cell R%dC%d", table, row, col), err)
		}
		result = &DOCXTablesClearCellResult{
			File:         filePath,
			Table:        clearResult.TableIndex,
			Block:        clearResult.BlockIndex,
			Row:          clearResult.RowIndex,
			Col:          clearResult.ColumnIndex,
			ContentHash:  clearResult.ContentHash,
			PreviousHash: clearResult.PreviousHash,
			PreviousText: clearResult.PreviousText,
			Flattened:    clearResult.Flattened,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
		}
		result.DOCXTableReadbackCommands = docxTableMutationReadbackCommands(destinationFile, clearResult.TableIndex)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxTablesClearCellCmd.Flags().IntVar(&docxTablesClearCellTable, "table", 0, "1-based table number")
	docxTablesClearCellCmd.Flags().IntVar(&docxTablesClearCellRow, "row", 0, "1-based table row")
	docxTablesClearCellCmd.Flags().IntVar(&docxTablesClearCellCol, "col", 0, "1-based table column")
	docxTablesClearCellCmd.Flags().StringVar(&docxTablesClearCellHash, "expect-hash", "", "expected sha256: table block hash from docx tables show or docx blocks")
	AddMutationFlags(docxTablesClearCellCmd)
	docxTablesCmd.AddCommand(docxTablesClearCellCmd)
}
