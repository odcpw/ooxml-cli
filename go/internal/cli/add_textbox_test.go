package cli

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
)

// TestBuildRichTextFromFlags verifies rich text building from CLI flags
func TestBuildRichTextFromFlags(t *testing.T) {
	// Save original values
	oldText := addTextboxText
	oldFontSize := addTextboxFontSize
	oldFontFamily := addTextboxFontFamily
	oldBold := addTextboxBold
	oldItalic := addTextboxItalic

	defer func() {
		addTextboxText = oldText
		addTextboxFontSize = oldFontSize
		addTextboxFontFamily = oldFontFamily
		addTextboxBold = oldBold
		addTextboxItalic = oldItalic
	}()

	// Set test values
	addTextboxText = "Test Text"
	addTextboxFontSize = 24
	addTextboxFontFamily = "Arial"
	addTextboxBold = true
	addTextboxItalic = false

	richText := buildRichTextFromFlags()

	if richText == nil {
		t.Error("richText should not be nil")
	}

	if richText.PlainText != "Test Text" {
		t.Errorf("expected PlainText='Test Text', got '%s'", richText.PlainText)
	}

	if len(richText.Paragraphs) == 0 {
		t.Error("expected at least one paragraph")
	}

	para := richText.Paragraphs[0]
	if len(para.Runs) == 0 {
		t.Error("expected at least one run in paragraph")
	}

	run := para.Runs[0]
	textRun, ok := run.(*model.TextRun)
	if !ok {
		t.Error("run should be a TextRun")
	}

	if textRun.Text != "Test Text" {
		t.Errorf("expected Text='Test Text', got '%s'", textRun.Text)
	}

	if textRun.Properties == nil {
		t.Error("run properties should not be nil")
	}

	if textRun.Properties.FontFamily != "Arial" {
		t.Errorf("expected FontFamily='Arial', got '%s'", textRun.Properties.FontFamily)
	}

	if textRun.Properties.FontSize == nil || *textRun.Properties.FontSize != 24 {
		t.Error("expected FontSize=24")
	}

	if textRun.Properties.Bold == nil || !*textRun.Properties.Bold {
		t.Error("expected Bold=true")
	}

	if textRun.Properties.Italic == nil || *textRun.Properties.Italic {
		t.Error("expected Italic=false")
	}
}

// TestBuildRichTextFromFlagsWithLevel verifies paragraph level is applied
func TestBuildRichTextFromFlagsWithLevel(t *testing.T) {
	oldLevel := addTextboxLevel
	oldAlign := addTextboxAlign

	defer func() {
		addTextboxLevel = oldLevel
		addTextboxAlign = oldAlign
	}()

	addTextboxText = "Indented text"
	addTextboxLevel = 2
	addTextboxAlign = "ctr"

	richText := buildRichTextFromFlags()

	if len(richText.Paragraphs) == 0 {
		t.Error("expected at least one paragraph")
	}

	para := richText.Paragraphs[0]
	if para.Properties == nil {
		t.Error("paragraph properties should not be nil")
	}

	if para.Properties.Level == nil || *para.Properties.Level != 2 {
		t.Error("expected Level=2")
	}

	if para.Properties.Alignment != "ctr" {
		t.Errorf("expected Alignment='ctr', got '%s'", para.Properties.Alignment)
	}
}

