package mutate

import (
	"errors"
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Errors returned by the filters/sorts mutations. Callers map these to
// ExitInvalidArgs at the CLI layer.
var (
	ErrNoAutoFilter        = errors.New("worksheet has no autoFilter; run set-autofilter first")
	ErrColumnFilterMissing = errors.New("column has no filter")
	ErrSortStateMissing    = errors.New("worksheet has no sortState")
	ErrRangeMismatch       = errors.New("range mismatch")
	ErrSortRefMismatch     = errors.New("sort ref mismatch")
	ErrFilterMismatch      = errors.New("filter mismatch")
	ErrColumnOutOfBounds   = errors.New("column ID exceeds range column count")
)

// validFilterOperators is the SpreadsheetML ST_FilterOperator enumeration. Only
// these six are valid on a <customFilter>. between/notBetween are CLI sugar that
// desugar to two customFilter children.
var validFilterOperators = map[string]bool{
	"equal":              true,
	"notEqual":           true,
	"lessThan":           true,
	"lessThanOrEqual":    true,
	"greaterThan":        true,
	"greaterThanOrEqual": true,
}

// customOpAliases normalizes user-friendly aliases to SpreadsheetML names.
var customOpAliases = map[string]string{
	"eq":                    "equal",
	"equals":                "equal",
	"==":                    "equal",
	"=":                     "equal",
	"ne":                    "notEqual",
	"!=":                    "notEqual",
	"<>":                    "notEqual",
	"lt":                    "lessThan",
	"<":                     "lessThan",
	"le":                    "lessThanOrEqual",
	"lte":                   "lessThanOrEqual",
	"<=":                    "lessThanOrEqual",
	"gt":                    "greaterThan",
	">":                     "greaterThan",
	"ge":                    "greaterThanOrEqual",
	"gte":                   "greaterThanOrEqual",
	">=":                    "greaterThanOrEqual",
	"greater-than":          "greaterThan",
	"less-than":             "lessThan",
	"greater-than-or-equal": "greaterThanOrEqual",
	"less-than-or-equal":    "lessThanOrEqual",
	"not-equal":             "notEqual",
	"between":               "between",
	"not-between":           "notBetween",
	"notbetween":            "notBetween",
}

// CustomFilterCriterion is one <customFilter> entry.
type CustomFilterCriterion struct {
	Operator string `json:"operator,omitempty"`
	Val      string `json:"val"`
}

// CustomFilter is a <customFilters> block.
type CustomFilter struct {
	And      bool                    `json:"and"`
	Criteria []CustomFilterCriterion `json:"criteria"`
}

// FilterColumn captures the state of one <filterColumn> within an autoFilter.
type FilterColumn struct {
	ColID        int           `json:"colId"`
	Values       []string      `json:"values,omitempty"`
	CustomFilter *CustomFilter `json:"customFilter,omitempty"`
}

// AutoFilterState is the readback view of an <autoFilter> element.
type AutoFilterState struct {
	Ref     string         `json:"ref"`
	Columns []FilterColumn `json:"columns,omitempty"`
}

// SortCondition is one <sortCondition> within a <sortState>.
type SortCondition struct {
	Ref        string `json:"ref"`
	Descending bool   `json:"descending"`
}

// SortStateInfo is the readback view of a <sortState> element.
type SortStateInfo struct {
	Ref        string          `json:"ref"`
	Conditions []SortCondition `json:"conditions,omitempty"`
}

// ---- requests ----

// SetAutoFilterRequest adds/replaces an autoFilter on a worksheet range or table.
type SetAutoFilterRequest struct {
	Package     opc.PackageSession
	SheetRef    model.SheetRef
	Table       *model.TableRef // non-nil targets table.xml instead of the worksheet
	Range       string
	ExpectRange string
	HasExpect   bool
}

// ClearAutoFilterRequest removes the autoFilter from a worksheet or table.
type ClearAutoFilterRequest struct {
	Package     opc.PackageSession
	SheetRef    model.SheetRef
	Table       *model.TableRef
	ExpectRange string
	HasExpect   bool
}

// AddColumnFilterRequest adds filter criteria to one column of an autoFilter.
type AddColumnFilterRequest struct {
	Package      opc.PackageSession
	SheetRef     model.SheetRef
	ColID        int
	Values       []string
	CustomOp     string
	CustomVal1   string
	CustomVal2   string
	HasCustom    bool
	ExpectFilter string
	HasExpect    bool
}

// ClearColumnFilterRequest removes one column's filter from an autoFilter.
type ClearColumnFilterRequest struct {
	Package  opc.PackageSession
	SheetRef model.SheetRef
	ColID    int
}

// SetSortRequest adds a sortCondition (creating the sortState if needed).
type SetSortRequest struct {
	Package    opc.PackageSession
	SheetRef   model.SheetRef
	Ref        string
	Column     string
	Descending bool
	ExpectSort string
	HasExpect  bool
}

// ClearSortRequest removes the worksheet sortState.
type ClearSortRequest struct {
	Package  opc.PackageSession
	SheetRef model.SheetRef
}

// FiltersSortsResult reports a filter/sort mutation outcome.
type FiltersSortsResult struct {
	Ref        string
	AutoFilter *AutoFilterState
	SortState  *SortStateInfo
}

// ---- readback ----

// ReadAutoFilter returns the worksheet-level autoFilter state, or nil if absent.
func ReadAutoFilter(session opc.PackageSession, sheet model.SheetRef) (*AutoFilterState, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, err
	}
	af := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter")
	if af == nil {
		return nil, nil
	}
	return autoFilterStateFromElement(af), nil
}

