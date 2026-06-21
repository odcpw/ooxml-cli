package table

import (
	"strings"
	"testing"

	"github.com/beevik/etree"
)

func TestParsePart(t *testing.T) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="7" name="Sales" displayName="Sales" ref="A1:C4" headerRowCount="1" totalsRowCount="0">
  <autoFilter ref="A1:C4"/>
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Amount"/>
    <tableColumn id="3" name="Active"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2"/>
</table>`); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	tableRef, err := ParsePart(doc, "/xl/tables/table1.xml")
	if err != nil {
		t.Fatalf("ParsePart returned error: %v", err)
	}
	if tableRef.ID != 7 || tableRef.DisplayName != "Sales" || tableRef.Range != "A1:C4" {
		t.Fatalf("unexpected table metadata: %+v", tableRef)
	}
	if tableRef.Rows != 4 || tableRef.Cols != 3 || tableRef.DataRowCount != 3 {
		t.Fatalf("unexpected table dimensions: %+v", tableRef)
	}
	if len(tableRef.Columns) != 3 || tableRef.Columns[2].Name != "Active" {
		t.Fatalf("unexpected columns: %+v", tableRef.Columns)
	}
	if tableRef.StyleName != "TableStyleMedium2" {
		t.Fatalf("style = %q, want TableStyleMedium2", tableRef.StyleName)
	}
}

func TestParsePartRejectsInvalidRange(t *testing.T) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" ref="bad"><tableColumns/></table>`); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}
	_, err := ParsePart(doc, "/xl/tables/table1.xml")
	if err == nil || !strings.Contains(err.Error(), "invalid table ref") {
		t.Fatalf("ParsePart error = %v, want invalid table ref", err)
	}
}
