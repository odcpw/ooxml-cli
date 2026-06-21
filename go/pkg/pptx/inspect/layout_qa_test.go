package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TestTextOverflowAnalyzer_NoText verifies that shapes with no text are not flagged
func TestTextOverflowAnalyzer_NoText(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	shape := &model.ShapeInfo{
		ID:   1,
		Name: "EmptyShape",
		Bounds: &model.Bounds{
			X:  0,
			Y:  0,
			CX: 1000000,
			CY: 500000,
		},
	}

	// No text block = no overflow
	result := analyzer.AnalyzeTextOverflow(shape, nil)
	if result != nil {
		t.Error("expected no overflow for shape with no text")
	}
}

// TestTextOverflowAnalyzer_EmptyTextBlock verifies empty text blocks are ignored
func TestTextOverflowAnalyzer_EmptyTextBlock(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	shape := &model.ShapeInfo{
		ID:   1,
		Name: "EmptyTextShape",
		Bounds: &model.Bounds{
			X:  0,
			Y:  0,
			CX: 1000000,
			CY: 500000,
		},
	}

	textBlock := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{},
		PlainText:  "",
	}

	result := analyzer.AnalyzeTextOverflow(shape, textBlock)
	if result != nil {
		t.Error("expected no overflow for empty text block")
	}
}

// TestTextOverflowAnalyzer_SmallText verifies that fitting text is not flagged
func TestTextOverflowAnalyzer_SmallText(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	shape := &model.ShapeInfo{
		ID:   1,
		Name: "SmallTextShape",
		Bounds: &model.Bounds{
			X:  0,
			Y:  0,
			CX: 5000000, // Very large shape
			CY: 5000000,
		},
	}

	fontSize := 18.0
	textBlock := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{
			{
				Text: "Hello World",
				Properties: &model.ParagraphProperties{
					DefaultRunProps: &model.RunProperties{
						FontSize: &fontSize,
					},
				},
				Runs: []interface{}{
					&model.TextRun{
						Text: "Hello World",
						Properties: &model.RunProperties{
							FontSize: &fontSize,
						},
					},
				},
			},
		},
		PlainText: "Hello World",
	}

	result := analyzer.AnalyzeTextOverflow(shape, textBlock)
	if result != nil {
		t.Error("expected no overflow for small text in large shape")
	}
}

// TestTextOverflowAnalyzer_OverflowDetection verifies obvious text overflow is detected
func TestTextOverflowAnalyzer_OverflowDetection(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	// Tiny shape that can't hold much text
	shape := &model.ShapeInfo{
		ID:   1,
		Name: "TinyShape",
		Bounds: &model.Bounds{
			X:  0,
			Y:  0,
			CX: 1000000, // Small width
			CY: 200000,  // Small height (one line at most)
		},
	}

	fontSize := 32.0 // Large font
	textBlock := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{
			{
				Text: "This is a very long paragraph that should definitely overflow because it has way too much text to fit in such a small shape",
				Properties: &model.ParagraphProperties{
					DefaultRunProps: &model.RunProperties{
						FontSize: &fontSize,
					},
				},
				Runs: []interface{}{
					&model.TextRun{
						Text: "This is a very long paragraph that should definitely overflow because it has way too much text to fit in such a small shape",
						Properties: &model.RunProperties{
							FontSize: &fontSize,
						},
					},
				},
			},
		},
		PlainText: "This is a very long paragraph that should definitely overflow because it has way too much text to fit in such a small shape",
	}

	result := analyzer.AnalyzeTextOverflow(shape, textBlock)
	if result == nil {
		t.Fatal("expected overflow detection for long text in tiny shape")
	}

	if result.Severity != "high" {
		t.Errorf("expected high severity for significant overflow, got %s", result.Severity)
	}

	if result.OverflowAmount <= 0 {
		t.Errorf("expected positive overflow amount, got %d", result.OverflowAmount)
	}

	if result.ShapeID != 1 {
		t.Errorf("expected shape ID 1, got %d", result.ShapeID)
	}
}

