package validate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func validateXLSXSemantics(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	workbook, err := xlsxinspect.ParseWorkbook(session)
	if err != nil {
		diags = append(diags, diag.Error(
			"XLSX_PARSE_ERROR",
			"failed to parse workbook structure: "+err.Error(),
		))
		return diags, nil
	}

	partMap := make(map[string]bool)
	for _, part := range session.ListParts() {
		partMap[part.URI] = true
	}

	if !partMap[workbook.PartURI] {
		diags = append(diags, diag.Error(
			"XLSX_MISSING_WORKBOOK",
			"workbook part not found: "+workbook.PartURI,
		))
		return diags, nil
	}
	if len(workbook.Sheets) == 0 {
		diags = append(diags, diag.Error(
			"XLSX_NO_SHEETS",
			"workbook contains no sheets",
		))
	}

	pivotCacheIDs := map[int]bool{}
	workbookDoc, err := session.ReadXMLPart(workbook.PartURI)
	if err != nil || workbookDoc == nil || workbookDoc.Root() == nil {
		diags = append(diags, diag.Error(
			"XLSX_WORKBOOK_PARSE_ERROR",
			fmt.Sprintf("failed to parse workbook part %s: %v", workbook.PartURI, err),
		))
	} else {
		root := workbookDoc.Root()
		diags = append(diags, validateWorkbookDefinedNames(root, workbook.PartURI, len(workbook.Sheets))...)
		cacheIDs, cacheDiags := validateWorkbookPivotCaches(session, workbook.PartURI, root, partMap)
		pivotCacheIDs = cacheIDs
		diags = append(diags, cacheDiags...)
	}

	sharedStringCount := 0
	if workbook.SharedStringsURI != "" {
		count, err := xlsxinspect.CountSharedStrings(session, workbook.SharedStringsURI)
		if err != nil {
			diags = append(diags, diag.Error(
				"XLSX_SHARED_STRINGS_PARSE_ERROR",
				"failed to parse shared string table: "+err.Error(),
			))
		} else {
			sharedStringCount = count
		}
	}

	styleCount := 0
	if workbook.StylesURI != "" {
		count, err := xlsxinspect.CountCellFormats(session, workbook.StylesURI)
		if err != nil {
			diags = append(diags, diag.Error(
				"XLSX_STYLES_PARSE_ERROR",
				"failed to parse styles: "+err.Error(),
			))
		} else {
			styleCount = count
		}
	}

	seenSheetIDs := map[string]bool{}
	reportedMissingSharedStrings := false
	for _, sheet := range workbook.Sheets {
		if sheet.Name == "" {
			diags = append(diags, diag.Error(
				"XLSX_SHEET_MISSING_NAME",
				fmt.Sprintf("sheet %d has no name", sheet.Number),
			))
		}
		if sheet.SheetID == "" {
			diags = append(diags, diag.Error(
				"XLSX_SHEET_MISSING_ID",
				fmt.Sprintf("sheet %d (%q) has no sheetId", sheet.Number, sheet.Name),
			))
		} else {
			sheetID, err := strconv.ParseUint(sheet.SheetID, 10, 32)
			if err != nil {
				diags = append(diags, diag.Error(
					"XLSX_SHEET_ID_INVALID",
					fmt.Sprintf("sheet %d (%q) has non-integer sheetId %q", sheet.Number, sheet.Name, sheet.SheetID),
				))
			} else if sheetID < 1 || sheetID > 65534 {
				diags = append(diags, diag.Error(
					"XLSX_SHEET_ID_OUT_OF_RANGE",
					fmt.Sprintf("sheet %d (%q) has sheetId %d outside 1..65534", sheet.Number, sheet.Name, sheetID),
				))
			}
			if seenSheetIDs[sheet.SheetID] {
				diags = append(diags, diag.Error(
					"XLSX_DUPLICATE_SHEET_ID",
					fmt.Sprintf("duplicate sheetId %s on sheet %d (%q)", sheet.SheetID, sheet.Number, sheet.Name),
				))
			}
		}
		seenSheetIDs[sheet.SheetID] = true

		if sheet.RelationshipID == "" {
			diags = append(diags, diag.Error(
				"XLSX_SHEET_MISSING_RELATIONSHIP",
				fmt.Sprintf("sheet %d (%q) has no relationship id", sheet.Number, sheet.Name),
			))
			continue
		}
		if sheet.PartURI == "" {
			diags = append(diags, diag.Error(
				"XLSX_SHEET_RELATIONSHIP_NOT_FOUND",
				fmt.Sprintf("sheet %d (%q) relationship %s not found in workbook rels", sheet.Number, sheet.Name, sheet.RelationshipID),
			))
			continue
		}
		if !partMap[sheet.PartURI] {
			diags = append(diags, diag.Error(
				"XLSX_MISSING_WORKSHEET",
				fmt.Sprintf("sheet %d (%q) points to missing worksheet part: %s", sheet.Number, sheet.Name, sheet.PartURI),
			))
			continue
		}

		worksheetDoc, err := session.ReadXMLPart(sheet.PartURI)
		if err != nil {
			diags = append(diags, diag.Error(
				"XLSX_WORKSHEET_PARSE_ERROR",
				fmt.Sprintf("failed to parse worksheet for sheet %d (%q): %v", sheet.Number, sheet.Name, err),
			))
			continue
		}
		root := worksheetDoc.Root()
		if root == nil || !xlsxElementMatches(root, "worksheet") {
			diags = append(diags, diag.Error(
				"XLSX_WORKSHEET_ROOT_ERROR",
				fmt.Sprintf("worksheet root element not found for sheet %d (%q)", sheet.Number, sheet.Name),
			))
			continue
		}

		cellDiags := validateWorksheetCells(root, sheet.Number, sheet.Name, workbook.SharedStringsURI, sharedStringCount, workbook.StylesURI, styleCount, &reportedMissingSharedStrings)
		diags = append(diags, cellDiags...)
		diags = append(diags, validateWorksheetDrawingFilterSortAndPivot(session, sheet.PartURI, root, sheet.Number, sheet.Name, partMap, pivotCacheIDs)...)
	}

	diags = append(diags, validateChartParts(session)...)
	return diags, nil
}