// ReadTableAutoFilter returns the table-level autoFilter state, or nil if absent.
func ReadTableAutoFilter(session opc.PackageSession, table model.TableRef) (*AutoFilterState, error) {
	_, tableRoot, err := readTableRoot(session, table)
	if err != nil {
		return nil, err
	}
	af := namespaces.FindChild(tableRoot, namespaces.NsSpreadsheetML, "autoFilter")
	if af == nil {
		return nil, nil
	}
	return autoFilterStateFromElement(af), nil
}

// ReadSortState returns the worksheet-level sortState, or nil if absent.
func ReadSortState(session opc.PackageSession, sheet model.SheetRef) (*SortStateInfo, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, err
	}
	ss := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sortState")
	if ss == nil {
		return nil, nil
	}
	return sortStateFromElement(ss), nil
}

// ---- autoFilter mutations ----

// SetAutoFilter adds (or replaces) an autoFilter element on the worksheet or table.
func SetAutoFilter(req *SetAutoFilterRequest) (*FiltersSortsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set autoFilter request is nil")
	}
	rangeRef, err := address.ParseRange(req.Range)
	if err != nil {
		return nil, fmt.Errorf("invalid range: %w", err)
	}
	normRange := rangeRef.String()

	if req.Table != nil {
		return setTableAutoFilter(req, normRange)
	}

	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	existing := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter")
	if err := guardExpectRange(existing, req.HasExpect, req.ExpectRange); err != nil {
		return nil, err
	}
	if existing != nil {
		// Replace just the @ref; preserve any existing column filters.
		existing.CreateAttr("ref", normRange)
	} else {
		af := newElement(prefix, "autoFilter")
		af.CreateAttr("ref", normRange)
		insertWorksheetChild(root, af, "autoFilter")
		existing = af
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FiltersSortsResult{Ref: normRange, AutoFilter: autoFilterStateFromElement(existing)}, nil
}

func setTableAutoFilter(req *SetAutoFilterRequest, normRange string) (*FiltersSortsResult, error) {
	doc, tableRoot, err := readTableRoot(req.Package, *req.Table)
	if err != nil {
		return nil, err
	}
	prefix := tableRoot.Space
	existing := namespaces.FindChild(tableRoot, namespaces.NsSpreadsheetML, "autoFilter")
	if err := guardExpectRange(existing, req.HasExpect, req.ExpectRange); err != nil {
		return nil, err
	}
	if existing != nil {
		existing.CreateAttr("ref", normRange)
	} else {
		af := newElement(prefix, "autoFilter")
		af.CreateAttr("ref", normRange)
		insertTableAutoFilter(tableRoot, af)
		existing = af
	}
	if err := req.Package.ReplaceXMLPart(req.Table.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace table part %s: %w", req.Table.PartURI, err)
	}
	return &FiltersSortsResult{Ref: normRange, AutoFilter: autoFilterStateFromElement(existing)}, nil
}

