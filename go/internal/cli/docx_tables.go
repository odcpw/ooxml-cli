package cli

import (
	"errors"
	"fmt"
	"os"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	docxmutate "github.com/ooxml-cli/ooxml-cli/pkg/docx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/cobra"
)

var docxTablesCmd = &cobra.Command{
	Use:     "tables",
	Aliases: []string{"table"},
	Short:   "Inspect and mutate DOCX tables",
	Long:    "Commands for inspecting and mutating Word main-document tables.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type DOCXTableSummary struct {
	File            string       `json:"file,omitempty"`
	Table           int          `json:"table"`
	Block           int          `json:"block"`
	PrimarySelector string       `json:"primarySelector,omitempty"`
	Selectors       []string     `json:"selectors,omitempty"`
	ContentHash     string       `json:"contentHash"`
	Rows            int          `json:"rows"`
	Cols            int          `json:"cols"`
	Merged          bool         `json:"merged"`
	Cells           [][]string   `json:"cells,omitempty"`
	TableInfo       *model.Table `json:"tableInfo,omitempty"`
}

func collectDOCXTables(pkg opc.PackageSession, documentURI string, tableIndex int, includeDetails bool) ([]DOCXTableSummary, error) {
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read main document: %v", err)
	}
	bodyElem, err := docxbody.FindBody(doc.Root())
	if err != nil {
		return nil, err
	}

	var tables []DOCXTableSummary
	tableNumber := 0
	for _, block := range docxbody.Blocks(bodyElem) {
		if block.Kind != model.BlockKindTable {
			continue
		}
		tableNumber++
		if tableIndex > 0 && tableNumber != tableIndex {
			continue
		}
		table := extractDOCXTable(block.Element)
		report := extract.ReportBlock(block, false)
		rows, cols := docxTableDimensions(table)
		summary := DOCXTableSummary{
			Table:           tableNumber,
			Block:           block.Index,
			PrimarySelector: fmt.Sprintf("%d", tableNumber),
			Selectors:       []string{fmt.Sprintf("%d", tableNumber)},
			ContentHash:     report.ContentHash,
			Rows:            rows,
			Cols:            cols,
			Merged:          docxTableHasMergedCells(block.Element),
		}
		if includeDetails {
			summary.TableInfo = table
		} else {
			summary.Cells = tableRowsAsStrings(table)
		}
		tables = append(tables, summary)
	}
	if tableIndex > 0 && len(tables) == 0 {
		return nil, TargetNotFoundError(fmt.Sprintf("table %d", tableIndex))
	}
	return tables, nil
}

func extractDOCXTable(tbl *etree.Element) *model.Table {
	table := &model.Table{Rows: make([]model.TableRow, 0)}
	for _, tr := range namespaces.FindChildren(tbl, namespaces.NsW, "tr") {
		row := model.TableRow{Cells: make([]string, 0)}
		for _, tc := range namespaces.FindChildren(tr, namespaces.NsW, "tc") {
			var paragraphs []string
			for _, p := range namespaces.FindChildren(tc, namespaces.NsW, "p") {
				paragraphs = append(paragraphs, docxbody.ParagraphText(p))
			}
			row.Cells = append(row.Cells, strings.Join(paragraphs, "\n"))
		}
		table.Rows = append(table.Rows, row)
	}
	return table
}

func docxTableDimensions(table *model.Table) (int, int) {
	if table == nil {
		return 0, 0
	}
	cols := 0
	for _, row := range table.Rows {
		if len(row.Cells) > cols {
			cols = len(row.Cells)
		}
	}
	return len(table.Rows), cols
}

func tableRowsAsStrings(table *model.Table) [][]string {
	if table == nil {
		return nil
	}
	rows := make([][]string, 0, len(table.Rows))
	for _, row := range table.Rows {
		rows = append(rows, append([]string(nil), row.Cells...))
	}
	return rows
}

func docxTableHasMergedCells(table *etree.Element) bool {
	return len(namespaces.FindDescendants(table, namespaces.NsW, "gridSpan")) > 0 ||
		len(namespaces.FindDescendants(table, namespaces.NsW, "vMerge")) > 0
}

func resolveRequiredDOCXTableText(cmd *cobra.Command, textFlag, textFileFlag, textValue, textFileValue string) (string, error) {
	textChanged := cmd.Flags().Lookup(textFlag).Changed
	textFileChanged := cmd.Flags().Lookup(textFileFlag).Changed
	if textChanged == textFileChanged {
		return "", InvalidArgsError("must specify exactly one of --text or --text-file")
	}
	if textChanged {
		return textValue, nil
	}
	data, err := os.ReadFile(textFileValue)
	if err != nil {
		return "", FileNotFoundError(textFileValue)
	}
	return string(data), nil
}

func mapDOCXTableMutationError(target string, err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	switch {
	case errors.Is(err, docxmutate.ErrTableIndexOutOfRange),
		errors.Is(err, docxmutate.ErrTableCellOutOfRange):
		return TargetNotFoundError(target)
	case errors.Is(err, docxmutate.ErrBlockHashMismatch):
		return NewCLIErrorf(ExitInvalidArgs, "%v", err)
	case errors.Is(err, docxmutate.ErrTableHasMergedCells),
		errors.Is(err, docxmutate.ErrDeleteLastTableRow):
		return InvalidArgsError(err.Error())
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate table: %v", err)
	}
}

func outputDOCXTablesJSON(cmd *cobra.Command, value any, label string) error {
	return writeLabeledJSON(cmd, value, label)
}

func init() {
	docxCmd.AddCommand(docxTablesCmd)
}
