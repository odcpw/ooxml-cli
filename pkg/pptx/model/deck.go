package model

// SlideDimensions represents the dimensions of slides in a presentation
type SlideDimensions struct {
	CX   int64  `json:"cx"`
	CY   int64  `json:"cy"`
	Unit string `json:"unit"`
}

// DeckSummary represents a high-level summary of a PPTX presentation
type DeckSummary struct {
	Slides         int              `json:"slides"`
	Masters        int              `json:"masters"`
	Layouts        int              `json:"layouts"`
	Themes         int              `json:"themes"`
	NotesMasters   int              `json:"notesMasters"`
	HandoutMasters int              `json:"handoutMasters"`
	MediaAssets    int              `json:"mediaAssets"`
	CustomXmlParts int              `json:"customXmlParts"`
	SlideSize      *SlideDimensions `json:"slideSize,omitempty"`
}