func validateWorkbookDefinedNames(root *etree.Element, workbookURI string, sheetCount int) []result.Diagnostic {
	definedNames := xmlx.FindChild(root, namespaces.NsSpreadsheet, "definedNames")
	if definedNames == nil {
		return nil
	}
	sheetNames := workbookSheetNames(root)
	seenByScope := map[string]string{}
	var diags []result.Diagnostic

	for idx, elem := range xmlx.FindChildren(definedNames, namespaces.NsSpreadsheet, "definedName") {
		label := xlsxDefinedNameLabel(idx+1, elem)
		name := strings.TrimSpace(elem.SelectAttrValue("name", ""))
		if name == "" {
			diags = append(diags, diag.Error(
				"XLSX_DEFINED_NAME_REQUIRED",
				fmt.Sprintf("%s %s is missing required name", workbookURI, label),
			))
		}

		scopeKey := "workbook"
		if rawScope := strings.TrimSpace(elem.SelectAttrValue("localSheetId", "")); rawScope != "" {
			scopeKey = "sheet:" + rawScope
			localSheetID, err := strconv.Atoi(rawScope)
			if err != nil || localSheetID < 0 {
				diags = append(diags, diag.Error(
					"XLSX_DEFINED_NAME_SCOPE",
					fmt.Sprintf("%s %s has invalid localSheetId %q", workbookURI, label, rawScope),
				))
			} else if localSheetID >= sheetCount {
				diags = append(diags, diag.Error(
					"XLSX_DEFINED_NAME_SCOPE",
					fmt.Sprintf("%s %s localSheetId %d is outside available sheet indexes 0..%d", workbookURI, label, localSheetID, sheetCount-1),
				))
			} else {
				scopeKey = fmt.Sprintf("sheet:%d", localSheetID)
			}
		}

		if name != "" {
			seenKey := strings.ToLower(name) + "\x00" + scopeKey
			if first := seenByScope[seenKey]; first != "" {
				diags = append(diags, diag.Error(
					"XLSX_DEFINED_NAME_DUPLICATE",
					fmt.Sprintf("%s %s duplicates %s in the same scope", workbookURI, label, first),
				))
			} else {
				seenByScope[seenKey] = label
			}
		}

		formula := strings.TrimSpace(elem.Text())
		if formula == "" {
			diags = append(diags, diag.Error(
				"XLSX_DEFINED_NAME_REQUIRED",
				fmt.Sprintf("%s %s has empty formula text", workbookURI, label),
			))
			continue
		}
		sheetName, refText, ok := parseSimpleDefinedNameSheetReference(formula)
		if !ok {
			continue
		}
		if !sheetNames[strings.ToLower(sheetName)] {
			diags = append(diags, diag.Error(
				"XLSX_DEFINED_NAME_REFERENCE",
				fmt.Sprintf("%s %s references missing sheet %q", workbookURI, label, sheetName),
			))
		}
		if err := validateDefinedNameReferenceText(refText); err != nil {
			diags = append(diags, diag.Error(
				"XLSX_DEFINED_NAME_REFERENCE",
				fmt.Sprintf("%s %s has invalid sheet reference %q: %v", workbookURI, label, refText, err),
			))
		}
	}
	return diags
}

func validateWorkbookPivotCaches(session opc.PackageSession, workbookURI string, root *etree.Element, partMap map[string]bool) (map[int]bool, []result.Diagnostic) {
	cacheIDs := map[int]bool{}
	pivotCaches := xmlx.FindChild(root, namespaces.NsSpreadsheet, "pivotCaches")
	if pivotCaches == nil {
		return cacheIDs, nil
	}

	relMap := xlsxRelationshipsByID(session.ListRelationships(workbookURI))
	seenCacheIDs := map[int]string{}
	var diags []result.Diagnostic
	for idx, elem := range xmlx.FindChildren(pivotCaches, namespaces.NsSpreadsheet, "pivotCache") {
		label := xlsxWorkbookPivotCacheLabel(idx+1, elem)
		rawCacheID := strings.TrimSpace(elem.SelectAttrValue("cacheId", ""))
		if rawCacheID == "" {
			diags = append(diags, diag.Error(
				"XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
				fmt.Sprintf("%s %s is missing required cacheId", workbookURI, label),
			))
		} else if cacheID, err := strconv.Atoi(rawCacheID); err != nil || cacheID <= 0 {
			diags = append(diags, diag.Error(
				"XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
				fmt.Sprintf("%s %s has invalid cacheId %q", workbookURI, label, rawCacheID),
			))
		} else {
			cacheIDs[cacheID] = true
			if first := seenCacheIDs[cacheID]; first != "" {
				diags = append(diags, diag.Error(
					"XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
					fmt.Sprintf("%s %s duplicates %s cacheId %d", workbookURI, label, first, cacheID),
				))
			} else {
				seenCacheIDs[cacheID] = label
			}
		}

		targetURI, relDiags := validateXLSXInternalRelationshipReference(session, workbookURI, label, elem, relMap, namespaces.RelPivotCache, namespaces.ContentTypePivotCache, "pivot cache definition part", "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", partMap)
		diags = append(diags, relDiags...)
		if len(relDiags) > 0 || targetURI == "" {
			continue
		}
		diags = append(diags, validatePivotCacheDefinitionRecordsRelationship(session, targetURI, partMap)...)
	}

	return cacheIDs, diags
}

