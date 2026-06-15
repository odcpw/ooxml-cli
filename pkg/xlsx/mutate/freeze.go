package mutate

import (
	"errors"
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Errors returned by the freeze-panes mutations. Callers map these to
// ExitInvalidArgs at the CLI layer.
var (
	// ErrStateMismatch is returned when an --expect-state guard does not match
	// the worksheet's current freeze state.
	ErrStateMismatch = errors.New("freeze state mismatch")
	// ErrNoFreeze is returned by ClearFreeze when the worksheet has no frozen pane.
	ErrNoFreeze = errors.New("worksheet has no frozen pane")
)

// FreezeState is the readback view of a frozen <pane> on a worksheet.
type FreezeState struct {
	Rows        int    `json:"rows"`
	Cols        int    `json:"cols"`
	TopLeftCell string `json:"topLeftCell"`
	Frozen      bool   `json:"frozen"`
}

// SetFreezeRequest freezes rows and/or columns on a worksheet.
type SetFreezeRequest struct {
	Package     opc.PackageSession
	SheetRef    model.SheetRef
	Rows        int
	Cols        int
	ExpectState string
	HasExpect   bool
}

// ClearFreezeRequest removes the frozen pane from a worksheet.
type ClearFreezeRequest struct {
	Package     opc.PackageSession
	SheetRef    model.SheetRef
	ExpectState string
	HasExpect   bool
}

// FreezeResult reports the freeze state after a mutation (nil State after clear).
type FreezeResult struct {
	State *FreezeState
}

// ---- readback ----

// ReadFreeze returns the worksheet's frozen pane state, or nil if absent.
func ReadFreeze(session opc.PackageSession, sheet model.SheetRef) (*FreezeState, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, err
	}
	return readFreeze(root), nil
}

// readFreeze extracts the frozen pane state from a worksheet root element.
func readFreeze(root *etree.Element) *FreezeState {
	pane := findFreezePane(root)
	if pane == nil {
		return nil
	}
	if pane.SelectAttrValue("state", "") != "frozen" {
		return nil
	}
	return freezeStateFromPane(pane)
}

func freezeStateFromPane(pane *etree.Element) *FreezeState {
	state := &FreezeState{Frozen: true}
	if v := pane.SelectAttrValue("xSplit", ""); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			state.Cols = n
		}
	}
	if v := pane.SelectAttrValue("ySplit", ""); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			state.Rows = n
		}
	}
	state.TopLeftCell = pane.SelectAttrValue("topLeftCell", "")
	if state.TopLeftCell == "" {
		state.TopLeftCell = freezeTopLeftCell(state.Rows, state.Cols)
	}
	return state
}

// findFreezePane locates worksheet > sheetViews > sheetView > pane (first sheetView).
func findFreezePane(root *etree.Element) *etree.Element {
	sheetViews := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetViews")
	if sheetViews == nil {
		return nil
	}
	sheetView := namespaces.FindChild(sheetViews, namespaces.NsSpreadsheetML, "sheetView")
	if sheetView == nil {
		return nil
	}
	return namespaces.FindChild(sheetView, namespaces.NsSpreadsheetML, "pane")
}

// freezeTopLeftCell derives the first visible cell after the frozen panes. Per
// OOXML, topLeftCell = column (cols+1), row (rows+1); ColumnIndexToLetters is
// 1-based, so cols+1 maps cols=1 -> "B".
func freezeTopLeftCell(rows, cols int) string {
	letters, err := address.ColumnIndexToLetters(cols + 1)
	if err != nil {
		return ""
	}
	return fmt.Sprintf("%s%d", letters, rows+1)
}

// ---- mutations ----

// SetFreeze freezes rows and/or columns on the worksheet, creating the
// sheetViews/sheetView/pane chain as needed.
func SetFreeze(req *SetFreezeRequest) (*FreezeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set freeze request is nil")
	}
	if req.Rows < 0 || req.Cols < 0 {
		return nil, fmt.Errorf("--rows and --cols must be >= 0")
	}
	if req.Rows == 0 && req.Cols == 0 {
		return nil, fmt.Errorf("provide at least one of --rows or --cols (>= 1)")
	}
	if req.Rows > address.MaxRow-1 {
		return nil, fmt.Errorf("--rows %d exceeds the maximum freezable rows (%d)", req.Rows, address.MaxRow-1)
	}
	if req.Cols > address.MaxColumn-1 {
		return nil, fmt.Errorf("--cols %d exceeds the maximum freezable columns (%d)", req.Cols, address.MaxColumn-1)
	}

	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if err := guardExpectState(root, req.HasExpect, req.ExpectState); err != nil {
		return nil, err
	}
	state := applyFreeze(root, req.Rows, req.Cols)
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FreezeResult{State: state}, nil
}

