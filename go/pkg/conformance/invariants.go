package conformance

import (
	"bytes"
	"encoding/xml"
	"fmt"
	"io"
	"path"
	"strconv"
	"strings"
	"time"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	docxns "github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxns "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

const (
	contentTypesPartURI   = "/[Content_Types].xml"
	contentTypesNamespace = "http://schemas.openxmlformats.org/package/2006/content-types"
	drawingMLNamespace    = "http://schemas.openxmlformats.org/drawingml/2006/main"

	contentTypePPTXSlide = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
	contentTypeChart     = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml"
	contentTypeDrawing   = "application/vnd.openxmlformats-officedocument.drawing+xml"

	contentTypePPTXPresentation      = "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
	contentTypePPTXPresentationMacro = "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml"
	contentTypePPTXTemplate          = "application/vnd.openxmlformats-officedocument.presentationml.template.main+xml"
	contentTypePPTXTemplateMacro     = "application/vnd.ms-powerpoint.template.macroEnabled.main+xml"
	contentTypePPTXSlideshow         = "application/vnd.openxmlformats-officedocument.presentationml.slideshow.main+xml"
	contentTypePPTXSlideshowMacro    = "application/vnd.ms-powerpoint.slideshow.macroEnabled.main+xml"
	contentTypePPTXSlideLayout       = "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"
	contentTypePPTXSlideMaster       = "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"
	contentTypePPTXNotesSlide        = "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"
	contentTypePPTXNotesMaster       = "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"
	contentTypePPTXTheme             = "application/vnd.openxmlformats-officedocument.theme+xml"
	contentTypePPTXTableStyles       = "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"
	contentTypePPTXComments          = "application/vnd.openxmlformats-officedocument.presentationml.comments+xml"
	contentTypePPTXCommentAuthors    = "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml"
	contentTypePPTXPresProps         = "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"
	contentTypePPTXViewProps         = "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"
	contentTypeXLSXChartSheet        = "application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml"
	contentTypeXLSXDialogSheet       = "application/vnd.openxmlformats-officedocument.spreadsheetml.dialogsheet+xml"
	contentTypeDOCXDocumentMacro     = "application/vnd.ms-word.document.macroEnabled.main+xml"
	contentTypeDOCXTemplate          = "application/vnd.openxmlformats-officedocument.wordprocessingml.template.main+xml"
	contentTypeDOCXTemplateMacro     = "application/vnd.ms-word.template.macroEnabledTemplate.main+xml"

	relTypePPTXSlide          = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"
	relTypePPTXSlideLayout    = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"
	relTypePPTXSlideMaster    = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster"
	relTypePPTXNotesSlide     = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide"
	relTypePPTXNotesMaster    = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster"
	relTypeOfficeTheme        = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme"
	relTypePPTXCommentAuthors = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors"
	relTypeImage              = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
	relTypeVideo              = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video"
	relTypeAudio              = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio"
	relTypeMedia              = "http://schemas.microsoft.com/office/2007/relationships/media"
	relTypePackage            = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package"

	relTypeXLSXChartSheet  = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartsheet"
	relTypeXLSXDialogSheet = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/dialogsheet"
)

var minZipModifiedTime = time.Date(1980, time.January, 1, 0, 0, 0, 0, time.UTC)

// CheckRepairInvariants validates high-value OOXML invariants that commonly
// cause Microsoft Office to repair a file even when the XML is well-formed.
func CheckRepairInvariants(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic
	styles := collectStylesReferenceInfo(session)
	diags = append(diags, checkContentTypesPart(session)...)
	diags = append(diags, checkPackageRelationshipClosure(session)...)
	for _, part := range session.ListParts() {
		diags = append(diags, checkKnownPartContentType(part.URI, part.ContentType)...)
		diags = append(diags, checkZipEntryMetadata(part.URI, session.GetZipMeta(part.URI))...)
		if opc.IsRelsFile(part.URI) {
			data, err := session.ReadRawPart(part.URI)
			if err != nil {
				diags = append(diags, diag.Errorf("OOXML_RELS_READ_ERROR", "failed to read relationships part %s: %v", part.URI, err))
				continue
			}
			if _, err := opc.ParseRelationships(sourceURIFromRelsPath(part.URI), data); err != nil {
				diags = append(diags, diag.Errorf("OOXML_RELS_PARSE_ERROR", "failed to parse relationships part %s: %v", part.URI, err))
			}
			continue
		}
		switch {
		case isXLSXWorkbookContentType(part.ContentType):
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "workbook", "workbook", xlsxns.NsSpreadsheetML, "XLSX_WORKBOOK_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkWorkbookSheetReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
			diags = append(diags, checkWorkbookPivotCacheReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
			diags = append(diags, checkWorkbookDefinedNames(part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypeSharedStrings:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "shared strings", "sst", xlsxns.NsSpreadsheetML, "XLSX_SHARED_STRINGS_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkSharedStringCounts(part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypeStyles:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "styles", "styleSheet", xlsxns.NsSpreadsheetML, "XLSX_STYLES_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkStylesCounts(part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypeCalcChain:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "calc chain", "calcChain", xlsxns.NsSpreadsheetML, "XLSX_CALC_CHAIN_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkCalcChainReferences(session, part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypeTable:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "table", "table", xlsxns.NsSpreadsheetML, "XLSX_TABLE_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkTableDefinition(part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypePivotTable:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "pivot table", "pivotTableDefinition", xlsxns.NsSpreadsheetML, "XLSX_PIVOT_TABLE_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkPivotTableDefinition(part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypePivotCache:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "pivot cache definition", "pivotCacheDefinition", xlsxns.NsSpreadsheetML, "XLSX_PIVOT_CACHE_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkPivotCacheDefinition(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
		case part.ContentType == xlsxns.ContentTypePivotRecords:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "pivot cache records", "pivotCacheRecords", xlsxns.NsSpreadsheetML, "XLSX_PIVOT_RECORDS_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkPivotRecordsDefinition(part.URI, doc.Root())...)
		case isPPTXPresentationContentType(part.ContentType):
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "presentation", "presentation", pptxns.NsP, "PPTX_PRESENTATION_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkPresentationReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
		case part.ContentType == docxns.ContentTypeDocument:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "document", "document", docxns.NsW, "DOCX_DOCUMENT_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkDOCXDrawingImageRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == docxns.ContentTypeHeader:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "header", "hdr", docxns.NsW, "DOCX_HEADER_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkDOCXDrawingImageRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == docxns.ContentTypeFooter:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "footer", "ftr", docxns.NsW, "DOCX_FOOTER_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkDOCXDrawingImageRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == xlsxns.ContentTypeWorksheet:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "worksheet", "worksheet", xlsxns.NsSpreadsheetML, "XLSX_WORKSHEET_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkElementOrder(part.URI, doc.Root(), worksheetChildOrder, "XLSX_WORKSHEET_CHILD_ORDER")...)
			diags = append(diags, checkWorksheetRelationshipReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
			diags = append(diags, checkWorksheetStyleReferences(part.URI, doc.Root(), styles)...)
		case part.ContentType == contentTypePPTXSlide:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "slide", "sld", pptxns.NsP, "PPTX_SLIDE_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkElementOrder(part.URI, doc.Root(), slideChildOrder, "PPTX_SLIDE_CHILD_ORDER")...)
			diags = append(diags, checkSlideAnimationTargets(part.URI, doc.Root())...)
			diags = append(diags, checkChartRelationshipReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
			diags = append(diags, checkDrawingMediaRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == contentTypePPTXSlideLayout:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "slide layout", "sldLayout", pptxns.NsP, "PPTX_SLIDE_LAYOUT_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkElementOrder(part.URI, doc.Root(), slideLayoutChildOrder, "PPTX_SLIDE_LAYOUT_CHILD_ORDER")...)
			diags = append(diags, checkSlideLayoutMasterRelationship(part.URI, session.ListRelationships(part.URI))...)
			diags = append(diags, checkDrawingMediaRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == contentTypePPTXSlideMaster:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "slide master", "sldMaster", pptxns.NsP, "PPTX_SLIDE_MASTER_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkElementOrder(part.URI, doc.Root(), slideMasterChildOrder, "PPTX_SLIDE_MASTER_CHILD_ORDER")...)
			diags = append(diags, checkSlideMasterLayoutReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
			diags = append(diags, checkDrawingMediaRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == contentTypeDrawing:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "drawing", "wsDr", xlsxns.NsSpreadsheetDrawing, "XLSX_DRAWING_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkWorksheetDrawing(part.URI, doc.Root())...)
			diags = append(diags, checkChartRelationshipReferences(part.URI, doc.Root(), session.ListRelationships(part.URI))...)
			diags = append(diags, checkDrawingMediaRelationshipReferences(session, part.URI, doc.Root())...)
		case part.ContentType == contentTypeChart:
			doc, rootDiags := readXMLPartAndCheckRoot(session, part.URI, "chart", "chartSpace", xlsxns.NsChart, "OOXML_CHART_ROOT")
			if len(rootDiags) > 0 {
				diags = append(diags, rootDiags...)
				continue
			}
			diags = append(diags, checkChartPart(part.URI, doc.Root())...)
			diags = append(diags, checkChartExternalDataRelationshipReferences(session, part.URI, doc.Root())...)
		}
	}
	return diags, nil
}

func checkContentTypesPart(session opc.PackageSession) []result.Diagnostic {
	if !sessionHasPart(session, contentTypesPartURI) {
		return nil
	}
	data, err := session.ReadRawPart(contentTypesPartURI)
	if err != nil {
		return []result.Diagnostic{
			diag.Errorf("OOXML_CONTENT_TYPES_READ_ERROR", "failed to read %s: %v", contentTypesPartURI, err),
		}
	}
	return checkContentTypesXML(data, sessionPartSet(session))
}

func checkContentTypesXML(data []byte, parts map[string]bool) []result.Diagnostic {
	decoder := xml.NewDecoder(bytes.NewReader(data))
	var diags []result.Diagnostic
	seenRoot := false
	rootOK := false
	parseOK := true
	seenDefaults := make(map[string]bool)
	seenOverrides := make(map[string]bool)
	for {
		tok, err := decoder.Token()
		if err != nil {
			if err != io.EOF {
				diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_PARSE_ERROR", "failed to parse %s: %v", contentTypesPartURI, err))
				parseOK = false
			}
			break
		}
		start, ok := tok.(xml.StartElement)
		if !ok {
			continue
		}
		if !seenRoot {
			seenRoot = true
			if start.Name.Space != contentTypesNamespace || start.Name.Local != "Types" {
				diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_ROOT", "%s root is {%s}%s, expected {%s}Types", contentTypesPartURI, start.Name.Space, start.Name.Local, contentTypesNamespace))
			} else {
				rootOK = true
			}
			continue
		}
		if start.Name.Space != contentTypesNamespace {
			continue
		}
		switch start.Name.Local {
		case "Default":
			extension := strings.TrimSpace(xmlAttr(start, "Extension"))
			contentType := strings.TrimSpace(xmlAttr(start, "ContentType"))
			if extension == "" || contentType == "" {
				diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_DEFAULT_REQUIRED", "%s <Default> must have non-empty Extension and ContentType attributes", contentTypesPartURI))
				continue
			}
			if seenDefaults[extension] {
				diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_DEFAULT_DUPLICATE", "%s repeats Default Extension %q", contentTypesPartURI, extension))
			}
			seenDefaults[extension] = true
		case "Override":
			rawPartName := strings.TrimSpace(xmlAttr(start, "PartName"))
			partName := opc.NormalizeURI(rawPartName)
			contentType := strings.TrimSpace(xmlAttr(start, "ContentType"))
			if rawPartName == "" || !strings.HasPrefix(rawPartName, "/") || partName == "/" || contentType == "" {
				diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_OVERRIDE_REQUIRED", "%s <Override> must have non-empty absolute PartName and ContentType attributes", contentTypesPartURI))
				continue
			}
			if seenOverrides[partName] {
				diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_OVERRIDE_DUPLICATE", "%s repeats Override PartName %q", contentTypesPartURI, partName))
			}
			seenOverrides[partName] = true
		}
	}
	if !seenRoot {
		diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_ROOT", "%s has no XML root", contentTypesPartURI))
	}
	if parseOK && rootOK {
		diags = append(diags, checkContentTypesCoverage(parts, seenDefaults, seenOverrides)...)
	}
	return diags
}

func xmlAttr(start xml.StartElement, local string) string {
	for _, attr := range start.Attr {
		if attr.Name.Local == local {
			return attr.Value
		}
	}
	return ""
}

func sessionHasPart(session opc.PackageSession, uri string) bool {
	uri = opc.NormalizeURI(uri)
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return true
		}
	}
	return false
}

func sessionPartSet(session opc.PackageSession) map[string]bool {
	parts := make(map[string]bool)
	for _, part := range session.ListParts() {
		parts[opc.NormalizeURI(part.URI)] = true
	}
	return parts
}

func checkContentTypesCoverage(parts map[string]bool, defaults, overrides map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	for partName := range overrides {
		if !parts[partName] {
			diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING", "%s Override PartName %q does not match a package part", contentTypesPartURI, partName))
		}
	}
	for partURI := range parts {
		if partURI == contentTypesPartURI {
			continue
		}
		if overrides[partURI] {
			continue
		}
		extension := strings.TrimSpace(opc.GetFileExtension(partURI))
		if extension != "" && defaults[extension] {
			continue
		}
		diags = append(diags, diag.Errorf("OOXML_CONTENT_TYPES_PART_UNMAPPED", "%s has no matching Override and no Default for extension %q", partURI, extension))
	}
	return diags
}

func checkKnownPartContentType(partURI, contentType string) []result.Diagnostic {
	expected := expectedContentTypesForPart(partURI)
	if len(expected) == 0 || containsString(expected, contentType) {
		return nil
	}
	return []result.Diagnostic{
		diag.Errorf("OOXML_CONTENT_TYPE_MISMATCH", "%s has content type %q, expected one of: %s", partURI, contentType, strings.Join(expected, ", ")),
	}
}

func expectedContentTypesForPart(partURI string) []string {
	uri := opc.NormalizeURI(partURI)
	base := path.Base(uri)
	switch {
	case opc.IsRelsFile(uri):
		return []string{opc.ContentTypeRelationships}
	case uri == "/xl/workbook.xml":
		return []string{xlsxns.ContentTypeWorkbook, xlsxns.ContentTypeWorkbookMacro, xlsxns.ContentTypeWorkbookTemplate, xlsxns.ContentTypeWorkbookAddin}
	case uri == "/xl/sharedStrings.xml":
		return []string{xlsxns.ContentTypeSharedStrings}
	case uri == "/xl/styles.xml":
		return []string{xlsxns.ContentTypeStyles}
	case uri == "/xl/calcChain.xml":
		return []string{xlsxns.ContentTypeCalcChain}
	case uri == "/ppt/presentation.xml":
		return []string{contentTypePPTXPresentation, contentTypePPTXPresentationMacro, contentTypePPTXTemplate, contentTypePPTXTemplateMacro, contentTypePPTXSlideshow, contentTypePPTXSlideshowMacro}
	case uri == "/ppt/tableStyles.xml":
		return []string{contentTypePPTXTableStyles}
	case uri == "/ppt/commentAuthors.xml":
		return []string{contentTypePPTXCommentAuthors}
	case uri == "/ppt/presProps.xml":
		return []string{contentTypePPTXPresProps}
	case uri == "/ppt/viewProps.xml":
		return []string{contentTypePPTXViewProps}
	case uri == "/word/document.xml":
		return []string{docxns.ContentTypeDocument, contentTypeDOCXDocumentMacro, contentTypeDOCXTemplate, contentTypeDOCXTemplateMacro}
	case uri == "/word/styles.xml":
		return []string{docxns.ContentTypeStyles}
	case uri == "/word/numbering.xml":
		return []string{docxns.ContentTypeNumbering}
	case uri == "/word/footnotes.xml":
		return []string{docxns.ContentTypeFootnotes}
	case uri == "/word/endnotes.xml":
		return []string{docxns.ContentTypeEndnotes}
	case uri == "/word/comments.xml":
		return []string{docxns.ContentTypeComments}
	case strings.HasPrefix(uri, "/xl/worksheets/") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypeWorksheet}
	case strings.HasPrefix(uri, "/xl/chartsheets/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypeXLSXChartSheet}
	case strings.HasPrefix(uri, "/xl/dialogSheets/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypeXLSXDialogSheet}
	case strings.HasPrefix(uri, "/xl/tables/") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypeTable}
	case strings.HasPrefix(uri, "/xl/charts/") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypeChart}
	case strings.HasPrefix(uri, "/xl/drawings/") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypeDrawing}
	case strings.HasPrefix(uri, "/xl/pivotTables/") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypePivotTable}
	case strings.HasPrefix(uri, "/xl/pivotCache/") && strings.HasPrefix(base, "pivotCacheDefinition") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypePivotCache}
	case strings.HasPrefix(uri, "/xl/pivotCache/") && strings.HasPrefix(base, "pivotCacheRecords") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypePivotRecords}
	case strings.HasPrefix(uri, "/xl/comments") && strings.HasSuffix(uri, ".xml"):
		return []string{xlsxns.ContentTypeComments}
	case strings.HasPrefix(uri, "/xl/drawings/") && strings.HasSuffix(uri, ".vml"):
		return []string{xlsxns.ContentTypeVml}
	case strings.HasPrefix(uri, "/word/header") && strings.HasSuffix(uri, ".xml"):
		return []string{docxns.ContentTypeHeader}
	case strings.HasPrefix(uri, "/word/footer") && strings.HasSuffix(uri, ".xml"):
		return []string{docxns.ContentTypeFooter}
	case strings.HasPrefix(uri, "/ppt/slides/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXSlide}
	case strings.HasPrefix(uri, "/ppt/slideLayouts/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXSlideLayout}
	case strings.HasPrefix(uri, "/ppt/slideMasters/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXSlideMaster}
	case strings.HasPrefix(uri, "/ppt/notesSlides/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXNotesSlide}
	case strings.HasPrefix(uri, "/ppt/notesMasters/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXNotesMaster}
	case strings.HasPrefix(uri, "/ppt/theme/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXTheme}
	case strings.HasPrefix(uri, "/ppt/charts/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypeChart}
	case strings.HasPrefix(uri, "/ppt/comments/") && strings.HasSuffix(uri, ".xml"):
		return []string{contentTypePPTXComments}
	default:
		return nil
	}
}

func containsString(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}

func checkPackageRelationshipClosure(session opc.PackageSession) []result.Diagnostic {
	var diags []result.Diagnostic
	parts := make(map[string]bool)
	contentTypes := make(map[string]string)
	relationshipSources := map[string]bool{"/": true}
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		parts[uri] = true
		contentTypes[uri] = part.ContentType
	}
	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		if opc.IsRelsFile(uri) {
			sourceURI := sourceURIFromRelsPath(uri)
			relationshipSources[sourceURI] = true
			if sourceURI != "/" && !parts[sourceURI] {
				diags = append(diags, diag.Errorf("OOXML_RELS_ORPHANED", "%s is a relationships part for missing source part %s", uri, sourceURI))
			}
			continue
		}
		relationshipSources[uri] = true
	}

	for sourceURI := range relationshipSources {
		seenIDs := make(map[string]bool)
		for _, rel := range session.ListRelationships(sourceURI) {
			label := relationshipLabel(sourceURI, rel)
			if strings.TrimSpace(rel.ID) == "" {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_MISSING_ID", "%s is missing Id", label))
			} else if seenIDs[rel.ID] {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_DUPLICATE_ID", "%s duplicates Id %s", label, rel.ID))
			}
			seenIDs[rel.ID] = true

			if strings.TrimSpace(rel.Type) == "" {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_MISSING_TYPE", "%s is missing Type", label))
			}
			if strings.TrimSpace(rel.Target) == "" {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_MISSING_TARGET", "%s is missing Target", label))
				continue
			}
			if rel.TargetMode != "" && rel.TargetMode != "External" {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_TARGET_MODE", "%s has unsupported TargetMode %q", label, rel.TargetMode))
			}
			if rel.TargetMode == "External" {
				continue
			}
			if looksExternalRelationshipTarget(rel.Target) {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_EXTERNAL_MODE_MISSING", "%s target %q looks external but TargetMode is not External", label, rel.Target))
				continue
			}
			targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, rel.Target))
			if !parts[targetURI] {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_TARGET_MISSING", "%s points to missing part %s", label, targetURI))
				continue
			}
			if expected := expectedRelationshipTargetContentTypes(sourceURI, targetURI, rel.Type); len(expected) > 0 && !containsString(expected, contentTypes[targetURI]) {
				diags = append(diags, diag.Errorf("OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "%s has type %q but target %s has content type %q; expected one of: %s", label, rel.Type, targetURI, contentTypes[targetURI], strings.Join(expected, ", ")))
			}
		}
	}

	return diags
}

func expectedRelationshipTargetContentTypes(sourceURI, targetURI, relType string) []string {
	sourceURI = opc.NormalizeURI(sourceURI)
	targetURI = opc.NormalizeURI(targetURI)
	switch relType {
	case xlsxns.RelOfficeDocument:
		switch {
		case strings.HasPrefix(targetURI, "/xl/"):
			return []string{xlsxns.ContentTypeWorkbook, xlsxns.ContentTypeWorkbookMacro, xlsxns.ContentTypeWorkbookTemplate, xlsxns.ContentTypeWorkbookAddin}
		case strings.HasPrefix(targetURI, "/ppt/"):
			return []string{contentTypePPTXPresentation, contentTypePPTXPresentationMacro, contentTypePPTXTemplate, contentTypePPTXTemplateMacro, contentTypePPTXSlideshow, contentTypePPTXSlideshowMacro}
		case strings.HasPrefix(targetURI, "/word/"):
			return []string{docxns.ContentTypeDocument, contentTypeDOCXDocumentMacro, contentTypeDOCXTemplate, contentTypeDOCXTemplateMacro}
		default:
			return nil
		}
	case xlsxns.RelWorksheet:
		return []string{xlsxns.ContentTypeWorksheet}
	case relTypeXLSXChartSheet:
		return []string{contentTypeXLSXChartSheet}
	case relTypeXLSXDialogSheet:
		return []string{contentTypeXLSXDialogSheet}
	case xlsxns.RelSharedStrings:
		return []string{xlsxns.ContentTypeSharedStrings}
	case xlsxns.RelStyles:
		switch {
		case strings.HasPrefix(sourceURI, "/word/") || strings.HasPrefix(targetURI, "/word/"):
			return []string{docxns.ContentTypeStyles}
		case strings.HasPrefix(sourceURI, "/xl/") || strings.HasPrefix(targetURI, "/xl/"):
			return []string{xlsxns.ContentTypeStyles}
		default:
			return nil
		}
	case docxns.RelNumbering:
		return []string{docxns.ContentTypeNumbering}
	case docxns.RelHeader:
		return []string{docxns.ContentTypeHeader}
	case docxns.RelFooter:
		return []string{docxns.ContentTypeFooter}
	case docxns.RelFootnotes:
		return []string{docxns.ContentTypeFootnotes}
	case docxns.RelEndnotes:
		return []string{docxns.ContentTypeEndnotes}
	case xlsxns.RelCalcChain:
		return []string{xlsxns.ContentTypeCalcChain}
	case xlsxns.RelTable:
		return []string{xlsxns.ContentTypeTable}
	case xlsxns.RelDrawing:
		return []string{xlsxns.ContentTypeDrawing}
	case xlsxns.RelChart:
		return []string{contentTypeChart}
	case xlsxns.RelPivotTable:
		return []string{xlsxns.ContentTypePivotTable}
	case xlsxns.RelPivotCache:
		return []string{xlsxns.ContentTypePivotCache}
	case xlsxns.RelPivotRecords:
		return []string{xlsxns.ContentTypePivotRecords}
	case xlsxns.RelComments:
		switch {
		case strings.HasPrefix(sourceURI, "/ppt/") || strings.HasPrefix(targetURI, "/ppt/"):
			return []string{contentTypePPTXComments}
		case strings.HasPrefix(sourceURI, "/xl/") || strings.HasPrefix(targetURI, "/xl/"):
			return []string{xlsxns.ContentTypeComments}
		case strings.HasPrefix(sourceURI, "/word/") || strings.HasPrefix(targetURI, "/word/"):
			return []string{docxns.ContentTypeComments}
		default:
			return nil
		}
	case relTypePPTXSlide:
		return []string{contentTypePPTXSlide}
	case relTypePPTXSlideLayout:
		return []string{contentTypePPTXSlideLayout}
	case relTypePPTXSlideMaster:
		return []string{contentTypePPTXSlideMaster}
	case relTypePPTXNotesSlide:
		return []string{contentTypePPTXNotesSlide}
	case relTypePPTXNotesMaster:
		return []string{contentTypePPTXNotesMaster}
	case relTypeOfficeTheme:
		return []string{contentTypePPTXTheme}
	case relTypePPTXCommentAuthors:
		return []string{contentTypePPTXCommentAuthors}
	default:
		return nil
	}
}

func checkWorkbookSheetReferences(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	sheets := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "sheets")
	if sheets == nil {
		return nil
	}
	relMap := relationshipsByID(rels)
	var diags []result.Diagnostic
	for idx, sheet := range xlsxns.FindChildren(sheets, xlsxns.NsSpreadsheetML, "sheet") {
		label := workbookSheetLabel(idx+1, sheet)
		rid := relationshipIDAttr(sheet)
		if rid == "" {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_SHEET_REFERENCE", "%s %s is missing required r:id for its worksheet relationship", partURI, label))
			continue
		}
		rel, ok := relMap[rid]
		if !ok {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_SHEET_REFERENCE", "%s %s references missing workbook relationship %s", partURI, label, rid))
			continue
		}
		if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_SHEET_REFERENCE", "%s %s relationship %s points to an external target; workbook sheets must resolve to internal worksheet parts", partURI, label, rid))
			continue
		}
		if !isWorkbookSheetRelationshipType(rel.Type) {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_SHEET_REFERENCE", "%s %s relationship %s has type %q, expected a workbook sheet relationship", partURI, label, rid, rel.Type))
		}
	}
	return diags
}

func isWorkbookSheetRelationshipType(relType string) bool {
	switch relType {
	case xlsxns.RelWorksheet, relTypeXLSXChartSheet, relTypeXLSXDialogSheet:
		return true
	default:
		return false
	}
}

func checkWorkbookPivotCacheReferences(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	pivotCaches := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "pivotCaches")
	if pivotCaches == nil {
		return nil
	}
	relMap := relationshipsByID(rels)
	seenCacheIDs := map[int]string{}
	var diags []result.Diagnostic
	for idx, elem := range xlsxns.FindChildren(pivotCaches, xlsxns.NsSpreadsheetML, "pivotCache") {
		label := workbookPivotCacheLabel(idx+1, elem)
		rawCacheID := strings.TrimSpace(elem.SelectAttrValue("cacheId", ""))
		if rawCacheID == "" {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "%s %s is missing required cacheId", partURI, label))
		} else if cacheID, err := strconv.Atoi(rawCacheID); err != nil || cacheID <= 0 {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "%s %s has invalid cacheId %q", partURI, label, rawCacheID))
		} else if first := seenCacheIDs[cacheID]; first != "" {
			diags = append(diags, diag.Errorf("XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "%s %s duplicates %s cacheId %d", partURI, label, first, cacheID))
		} else {
			seenCacheIDs[cacheID] = label
		}
		diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, xlsxns.RelPivotCache, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE")...)
	}
	return diags
}

func checkWorkbookDefinedNames(partURI string, root *etree.Element) []result.Diagnostic {
	definedNames := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "definedNames")
	if definedNames == nil {
		return nil
	}
	sheetNames, sheetCount := workbookSheetNames(root)
	seenByScope := make(map[string]string)
	var diags []result.Diagnostic
	for idx, elem := range xlsxns.FindChildren(definedNames, xlsxns.NsSpreadsheetML, "definedName") {
		label := definedNameLabel(idx+1, elem)
		name := strings.TrimSpace(elem.SelectAttrValue("name", ""))
		if name == "" {
			diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_REQUIRED", "%s %s is missing required name", partURI, label))
		}

		scopeKey := "workbook"
		if rawScope := strings.TrimSpace(elem.SelectAttrValue("localSheetId", "")); rawScope != "" {
			scopeKey = "sheet:" + rawScope
			localSheetID, err := strconv.Atoi(rawScope)
			if err != nil || localSheetID < 0 {
				diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_SCOPE", "%s %s has invalid localSheetId %q", partURI, label, rawScope))
			} else if localSheetID >= sheetCount {
				diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_SCOPE", "%s %s localSheetId %d is outside available sheet indexes 0..%d", partURI, label, localSheetID, sheetCount-1))
			}
		}

		if name != "" {
			seenKey := strings.ToLower(name) + "\x00" + scopeKey
			if first := seenByScope[seenKey]; first != "" {
				diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_DUPLICATE", "%s %s duplicates %s in the same scope", partURI, label, first))
			} else {
				seenByScope[seenKey] = label
			}
		}

		formula := strings.TrimSpace(elem.Text())
		if formula == "" {
			diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_REQUIRED", "%s %s has empty formula text", partURI, label))
			continue
		}
		if formulaContainsTokenOutsideString(formula, "#REF!") {
			diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_REFERENCE", "%s %s contains stale #REF! reference", partURI, label))
		}
		for _, sheetName := range extractDefinedNameSheetReferences(formula) {
			if !sheetNames[strings.ToLower(sheetName)] {
				diags = append(diags, diag.Errorf("XLSX_DEFINED_NAME_REFERENCE", "%s %s references missing sheet %q", partURI, label, sheetName))
			}
		}
	}
	return diags
}

func workbookSheetNames(root *etree.Element) (map[string]bool, int) {
	names := make(map[string]bool)
	sheets := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "sheets")
	if sheets == nil {
		return names, 0
	}
	sheetElems := xlsxns.FindChildren(sheets, xlsxns.NsSpreadsheetML, "sheet")
	for _, sheet := range sheetElems {
		name := strings.TrimSpace(sheet.SelectAttrValue("name", ""))
		if name != "" {
			names[strings.ToLower(name)] = true
		}
	}
	return names, len(sheetElems)
}

func extractDefinedNameSheetReferences(formula string) []string {
	formula = strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(formula), "="))
	var refs []string
	inString := false
	for i := 0; i < len(formula); i++ {
		switch formula[i] {
		case '"':
			if inString && i+1 < len(formula) && formula[i+1] == '"' {
				i++
				continue
			}
			inString = !inString
		case '!':
			if inString {
				continue
			}
			token, ok := definedNameQualifierBeforeBang(formula[:i])
			if !ok {
				continue
			}
			refs = append(refs, normalizeDefinedNameSheetQualifier(token)...)
		}
	}
	return refs
}

func formulaContainsTokenOutsideString(formula, token string) bool {
	token = strings.ToUpper(token)
	inString := false
	for i := 0; i < len(formula); i++ {
		if formula[i] == '"' {
			if inString && i+1 < len(formula) && formula[i+1] == '"' {
				i++
				continue
			}
			inString = !inString
			continue
		}
		if !inString && strings.HasPrefix(strings.ToUpper(formula[i:]), token) {
			return true
		}
	}
	return false
}

func definedNameQualifierBeforeBang(prefix string) (string, bool) {
	end := len(strings.TrimRight(prefix, " \t\r\n"))
	if end == 0 {
		return "", false
	}
	if prefix[end-1] == '\'' {
		for i := end - 2; i >= 0; i-- {
			if prefix[i] != '\'' {
				continue
			}
			if i > 0 && prefix[i-1] == '\'' {
				i--
				continue
			}
			return prefix[i:end], true
		}
		return "", false
	}
	start := end
	for start > 0 && !definedNameQualifierDelimiter(prefix[start-1]) {
		start--
	}
	token := strings.TrimSpace(prefix[start:end])
	return token, token != ""
}

func definedNameQualifierDelimiter(b byte) bool {
	switch b {
	case ' ', '\t', '\r', '\n', ',', '(', ')', '+', '-', '*', '/', '^', '&', '=', '<', '>':
		return true
	default:
		return false
	}
}

func normalizeDefinedNameSheetQualifier(token string) []string {
	token = strings.TrimSpace(token)
	if token == "" || strings.ContainsAny(token, "[]") {
		return nil
	}
	token = trimDefinedNameSheetQuotes(token)
	var refs []string
	for _, segment := range strings.Split(token, ":") {
		name := trimDefinedNameSheetQuotes(strings.TrimSpace(segment))
		if name != "" && !strings.EqualFold(name, "#REF") {
			refs = append(refs, name)
		}
	}
	return refs
}

func trimDefinedNameSheetQuotes(value string) string {
	value = strings.TrimSpace(value)
	if len(value) >= 2 && strings.HasPrefix(value, "'") && strings.HasSuffix(value, "'") {
		value = value[1 : len(value)-1]
		value = strings.ReplaceAll(value, "''", "'")
	}
	return value
}

func checkPresentationReferences(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	relMap := relationshipsByID(rels)
	var diags []result.Diagnostic
	diags = append(diags, checkPresentationReferenceList(partURI, root, relMap, "sldMasterIdLst", "sldMasterId", relTypePPTXSlideMaster, "slide master")...)
	diags = append(diags, checkPresentationReferenceList(partURI, root, relMap, "sldIdLst", "sldId", relTypePPTXSlide, "slide")...)
	return diags
}

func checkPresentationReferenceList(partURI string, root *etree.Element, relMap map[string]opc.RelationshipInfo, listName, itemName, expectedRelType, targetLabel string) []result.Diagnostic {
	list := pptxns.FindChild(root, pptxns.NsP, listName)
	if list == nil {
		return nil
	}
	var diags []result.Diagnostic
	for idx, elem := range pptxns.FindChildren(list, pptxns.NsP, itemName) {
		label := presentationReferenceLabel(itemName, idx+1, elem)
		rid := relationshipIDAttr(elem)
		if rid == "" {
			diags = append(diags, diag.Errorf("PPTX_PRESENTATION_REFERENCE", "%s %s is missing required r:id for its %s relationship", partURI, label, targetLabel))
			continue
		}
		rel, ok := relMap[rid]
		if !ok {
			diags = append(diags, diag.Errorf("PPTX_PRESENTATION_REFERENCE", "%s %s references missing presentation relationship %s", partURI, label, rid))
			continue
		}
		if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
			diags = append(diags, diag.Errorf("PPTX_PRESENTATION_REFERENCE", "%s %s relationship %s points to an external target; presentation %s entries must resolve to internal parts", partURI, label, rid, targetLabel))
			continue
		}
		if rel.Type != expectedRelType {
			diags = append(diags, diag.Errorf("PPTX_PRESENTATION_REFERENCE", "%s %s relationship %s has type %q, expected %q", partURI, label, rid, rel.Type, expectedRelType))
		}
	}
	return diags
}

