package mutate

import (
	"errors"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// newWorksheetRoot builds a minimal <worksheet> root with a <sheetData> child,
// mirroring the structure of a freshly authored worksheet part.
func newWorksheetRoot() (*etree.Document, *etree.Element) {
	doc := etree.NewDocument()
	root := doc.CreateElement("worksheet")
	root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	root.CreateElement("sheetData")
	return doc, root
}

func TestReadFreezeBlankSheet(t *testing.T) {
	_, root := newWorksheetRoot()
	if state := readFreeze(root); state != nil {
		t.Fatalf("expected nil freeze state on blank sheet, got %+v", state)
	}
}

func TestApplyFreezeRowsAndCols(t *testing.T) {
	_, root := newWorksheetRoot()
	state := applyFreeze(root, 1, 1)
	if state == nil || state.Rows != 1 || state.Cols != 1 {
		t.Fatalf("unexpected state: %+v", state)
	}
	if state.TopLeftCell != "B2" {
		t.Fatalf("expected topLeftCell B2, got %q", state.TopLeftCell)
	}
	if !state.Frozen {
		t.Fatalf("expected frozen true")
	}

	pane := findFreezePane(root)
	if pane == nil {
		t.Fatalf("expected pane element")
	}
	if pane.SelectAttrValue("xSplit", "") != "1" || pane.SelectAttrValue("ySplit", "") != "1" {
		t.Fatalf("unexpected split attrs: %+v", pane.Attr)
	}
	if pane.SelectAttrValue("state", "") != "frozen" {
		t.Fatalf("expected state=frozen")
	}
	if pane.SelectAttrValue("topLeftCell", "") != "B2" {
		t.Fatalf("expected topLeftCell B2 attr")
	}

	// sheetView must carry the required workbookViewId.
	sheetViews := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetViews")
	sheetView := namespaces.FindChild(sheetViews, namespaces.NsSpreadsheetML, "sheetView")
	if sheetView.SelectAttrValue("workbookViewId", "") != "0" {
		t.Fatalf("expected workbookViewId=0 on sheetView")
	}

	// sheetViews must come before sheetData (worksheet child order).
	if namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetViews").Index() >=
		namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetData").Index() {
		t.Fatalf("sheetViews must precede sheetData")
	}

	// Round-trips via readFreeze.
	rb := readFreeze(root)
	if rb == nil || rb.Rows != 1 || rb.Cols != 1 || rb.TopLeftCell != "B2" {
		t.Fatalf("readback mismatch: %+v", rb)
	}
}

func TestApplyFreezeRowsOnly(t *testing.T) {
	_, root := newWorksheetRoot()
	state := applyFreeze(root, 2, 0)
	if state.Rows != 2 || state.Cols != 0 {
		t.Fatalf("unexpected state: %+v", state)
	}
	if state.TopLeftCell != "A3" {
		t.Fatalf("expected topLeftCell A3, got %q", state.TopLeftCell)
	}
	pane := findFreezePane(root)
	if pane.SelectAttrValue("xSplit", "") != "" {
		t.Fatalf("expected no xSplit attr when cols=0, got %q", pane.SelectAttrValue("xSplit", ""))
	}
	if pane.SelectAttrValue("ySplit", "") != "2" {
		t.Fatalf("expected ySplit=2, got %q", pane.SelectAttrValue("ySplit", ""))
	}
}

func TestApplyFreezeColsOnly(t *testing.T) {
	_, root := newWorksheetRoot()
	state := applyFreeze(root, 0, 3)
	if state.Rows != 0 || state.Cols != 3 {
		t.Fatalf("unexpected state: %+v", state)
	}
	if state.TopLeftCell != "D1" {
		t.Fatalf("expected topLeftCell D1, got %q", state.TopLeftCell)
	}
	pane := findFreezePane(root)
	if pane.SelectAttrValue("ySplit", "") != "" {
		t.Fatalf("expected no ySplit attr when rows=0")
	}
	if pane.SelectAttrValue("xSplit", "") != "3" {
		t.Fatalf("expected xSplit=3")
	}
}