// ClearAutoFilter removes the autoFilter element from the worksheet or table.
func ClearAutoFilter(req *ClearAutoFilterRequest) (*FiltersSortsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear autoFilter request is nil")
	}
	if req.Table != nil {
		doc, tableRoot, err := readTableRoot(req.Package, *req.Table)
		if err != nil {
			return nil, err
		}
		af := namespaces.FindChild(tableRoot, namespaces.NsSpreadsheetML, "autoFilter")
		if err := guardExpectRange(af, req.HasExpect, req.ExpectRange); err != nil {
			return nil, err
		}
		if af == nil {
			return nil, ErrNoAutoFilter
		}
		tableRoot.RemoveChild(af)
		if err := req.Package.ReplaceXMLPart(req.Table.PartURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace table part %s: %w", req.Table.PartURI, err)
		}
		return &FiltersSortsResult{}, nil
	}

	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	af := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter")
	if err := guardExpectRange(af, req.HasExpect, req.ExpectRange); err != nil {
		return nil, err
	}
	if af == nil {
		return nil, ErrNoAutoFilter
	}
	root.RemoveChild(af)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FiltersSortsResult{}, nil
}

// AddColumnFilter adds value and/or custom criteria to one autoFilter column.
func AddColumnFilter(req *AddColumnFilterRequest) (*FiltersSortsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add column filter request is nil")
	}
	if req.ColID < 0 {
		return nil, fmt.Errorf("colId must be >= 0")
	}
	values := dedupeNonEmpty(req.Values)
	if len(values) == 0 && !req.HasCustom {
		return nil, fmt.Errorf("provide --values and/or a custom filter (--custom-op/--custom-val1)")
	}

	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	af := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter")
	if af == nil {
		return nil, ErrNoAutoFilter
	}
	colCount, err := autoFilterColumnCount(af)
	if err != nil {
		return nil, err
	}
	if req.ColID >= colCount {
		return nil, fmt.Errorf("%w: colId %d not in 0-%d", ErrColumnOutOfBounds, req.ColID, colCount-1)
	}

	prefix := root.Space
	existingCol := findFilterColumn(af, req.ColID)
	if err := guardExpectFilter(existingCol, req.HasExpect, req.ExpectFilter); err != nil {
		return nil, err
	}

	col := newElement(prefix, "filterColumn")
	col.CreateAttr("colId", strconv.Itoa(req.ColID))
	if len(values) > 0 {
		filters := newElement(prefix, "filters")
		for _, v := range values {
			f := newElement(prefix, "filter")
			f.CreateAttr("val", v)
			filters.AddChild(f)
		}
		col.AddChild(filters)
	}
	if req.HasCustom {
		custom, err := buildCustomFilter(prefix, req.CustomOp, req.CustomVal1, req.CustomVal2)
		if err != nil {
			return nil, err
		}
		col.AddChild(custom)
	}

	if existingCol != nil {
		af.RemoveChild(existingCol)
	}
	insertFilterColumn(af, col, req.ColID)

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FiltersSortsResult{Ref: af.SelectAttrValue("ref", ""), AutoFilter: autoFilterStateFromElement(af)}, nil
}

// ClearColumnFilter removes one column's filter from the autoFilter.
func ClearColumnFilter(req *ClearColumnFilterRequest) (*FiltersSortsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear column filter request is nil")
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	af := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "autoFilter")
	if af == nil {
		return nil, ErrNoAutoFilter
	}
	col := findFilterColumn(af, req.ColID)
	if col == nil {
		return nil, fmt.Errorf("%w: colId %d", ErrColumnFilterMissing, req.ColID)
	}
	af.RemoveChild(col)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FiltersSortsResult{Ref: af.SelectAttrValue("ref", ""), AutoFilter: autoFilterStateFromElement(af)}, nil
}

// ---- sortState mutations ----

