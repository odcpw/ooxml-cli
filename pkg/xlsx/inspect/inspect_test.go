package inspect

import (
	"fmt"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestParseWorkbookListsSheetsAndResolvesTargets(t *testing.T) {
	session := newWorkbookSession()

	workbook, err := ParseWorkbook(session)
	if err != nil {
		t.Fatalf("ParseWorkbook returned error: %v", err)
	}

	if workbook.PartURI != "/xl/workbook.xml" {
		t.Fatalf("workbook part URI = %q, want /xl/workbook.xml", workbook.PartURI)
	}
	if workbook.SharedStringsURI != "/xl/sharedStrings.xml" {
		t.Fatalf("shared strings URI = %q, want /xl/sharedStrings.xml", workbook.SharedStringsURI)
	}
	if workbook.StylesURI != "/xl/styles.xml" {
		t.Fatalf("styles URI = %q, want /xl/styles.xml", workbook.StylesURI)
	}
	if len(workbook.Sheets) != 2 {
		t.Fatalf("sheet count = %d, want 2", len(workbook.Sheets))
	}

	first := workbook.Sheets[0]
	if first.Position != 1 ||
		first.Number != 1 ||
		first.Name != "Summary" ||
		first.SheetID != "1" ||
		first.RelationshipID != "rSheet1" ||
		first.State != model.SheetStateVisible ||
		first.PartURI != "/xl/worksheets/sheet1.xml" ||
		first.RelationshipType != namespaces.RelWorksheet {
		t.Fatalf("first sheet = %+v", first)
	}

	second := workbook.Sheets[1]
	if second.Position != 2 ||
		second.Name != "Hidden Data" ||
		second.SheetID != "2" ||
		second.RelationshipID != "rSheet2" ||
		second.State != "hidden" ||
		second.PartURI != "/xl/worksheets/sheet2.xml" {
		t.Fatalf("second sheet = %+v", second)
	}
}

func TestListSheetsReturnsWorkbookOrderCopy(t *testing.T) {
	session := newWorkbookSession()

	sheets, err := ListSheets(session)
	if err != nil {
		t.Fatalf("ListSheets returned error: %v", err)
	}
	if len(sheets) != 2 {
		t.Fatalf("sheet count = %d, want 2", len(sheets))
	}
	if sheets[0].Name != "Summary" || sheets[1].Name != "Hidden Data" {
		t.Fatalf("sheet order = %+v", sheets)
	}

	sheets[0].Name = "mutated"
	again, err := ListSheets(session)
	if err != nil {
		t.Fatalf("ListSheets returned error on second call: %v", err)
	}
	if again[0].Name != "Summary" {
		t.Fatalf("ListSheets did not return fresh data, got %+v", again[0])
	}
}

func TestSummarizeWorkbookCountsPackageParts(t *testing.T) {
	session := newWorkbookSession()

	summary, err := SummarizeWorkbook(session)
	if err != nil {
		t.Fatalf("SummarizeWorkbook returned error: %v", err)
	}

	if summary.Type != string(opc.PackageTypeXLSX) {
		t.Fatalf("type = %q, want xlsx", summary.Type)
	}
	if summary.WorkbookPartURI != "/xl/workbook.xml" {
		t.Fatalf("workbook part URI = %q, want /xl/workbook.xml", summary.WorkbookPartURI)
	}
	if summary.SheetCount != 2 ||
		summary.WorksheetCount != 2 ||
		!summary.SharedStrings ||
		summary.SharedStringCount != 3 ||
		!summary.Styles ||
		summary.Tables != 1 ||
		summary.Charts != 1 ||
		summary.MediaAssets != 1 ||
		summary.CustomXMLParts != 1 {
		t.Fatalf("summary counts = %+v", summary)
	}
}

func TestCountSharedStringsAndCellFormats(t *testing.T) {
	session := newWorkbookSession()

	sharedStrings, err := CountSharedStrings(session, "/xl/sharedStrings.xml")
	if err != nil {
		t.Fatalf("CountSharedStrings returned error: %v", err)
	}
	if sharedStrings != 3 {
		t.Fatalf("shared string count = %d, want 3", sharedStrings)
	}

	cellFormats, err := CountCellFormats(session, "/xl/styles.xml")
	if err != nil {
		t.Fatalf("CountCellFormats returned error: %v", err)
	}
	if cellFormats != 2 {
		t.Fatalf("cell format count = %d, want 2", cellFormats)
	}
}

func TestParseWorkbookErrorsOnMissingSheetRelationship(t *testing.T) {
	session := newWorkbookSession()
	session.xmlParts["/xl/workbook.xml"] = workbookXML("rMissing")

	_, err := ParseWorkbook(session)
	if err == nil {
		t.Fatal("ParseWorkbook returned nil error")
	}
	if !strings.Contains(err.Error(), "rMissing") {
		t.Fatalf("error = %q, want missing relationship id", err.Error())
	}
}

func TestFindWorkbookPartFallsBackToWorkbookContentType(t *testing.T) {
	session := newWorkbookSession()
	session.relationships["/"] = nil

	uri, err := FindWorkbookPart(session)
	if err != nil {
		t.Fatalf("FindWorkbookPart returned error: %v", err)
	}
	if uri != "/xl/workbook.xml" {
		t.Fatalf("workbook URI = %q, want /xl/workbook.xml", uri)
	}
}

func TestFindWorkbookPartErrorsWhenWorkbookMissing(t *testing.T) {
	session := &mockSession{
		parts: []opc.PartInfo{
			{URI: "/ppt/presentation.xml", ContentType: "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml", IsXML: true},
		},
		relationships: map[string][]opc.RelationshipInfo{
			"/": {{ID: "rId1", Type: namespaces.RelOfficeDocument, Target: "ppt/presentation.xml"}},
		},
		xmlParts:     map[string]string{},
		contentTypes: map[string]string{"/ppt/presentation.xml": "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"},
	}

	_, err := FindWorkbookPart(session)
	if err == nil {
		t.Fatal("FindWorkbookPart returned nil error")
	}
}

func newWorkbookSession() *mockSession {
	parts := []opc.PartInfo{
		{URI: "/xl/workbook.xml", ContentType: namespaces.ContentTypeWorkbook, IsXML: true},
		{URI: "/xl/worksheets/sheet1.xml", ContentType: namespaces.ContentTypeWorksheet, IsXML: true},
		{URI: "/xl/worksheets/sheet2.xml", ContentType: namespaces.ContentTypeWorksheet, IsXML: true},
		{URI: "/xl/worksheets/_rels/sheet1.xml.rels", ContentType: "application/vnd.openxmlformats-package.relationships+xml", IsXML: true},
		{URI: "/xl/sharedStrings.xml", ContentType: namespaces.ContentTypeSharedStrings, IsXML: true},
		{URI: "/xl/styles.xml", ContentType: namespaces.ContentTypeStyles, IsXML: true},
		{URI: "/xl/tables/table1.xml", ContentType: namespaces.ContentTypeTable, IsXML: true},
		{URI: "/xl/charts/chart1.xml", ContentType: namespaces.ContentTypeChart, IsXML: true},
		{URI: "/xl/media/image1.png", ContentType: "image/png"},
		{URI: "/customXml/item1.xml", ContentType: "application/xml", IsXML: true},
		{URI: "/customXml/_rels/item1.xml.rels", ContentType: "application/vnd.openxmlformats-package.relationships+xml", IsXML: true},
	}

	contentTypes := make(map[string]string, len(parts))
	for _, part := range parts {
		contentTypes[part.URI] = part.ContentType
	}

	return &mockSession{
		parts: parts,
		relationships: map[string][]opc.RelationshipInfo{
			"/": {
				{ID: "rWorkbook", Type: namespaces.RelOfficeDocument, Target: "xl/workbook.xml"},
			},
			"/xl/workbook.xml": {
				{ID: "rSheet1", Type: namespaces.RelWorksheet, Target: "worksheets/sheet1.xml"},
				{ID: "rSheet2", Type: namespaces.RelWorksheet, Target: "/xl/worksheets/sheet2.xml"},
				{ID: "rSharedStrings", Type: namespaces.RelSharedStrings, Target: "sharedStrings.xml"},
				{ID: "rStyles", Type: namespaces.RelStyles, Target: "styles.xml"},
			},
		},
		xmlParts: map[string]string{
			"/xl/workbook.xml":      workbookXML("rSheet1"),
			"/xl/sharedStrings.xml": sharedStringsXML(),
			"/xl/styles.xml":        stylesXML(),
		},
		contentTypes: contentTypes,
	}
}

func workbookXML(firstSheetRID string) string {
	return fmt.Sprintf(`<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="%s" xmlns:r="%s">
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="%s"/>
    <sheet name="Hidden Data" sheetId="2" state="hidden" r:id="rSheet2"/>
  </sheets>
</workbook>`, namespaces.NsSpreadsheetML, namespaces.NsR, firstSheetRID)
}

func sharedStringsXML() string {
	return fmt.Sprintf(`<sst xmlns="%s" count="3" uniqueCount="3">
  <si><t>Alpha</t></si>
  <si><t>Beta</t></si>
  <si><t>Gamma</t></si>
</sst>`, namespaces.NsSpreadsheetML)
}

func stylesXML() string {
	return fmt.Sprintf(`<styleSheet xmlns="%s">
  <cellXfs count="2"><xf/><xf/></cellXfs>
</styleSheet>`, namespaces.NsSpreadsheetML)
}

type mockSession struct {
	parts         []opc.PartInfo
	relationships map[string][]opc.RelationshipInfo
	xmlParts      map[string]string
	contentTypes  map[string]string
}

func (m *mockSession) ListParts() []opc.PartInfo {
	parts := make([]opc.PartInfo, len(m.parts))
	copy(parts, m.parts)
	return parts
}

func (m *mockSession) ListRelationships(sourceURI string) []opc.RelationshipInfo {
	rels := m.relationships[sourceURI]
	out := make([]opc.RelationshipInfo, len(rels))
	copy(out, rels)
	return out
}

func (m *mockSession) ReadRawPart(uri string) ([]byte, error) {
	data, ok := m.xmlParts[uri]
	if !ok {
		return nil, fmt.Errorf("part %s not found", uri)
	}
	return []byte(data), nil
}

func (m *mockSession) ReadXMLPart(uri string) (*etree.Document, error) {
	data, err := m.ReadRawPart(uri)
	if err != nil {
		return nil, err
	}
	doc := etree.NewDocument()
	if err := doc.ReadFromBytes(data); err != nil {
		return nil, err
	}
	return doc, nil
}

func (m *mockSession) GetContentType(uri string) string {
	return m.contentTypes[uri]
}

func (m *mockSession) GetZipMeta(uri string) *opc.ZipEntryMeta {
	return nil
}

func (m *mockSession) ReplaceRawPart(uri string, data []byte, contentType string) error {
	return fmt.Errorf("mockSession is read-only")
}

func (m *mockSession) ReplaceXMLPart(uri string, doc *etree.Document) error {
	return fmt.Errorf("mockSession is read-only")
}

func (m *mockSession) AddPart(uri string, data []byte, contentType string, meta *opc.ZipEntryMeta) error {
	return fmt.Errorf("mockSession is read-only")
}

func (m *mockSession) RemovePart(uri string) error {
	return fmt.Errorf("mockSession is read-only")
}

func (m *mockSession) SaveAs(path string) error {
	return fmt.Errorf("mockSession is read-only")
}

func (m *mockSession) Close() error {
	return nil
}

func (m *mockSession) IsDirty() bool {
	return false
}

func (m *mockSession) Warnings() []string {
	return nil
}
