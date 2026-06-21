// Package rangeio parses and serializes rectangular XLSX cell matrices.
package rangeio

import (
	"bytes"
	"encoding/csv"
	"encoding/json"
	"fmt"
	"io"
	"strconv"
	"strings"
)

const (
	FormatJSON = "json"
	FormatCSV  = "csv"
	FormatTSV  = "tsv"

	RaggedReject    = "reject"
	RaggedFillEmpty = "fill-empty"
)

type Cell struct {
	Type    string
	Value   string
	Formula string
	Null    bool
}

type Matrix struct {
	Range          string
	MajorDimension string
	NullPolicy     string
	Values         [][]Cell
}

func NormalizeDataFormat(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", FormatJSON:
		return FormatJSON, nil
	case FormatCSV:
		return FormatCSV, nil
	case FormatTSV:
		return FormatTSV, nil
	default:
		return "", fmt.Errorf("invalid data format %q (must be json, csv, or tsv)", value)
	}
}

func NormalizeRaggedMode(value string) (string, error) {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "", RaggedReject:
		return RaggedReject, nil
	case RaggedFillEmpty:
		return RaggedFillEmpty, nil
	default:
		return "", fmt.Errorf("invalid ragged mode %q (must be reject or fill-empty)", value)
	}
}

func Decode(data []byte, format string) (*Matrix, error) {
	normalized, err := NormalizeDataFormat(format)
	if err != nil {
		return nil, err
	}
	switch normalized {
	case FormatJSON:
		return DecodeJSON(data)
	case FormatCSV, FormatTSV:
		rows, err := decodeDelimited(data, normalized)
		if err != nil {
			return nil, err
		}
		return &Matrix{MajorDimension: "rows", Values: rows}, nil
	default:
		return nil, fmt.Errorf("unsupported data format %q", normalized)
	}
}

func DecodeJSON(data []byte) (*Matrix, error) {
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var raw any
	if err := decoder.Decode(&raw); err != nil {
		return nil, err
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		return nil, fmt.Errorf("JSON input must contain exactly one value")
	}

	matrix := &Matrix{MajorDimension: "rows"}
	valueNode := raw
	if object, ok := raw.(map[string]any); ok {
		if rangeValue, ok := object["range"]; ok {
			matrix.Range, ok = rangeValue.(string)
			if !ok {
				return nil, fmt.Errorf("range must be a string")
			}
		}
		if majorValue, ok := object["majorDimension"]; ok {
			matrix.MajorDimension, ok = majorValue.(string)
			if !ok {
				return nil, fmt.Errorf("majorDimension must be a string")
			}
		}
		if nullPolicyValue, ok := object["nullPolicy"]; ok {
			matrix.NullPolicy, ok = nullPolicyValue.(string)
			if !ok {
				return nil, fmt.Errorf("nullPolicy must be a string")
			}
		}
		valueNode, ok = object["values"]
		if !ok {
			return nil, fmt.Errorf("JSON object must contain values")
		}
	}

	rows, err := parseRows(valueNode)
	if err != nil {
		return nil, err
	}
	switch strings.ToLower(strings.TrimSpace(matrix.MajorDimension)) {
	case "", "rows":
		matrix.MajorDimension = "rows"
		matrix.Values = rows
	case "columns":
		matrix.MajorDimension = "columns"
		matrix.Values = transpose(rows)
	default:
		return nil, fmt.Errorf("majorDimension must be rows or columns")
	}
	return matrix, nil
}

func Rectangularize(rows [][]Cell, mode string) ([][]Cell, int, int, error) {
	normalized, err := NormalizeRaggedMode(mode)
	if err != nil {
		return nil, 0, 0, err
	}
	if len(rows) == 0 {
		return nil, 0, 0, fmt.Errorf("values matrix cannot be empty")
	}
	cols := len(rows[0])
	maxCols := cols
	for _, row := range rows[1:] {
		if len(row) > maxCols {
			maxCols = len(row)
		}
	}
	if maxCols == 0 {
		return nil, 0, 0, fmt.Errorf("values matrix must contain at least one column")
	}
	out := make([][]Cell, len(rows))
	for rowIdx, row := range rows {
		if normalized == RaggedReject && len(row) != cols {
			return nil, 0, 0, fmt.Errorf("ragged matrix row %d has %d columns, want %d", rowIdx+1, len(row), cols)
		}
		out[rowIdx] = append([]Cell(nil), row...)
		for len(out[rowIdx]) < maxCols {
			out[rowIdx] = append(out[rowIdx], Cell{Type: "string"})
		}
	}
	return out, len(out), maxCols, nil
}