func checkSlideLayoutMasterRelationship(partURI string, rels []opc.RelationshipInfo) []result.Diagnostic {
	foundMaster := false
	var diags []result.Diagnostic
	for _, rel := range rels {
		if rel.Type != relTypePPTXSlideMaster {
			continue
		}
		foundMaster = true
		if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
			diags = append(diags, diag.Errorf("PPTX_SLIDE_LAYOUT_MASTER_REFERENCE", "%s slide master relationship %s points to an external target", partURI, relationshipIDOrPlaceholder(rel)))
		}
	}
	if !foundMaster {
		diags = append(diags, diag.Errorf("PPTX_SLIDE_LAYOUT_MASTER_REFERENCE", "%s has no slideMaster relationship; slide layouts must resolve to a slide master", partURI))
	}
	return diags
}

func checkSlideMasterLayoutReferences(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	layoutIDs := pptxns.FindChild(root, pptxns.NsP, "sldLayoutIdLst")
	if layoutIDs == nil {
		return nil
	}
	relMap := relationshipsByID(rels)
	var diags []result.Diagnostic
	for idx, elem := range pptxns.FindChildren(layoutIDs, pptxns.NsP, "sldLayoutId") {
		label := presentationReferenceLabel("sldLayoutId", idx+1, elem)
		diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, relTypePPTXSlideLayout, "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE")...)
	}
	return diags
}

