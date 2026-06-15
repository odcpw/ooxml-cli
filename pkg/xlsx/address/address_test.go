package address

import "testing"

func TestColumnLettersToIndex(t *testing.T) {
	tests := []struct {
		letters string
		want    int
	}{
		{"A", 1},
		{"Z", 26},
		{"AA", 27},
		{"AZ", 52},
		{"BA", 53},
		{"xfd", MaxColumn},
	}

	for _, tt := range tests {
		got, err := ColumnLettersToIndex(tt.letters)
		if err != nil {
			t.Fatalf("ColumnLettersToIndex(%q) returned error: %v", tt.letters, err)
		}
		if got != tt.want {
			t.Fatalf("ColumnLettersToIndex(%q) = %d, want %d", tt.letters, got, tt.want)
		}
	}
}

func TestColumnIndexToLetters(t *testing.T) {
	tests := []struct {
		index int
		want  string
	}{
		{1, "A"},
		{26, "Z"},
		{27, "AA"},
		{52, "AZ"},
		{53, "BA"},
		{MaxColumn, "XFD"},
	}

	for _, tt := range tests {
		got, err := ColumnIndexToLetters(tt.index)
		if err != nil {
			t.Fatalf("ColumnIndexToLetters(%d) returned error: %v", tt.index, err)
		}
		if got != tt.want {
			t.Fatalf("ColumnIndexToLetters(%d) = %q, want %q", tt.index, got, tt.want)
		}
	}
}

func TestColumnBounds(t *testing.T) {
	for _, letters := range []string{"", "A1", "XFE"} {
		if _, err := ColumnLettersToIndex(letters); err == nil {
			t.Fatalf("ColumnLettersToIndex(%q) expected error", letters)
		}
	}
	for _, index := range []int{0, -1, MaxColumn + 1} {
		if _, err := ColumnIndexToLetters(index); err == nil {
			t.Fatalf("ColumnIndexToLetters(%d) expected error", index)
		}
	}
}

func TestParseColumnAndNormalizeColumn(t *testing.T) {
	tests := []struct {
		input string
		index int
		text  string
	}{
		{" A ", 1, "A"},
		{"c", 3, "C"},
		{"xfd", MaxColumn, "XFD"},
	}

	for _, tt := range tests {
		got, err := ParseColumn(tt.input)
		if err != nil {
			t.Fatalf("ParseColumn(%q) returned error: %v", tt.input, err)
		}
		if got != tt.index {
			t.Fatalf("ParseColumn(%q) = %d, want %d", tt.input, got, tt.index)
		}
		text, err := NormalizeColumn(tt.input)
		if err != nil {
			t.Fatalf("NormalizeColumn(%q) returned error: %v", tt.input, err)
		}
		if text != tt.text {
			t.Fatalf("NormalizeColumn(%q) = %q, want %q", tt.input, text, tt.text)
		}
	}
}

func TestParseColumnInvalid(t *testing.T) {
	for _, input := range []string{"", "$A", "A1", "1", "XFE"} {
		if _, err := ParseColumn(input); err == nil {
			t.Fatalf("ParseColumn(%q) expected error", input)
		}
	}
}

func TestOffsetHelpers(t *testing.T) {
	if got, err := OffsetRow(2, -1); err != nil || got != 1 {
		t.Fatalf("OffsetRow(2, -1) = %d, %v; want 1, nil", got, err)
	}
	if got, err := OffsetColumn(26, 1); err != nil || got != 27 {
		t.Fatalf("OffsetColumn(26, 1) = %d, %v; want 27, nil", got, err)
	}
	ref, err := OffsetCell(CellRef{Column: 2, Row: 3}, 4, 5)
	if err != nil {
		t.Fatalf("OffsetCell returned error: %v", err)
	}
	if ref != (CellRef{Column: 7, Row: 7}) {
		t.Fatalf("OffsetCell = %#v, want G7 ref", ref)
	}

	for _, tt := range []struct {
		name string
		err  error
	}{
		{name: "row low", err: offsetRowError(1, -1)},
		{name: "row high", err: offsetRowError(MaxRow, 1)},
		{name: "column low", err: offsetColumnError(1, -1)},
		{name: "column high", err: offsetColumnError(MaxColumn, 1)},
	} {
		if tt.err == nil {
			t.Fatalf("%s expected error", tt.name)
		}
	}
}

