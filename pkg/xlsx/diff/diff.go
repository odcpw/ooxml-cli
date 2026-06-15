// Package diff computes a deterministic semantic diff between two XLSX
// workbooks: sheet membership, cell values, formulas, defined names, and table
// definitions. It reuses the existing inspect/sheet/table readers so the diff
// reflects the same model the rest of the CLI exposes.
package diff

import (
	"fmt"
	"sort"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
)

// SchemaVersion pins the XLSX semantic diff contract.
const SchemaVersion = "1.0"

// Report is the structured XLSX semantic diff result. All slices are sorted for
// deterministic output.
type Report struct {
	SchemaVersion    string            `json:"schemaVersion"`
	SheetCountA      int               `json:"sheetCountA"`
	SheetCountB      int               `json:"sheetCountB"`
	SheetCountEqual  bool              `json:"sheetCountEqual"`
	ChangedSheets    []string          `json:"changedSheets"`
	Sheets           []SheetDiff       `json:"sheets"`
	CellDiffs        []CellDiff        `json:"cellDiffs"`
	DefinedNameDiffs []DefinedNameDiff `json:"definedNameDiffs"`
	TableDiffs       []TableDiff       `json:"tableDiffs"`
}

// SheetDiff records a sheet added or removed between the two workbooks.
type SheetDiff struct {
	Sheet  string `json:"sheet"`
	Change string `json:"change"` // "added" or "removed"
}

// CellDiff records a per-cell value or formula change within a sheet.
type CellDiff struct {
	Sheet    string `json:"sheet"`
	Cell     string `json:"cell"`
	Property string `json:"property"` // "value" or "formula"
	Before   string `json:"before"`
	After    string `json:"after"`
}

// DefinedNameDiff records a workbook/sheet defined-name change.
type DefinedNameDiff struct {
	Name   string `json:"name"`
	Scope  string `json:"scope"`
	Change string `json:"change"` // "added", "removed", or "modified"
	Before string `json:"before,omitempty"`
	After  string `json:"after,omitempty"`
}

// TableDiff records a table definition change.
type TableDiff struct {
	Sheet    string `json:"sheet"`
	Table    string `json:"table"`
	Property string `json:"property"` // "presence", "range", "columns"
	Change   string `json:"change"`   // "added", "removed", or "modified"
	Before   string `json:"before,omitempty"`
	After    string `json:"after,omitempty"`
}

// SemanticDiff compares two XLSX packages without rendering.
func SemanticDiff(a, b opc.PackageSession) (*Report, error) {
	if a == nil || b == nil {
		return nil, fmt.Errorf("semantic diff requires two package sessions")
	}

	wbA, err := inspect.ParseWorkbook(a)
	if err != nil {
		return nil, fmt.Errorf("failed to parse baseline workbook: %w", err)
	}
	wbB, err := inspect.ParseWorkbook(b)
	if err != nil {
		return nil, fmt.Errorf("failed to parse candidate workbook: %w", err)
	}

	report := &Report{
		SchemaVersion:    SchemaVersion,
		SheetCountA:      len(wbA.Sheets),
		SheetCountB:      len(wbB.Sheets),
		SheetCountEqual:  len(wbA.Sheets) == len(wbB.Sheets),
		ChangedSheets:    []string{},
		Sheets:           []SheetDiff{},
		CellDiffs:        []CellDiff{},
		DefinedNameDiffs: []DefinedNameDiff{},
		TableDiffs:       []TableDiff{},
	}

	sheetsA := indexSheets(wbA)
	sheetsB := indexSheets(wbB)
	changed := map[string]struct{}{}

	for _, name := range sheetNameUnion(sheetsA, sheetsB) {
		_, inA := sheetsA[name]
		_, inB := sheetsB[name]
		switch {
		case inA && !inB:
			report.Sheets = append(report.Sheets, SheetDiff{Sheet: name, Change: "removed"})
			changed[name] = struct{}{}
		case !inA && inB:
			report.Sheets = append(report.Sheets, SheetDiff{Sheet: name, Change: "added"})
			changed[name] = struct{}{}
		default:
			cellsA, err := readCells(a, wbA, sheetsA[name])
			if err != nil {
				return nil, err
			}
			cellsB, err := readCells(b, wbB, sheetsB[name])
			if err != nil {
				return nil, err
			}
			for _, diff := range compareCells(name, cellsA, cellsB) {
				report.CellDiffs = append(report.CellDiffs, diff)
				changed[name] = struct{}{}
			}
		}
	}

	namesA, err := inspect.ListDefinedNames(a)
	if err != nil {
		return nil, fmt.Errorf("failed to list baseline defined names: %w", err)
	}
	namesB, err := inspect.ListDefinedNames(b)
	if err != nil {
		return nil, fmt.Errorf("failed to list candidate defined names: %w", err)
	}
	report.DefinedNameDiffs = append(report.DefinedNameDiffs, compareDefinedNames(namesA, namesB)...)

	tablesA, err := table.List(a, wbA, wbA.Sheets)
	if err != nil {
		return nil, fmt.Errorf("failed to list baseline tables: %w", err)
	}
	tablesB, err := table.List(b, wbB, wbB.Sheets)
	if err != nil {
		return nil, fmt.Errorf("failed to list candidate tables: %w", err)
	}
	for _, diff := range compareTables(tablesA, tablesB) {
		report.TableDiffs = append(report.TableDiffs, diff)
		if diff.Sheet != "" {
			changed[diff.Sheet] = struct{}{}
		}
	}

	for name := range changed {
		report.ChangedSheets = append(report.ChangedSheets, name)
	}
	sort.Strings(report.ChangedSheets)
	return report, nil
}

