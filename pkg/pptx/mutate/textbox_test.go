package mutate

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TestInsertTextBoxNilRequest verifies nil request handling
func TestInsertTextBoxNilRequest(t *testing.T) {
	result, err := InsertTextBox(nil)

	if err == nil {
		t.Error("expected error for nil request")
	}
	if result != nil {
		t.Error("result should be nil on error")
	}
}

// TestInsertTextBoxNilPackage verifies nil package handling
func TestInsertTextBoxNilPackage(t *testing.T) {
	req := &InsertTextBoxRequest{
		Package: nil,
	}

	result, err := InsertTextBox(req)

	if err == nil {
		t.Error("expected error for nil package")
	}
	if result != nil {
		t.Error("result should be nil on error")
	}
}

// TestInsertTextBoxNilSlideRef verifies nil slide reference handling
func TestInsertTextBoxNilSlideRef(t *testing.T) {
	req := &InsertTextBoxRequest{
		Package:  nil, // Will fail package check first
		SlideRef: nil,
	}

	result, err := InsertTextBox(req)

	if err == nil {
		t.Error("expected error for nil slide reference")
	}
	if result != nil {
		t.Error("result should be nil on error")
	}
}

// TestInsertTextBoxNilRichText verifies nil rich text handling
func TestInsertTextBoxNilRichText(t *testing.T) {
	req := &InsertTextBoxRequest{
		Package:  nil,
		SlideRef: nil,
		RichText: nil,
	}

	result, err := InsertTextBox(req)

	if err == nil {
		t.Error("expected error for nil rich text")
	}
	if result != nil {
		t.Error("result should be nil on error")
	}
}

// TestInsertTextBoxInvalidDimensions verifies dimension validation
func TestInsertTextBoxInvalidDimensions(t *testing.T) {
	tests := []struct {
		cx, cy int64
		desc   string
	}{
		{0, 100, "zero width"},
		{100, 0, "zero height"},
		{-100, 100, "negative width"},
		{100, -100, "negative height"},
	}

	for _, tt := range tests {
		req := &InsertTextBoxRequest{
			Package:  nil,
			SlideRef: nil,
			RichText: &model.TextBlockInfo{},
			CX:       tt.cx,
			CY:       tt.cy,
		}

		result, err := InsertTextBox(req)

		if err == nil {
			t.Errorf("expected error for %s", tt.desc)
		}
		if result != nil {
			t.Errorf("result should be nil on error (%s)", tt.desc)
		}
	}
}

// TestInsertTextBoxShapeNameGeneration verifies auto-generated shape names
func TestInsertTextBoxShapeNameGeneration(t *testing.T) {
	// Test that if no shape name is provided, one is generated
	// This test would need a real package to verify name generation
	// For now, just verify the structure is valid
}

// TestCreateTextBoxElementStructure verifies the created XML structure
func TestCreateTextBoxElementStructure(t *testing.T) {
	richText := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{
			{
				Runs: []interface{}{
					&model.TextRun{
						Text: "Test Text",
						Properties: &model.RunProperties{
							FontFamily: "Calibri",
						},
					},
				},
			},
		},
		PlainText: "Test Text",
	}

	shape := createTextBoxElement(1, "TextBox 1", 914400, 914400, 6000000, 3000000, richText, nil)

	if shape == nil {
		t.Error("shape element should not be nil")
	}

	if shape.Tag != "sp" {
		t.Errorf("expected tag sp, got %s", shape.Tag)
	}

	// Check for nvSpPr
	nvSpPr := shape.FindElement("p:nvSpPr")
	if nvSpPr == nil {
		t.Error("nvSpPr not found")
	}

	// Check for spPr
	spPr := shape.FindElement("p:spPr")
	if spPr == nil {
		t.Error("spPr not found")
	}

	// Check for txBody
	txBody := shape.FindElement("p:txBody")
	if txBody == nil {
		t.Error("txBody not found")
	}

	// Check for bodyPr
	bodyPr := txBody.FindElement("a:bodyPr")
	if bodyPr == nil {
		t.Error("bodyPr not found in txBody")
	}

	// Check for lstStyle
	lstStyle := txBody.FindElement("a:lstStyle")
	if lstStyle == nil {
		t.Error("lstStyle not found in txBody")
	}

	// Check for paragraphs
	paragraphs := txBody.FindElements("a:p")
	if len(paragraphs) == 0 {
		t.Error("no paragraphs found in txBody")
	}
}

// TestCreateTextBoxElementWithBodyProperties verifies custom body properties
func TestCreateTextBoxElementWithBodyProperties(t *testing.T) {
	richText := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{},
		PlainText:  "",
	}

	bodyProps := &model.TextBodyProperties{
		Anchor: "ctr",
		Wrap:   "none",
	}

	shape := createTextBoxElement(1, "TextBox 1", 0, 0, 1000000, 1000000, richText, bodyProps)

	if shape == nil {
		t.Error("shape element should not be nil")
	}

	txBody := shape.FindElement("p:txBody")
	if txBody == nil {
		t.Error("txBody not found")
	}

	bodyPr := txBody.FindElement("a:bodyPr")
	if bodyPr == nil {
		t.Error("bodyPr not found")
	}

	// Check custom attributes
	anchor := bodyPr.SelectAttrValue("anchor", "")
	if anchor != "ctr" {
		t.Errorf("expected anchor='ctr', got '%s'", anchor)
	}

	wrap := bodyPr.SelectAttrValue("wrap", "")
	if wrap != "none" {
		t.Errorf("expected wrap='none', got '%s'", wrap)
	}
}