func validateWorksheetCells(root *etree.Element, sheetNumber int, sheetName string, sharedStringsURI string, sharedStringCount int, stylesURI string, styleCount int, reportedMissingSharedStrings *bool) []result.Diagnostic {
	var diags []result.Diagnostic

	for _, cell := range xmlx.FindDescendants(root, namespaces.NsSpreadsheet, "c") {
		cellRef := cell.SelectAttrValue("r", "")
		if cellRef == "" {
			cellRef = "(unknown cell)"
		}

		cellType := cell.SelectAttrValue("t", "")
		if cellType == "s" {
			if sharedStringsURI == "" {
				if !*reportedMissingSharedStrings {
					diags = append(diags, diag.Error(
						"XLSX_MISSING_SHARED_STRINGS",
						"worksheet uses shared string cells but workbook has no shared string table relationship",
					))
					*reportedMissingSharedStrings = true
				}
			} else {
				valueElem := xmlx.FindChild(cell, namespaces.NsSpreadsheet, "v")
				if valueElem == nil {
					diags = append(diags, diag.Error(
						"XLSX_SHARED_STRING_MISSING_VALUE",
						fmt.Sprintf("sheet %d (%q) cell %s has shared string type but no value", sheetNumber, sheetName, cellRef),
					))
				} else if idx, err := strconv.Atoi(valueElem.Text()); err != nil || idx < 0 || idx >= sharedStringCount {
					diags = append(diags, diag.Error(
						"XLSX_SHARED_STRING_INDEX_OUT_OF_RANGE",
						fmt.Sprintf("sheet %d (%q) cell %s references shared string index %q outside 0..%d", sheetNumber, sheetName, cellRef, valueElem.Text(), sharedStringCount-1),
					))
				}
			}
		}

		styleText := cell.SelectAttrValue("s", "")
		if styleText == "" {
			continue
		}
		if stylesURI == "" {
			diags = append(diags, diag.Error(
				"XLSX_MISSING_STYLES",
				fmt.Sprintf("sheet %d (%q) cell %s has style index %s but workbook has no styles relationship", sheetNumber, sheetName, cellRef, styleText),
			))
			continue
		}
		styleIdx, err := strconv.Atoi(styleText)
		if err != nil || styleIdx < 0 || styleIdx >= styleCount {
			diags = append(diags, diag.Error(
				"XLSX_STYLE_INDEX_OUT_OF_RANGE",
				fmt.Sprintf("sheet %d (%q) cell %s references style index %q outside 0..%d", sheetNumber, sheetName, cellRef, styleText, styleCount-1),
			))
		}
	}

	return diags
}

func validateWorksheetDrawingFilterSortAndPivot(session opc.PackageSession, sheetURI string, root *etree.Element, sheetNumber int, sheetName string, partMap map[string]bool, pivotCacheIDs map[int]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	rels := session.ListRelationships(sheetURI)
	relMap := xlsxRelationshipsByID(rels)

	for idx, drawing := range xmlx.FindChildren(root, namespaces.NsSpreadsheet, "drawing") {
		label := fmt.Sprintf("sheet %d (%q) drawing #%d", sheetNumber, sheetName, idx+1)
		targetURI, relDiags := validateXLSXInternalRelationshipReference(session, sheetURI, label, drawing, relMap, namespaces.RelDrawing, namespaces.ContentTypeDrawing, "drawing part", "XLSX_WORKSHEET_DRAWING_REFERENCE", partMap)
		diags = append(diags, relDiags...)
		if len(relDiags) > 0 || targetURI == "" {
			continue
		}
		diags = append(diags, validateWorksheetDrawingPart(session, targetURI, partMap)...)
	}

	diags = append(diags, validateWorksheetAutoFiltersAndSorts(sheetURI, root)...)

	for _, rel := range rels {
		if rel.Type != namespaces.RelPivotTable {
			continue
		}
		label := fmt.Sprintf("sheet %d (%q) pivot table relationship %s", sheetNumber, sheetName, xlsxRelationshipIDOrPlaceholder(rel))
		relDiags := validateXLSXRelationshipTarget(session, sheetURI, label, rel, namespaces.RelPivotTable, namespaces.ContentTypePivotTable, "pivot table part", "XLSX_WORKSHEET_PIVOT_REFERENCE", partMap)
		diags = append(diags, relDiags...)
		if len(relDiags) > 0 {
			continue
		}
		targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(sheetURI, rel.Target))
		diags = append(diags, validatePivotTableCacheID(session, targetURI, label, pivotCacheIDs)...)
	}

	return diags
}

