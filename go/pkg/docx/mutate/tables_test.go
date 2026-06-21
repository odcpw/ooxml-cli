package mutate

import (
	"errors"
	"testing"

	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestSetTableCellTextEncodesAndReadsBack(t *testing.T) {
	pkg, documentURI := openFixture(t, "table")
	defer pkg.Close()

	want := " lead\tmid\nnext "
	result, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    1,
		ColumnIndex: 2,
		Text:        want,
	})
	if err != nil {
		t.Fatalf("SetTableCellText returned error: %v", err)
	}
	if result.PreviousText != "B1" || result.Text != want || result.Flattened {
		t.Fatalf("unexpected set-cell result: %+v", result)
	}

	extracted := extractText(t, pkg, documentURI)
	table := extracted.Blocks[0].Table
	if got := table.Rows[0].Cells[1]; got != want {
		t.Fatalf("cell readback = %q, want %q", got, want)
	}
}

func TestSetTableCellTextUsesExpectedHash(t *testing.T) {
	pkg, documentURI := openFixture(t, "table")
	defer pkg.Close()

	hash := tableBlockHashForTest(t, pkg, documentURI)
	result, err := SetTableCellText(&SetTableCellTextRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		TableIndex:   1,
		ExpectedHash: hash,
		RowIndex:     1,
		ColumnIndex:  1,
		Text:         "Guarded",
	})
	if err != nil {
		t.Fatalf("SetTableCellText returned error: %v", err)
	}
	if result.PreviousHash != hash || result.ContentHash == hash || result.BlockIndex != 1 {
		t.Fatalf("unexpected hash metadata: %+v", result)
	}

	_, err = SetTableCellText(&SetTableCellTextRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		TableIndex:   1,
		ExpectedHash: hash,
		RowIndex:     1,
		ColumnIndex:  1,
		Text:         "Stale",
	})
	if !errors.Is(err, ErrBlockHashMismatch) {
		t.Fatalf("stale hash error = %v, want ErrBlockHashMismatch", err)
	}
}

func TestClearTableCellText(t *testing.T) {
	pkg, documentURI := openFixture(t, "table")
	defer pkg.Close()

	result, err := ClearTableCellText(&ClearTableCellTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    2,
		ColumnIndex: 1,
	})
	if err != nil {
		t.Fatalf("ClearTableCellText returned error: %v", err)
	}
	if result.PreviousText != "A2" {
		t.Fatalf("previous text = %q, want A2", result.PreviousText)
	}
	extracted := extractText(t, pkg, documentURI)
	if got := extracted.Blocks[0].Table.Rows[1].Cells[0]; got != "" {
		t.Fatalf("cleared cell readback = %q, want empty", got)
	}
}

func tableBlockHashForTest(t *testing.T, pkg *opc.Package, documentURI string) string {
	t.Helper()
	result, err := extract.ExtractBlocks(&extract.ExtractBlocksRequest{
		Session:     pkg,
		DocumentURI: documentURI,
		Block:       1,
	})
	if err != nil {
		t.Fatalf("ExtractBlocks returned error: %v", err)
	}
	if len(result.Blocks) != 1 {
		t.Fatalf("block count = %d, want 1", len(result.Blocks))
	}
	return result.Blocks[0].ContentHash
}

func TestInsertAndDeleteTableRow(t *testing.T) {
	pkg, documentURI := openFixture(t, "table")
	defer pkg.Close()

	inserted, err := InsertTableRow(&InsertTableRowRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		At:          2,
	})
	if err != nil {
		t.Fatalf("InsertTableRow returned error: %v", err)
	}
	if inserted.Rows != 3 || inserted.Cols != 2 || inserted.RowIndex != 2 {
		t.Fatalf("unexpected insert result: %+v", inserted)
	}
	extracted := extractText(t, pkg, documentURI)
	if got := extracted.Blocks[0].Table.Rows[1].Cells; len(got) != 2 || got[0] != "" || got[1] != "" {
		t.Fatalf("inserted row cells = %+v, want two empty cells", got)
	}

	deleted, err := DeleteTableRow(&DeleteTableRowRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    2,
	})
	if err != nil {
		t.Fatalf("DeleteTableRow returned error: %v", err)
	}
	if deleted.Rows != 2 || deleted.Cols != 2 || deleted.RowIndex != 2 {
		t.Fatalf("unexpected delete result: %+v", deleted)
	}
	extracted = extractText(t, pkg, documentURI)
	if got := extracted.Blocks[0].Table.Rows[1].Cells[0]; got != "A2" {
		t.Fatalf("row after delete = %q, want A2", got)
	}
}

func TestTableMutationsAddRequiredTableScaffold(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(t *testing.T, pkg *opc.Package, documentURI string)
	}{
		{
			name: "set-cell",
			mutate: func(t *testing.T, pkg *opc.Package, documentURI string) {
				t.Helper()
				if _, err := SetTableCellText(&SetTableCellTextRequest{
					Package:     pkg,
					DocumentURI: documentURI,
					TableIndex:  1,
					RowIndex:    1,
					ColumnIndex: 1,
					Text:        "Scaffolded",
				}); err != nil {
					t.Fatalf("SetTableCellText returned error: %v", err)
				}
			},
		},
		{
			name: "insert-row",
			mutate: func(t *testing.T, pkg *opc.Package, documentURI string) {
				t.Helper()
				if _, err := InsertTableRow(&InsertTableRowRequest{
					Package:     pkg,
					DocumentURI: documentURI,
					TableIndex:  1,
					At:          2,
				}); err != nil {
					t.Fatalf("InsertTableRow returned error: %v", err)
				}
			},
		},
		{
			name: "delete-row",
			mutate: func(t *testing.T, pkg *opc.Package, documentURI string) {
				t.Helper()
				if _, err := DeleteTableRow(&DeleteTableRowRequest{
					Package:     pkg,
					DocumentURI: documentURI,
					TableIndex:  1,
					RowIndex:    2,
				}); err != nil {
					t.Fatalf("DeleteTableRow returned error: %v", err)
				}
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			pkg, documentURI := openFixture(t, "table")
			defer pkg.Close()

			tt.mutate(t, pkg, documentURI)
			assertTableScaffold(t, pkg, documentURI)
		})
	}
}

