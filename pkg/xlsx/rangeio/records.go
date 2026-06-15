package rangeio

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"strings"
)

const (
	MissingReject      = "reject"
	MissingSkip        = "skip"
	MissingEmptyString = "empty-string"
)

type RecordSet struct {
	Records []map[string]Cell
}

func DecodeRecords(data []byte) (*RecordSet, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var raw any
	if err := decoder.Decode(&raw); err != nil {
		return nil, err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("JSON input must contain exactly one value")
	}

	valueNode := raw
	if object, ok := raw.(map[string]any); ok {
		var found bool
		valueNode, found = object["records"]
		if !found {
			return nil, fmt.Errorf("JSON object must contain records")
		}
	}

	records, err := parseRecords(valueNode)
	if err != nil {
		return nil, err
	}
	return &RecordSet{Records: records}, nil
}

func NormalizeMissingPolicy(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", MissingReject:
		return MissingReject, nil
	case MissingSkip:
		return MissingSkip, nil
	case MissingEmptyString:
		return MissingEmptyString, nil
	default:
		return "", fmt.Errorf("invalid missing policy %q (must be reject, skip, or empty-string)", value)
	}
}

func RecordsToRows(records []map[string]Cell, columns []string, missingPolicy string, ignoreExtraFields bool) ([][]Cell, error) {
	if len(records) == 0 {
		return nil, fmt.Errorf("records cannot be empty")
	}
	normalizedMissing, err := NormalizeMissingPolicy(missingPolicy)
	if err != nil {
		return nil, err
	}
	if len(columns) == 0 {
		return nil, fmt.Errorf("table must have at least one column")
	}

	columnSet := make(map[string]struct{}, len(columns))
	for colIdx, column := range columns {
		if strings.TrimSpace(column) == "" {
			return nil, fmt.Errorf("table column %d has a blank name", colIdx+1)
		}
		if _, exists := columnSet[column]; exists {
			return nil, fmt.Errorf("duplicate table column name %q", column)
		}
		columnSet[column] = struct{}{}
	}

	rows := make([][]Cell, len(records))
	for rowIdx, record := range records {
		for key := range record {
			if _, exists := columnSet[key]; exists {
				continue
			}
			if ignoreExtraFields {
				continue
			}
			return nil, fmt.Errorf("records[%d] contains unknown field %q", rowIdx, key)
		}

		row := make([]Cell, len(columns))
		for colIdx, column := range columns {
			cell, exists := record[column]
			if exists {
				row[colIdx] = cell
				continue
			}
			switch normalizedMissing {
			case MissingReject:
				return nil, fmt.Errorf("records[%d] missing required field %q", rowIdx, column)
			case MissingSkip:
				row[colIdx] = Cell{Null: true}
			case MissingEmptyString:
				row[colIdx] = Cell{Type: "string", Value: ""}
			}
		}
		rows[rowIdx] = row
	}
	return rows, nil
}

func parseRecords(raw any) ([]map[string]Cell, error) {
	rawRecords, ok := raw.([]any)
	if !ok {
		return nil, fmt.Errorf("records must be an array of objects")
	}
	records := make([]map[string]Cell, len(rawRecords))
	for rowIdx, rawRecord := range rawRecords {
		object, ok := rawRecord.(map[string]any)
		if !ok {
			return nil, fmt.Errorf("records[%d] must be an object", rowIdx)
		}
		record := make(map[string]Cell, len(object))
		for key, rawCell := range object {
			if strings.TrimSpace(key) == "" {
				return nil, fmt.Errorf("records[%d] contains a blank field name", rowIdx)
			}
			cell, err := parseCell(rawCell)
			if err != nil {
				return nil, fmt.Errorf("records[%d].%s: %w", rowIdx, key, err)
			}
			record[key] = cell
		}
		records[rowIdx] = record
	}
	return records, nil
}
