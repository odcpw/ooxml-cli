package mutate

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestInsertRowsShiftsCellsAndPreservesStyles(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, structureWorksheetXML(`
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="2" spans="1:2"><c r="A2"><v>2</v></c><c r="B2" s="7"><v>3</v></c></row>
    <row r="4"><c r="C4"><v>4</v></c></row>
`, "A1:C4", ""))
	defer pkg.Close()

	result, err := InsertRows(&InsertRowsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		At:          2,
		Count:       2,
	})
	if err != nil {
		t.Fatalf("InsertRows returned error: %v", err)
	}
	if result.Axis != "rows" || result.Operation != "insert" || result.Start != 2 || result.Count != 2 || result.ShiftedRows != 2 || result.ShiftedCells != 3 {
		t.Fatalf("unexpected result: %+v", result)
	}

	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	assertDimension(t, doc.Root(), "A1:C6")
	assertCellValue(t, doc.Root(), "A1", "1")
	assertCellValue(t, doc.Root(), "A4", "2")
	assertCellValue(t, doc.Root(), "B4", "3")
	assertCellValue(t, doc.Root(), "C6", "4")
	if style := findCellInDoc(t, doc.Root(), "B4").SelectAttrValue("s", ""); style != "7" {
		t.Fatalf("B4 style = %q, want 7", style)
	}
	if strings.Contains(worksheetXMLString(t, pkg, workbook.Sheets[0].PartURI), `spans=`) {
		t.Fatalf("row spans were not removed after row insert")
	}
}

func TestDeleteRowsRemovesBandAndShiftsLowerRows(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, structureWorksheetXML(`
    <row r="1"><c r="A1"><v>1</v></c></row>
    <row r="2"><c r="A2"><v>2</v></c></row>
    <row r="3"><c r="B3"><v>3</v></c></row>
    <row r="5"><c r="C5"><v>5</v></c></row>
`, "A1:C5", ""))
	defer pkg.Close()

	result, err := DeleteRows(&DeleteRowsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Row:         2,
		Count:       2,
	})
	if err != nil {
		t.Fatalf("DeleteRows returned error: %v", err)
	}
	if result.RemovedRows != 2 || result.RemovedCells != 2 || result.ShiftedRows != 1 || result.ShiftedCells != 1 {
		t.Fatalf("unexpected result: %+v", result)
	}

	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	assertDimension(t, doc.Root(), "A1:C3")
	assertCellValue(t, doc.Root(), "A1", "1")
	assertCellValue(t, doc.Root(), "C3", "5")
	if cell := findOptionalCell(doc.Root(), "A2"); cell != nil {
		t.Fatalf("deleted cell A2 still exists")
	}
	if cell := findOptionalCell(doc.Root(), "B3"); cell != nil {
		t.Fatalf("deleted cell B3 still exists")
	}
}

func TestInsertColumnsShiftsCellsAndPreservesStyles(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, structureWorksheetXML(`
    <row r="1"><c r="A1"><v>1</v></c><c r="B1" s="7"><v>2</v></c><c r="D1"><v>4</v></c></row>
    <row r="2"><c r="C2"><v>3</v></c></row>
`, "A1:D2", ""))
	defer pkg.Close()

	result, err := InsertColumns(&InsertColumnsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		At:          2,
		Count:       2,
	})
	if err != nil {
		t.Fatalf("InsertColumns returned error: %v", err)
	}
	if result.Axis != "cols" || result.Operation != "insert" || result.Start != 2 || result.StartColumn != "B" || result.ShiftedCells != 3 {
		t.Fatalf("unexpected result: %+v", result)
	}

	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	assertDimension(t, doc.Root(), "A1:F2")
	assertCellValue(t, doc.Root(), "A1", "1")
	assertCellValue(t, doc.Root(), "D1", "2")
	assertCellValue(t, doc.Root(), "F1", "4")
	assertCellValue(t, doc.Root(), "E2", "3")
	if style := findCellInDoc(t, doc.Root(), "D1").SelectAttrValue("s", ""); style != "7" {
		t.Fatalf("D1 style = %q, want 7", style)
	}
}

