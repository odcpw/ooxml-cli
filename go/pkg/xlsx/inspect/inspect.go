package inspect

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

const relTypeTheme = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme"

// FindWorkbookPart resolves the workbook part from package root relationships.
func FindWorkbookPart(session opc.PackageSession) (string, error) {
	if session == nil {
		return "", fmt.Errorf("package session is nil")
	}

	for _, rel := range session.ListRelationships("/") {
		if rel.TargetMode == "External" {
			continue
		}
		targetURI := resolveTargetURI("/", rel.Target)
		if isWorkbookCandidate(session, targetURI) {
			return targetURI, nil
		}
	}

	for _, part := range session.ListParts() {
		if isWorkbookContentType(part.ContentType) {
			return opc.NormalizeURI(part.URI), nil
		}
	}

	return "", fmt.Errorf("xlsx workbook part not found")
}

// ParseWorkbook parses workbook.xml and resolves sheet relationship targets.
func ParseWorkbook(session opc.PackageSession) (*model.Workbook, error) {
	workbookURI, err := FindWorkbookPart(session)
	if err != nil {
		return nil, err
	}

	doc, err := session.ReadXMLPart(workbookURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook part %s: %w", workbookURI, err)
	}

	root := doc.Root()
	if root == nil {
		return nil, fmt.Errorf("workbook part %s has no root element", workbookURI)
	}
	if !isWorkbookRoot(root) {
		return nil, fmt.Errorf("workbook part %s root is %q, expected workbook", workbookURI, root.Tag)
	}

	workbook := &model.Workbook{
		PartURI: workbookURI,
		Sheets:  make([]model.SheetRef, 0),
	}

	relMap := workbookRelationshipMap(session, workbookURI, workbook)
	sheets := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "sheets")
	if sheets == nil {
		return workbook, nil
	}

	for position, sheetElem := range namespaces.FindChildren(sheets, namespaces.NsSpreadsheetML, "sheet") {
		sheet, err := parseSheetRef(workbookURI, relMap, position+1, sheetElem)
		if err != nil {
			return nil, err
		}
		sheet = model.WithSheetSelectors(sheet)
		workbook.Sheets = append(workbook.Sheets, sheet)
	}

	return workbook, nil
}

// ListSheets returns workbook sheets in workbook order.
func ListSheets(session opc.PackageSession) ([]model.SheetRef, error) {
	workbook, err := ParseWorkbook(session)
	if err != nil {
		return nil, err
	}

	sheets := make([]model.SheetRef, len(workbook.Sheets))
	copy(sheets, workbook.Sheets)
	return sheets, nil
}

// SummarizeWorkbook reads workbook structure and counts common XLSX parts.
func SummarizeWorkbook(session opc.PackageSession) (*model.WorkbookSummary, error) {
	workbook, err := ParseWorkbook(session)
	if err != nil {
		return nil, err
	}

	summary := &model.WorkbookSummary{
		Type:            string(opc.PackageTypeXLSX),
		WorkbookPartURI: workbook.PartURI,
		SheetCount:      len(workbook.Sheets),
	}
	if workbook.SharedStringsURI != "" {
		if count, err := CountSharedStrings(session, workbook.SharedStringsURI); err == nil {
			summary.SharedStringCount = count
		}
	}

	for _, part := range session.ListParts() {
		uri := opc.NormalizeURI(part.URI)
		contentType := part.ContentType

		switch {
		case isWorksheetPart(uri, contentType):
			summary.WorksheetCount++
		case isSharedStringsPart(uri, contentType):
			summary.SharedStrings = true
		case isStylesPart(uri, contentType):
			summary.Styles = true
		case isThemePart(uri, contentType):
			summary.Themes++
		case isTablePart(uri, contentType):
			summary.Tables++
		case isPivotTablePart(uri, contentType):
			summary.Pivots++
		case isPivotCachePart(uri, contentType):
			summary.PivotCaches++
		case isChartPart(uri, contentType):
			summary.Charts++
		case isMediaPart(uri):
			summary.MediaAssets++
		case isCustomXMLPart(uri):
			summary.CustomXMLParts++
		}
	}

	return summary, nil
}

// CountSharedStrings counts si entries in a shared string table.
func CountSharedStrings(session opc.PackageSession, sharedStringsURI string) (int, error) {
	if sharedStringsURI == "" {
		return 0, nil
	}
	doc, err := session.ReadXMLPart(sharedStringsURI)
	if err != nil {
		return 0, err
	}
	root := doc.Root()
	if !isSpreadsheetRoot(root, "sst") {
		return 0, fmt.Errorf("shared string table root element not found")
	}
	return len(namespaces.FindChildren(root, namespaces.NsSpreadsheetML, "si")), nil
}

// CountCellFormats counts cellXfs entries in styles.xml.
func CountCellFormats(session opc.PackageSession, stylesURI string) (int, error) {
	if stylesURI == "" {
		return 0, nil
	}
	doc, err := session.ReadXMLPart(stylesURI)
	if err != nil {
		return 0, err
	}
	root := doc.Root()
	if !isSpreadsheetRoot(root, "styleSheet") {
		return 0, fmt.Errorf("styles root element not found")
	}
	cellXfs := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "cellXfs")
	if cellXfs == nil {
		return 0, nil
	}
	if countText := cellXfs.SelectAttrValue("count", ""); countText != "" {
		if count, err := strconv.Atoi(countText); err == nil {
			return count, nil
		}
	}
	return len(namespaces.FindChildren(cellXfs, namespaces.NsSpreadsheetML, "xf")), nil
}

type workbookRel struct {
	targetURI string
	relType   string
}