// SetSort adds a sortCondition for --column to the worksheet sortState, creating
// it on first call. sortCondition/@ref is the single-column sub-range derived
// from the sortState ref bounds and the column letter (ST_Ref, required).
func SetSort(req *SetSortRequest) (*FiltersSortsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set sort request is nil")
	}
	sortRange, err := address.ParseRange(req.Ref)
	if err != nil {
		return nil, fmt.Errorf("invalid --ref: %w", err)
	}
	normRange := sortRange.String()
	condRef, err := sortConditionRef(sortRange, req.Column)
	if err != nil {
		return nil, err
	}

	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	ss := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sortState")
	if err := guardExpectSort(ss, req.HasExpect, req.ExpectSort); err != nil {
		return nil, err
	}
	if ss == nil {
		ss = newElement(prefix, "sortState")
		ss.CreateAttr("ref", normRange)
		insertWorksheetChild(root, ss, "sortState")
	} else {
		ss.CreateAttr("ref", normRange)
	}

	// Replace an existing condition for the same column, else append.
	if existing := findSortCondition(ss, condRef); existing != nil {
		ss.RemoveChild(existing)
	}
	cond := newElement(prefix, "sortCondition")
	if req.Descending {
		cond.CreateAttr("descending", "1")
	}
	cond.CreateAttr("ref", condRef)
	ss.AddChild(cond)

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FiltersSortsResult{Ref: normRange, SortState: sortStateFromElement(ss)}, nil
}

// ClearSort removes the worksheet sortState element.
func ClearSort(req *ClearSortRequest) (*FiltersSortsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear sort request is nil")
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	ss := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sortState")
	if ss == nil {
		return nil, ErrSortStateMissing
	}
	root.RemoveChild(ss)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FiltersSortsResult{}, nil
}

// ---- internal helpers ----

func readTableRoot(session opc.PackageSession, table model.TableRef) (*etree.Document, *etree.Element, error) {
	if session == nil {
		return nil, nil, fmt.Errorf("package session is nil")
	}
	if table.PartURI == "" {
		return nil, nil, fmt.Errorf("table %q has no part URI", table.DisplayName)
	}
	doc, err := session.ReadXMLPart(table.PartURI)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read table part %s: %w", table.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "table") {
		return nil, nil, fmt.Errorf("table part %s root element not found", table.PartURI)
	}
	return doc, root, nil
}

// insertTableAutoFilter inserts <autoFilter> as the first element child of a
// <table>; per CT_Table it precedes <sortState>, <tableColumns>, and <tableStyleInfo>.
func insertTableAutoFilter(tableRoot, af *etree.Element) {
	for _, existing := range tableRoot.ChildElements() {
		tableRoot.InsertChildAt(existing.Index(), af)
		return
	}
	tableRoot.AddChild(af)
}

// NormalizeCustomOperator maps an alias to a SpreadsheetML operator (or
// between/notBetween sugar). Returns an error for unknown operators.
func NormalizeCustomOperator(op string) (string, error) {
	trimmed := strings.TrimSpace(op)
	if trimmed == "" {
		return "", fmt.Errorf("custom operator cannot be empty")
	}
	lower := strings.ToLower(trimmed)
	if mapped, ok := customOpAliases[lower]; ok {
		return mapped, nil
	}
	// Allow exact SpreadsheetML names (case-sensitive enum values).
	if validFilterOperators[trimmed] {
		return trimmed, nil
	}
	return "", fmt.Errorf("invalid custom operator %q (use one of equal,notEqual,lessThan,lessThanOrEqual,greaterThan,greaterThanOrEqual,between,notBetween)", op)
}

// buildCustomFilter builds a <customFilters> element. between/notBetween desugar
// into two <customFilter> children; the operator attribute lives on each
// <customFilter>, never on <customFilters> (only @and lives there).
func buildCustomFilter(prefix, op, val1, val2 string) (*etree.Element, error) {
	normOp, err := NormalizeCustomOperator(op)
	if err != nil {
		return nil, err
	}
	if strings.TrimSpace(val1) == "" {
		return nil, fmt.Errorf("--custom-val1 is required for a custom filter")
	}
	custom := newElement(prefix, "customFilters")

	switch normOp {
	case "between", "notBetween":
		if strings.TrimSpace(val2) == "" {
			return nil, fmt.Errorf("--custom-val2 is required for %s", op)
		}
		if normOp == "between" {
			custom.CreateAttr("and", "1")
			addCustomFilter(prefix, custom, "greaterThanOrEqual", val1)
			addCustomFilter(prefix, custom, "lessThanOrEqual", val2)
		} else {
			// notBetween is an OR of the two open intervals.
			addCustomFilter(prefix, custom, "lessThan", val1)
			addCustomFilter(prefix, custom, "greaterThan", val2)
		}
	default:
		if strings.TrimSpace(val2) != "" {
			return nil, fmt.Errorf("--custom-val2 is only valid with the between or notBetween operator")
		}
		addCustomFilter(prefix, custom, normOp, val1)
	}
	return custom, nil
}