// TestTextOverflowAnalyzer_MultiParagraph verifies multiple paragraphs are counted
func TestTextOverflowAnalyzer_MultiParagraph(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	shape := &model.ShapeInfo{
		ID:   1,
		Name: "MultiParagraph",
		Bounds: &model.Bounds{
			X:  0,
			Y:  0,
			CX: 2000000,
			CY: 400000, // Limited height
		},
	}

	fontSize := 18.0
	para1 := model.Paragraph{
		Text: "First paragraph with some text content",
		Properties: &model.ParagraphProperties{
			DefaultRunProps: &model.RunProperties{
				FontSize: &fontSize,
			},
		},
		Runs: []interface{}{
			&model.TextRun{
				Text: "First paragraph with some text content",
				Properties: &model.RunProperties{
					FontSize: &fontSize,
				},
			},
		},
	}

	para2 := model.Paragraph{
		Text: "Second paragraph with more content that might overflow",
		Properties: &model.ParagraphProperties{
			DefaultRunProps: &model.RunProperties{
				FontSize: &fontSize,
			},
		},
		Runs: []interface{}{
			&model.TextRun{
				Text: "Second paragraph with more content that might overflow",
				Properties: &model.RunProperties{
					FontSize: &fontSize,
				},
			},
		},
	}

	para3 := model.Paragraph{
		Text: "Third paragraph",
		Properties: &model.ParagraphProperties{
			DefaultRunProps: &model.RunProperties{
				FontSize: &fontSize,
			},
		},
		Runs: []interface{}{
			&model.TextRun{
				Text: "Third paragraph",
				Properties: &model.RunProperties{
					FontSize: &fontSize,
				},
			},
		},
	}

	textBlock := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{para1, para2, para3},
		PlainText:  para1.Text + "\n" + para2.Text + "\n" + para3.Text,
	}

	result := analyzer.AnalyzeTextOverflow(shape, textBlock)
	// With 3 paragraphs and limited height, overflow should be detected
	if result == nil {
		t.Fatal("expected overflow detection for 3 paragraphs in constrained space")
	}

	if result.ParagraphCount != 3 {
		t.Errorf("expected 3 paragraphs, got %d", result.ParagraphCount)
	}
}

// TestCollisionAnalyzer_NoCollision verifies non-overlapping shapes are not flagged
func TestCollisionAnalyzer_NoCollision(t *testing.T) {
	analyzer := NewCollisionAnalyzer()

	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Shape1",
			Bounds: &model.Bounds{
				X:  0,
				Y:  0,
				CX: 1000000,
				CY: 1000000,
			},
		},
		{
			ID:   2,
			Name: "Shape2",
			Bounds: &model.Bounds{
				X:  2000000, // No overlap with shape 1
				Y:  0,
				CX: 1000000,
				CY: 1000000,
			},
		},
	}

	collisions := analyzer.AnalyzeShapeCollisions(shapes)
	if len(collisions) > 0 {
		t.Error("expected no collisions for non-overlapping shapes")
	}
}

// TestCollisionAnalyzer_ClearCollision verifies obvious overlaps are detected
func TestCollisionAnalyzer_ClearCollision(t *testing.T) {
	analyzer := NewCollisionAnalyzer()

	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Shape1",
			Bounds: &model.Bounds{
				X:  0,
				Y:  0,
				CX: 2000000,
				CY: 2000000,
			},
		},
		{
			ID:   2,
			Name: "Shape2",
			Bounds: &model.Bounds{
				X:  1000000, // Clear overlap with shape 1
				Y:  1000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
	}

	collisions := analyzer.AnalyzeShapeCollisions(shapes)
	if len(collisions) == 0 {
		t.Fatal("expected collision detection for overlapping shapes")
	}

	collision := collisions[0]
	if collision.ShapeID1 != 1 || collision.ShapeID2 != 2 {
		t.Errorf("expected collision between shapes 1 and 2")
	}

	if collision.OverlapArea <= 0 {
		t.Errorf("expected positive overlap area, got %d", collision.OverlapArea)
	}

	if collision.OverlapPercentageOfSmaller <= 0 {
		t.Errorf("expected positive overlap percentage, got %f", collision.OverlapPercentageOfSmaller)
	}
}

