package mutate

import (
	"archive/zip"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
)

func TestSetCellCreatesSortedCellAndRoundTrips(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	result, err := SetCell(&SetCellRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cell:        "b1",
		Value:       "42.50",
		Type:        CellValueNumber,
	})
	if err != nil {
		t.Fatalf("SetCell returned error: %v", err)
	}
	if !result.Created || result.Ref != "B1" || result.Value != "42.50" {
		t.Fatalf("unexpected result: %+v", result)
	}

	report := readMutatedSheet(t, pkg, workbook)
	if report.UsedRange.Ref != "A1:C1" {
		t.Fatalf("used range = %q, want A1:C1", report.UsedRange.Ref)
	}
	cells := report.Rows[0].Cells
	if len(cells) != 3 || cells[1].Ref != "B1" || cells[1].Type != model.CellTypeNumber || cells[1].Value != "42.50" {
		t.Fatalf("unexpected cells: %+v", cells)
	}
}

func TestSetCellPreservesStyleAndNumericLiteral(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1"><c r="A1" s="7"><v>1</v></c></row>
  </sheetData>
</worksheet>`)
	defer pkg.Close()

	_, err := SetCell(&SetCellRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cell:        "A1",
		Value:       "12345678901234567",
		Type:        CellValueNumber,
	})
	if err != nil {
		t.Fatalf("SetCell returned error: %v", err)
	}

	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	cell := namespaces.FindDescendants(doc.Root(), namespaces.NsSpreadsheetML, "c")[0]
	if cell.SelectAttrValue("s", "") != "7" {
		t.Fatalf("style attr = %q, want 7", cell.SelectAttrValue("s", ""))
	}
	value := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "v")
	if value == nil || value.Text() != "12345678901234567" {
		t.Fatalf("value element = %#v, want numeric literal", value)
	}
}

func TestSetFormulaMarksWorkbookForRecalc(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	_, err := SetCell(&SetCellRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cell:        "D4",
		Value:       "=SUM(A1:C1)",
		Type:        CellValueFormula,
	})
	if err != nil {
		t.Fatalf("SetCell returned error: %v", err)
	}

	sheetDoc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart sheet returned error: %v", err)
	}
	cell := findCellInDoc(t, sheetDoc.Root(), "D4")
	formula := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "f")
	if formula == nil || formula.Text() != "SUM(A1:C1)" {
		t.Fatalf("formula = %#v, want SUM(A1:C1)", formula)
	}
	if value := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "v"); value != nil {
		t.Fatalf("formula cell has cached value: %#v", value)
	}

	workbookDoc, err := pkg.ReadXMLPart(workbook.PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart workbook returned error: %v", err)
	}
	calcPr := namespaces.FindChild(workbookDoc.Root(), namespaces.NsSpreadsheetML, "calcPr")
	if calcPr == nil || calcPr.SelectAttrValue("fullCalcOnLoad", "") != "1" || calcPr.SelectAttrValue("forceFullCalc", "") != "1" {
		t.Fatalf("calcPr = %#v, want recalc flags", calcPr)
	}
}

func TestClearCellsRemovesCalcChainForClearedFormula(t *testing.T) {
	pkg, workbook := openTestCalcChainWorkbook(t)
	defer pkg.Close()

	rangeRef, err := address.ParseRange("A1")
	if err != nil {
		t.Fatalf("ParseRange returned error: %v", err)
	}
	result, err := ClearCells(&ClearCellsRequest{
		Package:  pkg,
		SheetRef: workbook.Sheets[0],
		Range:    rangeRef,
	})
	if err != nil {
		t.Fatalf("ClearCells returned error: %v", err)
	}
	if result.Cleared != 1 || strings.Join(result.Refs, ",") != "A1" {
		t.Fatalf("unexpected clear result: %+v", result)
	}
	if cell := findOptionalCell(readTestWorksheetRoot(t, pkg, workbook), "A1"); cell != nil {
		t.Fatalf("A1 still exists after clear: %#v", cell)
	}
	assertCalcChainInvalidated(t, pkg, workbook.PartURI)
}

func TestSetCellsOverFormulaInvalidatesCalcChain(t *testing.T) {
	pkg, workbook := openTestCalcChainWorkbook(t)
	defer pkg.Close()

	result, err := SetCells(&SetCellsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cells: []CellAssignment{
			{Ref: "A1", Type: CellValueNumber, Value: "42"},
		},
	})
	if err != nil {
		t.Fatalf("SetCells returned error: %v", err)
	}
	if result.Updated != 1 || len(result.Cells) != 1 || result.Cells[0].PreviousType != "formula" {
		t.Fatalf("unexpected set result: %+v", result)
	}
	cell := findCellInDoc(t, readTestWorksheetRoot(t, pkg, workbook), "A1")
	if formula := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "f"); formula != nil {
		t.Fatalf("A1 still has formula: %#v", formula)
	}
	assertCalcChainInvalidated(t, pkg, workbook.PartURI)
}

func TestSetRangeOverwriteFormulaInvalidatesCalcChain(t *testing.T) {
	pkg, workbook := openTestCalcChainWorkbook(t)
	defer pkg.Close()

	rangeRef, err := address.ParseRange("A1")
	if err != nil {
		t.Fatalf("ParseRange returned error: %v", err)
	}
	result, err := SetRange(&SetRangeRequest{
		Package:           pkg,
		WorkbookURI:       workbook.PartURI,
		SheetRef:          workbook.Sheets[0],
		Range:             rangeRef,
		Rows:              [][]RangeCell{{{Type: CellValueString, Value: "replacement"}}},
		OverwriteFormulas: true,
	})
	if err != nil {
		t.Fatalf("SetRange returned error: %v", err)
	}
	if result.Updated != 1 || len(result.Cells) != 1 || result.Cells[0].PreviousType != "formula" {
		t.Fatalf("unexpected set range result: %+v", result)
	}
	cell := findCellInDoc(t, readTestWorksheetRoot(t, pkg, workbook), "A1")
	if formula := namespaces.FindChild(cell, namespaces.NsSpreadsheetML, "f"); formula != nil {
		t.Fatalf("A1 still has formula: %#v", formula)
	}
	assertCalcChainInvalidated(t, pkg, workbook.PartURI)
}

func TestClearCellsRemovesUnstyledAndPreservesStyledCell(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>drop</t></is></c>
      <c r="B1" s="5"><v>99</v></c>
    </row>
  </sheetData>
</worksheet>`)
	defer pkg.Close()

	rangeRef, err := address.ParseRange("A1:B1")
	if err != nil {
		t.Fatalf("ParseRange returned error: %v", err)
	}
	result, err := ClearCells(&ClearCellsRequest{
		Package:  pkg,
		SheetRef: workbook.Sheets[0],
		Range:    rangeRef,
	})
	if err != nil {
		t.Fatalf("ClearCells returned error: %v", err)
	}
	if result.Cleared != 2 || strings.Join(result.Refs, ",") != "A1,B1" {
		t.Fatalf("unexpected clear result: %+v", result)
	}

	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	if cell := findOptionalCell(doc.Root(), "A1"); cell != nil {
		t.Fatalf("A1 still exists: %#v", cell)
	}
	cell := findCellInDoc(t, doc.Root(), "B1")
	if cell.SelectAttrValue("s", "") != "5" {
		t.Fatalf("B1 style = %q, want 5", cell.SelectAttrValue("s", ""))
	}
	if cell.SelectAttrValue("t", "") != "" || len(cell.ChildElements()) != 0 {
		t.Fatalf("B1 content not cleared: %#v", cell)
	}
}

