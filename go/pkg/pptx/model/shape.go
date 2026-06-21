package model

// ShapeType represents the type of shape element
type ShapeType string

const (
	ShapeTypeSP           ShapeType = "sp"           // Shape (connector, callout, rectangle, etc.)
	ShapeTypePic          ShapeType = "pic"          // Picture
	ShapeTypeGraphicFrame ShapeType = "graphicFrame" // Chart, table, SmartArt, etc.
	ShapeTypeGroup        ShapeType = "grpSp"        // Group shape
)

// Bounds represents the bounding box of a shape in EMUs (English Metric Units)
type Bounds struct {
	X  int64 `json:"x"`
	Y  int64 `json:"y"`
	CX int64 `json:"cx"`
	CY int64 `json:"cy"`
}

// CropInfo represents image cropping information
type CropInfo struct {
	Left   int `json:"left,omitempty"`   // Left crop in units of 100000
	Top    int `json:"top,omitempty"`    // Top crop in units of 100000
	Right  int `json:"right,omitempty"`  // Right crop in units of 100000
	Bottom int `json:"bottom,omitempty"` // Bottom crop in units of 100000
}

// Geometry represents complete geometry information for a shape/image
type Geometry struct {
	Bounds   *Bounds   `json:"bounds,omitempty"`   // Position and size in EMUs
	Rotation int       `json:"rotation,omitempty"` // Rotation in 1/60000 of a degree (0-5400000)
	FlipH    bool      `json:"flipH,omitempty"`    // Horizontal flip
	FlipV    bool      `json:"flipV,omitempty"`    // Vertical flip
	Crop     *CropInfo `json:"crop,omitempty"`     // Image crop info
}

// ShapeInfo represents a shape element on a slide or layout
type ShapeInfo struct {
	ID            int        `json:"id"`
	Name          string     `json:"shapeName"`
	Type          ShapeType  `json:"type"`
	Bounds        *Bounds    `json:"bounds,omitempty"`
	Geometry      *Geometry  `json:"geometry,omitempty"` // Complete geometry including rotation, flip, crop
	IsPlaceholder bool       `json:"isPlaceholder"`
	TableInfo     *TableInfo `json:"tableInfo,omitempty"`
	ImageRef      *ImageRef  `json:"imageRef,omitempty"`
	TextContent   string     `json:"textContent,omitempty"` // Text content extracted from shape (when --include-text is used)
}