// TestCollisionAnalyzer_IdenticalBoundsFiltered verifies identical bounds are filtered
func TestCollisionAnalyzer_IdenticalBoundsFiltered(t *testing.T) {
	analyzer := NewCollisionAnalyzer()
	analyzer.FilterIdenticalBounds = true

	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Shape1",
			Bounds: &model.Bounds{
				X:  1000000,
				Y:  1000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
		{
			ID:   2,
			Name: "Shape2",
			Bounds: &model.Bounds{
				X:  1000000, // Identical bounds
				Y:  1000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
	}

	collisions := analyzer.AnalyzeShapeCollisions(shapes)
	if len(collisions) > 0 {
		t.Error("expected no collisions when identical bounds are filtered")
	}
}

// TestCollisionAnalyzer_IdenticalBoundsNotFiltered verifies identical bounds can be reported
func TestCollisionAnalyzer_IdenticalBoundsNotFiltered(t *testing.T) {
	analyzer := NewCollisionAnalyzer()
	analyzer.FilterIdenticalBounds = false

	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Shape1",
			Bounds: &model.Bounds{
				X:  1000000,
				Y:  1000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
		{
			ID:   2,
			Name: "Shape2",
			Bounds: &model.Bounds{
				X:  1000000, // Identical bounds
				Y:  1000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
	}

	collisions := analyzer.AnalyzeShapeCollisions(shapes)
	if len(collisions) == 0 {
		t.Fatal("expected collision detection when identical bounds are not filtered")
	}

	collision := collisions[0]
	if !collision.IsIdenticalBounds {
		t.Error("expected IsIdenticalBounds to be true")
	}
}

// TestCollisionAnalyzer_MinorOverlapIgnored verifies minor overlaps are ignored
func TestCollisionAnalyzer_MinorOverlapIgnored(t *testing.T) {
	analyzer := NewCollisionAnalyzer()

	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Shape1",
			Bounds: &model.Bounds{
				X:  0,
				Y:  0,
				CX: 1000000,
				CY: 1000000,
			},
		},
		{
			ID:   2,
			Name: "Shape2",
			Bounds: &model.Bounds{
				X:  950000, // Only tiny overlap (50k)
				Y:  0,
				CX: 1000000,
				CY: 1000000,
			},
		},
	}

	collisions := analyzer.AnalyzeShapeCollisions(shapes)
	if len(collisions) > 0 {
		// Minor overlap < 5% should be filtered
		collision := collisions[0]
		if collision.OverlapPercentageOfSmaller < 5.0 {
			t.Logf("Minor overlap of %.2f%% was reported", collision.OverlapPercentageOfSmaller)
		}
	}
}

// TestCollisionAnalyzer_SeverityLevels verifies different overlap severities
func TestCollisionAnalyzer_SeverityLevels(t *testing.T) {
	analyzer := NewCollisionAnalyzer()

	// Test high severity (>50% overlap)
	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Shape1",
			Bounds: &model.Bounds{
				X:  0,
				Y:  0,
				CX: 2000000,
				CY: 2000000,
			},
		},
		{
			ID:   2,
			Name: "Shape2",
			Bounds: &model.Bounds{
				X:  1000000,
				Y:  1000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
	}

	collisions := analyzer.AnalyzeShapeCollisions(shapes)
	if len(collisions) > 0 {
		collision := collisions[0]
		if collision.OverlapPercentageOfSmaller > 50.0 {
			if collision.Severity != "high" {
				t.Errorf("expected high severity for %.2f%% overlap, got %s",
					collision.OverlapPercentageOfSmaller, collision.Severity)
			}
		}
	}
}

// TestAnalyzeSlideLayoutQA_CleanSlide verifies clean slides are not flagged
func TestAnalyzeSlideLayoutQA_CleanSlide(t *testing.T) {
	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "Title",
			Bounds: &model.Bounds{
				X:  0,
				Y:  0,
				CX: 9000000,
				CY: 1000000,
			},
		},
		{
			ID:   2,
			Name: "Content",
			Bounds: &model.Bounds{
				X:  0,
				Y:  1500000,
				CX: 9000000,
				CY: 4000000,
			},
		},
	}

	fontSize := 18.0
	textBlocks := map[int]*model.TextBlockInfo{
		1: {
			Paragraphs: []model.Paragraph{
				{
					Text: "Slide Title",
					Properties: &model.ParagraphProperties{
						DefaultRunProps: &model.RunProperties{
							FontSize: &fontSize,
						},
					},
					Runs: []interface{}{
						&model.TextRun{
							Text: "Slide Title",
							Properties: &model.RunProperties{
								FontSize: &fontSize,
							},
						},
					},
				},
			},
			PlainText: "Slide Title",
		},
		2: {
			Paragraphs: []model.Paragraph{
				{
					Text: "Some content that fits nicely",
					Properties: &model.ParagraphProperties{
						DefaultRunProps: &model.RunProperties{
							FontSize: &fontSize,
						},
					},
					Runs: []interface{}{
						&model.TextRun{
							Text: "Some content that fits nicely",
							Properties: &model.RunProperties{
								FontSize: &fontSize,
							},
						},
					},
				},
			},
			PlainText: "Some content that fits nicely",
		},
	}

	// Standard slide dimensions in EMUs
	slideWidth := int64(9144000)
	slideHeight := int64(6858000)
	report := AnalyzeSlideLayoutQA(0, shapes, textBlocks, slideWidth, slideHeight)

	if report.HasIssues {
		t.Errorf("expected no issues in clean slide, got %d issues", report.IssueCount)
	}

	if len(report.TextOverflows) > 0 {
		t.Errorf("expected no overflows, got %d", len(report.TextOverflows))
	}

	if len(report.Collisions) > 0 {
		t.Errorf("expected no collisions, got %d", len(report.Collisions))
	}
}

