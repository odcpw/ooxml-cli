package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
	"github.com/spf13/cobra"
)

var xlsxRangesCmd = &cobra.Command{
	Use:     "ranges",
	Aliases: []string{"range"},
	Short:   "Export and set rectangular worksheet ranges",
	Long:    "Commands for reading and mutating rectangular worksheet ranges as JSON, CSV, or TSV matrices.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

func requireXLSXRangeSheet(sheet string) error {
	if sheet == "" {
		return InvalidArgsError("--sheet is required for range commands")
	}
	return nil
}

func xlsxRangeDimensions(ref address.RangeRef) (rows, cols int) {
	minCol, minRow, maxCol, maxRow := ref.Bounds()
	return maxRow - minRow + 1, maxCol - minCol + 1
}

func xlsxRangeCellCount(ref address.RangeRef) int64 {
	rows, cols := xlsxRangeDimensions(ref)
	return int64(rows) * int64(cols)
}

func checkXLSXRangeMaxCells(ref address.RangeRef, maxCells int) error {
	if maxCells < 0 {
		return InvalidArgsError("--max-cells must be >= 0")
	}
	if maxCells > 0 && xlsxRangeCellCount(ref) > int64(maxCells) {
		return NewCLIErrorf(ExitInvalidArgs, "range %s contains %d cells, above --max-cells %d", ref.String(), xlsxRangeCellCount(ref), maxCells)
	}
	return nil
}

func xlsxRangeFromAnchor(anchor address.CellRef, rows, cols int) (address.RangeRef, error) {
	if rows < 1 || cols < 1 {
		return address.RangeRef{}, fmt.Errorf("range dimensions must be positive")
	}
	end, err := address.OffsetCell(anchor, rows-1, cols-1)
	if err != nil {
		return address.RangeRef{}, err
	}
	return address.RangeRef{Start: anchor, End: end}, nil
}

func xlsxRangeCellsFromRows(rows []model.Row) [][]rangeio.Cell {
	matrix := make([][]rangeio.Cell, len(rows))
	for rowIdx, row := range rows {
		matrix[rowIdx] = make([]rangeio.Cell, len(row.Cells))
		for colIdx, cell := range row.Cells {
			matrix[rowIdx][colIdx] = xlsxRangeCellFromModel(cell)
		}
	}
	return matrix
}

func xlsxRangeStyleIndexesFromRows(rows []model.Row) [][]any {
	return xlsxRangeFormatMatrixFromRows(rows, func(cell model.Cell) (any, bool) {
		if !cellHasFormatReadback(cell) {
			return nil, false
		}
		return cell.StyleIndex, true
	})
}

func xlsxRangeNumberFormatIDsFromRows(rows []model.Row) [][]any {
	return xlsxRangeFormatMatrixFromRows(rows, func(cell model.Cell) (any, bool) {
		if !cellHasFormatReadback(cell) {
			return nil, false
		}
		return cell.NumberFormatID, true
	})
}

func xlsxRangeNumberFormatCodesFromRows(rows []model.Row) [][]any {
	return xlsxRangeFormatMatrixFromRows(rows, func(cell model.Cell) (any, bool) {
		if !cellHasFormatReadback(cell) || cell.NumberFormatCode == "" {
			return nil, false
		}
		return cell.NumberFormatCode, true
	})
}

func xlsxRangeFormatMatrixFromRows(rows []model.Row, value func(model.Cell) (any, bool)) [][]any {
	matrix := make([][]any, len(rows))
	hasValue := false
	for rowIdx, row := range rows {
		matrix[rowIdx] = make([]any, len(row.Cells))
		for colIdx, cell := range row.Cells {
			if item, ok := value(cell); ok {
				matrix[rowIdx][colIdx] = item
				hasValue = true
			}
		}
	}
	if !hasValue {
		return nil
	}
	return matrix
}

func cellHasFormatReadback(cell model.Cell) bool {
	return cell.StyleIndex != 0 || cell.NumberFormatID != 0 || cell.NumberFormatCode != ""
}

func xlsxRangeCellFromModel(cell model.Cell) rangeio.Cell {
	if cell.Type == model.CellTypeEmpty && cell.Value == "" && cell.Formula == "" {
		return rangeio.Cell{Type: string(model.CellTypeEmpty), Null: true}
	}
	return rangeio.Cell{
		Type:    string(cell.Type),
		Value:   cell.Value,
		Formula: cell.Formula,
	}
}

func xlsxRangeCellsToMutate(rows [][]rangeio.Cell) ([][]mutate.RangeCell, error) {
	out := make([][]mutate.RangeCell, len(rows))
	for rowIdx, row := range rows {
		out[rowIdx] = make([]mutate.RangeCell, len(row))
		for colIdx, cell := range row {
			if cell.Null {
				out[rowIdx][colIdx] = mutate.RangeCell{Null: true}
				continue
			}
			typ, err := normalizeXLSXCellValueType(cell.Type)
			if err != nil {
				return nil, fmt.Errorf("values[%d][%d]: %w", rowIdx, colIdx, err)
			}
			value := cell.Value
			if typ == mutate.CellValueFormula && cell.Formula != "" {
				value = cell.Formula
			}
			out[rowIdx][colIdx] = mutate.RangeCell{
				Type:  typ,
				Value: value,
			}
		}
	}
	return out, nil
}

func init() {
	xlsxCmd.AddCommand(xlsxRangesCmd)
}
