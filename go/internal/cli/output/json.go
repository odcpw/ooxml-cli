package output

import (
	"encoding/json"
	"fmt"
	"io"
	"reflect"
)

// jsonFormatter implements Formatter for JSON output.
type jsonFormatter struct {
	w      io.Writer
	pretty bool
}

// newJSONFormatter creates a new JSON formatter.
func newJSONFormatter(w io.Writer, pretty bool) Formatter {
	return &jsonFormatter{
		w:      w,
		pretty: pretty,
	}
}

// FormatTable writes rows as a JSON array.
func (f *jsonFormatter) FormatTable(rows interface{}) error {
	rv := reflect.ValueOf(rows)
	if rv.Kind() != reflect.Slice {
		return fmt.Errorf("rows must be a slice, got %T", rows)
	}

	// Convert slice to interface{} slice
	var items []interface{}
	for i := 0; i < rv.Len(); i++ {
		items = append(items, rv.Index(i).Interface())
	}

	return f.marshalJSON(items)
}

// FormatObject writes an object as JSON.
func (f *jsonFormatter) FormatObject(obj interface{}) error {
	return f.marshalJSON(obj)
}

// FormatRaw writes raw bytes as-is (assumed to be valid JSON or text).
func (f *jsonFormatter) FormatRaw(data []byte) error {
	_, err := f.w.Write(data)
	if err != nil {
		return err
	}

	// Add newline for readability
	_, err = f.w.Write([]byte("\n"))
	return err
}

// FormatString writes a string as a JSON string.
func (f *jsonFormatter) FormatString(s string) error {
	return f.marshalJSON(s)
}

// marshalJSON marshals the object to JSON and writes it.
func (f *jsonFormatter) marshalJSON(obj interface{}) error {
	var data []byte
	var err error

	if f.pretty {
		data, err = json.MarshalIndent(obj, "", "  ")
	} else {
		data, err = json.Marshal(obj)
	}

	if err != nil {
		return fmt.Errorf("failed to marshal JSON: %w", err)
	}

	_, err = f.w.Write(data)
	if err != nil {
		return err
	}

	// Add newline for readability
	_, err = f.w.Write([]byte("\n"))
	return err
}
