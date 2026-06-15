package mutate

import (
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func TestSetTableCellTextUpdatesTargetCell(t *testing.T) {
	pkg, slideRef := openTableFixture(t)
	defer pkg.Close()

	result, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		SlideRef:    slideRef,
		TableID:     2,
		RowIndex:    1,
		ColumnIndex: 1,
		Text:        "Updated Cell",
	})
	if err != nil {
		t.Fatalf("SetTableCellText returned error: %v", err)
	}
	if result.PreviousText != "R1C1" || result.Text != "Updated Cell" {
		t.Fatalf("unexpected set-cell result: %+v", result)
	}

	table := readTableInfo(t, pkg, slideRef, 2)
	if got := table.Cells[1][1]; got != "Updated Cell" {
		t.Fatalf("cell readback = %q, want Updated Cell", got)
	}
	if got := table.Cells[1][0]; got != "R1C0" {
		t.Fatalf("neighbor cell changed: %q", got)
	}
}

func TestSetTableCellTextEncodesLineBreaksAndPreservedSpaces(t *testing.T) {
	pkg, slideRef := openTableFixture(t)
	defer pkg.Close()

	want := " lead\nnext "
	_, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		SlideRef:    slideRef,
		TableID:     2,
		RowIndex:    0,
		ColumnIndex: 0,
		Text:        want,
	})
	if err != nil {
		t.Fatalf("SetTableCellText returned error: %v", err)
	}

	table := readTableInfo(t, pkg, slideRef, 2)
	if got := table.Cells[0][0]; got != want {
		t.Fatalf("cell readback = %q, want %q", got, want)
	}

	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	slideXML, err := slideDoc.WriteToString()
	if err != nil {
		t.Fatalf("WriteToString returned error: %v", err)
	}
	if !strings.Contains(slideXML, `xml:space="preserve"`) {
		t.Fatalf("expected xml:space preservation in updated cell")
	}
}

func TestSetTableCellTextRejectsBadTargets(t *testing.T) {
	pkg, slideRef := openTableFixture(t)
	defer pkg.Close()

	tests := []SetTableCellTextRequest{
		{Package: pkg, SlideRef: slideRef, TableID: 99, RowIndex: 0, ColumnIndex: 0, Text: "x"},
		{Package: pkg, SlideRef: slideRef, TableID: 2, RowIndex: 99, ColumnIndex: 0, Text: "x"},
		{Package: pkg, SlideRef: slideRef, TableID: 2, RowIndex: 0, ColumnIndex: 99, Text: "x"},
	}
	for _, req := range tests {
		_, err := SetTableCellText(&req)
		if err == nil {
			t.Fatalf("expected error for request %+v", req)
		}
	}

	_, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		SlideRef:    slideRef,
		TableID:     2,
		RowIndex:    -1,
		ColumnIndex: 0,
		Text:        "x",
	})
	if err == nil {
		t.Fatalf("unexpected negative row error: %v", err)
	}
}

func TestSetTableTextMatrixUpdatesAllCells(t *testing.T) {
	pkg, slideRef := openTableFixture(t)
	defer pkg.Close()

	data := [][]string{
		{"A", "B", "C"},
		{"D", "R1C1", "F"},
		{"G", "H", "I"},
	}
	result, err := SetTableTextMatrix(&SetTableTextMatrixRequest{
		Package:  pkg,
		SlideRef: slideRef,
		TableID:  2,
		Data:     data,
	})
	if err != nil {
		t.Fatalf("SetTableTextMatrix returned error: %v", err)
	}
	if result.TableID != 2 || result.Rows != 3 || result.Cols != 3 || result.UpdatedCells != 9 || result.ChangedCells != 8 {
		t.Fatalf("unexpected matrix update result: %+v", result)
	}

	table := readTableInfo(t, pkg, slideRef, 2)
	for rowIndex, row := range data {
		for colIndex, want := range row {
			if got := table.Cells[rowIndex][colIndex]; got != want {
				t.Fatalf("cell R%dC%d = %q, want %q", rowIndex, colIndex, got, want)
			}
		}
	}
}

func TestSetTableTextMatrixRejectsUnsafeInputs(t *testing.T) {
	pkg, slideRef := openTableFixture(t)
	defer pkg.Close()

	tests := []struct {
		name string
		data [][]string
		want string
	}{
		{
			name: "row mismatch",
			data: [][]string{{"A", "B", "C"}, {"D", "E", "F"}},
			want: "dimension mismatch",
		},
		{
			name: "column mismatch",
			data: [][]string{{"A", "B"}, {"C", "D"}, {"E", "F"}},
			want: "dimension mismatch",
		},
		{
			name: "ragged source",
			data: [][]string{{"A", "B"}, {"C"}},
			want: "rectangular",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := SetTableTextMatrix(&SetTableTextMatrixRequest{
				Package:  pkg,
				SlideRef: slideRef,
				TableID:  2,
				Data:     tt.data,
			})
			if err == nil || !strings.Contains(err.Error(), tt.want) {
				t.Fatalf("error = %v, want containing %q", err, tt.want)
			}
		})
	}
}

func TestSetTableTextMatrixRejectsMergedTables(t *testing.T) {
	pkg, slideRef := openTableFixtureInDir(t, "table-merged")
	defer pkg.Close()

	_, err := SetTableTextMatrix(&SetTableTextMatrixRequest{
		Package:  pkg,
		SlideRef: slideRef,
		TableID:  2,
		Data:     [][]string{{"A"}},
	})
	if err == nil || !strings.Contains(err.Error(), "merged cells") {
		t.Fatalf("error = %v, want merged cells rejection", err)
	}
}

func openTableFixture(t *testing.T) (*opc.Package, *inspect.SlideRef) {
	return openTableFixtureInDir(t, "table-slide")
}

func openTableFixtureInDir(t *testing.T, fixtureDir string) (*opc.Package, *inspect.SlideRef) {
	t.Helper()
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "pptx", fixtureDir, "presentation.pptx"))
	if err != nil {
		t.Fatalf("opc.Open returned error: %v", err)
	}
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("ParsePresentation returned error: %v", err)
	}
	if len(graph.Slides) < 2 {
		pkg.Close()
		t.Fatalf("fixture has %d slides, want at least 2", len(graph.Slides))
	}
	return pkg, &graph.Slides[1]
}

func readTableInfo(t *testing.T, pkg opc.PackageSession, slideRef *inspect.SlideRef, tableID int) *model.TableInfo {
	t.Helper()
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	tbl, err := findTableInSlide(slideDoc.Root(), tableID)
	if err != nil {
		t.Fatalf("findTableInSlide returned error: %v", err)
	}
	if tbl == nil {
		t.Fatalf("table %d not found", tableID)
	}
	table := inspect.ParseTable(tbl)
	if table == nil {
		t.Fatal("ParseTable returned nil")
	}
	return table
}
