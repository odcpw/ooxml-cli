package template

import (
	"encoding/json"
	"testing"
	"time"
)

func TestSlotKindValid_Manifest(t *testing.T) {
	tests := []struct {
		kind  SlotKind
		valid bool
	}{
		{SlotKindText, true},
		{SlotKindRichText, true},
		{SlotKindBullets, true},
		{SlotKindImage, true},
		{SlotKindTable, true},
		{SlotKindNotes, true},
		{SlotKind("invalid"), false},
		{SlotKind(""), false},
	}

	for _, tt := range tests {
		t.Run(string(tt.kind), func(t *testing.T) {
			if tt.kind.IsValid() != tt.valid {
				t.Errorf("SlotKind(%q).IsValid() = %v, want %v", tt.kind, tt.kind.IsValid(), tt.valid)
			}
		})
	}
}

func TestVersionString_Manifest(t *testing.T) {
	tests := []struct {
		version *Version
		want    string
	}{
		{&Version{Major: 1, Minor: 0, Patch: 0}, "1.0.0"},
		{&Version{Major: 2, Minor: 3, Patch: 4}, "2.3.4"},
		{&Version{Major: 0, Minor: 0, Patch: 1}, "0.0.1"},
	}

	for _, tt := range tests {
		t.Run(tt.want, func(t *testing.T) {
			if got := tt.version.String(); got != tt.want {
				t.Errorf("Version.String() = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestVersionValidate_Manifest(t *testing.T) {
	now := time.Now()

	tests := []struct {
		name    string
		version *Version
		wantErr bool
	}{
		{
			name:    "valid version",
			version: &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
			wantErr: false,
		},
		{
			name:    "zero version",
			version: &Version{Major: 0, Minor: 0, Patch: 0, CreatedAt: now},
			wantErr: false,
		},
		{
			name:    "nil version",
			version: nil,
			wantErr: true,
		},
		{
			name:    "negative major",
			version: &Version{Major: -1, Minor: 0, Patch: 0, CreatedAt: now},
			wantErr: true,
		},
		{
			name:    "negative minor",
			version: &Version{Major: 0, Minor: -1, Patch: 0, CreatedAt: now},
			wantErr: true,
		},
		{
			name:    "negative patch",
			version: &Version{Major: 0, Minor: 0, Patch: -1, CreatedAt: now},
			wantErr: true,
		},
		{
			name:    "zero createdAt",
			version: &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: time.Time{}},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.version.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Version.Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestSlotValidate(t *testing.T) {
	pos := 10
	aspectRatio := 1.5

	tests := []struct {
		name    string
		slot    *Slot
		wantErr bool
	}{
		{
			name: "valid text slot",
			slot: &Slot{
				ID:       "text-1",
				Name:     "Title",
				Kind:     SlotKindText,
				Required: true,
				Bounds:   &Bounds{X: 0, Y: 0, CX: 1000000, CY: 500000},
			},
			wantErr: false,
		},
		{
			name: "valid table slot with dimensions",
			slot: &Slot{
				ID:        "table-1",
				Name:      "Data Table",
				Kind:      SlotKindTable,
				TableRows: &pos,
				TableCols: &pos,
				Bounds:    &Bounds{X: 0, Y: 0, CX: 5000000, CY: 3000000},
			},
			wantErr: false,
		},
		{
			name: "valid image slot with aspect ratio",
			slot: &Slot{
				ID:          "image-1",
				Name:        "Hero Image",
				Kind:        SlotKindImage,
				AspectRatio: &aspectRatio,
				Bounds:      &Bounds{X: 0, Y: 0, CX: 2000000, CY: 2000000},
			},
			wantErr: false,
		},
		{
			name:    "nil slot",
			slot:    nil,
			wantErr: true,
		},
		{
			name: "empty ID",
			slot: &Slot{
				ID:   "",
				Name: "Title",
				Kind: SlotKindText,
			},
			wantErr: true,
		},
		{
			name: "empty name",
			slot: &Slot{
				ID:   "slot-1",
				Name: "",
				Kind: SlotKindText,
			},
			wantErr: true,
		},
		{
			name: "invalid kind",
			slot: &Slot{
				ID:   "slot-1",
				Name: "Slot",
				Kind: SlotKind("invalid"),
			},
			wantErr: true,
		},
		{
			name: "zero bounds width",
			slot: &Slot{
				ID:     "slot-1",
				Name:   "Slot",
				Kind:   SlotKindText,
				Bounds: &Bounds{X: 0, Y: 0, CX: 0, CY: 100},
			},
			wantErr: true,
		},
		{
			name: "negative bounds height",
			slot: &Slot{
				ID:     "slot-1",
				Name:   "Slot",
				Kind:   SlotKindText,
				Bounds: &Bounds{X: 0, Y: 0, CX: 100, CY: -100},
			},
			wantErr: true,
		},
		{
			name: "zero table rows",
			slot: &Slot{
				ID:        "table-1",
				Name:      "Table",
				Kind:      SlotKindTable,
				TableRows: &pos,
				TableCols: new(int), // Initialize to 0
			},
			wantErr: true,
		},
		{
			name: "negative aspect ratio",
			slot: &Slot{
				ID:          "image-1",
				Name:        "Image",
				Kind:        SlotKindImage,
				AspectRatio: new(float64), // Initialize to 0.0
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.slot.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Slot.Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestArchetypeValidate(t *testing.T) {
	validSlot := Slot{
		ID:   "slot-1",
		Name: "Title",
		Kind: SlotKindText,
	}

	tests := []struct {
		name      string
		archetype *Archetype
		wantErr   bool
	}{
		{
			name: "valid archetype",
			archetype: &Archetype{
				ID:    "title-slide",
				Name:  "Title Slide",
				Slots: []Slot{validSlot},
			},
			wantErr: false,
		},
		{
			name:      "nil archetype",
			archetype: nil,
			wantErr:   true,
		},
		{
			name: "empty ID",
			archetype: &Archetype{
				ID:    "",
				Name:  "Slide",
				Slots: []Slot{validSlot},
			},
			wantErr: true,
		},
		{
			name: "empty name",
			archetype: &Archetype{
				ID:    "slide-1",
				Name:  "",
				Slots: []Slot{validSlot},
			},
			wantErr: true,
		},
		{
			name: "no slots",
			archetype: &Archetype{
				ID:    "slide-1",
				Name:  "Slide",
				Slots: []Slot{},
			},
			wantErr: true,
		},
		{
			name: "duplicate slot IDs",
			archetype: &Archetype{
				ID:   "slide-1",
				Name: "Slide",
				Slots: []Slot{
					{ID: "slot-1", Name: "Slot 1", Kind: SlotKindText},
					{ID: "slot-1", Name: "Slot 2", Kind: SlotKindText},
				},
			},
			wantErr: true,
		},
		{
			name: "invalid slot",
			archetype: &Archetype{
				ID:   "slide-1",
				Name: "Slide",
				Slots: []Slot{
					{ID: "", Name: "Invalid", Kind: SlotKindText},
				},
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.archetype.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Archetype.Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestTemplateManifestValidate(t *testing.T) {
	now := time.Now()
	validSlot := Slot{
		ID:   "slot-1",
		Name: "Title",
		Kind: SlotKindText,
	}
	validArchetype := Archetype{
		ID:    "slide-1",
		Name:  "Slide",
		Slots: []Slot{validSlot},
	}

	tests := []struct {
		name     string
		manifest *TemplateManifest
		wantErr  bool
	}{
		{
			name: "valid manifest",
			manifest: &TemplateManifest{
				ManifestVersion: "1.0",
				Name:            "My Template",
				Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
				CreatedAt:       now,
				ModifiedAt:      now,
				Archetypes:      []Archetype{validArchetype},
			},
			wantErr: false,
		},
		{
			name:     "nil manifest",
			manifest: nil,
			wantErr:  true,
		},
		{
			name: "empty name",
			manifest: &TemplateManifest{
				ManifestVersion: "1.0",
				Name:            "",
				Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
				Archetypes:      []Archetype{validArchetype},
			},
			wantErr: true,
		},
		{
			name: "empty manifestVersion",
			manifest: &TemplateManifest{
				ManifestVersion: "",
				Name:            "Template",
				Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
				Archetypes:      []Archetype{validArchetype},
			},
			wantErr: true,
		},
		{
			name: "nil version",
			manifest: &TemplateManifest{
				ManifestVersion: "1.0",
				Name:            "Template",
				Version:         nil,
				Archetypes:      []Archetype{validArchetype},
			},
			wantErr: true,
		},
		{
			name: "no archetypes",
			manifest: &TemplateManifest{
				ManifestVersion: "1.0",
				Name:            "Template",
				Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
				Archetypes:      []Archetype{},
			},
			wantErr: true,
		},
		{
			name: "duplicate archetype IDs",
			manifest: &TemplateManifest{
				ManifestVersion: "1.0",
				Name:            "Template",
				Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
				Archetypes: []Archetype{
					{ID: "slide-1", Name: "Slide 1", Slots: []Slot{validSlot}},
					{ID: "slide-1", Name: "Slide 2", Slots: []Slot{validSlot}},
				},
			},
			wantErr: true,
		},
		{
			name: "invalid archetype",
			manifest: &TemplateManifest{
				ManifestVersion: "1.0",
				Name:            "Template",
				Version:         &Version{Major: 1, Minor: 0, Patch: 0, CreatedAt: now},
				Archetypes: []Archetype{
					{ID: "", Name: "Invalid", Slots: []Slot{validSlot}},
				},
			},
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.manifest.ValidateManifest()
			if (err != nil) != tt.wantErr {
				t.Errorf("TemplateManifest.ValidateManifest() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestManifestJSONSerialization(t *testing.T) {
	now := time.Now().UTC().Truncate(time.Millisecond)

	manifest := &TemplateManifest{
		ManifestVersion: "1.0",
		Name:            "Corporate Template",
		Description:     "Standard corporate presentation template",
		Version: &Version{
			Major:     1,
			Minor:     0,
			Patch:     0,
			CreatedAt: now,
			Notes:     "Initial version",
		},
		CreatedAt:    now,
		ModifiedAt:   now,
		Author:       "Jane Doe",
		Organization: "Acme Corp",
		Archetypes: []Archetype{
			{
				ID:          "title-slide",
				Name:        "Title Slide",
				Description: "Main title slide with company logo",
				Slots: []Slot{
					{
						ID:             "title",
						Name:           "Title",
						Kind:           SlotKindText,
						Bounds:         &Bounds{X: 0, Y: 1000000, CX: 9000000, CY: 1500000},
						Required:       true,
						PlaceholderKey: "title",
					},
					{
						ID:             "subtitle",
						Name:           "Subtitle",
						Kind:           SlotKindText,
						Bounds:         &Bounds{X: 0, Y: 2500000, CX: 9000000, CY: 1000000},
						Required:       false,
						PlaceholderKey: "subtitle",
					},
				},
			},
			{
				ID:   "content-slide",
				Name: "Content Slide",
				Slots: []Slot{
					{
						ID:       "title",
						Name:     "Title",
						Kind:     SlotKindText,
						Required: true,
					},
					{
						ID:       "body",
						Name:     "Body Text",
						Kind:     SlotKindBullets,
						Required: true,
					},
				},
			},
		},
		Notes: "Maintained by the Communications team",
	}

	// Test JSON marshaling
	jsonData, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		t.Fatalf("failed to marshal manifest to JSON: %v", err)
	}

	// Test JSON unmarshaling
	var decoded TemplateManifest
	err = json.Unmarshal(jsonData, &decoded)
	if err != nil {
		t.Fatalf("failed to unmarshal manifest from JSON: %v", err)
	}

	// Verify key fields match
	if decoded.Name != manifest.Name {
		t.Errorf("Name mismatch: got %q, want %q", decoded.Name, manifest.Name)
	}

	if decoded.ManifestVersion != manifest.ManifestVersion {
		t.Errorf("ManifestVersion mismatch: got %q, want %q", decoded.ManifestVersion, manifest.ManifestVersion)
	}

	if len(decoded.Archetypes) != len(manifest.Archetypes) {
		t.Errorf("Archetype count mismatch: got %d, want %d", len(decoded.Archetypes), len(manifest.Archetypes))
	}

	if decoded.Archetypes[0].ID != manifest.Archetypes[0].ID {
		t.Errorf("First archetype ID mismatch: got %q, want %q", decoded.Archetypes[0].ID, manifest.Archetypes[0].ID)
	}

	if len(decoded.Archetypes[0].Slots) != len(manifest.Archetypes[0].Slots) {
		t.Errorf("Slot count mismatch: got %d, want %d", len(decoded.Archetypes[0].Slots), len(manifest.Archetypes[0].Slots))
	}
}

func TestManifestRoundtrip(t *testing.T) {
	// Create a manifest
	now := time.Now().UTC().Truncate(time.Millisecond)
	original := &TemplateManifest{
		ManifestVersion: "1.0",
		Name:            "Test Template",
		Version:         &Version{Major: 1, Minor: 2, Patch: 3, CreatedAt: now},
		CreatedAt:       now,
		ModifiedAt:      now,
		Archetypes: []Archetype{
			{
				ID:   "arch-1",
				Name: "Arch 1",
				Slots: []Slot{
					{ID: "slot-1", Name: "Slot 1", Kind: SlotKindText},
				},
			},
		},
	}

	// Validate the original
	if err := original.ValidateManifest(); err != nil {
		t.Fatalf("original manifest is invalid: %v", err)
	}

	// Marshal to JSON
	jsonData, err := json.Marshal(original)
	if err != nil {
		t.Fatalf("failed to marshal: %v", err)
	}

	// Unmarshal back
	decoded := &TemplateManifest{}
	if err := json.Unmarshal(jsonData, decoded); err != nil {
		t.Fatalf("failed to unmarshal: %v", err)
	}

	// Validate the decoded version
	if err := decoded.ValidateManifest(); err != nil {
		t.Fatalf("decoded manifest is invalid: %v", err)
	}

	// Verify key properties
	if decoded.Name != original.Name {
		t.Errorf("Name changed: %q != %q", decoded.Name, original.Name)
	}

	if decoded.Version.String() != original.Version.String() {
		t.Errorf("Version changed: %s != %s", decoded.Version.String(), original.Version.String())
	}

	if len(decoded.Archetypes) != len(original.Archetypes) {
		t.Errorf("Archetype count changed: %d != %d", len(decoded.Archetypes), len(original.Archetypes))
	}
}

func TestBackwardCompatibility(t *testing.T) {
	// Ensure that adding new optional fields doesn't break existing manifests
	minimalJSON := `{
		"manifestVersion": "1.0",
		"name": "Minimal Template",
		"version": {
			"major": 1,
			"minor": 0,
			"patch": 0,
			"createdAt": "2025-01-01T00:00:00Z"
		},
		"createdAt": "2025-01-01T00:00:00Z",
		"modifiedAt": "2025-01-01T00:00:00Z",
		"archetypes": [
			{
				"id": "arch-1",
				"name": "Arch 1",
				"slots": [
					{
						"id": "slot-1",
						"name": "Slot 1",
						"kind": "text"
					}
				]
			}
		]
	}`

	var manifest TemplateManifest
	if err := json.Unmarshal([]byte(minimalJSON), &manifest); err != nil {
		t.Fatalf("failed to unmarshal minimal manifest: %v", err)
	}

	if manifest.Name != "Minimal Template" {
		t.Errorf("Name mismatch: %q", manifest.Name)
	}

	if len(manifest.Archetypes) != 1 {
		t.Errorf("Expected 1 archetype, got %d", len(manifest.Archetypes))
	}

	if manifest.Archetypes[0].Slots[0].Kind != SlotKindText {
		t.Errorf("Slot kind mismatch: %q", manifest.Archetypes[0].Slots[0].Kind)
	}
}