func assertTableScaffold(t *testing.T, pkg *opc.Package, documentURI string) {
	t.Helper()
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	tables := namespaces.FindDescendants(doc.Root(), namespaces.NsW, "tbl")
	if len(tables) != 1 {
		t.Fatalf("table count = %d, want 1", len(tables))
	}
	children := tables[0].ChildElements()
	if len(children) < 3 {
		t.Fatalf("table child element count = %d, want at least 3", len(children))
	}
	if docxbody.LocalName(children[0].Tag) != "tblPr" || docxbody.LocalName(children[1].Tag) != "tblGrid" || docxbody.LocalName(children[2].Tag) != "tr" {
		t.Fatalf("table children begin with %s, %s, %s; want tblPr, tblGrid, tr", children[0].Tag, children[1].Tag, children[2].Tag)
	}
	if cols := len(namespaces.FindChildren(children[1], namespaces.NsW, "gridCol")); cols != 2 {
		t.Fatalf("gridCol count = %d, want 2", cols)
	}
}

func TestSetTableCellTextScaffoldsGridSpanWidth(t *testing.T) {
	pkg, documentURI := openFixture(t, "merged-table")
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	tables := namespaces.FindDescendants(doc.Root(), namespaces.NsW, "tbl")
	if len(tables) != 1 {
		t.Fatalf("table count = %d, want 1", len(tables))
	}
	rows := namespaces.FindChildren(tables[0], namespaces.NsW, "tr")
	if len(rows) < 2 {
		t.Fatalf("fixture table rows = %d, want at least 2", len(rows))
	}
	for _, row := range rows[1:] {
		tables[0].RemoveChild(row)
	}
	if err := pkg.ReplaceXMLPart(documentURI, doc); err != nil {
		t.Fatalf("ReplaceXMLPart returned error: %v", err)
	}

	if _, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    1,
		ColumnIndex: 1,
		Text:        "Still merged",
	}); err != nil {
		t.Fatalf("SetTableCellText returned error: %v", err)
	}

	doc, err = pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	tables = namespaces.FindDescendants(doc.Root(), namespaces.NsW, "tbl")
	tblGrid := namespaces.FindChild(tables[0], namespaces.NsW, "tblGrid")
	if tblGrid == nil {
		t.Fatal("table has no tblGrid")
	}
	if cols := len(namespaces.FindChildren(tblGrid, namespaces.NsW, "gridCol")); cols != 2 {
		t.Fatalf("gridCol count = %d, want 2 for gridSpan width", cols)
	}
}

func TestTableRowMutationRejectsMergedCells(t *testing.T) {
	pkg, documentURI := openFixture(t, "merged-table")
	defer pkg.Close()

	_, err := InsertTableRow(&InsertTableRowRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		At:          2,
	})
	if !errors.Is(err, ErrTableHasMergedCells) {
		t.Fatalf("insert merged table error = %v, want ErrTableHasMergedCells", err)
	}

	_, err = DeleteTableRow(&DeleteTableRowRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    1,
	})
	if !errors.Is(err, ErrTableHasMergedCells) {
		t.Fatalf("delete merged table error = %v, want ErrTableHasMergedCells", err)
	}
}

func TestTableMutationErrors(t *testing.T) {
	pkg, documentURI := openFixture(t, "mixed-blocks")
	defer pkg.Close()

	_, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  2,
		RowIndex:    1,
		ColumnIndex: 1,
		Text:        "missing",
	})
	if !errors.Is(err, ErrTableIndexOutOfRange) {
		t.Fatalf("missing table error = %v, want ErrTableIndexOutOfRange", err)
	}

	_, err = SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    1,
		ColumnIndex: 99,
		Text:        "missing",
	})
	if !errors.Is(err, ErrTableCellOutOfRange) {
		t.Fatalf("missing cell error = %v, want ErrTableCellOutOfRange", err)
	}
}

func TestTableCellSetPreservesCellProperties(t *testing.T) {
	pkg, documentURI := openFixture(t, "merged-table")
	defer pkg.Close()

	_, err := SetTableCellText(&SetTableCellTextRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		TableIndex:  1,
		RowIndex:    1,
		ColumnIndex: 1,
		Text:        "Still merged",
	})
	if err != nil {
		t.Fatalf("SetTableCellText returned error: %v", err)
	}
	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	if len(namespaces.FindDescendants(doc.Root(), namespaces.NsW, "gridSpan")) != 1 {
		t.Fatalf("expected gridSpan to be preserved")
	}
	extracted, err := extract.ExtractText(&extract.ExtractTextRequest{
		Session:     pkg,
		DocumentURI: documentURI,
	})
	if err != nil {
		t.Fatalf("ExtractText returned error: %v", err)
	}
	if extracted.Blocks[0].Kind != model.BlockKindTable || extracted.Blocks[0].Table.Rows[0].Cells[0] != "Still merged" {
		t.Fatalf("unexpected merged-cell readback: %+v", extracted.Blocks[0])
	}
}
