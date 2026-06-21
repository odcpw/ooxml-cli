package template

import (
	"testing"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TestCaptureOptions tests creation and defaults
func TestCaptureOptions(t *testing.T) {
	opts := CaptureOptions{
		Name:         "Test Template",
		Description:  "A test template",
		Author:       "Test Author",
		Organization: "Test Org",
	}

	if opts.Name != "Test Template" {
		t.Errorf("expected name 'Test Template', got %q", opts.Name)
	}

	if opts.Description != "A test template" {
		t.Errorf("expected description 'A test template', got %q", opts.Description)
	}
}

// TestConvertBounds tests boundary conversion
func TestConvertBounds(t *testing.T) {
	modelBounds := &model.Bounds{
		X:  100,
		Y:  200,
		CX: 300,
		CY: 400,
	}

	templateBounds := convertBounds(modelBounds)

	if templateBounds == nil {
		t.Error("expected non-nil bounds")
		return
	}

	if templateBounds.X != 100 || templateBounds.Y != 200 ||
		templateBounds.CX != 300 || templateBounds.CY != 400 {
		t.Errorf("bounds mismatch: got %v, want {X:100 Y:200 CX:300 CY:400}", templateBounds)
	}
}

// TestConvertBoundsNil tests nil boundary handling
func TestConvertBoundsNil(t *testing.T) {
	result := convertBounds(nil)
	if result != nil {
		t.Errorf("expected nil for nil input, got %v", result)
	}
}

// TestArchetypeValidation tests archetype creation and validation
func TestArchetypeValidation(t *testing.T) {
	arch := &Archetype{
		ID:   "test-arch",
		Name: "Test Archetype",
		Slots: []Slot{
			{
				ID:       "slot-1",
				Name:     "Title",
				Kind:     SlotKindText,
				Required: true,
			},
		},
	}

	if err := arch.Validate(); err != nil {
		t.Errorf("valid archetype failed validation: %v", err)
	}
}

// TestSlotKindValidation tests SlotKind validation
func TestSlotKindValidation(t *testing.T) {
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

	for _, test := range tests {
		if result := test.kind.IsValid(); result != test.valid {
			t.Errorf("SlotKind.IsValid() for %q: expected %v, got %v", test.kind, test.valid, result)
		}
	}
}

// TestVersionValidation tests Version validation
func TestVersionValidation(t *testing.T) {
	now := time.Now()
	v := &Version{
		Major:     1,
		Minor:     2,
		Patch:     3,
		CreatedAt: now,
	}

	if err := v.Validate(); err != nil {
		t.Errorf("valid version failed validation: %v", err)
	}

	// Test negative versions
	v.Minor = -1
	if err := v.Validate(); err == nil {
		t.Error("negative minor version should fail validation")
	}
}

// TestVersionString tests version string formatting
func TestVersionString(t *testing.T) {
	v := &Version{
		Major:     1,
		Minor:     2,
		Patch:     3,
		CreatedAt: time.Now(),
	}

	expected := "1.2.3"
	if result := v.String(); result != expected {
		t.Errorf("Version.String() expected %q, got %q", expected, result)
	}
}
