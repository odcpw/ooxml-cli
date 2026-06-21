package template

import (
	"fmt"
	"time"
)

// SlotKind represents the type of content a slot expects
type SlotKind string

const (
	// SlotKindText - plain text slot (a:t elements)
	SlotKindText SlotKind = "text"

	// SlotKindRichText - rich text slot (formatted text with runs)
	SlotKindRichText SlotKind = "richText"

	// SlotKindBullets - bulleted/list text slot (paragraphs with list properties)
	SlotKindBullets SlotKind = "bullets"

	// SlotKindImage - image placeholder or shape
	SlotKindImage SlotKind = "image"

	// SlotKindTable - table shape (a:tbl)
	SlotKindTable SlotKind = "table"

	// SlotKindNotes - notes slide content
	SlotKindNotes SlotKind = "notes"
)

// String returns the string representation of SlotKind
func (sk SlotKind) String() string {
	return string(sk)
}

// IsValid returns true if the SlotKind is a recognized kind
func (sk SlotKind) IsValid() bool {
	switch sk {
	case SlotKindText, SlotKindRichText, SlotKindBullets, SlotKindImage, SlotKindTable, SlotKindNotes:
		return true
	default:
		return false
	}
}

// Bounds represents a bounding box with position and size
type Bounds struct {
	X  int64 `json:"x"`  // Position in EMUs
	Y  int64 `json:"y"`  // Position in EMUs
	CX int64 `json:"cx"` // Width in EMUs
	CY int64 `json:"cy"` // Height in EMUs
}

// Slot represents a fillable area in an archetype
type Slot struct {
	// Unique identifier for this slot within the archetype
	ID string `json:"id"`

	// Human-readable name
	Name string `json:"name"`

	// Kind of content this slot expects
	Kind SlotKind `json:"kind"`

	// Position and size in EMUs (English Metric Units)
	Bounds *Bounds `json:"bounds,omitempty"`

	// Whether this slot must be filled during compilation
	Required bool `json:"required"`

	// Optional placeholder key if this maps to a normalized placeholder
	PlaceholderKey string `json:"placeholderKey,omitempty"`

	// Optional placeholder type/role if normalized
	PlaceholderRole string `json:"placeholderRole,omitempty"`

	// Optional placeholder index (from ph@idx)
	PlaceholderIndex string `json:"placeholderIndex,omitempty"`

	// For table slots: expected number of rows (optional, informational)
	TableRows *int `json:"tableRows,omitempty"`

	// For table slots: expected number of columns (optional, informational)
	TableCols *int `json:"tableCols,omitempty"`

	// For table slots: graphic frame shape id for in-place cell updates
	TableID *int `json:"tableId,omitempty"`

	// For image slots: aspect ratio hint (width/height, optional)
	AspectRatio *float64 `json:"aspectRatio,omitempty"`

	// Notes or constraints for users
	Notes string `json:"notes,omitempty"`
}

// StaticShape represents a shape that is not fillable (static content)
type StaticShape struct {
	// Unique identifier
	ID string `json:"id"`

	// Shape name
	Name string `json:"name"`

	// Shape type (sp, pic, graphicFrame, grpSp)
	Type string `json:"type"`

	// Position and size
	Bounds *Bounds `json:"bounds,omitempty"`

	// Whether this shape should be preserved during compilation
	Preserve bool `json:"preserve"`
}

// Archetype represents a template slide blueprint
type Archetype struct {
	// Unique identifier for this archetype (e.g., "title-slide", "content-2col")
	ID string `json:"id"`

	// Display name
	Name string `json:"name"`

	// Description of what this archetype is used for
	Description string `json:"description,omitempty"`

	// List of fillable slots in this archetype
	Slots []Slot `json:"slots"`

	// List of static shapes (decorations, logos, backgrounds)
	StaticShapes []StaticShape `json:"staticShapes,omitempty"`

	// Layout name/ID this archetype is based on
	LayoutName string `json:"layoutName,omitempty"`

	// Master name/ID this archetype is based on
	MasterName string `json:"masterName,omitempty"`

	// Source slide number this archetype was captured from
	SourceSlideNumber int `json:"sourceSlideNumber,omitempty"`

	// Notes about this archetype
	Notes string `json:"notes,omitempty"`
}

// Version represents versioning information
type Version struct {
	// Major version number
	Major int `json:"major"`

	// Minor version number
	Minor int `json:"minor"`

	// Patch version number
	Patch int `json:"patch"`

	// When this version was created
	CreatedAt time.Time `json:"createdAt"`

	// Free-form description of changes in this version
	Notes string `json:"notes,omitempty"`
}

// String returns the semantic version string (e.g., "1.0.0")
func (v *Version) String() string {
	return fmt.Sprintf("%d.%d.%d", v.Major, v.Minor, v.Patch)
}

// TemplateManifest is the root structure for captured template archetypes
type TemplateManifest struct {
	// Manifest version for schema compatibility
	ManifestVersion string `json:"manifestVersion"`

	// Template name (e.g., "Corporate Branding", "Quarterly Report")
	Name string `json:"name"`

	// Detailed description
	Description string `json:"description,omitempty"`

	// Version tracking for this template
	Version *Version `json:"version"`

	// When this template was created
	CreatedAt time.Time `json:"createdAt"`

	// When this template was last modified
	ModifiedAt time.Time `json:"modifiedAt"`

	// Author information
	Author string `json:"author,omitempty"`

	// Company or organization
	Organization string `json:"organization,omitempty"`

	// List of available archetypes
	Archetypes []Archetype `json:"archetypes"`

	// Optional metadata about the source deck
	SourceFile string `json:"sourceFile,omitempty"`

	// Optional: original PPTX theme colors (for reference)
	ThemeReference *ThemeReference `json:"themeReference,omitempty"`

	// Notes about this template
	Notes string `json:"notes,omitempty"`
}

