// Package sheet reads XLSX worksheet cells into stable inspection models.
package sheet

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sst"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/styles"
)

// DefaultDenseCellLimit caps dense include-empty output when callers do not set MaxCells.
const DefaultDenseCellLimit = 10000

// Context contains workbook-wide parts needed to decode worksheet cells.
type Context struct {
	SharedStrings *sst.Table
	Styles        *styles.Styles
}

// ReadOptions controls worksheet inspection and optional row data extraction.
type ReadOptions struct {
	Range        *address.RangeRef
	MaxRows      int
	MaxCells     int
	IncludeEmpty bool
	IncludeData  bool
}

// LoadContext loads workbook-wide shared strings and styles when present.
func LoadContext(session opc.PackageSession, workbook *model.Workbook) (*Context, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if workbook == nil {
		return nil, fmt.Errorf("workbook is nil")
	}

	ctx := &Context{}
	if workbook.SharedStringsURI != "" {
		data, err := session.ReadRawPart(workbook.SharedStringsURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read shared strings %s: %w", workbook.SharedStringsURI, err)
		}
		table, err := sst.ParseBytes(data)
		if err != nil {
			return nil, err
		}
		ctx.SharedStrings = table
	}
	if workbook.StylesURI != "" {
		data, err := session.ReadRawPart(workbook.StylesURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read styles %s: %w", workbook.StylesURI, err)
		}
		parsed, err := styles.ParseBytes(data)
		if err != nil {
			return nil, err
		}
		ctx.Styles = parsed
	}
	return ctx, nil
}

// Read parses a worksheet and returns sheet metadata plus optional decoded rows.
func Read(session opc.PackageSession, ref model.SheetRef, ctx *Context, opts ReadOptions) (*model.SheetReport, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	ref = model.WithSheetSelectors(ref)
	if ref.PartURI == "" {
		return nil, fmt.Errorf("sheet %q has no worksheet part URI", ref.Name)
	}
	if ctx == nil {
		ctx = &Context{}
	}

	doc, err := session.ReadXMLPart(ref.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", ref.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", ref.PartURI)
	}

	report := &model.SheetReport{
		Number:          ref.Number,
		Name:            ref.Name,
		SheetID:         ref.SheetID,
		State:           ref.State,
		PartURI:         ref.PartURI,
		PrimarySelector: ref.PrimarySelector,
		Selectors:       append([]string{}, ref.Selectors...),
	}
	if dim := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dimension"); dim != nil {
		report.DimensionDeclared = dim.SelectAttrValue("ref", "")
	}
	report.MergedCellCount = countMergedCells(root)

	cells, err := parseCells(root, ctx, opts.Range)
	if err != nil {
		return nil, err
	}
	report.CellCount = len(cells)
	report.RowCount = countRows(cells)
	report.UsedRange = usedRange(cells)

	if opts.IncludeData {
		report.Rows, report.Truncated = buildRows(cells, opts, report.UsedRange)
	}

	return report, nil
}

func parseCells(root *etree.Element, ctx *Context, rangeRef *address.RangeRef) ([]model.Cell, error) {
	sheetData := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData")
	if sheetData == nil {
		return nil, nil
	}

	var cells []model.Cell
	for _, rowElem := range namespaces.FindChildren(sheetData, namespaces.NsSpreadsheetML, "row") {
		rowText := rowElem.SelectAttrValue("r", "")
		if rowText == "" && len(namespaces.FindChildren(rowElem, namespaces.NsSpreadsheetML, "c")) > 0 {
			return nil, fmt.Errorf("worksheet row is missing r attribute")
		}
		if rowText != "" {
			if _, err := strconv.Atoi(rowText); err != nil {
				return nil, fmt.Errorf("invalid row r attribute %q: %w", rowText, err)
			}
		}

		for _, cellElem := range namespaces.FindChildren(rowElem, namespaces.NsSpreadsheetML, "c") {
			refText := cellElem.SelectAttrValue("r", "")
			if refText == "" {
				return nil, fmt.Errorf("worksheet cell is missing r attribute")
			}
			ref, err := address.ParseCell(refText)
			if err != nil {
				return nil, fmt.Errorf("invalid cell reference %q: %w", refText, err)
			}
			if rangeRef != nil && !rangeContains(*rangeRef, ref) {
				continue
			}
			cell, err := decodeCell(cellElem, ref, ctx)
			if err != nil {
				return nil, fmt.Errorf("failed to decode cell %s: %w", ref.String(), err)
			}
			cells = append(cells, cell)
		}
	}

	sort.Slice(cells, func(i, j int) bool {
		if cells[i].Row != cells[j].Row {
			return cells[i].Row < cells[j].Row
		}
		return cells[i].Col < cells[j].Col
	})
	return cells, nil
}

