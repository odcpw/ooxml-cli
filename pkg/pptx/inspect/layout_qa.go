package inspect

import (
	"fmt"
	"math"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TextOverflowAnalyzer provides heuristic text overflow detection.
// It estimates text height based on content, font size, and paragraph structure.
type TextOverflowAnalyzer struct {
	// DefaultLineHeightMultiplier is how many times the font size is used to estimate line height
	// PowerPoint typically uses ~1.2-1.5x font size for line spacing depending on settings
	DefaultLineHeightMultiplier float64
	// DefaultFontSizePoints is the fallback font size in points when none is specified
	DefaultFontSizePoints float64
}

// NewTextOverflowAnalyzer creates a new analyzer with reasonable defaults.
func NewTextOverflowAnalyzer() *TextOverflowAnalyzer {
	return &TextOverflowAnalyzer{
		DefaultLineHeightMultiplier: 1.3,
		DefaultFontSizePoints:       18.0,
	}
}

// AnalyzeTextOverflow analyzes a single shape for text overflow.
// Returns nil if no overflow is detected or if the shape has no text.
func (a *TextOverflowAnalyzer) AnalyzeTextOverflow(shape *model.ShapeInfo, textBlock *model.TextBlockInfo) *model.TextOverflowInfo {
	// No text = no overflow
	if textBlock == nil || len(textBlock.Paragraphs) == 0 {
		return nil
	}

	// Need bounds to check overflow
	if shape.Bounds == nil || shape.Bounds.CY <= 0 {
		return nil
	}

	availableHeight := shape.Bounds.CY

	// Account for text body insets if present
	if textBlock.BodyProperties != nil {
		if textBlock.BodyProperties.TopInset != nil {
			availableHeight -= *textBlock.BodyProperties.TopInset
		}
		if textBlock.BodyProperties.BottomInset != nil {
			availableHeight -= *textBlock.BodyProperties.BottomInset
		}
	}

	// Get the line height - use the largest font size found
	maxFontSize := a.estimateMaxFontSize(textBlock)
	lineHeightEMU := a.estimateLineHeight(maxFontSize)

	// Count total lines: sum of paragraphs + line wraps within paragraphs
	totalLines := a.estimateLineCount(textBlock)

	// Estimate total height needed
	estimatedHeight := lineHeightEMU * int64(totalLines)

	overflowAmount := estimatedHeight - availableHeight

	// Only flag if overflow is significant (more than half a line)
	if overflowAmount <= lineHeightEMU/2 {
		return nil
	}

	severity := "medium"
	if overflowAmount > lineHeightEMU*3 {
		severity = "high"
	} else if overflowAmount > 0 {
		severity = "low"
	}

	reason := fmt.Sprintf(
		"Text requires ~%d EMU height but only %d available (%d EMU overflow)",
		estimatedHeight, availableHeight, overflowAmount,
	)

	return &model.TextOverflowInfo{
		ShapeID:             shape.ID,
		ShapeName:           shape.Name,
		Severity:            severity,
		EstimatedTextHeight: estimatedHeight,
		AvailableHeight:     availableHeight,
		OverflowAmount:      overflowAmount,
		TextLength:          len(textBlock.PlainText),
		ParagraphCount:      len(textBlock.Paragraphs),
		AverageLineHeight:   lineHeightEMU,
		Reason:              reason,
	}
}

// estimateMaxFontSize finds the largest font size used in the text block.
// Returns font size in points.
func (a *TextOverflowAnalyzer) estimateMaxFontSize(textBlock *model.TextBlockInfo) float64 {
	maxSize := a.DefaultFontSizePoints

	for _, para := range textBlock.Paragraphs {
		// Check paragraph default run properties
		if para.Properties != nil && para.Properties.DefaultRunProps != nil {
			if para.Properties.DefaultRunProps.FontSize != nil {
				if *para.Properties.DefaultRunProps.FontSize > maxSize {
					maxSize = *para.Properties.DefaultRunProps.FontSize
				}
			}
		}

		// Check individual runs
		for _, run := range para.Runs {
			if textRun, ok := run.(*model.TextRun); ok {
				if textRun.Properties != nil && textRun.Properties.FontSize != nil {
					if *textRun.Properties.FontSize > maxSize {
						maxSize = *textRun.Properties.FontSize
					}
				}
			}
		}
	}

	return maxSize
}

// estimateLineHeight converts font size in points to EMUs (English Metric Units).
// 1 point = 12700 EMU, and we apply the line height multiplier.
func (a *TextOverflowAnalyzer) estimateLineHeight(fontSizePoints float64) int64 {
	// Convert points to EMU: 1 point = 12700 EMU
	const emuPerPoint = 12700.0
	lineHeightEMU := fontSizePoints * emuPerPoint * a.DefaultLineHeightMultiplier
	return int64(math.Round(lineHeightEMU))
}

// estimateLineCount estimates the number of lines needed to display the text.
// This is a heuristic that assumes:
// - Each paragraph uses at least one line
// - Lines wrap based on a rough character-width estimate
func (a *TextOverflowAnalyzer) estimateLineCount(textBlock *model.TextBlockInfo) int {
	totalLines := 0

	for _, para := range textBlock.Paragraphs {
		if para.Text == "" {
			continue
		}

		// Each paragraph uses at least 1 line
		paraLines := 1

		// Rough estimate: assume ~40 characters per line in a typical text box
		// This is very conservative and varies with actual shape width
		// but we don't have width here, so we use paragraph text length as proxy
		charPerLine := 40
		textLength := len(para.Text)

		if textLength > charPerLine {
			additionalLines := (textLength / charPerLine)
			paraLines += additionalLines
		}

		totalLines += paraLines
	}

	// Account for empty paragraphs (they still take vertical space)
	if totalLines == 0 {
		totalLines = 1
	}

	return totalLines
}

// CollisionAnalyzer provides heuristic shape collision (overlap) detection.
// It uses axis-aligned bounding box (AABB) overlap detection.
type CollisionAnalyzer struct {
	// FilterIdenticalBounds when true, suppresses reporting collisions where
	// two shapes have identical bounds (common for intentional layering/shadows)
	FilterIdenticalBounds bool
}

// NewCollisionAnalyzer creates a new collision analyzer with defaults.
func NewCollisionAnalyzer() *CollisionAnalyzer {
	return &CollisionAnalyzer{
		FilterIdenticalBounds: true,
	}
}

// AnalyzeShapeCollisions detects collisions among shapes in a slide.
// Returns only significant overlaps.
func (a *CollisionAnalyzer) AnalyzeShapeCollisions(shapes []model.ShapeInfo) []model.CollisionInfo {
	var collisions []model.CollisionInfo

	// Compare each pair of shapes
	for i := 0; i < len(shapes); i++ {
		for j := i + 1; j < len(shapes); j++ {
			if collision := a.analyzeCollisionPair(&shapes[i], &shapes[j]); collision != nil {
				collisions = append(collisions, *collision)
			}
		}
	}

	return collisions
}

// analyzeCollisionPair checks if two shapes collide.
func (a *CollisionAnalyzer) analyzeCollisionPair(shape1, shape2 *model.ShapeInfo) *model.CollisionInfo {
	// Need bounds for both shapes
	if shape1.Bounds == nil || shape2.Bounds == nil {
		return nil
	}

	bounds1 := shape1.Bounds
	bounds2 := shape2.Bounds

	// Check for axis-aligned bounding box overlap
	// Shapes overlap if:
	// - shape1.left < shape2.right AND
	// - shape2.left < shape1.right AND
	// - shape1.top < shape2.bottom AND
	// - shape2.top < shape1.bottom

	shape1Right := bounds1.X + bounds1.CX
	shape2Right := bounds2.X + bounds2.CX
	shape1Bottom := bounds1.Y + bounds1.CY
	shape2Bottom := bounds2.Y + bounds2.CY

	if bounds1.X >= shape2Right || bounds2.X >= shape1Right ||
		bounds1.Y >= shape2Bottom || bounds2.Y >= shape1Bottom {
		// No overlap
		return nil
	}

	// Calculate overlap
	overlapLeft := maxInt64(bounds1.X, bounds2.X)
	overlapTop := maxInt64(bounds1.Y, bounds2.Y)
	overlapRight := minInt64(shape1Right, shape2Right)
	overlapBottom := minInt64(shape1Bottom, shape2Bottom)

	overlapWidth := overlapRight - overlapLeft
	overlapHeight := overlapBottom - overlapTop
	overlapArea := overlapWidth * overlapHeight

	// Check if shapes have identical bounds
	isIdentical := bounds1.X == bounds2.X && bounds1.Y == bounds2.Y &&
		bounds1.CX == bounds2.CX && bounds1.CY == bounds2.CY

	// Filter identical bounds if configured (common for intentional overlays)
	if a.FilterIdenticalBounds && isIdentical {
		return nil
	}

	// Calculate overlap percentage relative to smaller shape
	area1 := bounds1.CX * bounds1.CY
	area2 := bounds2.CX * bounds2.CY
	smallerArea := minInt64(area1, area2)
	overlapPercentage := 0.0
	if smallerArea > 0 {
		overlapPercentage = float64(overlapArea) / float64(smallerArea) * 100.0
	}

	// Only report significant overlaps (>5% of smaller shape)
	if overlapPercentage < 5.0 {
		return nil
	}

	severity := "low"
	if overlapPercentage > 50.0 {
		severity = "high"
	} else if overlapPercentage > 20.0 {
		severity = "medium"
	}

	reason := "Shapes have overlapping bounding boxes"
	if isIdentical {
		reason = "Shapes have identical bounds (likely intentional overlay)"
	}

	return &model.CollisionInfo{
		ShapeID1:                   shape1.ID,
		ShapeName1:                 shape1.Name,
		ShapeID2:                   shape2.ID,
		ShapeName2:                 shape2.Name,
		Severity:                   severity,
		OverlapArea:                overlapArea,
		OverlapPercentageOfSmaller: overlapPercentage,
		Shape1Area:                 area1,
		Shape2Area:                 area2,
		IsIdenticalBounds:          isIdentical,
		Reason:                     reason,
	}
}

// SlideDensityAnalyzer provides slide area occupancy metrics.
type SlideDensityAnalyzer struct{}

// NewSlideDensityAnalyzer creates a new density analyzer.
func NewSlideDensityAnalyzer() *SlideDensityAnalyzer {
	return &SlideDensityAnalyzer{}
}

// CalculateSlideDensity calculates how much of the slide is occupied by shapes.
// Returns the percentage of slide area covered by shapes (0-100).
// Note: This is approximate and doesn't account for transparency or overlaps perfectly.
func (a *SlideDensityAnalyzer) CalculateSlideDensity(shapes []model.ShapeInfo, slideWidth, slideHeight int64) float64 {
	// Standard slide dimensions in EMU (for reference: 10 inches x 7.5 inches)
	// But we use provided dimensions
	if slideWidth <= 0 || slideHeight <= 0 {
		return 0.0
	}

	slideArea := slideWidth * slideHeight

	// Calculate total shape area (without accounting for overlaps)
	totalArea := int64(0)
	for _, shape := range shapes {
		if shape.Bounds != nil && shape.Bounds.CX > 0 && shape.Bounds.CY > 0 {
			area := shape.Bounds.CX * shape.Bounds.CY
			totalArea += area
		}
	}

	if totalArea == 0 {
		return 0.0
	}

	density := float64(totalArea) / float64(slideArea) * 100.0

	// Cap at 100% (shouldn't happen with correct bounds, but just in case)
	if density > 100.0 {
		density = 100.0
	}

	return density
}

// CalculateSlideDensityInfo calculates density metrics for a slide and returns them as SlideDensityInfo.
func (a *SlideDensityAnalyzer) CalculateSlideDensityInfo(shapes []model.ShapeInfo, slideWidth, slideHeight int64) model.SlideDensityInfo {
	slideArea := slideWidth * slideHeight

	// Calculate total shape area
	totalArea := int64(0)
	for _, shape := range shapes {
		if shape.Bounds != nil && shape.Bounds.CX > 0 && shape.Bounds.CY > 0 {
			area := shape.Bounds.CX * shape.Bounds.CY
			totalArea += area
		}
	}

	densityPercentage := 0.0
	if slideArea > 0 {
		densityPercentage = float64(totalArea) / float64(slideArea) * 100.0
		if densityPercentage > 100.0 {
			densityPercentage = 100.0
		}
	}

	// Classify the density
	classification := a.ClassifyDensity(densityPercentage)

	return model.SlideDensityInfo{
		TotalShapeArea:    totalArea,
		SlideArea:         slideArea,
		DensityPercentage: densityPercentage,
		ShapeCount:        len(shapes),
		Classification:    classification,
	}
}

// ClassifyDensity returns a classification string based on density percentage.
// Thresholds: empty (<5%), sparse (5-30%), moderate (30-70%), dense (>70%)
func (a *SlideDensityAnalyzer) ClassifyDensity(densityPercentage float64) string {
	switch {
	case densityPercentage < 5.0:
		return "empty"
	case densityPercentage < 30.0:
		return "sparse"
	case densityPercentage < 70.0:
		return "moderate"
	default:
		return "dense"
	}
}

// AnalyzeSlideLayoutQA performs comprehensive layout QA on a single slide.
// It detects text overflow, collisions, calculates density, and returns a complete report.
// slideWidth and slideHeight are the slide dimensions in EMUs (standard: 9144000 x 6858000)
func AnalyzeSlideLayoutQA(slideIndex int, shapes []model.ShapeInfo, textBlocks map[int]*model.TextBlockInfo, slideWidth, slideHeight int64) model.LayoutQAReport {
	report := model.LayoutQAReport{
		SlideIndex:  slideIndex,
		SlideNumber: slideIndex + 1,
	}

	overflowAnalyzer := NewTextOverflowAnalyzer()
	collisionAnalyzer := NewCollisionAnalyzer()
	densityAnalyzer := NewSlideDensityAnalyzer()

	// Analyze text overflow for each shape
	for i := range shapes {
		textBlock := textBlocks[shapes[i].ID]
		if overflow := overflowAnalyzer.AnalyzeTextOverflow(&shapes[i], textBlock); overflow != nil {
			report.TextOverflows = append(report.TextOverflows, *overflow)
		}
	}

	// Analyze shape collisions
	collisions := collisionAnalyzer.AnalyzeShapeCollisions(shapes)
	report.Collisions = collisions

	// Calculate slide density
	densityInfo := densityAnalyzer.CalculateSlideDensityInfo(shapes, slideWidth, slideHeight)
	report.Density = &densityInfo

	// Update summary
	report.IssueCount = len(report.TextOverflows) + len(report.Collisions)
	report.HasIssues = report.IssueCount > 0

	return report
}

// AnalyzePresentationLayoutQA performs comprehensive layout QA on all slides in a presentation.
// Returns a complete LayoutQAAnalysis report with summary statistics.
// slideReports should be a list of LayoutQAReport structs (one per slide)
// The function calculates summary metrics and aggregates findings.
func AnalyzePresentationLayoutQA(slideReports []model.LayoutQAReport) model.LayoutQAAnalysis {
	analysis := model.LayoutQAAnalysis{
		SlideReports: slideReports,
		TotalSlides:  len(slideReports),
		HasIssues:    false,
	}

	var totalDensity float64

	for _, report := range slideReports {
		if report.HasIssues {
			analysis.SlidesWithIssues++
			analysis.HasIssues = true
		}

		analysis.TotalTextOverflows += len(report.TextOverflows)
		analysis.TotalCollisions += len(report.Collisions)

		if report.Density != nil {
			totalDensity += report.Density.DensityPercentage
			if report.Density.Classification == "dense" {
				analysis.SlidesWithHighDensity++
			}
		}
	}

	if len(slideReports) > 0 {
		analysis.AverageDensity = totalDensity / float64(len(slideReports))
	}

	return analysis
}

// Helper functions
func minInt64(a, b int64) int64 {
	if a < b {
		return a
	}
	return b
}

func maxInt64(a, b int64) int64 {
	if a > b {
		return a
	}
	return b
}

// ShapeListToMap creates a map of shape IDs to shapes for quick lookup
func ShapeListToMap(shapes []model.ShapeInfo) map[int]*model.ShapeInfo {
	m := make(map[int]*model.ShapeInfo)
	for i := range shapes {
		m[shapes[i].ID] = &shapes[i]
	}
	return m
}