func EncodeDelimited(rows [][]Cell, format string) ([]byte, error) {
	normalized, err := NormalizeDataFormat(format)
	if err != nil {
		return nil, err
	}
	if normalized != FormatCSV && normalized != FormatTSV {
		return nil, fmt.Errorf("data format %q is not delimited", normalized)
	}
	var buf bytes.Buffer
	writer := csv.NewWriter(&buf)
	if normalized == FormatTSV {
		writer.Comma = '\t'
	}
	for _, row := range rows {
		fields := make([]string, len(row))
		for i, cell := range row {
			if !cell.Null {
				fields[i] = cell.Value
			}
		}
		if err := writer.Write(fields); err != nil {
			return nil, err
		}
	}
	writer.Flush()
	if err := writer.Error(); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

func PrimitiveValues(rows [][]Cell) [][]any {
	values := make([][]any, len(rows))
	for rowIdx, row := range rows {
		values[rowIdx] = make([]any, len(row))
		for colIdx, cell := range row {
			values[rowIdx][colIdx] = primitiveValue(cell)
		}
	}
	return values
}

func Types(rows [][]Cell) [][]string {
	types := make([][]string, len(rows))
	for rowIdx, row := range rows {
		types[rowIdx] = make([]string, len(row))
		for colIdx, cell := range row {
			if cell.Null {
				types[rowIdx][colIdx] = "empty"
			} else {
				types[rowIdx][colIdx] = cell.Type
			}
		}
	}
	return types
}

func Formulas(rows [][]Cell) [][]any {
	formulas := make([][]any, len(rows))
	for rowIdx, row := range rows {
		formulas[rowIdx] = make([]any, len(row))
		for colIdx, cell := range row {
			if cell.Formula != "" {
				formulas[rowIdx][colIdx] = cell.Formula
			}
		}
	}
	return formulas
}

func FormulaCount(rows [][]Cell) int {
	count := 0
	for _, row := range rows {
		for _, cell := range row {
			if cell.Formula != "" || cell.Type == "formula" {
				count++
			}
		}
	}
	return count
}

func parseRows(raw any) ([][]Cell, error) {
	rawRows, ok := raw.([]any)
	if !ok {
		return nil, fmt.Errorf("values must be an array of arrays")
	}
	rows := make([][]Cell, len(rawRows))
	for rowIdx, rawRow := range rawRows {
		cells, ok := rawRow.([]any)
		if !ok {
			return nil, fmt.Errorf("values[%d] must be an array", rowIdx)
		}
		rows[rowIdx] = make([]Cell, len(cells))
		for colIdx, rawCell := range cells {
			cell, err := parseCell(rawCell)
			if err != nil {
				return nil, fmt.Errorf("values[%d][%d]: %w", rowIdx, colIdx, err)
			}
			rows[rowIdx][colIdx] = cell
		}
	}
	return rows, nil
}

func parseCell(raw any) (Cell, error) {
	switch value := raw.(type) {
	case nil:
		return Cell{Null: true}, nil
	case string:
		return Cell{Type: "string", Value: value}, nil
	case json.Number:
		return Cell{Type: "number", Value: value.String()}, nil
	case bool:
		return Cell{Type: "bool", Value: strconv.FormatBool(value)}, nil
	case map[string]any:
		return parseObjectCell(value)
	default:
		return Cell{}, fmt.Errorf("unsupported JSON cell type %T", raw)
	}
}

func parseObjectCell(object map[string]any) (Cell, error) {
	if formulaRaw, ok := object["formula"]; ok {
		formula, ok := formulaRaw.(string)
		if !ok {
			return Cell{}, fmt.Errorf("formula must be a string")
		}
		if strings.TrimSpace(formula) == "" {
			return Cell{}, fmt.Errorf("formula cannot be empty")
		}
		return Cell{Type: "formula", Value: formula, Formula: formula}, nil
	}

	rawValue, ok := object["value"]
	if !ok {
		return Cell{}, fmt.Errorf("object cell must contain value or formula")
	}
	if rawValue == nil {
		return Cell{Null: true}, nil
	}
	cell, err := parseCell(rawValue)
	if err != nil {
		return Cell{}, err
	}
	if typeRaw, ok := object["type"]; ok {
		typ, ok := typeRaw.(string)
		if !ok {
			return Cell{}, fmt.Errorf("type must be a string")
		}
		cell.Type = strings.ToLower(strings.TrimSpace(typ))
		if cell.Type == "formula" {
			cell.Formula = cell.Value
		}
	}
	return cell, nil
}

func decodeDelimited(data []byte, format string) ([][]Cell, error) {
	reader := csv.NewReader(bytes.NewReader(data))
	if format == FormatTSV {
		reader.Comma = '\t'
	}
	reader.FieldsPerRecord = -1
	records, err := reader.ReadAll()
	if err != nil {
		return nil, err
	}
	rows := make([][]Cell, len(records))
	for rowIdx, record := range records {
		rows[rowIdx] = make([]Cell, len(record))
		for colIdx, value := range record {
			rows[rowIdx][colIdx] = Cell{Type: "string", Value: value}
		}
	}
	return rows, nil
}

func primitiveValue(cell Cell) any {
	if cell.Null {
		return nil
	}
	switch cell.Type {
	case "number":
		if cell.Value == "" {
			return nil
		}
		if _, err := strconv.ParseFloat(cell.Value, 64); err == nil {
			return json.Number(cell.Value)
		}
		return cell.Value
	case "bool", "boolean":
		switch strings.ToLower(strings.TrimSpace(cell.Value)) {
		case "true", "1":
			return true
		case "false", "0":
			return false
		default:
			return cell.Value
		}
	default:
		return cell.Value
	}
}

func transpose(rows [][]Cell) [][]Cell {
	maxCols := 0
	for _, row := range rows {
		if len(row) > maxCols {
			maxCols = len(row)
		}
	}
	out := make([][]Cell, maxCols)
	for colIdx := 0; colIdx < maxCols; colIdx++ {
		out[colIdx] = make([]Cell, len(rows))
		for rowIdx := range rows {
			if colIdx < len(rows[rowIdx]) {
				out[colIdx][rowIdx] = rows[rowIdx][colIdx]
			} else {
				out[colIdx][rowIdx] = Cell{Type: "string"}
			}
		}
	}
	return out
}
