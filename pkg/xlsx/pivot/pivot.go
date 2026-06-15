// Package pivot reads existing XLSX PivotTable definitions.
package pivot

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// List returns existing worksheet PivotTables in workbook and worksheet order.
func List(session opc.PackageSession, workbook *model.Workbook, sheets []model.SheetRef) ([]model.PivotRef, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if workbook == nil {
		return nil, fmt.Errorf("workbook is nil")
	}
	if sheets == nil {
		sheets = workbook.Sheets
	}

	cacheMap, err := workbookPivotCaches(session, workbook)
	if err != nil {
		return nil, err
	}

	var pivots []model.PivotRef
	for _, sheetRef := range sheets {
		if sheetRef.PartURI == "" || sheetRef.RelationshipType != namespaces.RelWorksheet {
			continue
		}
		sheetPivots, err := listForSheet(session, sheetRef, cacheMap, len(pivots)+1)
		if err != nil {
			return nil, err
		}
		pivots = append(pivots, sheetPivots...)
	}
	return pivots, nil
}

type cacheTarget struct {
	cacheID int
	rid     string
	uri     string
}

func workbookPivotCaches(session opc.PackageSession, workbook *model.Workbook) (map[int]cacheTarget, error) {
	result := map[int]cacheTarget{}
	if workbook == nil || workbook.PartURI == "" {
		return result, nil
	}

	doc, err := session.ReadXMLPart(workbook.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read workbook %s: %w", workbook.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "workbook") {
		return nil, fmt.Errorf("workbook part %s root element not found", workbook.PartURI)
	}

	pivotCaches := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "pivotCaches")
	if pivotCaches == nil {
		return result, nil
	}
	relMap := mapRelationships(session.ListRelationships(workbook.PartURI))
	for _, elem := range namespaces.FindChildren(pivotCaches, namespaces.NsSpreadsheetML, "pivotCache") {
		cacheID := parseOptionalInt(elem.SelectAttrValue("cacheId", ""), 0)
		rid, ok := namespaces.Attr(elem, namespaces.NsR, "id")
		if cacheID <= 0 || !ok || rid == "" {
			continue
		}
		rel, ok := relMap[rid]
		if !ok || rel.TargetMode == "External" || rel.Type != namespaces.RelPivotCache {
			continue
		}
		result[cacheID] = cacheTarget{
			cacheID: cacheID,
			rid:     rid,
			uri:     resolveTargetURI(workbook.PartURI, rel.Target),
		}
	}
	return result, nil
}

func listForSheet(session opc.PackageSession, sheetRef model.SheetRef, cacheMap map[int]cacheTarget, startNumber int) ([]model.PivotRef, error) {
	doc, err := session.ReadXMLPart(sheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", sheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", sheetRef.PartURI)
	}

	_ = root
	// PivotTables are linked from the worksheet via pivotTable relationships
	// (the standard packaging); the worksheet XML itself carries no
	// pivotTableDefinition element.
	var pivots []model.PivotRef
	for _, rel := range session.ListRelationships(sheetRef.PartURI) {
		if rel.TargetMode == "External" || rel.Type != namespaces.RelPivotTable {
			continue
		}
		pivotURI := resolveTargetURI(sheetRef.PartURI, rel.Target)
		pivot, err := readPart(session, pivotURI, cacheMap)
		if err != nil {
			return nil, err
		}
		pivot.Number = startNumber + len(pivots)
		pivot.Sheet = sheetRef.Name
		pivot.SheetNumber = sheetRef.Number
		pivot.SheetPartURI = sheetRef.PartURI
		pivot.RelationshipID = rel.ID
		pivot.PartURI = pivotURI
		pivot = model.WithPivotSelectors(pivot)
		pivots = append(pivots, pivot)
	}
	return pivots, nil
}

