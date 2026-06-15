package mutate

import (
	"archive/zip"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	xlsxtable "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/table"
)

func TestAppendTableRows(t *testing.T) {
	path := writeTableWorkbookForMutateTest(t, "A1:B3", false, "")
	pkg, workbook, tableRef := openTableWorkbookForMutateTest(t, path)
	defer pkg.Close()

	result, err := AppendTableRows(&AppendTableRowsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		Table:       tableRef,
		Rows: [][]RangeCell{
			{{Type: CellValueString, Value: "North"}, {Type: CellValueNumber, Value: "30"}},
			{{Type: CellValueString, Value: "South"}, {Type: CellValueNumber, Value: "40"}},
		},
		NullPolicy: RangeNullSkip,
	})
	if err != nil {
		t.Fatalf("AppendTableRows returned error: %v", err)
	}
	if result.PreviousRange != "A1:B3" || result.Range != "A1:B5" || result.AppendRange != "A4:B5" {
		t.Fatalf("unexpected append result: %+v", result)
	}

	tableRaw, err := pkg.ReadRawPart("/xl/tables/table1.xml")
	if err != nil {
		t.Fatalf("failed to read table part: %v", err)
	}
	if !strings.Contains(string(tableRaw), `ref="A1:B5"`) {
		t.Fatalf("table ref was not expanded:\n%s", string(tableRaw))
	}
	sheetRaw, err := pkg.ReadRawPart("/xl/worksheets/sheet1.xml")
	if err != nil {
		t.Fatalf("failed to read worksheet part: %v", err)
	}
	for _, want := range []string{`r="A4"`, `North`, `r="B5"`, `<v>40</v>`} {
		if !strings.Contains(string(sheetRaw), want) {
			t.Fatalf("worksheet missing %q:\n%s", want, string(sheetRaw))
		}
	}
}

func TestAppendTableRowsRejectsOverwriteAndTotals(t *testing.T) {
	overwritePath := writeTableWorkbookForMutateTest(t, "A1:B3", false, `<row r="4"><c r="A4" t="inlineStr"><is><t>occupied</t></is></c></row>`)
	pkg, workbook, tableRef := openTableWorkbookForMutateTest(t, overwritePath)
	defer pkg.Close()

	_, err := AppendTableRows(&AppendTableRowsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		Table:       tableRef,
		Rows:        [][]RangeCell{{{Type: CellValueString, Value: "North"}, {Type: CellValueNumber, Value: "30"}}},
		NullPolicy:  RangeNullSkip,
	})
	if err == nil || !strings.Contains(err.Error(), ErrTableAppendWouldOverwrite.Error()) {
		t.Fatalf("overwrite error = %v, want ErrTableAppendWouldOverwrite", err)
	}

	totalsPath := writeTableWorkbookForMutateTest(t, "A1:B4", true, "")
	pkg, workbook, tableRef = openTableWorkbookForMutateTest(t, totalsPath)
	defer pkg.Close()
	_, err = AppendTableRows(&AppendTableRowsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		Table:       tableRef,
		Rows:        [][]RangeCell{{{Type: CellValueString, Value: "North"}, {Type: CellValueNumber, Value: "30"}}},
		NullPolicy:  RangeNullSkip,
	})
	if err == nil || !strings.Contains(err.Error(), ErrTableHasTotals.Error()) {
		t.Fatalf("totals error = %v, want ErrTableHasTotals", err)
	}
}

func openTableWorkbookForMutateTest(t *testing.T, path string) (*opc.Package, *model.Workbook, model.TableRef) {
	t.Helper()
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open workbook: %v", err)
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("failed to parse workbook: %v", err)
	}
	tables, err := xlsxtable.List(pkg, workbook, workbook.Sheets)
	if err != nil {
		pkg.Close()
		t.Fatalf("failed to list tables: %v", err)
	}
	if len(tables) != 1 {
		pkg.Close()
		t.Fatalf("tables len = %d, want 1", len(tables))
	}
	return pkg, workbook, tables[0]
}

func writeTableWorkbookForMutateTest(t *testing.T, tableRef string, totals bool, extraRows string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "table-workbook.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create xlsx: %v", err)
	}
	defer file.Close()

	totalsAttrs := `totalsRowShown="0"`
	totalsRow := ""
	if totals {
		totalsAttrs = `totalsRowShown="1" totalsRowCount="1"`
		totalsRow = `<row r="4"><c r="A4" t="inlineStr"><is><t>Total</t></is></c><c r="B4"><v>30</v></c></row>`
	}
	dimension := tableRef
	if extraRows != "" && !strings.Contains(dimension, "B4") {
		dimension = "A1:B4"
	}

	zw := zip.NewWriter(file)
	addTestZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>`)
	addTestZipFile(t, zw, "_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/workbook.xml", `<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Data" sheetId="1" r:id="rId1"/></sheets>
</workbook>`)
	addTestZipFile(t, zw, "xl/_rels/workbook.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/worksheets/sheet1.xml", tableWorksheetXMLForMutateTest(dimension, totalsRow, extraRows))
	addTestZipFile(t, zw, "xl/worksheets/_rels/sheet1.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/tables/table1.xml", tablePartXMLForMutateTest(tableRef, totalsAttrs))
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close xlsx: %v", err)
	}
	return path
}

func tableWorksheetXMLForMutateTest(dimension, totalsRow, extraRows string) string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="` + dimension + `"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2"><v>10</v></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3"><v>20</v></c></row>
    ` + totalsRow + `
    ` + extraRows + `
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>`
}

func tablePartXMLForMutateTest(tableRef, totalsAttrs string) string {
	return `<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="` + tableRef + `" headerRowCount="1" ` + totalsAttrs + `>
  <autoFilter ref="` + tableRef + `"/>
  <tableColumns count="2">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Amount"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>`
}