func TestSetCellInPrefixedWorksheetRoundTrips(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, `<?xml version="1.0" encoding="UTF-8"?>
<x:worksheet xmlns:x="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <x:sheetData/>
</x:worksheet>`)
	defer pkg.Close()

	_, err := SetCell(&SetCellRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cell:        "A1",
		Value:       "prefixed",
		Type:        CellValueString,
	})
	if err != nil {
		t.Fatalf("SetCell returned error: %v", err)
	}
	report := readMutatedSheet(t, pkg, workbook)
	if len(report.Rows) != 1 || len(report.Rows[0].Cells) != 1 || report.Rows[0].Cells[0].Value != "prefixed" {
		t.Fatalf("prefixed worksheet readback failed: %+v", report.Rows)
	}
}

func TestSetCellsMixedTypesDuplicateLastWins(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	result, err := SetCells(&SetCellsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cells: []CellAssignment{
			{Ref: "D2", Type: CellValueString, Value: "first"},
			{Ref: "B2", Type: CellValueNumber, Value: "42"},
			{Ref: "D2", Type: CellValueString, Value: "last"},
			{Ref: "C2", Type: CellValueBool, Value: "true"},
			{Ref: "E2", Type: CellValueFormula, Value: "=SUM(B2:B2)"},
		},
	})
	if err != nil {
		t.Fatalf("SetCells returned error: %v", err)
	}
	if result.Updated != 4 || result.Created != 4 || result.FormulaCount != 1 || result.Range != "B2:E2" {
		t.Fatalf("unexpected result: %+v", result)
	}

	report := readMutatedSheet(t, pkg, workbook)
	row := report.Rows[1].Cells
	if len(row) != 4 || row[0].Ref != "B2" || row[0].Value != "42" || row[1].Ref != "C2" || row[1].Value != "true" || row[2].Ref != "D2" || row[2].Value != "last" || row[3].Formula != "SUM(B2:B2)" {
		t.Fatalf("unexpected row readback: %+v", row)
	}
}

func TestSetCellsInvalidBatchDoesNotMutate(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	_, err := SetCells(&SetCellsRequest{
		Package:     pkg,
		WorkbookURI: workbook.PartURI,
		SheetRef:    workbook.Sheets[0],
		Cells: []CellAssignment{
			{Ref: "B2", Type: CellValueNumber, Value: "not-a-number"},
		},
	})
	if err == nil {
		t.Fatal("SetCells expected error")
	}
	report := readMutatedSheet(t, pkg, workbook)
	if report.UsedRange.Ref != "A1:C1" {
		t.Fatalf("used range after failed mutation = %q, want A1:C1", report.UsedRange.Ref)
	}
}