func addCustomFilter(prefix string, parent *etree.Element, op, val string) {
	f := newElement(prefix, "customFilter")
	// equal is the default operator; omit it to match Excel's output, but emit
	// everything else explicitly.
	if op != "equal" {
		f.CreateAttr("operator", op)
	}
	f.CreateAttr("val", val)
	parent.AddChild(f)
}

func dedupeNonEmpty(values []string) []string {
	seen := map[string]bool{}
	var out []string
	for _, v := range values {
		if v == "" || seen[v] {
			continue
		}
		seen[v] = true
		out = append(out, v)
	}
	return out
}

func autoFilterColumnCount(af *etree.Element) (int, error) {
	ref := af.SelectAttrValue("ref", "")
	rangeRef, err := address.ParseRange(ref)
	if err != nil {
		return 0, fmt.Errorf("invalid autoFilter ref %q: %w", ref, err)
	}
	minCol, _, maxCol, _ := rangeRef.Bounds()
	return maxCol - minCol + 1, nil
}

func findFilterColumn(af *etree.Element, colID int) *etree.Element {
	for _, col := range namespaces.FindChildren(af, namespaces.NsSpreadsheetML, "filterColumn") {
		if id, err := strconv.Atoi(col.SelectAttrValue("colId", "")); err == nil && id == colID {
			return col
		}
	}
	return nil
}

// insertFilterColumn inserts a <filterColumn> ordered by colId, before any
// <sortState> child of the autoFilter.
func insertFilterColumn(af, col *etree.Element, colID int) {
	for _, existing := range af.ChildElements() {
		if namespaces.IsElement(existing, namespaces.NsSpreadsheetML, "filterColumn") {
			if id, err := strconv.Atoi(existing.SelectAttrValue("colId", "")); err == nil && id > colID {
				af.InsertChildAt(existing.Index(), col)
				return
			}
			continue
		}
		// Insert before sortState (or any non-filterColumn child).
		if namespaces.IsElement(existing, namespaces.NsSpreadsheetML, "sortState") {
			af.InsertChildAt(existing.Index(), col)
			return
		}
	}
	af.AddChild(col)
}

func findSortCondition(ss *etree.Element, ref string) *etree.Element {
	for _, cond := range namespaces.FindChildren(ss, namespaces.NsSpreadsheetML, "sortCondition") {
		if cond.SelectAttrValue("ref", "") == ref {
			return cond
		}
	}
	return nil
}

// sortConditionRef builds the single-column sub-range (ST_Ref) for a sort
// condition from the sortState range bounds and the chosen column letter.
func sortConditionRef(sortRange address.RangeRef, column string) (string, error) {
	colIdx, err := address.ColumnLettersToIndex(strings.TrimSpace(column))
	if err != nil {
		return "", fmt.Errorf("invalid --column: %w", err)
	}
	minCol, minRow, maxCol, maxRow := sortRange.Bounds()
	if colIdx < minCol || colIdx > maxCol {
		return "", fmt.Errorf("column %s is outside sort ref %s", strings.ToUpper(column), sortRange.String())
	}
	sub := address.RangeRef{
		Start: address.CellRef{Column: colIdx, Row: minRow},
		End:   address.CellRef{Column: colIdx, Row: maxRow},
	}
	return sub.String(), nil
}

