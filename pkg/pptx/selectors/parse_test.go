package selectors

import (
	"fmt"
	"testing"
)

func TestParsePlaceholderKey(t *testing.T) {
	tests := []struct {
		input    string
		expected *PlaceholderKeySelector
		wantErr  bool
	}{
		{
			input:    "title",
			expected: &PlaceholderKeySelector{Key: "title"},
			wantErr:  false,
		},
		{
			input:    "body:0",
			expected: &PlaceholderKeySelector{Key: "body:0"},
			wantErr:  false,
		},
		{
			input:    "pic:1",
			expected: &PlaceholderKeySelector{Key: "pic:1"},
			wantErr:  false,
		},
		{
			input:    "subtitle",
			expected: &PlaceholderKeySelector{Key: "subtitle"},
			wantErr:  false,
		},
		{
			input:    "shape:123", // This is a special case - gets parsed as shape ID, not key
			expected: nil,
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr && tt.expected != nil {
				key, ok := result.(*PlaceholderKeySelector)
				if !ok {
					t.Errorf("expected PlaceholderKeySelector, got %T", result)
					return
				}
				if key.Key != tt.expected.Key {
					t.Errorf("expected key %q, got %q", tt.expected.Key, key.Key)
				}
			}
		})
	}
}

func TestParsePlaceholderType(t *testing.T) {
	tests := []struct {
		input    string
		expected string
		wantErr  bool
	}{
		{
			input:    "@title",
			expected: "title",
			wantErr:  false,
		},
		{
			input:    "@body",
			expected: "body",
			wantErr:  false,
		},
		{
			input:    "@pic",
			expected: "pic",
			wantErr:  false,
		},
		{
			input:    "@chart",
			expected: "chart",
			wantErr:  false,
		},
		{
			input:    "@",
			expected: "",
			wantErr:  true,
		},
		{
			input:    "@ title", // with space after @
			expected: "title",
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				phType, ok := result.(*PlaceholderTypeSelector)
				if !ok {
					t.Errorf("expected PlaceholderTypeSelector, got %T", result)
					return
				}
				if phType.Role != tt.expected {
					t.Errorf("expected type %q, got %q", tt.expected, phType.Role)
				}
			}
		})
	}
}

func TestParsePlaceholderIndex(t *testing.T) {
	tests := []struct {
		input    string
		expected int
		wantErr  bool
	}{
		{
			input:    "#0",
			expected: 0,
			wantErr:  false,
		},
		{
			input:    "#1",
			expected: 1,
			wantErr:  false,
		},
		{
			input:    "#3",
			expected: 3,
			wantErr:  false,
		},
		{
			input:    "#12",
			expected: 12,
			wantErr:  false,
		},
		{
			input:    "#",
			expected: 0,
			wantErr:  true,
		},
		{
			input:    "#-1",
			expected: 0,
			wantErr:  true,
		},
		{
			input:    "#abc",
			expected: 0,
			wantErr:  true,
		},
		{
			input:    "# 5", // with space after #
			expected: 5,
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				phIdx, ok := result.(*PlaceholderIndexSelector)
				if !ok {
					t.Errorf("expected PlaceholderIndexSelector, got %T", result)
					return
				}
				if phIdx.Index != tt.expected {
					t.Errorf("expected index %d, got %d", tt.expected, phIdx.Index)
				}
			}
		})
	}
}

func TestParseShapeName(t *testing.T) {
	tests := []struct {
		input    string
		expected string
		wantErr  bool
	}{
		{
			input:    "~MyShape",
			expected: "MyShape",
			wantErr:  false,
		},
		{
			input:    "~Shape With Spaces",
			expected: "Shape With Spaces",
			wantErr:  false,
		},
		{
			input:    "~A",
			expected: "A",
			wantErr:  false,
		},
		{
			input:    "~",
			expected: "",
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				shapeName, ok := result.(*ShapeNameSelector)
				if !ok {
					t.Errorf("expected ShapeNameSelector, got %T", result)
					return
				}
				if shapeName.Name != tt.expected {
					t.Errorf("expected name %q, got %q", tt.expected, shapeName.Name)
				}
			}
		})
	}
}

