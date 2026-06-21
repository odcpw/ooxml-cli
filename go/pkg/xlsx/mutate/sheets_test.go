package mutate

import (
	"archive/zip"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	xlsxsheet "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
)

func TestAddSheetAppendsWorksheetAndRelationship(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	result, err := AddSheet(&AddSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: workbook.Sheets,
		Name:           "Data",
	})
	if err != nil {
		t.Fatalf("AddSheet returned error: %v", err)
	}
	if result.Number != 2 || result.Name != "Data" || result.RelationshipID != "rId2" || result.PartURI != "/xl/worksheets/sheet2.xml" {
		t.Fatalf("unexpected add result: %+v", result)
	}
	// sheetId is now randomly allocated (not max+1) so it is never reused after a
	// delete. Assert it is a valid, unique unsignedInt rather than a literal "2".
	sheetIDValue, convErr := strconv.Atoi(result.SheetID)
	if convErr != nil || sheetIDValue < 1 || sheetIDValue > sheetIDRandomCeiling {
		t.Fatalf("new sheetId %q is not in the Open XML SDK range 1..%d", result.SheetID, sheetIDRandomCeiling)
	}
	if result.SheetID == workbook.Sheets[0].SheetID {
		t.Fatalf("new sheetId %q collides with existing sheet", result.SheetID)
	}

	updated, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	if len(updated.Sheets) != 2 || updated.Sheets[1].Name != "Data" || updated.Sheets[1].PartURI != "/xl/worksheets/sheet2.xml" {
		t.Fatalf("unexpected workbook sheets: %+v", updated.Sheets)
	}
	if len(pkg.ListRelationships(workbook.PartURI)) != 2 {
		t.Fatalf("workbook relationships were not updated: %+v", pkg.ListRelationships(workbook.PartURI))
	}

	ctx, err := xlsxsheet.LoadContext(pkg, updated)
	if err != nil {
		t.Fatalf("LoadContext returned error: %v", err)
	}
	report, err := xlsxsheet.Read(pkg, updated.Sheets[1], ctx, xlsxsheet.ReadOptions{})
	if err != nil {
		t.Fatalf("Read new sheet returned error: %v", err)
	}
	if report.UsedRange.Empty != true || report.RowCount != 0 || report.CellCount != 0 {
		t.Fatalf("new sheet report = %+v, want empty", report)
	}
}

func TestNextSheetIDRejectsOutOfRangeExistingSheetID(t *testing.T) {
	_, err := nextSheetID([]model.SheetRef{{Name: "Bad", SheetID: strconv.Itoa(sheetIDRandomCeiling + 1)}})
	if err == nil || !strings.Contains(err.Error(), "must be between 1 and 65534") {
		t.Fatalf("nextSheetID error = %v, want range rejection", err)
	}
}

func TestAddSheetAfterPosition(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	first, err := AddSheet(&AddSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: workbook.Sheets,
		Name:           "Tail",
	})
	if err != nil {
		t.Fatalf("first AddSheet returned error: %v", err)
	}
	updated, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}

	second, err := AddSheet(&AddSheetRequest{
		Package:        pkg,
		WorkbookURI:    updated.PartURI,
		ExistingSheets: updated.Sheets,
		Name:           "Middle",
		AfterPosition:  1,
	})
	if err != nil {
		t.Fatalf("second AddSheet returned error: %v", err)
	}
	if first.Number != 2 || second.Number != 2 {
		t.Fatalf("unexpected insertion numbers: first=%+v second=%+v", first, second)
	}

	updated, err = xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	names := sheetNames(updated.Sheets)
	if strings.Join(names, ",") != "Sheet1,Middle,Tail" {
		t.Fatalf("sheet order = %v", names)
	}
}

func TestRenameSheetUpdatesWorkbookOnly(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	result, err := RenameSheet(&RenameSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: workbook.Sheets,
		SheetRef:       workbook.Sheets[0],
		Name:           "Renamed",
	})
	if err != nil {
		t.Fatalf("RenameSheet returned error: %v", err)
	}
	if result.PreviousName != "Sheet1" || result.Name != "Renamed" || result.PartURI != workbook.Sheets[0].PartURI {
		t.Fatalf("unexpected rename result: %+v", result)
	}

	updated, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	if len(updated.Sheets) != 1 || updated.Sheets[0].Name != "Renamed" || updated.Sheets[0].PartURI != workbook.Sheets[0].PartURI {
		t.Fatalf("unexpected workbook sheets after rename: %+v", updated.Sheets)
	}
}