func validatePivotCacheDefinitionRecordsRelationship(session opc.PackageSession, cacheURI string, partMap map[string]bool) []result.Diagnostic {
	doc, err := session.ReadXMLPart(cacheURI)
	if err != nil || doc == nil || doc.Root() == nil {
		return []result.Diagnostic{
			diag.Error("XLSX_PIVOT_CACHE_PARSE_ERROR", fmt.Sprintf("failed to parse pivot cache definition part %s: %v", cacheURI, err)),
		}
	}
	root := doc.Root()
	if !xlsxElementMatches(root, "pivotCacheDefinition") {
		return []result.Diagnostic{
			diag.Error("XLSX_PIVOT_CACHE_ROOT_ERROR", fmt.Sprintf("pivot cache definition part %s root element is <%s>, expected <pivotCacheDefinition>", cacheURI, xlsxLocalName(root))),
		}
	}
	rels := session.ListRelationships(cacheURI)
	relMap := xlsxRelationshipsByID(rels)
	var diags []result.Diagnostic
	rootRID := xlsxRelationshipIDAttr(root)
	if rootRID != "" {
		_, relDiags := validateXLSXInternalRelationshipReference(session, cacheURI, fmt.Sprintf("%s <pivotCacheDefinition>", cacheURI), root, relMap, namespaces.RelPivotRecords, namespaces.ContentTypePivotRecords, "pivot cache records part", "XLSX_PIVOT_CACHE_RECORDS_REFERENCE", partMap)
		diags = append(diags, relDiags...)
	}
	for _, rel := range rels {
		if rel.Type != namespaces.RelPivotRecords || rel.ID == rootRID {
			continue
		}
		label := fmt.Sprintf("%s pivot cache records relationship %s", cacheURI, xlsxRelationshipIDOrPlaceholder(rel))
		diags = append(diags, validateXLSXRelationshipTarget(session, cacheURI, label, rel, namespaces.RelPivotRecords, namespaces.ContentTypePivotRecords, "pivot cache records part", "XLSX_PIVOT_CACHE_RECORDS_REFERENCE", partMap)...)
	}
	return diags
}

func validatePivotTableCacheID(session opc.PackageSession, pivotURI, label string, pivotCacheIDs map[int]bool) []result.Diagnostic {
	doc, err := session.ReadXMLPart(pivotURI)
	if err != nil || doc == nil || doc.Root() == nil {
		return []result.Diagnostic{
			diag.Error("XLSX_PIVOT_TABLE_PARSE_ERROR", fmt.Sprintf("failed to parse pivot table part %s: %v", pivotURI, err)),
		}
	}
	root := doc.Root()
	if !xlsxElementMatches(root, "pivotTableDefinition") {
		return []result.Diagnostic{
			diag.Error("XLSX_PIVOT_TABLE_ROOT_ERROR", fmt.Sprintf("pivot table part %s root element is <%s>, expected <pivotTableDefinition>", pivotURI, xlsxLocalName(root))),
		}
	}
	var diags []result.Diagnostic
	if strings.TrimSpace(root.SelectAttrValue("name", "")) == "" {
		diags = append(diags, diag.Error(
			"XLSX_PIVOT_TABLE_DEFINITION",
			fmt.Sprintf("%s <pivotTableDefinition> is missing required name", pivotURI),
		))
	}
	rawCacheID := strings.TrimSpace(root.SelectAttrValue("cacheId", ""))
	if rawCacheID == "" {
		diags = append(diags, diag.Error("XLSX_WORKSHEET_PIVOT_CACHE_REFERENCE", fmt.Sprintf("%s points to %s with missing cacheId", label, pivotURI)))
	} else if cacheID, err := strconv.Atoi(rawCacheID); err != nil || cacheID <= 0 {
		diags = append(diags, diag.Error("XLSX_WORKSHEET_PIVOT_CACHE_REFERENCE", fmt.Sprintf("%s points to %s with invalid cacheId %q", label, pivotURI, rawCacheID)))
	} else if len(pivotCacheIDs) > 0 && !pivotCacheIDs[cacheID] {
		diags = append(diags, diag.Error("XLSX_WORKSHEET_PIVOT_CACHE_REFERENCE", fmt.Sprintf("%s points to %s with cacheId %d not declared in workbook pivotCaches", label, pivotURI, cacheID)))
	}

	location := xmlx.FindChild(root, namespaces.NsSpreadsheet, "location")
	if location == nil {
		diags = append(diags, diag.Error(
			"XLSX_PIVOT_TABLE_DEFINITION",
			fmt.Sprintf("%s <pivotTableDefinition> is missing required <location>", pivotURI),
		))
	} else {
		ref := strings.TrimSpace(location.SelectAttrValue("ref", ""))
		if ref == "" {
			diags = append(diags, diag.Error(
				"XLSX_PIVOT_TABLE_DEFINITION",
				fmt.Sprintf("%s <location> is missing required ref", pivotURI),
			))
		} else if _, err := address.ParseRange(ref); err != nil {
			diags = append(diags, diag.Error(
				"XLSX_PIVOT_TABLE_DEFINITION",
				fmt.Sprintf("%s <location> has invalid ref %q: %v", pivotURI, ref, err),
			))
		}
	}

	pivotFields := xmlx.FindChild(root, namespaces.NsSpreadsheet, "pivotFields")
	if pivotFields == nil {
		diags = append(diags, diag.Error(
			"XLSX_PIVOT_TABLE_DEFINITION",
			fmt.Sprintf("%s <pivotTableDefinition> is missing required <pivotFields>", pivotURI),
		))
		return diags
	}
	fields := xmlx.FindChildren(pivotFields, namespaces.NsSpreadsheet, "pivotField")
	if rawCount := strings.TrimSpace(pivotFields.SelectAttrValue("count", "")); rawCount != "" {
		count, ok := nonNegativeIntString(rawCount)
		if !ok {
			diags = append(diags, diag.Error(
				"XLSX_PIVOT_TABLE_DEFINITION",
				fmt.Sprintf("%s <pivotFields> count %q is not a valid non-negative integer", pivotURI, rawCount),
			))
		} else if count != len(fields) {
			diags = append(diags, diag.Error(
				"XLSX_PIVOT_TABLE_DEFINITION",
				fmt.Sprintf("%s <pivotFields> count is %d but contains %d <pivotField> children", pivotURI, count, len(fields)),
			))
		}
	}
	return diags
}