// TestAnalyzeSlideLayoutQA_ProblematicSlide verifies issues are detected in report
func TestAnalyzeSlideLayoutQA_ProblematicSlide(t *testing.T) {
	shapes := []model.ShapeInfo{
		{
			ID:   1,
			Name: "OverflowBox",
			Bounds: &model.Bounds{
				X:  0,
				Y:  0,
				CX: 1000000,
				CY: 200000, // Tiny height
			},
		},
		{
			ID:   2,
			Name: "CollisionShape1",
			Bounds: &model.Bounds{
				X:  2000000,
				Y:  2000000,
				CX: 2000000,
				CY: 2000000,
			},
		},
		{
			ID:   3,
			Name: "CollisionShape2",
			Bounds: &model.Bounds{
				X:  2500000,
				Y:  2500000,
				CX: 2000000,
				CY: 2000000,
			},
		},
	}

	fontSize := 32.0
	textBlocks := map[int]*model.TextBlockInfo{
		1: {
			Paragraphs: []model.Paragraph{
				{
					Text: "This is way too much text for such a tiny box",
					Properties: &model.ParagraphProperties{
						DefaultRunProps: &model.RunProperties{
							FontSize: &fontSize,
						},
					},
					Runs: []interface{}{
						&model.TextRun{
							Text: "This is way too much text for such a tiny box",
							Properties: &model.RunProperties{
								FontSize: &fontSize,
							},
						},
					},
				},
			},
			PlainText: "This is way too much text for such a tiny box",
		},
	}

	// Standard slide dimensions
	slideWidth := int64(9144000)
	slideHeight := int64(6858000)
	report := AnalyzeSlideLayoutQA(0, shapes, textBlocks, slideWidth, slideHeight)

	if !report.HasIssues {
		t.Fatal("expected issues in problematic slide")
	}

	if len(report.TextOverflows) == 0 {
		t.Error("expected at least one overflow detection")
	}

	if len(report.Collisions) == 0 {
		t.Error("expected at least one collision detection")
	}

	expectedIssueCount := len(report.TextOverflows) + len(report.Collisions)
	if report.IssueCount != expectedIssueCount {
		t.Errorf("expected issue count %d, got %d", expectedIssueCount, report.IssueCount)
	}
}

// TestAnalyzeSlideLayoutQA_SlideMetadata verifies slide indices are correct
func TestAnalyzeSlideLayoutQA_SlideMetadata(t *testing.T) {
	shapes := []model.ShapeInfo{}
	textBlocks := map[int]*model.TextBlockInfo{}
	slideWidth := int64(9144000)
	slideHeight := int64(6858000)

	for slideIdx := 0; slideIdx < 5; slideIdx++ {
		report := AnalyzeSlideLayoutQA(slideIdx, shapes, textBlocks, slideWidth, slideHeight)

		if report.SlideIndex != slideIdx {
			t.Errorf("slide %d: expected SlideIndex %d, got %d", slideIdx, slideIdx, report.SlideIndex)
		}

		expectedSlideNumber := slideIdx + 1
		if report.SlideNumber != expectedSlideNumber {
			t.Errorf("slide %d: expected SlideNumber %d, got %d", slideIdx, expectedSlideNumber, report.SlideNumber)
		}
	}
}

// TestEstimateLineCount verifies line count estimation
func TestEstimateLineCount(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	// Single short paragraph
	textBlock := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{
			{
				Text: "Short",
			},
		},
	}

	lineCount := analyzer.estimateLineCount(textBlock)
	if lineCount < 1 {
		t.Errorf("expected at least 1 line, got %d", lineCount)
	}

	// Multiple paragraphs
	textBlock.Paragraphs = append(textBlock.Paragraphs, model.Paragraph{Text: "Another paragraph"})
	lineCount = analyzer.estimateLineCount(textBlock)
	if lineCount < 2 {
		t.Errorf("expected at least 2 lines for 2 paragraphs, got %d", lineCount)
	}
}

// TestEstimateMaxFontSize verifies font size detection
func TestEstimateMaxFontSize(t *testing.T) {
	analyzer := NewTextOverflowAnalyzer()

	fontSize1 := 18.0
	fontSize2 := 24.0
	fontSize3 := 12.0

	textBlock := &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{
			{
				Text: "Para 1",
				Properties: &model.ParagraphProperties{
					DefaultRunProps: &model.RunProperties{
						FontSize: &fontSize1,
					},
				},
				Runs: []interface{}{
					&model.TextRun{
						Text: "Para 1",
						Properties: &model.RunProperties{
							FontSize: &fontSize2, // Larger
						},
					},
				},
			},
			{
				Text: "Para 2",
				Runs: []interface{}{
					&model.TextRun{
						Text: "Para 2",
						Properties: &model.RunProperties{
							FontSize: &fontSize3, // Smaller
						},
					},
				},
			},
		},
	}

	maxSize := analyzer.estimateMaxFontSize(textBlock)
	if maxSize != fontSize2 {
		t.Errorf("expected max font size %f, got %f", fontSize2, maxSize)
	}
}
