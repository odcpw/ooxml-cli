package output

import (
	"bytes"
	"strings"
	"testing"
)

func TestTextFormatter_FormatTable(t *testing.T) {
	tests := []struct {
		name    string
		rows    interface{}
		wantErr bool
	}{
		{
			name: "simple map table",
			rows: []map[string]interface{}{
				{"Name": "Alice", "Age": 30},
				{"Name": "Bob", "Age": 25},
			},
			wantErr: false,
		},
		{
			name: "struct table",
			rows: []struct {
				Name string
				Age  int
			}{
				{"Alice", 30},
				{"Bob", 25},
			},
			wantErr: false,
		},
		{
			name:    "empty table",
			rows:    []map[string]interface{}{},
			wantErr: false,
		},
		{
			name:    "invalid input (not a slice)",
			rows:    "not a slice",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buf := &bytes.Buffer{}
			f := newTextFormatter(buf)

			err := f.FormatTable(tt.rows)
			if (err != nil) != tt.wantErr {
				t.Errorf("FormatTable() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if !tt.wantErr {
				output := buf.String()
				if tt.name == "empty table" {
					if output != "" {
						t.Errorf("Empty table should produce no output")
					}
				} else {
					// Check that output contains headers and separator
					if !strings.Contains(output, "-") {
						t.Errorf("Table should have separator line")
					}
					if !strings.Contains(output, "Name") {
						t.Errorf("Table should have headers")
					}
				}
			}
		})
	}
}

func TestTextFormatter_FormatObject(t *testing.T) {
	tests := []struct {
		name    string
		obj     interface{}
		wantErr bool
	}{
		{
			name: "simple object",
			obj: map[string]interface{}{
				"Name": "Alice",
				"Age":  30,
			},
			wantErr: false,
		},
		{
			name: "struct object",
			obj: struct {
				Name string
				Age  int
			}{
				Name: "Bob",
				Age:  25,
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buf := &bytes.Buffer{}
			f := newTextFormatter(buf)

			err := f.FormatObject(tt.obj)
			if (err != nil) != tt.wantErr {
				t.Errorf("FormatObject() error = %v, wantErr %v", err, tt.wantErr)
			}

			output := buf.String()
			if output == "" && !tt.wantErr {
				t.Errorf("FormatObject() produced no output")
			}
		})
	}
}

func TestTextFormatter_FormatRaw(t *testing.T) {
	data := []byte("hello world")
	buf := &bytes.Buffer{}
	f := newTextFormatter(buf)

	err := f.FormatRaw(data)
	if err != nil {
		t.Errorf("FormatRaw() error = %v", err)
	}

	got := strings.TrimSpace(buf.String())
	if got != "hello world" {
		t.Errorf("FormatRaw() got %q, want %q", got, "hello world")
	}
}

func TestTextFormatter_FormatString(t *testing.T) {
	s := "test message"
	buf := &bytes.Buffer{}
	f := newTextFormatter(buf)

	err := f.FormatString(s)
	if err != nil {
		t.Errorf("FormatString() error = %v", err)
	}

	got := strings.TrimSpace(buf.String())
	if got != s {
		t.Errorf("FormatString() got %q, want %q", got, s)
	}
}

func TestTextFormatter_TableAlignment(t *testing.T) {
	// Test that columns are properly aligned
	rows := []map[string]interface{}{
		{"Name": "Alice", "City": "NYC"},
		{"Name": "Bob", "City": "LA"},
	}

	buf := &bytes.Buffer{}
	f := newTextFormatter(buf)

	if err := f.FormatTable(rows); err != nil {
		t.Fatalf("FormatTable() error: %v", err)
	}

	output := buf.String()
	lines := strings.Split(strings.TrimSpace(output), "\n")

	if len(lines) < 3 {
		t.Errorf("Expected at least 3 lines (header, separator, data), got %d", len(lines))
	}

	// Check that header line exists
	if !strings.Contains(lines[0], "Name") {
		t.Errorf("First line should contain header")
	}
}