func validateWorksheetDrawingPart(session opc.PackageSession, drawingURI string, partMap map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	doc, err := session.ReadXMLPart(drawingURI)
	if err != nil || doc == nil || doc.Root() == nil {
		return []result.Diagnostic{
			diag.Error("XLSX_DRAWING_PARSE_ERROR", fmt.Sprintf("failed to parse drawing part %s: %v", drawingURI, err)),
		}
	}
	root := doc.Root()
	if !xmlx.ElementMatches(root, namespaces.NsSpreadsheetDrawing, "wsDr") && xlsxLocalName(root) != "wsDr" {
		diags = append(diags, diag.Error(
			"XLSX_DRAWING_ROOT_ERROR",
			fmt.Sprintf("drawing part %s root element is <%s>, expected <xdr:wsDr>", drawingURI, xlsxLocalName(root)),
		))
	}

	for idx, anchor := range root.ChildElements() {
		anchorName := xlsxLocalName(anchor)
		if anchorName != "twoCellAnchor" && anchorName != "oneCellAnchor" && anchorName != "absoluteAnchor" {
			continue
		}
		label := fmt.Sprintf("%s <xdr:%s #%d>", drawingURI, anchorName, idx+1)
		diags = append(diags, validateDrawingAnchor(label, anchor, anchorName)...)
	}

	relMap := xlsxRelationshipsByID(session.ListRelationships(drawingURI))
	for idx, chart := range xmlx.FindDescendants(root, namespaces.NsChart, "chart") {
		label := fmt.Sprintf("%s chart reference #%d", drawingURI, idx+1)
		_, relDiags := validateXLSXInternalRelationshipReference(session, drawingURI, label, chart, relMap, namespaces.RelChart, namespaces.ContentTypeChart, "chart part", "XLSX_DRAWING_CHART_REFERENCE", partMap)
		diags = append(diags, relDiags...)
	}

	return diags
}

func validateDrawingAnchor(label string, anchor *etree.Element, anchorName string) []result.Diagnostic {
	var diags []result.Diagnostic
	if xlsxFindChildByLocal(anchor, "clientData") == nil {
		diags = append(diags, diag.Error(
			"XLSX_DRAWING_ANCHOR",
			fmt.Sprintf("%s is missing required <xdr:clientData>", label),
		))
	}

	switch anchorName {
	case "twoCellAnchor":
		fromMarker, fromOK := validateDrawingMarker(label, anchor, "from")
		toMarker, toOK := validateDrawingMarker(label, anchor, "to")
		diags = append(diags, fromMarker.diags...)
		diags = append(diags, toMarker.diags...)
		if fromOK && toOK && (toMarker.col < fromMarker.col || toMarker.row < fromMarker.row) {
			diags = append(diags, diag.Error(
				"XLSX_DRAWING_ANCHOR",
				fmt.Sprintf("%s has <xdr:to> before <xdr:from>", label),
			))
		}
	case "oneCellAnchor":
		marker, ok := validateDrawingMarker(label, anchor, "from")
		diags = append(diags, marker.diags...)
		if ok {
			diags = append(diags, validateDrawingPositiveExtent(label, anchor)...)
		}
	case "absoluteAnchor":
		diags = append(diags, validateDrawingPosition(label, anchor)...)
		diags = append(diags, validateDrawingPositiveExtent(label, anchor)...)
	}

	return diags
}

type drawingMarkerValidation struct {
	col   int
	row   int
	diags []result.Diagnostic
}

