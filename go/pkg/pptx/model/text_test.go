package model

import (
	"encoding/json"
	"testing"
)

// TestTextBlockInfoBackwardCompatibility ensures that omitempty fields don't appear in JSON
// when they are nil/zero, maintaining backward compatibility with existing consumers
func TestTextBlockInfoBackwardCompatibility(t *testing.T) {
	// Create a minimal TextBlockInfo with only required fields
	info := &TextBlockInfo{
		Paragraphs: []Paragraph{
			{
				Runs: []interface{}{
					&TextRun{
						Type: "text",
						Text: "Hello",
					},
				},
				Text: "Hello",
			},
		},
		PlainText: "Hello",
	}

	// Marshal to JSON
	data, err := json.Marshal(info)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	// Unmarshal to check structure
	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	// Verify that omitempty fields don't appear
	if _, exists := result["bodyProperties"]; exists {
		t.Errorf("bodyProperties should not appear in JSON when nil")
	}

	// Verify required fields are present
	if _, exists := result["paragraphs"]; !exists {
		t.Errorf("paragraphs should appear in JSON")
	}
	if _, exists := result["plainText"]; !exists {
		t.Errorf("plainText should appear in JSON")
	}
}

// TestTextBodyPropertiesSerialization tests that TextBodyProperties serializes correctly
func TestTextBodyPropertiesSerialization(t *testing.T) {
	inset := int64(914400) // 0.1 inch in EMU
	autofit := "normal"

	props := &TextBodyProperties{
		Anchor:      "t",
		Wrap:        "square",
		LeftInset:   &inset,
		AutofitType: autofit,
	}

	data, err := json.Marshal(props)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["anchor"] != "t" {
		t.Errorf("expected anchor='t', got %v", result["anchor"])
	}
	if result["wrap"] != "square" {
		t.Errorf("expected wrap='square', got %v", result["wrap"])
	}
	if result["leftInset"] != float64(inset) {
		t.Errorf("expected leftInset=%d, got %v", inset, result["leftInset"])
	}

	// Verify nil fields don't appear
	if _, exists := result["topInset"]; exists {
		t.Errorf("topInset should not appear when nil")
	}
}

// TestParagraphPropertiesSerialization tests rich paragraph properties
func TestParagraphPropertiesSerialization(t *testing.T) {
	level := int32(1)
	spacing := int64(200000)

	props := &ParagraphProperties{
		Level:           &level,
		Alignment:       "l",
		SpaceAfter:      &spacing,
		BulletMode:      "char",
		BulletCharacter: "•",
	}

	data, err := json.Marshal(props)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["level"] != float64(level) {
		t.Errorf("expected level=%d, got %v", level, result["level"])
	}
	if result["bulletMode"] != "char" {
		t.Errorf("expected bulletMode='char', got %v", result["bulletMode"])
	}

	// Verify nil fields don't appear
	if _, exists := result["autoNumberingScheme"]; exists {
		t.Errorf("autoNumberingScheme should not appear when nil/empty")
	}
}

// TestRunPropertiesSerialization tests text run properties
func TestRunPropertiesSerialization(t *testing.T) {
	fontSize := 24.0 // 24pt
	bold := true

	props := &RunProperties{
		FontFamily: "Arial",
		FontSize:   &fontSize,
		Bold:       &bold,
		Italic:     nil,
		Color:      "FF0000",
		Language:   "en-US",
	}

	data, err := json.Marshal(props)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["fontFamily"] != "Arial" {
		t.Errorf("expected fontFamily='Arial', got %v", result["fontFamily"])
	}
	if result["bold"] != true {
		t.Errorf("expected bold=true, got %v", result["bold"])
	}
	if result["color"] != "FF0000" {
		t.Errorf("expected color='FF0000', got %v", result["color"])
	}

	// Verify nil/false values still appear for bool/string
	if _, exists := result["italic"]; exists && result["italic"] == nil {
		t.Errorf("italic should not appear in JSON when nil")
	}
}

// TestBreakSegmentSerialization tests line break segments
func TestBreakSegmentSerialization(t *testing.T) {
	brk := &Break{
		Type: "break",
	}

	data, err := json.Marshal(brk)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["type"] != "break" {
		t.Errorf("expected type='break', got %v", result["type"])
	}
}

// TestTabSegmentSerialization tests tab segments
func TestTabSegmentSerialization(t *testing.T) {
	tab := &Tab{
		Type: "tab",
	}

	data, err := json.Marshal(tab)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["type"] != "tab" {
		t.Errorf("expected type='tab', got %v", result["type"])
	}
}

