package template

import (
	"testing"
	"time"
)

func createTestManifest() *TemplateManifest {
	now := time.Now()
	return &TemplateManifest{
		ManifestVersion: "1.0",
		Name:            "Test Template",
		Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
		CreatedAt:       now,
		ModifiedAt:      now,
		Archetypes: []Archetype{
			{
				ID:   "title-slide",
				Name: "Title Slide",
				Slots: []Slot{
					{ID: "title", Name: "Title", Kind: SlotKindText, Required: true},
					{ID: "subtitle", Name: "Subtitle", Kind: SlotKindText, Required: false},
				},
			},
			{
				ID:   "content-slide",
				Name: "Content Slide",
				Slots: []Slot{
					{ID: "title", Name: "Title", Kind: SlotKindText, Required: true},
					{ID: "body", Name: "Body", Kind: SlotKindBullets, Required: true},
					{ID: "image", Name: "Image", Kind: SlotKindImage, Required: false},
				},
			},
		},
	}
}

func TestParseCompilationSpecYAML(t *testing.T) {
	yamlData := `
version: "1.0"
title: "My Presentation"
slides:
  - archetype: title-slide
    content:
      title: "Welcome"
      subtitle: "Subtitle"
  - archetype: content-slide
    content:
      title: "Content"
      body:
        type: bullets
        value:
          items:
            - "Point 1"
            - "Point 2"
`

	spec, err := ParseCompilationSpec([]byte(yamlData))
	if err != nil {
		t.Fatalf("Failed to parse YAML spec: %v", err)
	}

	if spec == nil {
		t.Fatal("Spec is nil")
	}

	if len(spec.Slides) != 2 {
		t.Errorf("Expected 2 slides, got %d", len(spec.Slides))
	}

	if spec.Slides[0].Archetype != "title-slide" {
		t.Errorf("First slide archetype: expected 'title-slide', got %q", spec.Slides[0].Archetype)
	}

	if spec.Slides[1].Archetype != "content-slide" {
		t.Errorf("Second slide archetype: expected 'content-slide', got %q", spec.Slides[1].Archetype)
	}
}

func TestValidateCompilationSpecValid(t *testing.T) {
	manifest := createTestManifest()

	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title":    "Welcome",
					"subtitle": "Subtitle",
				},
			},
			{
				Archetype: "content-slide",
				Content: map[string]interface{}{
					"title": "Content",
					"body":  "• Point 1\n• Point 2",
				},
			},
		},
	}

	err := ValidateCompilationSpec(spec, manifest)
	if err != nil {
		t.Errorf("Valid spec rejected: %v", err)
	}
}

func TestValidateCompilationSpecMissingRequiredSlot(t *testing.T) {
	manifest := createTestManifest()

	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "content-slide",
				Content: map[string]interface{}{
					"title": "Content",
					// Missing required "body" slot
				},
			},
		},
	}

	err := ValidateCompilationSpec(spec, manifest)
	if err == nil {
		t.Error("Missing required slot was not caught")
	}
	if err.Error() != "slide 0: required slot \"body\" not provided" {
		t.Errorf("Wrong error message: %v", err)
	}
}

func TestValidateCompilationSpecUnknownArchetype(t *testing.T) {
	manifest := createTestManifest()

	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "unknown-archetype",
				Content: map[string]interface{}{
					"title": "Content",
				},
			},
		},
	}

	err := ValidateCompilationSpec(spec, manifest)
	if err == nil {
		t.Error("Unknown archetype was not caught")
	}
	if err.Error() != "slide 0: unknown archetype \"unknown-archetype\"" {
		t.Errorf("Wrong error message: %v", err)
	}
}

func TestValidateCompilationSpecUnknownSlot(t *testing.T) {
	manifest := createTestManifest()

	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "content-slide",
				Content: map[string]interface{}{
					"title":        "Content",
					"body":         "Content",
					"unknown-slot": "Value",
				},
			},
		},
	}

	err := ValidateCompilationSpec(spec, manifest)
	if err == nil {
		t.Error("Unknown slot was not caught")
	}
	if err.Error() != "slide 0: unknown slot \"unknown-slot\" in archetype \"content-slide\"" {
		t.Errorf("Wrong error message: %v", err)
	}
}

func TestValidateCompilationSpecEmptySlides(t *testing.T) {
	manifest := createTestManifest()

	spec := &CompilationSpec{
		Slides: []SlideSpec{},
	}

	err := ValidateCompilationSpec(spec, manifest)
	if err == nil {
		t.Error("Empty slides list was not caught")
	}
}

func TestValidateCompilationSpecNilManifest(t *testing.T) {
	spec := &CompilationSpec{
		Slides: []SlideSpec{
			{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title": "Welcome",
				},
			},
		},
	}

	err := ValidateCompilationSpec(spec, nil)
	if err == nil {
		t.Error("Nil manifest was not caught")
	}
}

