package output

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestJSONFormatter_FormatTable(t *testing.T) {
	tests := []struct {
		name      string
		rows      interface{}
		pretty    bool
		wantArray bool
		wantErr   bool
	}{
		{
			name: "simple table compact",
			rows: []map[string]interface{}{
				{"Name": "Alice", "Age": 30},
				{"Name": "Bob", "Age": 25},
			},
			pretty:    false,
			wantArray: true,
		},
		{
			name: "simple table pretty",
			rows: []map[string]interface{}{
				{"Name": "Alice"},
				{"Name": "Bob"},
			},
			pretty:    true,
			wantArray: true,
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
			pretty:    false,
			wantArray: true,
		},
		{
			name:      "empty table",
			rows:      []map[string]interface{}{},
			pretty:    false,
			wantArray: false, // empty tables produce no output
		},
		{
			name:    "invalid input",
			rows:    "not a slice",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buf := &bytes.Buffer{}
			f := newJSONFormatter(buf, tt.pretty)

			err := f.FormatTable(tt.rows)
			if (err != nil) != tt.wantErr {
				t.Errorf("FormatTable() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			if tt.wantErr {
				return
			}

			// Verify output is valid JSON
			output := strings.TrimSpace(buf.String())

			// Handle empty output case
			if output == "" {
				if tt.wantArray {
					t.Errorf("FormatTable() should produce output for non-empty tables")
				}
				return
			}

			var result interface{}
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Errorf("FormatTable() produced invalid JSON: %v", err)
			}

			// Check that it's an array
			if tt.wantArray {
				if _, ok := result.([]interface{}); !ok {
					t.Errorf("FormatTable() should produce JSON array, got %T", result)
				}
			}
		})
	}
}

func TestJSONFormatter_FormatObject(t *testing.T) {
	tests := []struct {
		name    string
		obj     interface{}
		pretty  bool
		wantErr bool
	}{
		{
			name:   "simple object compact",
			obj:    map[string]interface{}{"Name": "Alice", "Age": 30},
			pretty: false,
		},
		{
			name:   "simple object pretty",
			obj:    map[string]interface{}{"Name": "Bob", "Age": 25},
			pretty: true,
		},
		{
			name: "nested object",
			obj: map[string]interface{}{
				"Name": "Carol",
				"Address": map[string]interface{}{
					"City": "NYC",
				},
			},
			pretty: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			buf := &bytes.Buffer{}
			f := newJSONFormatter(buf, tt.pretty)

			err := f.FormatObject(tt.obj)
			if (err != nil) != tt.wantErr {
				t.Errorf("FormatObject() error = %v, wantErr %v", err, tt.wantErr)
				return
			}

			// Verify output is valid JSON
			output := strings.TrimSpace(buf.String())
			var result interface{}
			if err := json.Unmarshal([]byte(output), &result); err != nil {
				t.Errorf("FormatObject() produced invalid JSON: %v", err)
			}
		})
	}
}

func TestJSONFormatter_PrettyPrint(t *testing.T) {
	obj := map[string]interface{}{
		"Name": "Alice",
		"Age":  30,
	}

	// Compact output
	compactBuf := &bytes.Buffer{}
	f := newJSONFormatter(compactBuf, false)
	if err := f.FormatObject(obj); err != nil {
		t.Fatalf("FormatObject() error: %v", err)
	}
	compactOutput := strings.TrimSpace(compactBuf.String())

	// Pretty output
	prettyBuf := &bytes.Buffer{}
	f = newJSONFormatter(prettyBuf, true)
	if err := f.FormatObject(obj); err != nil {
		t.Fatalf("FormatObject() error: %v", err)
	}
	prettyOutput := strings.TrimSpace(prettyBuf.String())

	// Pretty should have newlines, compact should not (beyond final newline)
	if strings.Count(prettyOutput, "\n") < 1 {
		t.Errorf("Pretty output should have indentation/newlines")
	}

	// Both should be valid JSON
	var compactResult, prettyResult interface{}
	if err := json.Unmarshal([]byte(compactOutput), &compactResult); err != nil {
		t.Errorf("Compact JSON invalid: %v", err)
	}
	if err := json.Unmarshal([]byte(prettyOutput), &prettyResult); err != nil {
		t.Errorf("Pretty JSON invalid: %v", err)
	}
}

func TestJSONFormatter_FormatRaw(t *testing.T) {
	data := []byte(`{"key": "value"}`)
	buf := &bytes.Buffer{}
	f := newJSONFormatter(buf, false)

	err := f.FormatRaw(data)
	if err != nil {
		t.Errorf("FormatRaw() error = %v", err)
	}

	output := strings.TrimSpace(buf.String())
	if !strings.Contains(output, "key") || !strings.Contains(output, "value") {
		t.Errorf("FormatRaw() output mismatch: got %q", output)
	}
}

func TestJSONFormatter_FormatString(t *testing.T) {
	s := "hello world"
	buf := &bytes.Buffer{}
	f := newJSONFormatter(buf, false)

	err := f.FormatString(s)
	if err != nil {
		t.Errorf("FormatString() error = %v", err)
	}

	output := strings.TrimSpace(buf.String())

	// Verify it's valid JSON and is a quoted string
	var result string
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Errorf("FormatString() produced invalid JSON: %v", err)
	}

	if result != s {
		t.Errorf("FormatString() got %q, want %q", result, s)
	}
}