func decodeCell(elem *etree.Element, ref address.CellRef, ctx *Context) (model.Cell, error) {
	column, err := address.ColumnIndexToLetters(ref.Column)
	if err != nil {
		return model.Cell{}, err
	}

	cell := model.Cell{
		Ref:    ref.String(),
		Row:    ref.Row,
		Col:    ref.Column,
		Column: column,
		Type:   model.CellTypeEmpty,
	}

	styleIndex, hasStyle, err := parseStyleIndex(elem)
	if err != nil {
		return model.Cell{}, err
	}
	if hasStyle {
		cell.StyleIndex = styleIndex
		if ctx.Styles != nil {
			if numFmtID, formatCode, ok := ctx.Styles.NumberFormat(styleIndex); ok {
				cell.NumberFormatID = numFmtID
				cell.NumberFormatCode = formatCode
			}
			cell.DateStyle = ctx.Styles.IsDateStyle(styleIndex)
		}
	}

	cell.Formula = childText(elem, "f")
	raw := childText(elem, "v")
	cell.RawValue = raw

	switch elem.SelectAttrValue("t", "") {
	case "s":
		if raw == "" {
			cell.Type = model.CellTypeString
			return cell, nil
		}
		idx, err := strconv.Atoi(raw)
		if err != nil {
			return model.Cell{}, fmt.Errorf("invalid shared string index %q: %w", raw, err)
		}
		if ctx.SharedStrings == nil {
			return model.Cell{}, fmt.Errorf("shared string table is missing")
		}
		text, ok := ctx.SharedStrings.Text(idx)
		if !ok {
			return model.Cell{}, fmt.Errorf("shared string index %d out of range", idx)
		}
		cell.Type = model.CellTypeString
		cell.Value = text
	case "inlineStr":
		cell.Type = model.CellTypeString
		cell.Value = inlineStringText(elem)
	case "str":
		cell.Type = model.CellTypeString
		cell.Value = raw
	case "b":
		cell.Type = model.CellTypeBoolean
		cell.Value = booleanText(raw)
	case "e":
		cell.Type = model.CellTypeError
		cell.Value = raw
	case "d":
		cell.Type = model.CellTypeDate
		cell.Value = raw
	case "":
		if raw == "" && cell.Formula == "" {
			cell.Type = model.CellTypeEmpty
			return cell, nil
		}
		if cell.DateStyle {
			cell.Type = model.CellTypeDate
		} else {
			cell.Type = model.CellTypeNumber
		}
		cell.Value = raw
	default:
		cell.Type = model.CellTypeUnknown
		cell.Value = raw
	}

	return cell, nil
}

func parseStyleIndex(elem *etree.Element) (int, bool, error) {
	text := elem.SelectAttrValue("s", "")
	if text == "" {
		return 0, false, nil
	}
	styleIndex, err := strconv.Atoi(text)
	if err != nil {
		return 0, false, fmt.Errorf("invalid style index %q: %w", text, err)
	}
	if styleIndex < 0 {
		return 0, false, fmt.Errorf("style index %d cannot be negative", styleIndex)
	}
	return styleIndex, true, nil
}

func childText(elem *etree.Element, localName string) string {
	child := namespaces.FindChild(elem, namespaces.NsSpreadsheetML, localName)
	if child == nil {
		return ""
	}
	return child.Text()
}

func inlineStringText(elem *etree.Element) string {
	inline := namespaces.FindChild(elem, namespaces.NsSpreadsheetML, "is")
	if inline == nil {
		return ""
	}
	var builder strings.Builder
	for _, textElem := range namespaces.FindDescendants(inline, namespaces.NsSpreadsheetML, "t") {
		builder.WriteString(textElem.Text())
	}
	return builder.String()
}

func booleanText(raw string) string {
	switch strings.TrimSpace(raw) {
	case "1":
		return "true"
	case "0":
		return "false"
	default:
		return raw
	}
}

func countMergedCells(root *etree.Element) int {
	mergeCells := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "mergeCells")
	if mergeCells == nil {
		return 0
	}
	return len(namespaces.FindChildren(mergeCells, namespaces.NsSpreadsheetML, "mergeCell"))
}

func countRows(cells []model.Cell) int {
	rows := make(map[int]struct{})
	for _, cell := range cells {
		rows[cell.Row] = struct{}{}
	}
	return len(rows)
}

