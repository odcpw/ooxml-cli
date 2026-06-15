package template

import (
	"fmt"
	"gopkg.in/yaml.v3"
)

// CompilationSpec represents the complete specification for compiling a deck from a template
type CompilationSpec struct {
	// Optional version constraint
	Version string `yaml:"version" json:"version,omitempty"`

	// List of slides to generate
	Slides []SlideSpec `yaml:"slides" json:"slides"`

	// Optional theme/style overrides
	ThemeOverrides *ThemeOverrides `yaml:"themeOverrides" json:"themeOverrides,omitempty"`

	// Optional metadata
	Title       string `yaml:"title" json:"title,omitempty"`
	Author      string `yaml:"author" json:"author,omitempty"`
	Description string `yaml:"description" json:"description,omitempty"`
}

// SlideSpec represents a single slide specification
type SlideSpec struct {
	// Archetype ID to use for this slide
	Archetype string `yaml:"archetype" json:"archetype"`

	// Content for each slot in the archetype
	Content map[string]interface{} `yaml:"content" json:"content"`

	// Optional slide-level overrides
	Overrides *SlideOverrides `yaml:"overrides" json:"overrides,omitempty"`

	// Optional notes for this slide
	Notes string `yaml:"notes" json:"notes,omitempty"`
}

// SlideOverrides represents slide-level style/theme overrides
type SlideOverrides struct {
	// Background color (RGB hex)
	BackgroundColor string `yaml:"backgroundColor" json:"backgroundColor,omitempty"`

	// Theme color mapping overrides for this slide
	ColorMappings map[string]string `yaml:"colorMappings" json:"colorMappings,omitempty"`

	// Font overrides
	FontMappings map[string]string `yaml:"fontMappings" json:"fontMappings,omitempty"`
}

// ThemeOverrides represents deck-level theme/style overrides
type ThemeOverrides struct {
	// Color scheme updates
	Colors map[string]string `yaml:"colors" json:"colors,omitempty"`

	// Font updates
	MajorFont string `yaml:"majorFont" json:"majorFont,omitempty"`
	MinorFont string `yaml:"minorFont" json:"minorFont,omitempty"`
}

// SlotContent represents the value for a single slot
type SlotContent struct {
	Type  string      `yaml:"type" json:"type"`
	Value interface{} `yaml:"value" json:"value"`
}

// TextSlotValue represents text content
type TextSlotValue struct {
	Text string `yaml:"text" json:"text"`
}

// RichTextSlotValue represents rich text content with formatting
type RichTextSlotValue struct {
	Text       string                 `yaml:"text" json:"text"`
	Runs       []RichTextRun          `yaml:"runs" json:"runs,omitempty"`
	Formatting map[string]interface{} `yaml:"formatting" json:"formatting,omitempty"`
}

// RichTextRun represents a formatted text run
type RichTextRun struct {
	Text       string            `yaml:"text" json:"text"`
	Properties map[string]string `yaml:"properties" json:"properties,omitempty"`
}

// BulletsSlotValue represents bulleted/list content
type BulletsSlotValue struct {
	Items []string `yaml:"items" json:"items"`

	// Optional: bullet styles
	Style string `yaml:"style" json:"style,omitempty"`
}

// ImageSlotValue represents image content
type ImageSlotValue struct {
	// Path to image file (relative or absolute)
	Path string `yaml:"path" json:"path"`

	// Optional: image description/alt text
	Description string `yaml:"description" json:"description,omitempty"`

	// Optional: cropping/positioning info
	Crop *ImageCrop `yaml:"crop" json:"crop,omitempty"`
}

// ImageCrop represents image cropping information
type ImageCrop struct {
	Left   int `yaml:"left" json:"left,omitempty"`
	Top    int `yaml:"top" json:"top,omitempty"`
	Right  int `yaml:"right" json:"right,omitempty"`
	Bottom int `yaml:"bottom" json:"bottom,omitempty"`
}

// TableSlotValue represents table content
type TableSlotValue struct {
	// Table data as list of rows, each row is list of cells
	Data [][]interface{} `yaml:"data" json:"data"`

	// Optional: header row present
	HasHeaders bool `yaml:"hasHeaders" json:"hasHeaders,omitempty"`

	// Optional: banded rows styling
	BandedRows bool `yaml:"bandedRows" json:"bandedRows,omitempty"`

	// Optional: alternate text for accessibility
	AltText string `yaml:"altText" json:"altText,omitempty"`
}

// NotesSlotValue represents notes slide content
type NotesSlotValue struct {
	// Notes text
	Text string `yaml:"text" json:"text"`
}

// ParseCompilationSpec parses a compilation spec from YAML or JSON data
func ParseCompilationSpec(data []byte) (*CompilationSpec, error) {
	var spec CompilationSpec

	// Try YAML first (more permissive)
	err := yaml.Unmarshal(data, &spec)
	if err != nil {
		return nil, fmt.Errorf("failed to parse compilation spec: %w", err)
	}

	return &spec, nil
}

