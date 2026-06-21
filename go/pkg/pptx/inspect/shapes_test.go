package inspect

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func TestEnumerateShapes_MinimalTitle(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open minimal-title fixture: %v", err)
	}
	defer pkg.Close()

	// Read slide1
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	// Get the shape tree
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Should have 2 shapes: title and subtitle
	if len(shapes) != 2 {
		t.Errorf("expected 2 shapes, got %d", len(shapes))
	}

	// Check first shape (title)
	if shapes[0].ID != 2 {
		t.Errorf("shape 0: expected ID 2, got %d", shapes[0].ID)
	}
	if shapes[0].Name != "Title 1" {
		t.Errorf("shape 0: expected name 'Title 1', got '%s'", shapes[0].Name)
	}
	if shapes[0].Type != model.ShapeTypeSP {
		t.Errorf("shape 0: expected type %s, got %s", model.ShapeTypeSP, shapes[0].Type)
	}
	if !shapes[0].IsPlaceholder {
		t.Errorf("shape 0: expected IsPlaceholder true, got false")
	}

	// Check second shape (subtitle)
	if shapes[1].ID != 3 {
		t.Errorf("shape 1: expected ID 3, got %d", shapes[1].ID)
	}
	if shapes[1].Name != "Subtitle 2" {
		t.Errorf("shape 1: expected name 'Subtitle 2', got '%s'", shapes[1].Name)
	}
	if shapes[1].Type != model.ShapeTypeSP {
		t.Errorf("shape 1: expected type %s, got %s", model.ShapeTypeSP, shapes[1].Type)
	}
	if !shapes[1].IsPlaceholder {
		t.Errorf("shape 1: expected IsPlaceholder true, got false")
	}
}

func TestEnumerateShapes_TableSlide(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-slide fixture: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the table)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	// Get the shape tree
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Should have at least 1 shape (the table)
	if len(shapes) == 0 {
		t.Fatal("expected at least 1 shape, got 0")
	}

	// Find the graphic frame (table)
	var tableShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypeGraphicFrame {
			tableShape = &shapes[i]
			break
		}
	}

	if tableShape == nil {
		t.Fatal("expected to find a graphicFrame shape")
	}

	// Check table dimensions
	if tableShape.TableInfo == nil {
		t.Fatal("expected TableInfo to be set")
	}

	if tableShape.TableInfo.Rows != 3 {
		t.Errorf("expected 3 rows, got %d", tableShape.TableInfo.Rows)
	}
	if tableShape.TableInfo.Cols != 3 {
		t.Errorf("expected 3 columns, got %d", tableShape.TableInfo.Cols)
	}
}

func TestEnumerateShapes_PicturePlaceholder(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open picture-placeholder fixture: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the picture)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	// Get the shape tree
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Should have at least 1 shape (the picture)
	if len(shapes) == 0 {
		t.Fatal("expected at least 1 shape, got 0")
	}

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Check picture properties
	if picShape.ID != 2 {
		t.Errorf("expected ID 2, got %d", picShape.ID)
	}
	if picShape.Name != "Picture 1" {
		t.Errorf("expected name 'Picture 1', got '%s'", picShape.Name)
	}
	if picShape.ImageRef == nil {
		t.Fatal("expected ImageRef to be set")
	}
	if picShape.ImageRef.RelID != "rId2" {
		t.Errorf("expected RelID 'rId2', got '%s'", picShape.ImageRef.RelID)
	}
}

func TestEnumerateShapes_EmptyTree(t *testing.T) {
	// Test with nil element
	shapes := EnumerateShapes(nil)
	if len(shapes) != 0 {
		t.Errorf("expected 0 shapes for nil tree, got %d", len(shapes))
	}

	// Test with empty tree
	emptyTree := etree.NewElement("{http://schemas.openxmlformats.org/presentationml/2006/main}spTree")
	shapes = EnumerateShapes(emptyTree)
	if len(shapes) != 0 {
		t.Errorf("expected 0 shapes for empty tree, got %d", len(shapes))
	}
}

func TestTableInfo_Correct(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/table-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-slide fixture: %v", err)
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

	// Verify table structure: 3x3 with correct content
	if tableShape.TableInfo.Rows != 3 || tableShape.TableInfo.Cols != 3 {
		t.Fatalf("expected 3x3 table, got %dx%d", tableShape.TableInfo.Rows, tableShape.TableInfo.Cols)
	}

	expectedCells := [][]string{
		{"R0C0", "R0C1", "R0C2"},
		{"R1C0", "R1C1", "R1C2"},
		{"R2C0", "R2C1", "R2C2"},
	}

	for i, expectedRow := range expectedCells {
		for j, expectedText := range expectedRow {
			if i >= len(tableShape.TableInfo.Cells) || j >= len(tableShape.TableInfo.Cells[i]) {
				t.Errorf("cell [%d][%d] not found", i, j)
				continue
			}
			if tableShape.TableInfo.Cells[i][j] != expectedText {
				t.Errorf("cell [%d][%d]: expected '%s', got '%s'", i, j, expectedText, tableShape.TableInfo.Cells[i][j])
			}
		}
	}
}
