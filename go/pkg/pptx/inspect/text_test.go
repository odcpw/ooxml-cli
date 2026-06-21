package inspect

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestExtractTextBody_SingleParagraph(t *testing.T) {
	// Test with actual fixture file
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	// Find the first shape's text body
	sp := doc.FindElement(".//sp")
	if sp == nil {
		t.Fatal("sp not found")
	}

	txBody := sp.FindElement(".//txBody")
	if txBody == nil {
		t.Fatal("txBody not found")
	}

	text := ExtractTextBody(txBody)

	if text == nil {
		t.Fatal("expected non-nil TextBlockInfo")
	}

	if len(text.Paragraphs) < 1 {
		t.Fatalf("expected at least 1 paragraph, got %d", len(text.Paragraphs))
	}

	if len(text.PlainText) == 0 {
		t.Errorf("expected non-empty PlainText")
	}
}

func TestExtractTextBody_MultipleParagraphsFromMinimalTitle(t *testing.T) {
	// The minimal-title fixture has two shapes (title and subtitle)
	// Test that the subtitle has empty text
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	// Find all shapes
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)
	if len(shapes) < 2 {
		t.Fatalf("expected at least 2 shapes, got %d", len(shapes))
	}

	// The second shape (subtitle) should have empty text
	if len(shapes[1].Name) == 0 {
		t.Errorf("expected subtitle name, got empty string")
	}
}

func TestExtractTextBody_MinimalTitle(t *testing.T) {
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

	// Find the first shape's text body
	sp := doc.FindElement(".//sp")
	if sp == nil {
		t.Fatal("sp not found")
	}

	txBody := sp.FindElement(".//txBody")
	if txBody == nil {
		t.Fatal("txBody not found")
	}

	text := ExtractTextBody(txBody)

	if text == nil {
		t.Fatal("expected non-nil TextBlockInfo")
	}

	if len(text.Paragraphs) != 1 {
		t.Errorf("expected 1 paragraph, got %d", len(text.Paragraphs))
	}

	if !strings.Contains(text.PlainText, "Minimal Title Slide") {
		t.Errorf("expected 'Minimal Title Slide' in text, got '%s'", text.PlainText)
	}
}

func TestExtractTextBody_NilInput(t *testing.T) {
	text := ExtractTextBody(nil)

	if text == nil {
		t.Fatal("expected non-nil TextBlockInfo for nil input")
	}

	if len(text.Paragraphs) != 0 {
		t.Errorf("expected 0 paragraphs for nil input, got %d", len(text.Paragraphs))
	}

	if text.PlainText != "" {
		t.Errorf("expected empty PlainText for nil input, got '%s'", text.PlainText)
	}
}

func TestExtractTextBody_TableCells(t *testing.T) {
	// Test extracting text from table cells
	pkg, err := opc.Open("../../../testdata/pptx/table-slide/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open table-slide fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	// Find the graphic frame (table)
	graphicFrame := doc.FindElement(".//graphicFrame")
	if graphicFrame == nil {
		t.Fatal("graphicFrame not found")
	}

	// Extract text from the first cell
	txBody := graphicFrame.FindElement(".//tc//txBody")
	if txBody == nil {
		t.Fatal("txBody not found in first cell")
	}

	text := ExtractTextBody(txBody)
	if text == nil {
		t.Fatal("expected non-nil TextBlockInfo")
	}

	if len(text.Paragraphs) == 0 {
		t.Fatal("expected paragraphs in cell")
	}

	if !strings.Contains(text.PlainText, "R0C0") {
		t.Errorf("expected 'R0C0' in cell text, got '%s'", text.PlainText)
	}
}

func TestExtractTextBody_ParagraphPreservation(t *testing.T) {
	// Verify that empty paragraphs are preserved
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	// Find all shapes
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)
	if len(shapes) < 2 {
		t.Fatalf("expected at least 2 shapes, got %d", len(shapes))
	}

	// Get the subtitle's text body (should be mostly empty)
	// To do this, we need to find the second sp element directly
	allSPs := doc.FindElements(".//sp")
	if len(allSPs) < 2 {
		t.Fatalf("expected at least 2 sp elements, got %d", len(allSPs))
	}

	txBody := allSPs[1].FindElement(".//txBody")
	if txBody == nil {
		t.Fatal("subtitle txBody not found")
	}

	text := ExtractTextBody(txBody)
	if text == nil {
		t.Fatal("expected non-nil TextBlockInfo")
	}

	// Should have at least one paragraph
	if len(text.Paragraphs) == 0 {
		t.Fatal("expected at least one paragraph")
	}
}