func guardExpectRange(elem *etree.Element, has bool, expect string) error {
	if !has {
		return nil
	}
	current := ""
	if elem != nil {
		current = elem.SelectAttrValue("ref", "")
	}
	want, err := address.NormalizeRange(expect)
	if err != nil {
		return fmt.Errorf("invalid --expect-range: %w", err)
	}
	got := current
	if current != "" {
		if n, err := address.NormalizeRange(current); err == nil {
			got = n
		}
	}
	if got != want {
		return fmt.Errorf("%w: expected %s, found %q", ErrRangeMismatch, want, current)
	}
	return nil
}

func guardExpectSort(ss *etree.Element, has bool, expect string) error {
	if !has {
		return nil
	}
	current := ""
	if ss != nil {
		current = ss.SelectAttrValue("ref", "")
	}
	want, err := address.NormalizeRange(expect)
	if err != nil {
		return fmt.Errorf("invalid --expect-sort: %w", err)
	}
	got := current
	if current != "" {
		if n, err := address.NormalizeRange(current); err == nil {
			got = n
		}
	}
	if got != want {
		return fmt.Errorf("%w: expected %s, found %q", ErrSortRefMismatch, want, current)
	}
	return nil
}

func guardExpectFilter(col *etree.Element, has bool, expect string) error {
	if !has {
		return nil
	}
	current := "none"
	if col != nil {
		current = summarizeFilterColumn(col)
	}
	if current != strings.TrimSpace(expect) {
		return fmt.Errorf("%w: expected %q, found %q", ErrFilterMismatch, expect, current)
	}
	return nil
}

// summarizeFilterColumn produces a stable string for the --expect-filter guard:
// "none", "values:a,b,c", or "custom:op=val,...".
func summarizeFilterColumn(col *etree.Element) string {
	state := autoFilterColumnState(col)
	if len(state.Values) > 0 {
		return "values:" + strings.Join(state.Values, ",")
	}
	if state.CustomFilter != nil {
		var parts []string
		for _, c := range state.CustomFilter.Criteria {
			op := c.Operator
			if op == "" {
				op = "equal"
			}
			parts = append(parts, op+"="+c.Val)
		}
		return "custom:" + strings.Join(parts, ",")
	}
	return "none"
}

func autoFilterStateFromElement(af *etree.Element) *AutoFilterState {
	state := &AutoFilterState{Ref: af.SelectAttrValue("ref", "")}
	cols := namespaces.FindChildren(af, namespaces.NsSpreadsheetML, "filterColumn")
	for _, col := range cols {
		state.Columns = append(state.Columns, autoFilterColumnState(col))
	}
	sort.SliceStable(state.Columns, func(i, j int) bool {
		return state.Columns[i].ColID < state.Columns[j].ColID
	})
	return state
}

func autoFilterColumnState(col *etree.Element) FilterColumn {
	fc := FilterColumn{}
	if id, err := strconv.Atoi(col.SelectAttrValue("colId", "")); err == nil {
		fc.ColID = id
	}
	if filters := namespaces.FindChild(col, namespaces.NsSpreadsheetML, "filters"); filters != nil {
		for _, f := range namespaces.FindChildren(filters, namespaces.NsSpreadsheetML, "filter") {
			fc.Values = append(fc.Values, f.SelectAttrValue("val", ""))
		}
	}
	if custom := namespaces.FindChild(col, namespaces.NsSpreadsheetML, "customFilters"); custom != nil {
		cf := &CustomFilter{And: custom.SelectAttrValue("and", "") == "1"}
		for _, f := range namespaces.FindChildren(custom, namespaces.NsSpreadsheetML, "customFilter") {
			op := f.SelectAttrValue("operator", "")
			if op == "" {
				op = "equal"
			}
			cf.Criteria = append(cf.Criteria, CustomFilterCriterion{Operator: op, Val: f.SelectAttrValue("val", "")})
		}
		fc.CustomFilter = cf
	}
	return fc
}

func sortStateFromElement(ss *etree.Element) *SortStateInfo {
	info := &SortStateInfo{Ref: ss.SelectAttrValue("ref", "")}
	for _, cond := range namespaces.FindChildren(ss, namespaces.NsSpreadsheetML, "sortCondition") {
		info.Conditions = append(info.Conditions, SortCondition{
			Ref:        cond.SelectAttrValue("ref", ""),
			Descending: cond.SelectAttrValue("descending", "") == "1",
		})
	}
	return info
}