func checkSlideAnimationTargets(partURI string, root *etree.Element) []result.Diagnostic {
	timing := pptxns.FindChild(root, pptxns.NsP, "timing")
	if timing == nil {
		return nil
	}
	shapeIDs := collectSlideShapeIDs(root)
	var diags []result.Diagnostic
	for idx, spTgt := range pptxns.FindDescendants(timing, pptxns.NsP, "spTgt") {
		label := animationTargetLabel(idx+1, spTgt)
		raw := strings.TrimSpace(spTgt.SelectAttrValue("spid", ""))
		if raw == "" {
			diags = append(diags, diag.Errorf("PPTX_ANIMATION_TARGET_REFERENCE", "%s %s is missing required spid", partURI, label))
			continue
		}
		id, err := strconv.Atoi(raw)
		if err != nil || id < 0 {
			diags = append(diags, diag.Errorf("PPTX_ANIMATION_TARGET_REFERENCE", "%s %s has invalid spid %q", partURI, label, raw))
			continue
		}
		if !shapeIDs[id] {
			diags = append(diags, diag.Errorf("PPTX_ANIMATION_TARGET_REFERENCE", "%s %s references missing slide shape id %d", partURI, label, id))
		}
	}
	return diags
}

func collectSlideShapeIDs(root *etree.Element) map[int]bool {
	ids := make(map[int]bool)
	cSld := pptxns.FindChild(root, pptxns.NsP, "cSld")
	if cSld == nil {
		return ids
	}
	spTree := pptxns.FindChild(cSld, pptxns.NsP, "spTree")
	if spTree == nil {
		return ids
	}
	for _, cNvPr := range pptxns.FindDescendants(spTree, pptxns.NsP, "cNvPr") {
		id, err := strconv.Atoi(strings.TrimSpace(cNvPr.SelectAttrValue("id", "")))
		if err == nil && id >= 0 {
			ids[id] = true
		}
	}
	return ids
}

func checkWorksheetRelationshipReferences(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	relMap := relationshipsByID(rels)
	var diags []result.Diagnostic
	for idx, elem := range xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "drawing") {
		label := worksheetReferenceLabel("drawing", idx+1, elem)
		diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, xlsxns.RelDrawing, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE")...)
	}
	for idx, elem := range xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "legacyDrawing") {
		label := worksheetReferenceLabel("legacyDrawing", idx+1, elem)
		diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, xlsxns.RelVmlDrawing, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE")...)
	}
	for idx, elem := range xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "legacyDrawingHF") {
		label := worksheetReferenceLabel("legacyDrawingHF", idx+1, elem)
		diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, xlsxns.RelVmlDrawing, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE")...)
	}
	if tableParts := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "tableParts"); tableParts != nil {
		tablePartElems := xlsxns.FindChildren(tableParts, xlsxns.NsSpreadsheetML, "tablePart")
		diags = append(diags, checkTablePartsCount(partURI, tableParts, len(tablePartElems))...)
		for idx, elem := range tablePartElems {
			label := worksheetReferenceLabel("tablePart", idx+1, elem)
			diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, xlsxns.RelTable, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE")...)
		}
	}
	if hyperlinks := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "hyperlinks"); hyperlinks != nil {
		for idx, elem := range xlsxns.FindChildren(hyperlinks, xlsxns.NsSpreadsheetML, "hyperlink") {
			label := worksheetReferenceLabel("hyperlink", idx+1, elem)
			diags = append(diags, checkHyperlinkRelationshipReference(partURI, label, elem, relMap)...)
		}
	}
	for idx, elem := range xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "pivotTableDefinition") {
		label := worksheetReferenceLabel("pivotTableDefinition", idx+1, elem)
		diags = append(diags, diag.Errorf("XLSX_WORKSHEET_PIVOT_REFERENCE", "%s %s is not a valid worksheet child; pivotTableDefinition must be the root of a pivot table part", partURI, label))
	}
	for _, rel := range rels {
		if rel.Type != xlsxns.RelPivotTable {
			continue
		}
		if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
			diags = append(diags, diag.Errorf("XLSX_WORKSHEET_PIVOT_REFERENCE", "%s pivot table relationship %s points to an external target; worksheet pivot tables must resolve to internal pivot table parts", partURI, relationshipIDOrPlaceholder(rel)))
			continue
		}
	}
	return diags
}

