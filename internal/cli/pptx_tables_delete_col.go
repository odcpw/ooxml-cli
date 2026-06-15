package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type PPTXTablesDeleteColResult struct {
	File        string            `json:"file"`
	Output      string            `json:"output,omitempty"`
	DryRun      bool              `json:"dryRun"`
	Slide       int               `json:"slide"`
	TableID     int               `json:"tableId"`
	Col         int               `json:"col"`
	Rows        int               `json:"rows"`
	Cols        int               `json:"cols"`
	RowCount    int               `json:"rowCount"`
	Destination *PPTXTableSummary `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

var (
	pptxTablesDeleteColSlide   int
	pptxTablesDeleteColTableID int
	pptxTablesDeleteColTarget  string
	pptxTablesDeleteColCol     int
)

var pptxTablesDeleteColCmd = &cobra.Command{
	Use:   "delete-col <file>",
	Short: "Delete a column from a PPTX table",
	Long:  "Delete one column from a PowerPoint table. --col is a 1-based existing column number.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"slide": pptxTablesDeleteColSlide,
			"col":   pptxTablesDeleteColCol,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTablesDeleteCol(filePath, pptxTablesDeleteColSlide, pptxTablesDeleteColTableID, pptxTablesDeleteColTarget, pptxTablesDeleteColCol, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables delete-col")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("deleted slide %d table %d column %d; table is now %dx%d", result.Slide, result.TableID, result.Col, result.Rows, result.Cols)))
	},
}

func performPPTXTablesDeleteCol(filePath string, slideNumber, tableID int, target string, col int, mutOpts *MutationOptions) (*PPTXTablesDeleteColResult, error) {
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PPTXTablesDeleteColResult
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
		deleteResult, err := mutate.DeleteTableColumn(&mutate.DeleteTableColumnRequest{
			Package:     pkg,
			SlideRef:    slideRef,
			TableID:     resolvedTableID,
			ColumnIndex: col - 1,
		})
		if err != nil {
			return mapPPTXTableMutationError(err)
		}
		table, err := collectPPTXTableDestination(pkg, slideRef, resolvedTableID, destinationFile)
		if err != nil {
			return err
		}
		result = &PPTXTablesDeleteColResult{
			File:        filePath,
			Output:      destinationFile,
			DryRun:      mutOpts.DryRun,
			Slide:       slideNumber,
			TableID:     resolvedTableID,
			Col:         col,
			Rows:        table.Rows,
			Cols:        table.Cols,
			RowCount:    deleteResult.RowCount,
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
	pptxTablesDeleteColCmd.Flags().IntVar(&pptxTablesDeleteColSlide, "slide", 0, "1-based slide number")
	pptxTablesDeleteColCmd.Flags().IntVar(&pptxTablesDeleteColTableID, "table-id", 0, "table graphic-frame shape ID")
	pptxTablesDeleteColCmd.Flags().StringVar(&pptxTablesDeleteColTarget, "target", "", "table selector (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesDeleteColCmd.Flags().IntVar(&pptxTablesDeleteColCol, "col", 0, "1-based table column to delete")
	AddMutationFlags(pptxTablesDeleteColCmd)
	tablesCmd.AddCommand(pptxTablesDeleteColCmd)
}
