package cli

import (
	"fmt"
	"os"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

type DOCXTablesInsertRowResult struct {
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
	docxTablesInsertRowTable int
	docxTablesInsertRowAt    int
	docxTablesInsertRowHash  string
)

var docxTablesInsertRowCmd = &cobra.Command{
	Use:   "insert-row <file>",
	Short: "Insert an empty row into a DOCX table",
	Long:  "Insert an empty row into a main-document table. --at is the 1-based row position the new row will occupy.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"table": docxTablesInsertRowTable,
			"at":    docxTablesInsertRowAt,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		if err := requireDOCXBlockHash(docxTablesInsertRowHash); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performDOCXTablesInsertRow(filePath, docxTablesInsertRowTable, docxTablesInsertRowAt, docxTablesInsertRowHash, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputDOCXTablesJSON(cmd, result, "tables insert-row")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("inserted table %d row %d; table is now %dx%d", result.Table, result.Row, result.Rows, result.Cols)))
	},
}

func performDOCXTablesInsertRow(filePath string, table, at int, expectedHash string, mutOpts *MutationOptions) (*DOCXTablesInsertRowResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeDOCX)
	if err != nil {
		return nil, err
	}

	var result *DOCXTablesInsertRowResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		documentURI, err := docxinspect.FindMainDocumentPart(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to find main document: %v", err)
		}
		insertResult, err := docxmutate.InsertTableRow(&docxmutate.InsertTableRowRequest{
			Package:      pkg,
			DocumentURI:  documentURI,
			TableIndex:   table,
			ExpectedHash: expectedHash,
			At:           at,
		})
		if err != nil {
			return mapDOCXTableMutationError(fmt.Sprintf("table %d row %d", table, at), err)
		}
		result = &DOCXTablesInsertRowResult{
			File:         filePath,
			Table:        insertResult.TableIndex,
			Block:        insertResult.BlockIndex,
			Row:          insertResult.RowIndex,
			Rows:         insertResult.Rows,
			Cols:         insertResult.Cols,
			ContentHash:  insertResult.ContentHash,
			PreviousHash: insertResult.PreviousHash,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
		}
		result.DOCXTableReadbackCommands = docxTableMutationReadbackCommands(destinationFile, insertResult.TableIndex)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	docxTablesInsertRowCmd.Flags().IntVar(&docxTablesInsertRowTable, "table", 0, "1-based table number")
	docxTablesInsertRowCmd.Flags().IntVar(&docxTablesInsertRowAt, "at", 0, "1-based row position for the inserted row")
	docxTablesInsertRowCmd.Flags().StringVar(&docxTablesInsertRowHash, "expect-hash", "", "expected sha256: table block hash from docx tables show or docx blocks")
	AddMutationFlags(docxTablesInsertRowCmd)
	docxTablesCmd.AddCommand(docxTablesInsertRowCmd)
}
