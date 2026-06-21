package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type PPTXTablesDeleteRowResult struct {
	File        string            `json:"file"`
	Output      string            `json:"output,omitempty"`
	DryRun      bool              `json:"dryRun"`
	Slide       int               `json:"slide"`
	TableID     int               `json:"tableId"`
	Row         int               `json:"row"`
	Rows        int               `json:"rows"`
	Cols        int               `json:"cols"`
	CellCount   int               `json:"cellCount"`
	Destination *PPTXTableSummary `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

var (
	pptxTablesDeleteRowSlide   int
	pptxTablesDeleteRowTableID int
	pptxTablesDeleteRowTarget  string
	pptxTablesDeleteRowRow     int
)

var pptxTablesDeleteRowCmd = &cobra.Command{
	Use:   "delete-row <file>",
	Short: "Delete a row from a PPTX table",
	Long:  "Delete one row from a PowerPoint table. --row is a 1-based existing row number.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"slide": pptxTablesDeleteRowSlide,
			"row":   pptxTablesDeleteRowRow,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTablesDeleteRow(filePath, pptxTablesDeleteRowSlide, pptxTablesDeleteRowTableID, pptxTablesDeleteRowTarget, pptxTablesDeleteRowRow, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables delete-row")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("deleted slide %d table %d row %d; table is now %dx%d", result.Slide, result.TableID, result.Row, result.Rows, result.Cols)))
	},
}

func performPPTXTablesDeleteRow(filePath string, slideNumber, tableID int, target string, row int, mutOpts *MutationOptions) (*PPTXTablesDeleteRowResult, error) {
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PPTXTablesDeleteRowResult
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	if err := writer.Write(func(pkg opc.PackageSession) error {
		slideRef, err := resolvePPTXSlideRef(pkg, slideNumber)
		if err != nil {
			return err
		}
		resolvedTableID, err := resolveRequiredPPTXTableTarget(pkg, slideNumber, tableID, target)
		if err != nil {
			return err
		}
		deleteResult, err := mutate.DeleteTableRow(&mutate.DeleteTableRowRequest{
			Package:  pkg,
			SlideRef: slideRef,
			TableID:  resolvedTableID,
			RowIndex: row - 1,
		})
		if err != nil {
			return mapPPTXTableMutationError(err)
		}
		table, err := collectPPTXTableDestination(pkg, slideRef, resolvedTableID, destinationFile)
		if err != nil {
			return err
		}
		result = &PPTXTablesDeleteRowResult{
			File:        filePath,
			Output:      destinationFile,
			DryRun:      mutOpts.DryRun,
			Slide:       slideNumber,
			TableID:     resolvedTableID,
			Row:         row,
			Rows:        table.Rows,
			Cols:        table.Cols,
			CellCount:   deleteResult.CellCount,
			Destination: table,
		}
		result.PPTXBridgeReadbackCommands = pptxTableMutationReadbackCommands(table)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	pptxTablesDeleteRowCmd.Flags().IntVar(&pptxTablesDeleteRowSlide, "slide", 0, "1-based slide number")
	pptxTablesDeleteRowCmd.Flags().IntVar(&pptxTablesDeleteRowTableID, "table-id", 0, "table graphic-frame shape ID")
	pptxTablesDeleteRowCmd.Flags().StringVar(&pptxTablesDeleteRowTarget, "target", "", "table selector (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesDeleteRowCmd.Flags().IntVar(&pptxTablesDeleteRowRow, "row", 0, "1-based table row to delete")
	AddMutationFlags(pptxTablesDeleteRowCmd)
	tablesCmd.AddCommand(pptxTablesDeleteRowCmd)
}
