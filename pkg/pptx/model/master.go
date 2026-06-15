package model

// MasterInfo represents basic metadata about a slide master
type MasterInfo struct {
	PartURI    string   `json:"partUri"`
	LayoutURIs []string `json:"layoutUris"`
	ThemeURI   string   `json:"themeUri"`
}

// MasterReport represents a detailed report of a slide master
type MasterReport struct {
	ID         string        `json:"id"`
	PartURI    string        `json:"partUri"`
	LayoutURIs []string      `json:"layoutUris"`
	ThemeURI   string        `json:"themeUri"`
	Layouts    []*LayoutInfo `json:"layouts,omitempty"`
}
