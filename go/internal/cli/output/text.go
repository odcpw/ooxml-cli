package output

import (
	"fmt"
	"io"
	"reflect"
	"strings"
)

// textFormatter implements Formatter for human-readable text output.
type textFormatter struct {
	w io.Writer
}

// newTextFormatter creates a new text formatter.
func newTextFormatter(w io.Writer) Formatter {
	return &textFormatter{w: w}
}

// FormatTable writes rows as aligned columns.
func (f *textFormatter) FormatTable(rows interface{}) error {
	rv := reflect.ValueOf(rows)
	if rv.Kind() != reflect.Slice {
		return fmt.Errorf("rows must be a slice, got %T", rows)
	}

	if rv.Len() == 0 {
		return nil // empty table
	}

	// Get field names from first element
	first := rv.Index(0)
	var headers []string
	var fieldIndices []int

	if first.Kind() == reflect.Struct {
		t := first.Type()
		for i := 0; i < t.NumField(); i++ {
			headers = append(headers, t.Field(i).Name)
			fieldIndices = append(fieldIndices, i)
		}
	} else if first.Kind() == reflect.Map {
		m := first.Interface().(map[string]interface{})
		for k := range m {
			headers = append(headers, k)
		}
	} else {
		return fmt.Errorf("unsupported row type: %T", first.Interface())
	}

	// Calculate column widths
	widths := make([]int, len(headers))
	for i, h := range headers {
		widths[i] = len(h)
	}

	// Collect all rows as strings
	var rows_str [][]string
	for i := 0; i < rv.Len(); i++ {
		elem := rv.Index(i)
		var row []string

		if elem.Kind() == reflect.Struct {
			for _, idx := range fieldIndices {
				val := elem.Field(idx).Interface()
				s := fmt.Sprintf("%v", val)
				row = append(row, s)
				if len(s) > widths[len(row)-1] {
					widths[len(row)-1] = len(s)
				}
			}
		} else if elem.Kind() == reflect.Map {
			m := elem.Interface().(map[string]interface{})
			for _, h := range headers {
				s := fmt.Sprintf("%v", m[h])
				row = append(row, s)
				idx := len(row) - 1
				if len(s) > widths[idx] {
					widths[idx] = len(s)
				}
			}
		}
		rows_str = append(rows_str, row)
	}

	// Write headers
	headerLine := f.formatRow(headers, widths)
	_, err := f.w.Write([]byte(headerLine + "\n"))
	if err != nil {
		return err
	}

	// Write separator
	var sep []string
	for _, w := range widths {
		sep = append(sep, strings.Repeat("-", w))
	}
	sepLine := f.formatRow(sep, widths)
	_, err = f.w.Write([]byte(sepLine + "\n"))
	if err != nil {
		return err
	}

	// Write rows
	for _, row := range rows_str {
		rowLine := f.formatRow(row, widths)
		_, err := f.w.Write([]byte(rowLine + "\n"))
		if err != nil {
			return err
		}
	}

	return nil
}

// FormatObject writes an object as formatted text.
func (f *textFormatter) FormatObject(obj interface{}) error {
	s := fmt.Sprintf("%+v\n", obj)
	_, err := f.w.Write([]byte(s))
	return err
}

// FormatRaw writes raw bytes as-is.
func (f *textFormatter) FormatRaw(data []byte) error {
	_, err := f.w.Write(data)
	if err != nil {
		return err
	}
	_, err = f.w.Write([]byte("\n"))
	return err
}

// FormatString writes a string to the output.
func (f *textFormatter) FormatString(s string) error {
	_, err := f.w.Write([]byte(s + "\n"))
	return err
}

// formatRow formats a row with aligned columns.
func (f *textFormatter) formatRow(cells []string, widths []int) string {
	var parts []string
	for i, cell := range cells {
		padded := fmt.Sprintf("%-*s", widths[i], cell)
		parts = append(parts, padded)
	}
	return strings.Join(parts, " | ")
}