func validateDrawingMarker(label string, anchor *etree.Element, markerName string) (drawingMarkerValidation, bool) {
	marker := xlsxFindChildByLocal(anchor, markerName)
	if marker == nil {
		return drawingMarkerValidation{
			diags: []result.Diagnostic{
				diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s is missing required <xdr:%s>", label, markerName)),
			},
		}, false
	}

	var out drawingMarkerValidation
	col, okCol := drawingNonNegativeChildInt(marker, "col")
	row, okRow := drawingNonNegativeChildInt(marker, "row")
	out.col = col
	out.row = row
	if !okCol {
		out.diags = append(out.diags, diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s <xdr:%s> has missing or invalid non-negative <xdr:col>", label, markerName)))
	}
	if !okRow {
		out.diags = append(out.diags, diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s <xdr:%s> has missing or invalid non-negative <xdr:row>", label, markerName)))
	}
	for _, optional := range []string{"colOff", "rowOff"} {
		if child := xlsxFindChildByLocal(marker, optional); child != nil {
			if _, ok := nonNegativeIntText(child); !ok {
				out.diags = append(out.diags, diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s <xdr:%s> has invalid non-negative <xdr:%s>", label, markerName, optional)))
			}
		}
	}

	return out, okCol && okRow
}

func validateDrawingPosition(label string, anchor *etree.Element) []result.Diagnostic {
	pos := xlsxFindChildByLocal(anchor, "pos")
	if pos == nil {
		return []result.Diagnostic{
			diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s is missing required <xdr:pos>", label)),
		}
	}
	var diags []result.Diagnostic
	for _, attrName := range []string{"x", "y"} {
		if _, ok := nonNegativeIntString(pos.SelectAttrValue(attrName, "")); !ok {
			diags = append(diags, diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s <xdr:pos> has missing or invalid non-negative %s", label, attrName)))
		}
	}
	return diags
}

func validateDrawingPositiveExtent(label string, anchor *etree.Element) []result.Diagnostic {
	ext := xlsxFindChildByLocal(anchor, "ext")
	if ext == nil {
		return []result.Diagnostic{
			diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s is missing required <xdr:ext>", label)),
		}
	}
	var diags []result.Diagnostic
	for _, attrName := range []string{"cx", "cy"} {
		if value, ok := positiveIntString(ext.SelectAttrValue(attrName, "")); !ok || value <= 0 {
			diags = append(diags, diag.Error("XLSX_DRAWING_ANCHOR", fmt.Sprintf("%s <xdr:ext> has missing or invalid positive %s", label, attrName)))
		}
	}
	return diags
}

func validateWorksheetAutoFiltersAndSorts(sheetURI string, root *etree.Element) []result.Diagnostic {
	var diags []result.Diagnostic
	for idx, autoFilter := range xmlx.FindChildren(root, namespaces.NsSpreadsheet, "autoFilter") {
		label := fmt.Sprintf("%s <autoFilter #%d>", sheetURI, idx+1)
		filterRange, ok := validateAutoFilterRange(label, autoFilter)
		if !ok {
			diags = append(diags, filterRange.diags...)
			continue
		}
		diags = append(diags, filterRange.diags...)
		diags = append(diags, validateFilterColumns(label, autoFilter, filterRange.ref)...)
		if sortState := xmlx.FindChild(autoFilter, namespaces.NsSpreadsheet, "sortState"); sortState != nil {
			diags = append(diags, validateSortState(label+" <sortState>", sortState, &filterRange.ref)...)
		}
	}

	for idx, sortState := range xmlx.FindChildren(root, namespaces.NsSpreadsheet, "sortState") {
		label := fmt.Sprintf("%s <sortState #%d>", sheetURI, idx+1)
		diags = append(diags, validateSortState(label, sortState, nil)...)
	}

	return diags
}

type rangeValidation struct {
	ref   address.RangeRef
	diags []result.Diagnostic
}

func validateAutoFilterRange(label string, autoFilter *etree.Element) (rangeValidation, bool) {
	refText := strings.TrimSpace(autoFilter.SelectAttrValue("ref", ""))
	if refText == "" {
		return rangeValidation{
			diags: []result.Diagnostic{
				diag.Error("XLSX_AUTOFILTER_RANGE", fmt.Sprintf("%s is missing required ref", label)),
			},
		}, false
	}
	ref, err := address.ParseRange(refText)
	if err != nil {
		return rangeValidation{
			diags: []result.Diagnostic{
				diag.Error("XLSX_AUTOFILTER_RANGE", fmt.Sprintf("%s has invalid ref %q: %v", label, refText, err)),
			},
		}, false
	}
	return rangeValidation{ref: ref}, true
}

func validateFilterColumns(label string, autoFilter *etree.Element, filterRange address.RangeRef) []result.Diagnostic {
	var diags []result.Diagnostic
	minCol, _, maxCol, _ := filterRange.Bounds()
	width := maxCol - minCol + 1
	for idx, filterColumn := range xmlx.FindChildren(autoFilter, namespaces.NsSpreadsheet, "filterColumn") {
		colIDText := strings.TrimSpace(filterColumn.SelectAttrValue("colId", ""))
		colID, err := strconv.Atoi(colIDText)
		if err != nil || colID < 0 {
			diags = append(diags, diag.Error(
				"XLSX_AUTOFILTER_COLUMN",
				fmt.Sprintf("%s <filterColumn #%d> has invalid non-negative colId %q", label, idx+1, colIDText),
			))
			continue
		}
		if colID >= width {
			diags = append(diags, diag.Error(
				"XLSX_AUTOFILTER_COLUMN",
				fmt.Sprintf("%s <filterColumn #%d> colId %d is outside filter width %d", label, idx+1, colID, width),
			))
		}
	}
	return diags
}

func validateSortState(label string, sortState *etree.Element, containingRange *address.RangeRef) []result.Diagnostic {
	var diags []result.Diagnostic
	refText := strings.TrimSpace(sortState.SelectAttrValue("ref", ""))
	if refText == "" {
		return []result.Diagnostic{
			diag.Error("XLSX_SORT_STATE_RANGE", fmt.Sprintf("%s is missing required ref", label)),
		}
	}
	sortRange, err := address.ParseRange(refText)
	if err != nil {
		return []result.Diagnostic{
			diag.Error("XLSX_SORT_STATE_RANGE", fmt.Sprintf("%s has invalid ref %q: %v", label, refText, err)),
		}
	}
	if containingRange != nil && !xlsxRangeContains(*containingRange, sortRange) {
		diags = append(diags, diag.Error(
			"XLSX_SORT_STATE_RANGE",
			fmt.Sprintf("%s ref %q is outside containing autoFilter range %q", label, sortRange.String(), containingRange.String()),
		))
	}
	for idx, condition := range xmlx.FindChildren(sortState, namespaces.NsSpreadsheet, "sortCondition") {
		condRefText := strings.TrimSpace(condition.SelectAttrValue("ref", ""))
		if condRefText == "" {
			diags = append(diags, diag.Error(
				"XLSX_SORT_STATE_RANGE",
				fmt.Sprintf("%s <sortCondition #%d> is missing required ref", label, idx+1),
			))
			continue
		}
		condRange, err := address.ParseRange(condRefText)
		if err != nil {
			diags = append(diags, diag.Error(
				"XLSX_SORT_STATE_RANGE",
				fmt.Sprintf("%s <sortCondition #%d> has invalid ref %q: %v", label, idx+1, condRefText, err),
			))
			continue
		}
		if !xlsxRangeContains(sortRange, condRange) {
			diags = append(diags, diag.Error(
				"XLSX_SORT_STATE_RANGE",
				fmt.Sprintf("%s <sortCondition #%d> ref %q is outside sortState ref %q", label, idx+1, condRange.String(), sortRange.String()),
			))
		}
	}
	return diags
}

func validateXLSXInternalRelationshipReference(session opc.PackageSession, sourceURI, label string, elem *etree.Element, relMap map[string]opc.RelationshipInfo, expectedRelType, expectedContentType, expectedContent, code string, partMap map[string]bool) (string, []result.Diagnostic) {
	rid := xlsxRelationshipIDAttr(elem)
	if rid == "" {
		return "", []result.Diagnostic{
			diag.Error(code, fmt.Sprintf("%s is missing required r:id", label)),
		}
	}
	rel, ok := relMap[rid]
	if !ok {
		return "", []result.Diagnostic{
			diag.Error(code, fmt.Sprintf("%s references missing relationship %s", label, rid)),
		}
	}
	diags := validateXLSXRelationshipTarget(session, sourceURI, label, rel, expectedRelType, expectedContentType, expectedContent, code, partMap)
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, rel.Target)), diags
}

func validateXLSXRelationshipTarget(session opc.PackageSession, sourceURI, label string, rel opc.RelationshipInfo, expectedRelType, expectedContentType, expectedContent, code string, partMap map[string]bool) []result.Diagnostic {
	var diags []result.Diagnostic
	if strings.EqualFold(strings.TrimSpace(rel.TargetMode), "External") {
		return []result.Diagnostic{
			diag.Error(code, fmt.Sprintf("%s points to an external target; expected an internal %s", label, expectedContent)),
		}
	}
	if rel.Type != expectedRelType {
		diags = append(diags, diag.Error(
			code,
			fmt.Sprintf("%s has relationship type %q, expected %q", label, rel.Type, expectedRelType),
		))
	}
	targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, rel.Target))
	if targetURI == "" || !partMap[targetURI] {
		diags = append(diags, diag.Error(
			code,
			fmt.Sprintf("%s points to missing part %s", label, targetURI),
		))
		return diags
	}
	contentType := strings.TrimSpace(session.GetContentType(targetURI))
	if contentType != "" && contentType != expectedContentType {
		diags = append(diags, diag.Error(
			code,
			fmt.Sprintf("%s points to %s with content type %q, expected %s", label, targetURI, contentType, expectedContent),
		))
	}
	return diags
}