func TestApplyFreezeReplacesStaleSplit(t *testing.T) {
	_, root := newWorksheetRoot()
	applyFreeze(root, 2, 2)
	state := applyFreeze(root, 1, 0)
	if state.Cols != 0 || state.Rows != 1 {
		t.Fatalf("unexpected state: %+v", state)
	}
	pane := findFreezePane(root)
	if pane.SelectAttrValue("xSplit", "") != "" {
		t.Fatalf("expected stale xSplit removed on re-freeze")
	}
	// Only one pane should exist.
	sheetViews := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetViews")
	sheetView := namespaces.FindChild(sheetViews, namespaces.NsSpreadsheetML, "sheetView")
	if got := len(namespaces.FindChildren(sheetView, namespaces.NsSpreadsheetML, "pane")); got != 1 {
		t.Fatalf("expected exactly 1 pane, got %d", got)
	}
}

func TestClearFreeze(t *testing.T) {
	_, root := newWorksheetRoot()
	applyFreeze(root, 1, 1)
	if !clearFreeze(root) {
		t.Fatalf("expected clearFreeze to report removal")
	}
	if readFreeze(root) != nil {
		t.Fatalf("expected no freeze state after clear")
	}
	if findFreezePane(root) != nil {
		t.Fatalf("expected pane removed")
	}
	// sheetView is retained.
	sheetViews := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheetViews")
	if namespaces.FindChild(sheetViews, namespaces.NsSpreadsheetML, "sheetView") == nil {
		t.Fatalf("expected sheetView retained after clear")
	}
}

func TestClearFreezeNoPane(t *testing.T) {
	_, root := newWorksheetRoot()
	if clearFreeze(root) {
		t.Fatalf("expected clearFreeze to report nothing removed on blank sheet")
	}
}

func TestPaneIsFirstChildOfSheetView(t *testing.T) {
	_, root := newWorksheetRoot()
	// Pre-existing sheetView with a selection child.
	sheetViews := newElement("", "sheetViews")
	insertWorksheetChild(root, sheetViews, "sheetViews")
	sheetView := newElement("", "sheetView")
	sheetView.CreateAttr("workbookViewId", "0")
	sheetViews.AddChild(sheetView)
	sel := newElement("", "selection")
	sheetView.AddChild(sel)

	applyFreeze(root, 1, 1)
	children := sheetView.ChildElements()
	if children[0].Tag != "pane" {
		t.Fatalf("expected pane to be first child of sheetView, got %q", children[0].Tag)
	}
	if children[1].Tag != "selection" {
		t.Fatalf("expected selection to follow pane, got %q", children[1].Tag)
	}
}

func TestGuardExpectState(t *testing.T) {
	_, root := newWorksheetRoot()

	// Blank sheet: expect none -> ok; expect frozen -> mismatch.
	if err := guardExpectState(root, true, "none"); err != nil {
		t.Fatalf("expect none on blank sheet should pass: %v", err)
	}
	if err := guardExpectState(root, true, "frozen"); !errors.Is(err, ErrStateMismatch) {
		t.Fatalf("expect frozen on blank sheet should be ErrStateMismatch, got %v", err)
	}

	applyFreeze(root, 1, 1)
	if err := guardExpectState(root, true, "frozen"); err != nil {
		t.Fatalf("expect frozen on frozen sheet should pass: %v", err)
	}
	if err := guardExpectState(root, true, "none"); !errors.Is(err, ErrStateMismatch) {
		t.Fatalf("expect none on frozen sheet should be ErrStateMismatch, got %v", err)
	}

	// Invalid expect value.
	if err := guardExpectState(root, true, "bogus"); err == nil {
		t.Fatalf("expected error for invalid expect-state value")
	}

	// Not set -> always ok.
	if err := guardExpectState(root, false, ""); err != nil {
		t.Fatalf("guard should be no-op when not set: %v", err)
	}
}

func TestFreezeTopLeftCell(t *testing.T) {
	cases := []struct {
		rows, cols int
		want       string
	}{
		{1, 1, "B2"},
		{2, 0, "A3"},
		{0, 3, "D1"},
		{3, 5, "F4"},
	}
	for _, c := range cases {
		if got := freezeTopLeftCell(c.rows, c.cols); got != c.want {
			t.Fatalf("freezeTopLeftCell(%d,%d) = %q, want %q", c.rows, c.cols, got, c.want)
		}
	}
}