func TestMoveSheetReordersAndRemapsWorkbookPositions(t *testing.T) {
	pkg, workbook := openSheetLifecycleWorkbook(t, true)
	defer pkg.Close()

	result, err := MoveSheet(&MoveSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: workbook.Sheets,
		SheetRef:       workbook.Sheets[1],
		TargetPosition: 1,
	})
	if err != nil {
		t.Fatalf("MoveSheet returned error: %v", err)
	}
	if result.OldPosition != 2 || result.NewPosition != 1 || result.Name != "Data" || result.RelationshipID != "rId2" || result.PartURI != "/xl/worksheets/sheet2.xml" {
		t.Fatalf("unexpected move result: %+v", result)
	}
	updated, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	if strings.Join(sheetNames(updated.Sheets), ",") != "Data,Summary,Tail" {
		t.Fatalf("sheet order = %+v", updated.Sheets)
	}
	if updated.Sheets[0].SheetID != "20" || updated.Sheets[0].RelationshipID != "rId2" || updated.Sheets[0].PartURI != "/xl/worksheets/sheet2.xml" {
		t.Fatalf("moved sheet identity changed: %+v", updated.Sheets[0])
	}
	workbookXML := workbookXMLString(t, pkg, workbook.PartURI)
	for _, want := range []string{`activeTab="2"`, `firstSheet="1"`, `name="LocalSummary" localSheetId="1"`, `name="LocalTail" localSheetId="2"`} {
		if !strings.Contains(workbookXML, want) {
			t.Fatalf("workbook XML missing %q:\n%s", want, workbookXML)
		}
	}
}

func TestDeleteSheetRemovesPartsRelationshipsAndRemapsWorkbookPositions(t *testing.T) {
	pkg, workbook := openSheetLifecycleWorkbook(t, true)
	defer pkg.Close()

	result, err := DeleteSheet(&DeleteSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: workbook.Sheets,
		SheetRef:       workbook.Sheets[1],
	})
	if err != nil {
		t.Fatalf("DeleteSheet returned error: %v", err)
	}
	if result.Number != 2 || result.Name != "Data" || result.RemainingSheets != 2 || result.RemovedRelationshipID != "rId2" {
		t.Fatalf("unexpected delete result: %+v", result)
	}
	for _, want := range []string{"/xl/worksheets/sheet2.xml", "/xl/worksheets/_rels/sheet2.xml.rels", "/xl/calcChain.xml"} {
		if !containsSheetLifecyclePart(result.RemovedParts, want) {
			t.Fatalf("removed parts missing %s: %+v", want, result.RemovedParts)
		}
		if _, err := pkg.ReadRawPart(want); err == nil {
			t.Fatalf("removed part %s is still readable", want)
		}
	}
	for _, rel := range pkg.ListRelationships(workbook.PartURI) {
		if rel.ID == "rId2" || rel.Type == namespaces.RelCalcChain {
			t.Fatalf("workbook relationships still include removed rel: %+v", pkg.ListRelationships(workbook.PartURI))
		}
	}
	updated, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}
	if strings.Join(sheetNames(updated.Sheets), ",") != "Summary,Tail" {
		t.Fatalf("sheet order = %+v", updated.Sheets)
	}
	workbookXML := workbookXMLString(t, pkg, workbook.PartURI)
	for _, want := range []string{`activeTab="1"`, `firstSheet="0"`, `name="LocalSummary" localSheetId="0"`, `name="LocalTail" localSheetId="1"`} {
		if !strings.Contains(workbookXML, want) {
			t.Fatalf("workbook XML missing %q:\n%s", want, workbookXML)
		}
	}
	if strings.Contains(workbookXML, "LocalData") {
		t.Fatalf("deleted sheet scoped defined name remained:\n%s", workbookXML)
	}
}

func TestDeleteSheetRejectsLastAndLastVisibleSheet(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	if _, err := DeleteSheet(&DeleteSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: workbook.Sheets,
		SheetRef:       workbook.Sheets[0],
	}); err == nil {
		t.Fatal("DeleteSheet expected last sheet error")
	}

	pkg2, workbook2 := openSheetLifecycleWorkbook(t, false)
	defer pkg2.Close()
	workbook2.Sheets[1].State = model.SheetStateVisible
	workbook2.Sheets[2].State = "hidden"
	if _, err := DeleteSheet(&DeleteSheetRequest{
		Package:        pkg2,
		WorkbookURI:    workbook2.PartURI,
		ExistingSheets: workbook2.Sheets[1:],
		SheetRef:       workbook2.Sheets[1],
	}); err == nil {
		t.Fatal("DeleteSheet expected last visible sheet error")
	}
}