func xlsxRelationshipIDAttr(elem *etree.Element) string {
	if value, ok := namespaces.Attr(elem, namespaces.NsR, "id"); ok {
		return strings.TrimSpace(value)
	}
	return ""
}

func xlsxRelationshipsByID(rels []opc.RelationshipInfo) map[string]opc.RelationshipInfo {
	out := make(map[string]opc.RelationshipInfo, len(rels))
	for _, rel := range rels {
		if rel.ID == "" {
			continue
		}
		out[rel.ID] = rel
	}
	return out
}

func xlsxRelationshipIDOrPlaceholder(rel opc.RelationshipInfo) string {
	if strings.TrimSpace(rel.ID) == "" {
		return "(missing id)"
	}
	return rel.ID
}

func xlsxDefinedNameLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<definedName> at position %d", position)
	}
	name := strings.TrimSpace(elem.SelectAttrValue("name", ""))
	localSheetID := strings.TrimSpace(elem.SelectAttrValue("localSheetId", ""))
	var attrs []string
	if name != "" {
		attrs = append(attrs, fmt.Sprintf("name=%q", name))
	}
	if localSheetID != "" {
		attrs = append(attrs, fmt.Sprintf("localSheetId=%q", localSheetID))
	}
	if len(attrs) == 0 {
		return fmt.Sprintf("<definedName> at position %d", position)
	}
	return fmt.Sprintf("<definedName %s> at position %d", strings.Join(attrs, " "), position)
}