func openTestWorkbook(t *testing.T, worksheetXML string) (*opc.Package, *model.Workbook) {
	t.Helper()
	path := writeTestWorkbook(t, worksheetXML)
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("opc.Open returned error: %v", err)
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	return pkg, workbook
}

func readMutatedSheet(t *testing.T, pkg *opc.Package, workbook *model.Workbook) *model.SheetReport {
	t.Helper()
	ctx, err := xlsxsheet.LoadContext(pkg, workbook)
	if err != nil {
		t.Fatalf("LoadContext returned error: %v", err)
	}
	report, err := xlsxsheet.Read(pkg, workbook.Sheets[0], ctx, xlsxsheet.ReadOptions{IncludeData: true})
	if err != nil {
		t.Fatalf("Read returned error: %v", err)
	}
	return report
}

func readTestWorksheetRoot(t *testing.T, pkg *opc.Package, workbook *model.Workbook) *etree.Element {
	t.Helper()
	doc, err := pkg.ReadXMLPart(workbook.Sheets[0].PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart worksheet returned error: %v", err)
	}
	return doc.Root()
}

func findCellInDoc(t *testing.T, root *etree.Element, ref string) *etree.Element {
	t.Helper()
	cell := findOptionalCell(root, ref)
	if cell == nil {
		t.Fatalf("cell %s not found", ref)
	}
	return cell
}

func findOptionalCell(root *etree.Element, ref string) *etree.Element {
	for _, cell := range namespaces.FindDescendants(root, namespaces.NsSpreadsheetML, "c") {
		if cell.SelectAttrValue("r", "") == ref {
			return cell
		}
	}
	return nil
}

func writeTestWorkbook(t *testing.T, worksheetXML string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "workbook.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create workbook: %v", err)
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	addTestZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>`)
	addTestZipFile(t, zw, "_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/workbook.xml", `<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>`)
	addTestZipFile(t, zw, "xl/_rels/workbook.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/worksheets/sheet1.xml", worksheetXML)
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close workbook zip: %v", err)
	}
	return path
}

func openTestCalcChainWorkbook(t *testing.T) (*opc.Package, *model.Workbook) {
	t.Helper()
	path := writeTestCalcChainWorkbook(t)
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("opc.Open returned error: %v", err)
	}
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	return pkg, workbook
}

func writeTestCalcChainWorkbook(t *testing.T) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "calc-chain.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create workbook: %v", err)
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	addTestZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/calcChain.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"/>
</Types>`)
	addTestZipFile(t, zw, "_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/workbook.xml", `<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
</workbook>`)
	addTestZipFile(t, zw, "xl/_rels/workbook.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rIdCalc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain" Target="calcChain.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/worksheets/sheet1.xml", fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="%s">
  <dimension ref="A1:B1"/>
  <sheetData>
    <row r="1">
      <c r="A1"><f>SUM(B1:B1)</f><v>7</v></c>
      <c r="B1"><v>7</v></c>
    </row>
  </sheetData>
</worksheet>`, namespaces.NsSpreadsheetML))
	addTestZipFile(t, zw, "xl/calcChain.xml", fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<calcChain xmlns="%s"><c r="A1" i="1"/></calcChain>`, namespaces.NsSpreadsheetML))
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close workbook zip: %v", err)
	}
	return path
}

func assertCalcChainInvalidated(t *testing.T, pkg *opc.Package, workbookURI string) {
	t.Helper()
	if _, err := pkg.ReadRawPart("/xl/calcChain.xml"); err == nil {
		t.Fatal("calcChain part still exists")
	}
	for _, rel := range pkg.ListRelationships(workbookURI) {
		if rel.Type == namespaces.RelCalcChain {
			t.Fatalf("workbook relationships still include calcChain: %+v", pkg.ListRelationships(workbookURI))
		}
	}
	workbookDoc, err := pkg.ReadXMLPart(workbookURI)
	if err != nil {
		t.Fatalf("ReadXMLPart workbook returned error: %v", err)
	}
	calcPr := namespaces.FindChild(workbookDoc.Root(), namespaces.NsSpreadsheetML, "calcPr")
	if calcPr == nil || calcPr.SelectAttrValue("fullCalcOnLoad", "") != "1" || calcPr.SelectAttrValue("forceFullCalc", "") != "1" {
		t.Fatalf("calcPr = %#v, want recalc flags", calcPr)
	}
}

func addTestZipFile(t *testing.T, zw *zip.Writer, name, body string) {
	t.Helper()
	writer, err := zw.Create(name)
	if err != nil {
		t.Fatalf("failed to create zip entry %s: %v", name, err)
	}
	if _, err := writer.Write([]byte(body)); err != nil {
		t.Fatalf("failed to write zip entry %s: %v", name, err)
	}
}

func defaultWorksheetXML() string {
	return fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="%s">
  <sheetData>
    <row r="1">
      <c r="A1"><v>1</v></c>
      <c r="C1"><v>3</v></c>
    </row>
  </sheetData>
</worksheet>`, namespaces.NsSpreadsheetML)
}
