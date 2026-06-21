package validate

import (
	"fmt"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

type validationTestSession struct {
	parts         []opc.PartInfo
	relationships map[string][]opc.RelationshipInfo
	xmlParts      map[string]string
	rawParts      map[string][]byte
}

func (s *validationTestSession) ListParts() []opc.PartInfo {
	return s.parts
}

func (s *validationTestSession) ListRelationships(sourceURI string) []opc.RelationshipInfo {
	return s.relationships[sourceURI]
}

func (s *validationTestSession) ReadRawPart(uri string) ([]byte, error) {
	if raw, ok := s.rawParts[uri]; ok {
		return append([]byte(nil), raw...), nil
	}
	if xml, ok := s.xmlParts[uri]; ok {
		return []byte(xml), nil
	}
	return nil, fmt.Errorf("part not found: %s", uri)
}

func (s *validationTestSession) ReadXMLPart(uri string) (*etree.Document, error) {
	xml, ok := s.xmlParts[uri]
	if !ok {
		return nil, fmt.Errorf("part not found: %s", uri)
	}
	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		return nil, err
	}
	return doc, nil
}

func (s *validationTestSession) GetContentType(uri string) string {
	for _, part := range s.parts {
		if part.URI == uri {
			return part.ContentType
		}
	}
	return ""
}

func (s *validationTestSession) GetZipMeta(uri string) *opc.ZipEntryMeta {
	return nil
}

func (s *validationTestSession) ReplaceRawPart(uri string, data []byte, contentType string) error {
	return nil
}

func (s *validationTestSession) ReplaceXMLPart(uri string, doc *etree.Document) error {
	return nil
}

func (s *validationTestSession) AddPart(uri string, data []byte, contentType string, meta *opc.ZipEntryMeta) error {
	return nil
}

func (s *validationTestSession) RemovePart(uri string) error {
	return nil
}

func (s *validationTestSession) SaveAs(path string) error {
	return nil
}

func (s *validationTestSession) Close() error {
	return nil
}

func (s *validationTestSession) IsDirty() bool {
	return false
}

func (s *validationTestSession) Warnings() []string {
	return nil
}

func TestValidatePackageXLSXDispatchSkipsPPTXSemantics(t *testing.T) {
	session := newXLSXValidationSession()

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_PARSE_ERROR")
	assertNoErrorDiagnostics(t, diags)
}