func readPart(session opc.PackageSession, pivotURI string, cacheMap map[int]cacheTarget) (model.PivotRef, error) {
	doc, err := session.ReadXMLPart(pivotURI)
	if err != nil {
		return model.PivotRef{}, fmt.Errorf("failed to read pivot table part %s: %w", pivotURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "pivotTableDefinition") {
		return model.PivotRef{}, fmt.Errorf("pivot table part %s root element not found", pivotURI)
	}

	cacheID := parseOptionalInt(root.SelectAttrValue("cacheId", ""), 0)
	cacheTarget := cacheTarget{cacheID: cacheID}
	for _, rel := range session.ListRelationships(pivotURI) {
		if rel.TargetMode == "External" || rel.Type != namespaces.RelPivotCache {
			continue
		}
		cacheTarget.rid = rel.ID
		cacheTarget.uri = resolveTargetURI(pivotURI, rel.Target)
		break
	}
	if cacheTarget.uri == "" && cacheID > 0 {
		if target, ok := cacheMap[cacheID]; ok {
			cacheTarget = target
		}
	}

	var cache *model.PivotCacheRef
	if cacheTarget.uri != "" {
		cache, err = readCacheDefinition(session, cacheTarget)
		if err != nil {
			return model.PivotRef{}, err
		}
	}

	fieldNames := pivotFieldNames(cache)
	pivot := model.PivotRef{
		Name:         root.SelectAttrValue("name", ""),
		CacheID:      cacheID,
		Cache:        cache,
		Fields:       parsePivotFields(root, fieldNames),
		RowFields:    parseAxisFields(root, "row", "rowFields", "field", "x", fieldNames),
		ColumnFields: parseAxisFields(root, "column", "colFields", "field", "x", fieldNames),
		FilterFields: parseAxisFields(root, "filter", "pageFields", "pageField", "fld", fieldNames),
		DataFields:   parseDataFields(root, fieldNames),
	}
	if location := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "location"); location != nil {
		pivot.Location = strings.TrimSpace(location.SelectAttrValue("ref", ""))
		if rangeRef, err := address.ParseRange(pivot.Location); err == nil {
			minCol, minRow, maxCol, maxRow := rangeRef.Bounds()
			pivot.Rows = maxRow - minRow + 1
			pivot.Cols = maxCol - minCol + 1
		}
	}
	return pivot, nil
}

func readCacheDefinition(session opc.PackageSession, target cacheTarget) (*model.PivotCacheRef, error) {
	doc, err := session.ReadXMLPart(target.uri)
	if err != nil {
		return nil, fmt.Errorf("failed to read pivot cache part %s: %w", target.uri, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "pivotCacheDefinition") {
		return nil, fmt.Errorf("pivot cache part %s root element not found", target.uri)
	}

	cache := &model.PivotCacheRef{
		CacheID:          target.cacheID,
		PartURI:          target.uri,
		RelationshipID:   target.rid,
		RecordCount:      parseOptionalInt(root.SelectAttrValue("recordCount", ""), 0),
		CreatedVersion:   root.SelectAttrValue("createdVersion", ""),
		RefreshedVersion: root.SelectAttrValue("refreshedVersion", ""),
		RefreshOnLoad:    parseBool(root.SelectAttrValue("refreshOnLoad", "")),
	}
	if saveDataText := strings.TrimSpace(root.SelectAttrValue("saveData", "")); saveDataText != "" {
		saveData := parseBool(saveDataText)
		cache.SaveData = &saveData
	}
	for _, rel := range session.ListRelationships(target.uri) {
		if rel.TargetMode == "External" || rel.Type != namespaces.RelPivotRecords {
			continue
		}
		cache.RecordsPartURI = resolveTargetURI(target.uri, rel.Target)
		break
	}
	if cacheSource := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "cacheSource"); cacheSource != nil {
		cache.Source.Type = cacheSource.SelectAttrValue("type", "")
		if worksheetSource := namespaces.FindChild(cacheSource, namespaces.NsSpreadsheetML, "worksheetSource"); worksheetSource != nil {
			cache.Source.Sheet = worksheetSource.SelectAttrValue("sheet", "")
			cache.Source.Range = worksheetSource.SelectAttrValue("ref", "")
			cache.Source.Name = worksheetSource.SelectAttrValue("name", "")
		}
	}
	if cacheFields := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "cacheFields"); cacheFields != nil {
		for index, elem := range namespaces.FindChildren(cacheFields, namespaces.NsSpreadsheetML, "cacheField") {
			cache.Fields = append(cache.Fields, model.PivotCacheField{
				Index: index,
				Name:  elem.SelectAttrValue("name", ""),
			})
		}
	}
	return cache, nil
}