func usedRange(cells []model.Cell) model.UsedRange {
	if len(cells) == 0 {
		return model.UsedRange{Empty: true}
	}

	minRow, maxRow := cells[0].Row, cells[0].Row
	minCol, maxCol := cells[0].Col, cells[0].Col
	for _, cell := range cells[1:] {
		if cell.Row < minRow {
			minRow = cell.Row
		}
		if cell.Row > maxRow {
			maxRow = cell.Row
		}
		if cell.Col < minCol {
			minCol = cell.Col
		}
		if cell.Col > maxCol {
			maxCol = cell.Col
		}
	}

	startCol, _ := address.ColumnIndexToLetters(minCol)
	endCol, _ := address.ColumnIndexToLetters(maxCol)
	return model.UsedRange{
		Ref:    fmt.Sprintf("%s%d:%s%d", startCol, minRow, endCol, maxRow),
		MinRow: minRow,
		MaxRow: maxRow,
		MinCol: minCol,
		MaxCol: maxCol,
		Rows:   maxRow - minRow + 1,
		Cols:   maxCol - minCol + 1,
	}
}

func buildRows(cells []model.Cell, opts ReadOptions, used model.UsedRange) ([]model.Row, bool) {
	if opts.IncludeEmpty {
		return buildDenseRows(cells, opts, used)
	}
	return buildSparseRows(cells, opts)
}

func buildSparseRows(cells []model.Cell, opts ReadOptions) ([]model.Row, bool) {
	var rows []model.Row
	truncated := false
	emittedCells := 0
	rowIndex := map[int]int{}

	for _, cell := range cells {
		if opts.MaxCells > 0 && emittedCells >= opts.MaxCells {
			truncated = true
			break
		}
		idx, ok := rowIndex[cell.Row]
		if !ok {
			if opts.MaxRows > 0 && len(rows) >= opts.MaxRows {
				truncated = true
				break
			}
			rows = append(rows, model.Row{Number: cell.Row})
			idx = len(rows) - 1
			rowIndex[cell.Row] = idx
		}
		rows[idx].Cells = append(rows[idx].Cells, cell)
		emittedCells++
	}

	return rows, truncated
}

func buildDenseRows(cells []model.Cell, opts ReadOptions, used model.UsedRange) ([]model.Row, bool) {
	minCol, minRow, maxCol, maxRow, ok := outputBounds(opts.Range, used)
	if !ok {
		return nil, false
	}
	maxCells := opts.MaxCells
	if maxCells == 0 {
		maxCells = DefaultDenseCellLimit
	}

	byRef := make(map[string]model.Cell, len(cells))
	for _, cell := range cells {
		byRef[cell.Ref] = cell
	}

	var rows []model.Row
	truncated := false
	emittedCells := 0
	for row := minRow; row <= maxRow; row++ {
		if opts.MaxRows > 0 && len(rows) >= opts.MaxRows {
			truncated = true
			break
		}
		outRow := model.Row{Number: row}
		for col := minCol; col <= maxCol; col++ {
			if maxCells > 0 && emittedCells >= maxCells {
				truncated = true
				break
			}
			ref := cellRef(col, row)
			cell, ok := byRef[ref]
			if !ok {
				column, _ := address.ColumnIndexToLetters(col)
				cell = model.Cell{
					Ref:    ref,
					Row:    row,
					Col:    col,
					Column: column,
					Type:   model.CellTypeEmpty,
				}
			}
			outRow.Cells = append(outRow.Cells, cell)
			emittedCells++
		}
		rows = append(rows, outRow)
		if truncated {
			break
		}
	}
	return rows, truncated
}

func outputBounds(rangeRef *address.RangeRef, used model.UsedRange) (minCol, minRow, maxCol, maxRow int, ok bool) {
	if rangeRef != nil {
		minCol, minRow, maxCol, maxRow = rangeRef.Bounds()
		return minCol, minRow, maxCol, maxRow, true
	}
	if used.Empty {
		return 0, 0, 0, 0, false
	}
	return used.MinCol, used.MinRow, used.MaxCol, used.MaxRow, true
}

func rangeContains(rangeRef address.RangeRef, cell address.CellRef) bool {
	minCol, minRow, maxCol, maxRow := rangeRef.Bounds()
	return cell.Column >= minCol && cell.Column <= maxCol && cell.Row >= minRow && cell.Row <= maxRow
}

func cellRef(col, row int) string {
	column, _ := address.ColumnIndexToLetters(col)
	return fmt.Sprintf("%s%d", column, row)
}
