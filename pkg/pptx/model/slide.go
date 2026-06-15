package model

// SlideInfo represents basic metadata about a slide
type SlideInfo struct {
	PartURI      string `json:"partUri"`
	SlideNumber  int    `json:"slideNumber"`
	LayoutRef    string `json:"layoutRef"`
	NotesPartURI string `json:"notesPartUri,omitempty"`
}

// SlideReport represents a detailed report of a slide
type SlideReport struct {
	ID           string       `json:"id"`
	Slide        int          `json:"slide"`
	PartURI      string       `json:"partUri"`
	LayoutRef    string       `json:"layoutRef"`
	NotesPartURI string       `json:"notesPartUri,omitempty"`
	Shapes       []*ShapeInfo `json:"shapes,omitempty"`
	Images       []*ImageRef  `json:"images,omitempty"`
	Tables       []*TableInfo `json:"tables,omitempty"`
}