func xlsxWorkbookPivotCacheLabel(position int, elem *etree.Element) string {
	if elem == nil {
		return fmt.Sprintf("<pivotCache> at position %d", position)
	}
	cacheID := strings.TrimSpace(elem.SelectAttrValue("cacheId", ""))
	rid := xlsxRelationshipIDAttr(elem)
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

func workbookSheetNames(root *etree.Element) map[string]bool {
	names := make(map[string]bool)
	sheets := xmlx.FindChild(root, namespaces.NsSpreadsheet, "sheets")
	if sheets == nil {
		return names
	}
	for _, sheet := range xmlx.FindChildren(sheets, namespaces.NsSpreadsheet, "sheet") {
		name := strings.TrimSpace(sheet.SelectAttrValue("name", ""))
		if name != "" {
			names[strings.ToLower(name)] = true
		}
	}
	return names
}

func parseSimpleDefinedNameSheetReference(formula string) (string, string, bool) {
	formula = strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(formula), "="))
	if formula == "" || strings.Contains(formula, "[") || strings.Contains(formula, "]") {
		return "", "", false
	}
	bang := strings.LastIndex(formula, "!")
	if bang <= 0 || bang == len(formula)-1 {
		return "", "", false
	}
	sheetText := strings.TrimSpace(formula[:bang])
	refText := strings.TrimSpace(formula[bang+1:])
	if sheetText == "" || refText == "" || strings.ContainsAny(refText, " \t\r\n,()+-*/^&=<>") {
		return "", "", false
	}
	if strings.Contains(sheetText, ":") {
		return "", "", false
	}
	if !strings.HasPrefix(sheetText, "'") && strings.ContainsAny(sheetText, " \t\r\n,()+-*/^&=<>") {
		return "", "", false
	}
	sheetName := trimXLSXQuotedSheetName(sheetText)
	if sheetName == "" {
		return "", "", false
	}
	return sheetName, refText, true
}

func trimXLSXQuotedSheetName(value string) string {
	value = strings.TrimSpace(value)
	if len(value) >= 2 && strings.HasPrefix(value, "'") && strings.HasSuffix(value, "'") {
		value = value[1 : len(value)-1]
		value = strings.ReplaceAll(value, "''", "'")
	}
	return value
}

func validateDefinedNameReferenceText(refText string) error {
	if _, err := address.ParseRange(refText); err == nil {
		return nil
	} else if ok, wholeRefErr := validateDefinedNameWholeColumnOrRowReference(refText); ok {
		return wholeRefErr
	} else {
		return err
	}
}

func validateDefinedNameWholeColumnOrRowReference(refText string) (bool, error) {
	parts := strings.Split(refText, ":")
	if len(parts) > 2 {
		return false, nil
	}

	firstKind, err := validateDefinedNameWholeReferencePart(parts[0])
	if firstKind == "" {
		return false, nil
	}
	if err != nil {
		return true, err
	}
	if len(parts) == 1 {
		return true, nil
	}

	secondKind, err := validateDefinedNameWholeReferencePart(parts[1])
	if secondKind == "" {
		return false, nil
	}
	if err != nil {
		return true, err
	}
	if firstKind != secondKind {
		return true, fmt.Errorf("mixed whole-column and whole-row reference")
	}
	return true, nil
}

func validateDefinedNameWholeReferencePart(part string) (string, error) {
	normalized := strings.ReplaceAll(strings.TrimSpace(part), "$", "")
	if normalized == "" {
		return "", nil
	}

	allLetters := true
	allDigits := true
	for _, r := range normalized {
		if r < 'A' || r > 'Z' {
			if r < 'a' || r > 'z' {
				allLetters = false
			}
		}
		if r < '0' || r > '9' {
			allDigits = false
		}
	}
	if allLetters {
		if _, err := address.ParseColumn(normalized); err != nil {
			return "column", err
		}
		return "column", nil
	}
	if allDigits {
		row, err := strconv.Atoi(normalized)
		if err != nil {
			return "row", fmt.Errorf("invalid row reference %q: %w", part, err)
		}
		if row < 1 || row > address.MaxRow {
			return "row", fmt.Errorf("row %d out of XLSX bounds 1-%d", row, address.MaxRow)
		}
		return "row", nil
	}
	return "", nil
}

func xlsxFindChildByLocal(parent *etree.Element, local string) *etree.Element {
	if parent == nil {
		return nil
	}
	for _, child := range parent.ChildElements() {
		if xlsxLocalName(child) == local {
			return child
		}
	}
	return nil
}

func xlsxLocalName(elem *etree.Element) string {
	if elem == nil {
		return ""
	}
	if idx := strings.LastIndex(elem.Tag, "}"); idx >= 0 && idx+1 < len(elem.Tag) {
		return elem.Tag[idx+1:]
	}
	if idx := strings.LastIndex(elem.Tag, ":"); idx >= 0 && idx+1 < len(elem.Tag) {
		return elem.Tag[idx+1:]
	}
	return elem.Tag
}

func drawingNonNegativeChildInt(parent *etree.Element, local string) (int, bool) {
	child := xlsxFindChildByLocal(parent, local)
	if child == nil {
		return 0, false
	}
	return nonNegativeIntText(child)
}

func nonNegativeIntText(elem *etree.Element) (int, bool) {
	return nonNegativeIntString(elem.Text())
}

func nonNegativeIntString(value string) (int, bool) {
	parsed, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || parsed < 0 {
		return 0, false
	}
	return parsed, true
}

func positiveIntString(value string) (int, bool) {
	parsed, err := strconv.Atoi(strings.TrimSpace(value))
	if err != nil || parsed <= 0 {
		return 0, false
	}
	return parsed, true
}

func xlsxRangeContains(outer, inner address.RangeRef) bool {
	outerMinCol, outerMinRow, outerMaxCol, outerMaxRow := outer.Bounds()
	innerMinCol, innerMinRow, innerMaxCol, innerMaxRow := inner.Bounds()
	return innerMinCol >= outerMinCol && innerMaxCol <= outerMaxCol && innerMinRow >= outerMinRow && innerMaxRow <= outerMaxRow
}

func xlsxElementMatches(elem *etree.Element, local string) bool {
	return xmlx.ElementMatches(elem, namespaces.NsSpreadsheet, local) || elem.Tag == local
}
