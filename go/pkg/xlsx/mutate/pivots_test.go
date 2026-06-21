package mutate

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/rangeio"
)

func TestCreatePivotWritesPivotTableCacheRelationship(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	sourceRange, err := address.ParseRange("A1:C5")
	if err != nil {
		t.Fatalf("ParseRange returned error: %v", err)
	}
	targetAnchor, err := address.ParseCell("F1")
	if err != nil {
		t.Fatalf("ParseCell returned error: %v", err)
	}

	result, err := CreatePivot(&CreatePivotRequest{
		Package:      pkg,
		WorkbookURI:  workbook.PartURI,
		SourceSheet:  workbook.Sheets[0].Name,
		SourceRange:  sourceRange,
		TargetSheet:  workbook.Sheets[0],
		TargetAnchor: targetAnchor,
		RowFields:    []string{"Region"},
		ValueFields:  []PivotValueSpec{{Name: "Sales", Aggregation: "sum"}},
		SourceCells: [][]rangeio.Cell{
			{{Value: "Region"}, {Value: "Product"}, {Value: "Sales"}},
			{{Value: "North"}, {Value: "A"}, {Value: "42"}},
			{{Value: "South"}, {Value: "A"}, {Value: "58"}},
			{{Value: "North"}, {Value: "B"}, {Value: "30"}},
			{{Value: "South"}, {Value: "B"}, {Value: "33"}},
		},
	})
	if err != nil {
		t.Fatalf("CreatePivot returned error: %v", err)
	}

	rels := pkg.ListRelationships(result.PivotTableURI)
	if len(rels) != 1 {
		t.Fatalf("pivot table relationship count = %d, want 1: %+v", len(rels), rels)
	}
	if rels[0].Type != namespaces.RelPivotCache {
		t.Fatalf("pivot table relationship type = %q, want %q", rels[0].Type, namespaces.RelPivotCache)
	}
	if rels[0].Target != "../pivotCache/pivotCacheDefinition1.xml" {
		t.Fatalf("pivot table relationship target = %q, want ../pivotCache/pivotCacheDefinition1.xml", rels[0].Target)
	}
}

func TestCreatePivotDoesNotWriteInvalidWorksheetPivotTableDefinitionMarker(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()

	sourceRange, err := address.ParseRange("A1:C5")
	if err != nil {
		t.Fatalf("ParseRange returned error: %v", err)
	}
	targetAnchor, err := address.ParseCell("F1")
	if err != nil {
		t.Fatalf("ParseCell returned error: %v", err)
	}

	_, err = CreatePivot(&CreatePivotRequest{
		Package:      pkg,
		WorkbookURI:  workbook.PartURI,
		SourceSheet:  workbook.Sheets[0].Name,
		SourceRange:  sourceRange,
		TargetSheet:  workbook.Sheets[0],
		TargetAnchor: targetAnchor,
		RowFields:    []string{"Region"},
		ValueFields:  []PivotValueSpec{{Name: "Sales", Aggregation: "sum"}},
		SourceCells: [][]rangeio.Cell{
			{{Value: "Region"}, {Value: "Product"}, {Value: "Sales"}},
			{{Value: "North"}, {Value: "A"}, {Value: "42"}},
			{{Value: "South"}, {Value: "A"}, {Value: "58"}},
			{{Value: "North"}, {Value: "B"}, {Value: "30"}},
			{{Value: "South"}, {Value: "B"}, {Value: "33"}},
		},
	})
	if err != nil {
		t.Fatalf("CreatePivot returned error: %v", err)
	}

	sheet := workbook.Sheets[0]
	var pivotRID string
	for _, rel := range pkg.ListRelationships(sheet.PartURI) {
		if rel.Type == namespaces.RelPivotTable {
			pivotRID = rel.ID
			break
		}
	}
	if pivotRID == "" {
		t.Fatal("worksheet pivot table relationship missing")
	}

	doc, err := pkg.ReadXMLPart(sheet.PartURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	markers := namespaces.FindChildren(doc.Root(), namespaces.NsSpreadsheetML, "pivotTableDefinition")
	if len(markers) != 0 {
		t.Fatalf("worksheet pivot marker count = %d, want 0 because pivotTableDefinition is a pivot table part root, not a worksheet child", len(markers))
	}
}
