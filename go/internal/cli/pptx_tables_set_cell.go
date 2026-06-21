package cli

import (
	"fmt"
	"os"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/spf13/cobra"
)

type PPTXTablesSetCellResult struct {
	File         string            `json:"file"`
	Output       string            `json:"output,omitempty"`
	DryRun       bool              `json:"dryRun"`
	Slide        int               `json:"slide"`
	TableID      int               `json:"tableId"`
	Row          int               `json:"row"`
	Col          int               `json:"col"`
	Text         string            `json:"text"`
	PreviousText string            `json:"previousText"`
	Destination  *PPTXTableSummary `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

var (
	pptxTablesSetCellSlide    int
	pptxTablesSetCellTableID  int
	pptxTablesSetCellTarget   string
	pptxTablesSetCellRow      int
	pptxTablesSetCellCol      int
	pptxTablesSetCellText     string
	pptxTablesSetCellTextFile string
)

var pptxTablesSetCellCmd = &cobra.Command{
	Use:   "set-cell <file>",
	Short: "Set text in one PPTX table cell",
	Long:  "Set one table cell's plain text by slide number, table shape ID, and 1-based row/column.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		for name, value := range map[string]int{
			"slide": pptxTablesSetCellSlide,
			"row":   pptxTablesSetCellRow,
			"col":   pptxTablesSetCellCol,
		} {
			if err := parsePositiveIntFlag(value, name); err != nil {
				return err
			}
		}
		text, err := resolveRequiredPPTXTableText(cmd, "text", "text-file", pptxTablesSetCellText, pptxTablesSetCellTextFile)
		if err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		result, err := performPPTXTablesSetCell(filePath, pptxTablesSetCellSlide, pptxTablesSetCellTableID, pptxTablesSetCellTarget, pptxTablesSetCellRow, pptxTablesSetCellCol, text, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXTablesJSON(cmd, result, "tables set-cell")
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("set slide %d table %d cell R%dC%d = %q", result.Slide, result.TableID, result.Row, result.Col, result.Text)))
	},
}

func performPPTXTablesSetCell(filePath string, slideNumber, tableID int, target string, row, col int, text string, mutOpts *MutationOptions) (*PPTXTablesSetCellResult, error) {
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *PPTXTablesSetCellResult
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
		setResult, err := mutate.SetTableCellText(&mutate.SetTableCellTextRequest{
			Package:     pkg,
			SlideRef:    slideRef,
			TableID:     resolvedTableID,
			RowIndex:    row - 1,
			ColumnIndex: col - 1,
			Text:        text,
		})
		if err != nil {
			return mapPPTXTableMutationError(err)
		}
		destination, err := collectPPTXTableDestination(pkg, slideRef, resolvedTableID, destinationFile)
		if err != nil {
			return err
		}
		result = &PPTXTablesSetCellResult{
			File:         filePath,
			Output:       destinationFile,
			DryRun:       mutOpts.DryRun,
			Slide:        slideNumber,
			TableID:      setResult.TableID,
			Row:          row,
			Col:          col,
			Text:         setResult.Text,
			PreviousText: setResult.PreviousText,
			Destination:  destination,
		}
		result.PPTXBridgeReadbackCommands = pptxTableMutationReadbackCommands(destination)
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func init() {
	pptxTablesSetCellCmd.Flags().IntVar(&pptxTablesSetCellSlide, "slide", 0, "1-based slide number")
	pptxTablesSetCellCmd.Flags().IntVar(&pptxTablesSetCellTableID, "table-id", 0, "table graphic-frame shape ID")
	pptxTablesSetCellCmd.Flags().StringVar(&pptxTablesSetCellTarget, "target", "", "table selector (e.g., table:1, shape:2, ~Table 1)")
	pptxTablesSetCellCmd.Flags().IntVar(&pptxTablesSetCellRow, "row", 0, "1-based table row")
	pptxTablesSetCellCmd.Flags().IntVar(&pptxTablesSetCellCol, "col", 0, "1-based table column")
	pptxTablesSetCellCmd.Flags().StringVar(&pptxTablesSetCellText, "text", "", "replacement cell text; empty string clears the cell")
	pptxTablesSetCellCmd.Flags().StringVar(&pptxTablesSetCellTextFile, "text-file", "", "path to replacement cell text; empty files clear the cell")
	AddMutationFlags(pptxTablesSetCellCmd)
	tablesCmd.AddCommand(pptxTablesSetCellCmd)
}
