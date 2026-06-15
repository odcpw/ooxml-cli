// Package table reads and mutates existing XLSX table parts.
package table

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

// List returns existing worksheet tables in workbook and worksheet order.
func List(session opc.PackageSession, workbook *model.Workbook, sheets []model.SheetRef) ([]model.TableRef, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if workbook == nil {
		return nil, fmt.Errorf("workbook is nil")
	}
	if sheets == nil {
		sheets = workbook.Sheets
	}

	var tables []model.TableRef
	for _, sheetRef := range sheets {
		if sheetRef.PartURI == "" || sheetRef.RelationshipType != namespaces.RelWorksheet {
			continue
		}
		sheetTables, err := listForSheet(session, sheetRef, len(tables)+1)
		if err != nil {
			return nil, err
		}
		tables = append(tables, sheetTables...)
	}
	return tables, nil
}

func listForSheet(session opc.PackageSession, sheetRef model.SheetRef, startNumber int) ([]model.TableRef, error) {
	doc, err := session.ReadXMLPart(sheetRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read worksheet %s: %w", sheetRef.PartURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "worksheet") {
		return nil, fmt.Errorf("worksheet part %s root element not found", sheetRef.PartURI)
	}

	tableParts := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "tableParts")
	if tableParts == nil {
		return nil, nil
	}

	relMap := mapRelationships(session.ListRelationships(sheetRef.PartURI))
	tablePartElems := namespaces.FindChildren(tableParts, namespaces.NsSpreadsheetML, "tablePart")
	tables := make([]model.TableRef, 0, len(tablePartElems))
	for _, tablePart := range tablePartElems {
		rid, ok := namespaces.Attr(tablePart, namespaces.NsR, "id")
		if !ok || rid == "" {
			return nil, fmt.Errorf("worksheet %s tablePart is missing r:id", sheetRef.PartURI)
		}
		rel, ok := relMap[rid]
		if !ok {
			return nil, fmt.Errorf("worksheet %s table relationship %s not found", sheetRef.PartURI, rid)
		}
		if rel.TargetMode == "External" {
			return nil, fmt.Errorf("worksheet %s table relationship %s is external", sheetRef.PartURI, rid)
		}
		if rel.Type != namespaces.RelTable {
			return nil, fmt.Errorf("worksheet %s relationship %s is %s, expected table", sheetRef.PartURI, rid, rel.Type)
		}
		tableURI := resolveTargetURI(sheetRef.PartURI, rel.Target)
		table, err := ReadPart(session, tableURI)
		if err != nil {
			return nil, err
		}
		table.Number = startNumber + len(tables)
		table.Sheet = sheetRef.Name
		table.SheetNumber = sheetRef.Number
		table.SheetPartURI = sheetRef.PartURI
		table.RelationshipID = rid
		table.PartURI = tableURI
		table.PrimarySelector = ""
		table.Selectors = nil
		*table = model.WithTableSelectors(*table)
		tables = append(tables, *table)
	}
	return tables, nil
}

// ReadPart parses one table part.
func ReadPart(session opc.PackageSession, tableURI string) (*model.TableRef, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if tableURI == "" {
		return nil, fmt.Errorf("table part URI is empty")
	}
	doc, err := session.ReadXMLPart(tableURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read table part %s: %w", tableURI, err)
	}
	return ParsePart(doc, tableURI)
}

// ParsePart parses table XML into stable metadata.
func ParsePart(doc *etree.Document, tableURI string) (*model.TableRef, error) {
	if doc == nil || doc.Root() == nil {
		return nil, fmt.Errorf("table part %s has no root element", tableURI)
	}
	root := doc.Root()
	if !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "table") {
		return nil, fmt.Errorf("table part %s root element not found", tableURI)
	}

	refText := root.SelectAttrValue("ref", "")
	rangeRef, err := address.ParseRange(refText)
	if err != nil {
		return nil, fmt.Errorf("invalid table ref %q in %s: %w", refText, tableURI, err)
	}
	minCol, minRow, maxCol, maxRow := rangeRef.Bounds()
	rows := maxRow - minRow + 1
	cols := maxCol - minCol + 1

	headerRows := parseOptionalInt(root.SelectAttrValue("headerRowCount", ""), 1)
	totalsRows := parseOptionalInt(root.SelectAttrValue("totalsRowCount", ""), 0)
	dataRows := rows - headerRows - totalsRows
	if dataRows < 0 {
		dataRows = 0
	}

	table := &model.TableRef{
		PartURI:        tableURI,
		ID:             parseOptionalInt(root.SelectAttrValue("id", ""), 0),
		Name:           root.SelectAttrValue("name", ""),
		DisplayName:    root.SelectAttrValue("displayName", ""),
		Range:          rangeRef.String(),
		Rows:           rows,
		Cols:           cols,
		HeaderRowCount: headerRows,
		DataRowCount:   dataRows,
		TotalsRowCount: totalsRows,
	}
	if table.DisplayName == "" {
		table.DisplayName = table.Name
	}

	if styleInfo := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "tableStyleInfo"); styleInfo != nil {
		table.StyleName = styleInfo.SelectAttrValue("name", "")
	}
	if tableColumns := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "tableColumns"); tableColumns != nil {
		for _, col := range namespaces.FindChildren(tableColumns, namespaces.NsSpreadsheetML, "tableColumn") {
			table.Columns = append(table.Columns, model.TableColumn{
				ID:   parseOptionalInt(col.SelectAttrValue("id", ""), 0),
				Name: col.SelectAttrValue("name", ""),
			})
		}
	}
	*table = model.WithTableSelectors(*table)
	return table, nil
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