func indexSheets(wb *model.Workbook) map[string]model.SheetRef {
	indexed := map[string]model.SheetRef{}
	for _, ref := range wb.Sheets {
		indexed[ref.Name] = ref
	}
	return indexed
}

func sheetNameUnion(a, b map[string]model.SheetRef) []string {
	set := map[string]struct{}{}
	for name := range a {
		set[name] = struct{}{}
	}
	for name := range b {
		set[name] = struct{}{}
	}
	names := make([]string, 0, len(set))
	for name := range set {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func readCells(session opc.PackageSession, wb *model.Workbook, ref model.SheetRef) (map[string]model.Cell, error) {
	ctx, err := sheet.LoadContext(session, wb)
	if err != nil {
		return nil, fmt.Errorf("failed to load workbook context: %w", err)
	}
	report, err := sheet.Read(session, ref, ctx, sheet.ReadOptions{IncludeData: true, IncludeEmpty: false})
	if err != nil {
		return nil, fmt.Errorf("failed to read sheet %q: %w", ref.Name, err)
	}
	cells := map[string]model.Cell{}
	for _, row := range report.Rows {
		for _, cell := range row.Cells {
			cells[cell.Ref] = cell
		}
	}
	return cells, nil
}

func compareCells(sheetName string, before, after map[string]model.Cell) []CellDiff {
	diffs := make([]CellDiff, 0)
	for _, ref := range cellRefUnion(before, after) {
		left := before[ref]
		right := after[ref]
		if left.Value != right.Value {
			diffs = append(diffs, CellDiff{Sheet: sheetName, Cell: ref, Property: "value", Before: left.Value, After: right.Value})
		}
		if left.Formula != right.Formula {
			diffs = append(diffs, CellDiff{Sheet: sheetName, Cell: ref, Property: "formula", Before: left.Formula, After: right.Formula})
		}
	}
	return diffs
}

func cellRefUnion(a, b map[string]model.Cell) []string {
	set := map[string]struct{}{}
	for ref := range a {
		set[ref] = struct{}{}
	}
	for ref := range b {
		set[ref] = struct{}{}
	}
	refs := make([]string, 0, len(set))
	for ref := range set {
		refs = append(refs, ref)
	}
	sort.Slice(refs, func(i, j int) bool {
		ai, aj := a[refs[i]], a[refs[j]]
		bi, bj := b[refs[i]], b[refs[j]]
		ci := cellOrder(ai, bi)
		cj := cellOrder(aj, bj)
		if ci.row != cj.row {
			return ci.row < cj.row
		}
		if ci.col != cj.col {
			return ci.col < cj.col
		}
		return refs[i] < refs[j]
	})
	return refs
}

type cellCoord struct {
	row int
	col int
}

func cellOrder(a, b model.Cell) cellCoord {
	if a.Ref != "" {
		return cellCoord{row: a.Row, col: a.Col}
	}
	return cellCoord{row: b.Row, col: b.Col}
}

func compareDefinedNames(before, after []model.DefinedName) []DefinedNameDiff {
	indexBefore := indexDefinedNames(before)
	indexAfter := indexDefinedNames(after)
	diffs := make([]DefinedNameDiff, 0)
	for _, key := range definedNameKeyUnion(indexBefore, indexAfter) {
		left, inA := indexBefore[key]
		right, inB := indexAfter[key]
		switch {
		case inA && !inB:
			diffs = append(diffs, DefinedNameDiff{Name: left.Name, Scope: left.Scope, Change: "removed", Before: left.Ref})
		case !inA && inB:
			diffs = append(diffs, DefinedNameDiff{Name: right.Name, Scope: right.Scope, Change: "added", After: right.Ref})
		case left.Ref != right.Ref:
			diffs = append(diffs, DefinedNameDiff{Name: left.Name, Scope: left.Scope, Change: "modified", Before: left.Ref, After: right.Ref})
		}
	}
	return diffs
}

func indexDefinedNames(names []model.DefinedName) map[string]model.DefinedName {
	indexed := map[string]model.DefinedName{}
	for _, name := range names {
		indexed[definedNameKey(name)] = name
	}
	return indexed
}

func definedNameKey(name model.DefinedName) string {
	if name.Scope == "sheet" {
		return name.Scope + "\x00" + name.SheetName + "\x00" + name.Name
	}
	return name.Scope + "\x00\x00" + name.Name
}

func definedNameKeyUnion(a, b map[string]model.DefinedName) []string {
	set := map[string]struct{}{}
	for key := range a {
		set[key] = struct{}{}
	}
	for key := range b {
		set[key] = struct{}{}
	}
	keys := make([]string, 0, len(set))
	for key := range set {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func compareTables(before, after []model.TableRef) []TableDiff {
	indexBefore := indexTables(before)
	indexAfter := indexTables(after)
	diffs := make([]TableDiff, 0)
	for _, key := range tableKeyUnion(indexBefore, indexAfter) {
		left, inA := indexBefore[key]
		right, inB := indexAfter[key]
		switch {
		case inA && !inB:
			diffs = append(diffs, TableDiff{Sheet: left.Sheet, Table: tableName(left), Property: "presence", Change: "removed"})
		case !inA && inB:
			diffs = append(diffs, TableDiff{Sheet: right.Sheet, Table: tableName(right), Property: "presence", Change: "added"})
		default:
			if left.Range != right.Range {
				diffs = append(diffs, TableDiff{Sheet: left.Sheet, Table: tableName(left), Property: "range", Change: "modified", Before: left.Range, After: right.Range})
			}
			beforeCols := tableColumns(left)
			afterCols := tableColumns(right)
			if beforeCols != afterCols {
				diffs = append(diffs, TableDiff{Sheet: left.Sheet, Table: tableName(left), Property: "columns", Change: "modified", Before: beforeCols, After: afterCols})
			}
		}
	}
	return diffs
}

func indexTables(tables []model.TableRef) map[string]model.TableRef {
	indexed := map[string]model.TableRef{}
	for _, tbl := range tables {
		indexed[tbl.Sheet+"\x00"+tableName(tbl)] = tbl
	}
	return indexed
}

func tableName(tbl model.TableRef) string {
	if tbl.DisplayName != "" {
		return tbl.DisplayName
	}
	if tbl.Name != "" {
		return tbl.Name
	}
	return fmt.Sprintf("table:%d", tbl.ID)
}

func tableColumns(tbl model.TableRef) string {
	names := make([]string, 0, len(tbl.Columns))
	for _, col := range tbl.Columns {
		names = append(names, col.Name)
	}
	out := ""
	for i, name := range names {
		if i > 0 {
			out += ", "
		}
		out += name
	}
	return out
}

func tableKeyUnion(a, b map[string]model.TableRef) []string {
	set := map[string]struct{}{}
	for key := range a {
		set[key] = struct{}{}
	}
	for key := range b {
		set[key] = struct{}{}
	}
	keys := make([]string, 0, len(set))
	for key := range set {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}
