// Package address parses and formats XLSX A1-style cell references.
package address

import (
	"fmt"
	"strconv"
	"strings"
	"unicode"
)

const (
	// MaxColumn is the largest column index supported by XLSX worksheets.
	MaxColumn = 16384
	// MaxRow is the largest row index supported by XLSX worksheets.
	MaxRow = 1048576
)

// CellRef is a parsed A1 cell reference. Column and Row are 1-based.
type CellRef struct {
	Column    int
	Row       int
	AbsColumn bool
	AbsRow    bool
}

// RangeRef is a parsed A1 range. A single-cell range has Start == End.
type RangeRef struct {
	Start CellRef
	End   CellRef
}

// ParseCell parses an A1-style cell reference such as A1, $B$2, or c$3.
func ParseCell(value string) (CellRef, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return CellRef{}, fmt.Errorf("cell reference cannot be empty")
	}

	var ref CellRef
	if value[0] == '$' {
		ref.AbsColumn = true
		value = value[1:]
		if value == "" {
			return CellRef{}, fmt.Errorf("missing column in cell reference")
		}
	}

	colEnd := 0
	for colEnd < len(value) && isASCIIAlpha(value[colEnd]) {
		colEnd++
	}
	if colEnd == 0 {
		return CellRef{}, fmt.Errorf("missing column in cell reference")
	}

	column, err := ColumnLettersToIndex(value[:colEnd])
	if err != nil {
		return CellRef{}, err
	}
	ref.Column = column
	value = value[colEnd:]

	if value == "" {
		return CellRef{}, fmt.Errorf("missing row in cell reference")
	}
	if value[0] == '$' {
		ref.AbsRow = true
		value = value[1:]
		if value == "" {
			return CellRef{}, fmt.Errorf("missing row in cell reference")
		}
	}
	if strings.Contains(value, "$") {
		return CellRef{}, fmt.Errorf("invalid absolute marker in row reference")
	}
	for _, r := range value {
		if !unicode.IsDigit(r) {
			return CellRef{}, fmt.Errorf("invalid row %q in cell reference", value)
		}
	}

	row, err := strconv.Atoi(value)
	if err != nil {
		return CellRef{}, fmt.Errorf("invalid row %q: %w", value, err)
	}
	if row < 1 || row > MaxRow {
		return CellRef{}, fmt.Errorf("row %d out of XLSX bounds 1-%d", row, MaxRow)
	}
	ref.Row = row

	return ref, nil
}

// NormalizeCell returns the canonical A1 representation of a cell reference.
func NormalizeCell(value string) (string, error) {
	ref, err := ParseCell(value)
	if err != nil {
		return "", err
	}
	return ref.String(), nil
}

// ParseColumn parses a column-only reference such as A, c, or XFD.
func ParseColumn(value string) (int, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return 0, fmt.Errorf("column reference cannot be empty")
	}
	for _, r := range value {
		if r == '$' || unicode.IsDigit(r) {
			return 0, fmt.Errorf("invalid column reference %q", value)
		}
	}
	return ColumnLettersToIndex(value)
}

// NormalizeColumn returns the canonical uppercase column letters.
func NormalizeColumn(value string) (string, error) {
	column, err := ParseColumn(value)
	if err != nil {
		return "", err
	}
	return ColumnIndexToLetters(column)
}

// ParseRange parses an A1-style range such as A1:B2. A single cell is accepted.
func ParseRange(value string) (RangeRef, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return RangeRef{}, fmt.Errorf("range reference cannot be empty")
	}
	parts := strings.Split(value, ":")
	if len(parts) > 2 {
		return RangeRef{}, fmt.Errorf("invalid range reference %q", value)
	}

	start, err := ParseCell(parts[0])
	if err != nil {
		return RangeRef{}, fmt.Errorf("invalid range start: %w", err)
	}
	end := start
	if len(parts) == 2 {
		if strings.TrimSpace(parts[1]) == "" {
			return RangeRef{}, fmt.Errorf("range end cannot be empty")
		}
		end, err = ParseCell(parts[1])
		if err != nil {
			return RangeRef{}, fmt.Errorf("invalid range end: %w", err)
		}
	}

	return RangeRef{Start: start, End: end}, nil
}

