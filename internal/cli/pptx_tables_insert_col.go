package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type PPTXTablesInsertColResult struct {
	File        string            `json:"file"`
	Output      string            `json:"output,omitempty"`
	DryRun      bool              `json:"dryRun"`
	Slide       int               `json:"slide"`
	TableID     int               `json:"tableId"`
	At          int               `json:"at"`
	Rows        int               `json:"rows"`
	Cols        int               `json:"cols"`
	RowCount    int               `json:"rowCount"`
	WidthEMU    int64             `json:"widthEmu"`
	Destination *PPTXTableSummary `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

var (
	pptxTablesInsertColSlide    int
	pptxTablesInsertColTableID  int
	pptxTablesInsertColTarget   string
	pptxTablesInsertColAt       int
	pptxTablesInsertColWidthEMU int64
)

var pptxTablesInsertColCmd = &cobra.Command{
	Use:   "insert-col <file>",
	Short: "Insert an empty column into a PPTX table",
	Long:  "Insert an empty column into a PowerPoint table. --at is a 1-based position; --at 1 prepends and --at cols+1 appends.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"slide": pptxTablesInsertColSlide,
			"at":    pptxTablesInsertColAt,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		if pptxTablesInsertColWidthEMU < 0 {
			return InvalidArgsError("--width-emu must be >= 0")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTablesInsertCol(filePath, pptxTablesInsertColSlide, pptxTablesInsertColTableID, pptxTablesInsertColTarget, pptxTablesInsertColAt, pptxTablesInsertColWidthEMU, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables insert-col")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("inserted slide %d table %d column at %d; table is now %dx%d", result.Slide, result.TableID, result.At, result.Rows, result.Cols)))
	},
}

func performPPTXTablesInsertCol(filePath string, slideNumber, tableID int, target string, at int, widthEMU int64, mutOpts *MutationOptions) (*PPTXTablesInsertColResult, error) {
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PPTXTablesInsertColResult
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
		insertResult, err := mutate.InsertTableColumn(&mutate.InsertTableColumnRequest{
			Package:             pkg,
			SlideRef:            slideRef,
			TableID:             resolvedTableID,
			InsertAtColumnIndex: at - 1,
			Width:               widthEMU,
		})
		if err != nil {
			return mapPPTXTableMutationError(err)
		}
		table, err := collectPPTXTableDestination(pkg, slideRef, resolvedTableID, destinationFile)
		if err != nil {
			return err
		}
		result = &PPTXTablesInsertColResult{
			File:        filePath,
			Output:      destinationFile,
			DryRun:      mutOpts.DryRun,
			Slide:       slideNumber,
			TableID:     resolvedTableID,
			At:          at,
			Rows:        table.Rows,
			Cols:        table.Cols,
			RowCount:    insertResult.RowCount,
			WidthEMU:    insertResult.Width,
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
	pptxTablesInsertColCmd.Flags().IntVar(&pptxTablesInsertColSlide, "slide", 0, "1-based slide number")
	pptxTablesInsertColCmd.Flags().IntVar(&pptxTablesInsertColTableID, "table-id", 0, "table graphic-frame shape ID")
	pptxTablesInsertColCmd.Flags().StringVar(&pptxTablesInsertColTarget, "target", "", "table selector (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesInsertColCmd.Flags().IntVar(&pptxTablesInsertColAt, "at", 0, "1-based column position for the inserted column; cols+1 appends")
	pptxTablesInsertColCmd.Flags().Int64Var(&pptxTablesInsertColWidthEMU, "width-emu", 0, "inserted column width in EMUs; 0 uses the existing average")
	AddMutationFlags(pptxTablesInsertColCmd)
	tablesCmd.AddCommand(pptxTablesInsertColCmd)
}