// TestFieldSegmentSerialization tests field segments
func TestFieldSegmentSerialization(t *testing.T) {
	field := &Field{
		Type:      "field",
		ID:        "fld1",
		FieldType: "slidenum",
		Text:      "1",
	}

	data, err := json.Marshal(field)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["type"] != "field" {
		t.Errorf("expected type='field', got %v", result["type"])
	}
	if result["fieldType"] != "slidenum" {
		t.Errorf("expected fieldType='slidenum', got %v", result["fieldType"])
	}
}

// TestTextRunSerialization tests text run serialization
func TestTextRunSerialization(t *testing.T) {
	run := &TextRun{
		Type: "text",
		Text: "Bold text",
		Properties: &RunProperties{
			Bold: &[]bool{true}[0],
		},
	}

	data, err := json.Marshal(run)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	var result map[string]interface{}
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result["text"] != "Bold text" {
		t.Errorf("expected text='Bold text', got %v", result["text"])
	}

	if props, ok := result["properties"].(map[string]interface{}); ok {
		if props["bold"] != true {
			t.Errorf("expected properties.bold=true, got %v", props["bold"])
		}
	} else {
		t.Errorf("expected properties to be a map, got %T", result["properties"])
	}
}

// TestComplexTextBlockInfoSerialization tests a richly formatted text block
func TestComplexTextBlockInfoSerialization(t *testing.T) {
	fontSize := 24.0
	level := int32(1)
	spacing := int64(200000)
	bold := true

	info := &TextBlockInfo{
		PlainText: "Hello\nWorld",
		BodyProperties: &TextBodyProperties{
			Anchor: "t",
			Wrap:   "square",
		},
		Paragraphs: []Paragraph{
			{
				Text:  "Hello",
				Level: &level,
				Properties: &ParagraphProperties{
					Level:      &level,
					Alignment:  "l",
					SpaceAfter: &spacing,
				},
				Runs: []interface{}{
					&TextRun{
						Type: "text",
						Text: "Hello",
						Properties: &RunProperties{
							FontFamily: "Arial",
							FontSize:   &fontSize,
							Bold:       &bold,
						},
					},
				},
			},
			{
				Text: "World",
				Runs: []interface{}{
					&TextRun{
						Type: "text",
						Text: "World",
					},
				},
			},
		},
	}

	data, err := json.Marshal(info)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	// Verify it unmarshals back
	var result TextBlockInfo
	if err := json.Unmarshal(data, &result); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	if result.PlainText != "Hello\nWorld" {
		t.Errorf("expected plainText='Hello\\nWorld', got %q", result.PlainText)
	}

	if len(result.Paragraphs) != 2 {
		t.Errorf("expected 2 paragraphs, got %d", len(result.Paragraphs))
	}

	if result.BodyProperties == nil {
		t.Errorf("expected bodyProperties to be preserved")
	}
}

// TestRoundTripSerialization tests that data survives marshal/unmarshal cycle
func TestRoundTripSerialization(t *testing.T) {
	fontSize := 24.0
	level := int32(2)
	spacing := int64(300000)
	inset := int64(914400)

	original := &TextBlockInfo{
		PlainText: "Test paragraph",
		BodyProperties: &TextBodyProperties{
			Anchor:    "ctr",
			LeftInset: &inset,
		},
		Paragraphs: []Paragraph{
			{
				Text:  "Test paragraph",
				Level: &level,
				Properties: &ParagraphProperties{
					Level:           &level,
					Alignment:       "ctr",
					SpaceAfter:      &spacing,
					BulletMode:      "char",
					BulletCharacter: "•",
				},
				Runs: []interface{}{
					&TextRun{
						Type: "text",
						Text: "Test",
						Properties: &RunProperties{
							FontFamily: "Calibri",
							FontSize:   &fontSize,
						},
					},
					&Break{
						Type: "break",
					},
					&TextRun{
						Type: "text",
						Text: "paragraph",
					},
				},
			},
		},
	}

	// Marshal
	data, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	// Unmarshal
	var restored TextBlockInfo
	if err := json.Unmarshal(data, &restored); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	// Verify key fields
	if restored.PlainText != original.PlainText {
		t.Errorf("plainText not preserved")
	}

	if len(restored.Paragraphs) != len(original.Paragraphs) {
		t.Errorf("paragraph count mismatch")
	}

	// Re-marshal and compare
	data2, err := json.Marshal(&restored)
	if err != nil {
		t.Fatalf("failed to re-marshal: %v", err)
	}

	var restored2 TextBlockInfo
	if err := json.Unmarshal(data2, &restored2); err != nil {
		t.Fatalf("failed to unmarshal second time: %v", err)
	}

	if restored2.PlainText != original.PlainText {
		t.Errorf("plainText not preserved after second round-trip")
	}
}
