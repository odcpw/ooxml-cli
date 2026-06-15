package model

// ImageRef represents a reference to an image in a presentation
type ImageRef struct {
	RelID       string `json:"relId"`
	TargetURI   string `json:"targetUri"`
	ContentType string `json:"contentType"`
}

// ExtractedImageInfo represents information about an extracted image
type ExtractedImageInfo struct {
	// Image identification and location
	SourcePartURI  string `json:"sourcePartUri"`  // Where the image reference is located (e.g., /ppt/slides/slide1.xml)
	ShapeID        int    `json:"shapeId"`        // Shape ID that contains the image
	ShapeName      string `json:"shapeName"`      // Shape name
	RelationshipID string `json:"relationshipId"` // The r:embed relationship ID
	TargetURI      string `json:"targetUri"`      // Resolved target URI (e.g., /ppt/media/image1.png)
	ContentType    string `json:"contentType"`    // Content type (e.g., image/png)
	FilePath       string `json:"filePath"`       // Extracted file path (relative to output directory)
	FileSize       int64  `json:"fileSize"`       // Size in bytes

	// Geometry information
	Geometry *Geometry `json:"geometry,omitempty"` // Position, size, rotation, flip, crop info

	// Optional layout image indicator
	IsLayoutImage bool `json:"isLayoutImage,omitempty"` // True if from layout, false if from slide
}

// ExtractImagesManifest represents the complete manifest of extracted images
type ExtractImagesManifest struct {
	File            string               `json:"file"`                  // Source PPTX file
	SlideNumber     int                  `json:"slideNumber,omitempty"` // Slide number if filtering by slide
	OutputDirectory string               `json:"outputDirectory"`       // Where images were extracted
	IncludeLayout   bool                 `json:"includeLayout"`         // Whether layout images were included
	ImagesCount     int                  `json:"imagesCount"`           // Total number of images extracted
	Images          []ExtractedImageInfo `json:"images"`                // List of extracted images
}