func TestDeleteColumnsRemovesBandAndShiftsRightCells(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, structureWorksheetXML(`
    <row r="1"><c r="A1"><v>1</v></c><c r="B1"><v>2</v></c><c r="C1"><v>3</v></c><c r="E1"><v>5</v></c></row>
    <row r="2"><c r="D2"><v>4</v></c></row>
`, "A1:E2", ""))
	defer pkg.Close()

	result, err := DeleteColumns(&DeleteColumnsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Column:      2,
		Count:       2,
	})
	if err != nil {
		t.Fatalf("DeleteColumns returned error: %v", err)
	}
	if result.StartColumn != "B" || result.RemovedCells != 2 || result.ShiftedCells != 2 {
		t.Fatalf("unexpected result: %+v", result)
	}

	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	assertDimension(t, doc.Root(), "A1:C2")
	assertCellValue(t, doc.Root(), "A1", "1")
	assertCellValue(t, doc.Root(), "C1", "5")
	assertCellValue(t, doc.Root(), "B2", "4")
	if cell := findOptionalCell(doc.Root(), "B1"); cell != nil {
		t.Fatalf("deleted cell B1 still exists")
	}
}

func TestStructureEditsRejectWorksheetHazards(t *testing.T) {
	tests := []struct {
		name         string
		worksheetXML string
		columnEdit   bool
		want         error
	}{
		{
			name: "formulas",
			worksheetXML: structureWorksheetXML(`
    <row r="1"><c r="A1"><f>SUM(B1)</f></c></row>
`, "A1", ""),
			want: ErrWorksheetHasFormulas,
		},
		{
			name:         "merged cells",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>`),
			want:         ErrWorksheetHasMergedCells,
		},
		{
			name:         "tables",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<tableParts count="1"><tablePart r:id="rId1"/></tableParts>`),
			want:         ErrWorksheetHasTables,
		},
		{
			name:         "filters",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<autoFilter ref="A1:B2"/>`),
			want:         ErrWorksheetHasAutofilter,
		},
		{
			name:         "drawings",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<drawing r:id="rId1"/>`),
			want:         ErrWorksheetHasDrawings,
		},
		{
			name:         "hyperlinks",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<hyperlinks><hyperlink ref="A1" r:id="rId1"/></hyperlinks>`),
			want:         ErrWorksheetHasHyperlinks,
		},
		{
			name:         "conditional formatting",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<conditionalFormatting sqref="A1"><cfRule type="expression" priority="1"><formula>TRUE</formula></cfRule></conditionalFormatting>`),
			want:         ErrWorksheetHasConditionalFormatting,
		},
		{
			name:         "data validations",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<dataValidations count="1"><dataValidation sqref="A1" type="whole"/></dataValidations>`),
			want:         ErrWorksheetHasDataValidations,
		},
		{
			name:         "column metadata",
			worksheetXML: structureWorksheetXML(`<row r="1"><c r="A1"><v>1</v></c></row>`, "A1", `<cols><col min="1" max="1" width="12" customWidth="1"/></cols>`),
			columnEdit:   true,
			want:         ErrWorksheetHasColumnMetadata,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			pkg, workbook := openTestWorkbook(t, tt.worksheetXML)
			defer pkg.Close()

			var err error
			if tt.columnEdit {
				_, err = InsertColumns(&InsertColumnsRequest{
					Package:     pkg,
					WorkbookURI: workbook.PartURI,
					SheetRef:    workbook.Sheets[0],
					At:          1,
					Count:       1,
				})
			} else {
				_, err = InsertRows(&InsertRowsRequest{
					Package:     pkg,
					WorkbookURI: workbook.PartURI,
					SheetRef:    workbook.Sheets[0],
					At:          1,
					Count:       1,
				})
			}
			if !errors.Is(err, tt.want) {
				t.Fatalf("error = %v, want %v", err, tt.want)
			}
		})
	}
}

func TestStructureEditsRejectWorkbookHazards(t *testing.T) {
	t.Run("defined names", func(t *testing.T) {
		pkg, workbook := openSheetLifecycleWorkbook(t, false)
		defer pkg.Close()

		_, err := InsertRows(&InsertRowsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    workbook.Sheets[0],
			At:          1,
			Count:       1,
		})
		if !errors.Is(err, ErrWorkbookHasDefinedNames) {
			t.Fatalf("error = %v, want %v", err, ErrWorkbookHasDefinedNames)
		}
	})

	t.Run("calc chain", func(t *testing.T) {
		pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
		defer pkg.Close()
		if err := pkg.AddPart("/xl/calcChain.xml", []byte(fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<calcChain xmlns="%s"><c r="A1" i="1"/></calcChain>`, namespaces.NsSpreadsheetML)), "application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml", nil); err != nil {
			t.Fatalf("AddPart calcChain returned error: %v", err)
		}

		_, err := InsertRows(&InsertRowsRequest{
			Package:     pkg,
			WorkbookURI: workbook.PartURI,
			SheetRef:    workbook.Sheets[0],
			At:          1,
			Count:       1,
		})
		if !errors.Is(err, ErrWorkbookHasCalcChain) {
			t.Fatalf("error = %v, want %v", err, ErrWorkbookHasCalcChain)
		}
	})
}