// ValidateCompilationSpec validates a compilation spec against a manifest
func ValidateCompilationSpec(spec *CompilationSpec, manifest *TemplateManifest) error {
	if spec == nil {
		return fmt.Errorf("compilation spec is nil")
	}

	if manifest == nil {
		return fmt.Errorf("template manifest is nil")
	}

	if len(spec.Slides) == 0 {
		return fmt.Errorf("compilation spec must have at least one slide")
	}

	// Index archetypes by ID for quick lookup
	archetypeMap := make(map[string]*Archetype)
	for i := range manifest.Archetypes {
		arch := &manifest.Archetypes[i]
		archetypeMap[arch.ID] = arch
	}

	// Validate each slide
	for i, slideSpec := range spec.Slides {
		if err := validateSlideSpec(&slideSpec, i, archetypeMap); err != nil {
			return err
		}
	}

	return nil
}

// validateSlideSpec validates a single slide specification
func validateSlideSpec(slide *SlideSpec, index int, archetypeMap map[string]*Archetype) error {
	if slide.Archetype == "" {
		return fmt.Errorf("slide %d: archetype must be specified", index)
	}

	// Find the archetype
	archetype, ok := archetypeMap[slide.Archetype]
	if !ok {
		return fmt.Errorf("slide %d: unknown archetype %q", index, slide.Archetype)
	}

	if len(slide.Content) == 0 {
		return fmt.Errorf("slide %d: must provide content for at least one slot", index)
	}

	// Index required slots by ID
	requiredSlots := make(map[string]*Slot)
	for i := range archetype.Slots {
		slot := &archetype.Slots[i]
		if slot.Required {
			requiredSlots[slot.ID] = slot
		}
	}

	// Index all slots by ID
	allSlots := make(map[string]*Slot)
	for i := range archetype.Slots {
		slot := &archetype.Slots[i]
		allSlots[slot.ID] = slot
	}

	// Validate provided content
	for slotID, content := range slide.Content {
		// Check if slot exists
		slot, ok := allSlots[slotID]
		if !ok {
			// Check by slot name as fallback
			found := false
			for _, s := range archetype.Slots {
				if s.Name == slotID {
					found = true
					break
				}
			}
			if !found {
				return fmt.Errorf("slide %d: unknown slot %q in archetype %q", index, slotID, slide.Archetype)
			}
		}

		// Validate content type matches slot kind if slot exists
		if slot != nil && content != nil {
			if err := validateSlotContent(slotID, content, slot); err != nil {
				return fmt.Errorf("slide %d: %w", index, err)
			}
		}
	}

	// Verify all required slots are provided
	for requiredSlotID := range requiredSlots {
		if _, provided := slide.Content[requiredSlotID]; !provided {
			return fmt.Errorf("slide %d: required slot %q not provided", index, requiredSlotID)
		}
	}

	return nil
}

// validateSlotContent validates that content matches the slot kind
func validateSlotContent(slotID string, content interface{}, slot *Slot) error {
	// Content can be a string for simple text, or a structured object
	switch content := content.(type) {
	case string:
		// Strings are valid for text/richText/bullets/notes slots and for image paths.
		switch slot.Kind {
		case SlotKindText, SlotKindRichText, SlotKindBullets, SlotKindNotes, SlotKindImage:
			return nil
		default:
			return fmt.Errorf("slot %q expects kind %s, but received string content", slotID, slot.Kind)
		}

	case map[string]interface{}:
		// Structured content for complex slots
		contentType, ok := content["type"].(string)
		if !ok {
			contentType = string(slot.Kind) // Use slot kind as default
		}

		// Validate content type matches slot kind
		if contentType != string(slot.Kind) {
			return fmt.Errorf("slot %q: content type %q doesn't match slot kind %s", slotID, contentType, slot.Kind)
		}

		return nil

	case nil:
		// Nil content is acceptable for non-required slots
		return nil

	default:
		// Any other type is accepted but may fail at compile time
		return nil
	}
}

// ValidateSlideSpecAgainstManifest validates a slide spec comprehensively
func ValidateSlideSpecAgainstManifest(slide *SlideSpec, index int, manifest *TemplateManifest) error {
	if slide == nil {
		return fmt.Errorf("slide %d: spec is nil", index)
	}

	if slide.Archetype == "" {
		return fmt.Errorf("slide %d: archetype must be specified", index)
	}

	// Find archetype
	var archetype *Archetype
	for i := range manifest.Archetypes {
		if manifest.Archetypes[i].ID == slide.Archetype {
			archetype = &manifest.Archetypes[i]
			break
		}
	}

	if archetype == nil {
		return fmt.Errorf("slide %d: unknown archetype %q", index, slide.Archetype)
	}

	// Validate all provided slots exist
	for slotID := range slide.Content {
		found := false
		for _, slot := range archetype.Slots {
			if slot.ID == slotID || slot.Name == slotID {
				found = true
				break
			}
		}
		if !found {
			return fmt.Errorf("slide %d: unknown slot %q", index, slotID)
		}
	}

	// Validate required slots are provided
	for _, slot := range archetype.Slots {
		if slot.Required {
			if _, ok := slide.Content[slot.ID]; !ok {
				if _, ok := slide.Content[slot.Name]; !ok {
					return fmt.Errorf("slide %d: required slot %q not provided", index, slot.ID)
				}
			}
		}
	}

	return nil
}