func checkTablePartsCount(partURI string, tableParts *etree.Element, actual int) []result.Diagnostic {
	raw := strings.TrimSpace(tableParts.SelectAttrValue("count", ""))
	if raw == "" {
		return nil
	}
	count, err := strconv.Atoi(raw)
	if err != nil {
		return []result.Diagnostic{
			diag.Errorf("XLSX_WORKSHEET_TABLEPARTS_COUNT", "%s <tableParts> count %q is not a valid integer", partURI, raw),
		}
	}
	if count != actual {
		return []result.Diagnostic{
			diag.Errorf("XLSX_WORKSHEET_TABLEPARTS_COUNT", "%s <tableParts> count is %d but contains %d <tablePart> entries", partURI, count, actual),
		}
	}
	return nil
}

func checkTableDefinition(partURI string, root *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	diags = append(diags, checkOrderedContainer(partURI, root, tableChildOrder, "XLSX_TABLE_CHILD_ORDER")...)

	rawID := strings.TrimSpace(root.SelectAttrValue("id", ""))
	if rawID == "" {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> is missing required id", partURI))
	} else if id, err := strconv.Atoi(rawID); err != nil || id <= 0 {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> has invalid id %q", partURI, rawID))
	}

	if strings.TrimSpace(root.SelectAttrValue("name", "")) == "" {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> is missing required name", partURI))
	}
	if strings.TrimSpace(root.SelectAttrValue("displayName", "")) == "" {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> is missing required displayName", partURI))
	}

	var tableRef address.RangeRef
	refText := strings.TrimSpace(root.SelectAttrValue("ref", ""))
	if refText == "" {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> is missing required ref", partURI))
	} else {
		parsed, err := address.ParseRange(refText)
		if err != nil {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> has invalid ref %q: %v", partURI, refText, err))
		} else {
			tableRef = parsed
			if parsed.Start.Column > parsed.End.Column || parsed.Start.Row > parsed.End.Row {
				diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> ref %q is not top-left to bottom-right", partURI, refText))
			}
		}
	}

	if autoFilter := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "autoFilter"); autoFilter != nil {
		raw := strings.TrimSpace(autoFilter.SelectAttrValue("ref", ""))
		if raw == "" {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <autoFilter> is missing required ref", partURI))
		} else if parsed, err := address.ParseRange(raw); err != nil {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <autoFilter> has invalid ref %q: %v", partURI, raw, err))
		} else if refText != "" && tableRef != (address.RangeRef{}) && parsed.String() != tableRef.String() {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <autoFilter> ref %q does not match table ref %q", partURI, parsed.String(), tableRef.String()))
		}
	}

	tableColumns := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "tableColumns")
	if tableColumns == nil {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <table> is missing required <tableColumns>", partURI))
		return diags
	}
	columns := xlsxns.FindChildren(tableColumns, xlsxns.NsSpreadsheetML, "tableColumn")
	rawCount := strings.TrimSpace(tableColumns.SelectAttrValue("count", ""))
	if rawCount == "" {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <tableColumns> is missing required count", partURI))
	} else if count, err := strconv.Atoi(rawCount); err != nil {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <tableColumns> count %q is not a valid integer", partURI, rawCount))
	} else if count != len(columns) {
		diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s <tableColumns> count is %d but contains %d <tableColumn> entries", partURI, count, len(columns)))
	}
	if tableRef != (address.RangeRef{}) {
		minCol, _, maxCol, _ := tableRef.Bounds()
		width := maxCol - minCol + 1
		if width != len(columns) {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s table ref spans %d columns but <tableColumns> contains %d entries", partURI, width, len(columns)))
		}
	}

	seenIDs := map[int]bool{}
	for idx, column := range columns {
		label := tableColumnLabel(idx+1, column)
		rawColumnID := strings.TrimSpace(column.SelectAttrValue("id", ""))
		if rawColumnID == "" {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s %s is missing required id", partURI, label))
		} else if id, err := strconv.Atoi(rawColumnID); err != nil || id <= 0 {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s %s has invalid id %q", partURI, label, rawColumnID))
		} else if seenIDs[id] {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s %s duplicates tableColumn id %d", partURI, label, id))
		} else {
			seenIDs[id] = true
		}
		if strings.TrimSpace(column.SelectAttrValue("name", "")) == "" {
			diags = append(diags, diag.Errorf("XLSX_TABLE_DEFINITION", "%s %s is missing required name", partURI, label))
		}
	}
	return diags
}

func tableColumnLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<tableColumn #%d>", position)
	}
	var attrs []string
	if id := strings.TrimSpace(elem.SelectAttrValue("id", "")); id != "" {
		attrs = append(attrs, fmt.Sprintf("id=%q", id))
	}
	if name := strings.TrimSpace(elem.SelectAttrValue("name", "")); name != "" {
		attrs = append(attrs, fmt.Sprintf("name=%q", name))
	}
	if len(attrs) == 0 {
		return fmt.Sprintf("<tableColumn #%d>", position)
	}
	return fmt.Sprintf("<tableColumn #%d %s>", position, strings.Join(attrs, " "))
}

func checkPivotTableDefinition(partURI string, root *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	diags = append(diags, checkOrderedContainer(partURI, root, pivotTableChildOrder, "XLSX_PIVOT_TABLE_CHILD_ORDER")...)

	if strings.TrimSpace(root.SelectAttrValue("name", "")) == "" {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <pivotTableDefinition> is missing required name", partURI))
	}
	rawCacheID := strings.TrimSpace(root.SelectAttrValue("cacheId", ""))
	if rawCacheID == "" {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <pivotTableDefinition> is missing required cacheId", partURI))
	} else if cacheID, err := strconv.Atoi(rawCacheID); err != nil || cacheID <= 0 {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <pivotTableDefinition> has invalid cacheId %q", partURI, rawCacheID))
	}

	location := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "location")
	if location == nil {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <pivotTableDefinition> is missing required <location>", partURI))
	} else {
		ref := strings.TrimSpace(location.SelectAttrValue("ref", ""))
		if ref == "" {
			diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <location> is missing required ref", partURI))
		} else if _, err := address.ParseRange(ref); err != nil {
			diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <location> has invalid ref %q: %v", partURI, ref, err))
		}
	}

	pivotFields := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "pivotFields")
	if pivotFields == nil {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s <pivotTableDefinition> is missing required <pivotFields>", partURI))
		return diags
	}
	fields := xlsxns.FindChildren(pivotFields, xlsxns.NsSpreadsheetML, "pivotField")
	diags = append(diags, checkCountedChildren(partURI, pivotFields, "pivotFields", "pivotField", "XLSX_PIVOT_TABLE_DEFINITION")...)
	fieldCount := len(fields)

	diags = append(diags, checkPivotFieldIndexCollection(partURI, root, "rowFields", "field", "x", fieldCount)...)
	diags = append(diags, checkPivotFieldIndexCollection(partURI, root, "colFields", "field", "x", fieldCount)...)
	diags = append(diags, checkPivotFieldIndexCollection(partURI, root, "pageFields", "pageField", "fld", fieldCount)...)
	if dataFields := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "dataFields"); dataFields != nil {
		diags = append(diags, checkCountedChildren(partURI, dataFields, "dataFields", "dataField", "XLSX_PIVOT_TABLE_DEFINITION")...)
		for idx, dataField := range xlsxns.FindChildren(dataFields, xlsxns.NsSpreadsheetML, "dataField") {
			label := pivotChildLabel("dataField", idx+1, dataField, "fld")
			diags = append(diags, checkPivotFieldIndexAttr(partURI, label, dataField, "fld", fieldCount)...)
			if strings.TrimSpace(dataField.SelectAttrValue("name", "")) == "" {
				diags = append(diags, diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s %s is missing required name", partURI, label))
			}
		}
	}
	return diags
}

func checkPivotCacheDefinition(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	var diags []result.Diagnostic
	diags = append(diags, checkOrderedContainer(partURI, root, pivotCacheChildOrder, "XLSX_PIVOT_CACHE_CHILD_ORDER")...)

	if raw := strings.TrimSpace(root.SelectAttrValue("recordCount", "")); raw != "" {
		if count, err := strconv.Atoi(raw); err != nil || count < 0 {
			diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <pivotCacheDefinition> recordCount %q is not a valid non-negative integer", partURI, raw))
		}
	}
	if rid := relationshipIDAttr(root); rid != "" {
		diags = append(diags, checkInternalRelationshipReference(partURI, "<pivotCacheDefinition>", root, relationshipsByID(rels), xlsxns.RelPivotRecords, "XLSX_PIVOT_CACHE_RECORDS_REFERENCE")...)
	}

	cacheSource := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "cacheSource")
	if cacheSource == nil {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <pivotCacheDefinition> is missing required <cacheSource>", partURI))
	} else {
		sourceType := strings.TrimSpace(cacheSource.SelectAttrValue("type", ""))
		if sourceType == "" {
			diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <cacheSource> is missing required type", partURI))
		}
		if sourceType == "worksheet" {
			worksheetSource := xlsxns.FindChild(cacheSource, xlsxns.NsSpreadsheetML, "worksheetSource")
			if worksheetSource == nil {
				diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s worksheet <cacheSource> is missing required <worksheetSource>", partURI))
			} else {
				ref := strings.TrimSpace(worksheetSource.SelectAttrValue("ref", ""))
				name := strings.TrimSpace(worksheetSource.SelectAttrValue("name", ""))
				if ref == "" && name == "" {
					diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <worksheetSource> must define ref or name", partURI))
				}
				if ref != "" {
					if _, err := address.ParseRange(ref); err != nil {
						diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <worksheetSource> has invalid ref %q: %v", partURI, ref, err))
					}
					if strings.TrimSpace(worksheetSource.SelectAttrValue("sheet", "")) == "" {
						diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <worksheetSource> with ref %q is missing required sheet", partURI, ref))
					}
				}
			}
		}
	}

	cacheFields := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "cacheFields")
	if cacheFields == nil {
		diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s <pivotCacheDefinition> is missing required <cacheFields>", partURI))
		return diags
	}
	diags = append(diags, checkCountedChildren(partURI, cacheFields, "cacheFields", "cacheField", "XLSX_PIVOT_CACHE_DEFINITION")...)
	for idx, field := range xlsxns.FindChildren(cacheFields, xlsxns.NsSpreadsheetML, "cacheField") {
		label := pivotChildLabel("cacheField", idx+1, field, "name")
		if strings.TrimSpace(field.SelectAttrValue("name", "")) == "" {
			diags = append(diags, diag.Errorf("XLSX_PIVOT_CACHE_DEFINITION", "%s %s is missing required name", partURI, label))
		}
	}
	return diags
}

func checkPivotRecordsDefinition(partURI string, root *etree.Element) []result.Diagnostic {
	records := xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "r")
	rawCount := strings.TrimSpace(root.SelectAttrValue("count", ""))
	if rawCount == "" {
		return nil
	}
	count, err := strconv.Atoi(rawCount)
	if err != nil || count < 0 {
		return []result.Diagnostic{
			diag.Errorf("XLSX_PIVOT_RECORDS_DEFINITION", "%s <pivotCacheRecords> count %q is not a valid non-negative integer", partURI, rawCount),
		}
	}
	if count != len(records) {
		return []result.Diagnostic{
			diag.Errorf("XLSX_PIVOT_RECORDS_DEFINITION", "%s <pivotCacheRecords> count is %d but contains %d <r> records", partURI, count, len(records)),
		}
	}
	return nil
}

func checkPivotFieldIndexCollection(partURI string, root *etree.Element, parentName, childName, attrName string, fieldCount int) []result.Diagnostic {
	parent := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, parentName)
	if parent == nil {
		return nil
	}
	var diags []result.Diagnostic
	diags = append(diags, checkCountedChildren(partURI, parent, parentName, childName, "XLSX_PIVOT_TABLE_DEFINITION")...)
	for idx, child := range xlsxns.FindChildren(parent, xlsxns.NsSpreadsheetML, childName) {
		label := pivotChildLabel(childName, idx+1, child, attrName)
		diags = append(diags, checkPivotFieldIndexAttr(partURI, label, child, attrName, fieldCount)...)
	}
	return diags
}