// applyFreeze writes the frozen pane into the worksheet root and returns the
// resulting state.
func applyFreeze(root *etree.Element, rows, cols int) *FreezeState {
	prefix := root.Space
	sheetView := ensureSheetView(root, prefix)

	pane := namespaces.FindChild(sheetView, namespaces.NsSpreadsheetML, "pane")
	if pane == nil {
		pane = newElement(prefix, "pane")
		// pane is the first child of sheetView (before selection/extLst).
		insertSheetViewChild(sheetView, pane, "pane")
	}
	// Reset split attributes so a re-freeze does not leave stale values.
	pane.RemoveAttr("xSplit")
	pane.RemoveAttr("ySplit")
	if cols > 0 {
		pane.CreateAttr("xSplit", strconv.Itoa(cols))
	}
	if rows > 0 {
		pane.CreateAttr("ySplit", strconv.Itoa(rows))
	}
	topLeft := freezeTopLeftCell(rows, cols)
	pane.CreateAttr("topLeftCell", topLeft)
	pane.CreateAttr("state", "frozen")

	return &FreezeState{Rows: rows, Cols: cols, TopLeftCell: topLeft, Frozen: true}
}

// ClearFreeze removes the frozen pane from the worksheet.
func ClearFreeze(req *ClearFreezeRequest) (*FreezeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("clear freeze request is nil")
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	if err := guardExpectState(root, req.HasExpect, req.ExpectState); err != nil {
		return nil, err
	}
	if !clearFreeze(root) {
		return nil, ErrNoFreeze
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &FreezeResult{}, nil
}

// clearFreeze removes the frozen pane element; selections are left untouched.
// Returns false when there is no frozen pane to remove.
func clearFreeze(root *etree.Element) bool {
	pane := findFreezePane(root)
	if pane == nil || pane.SelectAttrValue("state", "") != "frozen" {
		return false
	}
	pane.Parent().RemoveChild(pane)
	return true
}

// ensureSheetView returns the first sheetView, creating the sheetViews/sheetView
// chain when absent. A freshly created sheetView carries the required
// workbookViewId="0" attribute.
func ensureSheetView(root *etree.Element, prefix string) *etree.Element {
	sheetViews := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetViews")
	if sheetViews == nil {
		sheetViews = newElement(prefix, "sheetViews")
		insertWorksheetChild(root, sheetViews, "sheetViews")
	}
	sheetView := namespaces.FindChild(sheetViews, namespaces.NsSpreadsheetML, "sheetView")
	if sheetView == nil {
		sheetView = newElement(prefix, "sheetView")
		sheetView.CreateAttr("workbookViewId", "0")
		sheetViews.AddChild(sheetView)
	} else if sheetView.SelectAttrValue("workbookViewId", "") == "" {
		sheetView.CreateAttr("workbookViewId", "0")
	}
	return sheetView
}

// insertSheetViewChild inserts a child of sheetView respecting CT_SheetView
// order: pane < selection < pivotSelection < extLst.
func insertSheetViewChild(sheetView, child *etree.Element, localName string) {
	targetOrder := sheetViewChildOrder(localName)
	for _, existing := range sheetView.ChildElements() {
		if !namespaces.IsElement(existing, namespaces.NsSpreadsheetML, existing.Tag) {
			continue
		}
		if sheetViewChildOrder(existing.Tag) > targetOrder {
			sheetView.InsertChildAt(existing.Index(), child)
			return
		}
	}
	sheetView.AddChild(child)
}

func sheetViewChildOrder(localName string) int {
	switch localName {
	case "pane":
		return 10
	case "selection":
		return 20
	case "pivotSelection":
		return 30
	case "extLst":
		return 40
	default:
		return 1000
	}
}

// guardExpectState enforces the --expect-state guard ("none" or "frozen").
func guardExpectState(root *etree.Element, has bool, expect string) error {
	if !has {
		return nil
	}
	current := "none"
	if readFreeze(root) != nil {
		current = "frozen"
	}
	switch expect {
	case "none", "frozen":
	default:
		return fmt.Errorf("invalid --expect-state %q (use none|frozen)", expect)
	}
	if current != expect {
		return fmt.Errorf("%w: expected %q, found %q", ErrStateMismatch, expect, current)
	}
	return nil
}
