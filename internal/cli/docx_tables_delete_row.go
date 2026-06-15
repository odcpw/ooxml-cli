package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXTablesDeleteRowResult struct {
	File         string `json:"file"`
	Table        int    `json:"table"`
	Block        int    `json:"block"`
	Row          int    `json:"row"`
	Rows         int    `json:"rows"`
	Cols         int    `json:"cols"`
	ContentHash  string `json:"contentHash"`
	PreviousHash string `json:"previousHash"`
	Output       string `json:"output,omitempty"`
	DryRun       bool   `json:"dryRun"`
	DOCXTableReadbackCommands
}

var (
	docxTablesDeleteRowTable int
	docxTablesDeleteRowRow   int
	docxTablesDeleteRowHash  string
)

var docxTablesDeleteRowCmd = &cobra.Command{
	Use:   "delete-row <file>",
	Short: "Delete a row from a DOCX table",
	Long:  "Delete one row from a main-document table.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"table": docxTablesDeleteRowTable,
			"row":   docxTablesDeleteRowRow,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		if err := requireDOCXBlockHash(docxTablesDeleteRowHash); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXTablesDeleteRow(filePath, docxTablesDeleteRowTable, docxTablesDeleteRowRow, docxTablesDeleteRowHash, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "tables delete-row")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("deleted table %d row %d; table is now %dx%d", result.Table, result.Row, result.Rows, result.Cols)))
	},
}

func performDOCXTablesDeleteRow(filePath string, table, row int, expectedHash string, mutOpts *MutationOptions) (*DOCXTablesDeleteRowResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXTablesDeleteRowResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		deleteResult, err := docxmutate.DeleteTableRow(&docxmutate.DeleteTableRowRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			TableIndex:   table,
			ExpectedHash: expectedHash,
			RowIndex:     row,
		})
		if err != nil {
			return mapDOCXTableMutationError(fmt.Sprintf("table %d row %d", table, row), err)
		}
		result = &DOCXTablesDeleteRowResult{
			File:         filePath,
			Table:        deleteResult.TableIndex,
			Block:        deleteResult.BlockIndex,
			Row:          deleteResult.RowIndex,
			Rows:         deleteResult.Rows,
			Cols:         deleteResult.Cols,
			ContentHash:  deleteResult.ContentHash,
			PreviousHash: deleteResult.PreviousHash,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
		}
		result.DOCXTableReadbackCommands = docxTableMutationReadbackCommands(destinationFile, deleteResult.TableIndex)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxTablesDeleteRowCmd.Flags().IntVar(&docxTablesDeleteRowTable, "table", 0, "1-based table number")
	docxTablesDeleteRowCmd.Flags().IntVar(&docxTablesDeleteRowRow, "row", 0, "1-based table row")
	docxTablesDeleteRowCmd.Flags().StringVar(&docxTablesDeleteRowHash, "expect-hash", "", "expected sha256: table block hash from docx tables show or docx blocks")
	AddMutationFlags(docxTablesDeleteRowCmd)
	docxTablesCmd.AddCommand(docxTablesDeleteRowCmd)
}