func TestSheetLifecycleRejectsInvalidNames(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	tests := []string{"", "   ", "Bad/Name", "'Quoted", strings.Repeat("A", 32), "sheet1", "History"}
	for _, name := range tests {
		_, err := AddSheet(&AddSheetRequest{
			Package:        pkg,
			WorkbookURI:    workbook.PartURI,
			ExistingSheets: workbook.Sheets,
			Name:           name,
		})
		if err == nil {
			t.Fatalf("AddSheet name %q expected error", name)
		}
	}

	_, err := RenameSheet(&RenameSheetRequest{
		Package:        pkg,
		WorkbookURI:    workbook.PartURI,
		ExistingSheets: append(workbook.Sheets, model.SheetRef{Name: "Data", RelationshipID: "rId2"}),
		SheetRef:       workbook.Sheets[0],
		Name:           "Data",
	})
	if err == nil {
		t.Fatal("RenameSheet expected duplicate name error")
	}
}

func openSheetLifecycleWorkbook(t *testing.T, includeCalcChain bool) (*opc.Package, *model.Workbook) {
	t.Helper()
	path := writeSheetLifecycleWorkbook(t, includeCalcChain)
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

func writeSheetLifecycleWorkbook(t *testing.T, includeCalcChain bool) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "sheet-lifecycle.xlsx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create workbook: %v", err)
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	addTestZipFile(t, zw, "[Content_Types].xml", sheetLifecycleContentTypes(includeCalcChain))
	addTestZipFile(t, zw, "_rels/.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>`)
	addTestZipFile(t, zw, "xl/workbook.xml", `<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <bookViews><workbookView activeTab="2" firstSheet="0"/></bookViews>
  <sheets>
    <sheet name="Summary" sheetId="10" r:id="rId1"/>
    <sheet name="Data" sheetId="20" r:id="rId2"/>
    <sheet name="Tail" sheetId="30" r:id="rId3"/>
  </sheets>
  <definedNames>
    <definedName name="GlobalName">Summary!$A$1</definedName>
    <definedName name="LocalSummary" localSheetId="0">Summary!$A$1</definedName>
    <definedName name="LocalData" localSheetId="1">Data!$A$1</definedName>
    <definedName name="LocalTail" localSheetId="2">Tail!$A$1</definedName>
  </definedNames>
</workbook>`)
	addTestZipFile(t, zw, "xl/_rels/workbook.xml.rels", sheetLifecycleWorkbookRels(includeCalcChain))
	for i := 1; i <= 3; i++ {
		addTestZipFile(t, zw, fmt.Sprintf("xl/worksheets/sheet%d.xml", i), defaultWorksheetXML())
	}
	addTestZipFile(t, zw, "xl/worksheets/_rels/sheet2.xml.rels", `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`)
	if includeCalcChain {
		addTestZipFile(t, zw, "xl/calcChain.xml", fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<calcChain xmlns="%s"><c r="A1" i="2"/></calcChain>`, namespaces.NsSpreadsheetML))
	}
	if err := zw.Close(); err != nil {
		t.Fatalf("failed to close workbook zip: %v", err)
	}
	return path
}

func sheetLifecycleContentTypes(includeCalcChain bool) string {
	extra := ""
	if includeCalcChain {
		extra = `  <Override PartName="/xl/calcChain.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.calcChain+xml"/>
`
	}
	return `<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet3.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
` + extra + `</Types>`
}

func sheetLifecycleWorkbookRels(includeCalcChain bool) string {
	extra := ""
	if includeCalcChain {
		extra = `  <Relationship Id="rIdCalc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/calcChain" Target="calcChain.xml"/>
`
	}
	return `<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet3.xml"/>
` + extra + `</Relationships>`
}

func workbookXMLString(t *testing.T, pkg *opc.Package, uri string) string {
	t.Helper()
	data, err := pkg.ReadRawPart(uri)
	if err != nil {
		t.Fatalf("ReadRawPart %s returned error: %v", uri, err)
	}
	return string(data)
}

func containsSheetLifecyclePart(parts []string, want string) bool {
	for _, part := range parts {
		if part == want {
			return true
		}
	}
	return false
}

func sheetNames(sheets []model.SheetRef) []string {
	names := make([]string, 0, len(sheets))
	for _, sheet := range sheets {
		names = append(names, sheet.Name)
	}
	return names
}