func TestValidateCompilationSpecNilSpec(t *testing.T) {
	manifest := createTestManifest()

	err := ValidateCompilationSpec(nil, manifest)
	if err == nil {
		t.Error("Nil spec was not caught")
	}
}

func TestSlotContentValidation(t *testing.T) {
	tests := []struct {
		name     string
		slotKind SlotKind
		content  interface{}
		wantErr  bool
	}{
		{
			name:     "string content for text slot",
			slotKind: SlotKindText,
			content:  "Hello world",
			wantErr:  false,
		},
		{
			name:     "string content for richText slot",
			slotKind: SlotKindRichText,
			content:  "Formatted text",
			wantErr:  false,
		},
		{
			name:     "string content for bullets slot",
			slotKind: SlotKindBullets,
			content:  "• Point 1",
			wantErr:  false,
		},
		{
			name:     "nil content",
			slotKind: SlotKindText,
			content:  nil,
			wantErr:  false,
		},
		{
			name:     "map content with type",
			slotKind: SlotKindImage,
			content: map[string]interface{}{
				"type": "image",
				"path": "/path/to/image.png",
			},
			wantErr: false,
		},
		{
			name:     "string content for image slot",
			slotKind: SlotKindImage,
			content:  "image-path",
			wantErr:  false,
		},
		{
			name:     "string content for table slot (mismatch)",
			slotKind: SlotKindTable,
			content:  "some data",
			wantErr:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			slot := &Slot{ID: "test", Name: "Test", Kind: tt.slotKind}
			err := validateSlotContent("test", tt.content, slot)
			if (err != nil) != tt.wantErr {
				t.Errorf("validateSlotContent() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestValidateSlideSpecAgainstManifest(t *testing.T) {
	manifest := createTestManifest()

	tests := []struct {
		name    string
		slide   *SlideSpec
		index   int
		wantErr bool
	}{
		{
			name: "valid slide",
			slide: &SlideSpec{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title": "Welcome",
				},
			},
			index:   0,
			wantErr: false,
		},
		{
			name: "missing archetype",
			slide: &SlideSpec{
				Archetype: "",
				Content: map[string]interface{}{
					"title": "Welcome",
				},
			},
			index:   0,
			wantErr: true,
		},
		{
			name: "unknown archetype",
			slide: &SlideSpec{
				Archetype: "unknown",
				Content: map[string]interface{}{
					"title": "Welcome",
				},
			},
			index:   0,
			wantErr: true,
		},
		{
			name: "unknown slot",
			slide: &SlideSpec{
				Archetype: "title-slide",
				Content: map[string]interface{}{
					"title":       "Welcome",
					"unknown-key": "value",
				},
			},
			index:   0,
			wantErr: true,
		},
		{
			name: "missing required slot",
			slide: &SlideSpec{
				Archetype: "content-slide",
				Content: map[string]interface{}{
					"title": "Content",
					// Missing required "body" slot
				},
			},
			index:   0,
			wantErr: true,
		},
		{
			name:    "nil slide",
			slide:   nil,
			index:   0,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateSlideSpecAgainstManifest(tt.slide, tt.index, manifest)
			if (err != nil) != tt.wantErr {
				t.Errorf("ValidateSlideSpecAgainstManifest() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestCompilationSpecDocumentation(t *testing.T) {
	// This test documents the expected spec format
	exampleYAML := `# Compilation spec for template-driven deck generation
version: "1.0"
title: "Generated Presentation"
author: "User Name"
description: "Auto-generated from template"

# List of slides to generate
slides:
  # Slide 1: Title Slide
  - archetype: title-slide
    content:
      title: "My Presentation"
      subtitle: "Subtitle line"
    notes: "Optional speaker notes"

  # Slide 2: Content Slide
  - archetype: content-slide
    content:
      title: "First Topic"
      body:
        type: bullets
        items:
          - "First point"
          - "Second point"
          - "Third point"
      image:
        path: "images/photo.png"
        description: "Optional description"

  # Slide 3: Content Slide with table
  - archetype: content-slide
    content:
      title: "Data Table"
      body: "Simple bullet point"
      # Tables can be specified as structured data
      # (exact format depends on table slot kind)

# Optional theme overrides
themeOverrides:
  colors:
    accent1: "FF0000"
    accent2: "00FF00"
  majorFont: "Arial"
  minorFont: "Helvetica"
`

	// Just verify it parses without error
	spec, err := ParseCompilationSpec([]byte(exampleYAML))
	if err != nil {
		t.Fatalf("Failed to parse example spec: %v", err)
	}

	if spec == nil || len(spec.Slides) < 3 {
		t.Error("Example spec should have 3+ slides")
	}
}
