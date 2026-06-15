package model

// RawPlaceholder represents the raw placeholder attributes from the PPTX file
// before normalization/resolution
type RawPlaceholder struct {
	Type   string // p:ph@type (e.g., "title", "body", "pic")
	Idx    int    // p:ph@idx (0-based index, or -1 if not present)
	Sz     string // p:ph@sz (placeholder size enum)
	Orient string // p:ph@orient (placeholder orientation)
}

// ResolvedPlaceholder represents a placeholder after type resolution
// and canonical role mapping.
type ResolvedPlaceholder struct {
	// Original XML attributes
	Raw RawPlaceholder

	// Resolved canonical role (from CanonicalRole mapping)
	Role string

	// Associated shape metadata
	ShapeID   int    // Shape ID from nvSpPr
	ShapeName string // Shape name
}

// PlaceholderInfo represents a placeholder on a slide or layout (JSON output format).
// Used by inspect commands for reporting.
type PlaceholderInfo struct {
	Key          string    `json:"key"`                // Generated key (from GenerateKey)
	Role         string    `json:"role"`               // Canonical role (from CanonicalRole)
	Index        int       `json:"index"`              // Original p:ph@idx
	ShapeName    string    `json:"shapeName"`          // Shape name from p:cNvPr
	LiteralType  string    `json:"literalType"`        // Original p:ph@type if different from resolved
	ResolvedType string    `json:"resolvedType"`       // Final resolved type after inheritance
	Geometry     *Geometry `json:"geometry,omitempty"` // Position, size, rotation, flip, crop info
}