func offsetRowError(row, delta int) error {
	_, err := OffsetRow(row, delta)
	return err
}

func offsetColumnError(column, delta int) error {
	_, err := OffsetColumn(column, delta)
	return err
}

func TestParseCell(t *testing.T) {
	tests := []struct {
		input string
		want  CellRef
		text  string
	}{
		{"a1", CellRef{Column: 1, Row: 1}, "A1"},
		{"$B$12", CellRef{Column: 2, Row: 12, AbsColumn: true, AbsRow: true}, "$B$12"},
		{"C$3", CellRef{Column: 3, Row: 3, AbsRow: true}, "C$3"},
		{"$d4", CellRef{Column: 4, Row: 4, AbsColumn: true}, "$D4"},
		{" XFD1048576 ", CellRef{Column: MaxColumn, Row: MaxRow}, "XFD1048576"},
	}

	for _, tt := range tests {
		got, err := ParseCell(tt.input)
		if err != nil {
			t.Fatalf("ParseCell(%q) returned error: %v", tt.input, err)
		}
		if got != tt.want {
			t.Fatalf("ParseCell(%q) = %#v, want %#v", tt.input, got, tt.want)
		}
		if got.String() != tt.text {
			t.Fatalf("ParseCell(%q).String() = %q, want %q", tt.input, got.String(), tt.text)
		}
	}
}

func TestParseCellInvalid(t *testing.T) {
	for _, input := range []string{"", "A", "1", "A0", "A1048577", "XFE1", "$$A1", "A$"} {
		if _, err := ParseCell(input); err == nil {
			t.Fatalf("ParseCell(%q) expected error", input)
		}
	}
}

func TestNormalizeCell(t *testing.T) {
	got, err := NormalizeCell(" $a$10 ")
	if err != nil {
		t.Fatalf("NormalizeCell returned error: %v", err)
	}
	if got != "$A$10" {
		t.Fatalf("NormalizeCell = %q, want %q", got, "$A$10")
	}
}

func TestParseRange(t *testing.T) {
	tests := []struct {
		input  string
		output string
		bounds [4]int
	}{
		{"a1:b2", "A1:B2", [4]int{1, 1, 2, 2}},
		{"$A$1:$b2", "$A$1:$B2", [4]int{1, 1, 2, 2}},
		{"C3", "C3", [4]int{3, 3, 3, 3}},
		{"B2:A1", "B2:A1", [4]int{1, 1, 2, 2}},
	}

	for _, tt := range tests {
		got, err := ParseRange(tt.input)
		if err != nil {
			t.Fatalf("ParseRange(%q) returned error: %v", tt.input, err)
		}
		if got.String() != tt.output {
			t.Fatalf("ParseRange(%q).String() = %q, want %q", tt.input, got.String(), tt.output)
		}
		minCol, minRow, maxCol, maxRow := got.Bounds()
		bounds := [4]int{minCol, minRow, maxCol, maxRow}
		if bounds != tt.bounds {
			t.Fatalf("ParseRange(%q).Bounds() = %v, want %v", tt.input, bounds, tt.bounds)
		}
	}
}

func TestParseRangeInvalid(t *testing.T) {
	for _, input := range []string{"", "A1:", ":B2", "A1:B2:C3", "A0:B2"} {
		if _, err := ParseRange(input); err == nil {
			t.Fatalf("ParseRange(%q) expected error", input)
		}
	}
}