func TestStructureEditsRejectInvalidReferences(t *testing.T) {
	tests := []struct {
		name         string
		worksheetXML string
	}{
		{
			name: "missing row reference",
			worksheetXML: `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>`,
		},
		{
			name: "missing cell reference",
			worksheetXML: `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c><v>1</v></c></row></sheetData>
</worksheet>`,
		},
		{
			name: "row cell mismatch",
			worksheetXML: `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="2"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>`,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			pkg, workbook := openTestWorkbook(t, tt.worksheetXML)
			defer pkg.Close()

			_, err := InsertRows(&InsertRowsRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				SheetRef:    workbook.Sheets[0],
				At:          1,
				Count:       1,
			})
			if !errors.Is(err, ErrWorksheetHasInvalidReferences) {
				t.Fatalf("error = %v, want %v", err, ErrWorksheetHasInvalidReferences)
			}
		})
	}
}

func TestStructureEditsRejectOutOfBounds(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	if _, err := InsertRows(&InsertRowsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		At:          address.MaxRow,
		Count:       2,
	}); !errors.Is(err, ErrWorksheetStructureOutOfBounds) {
		t.Fatalf("InsertRows error = %v, want out-of-bounds", err)
	}

	if _, err := InsertColumns(&InsertColumnsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		At:          address.MaxColumn,
		Count:       2,
	}); !errors.Is(err, ErrWorksheetStructureOutOfBounds) {
		t.Fatalf("InsertColumns error = %v, want out-of-bounds", err)
	}
}

func structureWorksheetXML(rows, dimension, extras string) string {
	dimensionXML := ""
	if dimension != "" {
		dimensionXML = fmt.Sprintf(`<dimension ref="%s"/>`, dimension)
	}
	return fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="%s" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  %s
  <sheetData>
%s
  </sheetData>
  %s
</worksheet>`, namespaces.NsSpreadsheetML, dimensionXML, rows, extras)
}

func assertDimension(t *testing.T, root *etree.Element, want string) {
	t.Helper()
	elem := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dimension")
	if elem == nil {
		t.Fatalf("dimension element missing, want %s", want)
	}
	if got := elem.SelectAttrValue("ref", ""); got != want {
		t.Fatalf("dimension ref = %q, want %q", got, want)
	}
}

func assertCellValue(t *testing.T, root *etree.Element, ref, want string) {
	t.Helper()
	cell := findCellInDoc(t, root, ref)
	if got := childText(cell, "v"); got != want {
		t.Fatalf("%s value = %q, want %q", ref, got, want)
	}
}

func worksheetXMLString(t *testing.T, pkg interface{ ReadRawPart(string) ([]byte, error) }, uri string) string {
	t.Helper()
	raw, err := pkg.ReadRawPart(uri)
	if err != nil {
		t.Fatalf("ReadRawPart returned error: %v", err)
	}
	return string(raw)
}
