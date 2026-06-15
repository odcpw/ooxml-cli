package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type PPTXTablesInsertRowResult struct {
	File        string            `json:"file"`
	Output      string            `json:"output,omitempty"`
	DryRun      bool              `json:"dryRun"`
	Slide       int               `json:"slide"`
	TableID     int               `json:"tableId"`
	At          int               `json:"at"`
	Rows        int               `json:"rows"`
	Cols        int               `json:"cols"`
	CellCount   int               `json:"cellCount"`
	Destination *PPTXTableSummary `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

var (
	pptxTablesInsertRowSlide   int
	pptxTablesInsertRowTableID int
	pptxTablesInsertRowTarget  string
	pptxTablesInsertRowAt      int
)

var pptxTablesInsertRowCmd = &cobra.Command{
	Use:   "insert-row <file>",
	Short: "Insert an empty row into a PPTX table",
	Long:  "Insert an empty row into a PowerPoint table. --at is a 1-based position; --at 1 prepends and --at rows+1 appends.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"slide": pptxTablesInsertRowSlide,
			"at":    pptxTablesInsertRowAt,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTablesInsertRow(filePath, pptxTablesInsertRowSlide, pptxTablesInsertRowTableID, pptxTablesInsertRowTarget, pptxTablesInsertRowAt, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables insert-row")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("inserted slide %d table %d row at %d; table is now %dx%d", result.Slide, result.TableID, result.At, result.Rows, result.Cols)))
	},
}

func performPPTXTablesInsertRow(filePath string, slideNumber, tableID int, target string, at int, mutOpts *MutationOptions) (*PPTXTablesInsertRowResult, error) {
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PPTXTablesInsertRowResult
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
		insertResult, err := mutate.InsertTableRow(&mutate.InsertTableRowRequest{
			Package:          pkg,
			SlideRef:         slideRef,
			TableID:          resolvedTableID,
			InsertAtRowIndex: at - 1,
		})
		if err != nil {
			return mapPPTXTableMutationError(err)
		}
		table, err := collectPPTXTableDestination(pkg, slideRef, resolvedTableID, destinationFile)
		if err != nil {
			return err
		}
		result = &PPTXTablesInsertRowResult{
			File:        filePath,
			Output:      destinationFile,
			DryRun:      mutOpts.DryRun,
			Slide:       slideNumber,
			TableID:     resolvedTableID,
			At:          at,
			Rows:        table.Rows,
			Cols:        table.Cols,
			CellCount:   insertResult.CellCount,
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
	pptxTablesInsertRowCmd.Flags().IntVar(&pptxTablesInsertRowSlide, "slide", 0, "1-based slide number")
	pptxTablesInsertRowCmd.Flags().IntVar(&pptxTablesInsertRowTableID, "table-id", 0, "table graphic-frame shape ID")
	pptxTablesInsertRowCmd.Flags().StringVar(&pptxTablesInsertRowTarget, "target", "", "table selector (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesInsertRowCmd.Flags().IntVar(&pptxTablesInsertRowAt, "at", 0, "1-based row position for the inserted row; rows+1 appends")
	AddMutationFlags(pptxTablesInsertRowCmd)
	tablesCmd.AddCommand(pptxTablesInsertRowCmd)
}