func parsePivotFields(root *etree.Element, fieldNames map[int]string) []model.PivotFieldRef {
	pivotFields := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "pivotFields")
	if pivotFields == nil {
		return nil
	}
	var fields []model.PivotFieldRef
	for index, elem := range namespaces.FindChildren(pivotFields, namespaces.NsSpreadsheetML, "pivotField") {
		field := model.PivotFieldRef{
			Index: index,
			Name:  fieldNames[index],
			Axis:  normalizeAxis(elem.SelectAttrValue("axis", "")),
		}
		if subtotal := firstEnabledSubtotal(elem); subtotal != "" {
			field.Subtotal = subtotal
		}
		fields = append(fields, field)
	}
	return fields
}

func parseAxisFields(root *etree.Element, axis, parentName, childName, attrName string, fieldNames map[int]string) []model.PivotFieldRef {
	parent := namespaces.FindChild(root, namespaces.NsSpreadsheetML, parentName)
	if parent == nil {
		return nil
	}
	var fields []model.PivotFieldRef
	for _, elem := range namespaces.FindChildren(parent, namespaces.NsSpreadsheetML, childName) {
		index := parseOptionalInt(elem.SelectAttrValue(attrName, ""), -1)
		fields = append(fields, model.PivotFieldRef{
			Index: index,
			Name:  fieldNames[index],
			Axis:  axis,
		})
	}
	return fields
}

func parseDataFields(root *etree.Element, fieldNames map[int]string) []model.PivotFieldRef {
	dataFields := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "dataFields")
	if dataFields == nil {
		return nil
	}
	var fields []model.PivotFieldRef
	for _, elem := range namespaces.FindChildren(dataFields, namespaces.NsSpreadsheetML, "dataField") {
		index := parseOptionalInt(elem.SelectAttrValue("fld", ""), -1)
		fields = append(fields, model.PivotFieldRef{
			Index:    index,
			Name:     fieldNames[index],
			Axis:     "data",
			Subtotal: elem.SelectAttrValue("subtotal", ""),
			Caption:  elem.SelectAttrValue("name", ""),
		})
	}
	return fields
}

func pivotFieldNames(cache *model.PivotCacheRef) map[int]string {
	names := map[int]string{}
	if cache == nil {
		return names
	}
	for _, field := range cache.Fields {
		names[field.Index] = field.Name
	}
	return names
}

func firstEnabledSubtotal(elem *etree.Element) string {
	for _, name := range []string{"sumSubtotal", "countASubtotal", "avgSubtotal", "maxSubtotal", "minSubtotal", "productSubtotal", "countSubtotal", "stdDevSubtotal", "stdDevPSubtotal", "varSubtotal", "varPSubtotal"} {
		if parseBool(elem.SelectAttrValue(name, "")) {
			return strings.TrimSuffix(name, "Subtotal")
		}
	}
	return ""
}

func normalizeAxis(value string) string {
	switch value {
	case "axisRow":
		return "row"
	case "axisCol":
		return "column"
	case "axisPage":
		return "filter"
	case "axisValues":
		return "data"
	default:
		return value
	}
}

func mapRelationships(rels []opc.RelationshipInfo) map[string]opc.RelationshipInfo {
	mapped := make(map[string]opc.RelationshipInfo, len(rels))
	for _, rel := range rels {
		mapped[rel.ID] = rel
	}
	return mapped
}

func resolveTargetURI(sourceURI, target string) string {
	if strings.HasPrefix(target, "/") {
		return opc.NormalizeURI(target)
	}
	return opc.NormalizeURI(opc.ResolveRelationshipTarget(sourceURI, target))
}

func parseOptionalInt(value string, fallback int) int {
	if strings.TrimSpace(value) == "" {
		return fallback
	}
	parsed, err := strconv.Atoi(value)
	if err != nil {
		return fallback
	}
	return parsed
}

func parseBool(value string) bool {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "1", "true", "on":
		return true
	default:
		return false
	}
}