func workbookRelationshipMap(session opc.PackageSession, workbookURI string, workbook *model.Workbook) map[string]workbookRel {
	rels := session.ListRelationships(workbookURI)
	relMap := make(map[string]workbookRel, len(rels))
	for _, rel := range rels {
		if rel.ID == "" || rel.TargetMode == "External" {
			continue
		}
		targetURI := resolveTargetURI(workbookURI, rel.Target)
		relMap[rel.ID] = workbookRel{
			targetURI: targetURI,
			relType:   rel.Type,
		}

		switch rel.Type {
		case namespaces.RelSharedStrings:
			workbook.SharedStringsURI = targetURI
		case namespaces.RelStyles:
			workbook.StylesURI = targetURI
		case relTypeTheme:
			workbook.ThemeURI = targetURI
		}
	}
	return relMap
}

func resolveTargetURI(sourceURI, target string) string {
	if strings.HasPrefix(target, "/") {
		return opc.NormalizeURI(target)
	}
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, target))
}

func parseSheetRef(workbookURI string, relMap map[string]workbookRel, position int, sheetElem *etree.Element) (model.SheetRef, error) {
	rid, ok := namespaces.Attr(sheetElem, namespaces.NsR, "id")
	if !ok || rid == "" {
		return model.SheetRef{}, fmt.Errorf("sheet at position %d in %s is missing r:id", position, workbookURI)
	}

	rel, ok := relMap[rid]
	if !ok {
		return model.SheetRef{}, fmt.Errorf("sheet relationship %s not found in %s relationships", rid, workbookURI)
	}

	state := sheetElem.SelectAttrValue("state", model.SheetStateVisible)
	if state == "" {
		state = model.SheetStateVisible
	}

	return model.SheetRef{
		Position:         position,
		Number:           position,
		Name:             sheetElem.SelectAttrValue("name", ""),
		SheetID:          sheetElem.SelectAttrValue("sheetId", ""),
		State:            state,
		RelationshipID:   rid,
		PartURI:          rel.targetURI,
		RelationshipType: rel.relType,
	}, nil
}

func isWorkbookCandidate(session opc.PackageSession, uri string) bool {
	if uri == "" || uri == "/" {
		return false
	}
	if isWorkbookContentType(session.GetContentType(uri)) {
		return true
	}
	return uri == "/xl/workbook.xml"
}

func isWorkbookRoot(root *etree.Element) bool {
	return isSpreadsheetRoot(root, "workbook")
}

func isSpreadsheetRoot(root *etree.Element, localName string) bool {
	if root == nil || root.Tag != localName {
		return false
	}
	ns := root.NamespaceURI()
	return ns == "" || ns == namespaces.NsSpreadsheetML
}

func isWorkbookContentType(contentType string) bool {
	switch contentType {
	case namespaces.ContentTypeWorkbook,
		namespaces.ContentTypeWorkbookMacro,
		namespaces.ContentTypeWorkbookAddin,
		namespaces.ContentTypeWorkbookTemplate:
		return true
	default:
		return strings.Contains(contentType, "spreadsheetml.sheet.main+xml") ||
			strings.Contains(contentType, "spreadsheetml.template.main+xml") ||
			strings.Contains(contentType, "ms-excel.sheet.macroEnabled.main+xml") ||
			strings.Contains(contentType, "ms-excel.addin.macroEnabled.main+xml")
	}
}

func isWorksheetPart(uri, contentType string) bool {
	if contentType == namespaces.ContentTypeWorksheet {
		return true
	}
	return isXMLDataPart(uri) && strings.HasPrefix(uri, "/xl/worksheets/")
}

func isSharedStringsPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeSharedStrings || uri == "/xl/sharedStrings.xml"
}

func isStylesPart(uri, contentType string) bool {
	return contentType == namespaces.ContentTypeStyles || uri == "/xl/styles.xml"
}

func isThemePart(uri, contentType string) bool {
	return contentType == "application/vnd.openxmlformats-officedocument.theme+xml" ||
		isXMLDataPart(uri) && strings.HasPrefix(uri, "/xl/theme/")
}

func isTablePart(uri, contentType string) bool {
	if contentType == namespaces.ContentTypeTable {
		return true
	}
	return isXMLDataPart(uri) && strings.HasPrefix(uri, "/xl/tables/")
}

func isPivotTablePart(uri, contentType string) bool {
	if contentType == namespaces.ContentTypePivotTable {
		return true
	}
	return isXMLDataPart(uri) && strings.HasPrefix(uri, "/xl/pivotTables/")
}

func isPivotCachePart(uri, contentType string) bool {
	if contentType == namespaces.ContentTypePivotCache {
		return true
	}
	return isXMLDataPart(uri) &&
		strings.HasPrefix(uri, "/xl/pivotCache/") &&
		strings.HasPrefix(opc.GetFileName(uri), "pivotCacheDefinition")
}

func isChartPart(uri, contentType string) bool {
	if contentType == namespaces.ContentTypeChart {
		return true
	}
	return isXMLDataPart(uri) &&
		strings.HasPrefix(uri, "/xl/charts/") &&
		strings.HasPrefix(opc.GetFileName(uri), "chart")
}

func isMediaPart(uri string) bool {
	return strings.HasPrefix(uri, "/xl/media/") && !strings.Contains(uri, "/_rels/")
}

func isCustomXMLPart(uri string) bool {
	return isXMLDataPart(uri) && strings.HasPrefix(uri, "/customXml/")
}

func isXMLDataPart(uri string) bool {
	return strings.HasSuffix(uri, ".xml") && !strings.Contains(uri, "/_rels/") && !strings.HasSuffix(uri, ".rels")
}
