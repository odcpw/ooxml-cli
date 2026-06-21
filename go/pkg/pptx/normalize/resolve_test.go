package normalize

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// TestResolveFromSlideType tests that slide-level type is used when present.
func TestResolveFromSlideType(t *testing.T) {
	slidePh := &model.RawPlaceholder{
		Type: "title",
		Idx:  0,
	}

	resolved := ResolvePlaceholder(slidePh, nil, nil)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	if resolved.Raw.Type != "title" {
		t.Errorf("expected type=title, got %q", resolved.Raw.Type)
	}
}

// TestResolveFromLayoutType tests inheritance from layout when slide type is missing.
func TestResolveFromLayoutType(t *testing.T) {
	// Slide placeholder with no type, idx=1
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  1,
	}

	// Layout placeholder with type=body at idx=1
	layoutPhs := []*model.RawPlaceholder{
		{Type: "title", Idx: 0},
		{Type: "body", Idx: 1},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, nil)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	if resolved.Raw.Type != "body" {
		t.Errorf("expected type=body from layout, got %q", resolved.Raw.Type)
	}
}

// TestResolveFromMasterType tests fallback to master when both slide and layout types are missing.
func TestResolveFromMasterType(t *testing.T) {
	// Slide placeholder with no type, idx=1
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  1,
	}

	// Layout placeholder with no type at idx=1
	layoutPhs := []*model.RawPlaceholder{
		{Type: "title", Idx: 0},
		{Type: "", Idx: 1},
	}

	// Master placeholder with type=body at idx=1
	masterPhs := []*model.RawPlaceholder{
		{Type: "title", Idx: 0},
		{Type: "body", Idx: 1},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, masterPhs)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	if resolved.Raw.Type != "body" {
		t.Errorf("expected type=body from master, got %q", resolved.Raw.Type)
	}
}

// TestResolvePreferSlideOverLayout tests that slide type takes precedence over layout.
func TestResolvePreferSlideOverLayout(t *testing.T) {
	// Slide has type=pic
	slidePh := &model.RawPlaceholder{
		Type: "pic",
		Idx:  2,
	}

	// Layout has type=body at same idx
	layoutPhs := []*model.RawPlaceholder{
		{Type: "body", Idx: 2},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, nil)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	if resolved.Raw.Type != "pic" {
		t.Errorf("expected slide type pic to take precedence, got %q", resolved.Raw.Type)
	}
}

// TestResolvePreferLayoutOverMaster tests that layout type takes precedence over master.
func TestResolvePreferLayoutOverMaster(t *testing.T) {
	// Slide has no type
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  1,
	}

	// Layout has type=body
	layoutPhs := []*model.RawPlaceholder{
		{Type: "body", Idx: 1},
	}

	// Master has type=pic at same idx
	masterPhs := []*model.RawPlaceholder{
		{Type: "pic", Idx: 1},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, masterPhs)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	if resolved.Raw.Type != "body" {
		t.Errorf("expected layout type body to take precedence over master, got %q", resolved.Raw.Type)
	}
}

// TestResolveNonMatchingIdx tests that non-matching indexes are not used.
func TestResolveNonMatchingIdx(t *testing.T) {
	// Slide placeholder with idx=2 but no type
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  2,
	}

	// Layout has body at idx=1, not idx=2
	layoutPhs := []*model.RawPlaceholder{
		{Type: "body", Idx: 1},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, nil)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	// Type should remain empty since no matching layout placeholder
	if resolved.Raw.Type != "" {
		t.Errorf("expected empty type for non-matching idx, got %q", resolved.Raw.Type)
	}
}

// TestResolveNoIndexNoInheritance tests placeholder with no type and no idx.
func TestResolveNoIndexNoInheritance(t *testing.T) {
	// Slide placeholder with no type and invalid idx
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  -1,
	}

	// Even if layout/master have types, no inheritance without idx
	layoutPhs := []*model.RawPlaceholder{
		{Type: "body", Idx: 0},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, nil)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	// Type should remain empty
	if resolved.Raw.Type != "" {
		t.Errorf("expected empty type for placeholder without idx, got %q", resolved.Raw.Type)
	}
}

// TestResolveMultipleLayouts tests with multiple layout placeholders.
func TestResolveMultipleLayouts(t *testing.T) {
	// Slide at idx=3 with no type
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  3,
	}

	// Multiple layout placeholders
	layoutPhs := []*model.RawPlaceholder{
		{Type: "title", Idx: 0},
		{Type: "body", Idx: 1},
		{Type: "pic", Idx: 2},
		{Type: "tbl", Idx: 3},
	}

	resolved := ResolvePlaceholder(slidePh, layoutPhs, nil)

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	if resolved.Raw.Type != "tbl" {
		t.Errorf("expected type=tbl at idx=3, got %q", resolved.Raw.Type)
	}
}

// TestResolveNil tests that ResolvePlaceholder handles nil input.
func TestResolveNil(t *testing.T) {
	resolved := ResolvePlaceholder(nil, nil, nil)

	if resolved != nil {
		t.Errorf("expected nil for nil input, got %+v", resolved)
	}
}

// TestResolveEmptyPlaceholderLists tests with empty layout/master lists.
func TestResolveEmptyPlaceholderLists(t *testing.T) {
	slidePh := &model.RawPlaceholder{
		Type: "",
		Idx:  1,
	}

	resolved := ResolvePlaceholder(slidePh, []*model.RawPlaceholder{}, []*model.RawPlaceholder{})

	if resolved == nil {
		t.Fatal("ResolvePlaceholder returned nil")
	}

	// Type should remain empty with no matching placeholders
	if resolved.Raw.Type != "" {
		t.Errorf("expected empty type with empty lists, got %q", resolved.Raw.Type)
	}
}