func checkPivotFieldIndexAttr(partURI, label string, elem *etree.Element, attrName string, fieldCount int) []result.Diagnostic {
	raw := strings.TrimSpace(elem.SelectAttrValue(attrName, ""))
	if raw == "" {
		return []result.Diagnostic{
			diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s %s is missing required %s", partURI, label, attrName),
		}
	}
	index, err := strconv.Atoi(raw)
	if err != nil || index < 0 {
		return []result.Diagnostic{
			diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s %s has invalid %s %q", partURI, label, attrName, raw),
		}
	}
	if index >= fieldCount {
		return []result.Diagnostic{
			diag.Errorf("XLSX_PIVOT_TABLE_DEFINITION", "%s %s references pivot field index %d outside available fields 0..%d", partURI, label, index, fieldCount-1),
		}
	}
	return nil
}

func checkCountedChildren(partURI string, parent *etree.Element, parentName, childName, code string) []result.Diagnostic {
	children := xlsxns.FindChildren(parent, xlsxns.NsSpreadsheetML, childName)
	rawCount := strings.TrimSpace(parent.SelectAttrValue("count", ""))
	if rawCount == "" {
		return []result.Diagnostic{
			diag.Errorf(code, "%s <%s> is missing required count", partURI, parentName),
		}
	}
	count, err := strconv.Atoi(rawCount)
	if err != nil || count < 0 {
		return []result.Diagnostic{
			diag.Errorf(code, "%s <%s> count %q is not a valid non-negative integer", partURI, parentName, rawCount),
		}
	}
	if count != len(children) {
		return []result.Diagnostic{
			diag.Errorf(code, "%s <%s> count is %d but contains %d <%s> entries", partURI, parentName, count, len(children), childName),
		}
	}
	return nil
}

func pivotChildLabel(name string, position int, elem *etree.Element, attrName string) string {
	if elem == nil {
		return fmt.Sprintf("<%s #%d>", name, position)
	}
	if attrValue := strings.TrimSpace(elem.SelectAttrValue(attrName, "")); attrValue != "" {
		return fmt.Sprintf("<%s #%d %s=%q>", name, position, attrName, attrValue)
	}
	return fmt.Sprintf("<%s #%d>", name, position)
}

func checkChartRelationshipReferences(partURI string, root *etree.Element, rels []opc.RelationshipInfo) []result.Diagnostic {
	relMap := relationshipsByID(rels)
	var diags []result.Diagnostic
	for idx, elem := range xlsxns.FindDescendants(root, xlsxns.NsChart, "chart") {
		label := chartReferenceLabel(idx+1, elem)
		diags = append(diags, checkInternalRelationshipReference(partURI, label, elem, relMap, xlsxns.RelChart, "OOXML_CHART_RELATIONSHIP_REFERENCE")...)
	}
	return diags
}

func checkChartExternalDataRelationshipReferences(session opc.PackageSession, partURI string, root *etree.Element) []result.Diagnostic {
	relMap := relationshipsByID(session.ListRelationships(partURI))
	var diags []result.Diagnostic
	for idx, elem := range xlsxns.FindChildren(root, xlsxns.NsChart, "externalData") {
		rid := relationshipIDAttr(elem)
		label := chartExternalDataLabel(idx+1, elem)
		diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "id", relMap, relTypePackage, "OOXML_CHART_EXTERNAL_DATA_REFERENCE", true, isEmbeddedSpreadsheetPackageContentType, "embedded spreadsheet package")...)
		diags = append(diags, checkChartExternalDataEmbeddedWorkbookOpen(session, partURI, label, rid, relMap)...)
	}
	return diags
}

func checkChartExternalDataEmbeddedWorkbookOpen(session opc.PackageSession, partURI, label, rid string, relMap map[string]opc.RelationshipInfo) []result.Diagnostic {
	if rid == "" {
		return nil
	}
	rel, ok := relMap[rid]
	if !ok || rel.Type != relTypePackage || strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
		return nil
	}
	targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(partURI, rel.Target))
	if !sessionHasPart(session, targetURI) || !isEmbeddedSpreadsheetPackageContentType(session.GetContentType(targetURI)) {
		return nil
	}
	raw, err := session.ReadRawPart(targetURI)
	if err != nil {
		return []result.Diagnostic{
			diag.Errorf("OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN", "%s %s relationship %s points to %s but embedded workbook could not be read: %v", partURI, label, rid, targetURI, err),
		}
	}
	embedded, err := opc.OpenBytes(raw)
	if err != nil {
		return []result.Diagnostic{
			diag.Errorf("OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN", "%s %s relationship %s points to %s but embedded workbook could not be opened as an OOXML package: %v", partURI, label, rid, targetURI, err),
		}
	}
	defer embedded.Close()
	if detected := opc.DetectType(embedded); detected != opc.PackageTypeXLSX {
		return []result.Diagnostic{
			diag.Errorf("OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN", "%s %s relationship %s points to %s but embedded package type is %s, expected xlsx", partURI, label, rid, targetURI, detected),
		}
	}
	return nil
}