func TestValidatePackageUnknownSkipsTypedSemantics(t *testing.T) {
	session := &validationTestSession{
		parts: []opc.PartInfo{
			xmlPart("/[Content_Types].xml", "application/xml"),
			xmlPart("/_rels/.rels", "application/vnd.openxmlformats-package.relationships+xml"),
		},
		relationships: map[string][]opc.RelationshipInfo{},
		xmlParts: map[string]string{
			"/[Content_Types].xml": `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`,
			"/_rels/.rels":         `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
		},
	}

	if packageType := opc.DetectType(session); packageType != opc.PackageTypeUnknown {
		t.Fatalf("DetectType() = %q, want %q", packageType, opc.PackageTypeUnknown)
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_PARSE_ERROR")
	assertNoDiagnosticCode(t, diags, "XLSX_PARSE_ERROR")
	assertNoErrorDiagnostics(t, diags)
}

func TestValidateXLSXMissingWorksheet(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = removePart(session.parts, "/xl/worksheets/sheet1.xml")
	delete(session.xmlParts, "/xl/worksheets/sheet1.xml")

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_PARSE_ERROR")
	assertHasDiagnosticCode(t, diags, "REL_DANGLING_TARGET")
	assertHasDiagnosticCode(t, diags, "XLSX_MISSING_WORKSHEET")
}

func TestValidateXLSXSharedStringIndexOutOfRange(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, xmlPart("/xl/sharedStrings.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"))
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings",
		Target:    "sharedStrings.xml",
	})
	session.xmlParts["/xl/sharedStrings.xml"] = `<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>only string</t></si></sst>`
	session.xmlParts["/xl/worksheets/sheet1.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="s"><v>1</v></c></row></sheetData></worksheet>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_PARSE_ERROR")
	assertHasDiagnosticCode(t, diags, "XLSX_SHARED_STRING_INDEX_OUT_OF_RANGE")
}

func TestValidateXLSXSheetIDRange(t *testing.T) {
	tests := []struct {
		name    string
		sheetID string
		code    string
	}{
		{name: "zero", sheetID: "0", code: "XLSX_SHEET_ID_OUT_OF_RANGE"},
		{name: "above-sdk-max", sheetID: "65535", code: "XLSX_SHEET_ID_OUT_OF_RANGE"},
		{name: "non-integer", sheetID: "not-a-number", code: "XLSX_SHEET_ID_INVALID"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			session := newXLSXValidationSession()
			session.xmlParts["/xl/workbook.xml"] = fmt.Sprintf(`<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="%s" r:id="rId1"/></sheets></workbook>`, tt.sheetID)

			diags, err := ValidatePackage(session)
			if err != nil {
				t.Fatalf("ValidatePackage returned error: %v", err)
			}
			assertHasDiagnosticCode(t, diags, tt.code)
		})
	}
}

func TestValidateXLSXDefinedNames(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, xmlPart("/xl/worksheets/sheet2.xml", namespaces.ContentTypeWorksheet))
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      namespaces.RelWorksheet,
		Target:    "worksheets/sheet2.xml",
	})
	session.xmlParts["/xl/workbook.xml"] = `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
    <sheet name="Report Data" sheetId="2" r:id="rId2"/>
  </sheets>
  <definedNames>
    <definedName>Sheet1!$A$1</definedName>
    <definedName name="BadScope" localSheetId="2">Sheet1!$A$1</definedName>
    <definedName name="BadScopeText" localSheetId="abc">Sheet1!$A$1</definedName>
    <definedName name="Sales">Sheet1!$A$1</definedName>
    <definedName name="sales">Sheet1!$B$1</definedName>
    <definedName name="MissingSheet">'Missing Sheet'!$A$1</definedName>
    <definedName name="BadRange">Sheet1!XFE1</definedName>
    <definedName name="Empty"></definedName>
  </definedNames>
</workbook>`
	session.xmlParts["/xl/worksheets/sheet2.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_REQUIRED")
	assertHasDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_SCOPE")
	assertHasDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_DUPLICATE")
	assertHasDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_REFERENCE")
}

func TestValidateXLSXValidDefinedNames(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, xmlPart("/xl/worksheets/sheet2.xml", namespaces.ContentTypeWorksheet))
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      namespaces.RelWorksheet,
		Target:    "worksheets/sheet2.xml",
	})
	session.xmlParts["/xl/workbook.xml"] = `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rId1"/>
    <sheet name="Report Data" sheetId="2" r:id="rId2"/>
  </sheets>
  <definedNames>
    <definedName name="_xlnm.Print_Area" localSheetId="1">'Report Data'!$A$1:$D$20</definedName>
    <definedName name="_xlnm.Print_Titles" localSheetId="1">'Report Data'!$1:$2</definedName>
    <definedName name="Sales">Sheet1!$A$1</definedName>
    <definedName name="WholeColumns">'Report Data'!$A:$D</definedName>
    <definedName name="FormulaRef">SUM(Sheet1!$A$1,'Report Data'!$B$2)</definedName>
    <definedName name="ExternalRef">[Book.xlsx]Other!$A$1</definedName>
  </definedNames>
</workbook>`
	session.xmlParts["/xl/worksheets/sheet2.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_REQUIRED")
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_SCOPE")
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_DUPLICATE")
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_REFERENCE")
	assertNoErrorDiagnostics(t, diags)
}

func TestValidateXLSXWorksheetDrawingRelationship(t *testing.T) {
	session := newXLSXValidationSession()
	session.xmlParts["/xl/worksheets/sheet1.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData/><drawing r:id="rIdMissing"/></worksheet>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_WORKSHEET_DRAWING_REFERENCE")
}

func TestValidateXLSXDrawingAnchorAndChartReference(t *testing.T) {
	session := newXLSXValidationSession()
	addXLSXDrawingPart(session, namespaces.ContentTypeDrawing, `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor>
    <xdr:from><xdr:col>2</xdr:col><xdr:row>2</xdr:row></xdr:from>
    <xdr:to><xdr:col>1</xdr:col><xdr:row>1</xdr:row></xdr:to>
    <xdr:graphicFrame>
      <a:graphic><a:graphicData><c:chart r:id="rIdChart1"/></a:graphicData></a:graphic>
    </xdr:graphicFrame>
  </xdr:twoCellAnchor>
</xdr:wsDr>`)
	session.relationships["/xl/drawings/drawing1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/drawings/drawing1.xml",
			ID:        "rIdChart1",
			Type:      namespaces.RelChart,
			Target:    "../worksheets/sheet1.xml",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_DRAWING_ANCHOR")
	assertHasDiagnosticCode(t, diags, "XLSX_DRAWING_CHART_REFERENCE")
}

func TestValidateXLSXAutoFilterAndSortRanges(t *testing.T) {
	session := newXLSXValidationSession()
	session.xmlParts["/xl/worksheets/sheet1.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData/>
  <autoFilter ref="A1:B10">
    <filterColumn colId="2"/>
    <sortState ref="C2:C10">
      <sortCondition ref="D2:D10"/>
    </sortState>
  </autoFilter>
</worksheet>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_AUTOFILTER_COLUMN")
	assertHasDiagnosticCode(t, diags, "XLSX_SORT_STATE_RANGE")
}

func TestValidateXLSXWorksheetPivotRelationshipTarget(t *testing.T) {
	session := newXLSXValidationSession()
	session.relationships["/xl/worksheets/sheet1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/worksheets/sheet1.xml",
			ID:        "rIdPivot1",
			Type:      namespaces.RelPivotTable,
			Target:    "sheet1.xml",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_WORKSHEET_PIVOT_REFERENCE")
}

func TestValidateXLSXWorkbookPivotCacheTopology(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, xmlPart("/xl/pivotCache/pivotCacheDefinition1.xml", namespaces.ContentTypePivotCache))
	session.xmlParts["/xl/workbook.xml"] = `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
  <pivotCaches>
    <pivotCache cacheId="1" r:id="rIdCache1"/>
    <pivotCache cacheId="2"/>
    <pivotCache cacheId="0" r:id="rIdMissingCache"/>
    <pivotCache cacheId="1" r:id="rIdWrongCache"/>
  </pivotCaches>
</workbook>`
	session.xmlParts["/xl/pivotCache/pivotCacheDefinition1.xml"] = `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rIdRecords1"/>`
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"],
		opc.RelationshipInfo{SourceURI: "/xl/workbook.xml", ID: "rIdCache1", Type: namespaces.RelPivotCache, Target: "pivotCache/pivotCacheDefinition1.xml"},
		opc.RelationshipInfo{SourceURI: "/xl/workbook.xml", ID: "rIdWrongCache", Type: namespaces.RelWorksheet, Target: "worksheets/sheet1.xml"},
	)
	session.relationships["/xl/pivotCache/pivotCacheDefinition1.xml"] = []opc.RelationshipInfo{
		{SourceURI: "/xl/pivotCache/pivotCacheDefinition1.xml", ID: "rIdRecords1", Type: namespaces.RelPivotRecords, Target: "pivotCacheRecords1.xml"},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE")
	assertHasDiagnosticCode(t, diags, "XLSX_PIVOT_CACHE_RECORDS_REFERENCE")
}

func TestValidateXLSXPivotCacheRecordsRelationshipWithoutRootRID(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, xmlPart("/xl/pivotCache/pivotCacheDefinition1.xml", namespaces.ContentTypePivotCache))
	session.xmlParts["/xl/workbook.xml"] = `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
  <pivotCaches><pivotCache cacheId="1" r:id="rIdCache1"/></pivotCaches>
</workbook>`
	session.xmlParts["/xl/pivotCache/pivotCacheDefinition1.xml"] = `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rIdCache1",
		Type:      namespaces.RelPivotCache,
		Target:    "pivotCache/pivotCacheDefinition1.xml",
	})
	session.relationships["/xl/pivotCache/pivotCacheDefinition1.xml"] = []opc.RelationshipInfo{
		{SourceURI: "/xl/pivotCache/pivotCacheDefinition1.xml", ID: "rIdRecords1", Type: namespaces.RelPivotRecords, Target: "missingPivotCacheRecords.xml"},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_PIVOT_CACHE_RECORDS_REFERENCE")
}

func TestValidateXLSXWorksheetPivotCacheIDReference(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts,
		xmlPart("/xl/pivotCache/pivotCacheDefinition1.xml", namespaces.ContentTypePivotCache),
		xmlPart("/xl/pivotTables/pivotTable1.xml", namespaces.ContentTypePivotTable),
	)
	session.xmlParts["/xl/workbook.xml"] = `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
  <pivotCaches><pivotCache cacheId="1" r:id="rIdCache1"/></pivotCaches>
</workbook>`
	session.xmlParts["/xl/pivotCache/pivotCacheDefinition1.xml"] = `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`
	session.xmlParts["/xl/pivotTables/pivotTable1.xml"] = `<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="PivotTable1" cacheId="2"/>`
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rIdCache1",
		Type:      namespaces.RelPivotCache,
		Target:    "pivotCache/pivotCacheDefinition1.xml",
	})
	session.relationships["/xl/worksheets/sheet1.xml"] = []opc.RelationshipInfo{
		{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdPivot1", Type: namespaces.RelPivotTable, Target: "../pivotTables/pivotTable1.xml"},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_WORKSHEET_PIVOT_CACHE_REFERENCE")
}

func TestValidateXLSXPivotTableDefinition(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts,
		xmlPart("/xl/pivotCache/pivotCacheDefinition1.xml", namespaces.ContentTypePivotCache),
		xmlPart("/xl/pivotTables/pivotTable1.xml", namespaces.ContentTypePivotTable),
	)
	session.xmlParts["/xl/workbook.xml"] = `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets>
  <pivotCaches><pivotCache cacheId="1" r:id="rIdCache1"/></pivotCaches>
</workbook>`
	session.xmlParts["/xl/pivotCache/pivotCacheDefinition1.xml"] = `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`
	session.xmlParts["/xl/pivotTables/pivotTable1.xml"] = `<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" cacheId="1">
  <location ref="bad"/>
  <pivotFields count="2"><pivotField/></pivotFields>
</pivotTableDefinition>`
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rIdCache1",
		Type:      namespaces.RelPivotCache,
		Target:    "pivotCache/pivotCacheDefinition1.xml",
	})
	session.relationships["/xl/worksheets/sheet1.xml"] = []opc.RelationshipInfo{
		{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdPivot1", Type: namespaces.RelPivotTable, Target: "../pivotTables/pivotTable1.xml"},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "XLSX_PIVOT_TABLE_DEFINITION")
	assertNoDiagnosticCode(t, diags, "XLSX_WORKSHEET_PIVOT_CACHE_REFERENCE")
}

func TestValidateXLSXValidDrawingChartFilterSort(t *testing.T) {
	session := newXLSXValidationSession()
	addXLSXDrawingPart(session, namespaces.ContentTypeDrawing, `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor>
    <xdr:from><xdr:col>0</xdr:col><xdr:row>0</xdr:row></xdr:from>
    <xdr:to><xdr:col>2</xdr:col><xdr:row>8</xdr:row></xdr:to>
    <xdr:graphicFrame>
      <a:graphic><a:graphicData><c:chart r:id="rIdChart1"/></a:graphicData></a:graphic>
    </xdr:graphicFrame>
    <xdr:clientData/>
  </xdr:twoCellAnchor>
</xdr:wsDr>`)
	session.parts = append(session.parts, xmlPart("/xl/charts/chart1.xml", namespaces.ContentTypeChart))
	session.xmlParts["/xl/charts/chart1.xml"] = `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea/></c:chart></c:chartSpace>`
	session.relationships["/xl/drawings/drawing1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/drawings/drawing1.xml",
			ID:        "rIdChart1",
			Type:      namespaces.RelChart,
			Target:    "../charts/chart1.xml",
		},
	}
	session.xmlParts["/xl/worksheets/sheet1.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <autoFilter ref="A1:C10">
    <filterColumn colId="2"/>
    <sortState ref="A2:C10">
      <sortCondition ref="C2:C10"/>
    </sortState>
  </autoFilter>
  <drawing r:id="rIdDrawing1"/>
</worksheet>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_WORKSHEET_DRAWING_REFERENCE")
	assertNoDiagnosticCode(t, diags, "XLSX_DRAWING_ANCHOR")
	assertNoDiagnosticCode(t, diags, "XLSX_DRAWING_CHART_REFERENCE")
	assertNoDiagnosticCode(t, diags, "XLSX_AUTOFILTER_COLUMN")
	assertNoDiagnosticCode(t, diags, "XLSX_SORT_STATE_RANGE")
	assertNoErrorDiagnostics(t, diags)
}

func newXLSXValidationSession() *validationTestSession {
	return &validationTestSession{
		parts: []opc.PartInfo{
			xmlPart("/[Content_Types].xml", "application/xml"),
			xmlPart("/_rels/.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			xmlPart("/xl/workbook.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"),
			xmlPart("/xl/_rels/workbook.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			xmlPart("/xl/worksheets/sheet1.xml", "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"),
		},
		relationships: map[string][]opc.RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
					Target:    "xl/workbook.xml",
				},
			},
			"/xl/workbook.xml": {
				{
					SourceURI: "/xl/workbook.xml",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
					Target:    "worksheets/sheet1.xml",
				},
			},
		},
		xmlParts: map[string]string{
			"/[Content_Types].xml":        `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`,
			"/_rels/.rels":                `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/xl/workbook.xml":            `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>`,
			"/xl/_rels/workbook.xml.rels": `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/xl/worksheets/sheet1.xml":   `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>`,
		},
	}
}

func addXLSXDrawingPart(session *validationTestSession, contentType, drawingXML string) {
	session.parts = append(session.parts, xmlPart("/xl/drawings/drawing1.xml", contentType))
	session.xmlParts["/xl/drawings/drawing1.xml"] = drawingXML
	session.xmlParts["/xl/worksheets/sheet1.xml"] = `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData/><drawing r:id="rIdDrawing1"/></worksheet>`
	session.relationships["/xl/worksheets/sheet1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/worksheets/sheet1.xml",
			ID:        "rIdDrawing1",
			Type:      namespaces.RelDrawing,
			Target:    "../drawings/drawing1.xml",
		},
	}
}

func xmlPart(uri, contentType string) opc.PartInfo {
	return opc.PartInfo{
		URI:         uri,
		ContentType: contentType,
		IsXML:       true,
	}
}

func removePart(parts []opc.PartInfo, uri string) []opc.PartInfo {
	var remaining []opc.PartInfo
	for _, part := range parts {
		if part.URI != uri {
			remaining = append(remaining, part)
		}
	}
	return remaining
}

func assertNoErrorDiagnostics(t *testing.T, diags []result.Diagnostic) {
	t.Helper()
	for _, d := range diags {
		if d.Severity == result.Error {
			t.Fatalf("unexpected error diagnostic %s: %s", d.Code, d.Message)
		}
	}
}

func assertNoDiagnosticCode(t *testing.T, diags []result.Diagnostic, code string) {
	t.Helper()
	for _, d := range diags {
		if d.Code == code {
			t.Fatalf("unexpected diagnostic %s: %s", d.Code, d.Message)
		}
	}
}

func assertHasDiagnosticCode(t *testing.T, diags []result.Diagnostic, code string) {
	t.Helper()
	for _, d := range diags {
		if d.Code == code {
			return
		}
	}
	t.Fatalf("expected diagnostic %s, got %+v", code, diags)
}
