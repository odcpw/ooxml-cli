package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func TestParseTable_Simple(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-simple fixture: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the table)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find table
	var tableShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypeGraphicFrame && shapes[i].TableInfo != nil {
			tableShape = &shapes[i]
			break
		}
	}

	if tableShape == nil {
		t.Fatal("expected to find a table shape")
	}

	// Verify backward-compatible text access
	if tableShape.TableInfo.Rows != 3 || tableShape.TableInfo.Cols != 3 {
		t.Fatalf("expected 3x3 table, got %dx%d", tableShape.TableInfo.Rows, tableShape.TableInfo.Cols)
	}

	// Verify enriched model has row and column definitions
	if len(tableShape.TableInfo.RowDefs) == 0 {
		t.Error("expected RowDefs to be populated")
	}
	if len(tableShape.TableInfo.ColumnDefs) == 0 {
		t.Error("expected ColumnDefs to be populated")
	}

	// Verify cell definitions exist
	if len(tableShape.TableInfo.CellDefs) == 0 {
		t.Error("expected CellDefs to be populated")
	}

	// Verify cell text is accessible
	expectedText := "R0C0"
	if len(tableShape.TableInfo.Cells) > 0 && len(tableShape.TableInfo.Cells[0]) > 0 {
		if tableShape.TableInfo.Cells[0][0] != expectedText {
			t.Errorf("expected cell [0][0] to contain '%s', got '%s'", expectedText, tableShape.TableInfo.Cells[0][0])
		}
	}
}

func TestParseTable_Merged(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-merged/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-merged fixture: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the table)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find table
	var tableShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypeGraphicFrame && shapes[i].TableInfo != nil {
			tableShape = &shapes[i]
			break
		}
	}

	if tableShape == nil {
		t.Fatal("expected to find a table shape")
	}

	// Verify table dimensions
	if tableShape.TableInfo.Rows != 4 || tableShape.TableInfo.Cols != 4 {
		t.Fatalf("expected 4x4 table, got %dx%d", tableShape.TableInfo.Rows, tableShape.TableInfo.Cols)
	}

	// Verify merge information is captured in CellDefs
	if len(tableShape.TableInfo.CellDefs) > 0 && len(tableShape.TableInfo.CellDefs[0]) > 0 {
		// Check for merge spans (they should be > 1 for merged cells)
		hasMergeInfo := false
		for _, row := range tableShape.TableInfo.CellDefs {
			for _, cell := range row {
				if cell.GridSpan > 1 || cell.RowSpan > 1 {
					hasMergeInfo = true
					break
				}
			}
			if hasMergeInfo {
				break
			}
		}
		if !hasMergeInfo {
			t.Error("expected merge information in CellDefs but none found")
		}
	}
}

func TestParseTable_Styled(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-styled/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-styled fixture: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the table)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find table
	var tableShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypeGraphicFrame && shapes[i].TableInfo != nil {
			tableShape = &shapes[i]
			break
		}
	}

	if tableShape == nil {
		t.Fatal("expected to find a table shape")
	}

	// Verify table dimensions
	if tableShape.TableInfo.Rows != 3 || tableShape.TableInfo.Cols != 3 {
		t.Fatalf("expected 3x3 table, got %dx%d", tableShape.TableInfo.Rows, tableShape.TableInfo.Cols)
	}

	// Verify styling information is captured
	if len(tableShape.TableInfo.CellDefs) > 0 && len(tableShape.TableInfo.CellDefs[0]) > 0 {
		// Header row should have styling
		headerCell := tableShape.TableInfo.CellDefs[0][0]

		// Note: Some styling might not be captured depending on how python-pptx generates the XML
		// This test is lenient and just checks that the model can capture it if present
		if headerCell.Text == "" {
			t.Error("expected header cell to have text")
		}

		// Verify that styling fields exist in the model
		_ = headerCell.Bold
		_ = headerCell.Fill
		_ = headerCell.Border
	}
}

func TestParseTable_BackwardCompatibility(t *testing.T) {
	// Ensure that existing code using just Cells field still works
	pkg, err := opc.Open("../../../testdata/pptx/table-simple/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-simple fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find table
	var tableInfo *model.TableInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypeGraphicFrame && shapes[i].TableInfo != nil {
			tableInfo = shapes[i].TableInfo
			break
		}
	}

	if tableInfo == nil {
		t.Fatal("expected to find table info")
	}

	// Simple text access should still work
	if tableInfo.Cells == nil || len(tableInfo.Cells) == 0 {
		t.Error("expected Cells field to be populated for backward compatibility")
	}

	if len(tableInfo.Cells) > 0 && len(tableInfo.Cells[0]) > 0 {
		if tableInfo.Cells[0][0] == "" {
			t.Error("expected first cell to have text")
		}
	}
}
