package validate

import (
	"bytes"
	"image"
	"image/png"
	"path/filepath"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestValidateDOCXDispatchesDOCXSemantics(t *testing.T) {
	pkg := openValidateDOCXFixture(t, "minimal")
	defer pkg.Close()

	diags, err := ValidatePackage(pkg)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	for _, diag := range diags {
		if strings.HasPrefix(diag.Code, "PPTX_") || strings.HasPrefix(diag.Code, "XLSX_") {
			t.Fatalf("unexpected non-DOCX diagnostic: %+v", diag)
		}
	}
	if hasDiagnosticCode(diags, "DOCX_PARSE_ERROR") {
		t.Fatalf("unexpected parse diagnostic: %+v", diags)
	}
}

func TestValidateDOCXMissingDocument(t *testing.T) {
	pkg := openValidateDOCXFixture(t, "corrupted-missing-document")
	defer pkg.Close()

	diags, err := ValidatePackage(pkg)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	if !hasDiagnosticCode(diags, "DOCX_MISSING_DOCUMENT") {
		t.Fatalf("diagnostics = %+v, want DOCX_MISSING_DOCUMENT", diags)
	}
}

func TestValidateDOCXTableScaffold(t *testing.T) {
	pkg := openValidateDOCXFixture(t, "table")
	defer pkg.Close()

	diags, err := ValidatePackage(pkg)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	if !hasDiagnosticCode(diags, "DOCX_TABLE_SCAFFOLD") {
		t.Fatalf("diagnostics = %+v, want DOCX_TABLE_SCAFFOLD", diags)
	}
	if hasDiagnosticCode(diags, "DOCX_TABLE_GRID_MISMATCH") {
		t.Fatalf("diagnostics = %+v, did not want DOCX_TABLE_GRID_MISMATCH for table without tblGrid", diags)
	}
}

func TestValidateDOCXTableGridMismatch(t *testing.T) {
	pkg := openValidateDOCXFixture(t, "table")
	defer pkg.Close()

	doc, table := openFirstDOCXTableForValidationTest(t, pkg)
	setTableScaffoldForValidationTest(table, 1)
	replaceValidateDOCXDocumentForTest(t, pkg, doc)

	diags, err := ValidatePackage(pkg)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	if !hasDiagnosticCode(diags, "DOCX_TABLE_GRID_MISMATCH") {
		t.Fatalf("diagnostics = %+v, want DOCX_TABLE_GRID_MISMATCH", diags)
	}
	if hasDiagnosticCode(diags, "DOCX_TABLE_SCAFFOLD") {
		t.Fatalf("diagnostics = %+v, did not want DOCX_TABLE_SCAFFOLD", diags)
	}
}

func TestValidateDOCXTableGridSpanMatchesGrid(t *testing.T) {
	pkg := openValidateDOCXFixture(t, "merged-table")
	defer pkg.Close()

	doc, table := openFirstDOCXTableForValidationTest(t, pkg)
	rows := namespaces.FindChildren(table, namespaces.NsW, "tr")
	if len(rows) < 2 {
		t.Fatalf("fixture table rows = %d, want at least 2", len(rows))
	}
	for _, row := range rows[1:] {
		table.RemoveChild(row)
	}
	setTableScaffoldForValidationTest(table, 2)
	replaceValidateDOCXDocumentForTest(t, pkg, doc)

	diags, err := ValidatePackage(pkg)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	if hasDiagnosticCode(diags, "DOCX_TABLE_GRID_MISMATCH") {
		t.Fatalf("diagnostics = %+v, did not want DOCX_TABLE_GRID_MISMATCH for gridSpan width", diags)
	}
}

func TestValidateDOCXNestedTableScaffold(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:body>
			<w:tbl>
				<w:tblPr/>
				<w:tblGrid><w:gridCol w:w="0"/></w:tblGrid>
				<w:tr>
					<w:tc>
						<w:p/>
						<w:tbl>
							<w:tr><w:tc><w:p/></w:tc></w:tr>
						</w:tbl>
						<w:p/>
					</w:tc>
				</w:tr>
			</w:tbl>
		</w:body>
	</w:document>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_TABLE_SCAFFOLD")
}

func TestValidateDOCXNestedTableCellRequiresTrailingParagraph(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:body>
			<w:tbl>
				<w:tblPr/>
				<w:tblGrid><w:gridCol w:w="0"/></w:tblGrid>
				<w:tr>
					<w:tc>
						<w:p/>
						<w:tbl>
							<w:tblPr/>
							<w:tblGrid><w:gridCol w:w="0"/></w:tblGrid>
							<w:tr><w:tc><w:p/></w:tc></w:tr>
						</w:tbl>
					</w:tc>
				</w:tr>
			</w:tbl>
		</w:body>
	</w:document>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_TABLE_SCAFFOLD")
}

func TestValidateDOCXValidNestedTableCell(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:body>
			<w:tbl>
				<w:tblPr/>
				<w:tblGrid><w:gridCol w:w="0"/></w:tblGrid>
				<w:tr>
					<w:tc>
						<w:p/>
						<w:tbl>
							<w:tblPr/>
							<w:tblGrid><w:gridCol w:w="0"/></w:tblGrid>
							<w:tr><w:tc><w:p/></w:tc></w:tr>
						</w:tbl>
						<w:p/>
					</w:tc>
				</w:tr>
			</w:tbl>
		</w:body>
	</w:document>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	if hasDiagnosticCode(diags, "DOCX_TABLE_SCAFFOLD") {
		t.Fatalf("diagnostics = %+v, did not want DOCX_TABLE_SCAFFOLD", diags)
	}
	if hasDiagnosticCode(diags, "DOCX_TABLE_GRID_MISMATCH") {
		t.Fatalf("diagnostics = %+v, did not want DOCX_TABLE_GRID_MISMATCH", diags)
	}
}

func TestValidateDOCXStyleReferences(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:body>
			<w:p>
				<w:pPr><w:pStyle w:val="Emphasis"/></w:pPr>
				<w:r><w:rPr><w:rStyle w:val="MissingChar"/></w:rPr><w:t>styled</w:t></w:r>
			</w:p>
		</w:body>
	</w:document>`)
	addDOCXStylesPartForTest(session, `<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:style w:type="character" w:styleId="Emphasis"><w:name w:val="Emphasis"/></w:style>
	</w:styles>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_STYLE_TYPE_MISMATCH")
	assertHasDiagnosticCode(t, diags, "DOCX_MISSING_STYLE_REFERENCE")
}

func TestValidateDOCXMissingCommentReference(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:body>
			<w:p>
				<w:commentRangeStart w:id="7"/>
				<w:r><w:t>commented</w:t></w:r>
				<w:commentRangeEnd w:id="7"/>
				<w:r><w:commentReference w:id="7"/></w:r>
			</w:p>
		</w:body>
	</w:document>`)
	addDOCXCommentsPartForTest(session, `<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:comment w:id="6" w:author="Test"><w:p><w:r><w:t>other</w:t></w:r></w:p></w:comment>
	</w:comments>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_MISSING_COMMENT_REFERENCE")
}

func TestValidateDOCXCommentReferenceRequiresCommentsRelationship(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
		<w:body><w:p><w:r><w:commentReference w:id="1"/></w:r></w:p></w:body>
	</w:document>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_MISSING_COMMENTS")
}

func TestValidateDOCXDrawingExtentAndImageRelationship(t *testing.T) {
	session := newDOCXValidationSessionForTest(`<w:document
		xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
		xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
		xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
		xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
		<w:body>
			<w:p><w:r><w:drawing><wp:inline><wp:extent cx="0" cy="1000"/><a:graphic><a:graphicData><a:blip r:embed="rIdImage"/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
		</w:body>
	</w:document>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_DRAWING_INVALID_EXTENT")
	assertHasDiagnosticCode(t, diags, "DOCX_MISSING_IMAGE_RELATIONSHIP")
}

func TestValidateDOCXImagePayloadSignature(t *testing.T) {
	session := newDOCXValidationSessionForTest(docxDocumentWithImageRelationship("rIdImage"))
	addDOCXImagePartForTest(session, "rIdImage", "/word/media/image1.png", "image/png", []byte("not really a png"))

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_IMAGE_PAYLOAD")
}

func TestValidateDOCXImageRelationshipRequiresImageContentType(t *testing.T) {
	session := newDOCXValidationSessionForTest(docxDocumentWithImageRelationship("rIdImage"))
	addDOCXImagePartForTest(session, "rIdImage", "/word/media/media1.mp4", "video/mp4", []byte("not image bytes"))

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_IMAGE_CONTENT_TYPE")
}

func TestValidateDOCXImagePayloadAllowsValidAndUnknownImageTypes(t *testing.T) {
	session := newDOCXValidationSessionForTest(docxDocumentWithImageRelationship("rIdImage"))
	addDOCXImagePartForTest(session, "rIdImage", "/word/media/image1.png", "image/png", minimalDOCXPNGBytes())
	addDOCXImagePartForTest(session, "rIdSvg", "/word/media/vector.svg", "image/svg+xml", []byte("<svg/>"))
	session.xmlParts["/word/document.xml"] = docxDocumentWithTwoImageRelationships("rIdImage", "rIdSvg")

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "DOCX_IMAGE_PAYLOAD")
}

func TestValidateDOCXHeaderImagePayloadSignature(t *testing.T) {
	session := newDOCXValidationSessionForTest(docxDocumentWithHeaderReference("rIdHeader"))
	addDOCXHeaderPartForTest(session, "rIdHeader", "/word/header1.xml", docxHeaderWithImageRelationship("rIdHeaderImage"))
	addDOCXImagePartForSourceForTest(session, "/word/header1.xml", "rIdHeaderImage", "/word/media/header.png", "image/png", []byte("not really a png"))

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "DOCX_IMAGE_PAYLOAD")
}

func openValidateDOCXFixture(t *testing.T, name string) *opc.Package {
	t.Helper()
	path := filepath.Join("..", "..", "testdata", "docx", name, "document.docx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture %s: %v", name, err)
	}
	return pkg
}

func openFirstDOCXTableForValidationTest(t *testing.T, pkg *opc.Package) (*etree.Document, *etree.Element) {
	t.Helper()
	doc, err := pkg.ReadXMLPart("/word/document.xml")
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	tables := namespaces.FindDescendants(doc.Root(), namespaces.NsW, "tbl")
	if len(tables) == 0 {
		t.Fatal("fixture has no DOCX table")
	}
	return doc, tables[0]
}

func replaceValidateDOCXDocumentForTest(t *testing.T, pkg *opc.Package, doc *etree.Document) {
	t.Helper()
	if err := pkg.ReplaceXMLPart("/word/document.xml", doc); err != nil {
		t.Fatalf("ReplaceXMLPart returned error: %v", err)
	}
}

func setTableScaffoldForValidationTest(table *etree.Element, gridCols int) {
	if tblPr := namespaces.FindChild(table, namespaces.NsW, "tblPr"); tblPr != nil {
		table.RemoveChild(tblPr)
	}
	if tblGrid := namespaces.FindChild(table, namespaces.NsW, "tblGrid"); tblGrid != nil {
		table.RemoveChild(tblGrid)
	}

	firstRowIndex := len(table.Child)
	for _, child := range table.ChildElements() {
		if namespaces.IsElement(child, namespaces.NsW, "tr") {
			firstRowIndex = child.Index()
			break
		}
	}

	tblPr := wordElementForValidationTest("tblPr")
	tblGrid := wordElementForValidationTest("tblGrid")
	for i := 0; i < gridCols; i++ {
		gridCol := wordElementForValidationTest("gridCol")
		gridCol.CreateAttr("w:w", "0")
		tblGrid.AddChild(gridCol)
	}
	table.InsertChildAt(firstRowIndex, tblPr)
	table.InsertChildAt(firstRowIndex+1, tblGrid)
}

func wordElementForValidationTest(localName string) *etree.Element {
	elem := etree.NewElement(localName)
	elem.Space = "w"
	return elem
}

func newDOCXValidationSessionForTest(documentXML string) *validationTestSession {
	return &validationTestSession{
		parts: []opc.PartInfo{
			xmlPart("/[Content_Types].xml", "application/xml"),
			xmlPart("/_rels/.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			xmlPart("/word/document.xml", namespaces.ContentTypeDocument),
			xmlPart("/word/_rels/document.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"),
		},
		relationships: map[string][]opc.RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      namespaces.RelOfficeDocument,
					Target:    "word/document.xml",
				},
			},
			"/word/document.xml": {},
		},
		xmlParts: map[string]string{
			"/[Content_Types].xml":          `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`,
			"/_rels/.rels":                  `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/word/document.xml":            documentXML,
			"/word/_rels/document.xml.rels": `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
		},
	}
}

func addDOCXImagePartForTest(session *validationTestSession, rid, uri, contentType string, raw []byte) {
	addDOCXImagePartForSourceForTest(session, "/word/document.xml", rid, uri, contentType, raw)
}

func addDOCXImagePartForSourceForTest(session *validationTestSession, sourceURI, rid, uri, contentType string, raw []byte) {
	session.parts = append(session.parts, opc.PartInfo{URI: uri, ContentType: contentType, SizeBytes: int64(len(raw))})
	session.relationships[sourceURI] = append(session.relationships[sourceURI], opc.RelationshipInfo{
		SourceURI: sourceURI,
		ID:        rid,
		Type:      namespaces.RelImage,
		Target:    strings.TrimPrefix(uri, "/word/"),
	})
	if session.rawParts == nil {
		session.rawParts = map[string][]byte{}
	}
	session.rawParts[uri] = raw
}

func addDOCXHeaderPartForTest(session *validationTestSession, rid, uri, headerXML string) {
	session.parts = append(session.parts,
		xmlPart(uri, namespaces.ContentTypeHeader),
		xmlPart("/word/_rels/header1.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"),
	)
	session.relationships["/word/document.xml"] = append(session.relationships["/word/document.xml"], opc.RelationshipInfo{
		SourceURI: "/word/document.xml",
		ID:        rid,
		Type:      namespaces.RelHeader,
		Target:    strings.TrimPrefix(uri, "/word/"),
	})
	session.relationships[uri] = []opc.RelationshipInfo{}
	session.xmlParts[uri] = headerXML
	session.xmlParts["/word/_rels/header1.xml.rels"] = `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`
}

func docxDocumentWithImageRelationship(rid string) string {
	return docxDocumentWithTwoImageRelationships(rid, "")
}

func docxDocumentWithTwoImageRelationships(firstRID, secondRID string) string {
	secondBlip := ""
	if secondRID != "" {
		secondBlip = `<a:blip r:embed="` + secondRID + `"/>`
	}
	return `<w:document
		xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
		xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
		xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
		xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
		<w:body>
			<w:p><w:r><w:drawing><wp:inline><wp:extent cx="1000" cy="1000"/><a:graphic><a:graphicData><a:blip r:embed="` + firstRID + `"/>` + secondBlip + `</a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>
		</w:body>
	</w:document>`
}

func docxDocumentWithHeaderReference(rid string) string {
	return `<w:document
		xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
		xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
		<w:body><w:sectPr><w:headerReference w:type="default" r:id="` + rid + `"/></w:sectPr></w:body>
	</w:document>`
}

func docxHeaderWithImageRelationship(rid string) string {
	return `<w:hdr
		xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
		xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
		xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
		<w:p><w:r><w:drawing><a:blip r:embed="` + rid + `"/></w:drawing></w:r></w:p>
	</w:hdr>`
}

func minimalDOCXPNGBytes() []byte {
	var buf bytes.Buffer
	if err := png.Encode(&buf, image.NewRGBA(image.Rect(0, 0, 1, 1))); err != nil {
		panic(err)
	}
	return buf.Bytes()
}

func addDOCXStylesPartForTest(session *validationTestSession, stylesXML string) {
	session.parts = append(session.parts, xmlPart("/word/styles.xml", namespaces.ContentTypeStyles))
	session.relationships["/word/document.xml"] = append(session.relationships["/word/document.xml"], opc.RelationshipInfo{
		SourceURI: "/word/document.xml",
		ID:        "rIdStyles",
		Type:      namespaces.RelStyles,
		Target:    "styles.xml",
	})
	session.xmlParts["/word/styles.xml"] = stylesXML
}

func addDOCXCommentsPartForTest(session *validationTestSession, commentsXML string) {
	session.parts = append(session.parts, xmlPart("/word/comments.xml", namespaces.ContentTypeComments))
	session.relationships["/word/document.xml"] = append(session.relationships["/word/document.xml"], opc.RelationshipInfo{
		SourceURI: "/word/document.xml",
		ID:        "rIdComments",
		Type:      namespaces.RelComments,
		Target:    "comments.xml",
	})
	session.xmlParts["/word/comments.xml"] = commentsXML
}

func hasDiagnosticCode(diags []result.Diagnostic, code string) bool {
	for _, diag := range diags {
		if diag.Code == code {
			return true
		}
	}
	return false
}
