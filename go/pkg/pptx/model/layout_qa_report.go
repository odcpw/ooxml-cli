package model

// TextOverflowInfo represents a potential text overflow condition in a shape.
// This is a HEURISTIC analysis - it flags shapes where text may overflow based on
// estimated text dimensions and available shape geometry. Not all flagged cases
// will actually overflow in LibreOffice/PowerPoint depending on exact rendering
// and font metrics. False negatives (missed overflow) are possible with unusual
// fonts or very aggressive text wrapping.
type TextOverflowInfo struct {
	// ShapeID is the unique ID of the shape containing the text
	ShapeID int `json:"shapeId"`
	// ShapeName is the name of the shape
	ShapeName string `json:"shapeName"`
	// Severity indicates the confidence in the overflow detection: "high", "medium", "low"
	Severity string `json:"severity"`
	// EstimatedTextHeight is the estimated height needed for the text content (in EMUs)
	EstimatedTextHeight int64 `json:"estimatedTextHeight"`
	// AvailableHeight is the height available in the shape (in EMUs)
	AvailableHeight int64 `json:"availableHeight"`
	// OverflowAmount is how much the text exceeds available height (in EMUs)
	// Positive means overflow, negative means excess space
	OverflowAmount int64 `json:"overflowAmount"`
	// TextLength is the length of the plain text content (in characters)
	TextLength int `json:"textLength"`
	// ParagraphCount is the number of paragraphs in the text
	ParagraphCount int `json:"paragraphCount"`
	// AverageLineHeight is the estimated average line height based on font size (in EMUs)
	// Calculated from run properties, may be approximate
	AverageLineHeight int64 `json:"averageLineHeight"`
	// Reason provides a human-readable explanation of why overflow was flagged
	Reason string `json:"reason"`
}

// CollisionInfo represents a potential collision (overlap) between two shapes.
// This is a HEURISTIC analysis - it detects axis-aligned bounding box overlaps.
// It may miss collisions with rotated shapes or filtered intentional overlays.
// False positives may occur with shapes that are intentionally identical overlays
// for visual effects (e.g., shadow effects, intentional layering).
type CollisionInfo struct {
	// ShapeID1 is the ID of the first shape
	ShapeID1 int `json:"shapeId1"`
	// ShapeName1 is the name of the first shape
	ShapeName1 string `json:"shapeName1"`
	// ShapeID2 is the ID of the second shape
	ShapeID2 int `json:"shapeId2"`
	// ShapeName2 is the name of the second shape
	ShapeName2 string `json:"shapeName2"`
	// Severity indicates the confidence in the collision detection: "high", "medium", "low"
	Severity string `json:"severity"`
	// OverlapArea is the estimated area of overlap in square EMUs
	OverlapArea int64 `json:"overlapArea"`
	// OverlapPercentageOfSmaller is the overlap as a percentage of the smaller shape's area
	// 0-100, higher values indicate more significant overlap
	OverlapPercentageOfSmaller float64 `json:"overlapPercentageOfSmaller"`
	// Shape1Area is the area of the first shape (in square EMUs)
	Shape1Area int64 `json:"shape1Area"`
	// Shape2Area is the area of the second shape (in square EMUs)
	Shape2Area int64 `json:"shape2Area"`
	// IsIdenticalBounds indicates if the two shapes have identical bounding boxes
	// (common for intentional overlays like shadows or layered effects)
	IsIdenticalBounds bool `json:"isIdenticalBounds"`
	// Reason provides a human-readable explanation of the collision
	Reason string `json:"reason"`
}

// SlideDensityInfo represents the area occupancy metrics for a slide.
type SlideDensityInfo struct {
	// TotalShapeArea is the sum of areas of all shapes (in square EMUs)
	TotalShapeArea int64 `json:"totalShapeArea"`
	// SlideArea is the total slide area (in square EMUs)
	SlideArea int64 `json:"slideArea"`
	// DensityPercentage is the percentage of slide area occupied by shapes (0-100)
	DensityPercentage float64 `json:"densityPercentage"`
	// ShapeCount is the number of shapes on the slide
	ShapeCount int `json:"shapeCount"`
	// Classification categorizes the slide density: "empty", "sparse", "moderate", "dense"
	Classification string `json:"classification"`
}

// LayoutQAReport represents the complete layout quality analysis for a slide.
type LayoutQAReport struct {
	// SlideIndex is the 0-based index of the slide in the presentation
	SlideIndex int `json:"slideIndex"`
	// SlideNumber is the 1-based slide number
	SlideNumber int `json:"slideNumber"`
	// TextOverflows is the list of detected text overflow conditions
	TextOverflows []TextOverflowInfo `json:"textOverflows,omitempty"`
	// Collisions is the list of detected shape collisions
	Collisions []CollisionInfo `json:"collisions,omitempty"`
	// Density is the slide area occupancy metrics
	Density *SlideDensityInfo `json:"density,omitempty"`
	// HasIssues indicates if any issues were detected
	HasIssues bool `json:"hasIssues"`
	// IssueCount is the total number of issues (overflows + collisions)
	IssueCount int `json:"issueCount"`
	// Notes contains any additional diagnostic information
	Notes string `json:"notes,omitempty"`
}

// LayoutQAAnalysis is the top-level report for a presentation's layout quality.
type LayoutQAAnalysis struct {
	// SlideReports contains reports for each slide that was analyzed
	SlideReports []LayoutQAReport `json:"slideReports,omitempty"`
	// TotalSlides is the total number of slides analyzed
	TotalSlides int `json:"totalSlides"`
	// SlidesWithIssues is the count of slides with at least one issue
	SlidesWithIssues int `json:"slidesWithIssues"`
	// SlidesWithHighDensity is the count of slides with "dense" classification
	SlidesWithHighDensity int `json:"slidesWithHighDensity"`
	// AverageDensity is the average density percentage across all slides
	AverageDensity float64 `json:"averageDensity"`
	// TotalTextOverflows is the total count of text overflow detections
	TotalTextOverflows int `json:"totalTextOverflows"`
	// TotalCollisions is the total count of collision detections
	TotalCollisions int `json:"totalCollisions"`
	// HasIssues indicates if any issues were detected in any slide
	HasIssues bool `json:"hasIssues"`
}