// NormalizeRange returns the canonical A1 representation of a range.
func NormalizeRange(value string) (string, error) {
	ref, err := ParseRange(value)
	if err != nil {
		return "", err
	}
	return ref.String(), nil
}

// ColumnLettersToIndex converts column letters such as A or XFD to a 1-based index.
func ColumnLettersToIndex(letters string) (int, error) {
	letters = strings.TrimSpace(letters)
	if letters == "" {
		return 0, fmt.Errorf("column letters cannot be empty")
	}

	index := 0
	for _, r := range letters {
		if r >= 'a' && r <= 'z' {
			r -= 'a' - 'A'
		}
		if r < 'A' || r > 'Z' {
			return 0, fmt.Errorf("invalid column letter %q", r)
		}
		index = index*26 + int(r-'A'+1)
		if index > MaxColumn {
			return 0, fmt.Errorf("column %q out of XLSX bounds A-XFD", letters)
		}
	}
	return index, nil
}

// ColumnIndexToLetters converts a 1-based column index to letters.
func ColumnIndexToLetters(index int) (string, error) {
	if index < 1 || index > MaxColumn {
		return "", fmt.Errorf("column index %d out of XLSX bounds 1-%d", index, MaxColumn)
	}

	var buf []byte
	for index > 0 {
		index--
		buf = append(buf, byte('A'+index%26))
		index /= 26
	}
	for i, j := 0, len(buf)-1; i < j; i, j = i+1, j-1 {
		buf[i], buf[j] = buf[j], buf[i]
	}
	return string(buf), nil
}

// OffsetRow returns row + delta after validating XLSX row bounds.
func OffsetRow(row, delta int) (int, error) {
	shifted := int64(row) + int64(delta)
	if shifted < 1 || shifted > MaxRow {
		return 0, fmt.Errorf("row %d offset by %d is out of XLSX bounds 1-%d", row, delta, MaxRow)
	}
	return int(shifted), nil
}

// OffsetColumn returns column + delta after validating XLSX column bounds.
func OffsetColumn(column, delta int) (int, error) {
	shifted := int64(column) + int64(delta)
	if shifted < 1 || shifted > MaxColumn {
		return 0, fmt.Errorf("column %d offset by %d is out of XLSX bounds 1-%d", column, delta, MaxColumn)
	}
	return int(shifted), nil
}

// OffsetCell returns a cell reference shifted by row and column deltas.
func OffsetCell(ref CellRef, rowDelta, columnDelta int) (CellRef, error) {
	row, err := OffsetRow(ref.Row, rowDelta)
	if err != nil {
		return CellRef{}, err
	}
	column, err := OffsetColumn(ref.Column, columnDelta)
	if err != nil {
		return CellRef{}, err
	}
	ref.Row = row
	ref.Column = column
	return ref, nil
}

// Bounds returns the normalized rectangular bounds for the range.
func (r RangeRef) Bounds() (minColumn, minRow, maxColumn, maxRow int) {
	minColumn, maxColumn = ordered(r.Start.Column, r.End.Column)
	minRow, maxRow = ordered(r.Start.Row, r.End.Row)
	return minColumn, minRow, maxColumn, maxRow
}

func (c CellRef) String() string {
	column, err := ColumnIndexToLetters(c.Column)
	if err != nil {
		return ""
	}

	var b strings.Builder
	if c.AbsColumn {
		b.WriteByte('$')
	}
	b.WriteString(column)
	if c.AbsRow {
		b.WriteByte('$')
	}
	b.WriteString(strconv.Itoa(c.Row))
	return b.String()
}

func (r RangeRef) String() string {
	if r.Start == r.End {
		return r.Start.String()
	}
	return r.Start.String() + ":" + r.End.String()
}

func isASCIIAlpha(b byte) bool {
	return (b >= 'A' && b <= 'Z') || (b >= 'a' && b <= 'z')
}

func ordered(a, b int) (int, int) {
	if a <= b {
		return a, b
	}
	return b, a
}