func TestParseShapeID(t *testing.T) {
	tests := []struct {
		input    string
		expected int
		wantErr  bool
	}{
		{
			input:    "shape:5",
			expected: 5,
			wantErr:  false,
		},
		{
			input:    "shape:123",
			expected: 123,
			wantErr:  false,
		},
		{
			input:    "shape:0",
			expected: 0,
			wantErr:  false,
		},
		{
			input:    "shape:",
			expected: 0,
			wantErr:  true,
		},
		{
			input:    "shape:-1",
			expected: 0,
			wantErr:  true,
		},
		{
			input:    "shape:abc",
			expected: 0,
			wantErr:  true,
		},
		{
			input:    "shape: 42", // with space after :
			expected: 42,
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				shapeID, ok := result.(*ShapeIDSelector)
				if !ok {
					t.Errorf("expected ShapeIDSelector, got %T", result)
					return
				}
				if shapeID.ID != tt.expected {
					t.Errorf("expected ID %d, got %d", tt.expected, shapeID.ID)
				}
			}
		})
	}
}

func TestParseSlideNumber(t *testing.T) {
	tests := []struct {
		input    string
		expected int
		wantErr  bool
	}{
		{
			input:    "1",
			expected: 1,
			wantErr:  false,
		},
		{
			input:    "5",
			expected: 5,
			wantErr:  false,
		},
		{
			input:    "42",
			expected: 42,
			wantErr:  false,
		},
		{
			input:    "0",
			expected: 0,
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				slideNum, ok := result.(*SlideNumberSelector)
				if !ok {
					t.Errorf("expected SlideNumberSelector, got %T", result)
					return
				}
				if slideNum.Number != tt.expected {
					t.Errorf("expected number %d, got %d", tt.expected, slideNum.Number)
				}
			}
		})
	}
}

func TestParseSlideRange(t *testing.T) {
	tests := []struct {
		input    string
		expected []SlideRange
		wantErr  bool
	}{
		{
			input:    "1-3",
			expected: []SlideRange{{Start: 1, End: 3}},
			wantErr:  false,
		},
		{
			input:    "1-5",
			expected: []SlideRange{{Start: 1, End: 5}},
			wantErr:  false,
		},
		{
			input:    "5-10",
			expected: []SlideRange{{Start: 5, End: 10}},
			wantErr:  false,
		},
		{
			input:    "1,3,5",
			expected: []SlideRange{{Start: 1, End: 1}, {Start: 3, End: 3}, {Start: 5, End: 5}},
			wantErr:  false,
		},
		{
			input:    "1-3,5-7",
			expected: []SlideRange{{Start: 1, End: 3}, {Start: 5, End: 7}},
			wantErr:  false,
		},
		{
			input:    "1,3-5,7",
			expected: []SlideRange{{Start: 1, End: 1}, {Start: 3, End: 5}, {Start: 7, End: 7}},
			wantErr:  false,
		},
		{
			input:    "3-1",
			expected: nil,
			wantErr:  true,
		},
		{
			input:    "1-",
			expected: nil,
			wantErr:  true,
		},
		{
			input:    "1-2-3",
			expected: nil,
			wantErr:  true,
		},
		{
			input:    "0-5",
			expected: nil,
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				slideRange, ok := result.(*SlideRangeSelector)
				if !ok {
					t.Errorf("expected SlideRangeSelector, got %T", result)
					return
				}

				if len(slideRange.Ranges) != len(tt.expected) {
					t.Errorf("expected %d ranges, got %d", len(tt.expected), len(slideRange.Ranges))
					return
				}

				for i, r := range slideRange.Ranges {
					if r.Start != tt.expected[i].Start || r.End != tt.expected[i].End {
						t.Errorf("range %d: expected %v, got %v", i, tt.expected[i], r)
					}
				}
			}
		})
	}
}

func TestParseEmptyInput(t *testing.T) {
	_, err := Parse("")
	if err == nil {
		t.Error("Parse() should error on empty input")
	}
}