func checkDrawingMediaRelationshipReferences(session opc.PackageSession, partURI string, root *etree.Element) []result.Diagnostic {
	relMap := relationshipsByID(session.ListRelationships(partURI))
	var diags []result.Diagnostic
	for idx, elem := range xlsxns.FindDescendants(root, drawingMLNamespace, "blip") {
		if rid := relationshipAttr(elem, "embed"); rid != "" {
			label := drawingRelationshipLabel("a:blip", idx+1, "embed", rid)
			diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "embed", relMap, relTypeImage, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", false, imagex.IsContentType, "image/*")...)
			diags = append(diags, checkImageRelationshipPayload(session, partURI, label, rid, relMap)...)
		}
		if rid := relationshipAttr(elem, "link"); rid != "" {
			label := drawingRelationshipLabel("a:blip", idx+1, "link", rid)
			diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "link", relMap, relTypeImage, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", true, imagex.IsContentType, "image/*")...)
			diags = append(diags, checkImageRelationshipPayload(session, partURI, label, rid, relMap)...)
		}
	}
	for idx, elem := range xlsxns.FindDescendants(root, drawingMLNamespace, "videoFile") {
		rid := relationshipAttr(elem, "link")
		label := drawingRelationshipLabel("a:videoFile", idx+1, "link", rid)
		diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "link", relMap, relTypeVideo, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", true, isVideoContentType, "video/*")...)
	}
	for idx, elem := range xlsxns.FindDescendants(root, drawingMLNamespace, "audioFile") {
		rid := relationshipAttr(elem, "link")
		label := drawingRelationshipLabel("a:audioFile", idx+1, "link", rid)
		diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "link", relMap, relTypeAudio, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", true, isAudioContentType, "audio/*")...)
	}
	for idx, elem := range pptxns.FindDescendants(root, pptxns.Np14, "media") {
		rid := relationshipAttr(elem, "embed")
		label := drawingRelationshipLabel("p14:media", idx+1, "embed", rid)
		diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "embed", relMap, relTypeMedia, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", false, isAudioVideoContentType, "audio/* or video/*")...)
	}
	return diags
}

func checkDOCXDrawingImageRelationshipReferences(session opc.PackageSession, partURI string, root *etree.Element) []result.Diagnostic {
	relMap := relationshipsByID(session.ListRelationships(partURI))
	var diags []result.Diagnostic
	for idx, elem := range docxns.FindDescendants(root, drawingMLNamespace, "blip") {
		if rid := relationshipAttr(elem, "embed"); rid != "" {
			label := drawingRelationshipLabel("a:blip", idx+1, "embed", rid)
			diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "embed", relMap, relTypeImage, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", false, imagex.IsContentType, "image/*")...)
			diags = append(diags, checkImageRelationshipPayload(session, partURI, label, rid, relMap)...)
		}
		if rid := relationshipAttr(elem, "link"); rid != "" {
			label := drawingRelationshipLabel("a:blip", idx+1, "link", rid)
			diags = append(diags, checkRelationshipReferenceTarget(session, partURI, label, rid, "link", relMap, relTypeImage, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", true, imagex.IsContentType, "image/*")...)
			diags = append(diags, checkImageRelationshipPayload(session, partURI, label, rid, relMap)...)
		}
	}
	return diags
}

func checkImageRelationshipPayload(session opc.PackageSession, partURI, label, rid string, relMap map[string]opc.RelationshipInfo) []result.Diagnostic {
	rel, ok := relMap[rid]
	if !ok || rel.Type != relTypeImage || strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
		return nil
	}
	targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(partURI, rel.Target))
	contentType := session.GetContentType(targetURI)
	if !sessionHasPart(session, targetURI) || !imagex.IsContentType(contentType) || !imagex.HasKnownSignature(contentType) {
		return nil
	}
	raw, err := session.ReadRawPart(targetURI)
	if err != nil {
		return []result.Diagnostic{
			diag.Errorf("OOXML_IMAGE_PAYLOAD", "%s %s relationship %s points to %s but image payload could not be read: %v", partURI, label, rid, targetURI, err),
		}
	}
	if !imagex.PayloadMatchesContentType(contentType, raw) {
		return []result.Diagnostic{
			diag.Errorf("OOXML_IMAGE_PAYLOAD", "%s %s relationship %s points to %s with content type %q but payload signature does not match", partURI, label, rid, targetURI, strings.TrimSpace(contentType)),
		}
	}
	return nil
}

func checkRelationshipReferenceTarget(session opc.PackageSession, partURI, label, rid, attrName string, relMap map[string]opc.RelationshipInfo, expectedRelType, code string, allowExternal bool, contentTypeOK func(string) bool, expectedContent string) []result.Diagnostic {
	if rid == "" {
		return []result.Diagnostic{
			diag.Errorf(code, "%s %s is missing required r:%s for its relationship", partURI, label, attrName),
		}
	}
	rel, ok := relMap[rid]
	if !ok {
		return []result.Diagnostic{
			diag.Errorf(code, "%s %s references missing relationship %s", partURI, label, rid),
		}
	}

	var diags []result.Diagnostic
	if rel.Type != expectedRelType {
		diags = append(diags, diag.Errorf(code, "%s %s relationship %s has type %q, expected %q", partURI, label, rid, rel.Type, expectedRelType))
	}
	if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
		if !allowExternal {
			diags = append(diags, diag.Errorf(code, "%s %s relationship %s points to an external target; expected an internal relationship of type %q", partURI, label, rid, expectedRelType))
		}
		return diags
	}

	targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(partURI, rel.Target))
	if !sessionHasPart(session, targetURI) {
		return diags
	}
	contentType := strings.TrimSpace(session.GetContentType(targetURI))
	if contentType == "" {
		return diags
	}
	if contentTypeOK != nil && !contentTypeOK(contentType) {
		diags = append(diags, diag.Errorf(code, "%s %s relationship %s points to %s with content type %q, expected %s", partURI, label, rid, targetURI, contentType, expectedContent))
	}
	return diags
}

func checkInternalRelationshipReference(partURI, label string, elem *etree.Element, relMap map[string]opc.RelationshipInfo, expectedRelType, code string) []result.Diagnostic {
	rid := relationshipIDAttr(elem)
	if rid == "" {
		return []result.Diagnostic{
			diag.Errorf(code, "%s %s is missing required r:id for its relationship", partURI, label),
		}
	}
	rel, ok := relMap[rid]
	if !ok {
		return []result.Diagnostic{
			diag.Errorf(code, "%s %s references missing relationship %s", partURI, label, rid),
		}
	}
	if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
		return []result.Diagnostic{
			diag.Errorf(code, "%s %s relationship %s points to an external target; expected an internal relationship of type %q", partURI, label, rid, expectedRelType),
		}
	}
	if rel.Type != expectedRelType {
		return []result.Diagnostic{
			diag.Errorf(code, "%s %s relationship %s has type %q, expected %q", partURI, label, rid, rel.Type, expectedRelType),
		}
	}
	return nil
}

func checkHyperlinkRelationshipReference(partURI, label string, elem *etree.Element, relMap map[string]opc.RelationshipInfo) []result.Diagnostic {
	rid := relationshipIDAttr(elem)
	if rid == "" {
		return nil
	}
	rel, ok := relMap[rid]
	if !ok {
		return []result.Diagnostic{
			diag.Errorf("XLSX_WORKSHEET_HYPERLINK_REFERENCE", "%s %s references missing hyperlink relationship %s", partURI, label, rid),
		}
	}
	if rel.Type != xlsxns.RelHyperlink {
		return []result.Diagnostic{
			diag.Errorf("XLSX_WORKSHEET_HYPERLINK_REFERENCE", "%s %s relationship %s has type %q, expected %q", partURI, label, rid, rel.Type, xlsxns.RelHyperlink),
		}
	}
	if !strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
		return []result.Diagnostic{
			diag.Errorf("XLSX_WORKSHEET_HYPERLINK_REFERENCE", "%s %s relationship %s has TargetMode %q; worksheet hyperlink r:id relationships must be External", partURI, label, rid, strings.TrimSpace(rel.TargetMode)),
		}
	}
	return nil
}

func checkCalcChainReferences(session opc.PackageSession, partURI string, root *etree.Element) []result.Diagnostic {
	ctx := collectCalcChainContext(session)
	var diags []result.Diagnostic
	currentSheetURI := ""
	for idx, elem := range xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "c") {
		label := calcChainEntryLabel(idx+1, elem)
		refText := strings.TrimSpace(elem.SelectAttrValue("r", ""))
		if refText == "" {
			diags = append(diags, diag.Errorf("XLSX_CALC_CHAIN_REFERENCE", "%s %s is missing required cell reference r", partURI, label))
			continue
		}
		ref, err := address.NormalizeCell(refText)
		if err != nil {
			diags = append(diags, diag.Errorf("XLSX_CALC_CHAIN_REFERENCE", "%s %s has invalid cell reference %q: %v", partURI, label, refText, err))
			continue
		}

		if rawSheetID := strings.TrimSpace(elem.SelectAttrValue("i", "")); rawSheetID != "" {
			sheetURI, ok := ctx.sheetByCalcID[rawSheetID]
			if !ok {
				diags = append(diags, diag.Errorf("XLSX_CALC_CHAIN_REFERENCE", "%s %s references unknown sheet id/index %q", partURI, label, rawSheetID))
				currentSheetURI = ""
				continue
			}
			currentSheetURI = sheetURI
		} else if currentSheetURI == "" {
			currentSheetURI = ctx.firstSheetURI
		}
		if currentSheetURI == "" {
			diags = append(diags, diag.Errorf("XLSX_CALC_CHAIN_REFERENCE", "%s %s cannot be resolved to a worksheet", partURI, label))
			continue
		}
		if !ctx.formulaCells[currentSheetURI][ref] {
			diags = append(diags, diag.Errorf("XLSX_CALC_CHAIN_REFERENCE", "%s %s points to %s!%s, but that cell has no formula", partURI, label, currentSheetURI, ref))
		}
	}
	return diags
}

type calcChainContext struct {
	sheetByCalcID map[string]string
	firstSheetURI string
	formulaCells  map[string]map[string]bool
}

func collectCalcChainContext(session opc.PackageSession) calcChainContext {
	ctx := calcChainContext{
		sheetByCalcID: make(map[string]string),
		formulaCells:  make(map[string]map[string]bool),
	}
	for _, part := range session.ListParts() {
		if !isXLSXWorkbookContentType(part.ContentType) {
			continue
		}
		doc, err := session.ReadXMLPart(part.URI)
		if err != nil || doc.Root() == nil {
			continue
		}
		root := doc.Root()
		if localName(root) != "workbook" || root.NamespaceURI() != xlsxns.NsSpreadsheetML {
			continue
		}
		relMap := relationshipsByID(session.ListRelationships(part.URI))
		sheets := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "sheets")
		if sheets == nil {
			continue
		}
		for idx, sheet := range xlsxns.FindChildren(sheets, xlsxns.NsSpreadsheetML, "sheet") {
			rid := relationshipIDAttr(sheet)
			rel, ok := relMap[rid]
			if rid == "" || !ok || rel.Type != xlsxns.RelWorksheet || strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
				continue
			}
			sheetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(part.URI, rel.Target))
			if ctx.firstSheetURI == "" {
				ctx.firstSheetURI = sheetURI
			}
			if sheetID := strings.TrimSpace(sheet.SelectAttrValue("sheetId", "")); sheetID != "" {
				ctx.sheetByCalcID[sheetID] = sheetURI
			}
			positionID := strconv.Itoa(idx + 1)
			if _, exists := ctx.sheetByCalcID[positionID]; !exists {
				ctx.sheetByCalcID[positionID] = sheetURI
			}
			if _, ok := ctx.formulaCells[sheetURI]; !ok {
				ctx.formulaCells[sheetURI] = collectWorksheetFormulaCells(session, sheetURI)
			}
		}
	}
	return ctx
}

func collectWorksheetFormulaCells(session opc.PackageSession, sheetURI string) map[string]bool {
	formulas := make(map[string]bool)
	doc, err := session.ReadXMLPart(sheetURI)
	if err != nil || doc.Root() == nil {
		return formulas
	}
	root := doc.Root()
	if localName(root) != "worksheet" || root.NamespaceURI() != xlsxns.NsSpreadsheetML {
		return formulas
	}
	for _, cell := range xlsxns.FindDescendants(root, xlsxns.NsSpreadsheetML, "c") {
		if xlsxns.FindChild(cell, xlsxns.NsSpreadsheetML, "f") == nil {
			continue
		}
		ref, err := address.NormalizeCell(cell.SelectAttrValue("r", ""))
		if err != nil {
			continue
		}
		formulas[ref] = true
	}
	return formulas
}

func checkSharedStringCounts(partURI string, root *etree.Element) []result.Diagnostic {
	items := xlsxns.FindChildren(root, xlsxns.NsSpreadsheetML, "si")
	_, countPresent, countOK := optionalUnsignedIntAttr(root, "count")
	uniqueCount, uniqueCountPresent, uniqueCountOK := optionalUnsignedIntAttr(root, "uniqueCount")

	var diags []result.Diagnostic
	if countPresent && !countOK {
		diags = append(diags, diag.Errorf("XLSX_SHARED_STRINGS_COUNTS", "%s <sst> count %q is not a valid unsigned integer", partURI, root.SelectAttrValue("count", "")))
	}
	if uniqueCountPresent && !uniqueCountOK {
		diags = append(diags, diag.Errorf("XLSX_SHARED_STRINGS_COUNTS", "%s <sst> uniqueCount %q is not a valid unsigned integer", partURI, root.SelectAttrValue("uniqueCount", "")))
	}
	if countPresent && !uniqueCountPresent {
		diags = append(diags, diag.Errorf("XLSX_SHARED_STRINGS_COUNTS", "%s <sst> uses count without required uniqueCount", partURI))
	}
	if uniqueCountPresent && !countPresent {
		diags = append(diags, diag.Errorf("XLSX_SHARED_STRINGS_COUNTS", "%s <sst> uses uniqueCount without required count", partURI))
	}
	if uniqueCountPresent && uniqueCountOK && uniqueCount != len(items) {
		diags = append(diags, diag.Errorf("XLSX_SHARED_STRINGS_COUNTS", "%s <sst> uniqueCount is %d but contains %d <si> entries", partURI, uniqueCount, len(items)))
	}
	return diags
}

func checkStylesCounts(partURI string, root *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	diags = append(diags, checkStyleCollectionCount(partURI, root, "numFmts", "numFmt")...)
	diags = append(diags, checkStyleCollectionCount(partURI, root, "cellXfs", "xf")...)
	return diags
}

func checkStyleCollectionCount(partURI string, root *etree.Element, collectionName, childName string) []result.Diagnostic {
	collection := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, collectionName)
	if collection == nil {
		return nil
	}
	actual := len(xlsxns.FindChildren(collection, xlsxns.NsSpreadsheetML, childName))
	declared, present, ok := optionalUnsignedIntAttr(collection, "count")
	if !present {
		return nil
	}
	if !ok {
		return []result.Diagnostic{
			diag.Errorf("XLSX_STYLES_COUNT_MISMATCH", "%s <%s> count %q is not a valid unsigned integer", partURI, collectionName, collection.SelectAttrValue("count", "")),
		}
	}
	if declared != actual {
		return []result.Diagnostic{
			diag.Errorf("XLSX_STYLES_COUNT_MISMATCH", "%s <%s> count is %d but contains %d <%s> entries", partURI, collectionName, declared, actual, childName),
		}
	}
	return nil
}

type stylesReferenceInfo struct {
	hasPart     bool
	usable      bool
	cellXfCount int
}

func collectStylesReferenceInfo(session opc.PackageSession) stylesReferenceInfo {
	for _, part := range session.ListParts() {
		if part.ContentType != xlsxns.ContentTypeStyles {
			continue
		}
		info := stylesReferenceInfo{hasPart: true}
		doc, err := session.ReadXMLPart(part.URI)
		if err != nil {
			return info
		}
		root := doc.Root()
		if root == nil || localName(root) != "styleSheet" || root.NamespaceURI() != xlsxns.NsSpreadsheetML {
			return info
		}
		info.usable = true
		if cellXfs := xlsxns.FindChild(root, xlsxns.NsSpreadsheetML, "cellXfs"); cellXfs != nil {
			info.cellXfCount = len(xlsxns.FindChildren(cellXfs, xlsxns.NsSpreadsheetML, "xf"))
		}
		return info
	}
	return stylesReferenceInfo{}
}

func checkWorksheetStyleReferences(partURI string, root *etree.Element, styles stylesReferenceInfo) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, cell := range xlsxns.FindDescendants(root, xlsxns.NsSpreadsheetML, "c") {
		raw := strings.TrimSpace(cell.SelectAttrValue("s", ""))
		if raw == "" {
			continue
		}
		label := worksheetCellLabel(cell)
		if !styles.hasPart {
			diags = append(diags, diag.Errorf("XLSX_CELL_STYLE_REFERENCE", "%s %s has style index %q but the package has no styles part", partURI, label, raw))
			continue
		}
		if !styles.usable {
			continue
		}
		index, err := strconv.Atoi(raw)
		if err != nil || index < 0 {
			diags = append(diags, diag.Errorf("XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE", "%s %s style index %q is not a valid non-negative integer", partURI, label, raw))
			continue
		}
		if index >= styles.cellXfCount {
			diags = append(diags, diag.Errorf("XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE", "%s %s style index %d is outside available cellXfs 0..%d", partURI, label, index, styles.cellXfCount-1))
		}
	}
	return diags
}

func optionalUnsignedIntAttr(elem *etree.Element, name string) (int, bool, bool) {
	attr := elem.SelectAttr(name)
	if attr == nil {
		return 0, false, false
	}
	raw := strings.TrimSpace(attr.Value)
	if raw == "" {
		return 0, true, false
	}
	value, err := strconv.Atoi(raw)
	if err != nil || value < 0 {
		return 0, true, false
	}
	return value, true, true
}

func relationshipsByID(rels []opc.RelationshipInfo) map[string]opc.RelationshipInfo {
	out := make(map[string]opc.RelationshipInfo, len(rels))
	for _, rel := range rels {
		id := strings.TrimSpace(rel.ID)
		if id == "" {
			continue
		}
		out[id] = rel
	}
	return out
}

func relationshipIDAttr(elem *etree.Element) string {
	return relationshipAttr(elem, "id")
}

func relationshipAttr(elem *etree.Element, localName string) string {
	if value, ok := xlsxns.Attr(elem, xlsxns.NsR, localName); ok {
		return strings.TrimSpace(value)
	}
	if attr := elem.SelectAttr("r:" + localName); attr != nil {
		return strings.TrimSpace(attr.Value)
	}
	for _, attr := range elem.Attr {
		if attr.Key == localName && (attr.Space == "r" || attr.Space == xlsxns.NsR || attr.Space == pptxns.NsR) {
			return strings.TrimSpace(attr.Value)
		}
	}
	return ""
}

func worksheetReferenceLabel(itemName string, position int, elem *etree.Element) string {
	rid := relationshipIDAttr(elem)
	if rid != "" {
		return fmt.Sprintf("<%s r:id=%q> at position %d", itemName, rid, position)
	}
	return fmt.Sprintf("<%s> at position %d", itemName, position)
}

func chartReferenceLabel(position int, elem *etree.Element) string {
	rid := relationshipIDAttr(elem)
	if rid != "" {
		return fmt.Sprintf("<c:chart r:id=%q> at position %d", rid, position)
	}
	return fmt.Sprintf("<c:chart> at position %d", position)
}

func chartExternalDataLabel(position int, elem *etree.Element) string {
	rid := relationshipIDAttr(elem)
	if rid != "" {
		return fmt.Sprintf("<c:externalData r:id=%q> at position %d", rid, position)
	}
	return fmt.Sprintf("<c:externalData> at position %d", position)
}

func drawingRelationshipLabel(elementName string, position int, attrName, rid string) string {
	if rid != "" {
		return fmt.Sprintf("<%s r:%s=%q> at position %d", elementName, attrName, rid, position)
	}
	return fmt.Sprintf("<%s> at position %d", elementName, position)
}

func isVideoContentType(contentType string) bool {
	return strings.HasPrefix(strings.ToLower(strings.TrimSpace(contentType)), "video/")
}

func isAudioContentType(contentType string) bool {
	return strings.HasPrefix(strings.ToLower(strings.TrimSpace(contentType)), "audio/")
}

func isAudioVideoContentType(contentType string) bool {
	return isAudioContentType(contentType) || isVideoContentType(contentType)
}

func isEmbeddedSpreadsheetPackageContentType(contentType string) bool {
	switch strings.ToLower(strings.TrimSpace(contentType)) {
	case "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
		"application/vnd.openxmlformats-officedocument.spreadsheetml.template",
		"application/vnd.ms-excel.sheet.macroenabled.12",
		"application/vnd.ms-excel.template.macroenabled.12",
		"application/vnd.ms-excel.sheet.binary.macroenabled.12":
		return true
	default:
		return false
	}
}

func worksheetCellLabel(cell *etree.Element) string {
	ref := ""
	if cell != nil {
		ref = strings.TrimSpace(cell.SelectAttrValue("r", ""))
	}
	if ref != "" {
		return fmt.Sprintf("cell %s", ref)
	}
	return "cell"
}

func calcChainEntryLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<c> at position %d", position)
	}
	ref := strings.TrimSpace(elem.SelectAttrValue("r", ""))
	sheetID := strings.TrimSpace(elem.SelectAttrValue("i", ""))
	switch {
	case ref != "" && sheetID != "":
		return fmt.Sprintf("<c r=%q i=%q> at position %d", ref, sheetID, position)
	case ref != "":
		return fmt.Sprintf("<c r=%q> at position %d", ref, position)
	case sheetID != "":
		return fmt.Sprintf("<c i=%q> at position %d", sheetID, position)
	default:
		return fmt.Sprintf("<c> at position %d", position)
	}
}

func definedNameLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<definedName> at position %d", position)
	}
	name := strings.TrimSpace(elem.SelectAttrValue("name", ""))
	scope := strings.TrimSpace(elem.SelectAttrValue("localSheetId", ""))
	switch {
	case name != "" && scope != "":
		return fmt.Sprintf("<definedName name=%q localSheetId=%q> at position %d", name, scope, position)
	case name != "":
		return fmt.Sprintf("<definedName name=%q> at position %d", name, position)
	case scope != "":
		return fmt.Sprintf("<definedName localSheetId=%q> at position %d", scope, position)
	default:
		return fmt.Sprintf("<definedName> at position %d", position)
	}
}

func animationTargetLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<p:spTgt> at position %d", position)
	}
	spid := strings.TrimSpace(elem.SelectAttrValue("spid", ""))
	if spid != "" {
		return fmt.Sprintf("<p:spTgt spid=%q> at position %d", spid, position)
	}
	return fmt.Sprintf("<p:spTgt> at position %d", position)
}

func relationshipIDOrPlaceholder(rel opc.RelationshipInfo) string {
	id := strings.TrimSpace(rel.ID)
	if id != "" {
		return id
	}
	return "<missing-id>"
}

func workbookSheetLabel(position int, sheet *etree.Element) string {
	if sheet == nil {
		return fmt.Sprintf("sheet #%d", position)
	}
	name := strings.TrimSpace(sheet.SelectAttrValue("name", ""))
	sheetID := strings.TrimSpace(sheet.SelectAttrValue("sheetId", ""))
	switch {
	case name != "" && sheetID != "":
		return fmt.Sprintf("sheet #%d name %q sheetId %s", position, name, sheetID)
	case name != "":
		return fmt.Sprintf("sheet #%d name %q", position, name)
	case sheetID != "":
		return fmt.Sprintf("sheet #%d sheetId %s", position, sheetID)
	default:
		return fmt.Sprintf("sheet #%d", position)
	}
}

func workbookPivotCacheLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<pivotCache> at position %d", position)
	}
	cacheID := strings.TrimSpace(elem.SelectAttrValue("cacheId", ""))
	rid := relationshipIDAttr(elem)
	var attrs []string
	if cacheID != "" {
		attrs = append(attrs, fmt.Sprintf("cacheId=%q", cacheID))
	}
	if rid != "" {
		attrs = append(attrs, fmt.Sprintf("r:id=%q", rid))
	}
	if len(attrs) == 0 {
		return fmt.Sprintf("<pivotCache> at position %d", position)
	}
	return fmt.Sprintf("<pivotCache %s> at position %d", strings.Join(attrs, " "), position)
}

func presentationReferenceLabel(itemName string, position int, elem *etree.Element) string {
	id := ""
	if elem != nil {
		id = strings.TrimSpace(elem.SelectAttrValue("id", ""))
	}
	if id != "" {
		return fmt.Sprintf("<p:%s id=%q> at position %d", itemName, id, position)
	}
	return fmt.Sprintf("<p:%s> at position %d", itemName, position)
}

func relationshipLabel(sourceURI string, rel opc.RelationshipInfo) string {
	if rel.ID != "" {
		return fmt.Sprintf("%s relationship %s", sourceURI, rel.ID)
	}
	return fmt.Sprintf("%s relationship", sourceURI)
}

func sourceURIFromRelsPath(relsPath string) string {
	relsPath = opc.NormalizeURI(relsPath)
	if relsPath == "/_rels/.rels" {
		return "/"
	}
	dir := opc.GetDirectory(relsPath)
	if strings.HasSuffix(dir, "/_rels") {
		dir = strings.TrimSuffix(dir, "/_rels")
		if dir == "" {
			dir = "/"
		}
	}
	fileName := opc.GetFileName(relsPath)
	if strings.HasSuffix(fileName, ".rels") {
		fileName = strings.TrimSuffix(fileName, ".rels")
	}
	return opc.JoinPaths(dir, fileName)
}

func looksExternalRelationshipTarget(target string) bool {
	lowered := strings.ToLower(strings.TrimSpace(target))
	return strings.Contains(lowered, "://") ||
		strings.HasPrefix(lowered, "mailto:") ||
		strings.HasPrefix(lowered, "file:") ||
		strings.HasPrefix(lowered, "urn:")
}

func checkZipEntryMetadata(partURI string, meta *opc.ZipEntryMeta) []result.Diagnostic {
	if meta == nil {
		return nil
	}
	if meta.ModifiedTime.IsZero() || meta.ModifiedTime.Before(minZipModifiedTime) {
		return []result.Diagnostic{
			diag.Errorf("OOXML_ZIP_TIMESTAMP_INVALID", "%s has invalid ZIP modified time %s; Office may repair packages with zero or pre-1980 ZIP dates", partURI, meta.ModifiedTime.Format(time.RFC3339)),
		}
	}
	return nil
}

func readXMLPartAndCheckRoot(session opc.PackageSession, partURI, label, expectedLocalName, expectedNamespace, code string) (*etree.Document, []result.Diagnostic) {
	doc, err := session.ReadXMLPart(partURI)
	if err != nil {
		return nil, []result.Diagnostic{
			diag.Errorf("OOXML_XML_PARSE_ERROR", "failed to read %s %s: %v", label, partURI, err),
		}
	}
	if rootDiags := checkRootName(partURI, doc.Root(), expectedLocalName, expectedNamespace, code); len(rootDiags) > 0 {
		return doc, rootDiags
	}
	return doc, nil
}

func checkRootName(partURI string, root *etree.Element, expectedLocalName, expectedNamespace, code string) []result.Diagnostic {
	if root == nil {
		return []result.Diagnostic{diag.Errorf(code, "%s has no XML root", partURI)}
	}
	actualLocalName := localName(root)
	actualNamespace := root.NamespaceURI()
	if actualLocalName != expectedLocalName || actualNamespace != expectedNamespace {
		return []result.Diagnostic{
			diag.Errorf(code, "%s root is {%s}%s, expected {%s}%s", partURI, actualNamespace, actualLocalName, expectedNamespace, expectedLocalName),
		}
	}
	return nil
}

func isXLSXWorkbookContentType(contentType string) bool {
	switch contentType {
	case xlsxns.ContentTypeWorkbook,
		xlsxns.ContentTypeWorkbookMacro,
		xlsxns.ContentTypeWorkbookTemplate,
		xlsxns.ContentTypeWorkbookAddin:
		return true
	default:
		return false
	}
}

func isPPTXPresentationContentType(contentType string) bool {
	switch contentType {
	case contentTypePPTXPresentation,
		contentTypePPTXPresentationMacro,
		contentTypePPTXTemplate,
		contentTypePPTXTemplateMacro,
		contentTypePPTXSlideshow,
		contentTypePPTXSlideshowMacro:
		return true
	default:
		return false
	}
}

func checkElementOrder(partURI string, root *etree.Element, order func(string) int, code string) []result.Diagnostic {
	if root == nil {
		return []result.Diagnostic{diag.Errorf(code, "%s has no XML root", partURI)}
	}
	var diags []result.Diagnostic
	lastOrder := -1
	lastName := ""
	for _, child := range root.ChildElements() {
		name := localName(child)
		current := order(name)
		if current == 0 {
			continue
		}
		if lastOrder > current {
			diags = append(diags, diag.Errorf(code, "%s has <%s> after <%s>; expected schema child order", partURI, name, lastName))
			continue
		}
		lastOrder = current
		lastName = name
	}
	return diags
}

func checkWorksheetDrawing(partURI string, root *etree.Element) []result.Diagnostic {
	if rootDiags := checkRootName(partURI, root, "wsDr", xlsxns.NsSpreadsheetDrawing, "XLSX_DRAWING_ROOT"); len(rootDiags) > 0 {
		return rootDiags
	}
	var diags []result.Diagnostic
	for _, anchor := range root.ChildElements() {
		switch localName(anchor) {
		case "twoCellAnchor":
			diags = append(diags, checkTwoCellAnchor(partURI, anchor)...)
		case "oneCellAnchor":
			diags = append(diags, checkOrderedContainer(partURI, anchor, oneCellAnchorOrder, "XLSX_DRAWING_ANCHOR_ORDER")...)
			diags = append(diags, requireChildren(partURI, anchor, "XLSX_DRAWING_ANCHOR_REQUIRED", "from", "ext", "clientData")...)
		case "absoluteAnchor":
			diags = append(diags, checkOrderedContainer(partURI, anchor, absoluteAnchorOrder, "XLSX_DRAWING_ANCHOR_ORDER")...)
			diags = append(diags, requireChildren(partURI, anchor, "XLSX_DRAWING_ANCHOR_REQUIRED", "pos", "ext", "clientData")...)
		}
	}
	return diags
}

func checkTwoCellAnchor(partURI string, anchor *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	diags = append(diags, checkOrderedContainer(partURI, anchor, twoCellAnchorOrder, "XLSX_DRAWING_ANCHOR_ORDER")...)
	diags = append(diags, requireChildren(partURI, anchor, "XLSX_DRAWING_ANCHOR_REQUIRED", "from", "to", "clientData")...)
	objectCount := 0
	for _, child := range anchor.ChildElements() {
		switch localName(child) {
		case "sp", "grpSp", "graphicFrame", "cxnSp", "pic", "contentPart":
			objectCount++
		}
	}
	if objectCount != 1 {
		diags = append(diags, diag.Errorf("XLSX_DRAWING_ANCHOR_REQUIRED", "%s twoCellAnchor must contain exactly one drawing object before clientData, found %d", partURI, objectCount))
	}
	return diags
}

func checkChartPart(partURI string, root *etree.Element) []result.Diagnostic {
	if rootDiags := checkRootName(partURI, root, "chartSpace", xlsxns.NsChart, "OOXML_CHART_ROOT"); len(rootDiags) > 0 {
		return rootDiags
	}
	var diags []result.Diagnostic
	diags = append(diags, checkOrderedContainer(partURI, root, chartSpaceChildOrder, "OOXML_CHARTSPACE_CHILD_ORDER")...)
	for _, chart := range childrenByLocal(root, "chart") {
		diags = append(diags, checkOrderedContainer(partURI, chart, chartChildOrder, "OOXML_CHART_CHILD_ORDER")...)
		for _, plotArea := range childrenByLocal(chart, "plotArea") {
			diags = append(diags, checkOrderedContainer(partURI, plotArea, plotAreaChildOrder, "OOXML_PLOTAREA_CHILD_ORDER")...)
			diags = append(diags, checkChartTypeOrder(partURI, plotArea)...)
			diags = append(diags, checkChartAxisReferences(partURI, plotArea)...)
			diags = append(diags, checkChartSeriesCaches(partURI, plotArea)...)
		}
	}
	return diags
}

func checkChartTypeOrder(partURI string, plotArea *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, child := range plotArea.ChildElements() {
		switch localName(child) {
		case "barChart":
			diags = append(diags, checkOrderedContainer(partURI, child, barChartChildOrder, "OOXML_BARCHART_CHILD_ORDER")...)
		case "lineChart":
			diags = append(diags, checkOrderedContainer(partURI, child, lineChartChildOrder, "OOXML_LINECHART_CHILD_ORDER")...)
		case "areaChart":
			diags = append(diags, checkOrderedContainer(partURI, child, areaChartChildOrder, "OOXML_AREACHART_CHILD_ORDER")...)
		case "pieChart":
			diags = append(diags, checkOrderedContainer(partURI, child, pieChartChildOrder, "OOXML_PIECHART_CHILD_ORDER")...)
		case "scatterChart":
			diags = append(diags, checkOrderedContainer(partURI, child, scatterChartChildOrder, "OOXML_SCATTERCHART_CHILD_ORDER")...)
		}
	}
	return diags
}

func checkChartAxisReferences(partURI string, plotArea *etree.Element) []result.Diagnostic {
	axisLabels := map[string]string{}
	var diags []result.Diagnostic
	for _, axis := range plotArea.ChildElements() {
		if !isChartAxisElement(localName(axis)) {
			continue
		}
		label := chartAxisLabel(axis)
		axisID := chartChildVal(axis, "axId")
		if axisID == "" {
			diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s is missing required <c:axId val>", partURI, label))
		} else if first := axisLabels[axisID]; first != "" {
			diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s duplicates axis id %s from %s", partURI, label, axisID, first))
		} else {
			axisLabels[axisID] = label
		}
		crossID := chartChildVal(axis, "crossAx")
		if crossID == "" {
			diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s is missing required <c:crossAx val>", partURI, label))
			continue
		}
		if axisID != "" && crossID == axisID {
			diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s crossAx references its own axis id %s", partURI, label, axisID))
			continue
		}
		// The target axis might appear later; validate after all axis IDs are collected.
	}
	for _, axis := range plotArea.ChildElements() {
		if !isChartAxisElement(localName(axis)) {
			continue
		}
		crossID := chartChildVal(axis, "crossAx")
		if crossID == "" {
			continue
		}
		if axisLabels[crossID] == "" {
			diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s crossAx references missing axis id %s", partURI, chartAxisLabel(axis), crossID))
		}
	}
	for _, plot := range plotArea.ChildElements() {
		plotName := localName(plot)
		if !chartTypeRequiresAxes(plotName) {
			continue
		}
		refs := chartAxisRefIDs(plot)
		if len(refs) < 2 {
			diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s <c:%s> has %d <c:axId> references; expected at least 2 axis references", partURI, plotName, len(refs)))
		}
		seen := map[string]bool{}
		for idx, axisID := range refs {
			label := fmt.Sprintf("<c:%s>/<c:axId #%d>", plotName, idx+1)
			if axisID == "" {
				diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s is missing required val", partURI, label))
				continue
			}
			if seen[axisID] {
				diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s duplicates axis id %s inside <c:%s>", partURI, label, axisID, plotName))
			}
			seen[axisID] = true
			if axisLabels[axisID] == "" {
				diags = append(diags, diag.Errorf("OOXML_CHART_AXIS_REFERENCE", "%s %s references missing axis id %s", partURI, label, axisID))
			}
		}
	}
	return diags
}

func isChartAxisElement(name string) bool {
	switch name {
	case "catAx", "dateAx", "valAx", "serAx":
		return true
	default:
		return false
	}
}

func chartTypeRequiresAxes(name string) bool {
	switch name {
	case "areaChart", "area3DChart", "barChart", "bar3DChart", "bubbleChart",
		"lineChart", "line3DChart", "radarChart", "scatterChart", "stockChart",
		"surfaceChart", "surface3DChart":
		return true
	default:
		return false
	}
}

func chartAxisRefIDs(plot *etree.Element) []string {
	var ids []string
	for _, child := range childrenByLocal(plot, "axId") {
		ids = append(ids, strings.TrimSpace(child.SelectAttrValue("val", "")))
	}
	return ids
}

func chartAxisLabel(axis *etree.Element) string {
	name := localName(axis)
	axisID := chartChildVal(axis, "axId")
	if axisID != "" {
		return fmt.Sprintf("<c:%s axId=%q>", name, axisID)
	}
	return fmt.Sprintf("<c:%s>", name)
}

func chartChildVal(parent *etree.Element, childName string) string {
	child := firstChildByLocal(parent, childName)
	if child == nil {
		return ""
	}
	return strings.TrimSpace(child.SelectAttrValue("val", ""))
}

func checkChartSeriesCaches(partURI string, plotArea *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, plot := range plotArea.ChildElements() {
		plotName := localName(plot)
		series := childrenByLocal(plot, "ser")
		if len(series) == 0 {
			continue
		}
		seenIdx := map[int]string{}
		seenOrder := map[int]string{}
		for idx, ser := range series {
			label := fmt.Sprintf("<c:%s>/<c:ser #%d>", plotName, idx+1)
			diags = append(diags, checkChartSeriesOrdinal(partURI, label, ser, "idx", seenIdx)...)
			diags = append(diags, checkChartSeriesOrdinal(partURI, label, ser, "order", seenOrder)...)
			diags = append(diags, checkChartSeriesSourceCaches(partURI, label, ser)...)
		}
	}
	return diags
}

func checkChartSeriesOrdinal(partURI, label string, ser *etree.Element, childName string, seen map[int]string) []result.Diagnostic {
	var diags []result.Diagnostic
	value, raw, ok := chartRequiredIntChildVal(ser, childName)
	if !ok {
		return []result.Diagnostic{diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s is missing required <c:%s val>", partURI, label, childName)}
	}
	if value < 0 {
		return []result.Diagnostic{diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s has invalid <c:%s val=%q>; expected a non-negative integer", partURI, label, childName, raw)}
	}
	if first := seen[value]; first != "" {
		diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s duplicates series %s %d from %s", partURI, label, childName, value, first))
	} else {
		seen[value] = label
	}
	return diags
}

func checkChartSeriesSourceCaches(partURI, seriesLabel string, ser *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, roleName := range []string{"tx", "cat", "val", "xVal", "yVal", "bubbleSize"} {
		role := firstChildByLocal(ser, roleName)
		if role == nil {
			continue
		}
		for _, refName := range []string{"strRef", "numRef"} {
			ref := firstChildByLocal(role, refName)
			if ref == nil {
				continue
			}
			refLabel := fmt.Sprintf("%s/<c:%s>/<c:%s>", seriesLabel, roleName, refName)
			if firstChildByLocal(ref, "f") == nil {
				diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s is missing required <c:f>", partURI, refLabel))
			}
			expectedCache := chartExpectedCacheForRef(refName)
			for _, cache := range chartDirectCaches(ref) {
				cacheName := localName(cache)
				if expectedCache != "" && cacheName != expectedCache {
					diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s has <c:%s>; expected <c:%s>", partURI, refLabel, cacheName, expectedCache))
				}
				diags = append(diags, checkChartSeriesCache(partURI, refLabel, cache)...)
			}
		}
	}
	return diags
}

func checkChartSeriesCache(partURI, refLabel string, cache *etree.Element) []result.Diagnostic {
	cacheName := localName(cache)
	points := childrenByLocal(cache, "pt")
	var diags []result.Diagnostic
	count, rawCount, ok := chartRequiredIntChildVal(cache, "ptCount")
	if !ok {
		diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s/<c:%s> is missing required <c:ptCount val>", partURI, refLabel, cacheName))
	} else if count < 0 {
		diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s/<c:%s> has invalid ptCount %q; expected a non-negative integer", partURI, refLabel, cacheName, rawCount))
	} else if count != len(points) {
		diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s/<c:%s> ptCount=%d but contains %d <c:pt> elements", partURI, refLabel, cacheName, count, len(points)))
	}
	seenPointIdx := map[int]bool{}
	for pointNumber, point := range points {
		pointIdx, rawIdx, ok := chartRequiredIntAttr(point, "idx")
		pointLabel := fmt.Sprintf("%s/<c:%s>/<c:pt #%d>", refLabel, cacheName, pointNumber+1)
		if !ok {
			diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s is missing required idx", partURI, pointLabel))
			continue
		}
		if pointIdx < 0 {
			diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s has invalid idx %q; expected a non-negative integer", partURI, pointLabel, rawIdx))
			continue
		}
		if seenPointIdx[pointIdx] {
			diags = append(diags, diag.Errorf("OOXML_CHART_SERIES_CACHE", "%s %s duplicates point idx %d", partURI, pointLabel, pointIdx))
		}
		seenPointIdx[pointIdx] = true
	}
	return diags
}

func chartExpectedCacheForRef(refName string) string {
	switch refName {
	case "numRef":
		return "numCache"
	case "strRef":
		return "strCache"
	default:
		return ""
	}
}

func chartDirectCaches(parent *etree.Element) []*etree.Element {
	var out []*etree.Element
	for _, child := range parent.ChildElements() {
		switch localName(child) {
		case "numCache", "strCache":
			out = append(out, child)
		}
	}
	return out
}

func chartRequiredIntChildVal(parent *etree.Element, childName string) (int, string, bool) {
	child := firstChildByLocal(parent, childName)
	if child == nil {
		return 0, "", false
	}
	return chartRequiredIntAttr(child, "val")
}

func chartRequiredIntAttr(elem *etree.Element, attrName string) (int, string, bool) {
	raw := strings.TrimSpace(elem.SelectAttrValue(attrName, ""))
	if raw == "" {
		return 0, "", false
	}
	value, err := strconv.Atoi(raw)
	if err != nil {
		return -1, raw, true
	}
	return value, raw, true
}

func checkOrderedContainer(partURI string, parent *etree.Element, order func(string) int, code string) []result.Diagnostic {
	parentName := localName(parent)
	var diags []result.Diagnostic
	lastOrder := -1
	lastName := ""
	for _, child := range parent.ChildElements() {
		name := localName(child)
		current := order(name)
		if current == 0 {
			continue
		}
		if lastOrder > current {
			diags = append(diags, diag.Errorf(code, "%s <%s> has <%s> after <%s>; expected schema child order", partURI, parentName, name, lastName))
			continue
		}
		lastOrder = current
		lastName = name
	}
	return diags
}

func requireChildren(partURI string, parent *etree.Element, code string, names ...string) []result.Diagnostic {
	var diags []result.Diagnostic
	for _, name := range names {
		if firstChildByLocal(parent, name) == nil {
			diags = append(diags, diag.Errorf(code, "%s <%s> is missing required <%s>", partURI, localName(parent), name))
		}
	}
	return diags
}

func firstChildByLocal(parent *etree.Element, name string) *etree.Element {
	for _, child := range parent.ChildElements() {
		if localName(child) == name {
			return child
		}
	}
	return nil
}

func childrenByLocal(parent *etree.Element, name string) []*etree.Element {
	var out []*etree.Element
	for _, child := range parent.ChildElements() {
		if localName(child) == name {
			out = append(out, child)
		}
	}
	return out
}

func localName(elem *etree.Element) string {
	if elem == nil {
		return ""
	}
	tag := elem.Tag
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.LastIndex(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}

func orderMap(names ...string) func(string) int {
	orders := make(map[string]int, len(names))
	for idx, name := range names {
		orders[name] = idx + 1
	}
	return func(name string) int {
		return orders[name]
	}
}

var worksheetChildOrder = orderMap(
	"sheetPr", "dimension", "sheetViews", "sheetFormatPr", "cols", "sheetData",
	"sheetCalcPr", "sheetProtection", "protectedRanges", "scenarios", "autoFilter",
	"sortState", "dataConsolidate", "customSheetViews", "mergeCells", "phoneticPr",
	"conditionalFormatting", "dataValidations", "hyperlinks", "printOptions",
	"pageMargins", "pageSetup", "headerFooter", "rowBreaks", "colBreaks",
	"customProperties", "cellWatches", "ignoredErrors", "smartTags", "drawing",
	"legacyDrawing", "legacyDrawingHF", "drawingHF", "picture", "oleObjects",
	"controls", "webPublishItems", "tableParts", "extLst",
)

var tableChildOrder = orderMap("autoFilter", "sortState", "tableColumns", "tableStyleInfo", "extLst")
var pivotTableChildOrder = orderMap("location", "pivotFields", "rowFields", "rowItems", "colFields", "colItems", "pageFields", "dataFields", "formats", "conditionalFormats", "chartFormats", "pivotHierarchies", "pivotTableStyleInfo", "filters", "rowHierarchiesUsage", "colHierarchiesUsage", "extLst")
var pivotCacheChildOrder = orderMap("cacheSource", "cacheFields", "cacheHierarchies", "kpis", "tupleCache", "calculatedItems", "calculatedMembers", "dimensions", "measureGroups", "maps", "extLst")
var slideChildOrder = orderMap("cSld", "clrMapOvr", "transition", "timing", "extLst")
var slideLayoutChildOrder = orderMap("cSld", "clrMapOvr", "transition", "timing", "hf", "extLst")
var slideMasterChildOrder = orderMap("cSld", "clrMap", "sldLayoutIdLst", "transition", "timing", "hf", "txStyles", "extLst")

var twoCellAnchorOrder = orderMap("from", "to", "sp", "grpSp", "graphicFrame", "cxnSp", "pic", "contentPart", "clientData")
var oneCellAnchorOrder = orderMap("from", "ext", "sp", "grpSp", "graphicFrame", "cxnSp", "pic", "contentPart", "clientData")
var absoluteAnchorOrder = orderMap("pos", "ext", "sp", "grpSp", "graphicFrame", "cxnSp", "pic", "contentPart", "clientData")

var chartSpaceChildOrder = orderMap(
	"date1904", "lang", "roundedCorners", "style", "clrMapOvr", "pivotSource",
	"protection", "chart", "spPr", "txPr", "externalData", "printSettings",
	"userShapes", "extLst",
)

var chartChildOrder = orderMap(
	"title", "autoTitleDeleted", "pivotFmts", "view3D", "floor", "sideWall",
	"backWall", "plotArea", "legend", "plotVisOnly", "dispBlanksAs",
	"showDLblsOverMax", "extLst",
)

func plotAreaChildOrder(name string) int {
	switch name {
	case "layout":
		return 1
	case "areaChart", "area3DChart", "lineChart", "line3DChart", "stockChart",
		"radarChart", "scatterChart", "pieChart", "pie3DChart", "doughnutChart",
		"barChart", "bar3DChart", "ofPieChart", "surfaceChart", "surface3DChart",
		"bubbleChart":
		return 2
	case "valAx", "catAx", "dateAx", "serAx":
		// Excel and PowerPoint commonly serialize catAx before valAx. The
		// repair-sensitive invariant is that axes stay after plot nodes and
		// before post-plot decoration, not the order among axis siblings.
		return 3
	case "dTable":
		return 4
	case "spPr":
		return 5
	case "extLst":
		return 6
	default:
		return 0
	}
}

var barChartChildOrder = orderMap("barDir", "grouping", "varyColors", "ser", "dLbls", "gapWidth", "overlap", "serLines", "axId", "extLst")
var lineChartChildOrder = orderMap("grouping", "varyColors", "ser", "dLbls", "dropLines", "hiLowLines", "upDownBars", "marker", "smooth", "axId", "extLst")
var areaChartChildOrder = orderMap("grouping", "varyColors", "ser", "dLbls", "dropLines", "axId", "extLst")
var pieChartChildOrder = orderMap("varyColors", "ser", "dLbls", "firstSliceAng", "extLst")
var scatterChartChildOrder = orderMap("scatterStyle", "varyColors", "ser", "dLbls", "axId", "extLst")
