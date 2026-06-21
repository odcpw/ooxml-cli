package cli

import (
	"encoding/json"
	"fmt"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/spf13/cobra"
)

type XLSXTablesResult struct {
	File            string          `json:"file"`
	ValidateCommand string          `json:"validateCommand,omitempty"`
	Tables          []XLSXTableItem `json:"tables"`
}

func selectXLSXTable(tables []model.TableRef, selector string) (model.TableRef, error) {
	if len(tables) == 0 {
		return model.TableRef{}, NewCLIErrorf(ExitInvalidArgs, "workbook has no tables")
	}
	selector = strings.TrimSpace(selector)
	if selector == "" {
		if len(tables) == 1 {
			return tables[0], nil
		}
		return model.TableRef{}, InvalidArgsError("--table is required when workbook has multiple tables")
	}
	for _, tableRef := range tables {
		withSelectors := model.WithTableSelectors(tableRef)
		if model.SelectorMatches(withSelectors.Selectors, selector) {
			return withSelectors, nil
		}
	}
	if number, err := strconv.Atoi(selector); err == nil {
		if number < 1 || number > len(tables) {
			return model.TableRef{}, NewCLIErrorf(ExitTargetNotFound, "table %d is out of range (1-%d)", number, len(tables))
		}
		return model.WithTableSelectors(tables[number-1]), nil
	}
	candidates := tableSelectorCandidates(tables)
	return model.TableRef{}, SelectorNotFoundError("table", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), "ooxml --json xlsx tables list <file>")
}

func tableSelectorCandidates(tables []model.TableRef) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(tables))
	for _, table := range tables {
		withSelectors := model.WithTableSelectors(table)
		out = append(out, SelectorCandidate{Primary: withSelectors.PrimarySelector, Selectors: withSelectors.Selectors})
	}
	return out
}

func outputXLSXTablesJSON(cmd *cobra.Command, filePath string, tables []model.TableRef) error {
	config := GetGlobalConfig(cmd)
	items := make([]XLSXTableItem, 0, len(tables))
	for _, table := range tables {
		items = append(items, xlsxTableItem(filePath, table))
	}
	result := XLSXTablesResult{File: filePath, ValidateCommand: xlsxValidateCommand(filePath), Tables: items}
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal tables JSON: %v", err)
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXTablesText(cmd *cobra.Command, tables []model.TableRef) error {
	if len(tables) == 0 {
		return writeXLSXOutput(cmd, []byte("no tables found"))
	}

	out := ""
	for i, tableRef := range tables {
		if i > 0 {
			out += "\n"
		}
		out += fmt.Sprintf("[%d] %s\n", tableRef.Number, tableRef.DisplayName)
		out += fmt.Sprintf("  sheet: %s (%d)\n", tableRef.Sheet, tableRef.SheetNumber)
		out += fmt.Sprintf("  range: %s (%d rows x %d cols)\n", tableRef.Range, tableRef.Rows, tableRef.Cols)
		out += fmt.Sprintf("  dataRows: %d\n", tableRef.DataRowCount)
		out += fmt.Sprintf("  part: %s\n", tableRef.PartURI)
	}
	return writeXLSXOutput(cmd, []byte(out))
}
