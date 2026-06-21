package model

// LayoutInfo represents basic metadata about a slide layout
type LayoutInfo struct {
	PartURI   string `json:"partUri"`
	Name      string `json:"name"`
	MasterRef string `json:"masterRef"`
	Preserve  bool   `json:"preserve"`
	UserDrawn bool   `json:"userDrawn"`
}

// LayoutReport represents a detailed report of a slide layout
type LayoutReport struct {
	ID           string             `json:"id"`
	Name         string             `json:"name"`
	PartURI      string             `json:"partUri"`
	MasterID     string             `json:"masterId"`
	Preserve     bool               `json:"preserve"`
	UserDrawn    bool               `json:"userDrawn"`
	Placeholders []*PlaceholderInfo `json:"placeholders,omitempty"`
	Shapes       []*ShapeInfo       `json:"shapes,omitempty"`
	Images       []*ImageRef        `json:"images,omitempty"`
}