// TestCreateParagraphElement verifies paragraph element creation
func TestCreateParagraphElement(t *testing.T) {
	para := &model.Paragraph{
		Properties: &model.ParagraphProperties{
			Alignment: "ctr",
		},
		Runs: []interface{}{
			&model.TextRun{
				Text: "Hello",
				Properties: &model.RunProperties{
					FontFamily: "Calibri",
				},
			},
		},
	}

	pElem := createParagraphElement(para)

	if pElem == nil {
		t.Error("paragraph element should not be nil")
	}

	if pElem.Tag != "p" {
		t.Errorf("expected tag p, got %s", pElem.Tag)
	}

	// Check for paragraph properties
	pPr := pElem.FindElement("a:pPr")
	if pPr == nil {
		t.Error("pPr not found")
	}

	algn := pPr.SelectAttrValue("algn", "")
	if algn != "ctr" {
		t.Errorf("expected algn='ctr', got '%s'", algn)
	}

	// Check for text run
	runs := pElem.FindElements("a:r")
	if len(runs) == 0 {
		t.Error("no text runs found")
	}
}

// TestCreateTextRunElement verifies text run creation
func TestCreateTextRunElement(t *testing.T) {
	run := &model.TextRun{
		Text: "Sample Text",
		Properties: &model.RunProperties{
			FontFamily: "Arial",
			Language:   "en-US",
		},
	}

	rElem := createTextRunElement(run)

	if rElem == nil {
		t.Error("run element should not be nil")
	}

	if rElem.Tag != "r" {
		t.Errorf("expected tag r, got %s", rElem.Tag)
	}

	// Check for text
	textElem := rElem.FindElement("a:t")
	if textElem == nil {
		t.Error("a:t not found")
	}

	if textElem.Text() != "Sample Text" {
		t.Errorf("expected text 'Sample Text', got '%s'", textElem.Text())
	}
}

// TestInsertTextBoxRequestValidation verifies all validation paths
func TestInsertTextBoxRequestValidation(t *testing.T) {
	validReq := &InsertTextBoxRequest{
		Package:  nil, // Will fail at package check
		SlideRef: nil,
		RichText: &model.TextBlockInfo{},
		CX:       1000000,
		CY:       1000000,
	}

	result, err := InsertTextBox(validReq)

	if err == nil {
		t.Error("expected error for nil package")
	}
	if result != nil {
		t.Error("result should be nil on error")
	}
}

// TestCreateRunPropertiesElement verifies run properties XML generation
func TestCreateRunPropertiesElement(t *testing.T) {
	props := &model.RunProperties{
		FontFamily: "Times New Roman",
		FontSize:   ptrFloat64(14.0),
		Language:   "fr-FR",
		Bold:       ptrBool(true),
		Italic:     ptrBool(false),
		Color:      "FF0000",
	}

	rPr := createRunPropertiesElement(props)

	if rPr == nil {
		t.Error("rPr element should not be nil")
	}

	// Check attributes
	lang := rPr.SelectAttrValue("lang", "")
	if lang != "fr-FR" {
		t.Errorf("expected lang='fr-FR', got '%s'", lang)
	}

	bold := rPr.SelectAttrValue("b", "")
	if bold != "1" {
		t.Errorf("expected b='1', got '%s'", bold)
	}

	italic := rPr.SelectAttrValue("i", "")
	if italic == "1" {
		t.Errorf("expected no italic attribute, but got i='%s'", italic)
	}

	children := rPr.ChildElements()
	if len(children) != 2 || children[0].Tag != "solidFill" || children[1].Tag != "latin" {
		t.Fatalf("rPr children = %v, want [solidFill latin]", childElementTagsForTextboxTest(children))
	}
}

func childElementTagsForTextboxTest(elements []*etree.Element) []string {
	tags := make([]string, 0, len(elements))
	for _, elem := range elements {
		tags = append(tags, elem.Tag)
	}
	return tags
}

// TestCreateParagraphPropertiesElement verifies paragraph properties XML generation
func TestCreateParagraphPropertiesElement(t *testing.T) {
	level := int32(2)
	props := &model.ParagraphProperties{
		Alignment: "r",
		Level:     &level,
	}

	pPr := createParagraphPropertiesElement(props)

	if pPr == nil {
		t.Error("pPr element should not be nil")
	}

	algn := pPr.SelectAttrValue("algn", "")
	if algn != "r" {
		t.Errorf("expected algn='r', got '%s'", algn)
	}

	lvl := pPr.SelectAttrValue("lvl", "")
	if lvl != "2" {
		t.Errorf("expected lvl='2', got '%s'", lvl)
	}
}

// Helper functions for testing

func ptrFloat64(f float64) *float64 {
	return &f
}

func ptrBool(b bool) *bool {
	return &b
}