func TestParseStringRepresentation(t *testing.T) {
	tests := []struct {
		input    string
		selector Selector
		expected string
	}{
		{
			input:    "title",
			selector: &PlaceholderKeySelector{Key: "title"},
			expected: "title",
		},
		{
			input:    "@body",
			selector: &PlaceholderTypeSelector{Role: "body"},
			expected: "@body",
		},
		{
			input:    "#5",
			selector: &PlaceholderIndexSelector{Index: 5},
			expected: "#5",
		},
		{
			input:    "~MyShape",
			selector: &ShapeNameSelector{Name: "MyShape"},
			expected: "~MyShape",
		},
		{
			input:    "shape:42",
			selector: &ShapeIDSelector{ID: 42},
			expected: "shape:42",
		},
		{
			input:    "3",
			selector: &SlideNumberSelector{Number: 3},
			expected: "3",
		},
		{
			input:    "1-3",
			selector: &SlideRangeSelector{Ranges: []SlideRange{{Start: 1, End: 3}}},
			expected: "1-3",
		},
		{
			input:    "1,3,5",
			selector: &SlideRangeSelector{Ranges: []SlideRange{{Start: 1, End: 1}, {Start: 3, End: 3}, {Start: 5, End: 5}}},
			expected: "1,3,5",
		},
	}

	for _, tt := range tests {
		t.Run(fmt.Sprintf("%T", tt.selector), func(t *testing.T) {
			result := tt.selector.String()
			if result != tt.expected {
				t.Errorf("String() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestParseRoundtrip(t *testing.T) {
	tests := []string{
		"title",
		"body:0",
		"body:1",
		"pic:3",
		"@title",
		"@body",
		"@chart",
		"#0",
		"#5",
		"#12",
		"~MyShape",
		"shape:5",
		"1",
		"5",
		"1-3",
		"1-5",
		"1,3,5",
		"1-3,5-7",
	}

	for _, input := range tests {
		t.Run(input, func(t *testing.T) {
			parsed, err := Parse(input)
			if err != nil {
				t.Fatalf("Parse() failed: %v", err)
			}

			result := parsed.String()
			if result != input {
				t.Errorf("roundtrip failed: input %q, parsed.String() %q", input, result)
			}
		})
	}
}

func TestParseWithWhitespace(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"  title  ", "title"},
		{"  @body  ", "@body"},
		{"  #5  ", "#5"},
		{"  shape:10  ", "shape:10"},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			parsed, err := Parse(tt.input)
			if err != nil {
				t.Fatalf("Parse() failed: %v", err)
			}

			result := parsed.String()
			if result != tt.expected {
				t.Errorf("Parse() whitespace handling: expected %q, got %q", tt.expected, result)
			}
		})
	}
}

func TestParseWildcardSelectors(t *testing.T) {
	tests := []struct {
		input    string
		expected Selector
		wantErr  bool
	}{
		{
			input:    "@*",
			expected: &WildcardAllPlaceholdersSelector{},
			wantErr:  false,
		},
		{
			input:    "@all-placeholders",
			expected: &WildcardAllPlaceholdersSelector{},
			wantErr:  false,
		},
		{
			input:    "@all-shapes",
			expected: &WildcardAllShapesSelector{ExcludePlaceholders: false},
			wantErr:  false,
		},
		{
			input:    "@all-shapes-nonph",
			expected: &WildcardAllShapesSelector{ExcludePlaceholders: true},
			wantErr:  false,
		},
		{
			input:    "@all-pictures",
			expected: &WildcardAllPicturesSelector{},
			wantErr:  false,
		},
		{
			input:    "@all-tables",
			expected: &WildcardAllTablesSelector{},
			wantErr:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			result, err := Parse(tt.input)

			if (err != nil) != tt.wantErr {
				t.Errorf("Parse() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				if result.Type() != tt.expected.Type() {
					t.Errorf("expected type %v, got %v", tt.expected.Type(), result.Type())
					return
				}

				// Check specific types if needed
				switch expected := tt.expected.(type) {
				case *WildcardAllShapesSelector:
					actual := result.(*WildcardAllShapesSelector)
					if actual.ExcludePlaceholders != expected.ExcludePlaceholders {
						t.Errorf("ExcludePlaceholders mismatch: expected %v, got %v", expected.ExcludePlaceholders, actual.ExcludePlaceholders)
					}
				}
			}
		})
	}
}

func TestParseWildcardRoundtrip(t *testing.T) {
	tests := []string{
		"@*",
		"@all-placeholders",
		"@all-shapes",
		"@all-shapes-nonph",
		"@all-pictures",
		"@all-tables",
	}

	for _, input := range tests {
		t.Run(input, func(t *testing.T) {
			parsed, err := Parse(input)
			if err != nil {
				t.Fatalf("Parse() failed: %v", err)
			}

			result := parsed.String()
			if result != input {
				t.Errorf("roundtrip failed: input %q, parsed.String() %q", input, result)
			}
		})
	}
}