// ThemeReference contains reference information about the source theme
type ThemeReference struct {
	// Named color scheme entries (e.g., "accent1", "accent2")
	Colors map[string]string `json:"colors,omitempty"`

	// Font information
	MajorFont string `json:"majorFont,omitempty"`
	MinorFont string `json:"minorFont,omitempty"`
}

// ValidateManifest performs comprehensive validation of a template manifest
func (tm *TemplateManifest) ValidateManifest() error {
	if tm == nil {
		return fmt.Errorf("manifest is nil")
	}

	if tm.Name == "" {
		return fmt.Errorf("manifest must have a non-empty name")
	}

	if tm.ManifestVersion == "" {
		return fmt.Errorf("manifest must have a manifestVersion")
	}

	if tm.Version == nil {
		return fmt.Errorf("manifest must have version information")
	}

	if err := tm.Version.Validate(); err != nil {
		return fmt.Errorf("invalid version: %w", err)
	}

	if len(tm.Archetypes) == 0 {
		return fmt.Errorf("manifest must have at least one archetype")
	}

	// Track archetype IDs for uniqueness
	seenArchetypeIDs := make(map[string]bool)
	for i, arch := range tm.Archetypes {
		if arch.ID == "" {
			return fmt.Errorf("archetype at index %d has empty ID", i)
		}

		if seenArchetypeIDs[arch.ID] {
			return fmt.Errorf("duplicate archetype ID: %s", arch.ID)
		}
		seenArchetypeIDs[arch.ID] = true

		if err := arch.Validate(); err != nil {
			return fmt.Errorf("archetype %s is invalid: %w", arch.ID, err)
		}
	}

	return nil
}

// Validate performs validation on a Version
func (v *Version) Validate() error {
	if v == nil {
		return fmt.Errorf("version is nil")
	}

	if v.Major < 0 || v.Minor < 0 || v.Patch < 0 {
		return fmt.Errorf("version numbers must be non-negative, got %d.%d.%d", v.Major, v.Minor, v.Patch)
	}

	if v.CreatedAt.IsZero() {
		return fmt.Errorf("version must have a createdAt timestamp")
	}

	return nil
}

// Validate performs validation on an Archetype
func (a *Archetype) Validate() error {
	if a == nil {
		return fmt.Errorf("archetype is nil")
	}

	if a.ID == "" {
		return fmt.Errorf("archetype must have an ID")
	}

	if a.Name == "" {
		return fmt.Errorf("archetype %s must have a name", a.ID)
	}

	if len(a.Slots) == 0 {
		return fmt.Errorf("archetype %s must have at least one slot", a.ID)
	}

	// Track slot IDs for uniqueness
	seenSlotIDs := make(map[string]bool)
	for i, slot := range a.Slots {
		if slot.ID == "" {
			return fmt.Errorf("archetype %s: slot at index %d has empty ID", a.ID, i)
		}

		if seenSlotIDs[slot.ID] {
			return fmt.Errorf("archetype %s: duplicate slot ID %s", a.ID, slot.ID)
		}
		seenSlotIDs[slot.ID] = true

		if err := slot.Validate(); err != nil {
			return fmt.Errorf("archetype %s, slot %s is invalid: %w", a.ID, slot.ID, err)
		}
	}

	return nil
}

// Validate performs validation on a Slot
func (s *Slot) Validate() error {
	if s == nil {
		return fmt.Errorf("slot is nil")
	}

	if s.ID == "" {
		return fmt.Errorf("slot must have an ID")
	}

	if s.Name == "" {
		return fmt.Errorf("slot %s must have a name", s.ID)
	}

	if !s.Kind.IsValid() {
		return fmt.Errorf("slot %s: invalid kind %q", s.ID, s.Kind)
	}

	// Validate bounds if present
	if s.Bounds != nil {
		if s.Bounds.CX <= 0 || s.Bounds.CY <= 0 {
			return fmt.Errorf("slot %s: bounds must have positive width and height", s.ID)
		}
	}

	// Validate table dimensions if present
	if s.Kind == SlotKindTable {
		if s.TableRows != nil && *s.TableRows <= 0 {
			return fmt.Errorf("slot %s: tableRows must be positive", s.ID)
		}
		if s.TableCols != nil && *s.TableCols <= 0 {
			return fmt.Errorf("slot %s: tableCols must be positive", s.ID)
		}
	}

	// Validate aspect ratio if present
	if s.AspectRatio != nil && *s.AspectRatio <= 0 {
		return fmt.Errorf("slot %s: aspectRatio must be positive", s.ID)
	}

	return nil
}

// Validate performs validation on a StaticShape
func (ss *StaticShape) Validate() error {
	if ss == nil {
		return fmt.Errorf("staticShape is nil")
	}

	if ss.ID == "" {
		return fmt.Errorf("staticShape must have an ID")
	}

	return nil
}