// TestApplyFlagsToRunProperties verifies flag application to run properties
func TestApplyFlagsToRunProperties(t *testing.T) {
	oldFontSize := addTextboxFontSize
	oldFontFamily := addTextboxFontFamily
	oldBold := addTextboxBold
	oldItalic := addTextboxItalic
	oldColor := addTextboxColor

	defer func() {
		addTextboxFontSize = oldFontSize
		addTextboxFontFamily = oldFontFamily
		addTextboxBold = oldBold
		addTextboxItalic = oldItalic
		addTextboxColor = oldColor
	}()

	addTextboxFontSize = 20
	addTextboxFontFamily = "Times New Roman"
	addTextboxBold = true
	addTextboxItalic = true
	addTextboxColor = "FF0000"

	props := &model.RunProperties{}
	applyFlagsToRunProperties(props)

	if props.FontSize == nil || *props.FontSize != 20 {
		t.Error("expected FontSize=20")
	}

	if props.FontFamily != "Times New Roman" {
		t.Errorf("expected FontFamily='Times New Roman', got '%s'", props.FontFamily)
	}

	if props.Bold == nil || !*props.Bold {
		t.Error("expected Bold=true")
	}

	if props.Italic == nil || !*props.Italic {
		t.Error("expected Italic=true")
	}

	if props.Color != "FF0000" {
		t.Errorf("expected Color='FF0000', got '%s'", props.Color)
	}
}

// TestApplyFlagsToRunPropertiesPartial verifies partial flag application
func TestApplyFlagsToRunPropertiesPartial(t *testing.T) {
	oldFontSize := addTextboxFontSize
	oldFontFamily := addTextboxFontFamily
	oldBold := addTextboxBold
	oldColor := addTextboxColor

	defer func() {
		addTextboxFontSize = oldFontSize
		addTextboxFontFamily = oldFontFamily
		addTextboxBold = oldBold
		addTextboxColor = oldColor
	}()

	addTextboxFontSize = 0 // Will be ignored
	addTextboxFontFamily = ""
	addTextboxBold = false // Will be ignored
	addTextboxColor = "00FF00"

	props := &model.RunProperties{}
	applyFlagsToRunProperties(props)

	// Only non-default flags should be applied
	if props.FontSize != nil {
		t.Error("FontSize should not be set when flag is 0")
	}

	if props.FontFamily != "" {
		t.Error("FontFamily should not be set when flag is empty")
	}

	if props.Bold != nil {
		t.Error("Bold should not be set when flag is false")
	}

	if props.Color != "00FF00" {
		t.Errorf("expected Color='00FF00', got '%s'", props.Color)
	}
}

// TestBuildRichTextDefaults verifies default values
func TestBuildRichTextDefaults(t *testing.T) {
	oldText := addTextboxText
	oldFontSize := addTextboxFontSize
	oldFontFamily := addTextboxFontFamily

	defer func() {
		addTextboxText = oldText
		addTextboxFontSize = oldFontSize
		addTextboxFontFamily = oldFontFamily
	}()

	addTextboxText = "Simple text"
	addTextboxFontSize = 18 // Default
	addTextboxFontFamily = "Calibri"

	richText := buildRichTextFromFlags()

	para := richText.Paragraphs[0]
	run := para.Runs[0].(*model.TextRun)

	if run.Properties.FontSize == nil || *run.Properties.FontSize != 18 {
		t.Error("expected default FontSize=18")
	}

	if run.Properties.FontFamily != "Calibri" {
		t.Error("expected default FontFamily='Calibri'")
	}

	if run.Properties.Language != "en-US" {
		t.Errorf("expected Language='en-US', got '%s'", run.Properties.Language)
	}
}

// TestInsertTextBoxRequestStructure verifies the request structure
func TestInsertTextBoxRequestStructure(t *testing.T) {
	richText := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{},
		PlainText:  "Test",
	}

	// Just verify the struct can be created without runtime errors
	req := &mutate.InsertTextBoxRequest{
		Package:   nil,
		SlideRef:  nil,
		RichText:  richText,
		X:         1000000,
		Y:         1000000,
		CX:        5000000,
		CY:        3000000,
		ShapeName: "TestBox",
	}

	if req == nil {
		t.Error("request should not be nil")
	}

	if req.RichText != richText {
		t.Error("RichText not properly assigned")
	}

	if req.X != 1000000 || req.Y != 1000000 {
		t.Error("Position not properly assigned")
	}

	if req.CX != 5000000 || req.CY != 3000000 {
		t.Error("Size not properly assigned")
	}
}
