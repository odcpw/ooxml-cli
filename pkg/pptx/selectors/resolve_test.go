package selectors

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func TestResolvePlaceholderKey(t *testing.T) {
	placeholders := []model.ShapeInfo{
		{ID: 1, Name: "Title 1"},
		{ID: 2, Name: "Body 1"},
		{ID: 3, Name: "Body 2"},
	}

	placeholderKeys := map[int]string{
		1: "title",
		2: "body:0",
		3: "body:1",
	}

	placeholderRoles := map[int]string{
		1: "title",
		2: "body",
		3: "body",
	}

	ctx := NewResolutionContext(placeholders, placeholders, placeholderKeys, placeholderRoles, 10)

	tests := []struct {
		selector string
		expected int
		notFound bool
	}{
		{"title", 1, false},
		{"body:0", 2, false},
		{"body:1", 3, false},
		{"invalid", 0, true},
	}

	for _, tt := range tests {
		t.Run(tt.selector, func(t *testing.T) {
			sel := &PlaceholderKeySelector{Key: tt.selector}
			result := ResolveForShape(sel, ctx)

			if tt.notFound {
				if result.HasMatches() {
					t.Errorf("expected no matches, got %v", result.Matches)
				}
				if !result.IsNotFound() {
					t.Errorf("expected IsNotFound=true")
				}
			} else {
				if !result.HasMatches() {
					t.Errorf("expected matches, got %v", result.NotFoundError)
				}
				if len(result.Matches) != 1 || result.Matches[0] != tt.expected {
					t.Errorf("expected match %d, got %v", tt.expected, result.Matches)
				}
			}
		})
	}
}

func TestResolvePlaceholderType(t *testing.T) {
	placeholders := []model.ShapeInfo{
		{ID: 1, Name: "Title 1"},
		{ID: 2, Name: "Body 1"},
		{ID: 3, Name: "Body 2"},
		{ID: 4, Name: "Chart 1"},
	}

	placeholderKeys := map[int]string{
		1: "title",
		2: "body:0",
		3: "body:1",
		4: "chart:0",
	}

	placeholderRoles := map[int]string{
		1: "title",
		2: "body",
		3: "body",
		4: "chart",
	}

	ctx := NewResolutionContext(placeholders, placeholders, placeholderKeys, placeholderRoles, 10)

	tests := []struct {
		phType   string
		expected []int
	}{
		{"title", []int{1}},
		{"body", []int{2, 3}},
		{"chart", []int{4}},
		{"pic", []int{}},
	}

	for _, tt := range tests {
		t.Run(tt.phType, func(t *testing.T) {
			sel := &PlaceholderTypeSelector{Role: tt.phType}
			result := ResolveForShape(sel, ctx)

			if len(tt.expected) == 0 {
				if result.HasMatches() {
					t.Errorf("expected no matches, got %v", result.Matches)
				}
			} else {
				if len(result.Matches) != len(tt.expected) {
					t.Errorf("expected %d matches, got %d", len(tt.expected), len(result.Matches))
				}
				for i, expected := range tt.expected {
					if i < len(result.Matches) && result.Matches[i] != expected {
						t.Errorf("match %d: expected %d, got %d", i, expected, result.Matches[i])
					}
				}
			}
		})
	}
}

func TestResolveShapeName(t *testing.T) {
	shapes := []model.ShapeInfo{
		{ID: 1, Name: "Title"},
		{ID: 2, Name: "Body"},
		{ID: 3, Name: "Image"},
	}

	ctx := NewResolutionContext(shapes, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	tests := []struct {
		name     string
		expected int
		notFound bool
	}{
		{"Title", 1, false},
		{"Body", 2, false},
		{"Image", 3, false},
		{"Unknown", 0, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sel := &ShapeNameSelector{Name: tt.name}
			result := ResolveForShape(sel, ctx)

			if tt.notFound {
				if result.HasMatches() {
					t.Errorf("expected no matches, got %v", result.Matches)
				}
			} else {
				if len(result.Matches) != 1 || result.Matches[0] != tt.expected {
					t.Errorf("expected match %d, got %v", tt.expected, result.Matches)
				}
			}
		})
	}
}

func TestResolveShapeID(t *testing.T) {
	shapes := []model.ShapeInfo{
		{ID: 1, Name: "Shape 1"},
		{ID: 2, Name: "Shape 2"},
		{ID: 5, Name: "Shape 5"},
	}

	ctx := NewResolutionContext(shapes, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	tests := []struct {
		id       int
		expected int
		notFound bool
	}{
		{1, 1, false},
		{2, 2, false},
		{5, 5, false},
		{10, 0, true},
	}

	for _, tt := range tests {
		t.Run(string(rune(tt.id)), func(t *testing.T) {
			sel := &ShapeIDSelector{ID: tt.id}
			result := ResolveForShape(sel, ctx)

			if tt.notFound {
				if result.HasMatches() {
					t.Errorf("expected no matches, got %v", result.Matches)
				}
			} else {
				if len(result.Matches) != 1 || result.Matches[0] != tt.expected {
					t.Errorf("expected match %d, got %v", tt.expected, result.Matches)
				}
			}
		})
	}
}

func TestResolveSlideNumber(t *testing.T) {
	ctx := NewResolutionContext([]model.ShapeInfo{}, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	tests := []struct {
		slideNum int
		expected []int
		notFound bool
	}{
		{1, []int{1}, false},
		{5, []int{5}, false},
		{10, []int{10}, false},
		{11, []int{}, true},
		{0, []int{}, true},
	}

	for _, tt := range tests {
		t.Run(string(rune(tt.slideNum)), func(t *testing.T) {
			sel := &SlideNumberSelector{Number: tt.slideNum}
			result := ResolveForSlides(sel, ctx)

			if tt.notFound {
				if result.HasMatches() {
					t.Errorf("expected no matches, got %v", result.Matches)
				}
			} else {
				if len(result.Matches) != len(tt.expected) {
					t.Errorf("expected %d matches, got %d", len(tt.expected), len(result.Matches))
				}
				for i, expected := range tt.expected {
					if i < len(result.Matches) && result.Matches[i] != expected {
						t.Errorf("match %d: expected %d, got %d", i, expected, result.Matches[i])
					}
				}
			}
		})
	}
}

func TestResolveSlideRange(t *testing.T) {
	ctx := NewResolutionContext([]model.ShapeInfo{}, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	tests := []struct {
		name     string
		ranges   []SlideRange
		expected []int
		notFound bool
	}{
		{
			name:     "single range",
			ranges:   []SlideRange{{Start: 1, End: 3}},
			expected: []int{1, 2, 3},
			notFound: false,
		},
		{
			name:     "multiple ranges",
			ranges:   []SlideRange{{Start: 1, End: 2}, {Start: 5, End: 6}},
			expected: []int{1, 2, 5, 6},
			notFound: false,
		},
		{
			name:     "mixed individual and range",
			ranges:   []SlideRange{{Start: 1, End: 1}, {Start: 3, End: 5}},
			expected: []int{1, 3, 4, 5},
			notFound: false,
		},
		{
			name:     "range exceeds total slides",
			ranges:   []SlideRange{{Start: 8, End: 15}},
			expected: []int{8, 9, 10},
			notFound: false,
		},
		{
			name:     "range out of bounds",
			ranges:   []SlideRange{{Start: 11, End: 15}},
			expected: []int{},
			notFound: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			sel := &SlideRangeSelector{Ranges: tt.ranges}
			result := ResolveForSlides(sel, ctx)

			if tt.notFound {
				if result.HasMatches() {
					t.Errorf("expected no matches, got %v", result.Matches)
				}
			} else {
				if len(result.Matches) != len(tt.expected) {
					t.Errorf("expected %d matches, got %d", len(tt.expected), len(result.Matches))
				}
				for i, expected := range tt.expected {
					if i < len(result.Matches) && result.Matches[i] != expected {
						t.Errorf("match %d: expected %d, got %d", i, expected, result.Matches[i])
					}
				}
			}
		})
	}
}

func TestResolveUnsupportedSelector(t *testing.T) {
	ctx := NewResolutionContext([]model.ShapeInfo{}, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	// Test resolving a slide selector for shapes
	sel := &SlideNumberSelector{Number: 1}
	result := ResolveForSlides(sel, ctx)
	if !result.HasMatches() {
		t.Errorf("expected slide 1 to match")
	}

	// Test resolving a shape selector for slides
	shapeSel := &ShapeIDSelector{ID: 1}
	shapeResult := ResolveForShape(shapeSel, ctx)
	if shapeResult.HasMatches() {
		t.Errorf("expected shape selector to not match in shape context")
	}
}

func TestSimpleShapeContext(t *testing.T) {
	shapes := []model.ShapeInfo{
		{ID: 1, Name: "Shape 1"},
		{ID: 2, Name: "Shape 2"},
		{ID: 3, Name: "Shape 1"}, // Duplicate name
	}

	ctx := NewSimpleShapeContext(shapes)

	t.Run("get by ID", func(t *testing.T) {
		shape := ctx.GetShapeByID(2)
		if shape == nil {
			t.Errorf("expected to find shape with ID 2")
		}
		if shape.Name != "Shape 2" {
			t.Errorf("expected name Shape 2, got %s", shape.Name)
		}
	})

	t.Run("get by name - single match", func(t *testing.T) {
		shapes := ctx.GetShapesByName("Shape 2")
		if len(shapes) != 1 {
			t.Errorf("expected 1 match, got %d", len(shapes))
		}
		if shapes[0].ID != 2 {
			t.Errorf("expected ID 2, got %d", shapes[0].ID)
		}
	})

	t.Run("get by name - multiple matches", func(t *testing.T) {
		shapes := ctx.GetShapesByName("Shape 1")
		if len(shapes) != 2 {
			t.Errorf("expected 2 matches, got %d", len(shapes))
		}
	})

	t.Run("get nonexistent by ID", func(t *testing.T) {
		shape := ctx.GetShapeByID(99)
		if shape != nil {
			t.Errorf("expected no match for nonexistent ID")
		}
	})

	t.Run("get nonexistent by name", func(t *testing.T) {
		shapes := ctx.GetShapesByName("Nonexistent")
		if len(shapes) != 0 {
			t.Errorf("expected no matches, got %d", len(shapes))
		}
	})
}

func TestSimplePlaceholderContext(t *testing.T) {
	placeholders := []model.ShapeInfo{
		{ID: 1, Name: "Title"},
		{ID: 2, Name: "Body 1"},
		{ID: 3, Name: "Body 2"},
	}

	placeholderKeys := map[int]string{
		1: "title",
		2: "body:0",
		3: "body:1",
	}

	placeholderRoles := map[int]string{
		1: "title",
		2: "body",
		3: "body",
	}

	ctx := NewSimplePlaceholderContext(placeholders, placeholderKeys, placeholderRoles)

	t.Run("get by key", func(t *testing.T) {
		ph := ctx.GetPlaceholderByKey("body:0")
		if ph == nil {
			t.Errorf("expected to find placeholder with key body:0")
		}
		if ph.ID != 2 {
			t.Errorf("expected ID 2, got %d", ph.ID)
		}
	})

	t.Run("get by type - single", func(t *testing.T) {
		phs := ctx.GetPlaceholdersByType("title")
		if len(phs) != 1 {
			t.Errorf("expected 1 match, got %d", len(phs))
		}
	})

	t.Run("get by type - multiple", func(t *testing.T) {
		phs := ctx.GetPlaceholdersByType("body")
		if len(phs) != 2 {
			t.Errorf("expected 2 matches, got %d", len(phs))
		}
	})

	t.Run("list all keys", func(t *testing.T) {
		keys := ctx.ListAllPlaceholderKeys()
		if len(keys) != 3 {
			t.Errorf("expected 3 keys, got %d", len(keys))
		}
	})

	t.Run("get nonexistent by key", func(t *testing.T) {
		ph := ctx.GetPlaceholderByKey("nonexistent")
		if ph != nil {
			t.Errorf("expected no match")
		}
	})

	t.Run("get nonexistent by type", func(t *testing.T) {
		phs := ctx.GetPlaceholdersByType("chart")
		if len(phs) != 0 {
			t.Errorf("expected no matches, got %d", len(phs))
		}
	})
}

func TestSimpleSlideContext(t *testing.T) {
	ctx := NewSimpleSlideContext(10)

	if ctx.GetTotalSlides() != 10 {
		t.Errorf("expected 10 slides, got %d", ctx.GetTotalSlides())
	}
}

func TestIntegrationPlaceholderSelection(t *testing.T) {
	// Test a typical layout with title, subtitle, and body placeholders
	placeholders := []model.ShapeInfo{
		{ID: 1, Name: "Title 1"},
		{ID: 2, Name: "Subtitle 1"},
		{ID: 3, Name: "Body 1"},
		{ID: 4, Name: "Body 2"},
	}

	placeholderKeys := map[int]string{
		1: "title",
		2: "subtitle",
		3: "body:0",
		4: "body:1",
	}

	placeholderRoles := map[int]string{
		1: "title",
		2: "subtitle",
		3: "body",
		4: "body",
	}

	ctx := NewResolutionContext(placeholders, placeholders, placeholderKeys, placeholderRoles, 10)

	tests := []struct {
		name     string
		selector Selector
		expected []int
	}{
		{
			name:     "select title by key",
			selector: &PlaceholderKeySelector{Key: "title"},
			expected: []int{1},
		},
		{
			name:     "select body:0 by key",
			selector: &PlaceholderKeySelector{Key: "body:0"},
			expected: []int{3},
		},
		{
			name:     "select all body by type",
			selector: &PlaceholderTypeSelector{Role: "body"},
			expected: []int{3, 4},
		},
		{
			name:     "select body:1 by key",
			selector: &PlaceholderKeySelector{Key: "body:1"},
			expected: []int{4},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ResolveForShape(tt.selector, ctx)
			if len(result.Matches) != len(tt.expected) {
				t.Errorf("expected %d matches, got %d", len(tt.expected), len(result.Matches))
				return
			}
			for i, expected := range tt.expected {
				if result.Matches[i] != expected {
					t.Errorf("match %d: expected %d, got %d", i, expected, result.Matches[i])
				}
			}
		})
	}
}

func TestIntegrationSlideSelection(t *testing.T) {
	ctx := NewResolutionContext([]model.ShapeInfo{}, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 20)

	tests := []struct {
		name     string
		selector Selector
		expected []int
	}{
		{
			name:     "single slide",
			selector: &SlideNumberSelector{Number: 1},
			expected: []int{1},
		},
		{
			name:     "range",
			selector: &SlideRangeSelector{Ranges: []SlideRange{{Start: 5, End: 7}}},
			expected: []int{5, 6, 7},
		},
		{
			name:     "multiple ranges",
			selector: &SlideRangeSelector{Ranges: []SlideRange{{Start: 1, End: 2}, {Start: 5, End: 5}, {Start: 10, End: 12}}},
			expected: []int{1, 2, 5, 10, 11, 12},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ResolveForSlides(tt.selector, ctx)
			if len(result.Matches) != len(tt.expected) {
				t.Errorf("expected %d matches, got %d", len(tt.expected), len(result.Matches))
				return
			}
			for i, expected := range tt.expected {
				if result.Matches[i] != expected {
					t.Errorf("match %d: expected %d, got %d", i, expected, result.Matches[i])
				}
			}
		})
	}
}

func TestWildcardAllPlaceholders(t *testing.T) {
	placeholders := []model.ShapeInfo{
		{ID: 1, Name: "Title 1"},
		{ID: 2, Name: "Body 1"},
		{ID: 3, Name: "Body 2"},
	}

	placeholderKeys := map[int]string{
		1: "title",
		2: "body:0",
		3: "body:1",
	}

	placeholderRoles := map[int]string{
		1: "title",
		2: "body",
		3: "body",
	}

	ctx := NewResolutionContext(placeholders, placeholders, placeholderKeys, placeholderRoles, 10)

	t.Run("@* should match all placeholders", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllPlaceholdersSelector{}, ctx)
		if len(result.Matches) != 3 {
			t.Errorf("expected 3 matches, got %d", len(result.Matches))
		}
	})

	t.Run("@all-placeholders should match all placeholders", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllPlaceholdersSelector{}, ctx)
		if len(result.Matches) != 3 {
			t.Errorf("expected 3 matches, got %d", len(result.Matches))
		}
	})
}

func TestWildcardAllShapes(t *testing.T) {
	shapes := []model.ShapeInfo{
		{ID: 1, Name: "Title", Type: model.ShapeTypeSP, IsPlaceholder: true},
		{ID: 2, Name: "Picture 1", Type: model.ShapeTypePic, IsPlaceholder: false},
		{ID: 3, Name: "Shape 1", Type: model.ShapeTypeSP, IsPlaceholder: false},
	}

	ctx := NewResolutionContext(shapes, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	t.Run("@all-shapes should match all shapes", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllShapesSelector{ExcludePlaceholders: false}, ctx)
		if len(result.Matches) != 3 {
			t.Errorf("expected 3 matches, got %d", len(result.Matches))
		}
	})

	t.Run("@all-shapes-nonph should exclude placeholders", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllShapesSelector{ExcludePlaceholders: true}, ctx)
		if len(result.Matches) != 2 {
			t.Errorf("expected 2 matches (excluding placeholder), got %d", len(result.Matches))
		}
	})
}

func TestWildcardAllPictures(t *testing.T) {
	shapes := []model.ShapeInfo{
		{ID: 1, Name: "Title", Type: model.ShapeTypeSP, IsPlaceholder: true},
		{ID: 2, Name: "Picture 1", Type: model.ShapeTypePic, IsPlaceholder: false},
		{ID: 3, Name: "Picture 2", Type: model.ShapeTypePic, IsPlaceholder: false},
		{ID: 4, Name: "Shape 1", Type: model.ShapeTypeSP, IsPlaceholder: false},
	}

	ctx := NewResolutionContext(shapes, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	t.Run("@all-pictures should match only pictures", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllPicturesSelector{}, ctx)
		if len(result.Matches) != 2 {
			t.Errorf("expected 2 matches, got %d", len(result.Matches))
		}
		// Check the IDs
		if result.Matches[0] != 2 || result.Matches[1] != 3 {
			t.Errorf("expected IDs 2 and 3, got %v", result.Matches)
		}
	})
}

func TestWildcardAllTables(t *testing.T) {
	shapes := []model.ShapeInfo{
		{ID: 1, Name: "Title", Type: model.ShapeTypeSP, IsPlaceholder: true},
		{ID: 2, Name: "Table 1", Type: model.ShapeTypeGraphicFrame, IsPlaceholder: false, TableInfo: &model.TableInfo{Rows: 2, Cols: 3}},
		{ID: 3, Name: "Picture 1", Type: model.ShapeTypePic, IsPlaceholder: false},
		{ID: 4, Name: "Table 2", Type: model.ShapeTypeGraphicFrame, IsPlaceholder: false, TableInfo: &model.TableInfo{Rows: 3, Cols: 2}},
	}

	ctx := NewResolutionContext(shapes, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	t.Run("@all-tables should match only tables", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllTablesSelector{}, ctx)
		if len(result.Matches) != 2 {
			t.Errorf("expected 2 matches, got %d", len(result.Matches))
		}
		if result.Matches[0] != 2 || result.Matches[1] != 4 {
			t.Errorf("expected IDs 2 and 4, got %v", result.Matches)
		}
	})
}

func TestWildcardEmptyResults(t *testing.T) {
	ctx := NewResolutionContext([]model.ShapeInfo{}, []model.ShapeInfo{}, map[int]string{}, map[int]string{}, 10)

	t.Run("@all-placeholders with no placeholders", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllPlaceholdersSelector{}, ctx)
		if result.HasMatches() {
			t.Errorf("expected no matches")
		}
		if !result.IsNotFound() {
			t.Errorf("expected IsNotFound() to be true")
		}
	})

	t.Run("@all-pictures with no pictures", func(t *testing.T) {
		result := ResolveForShape(&WildcardAllPicturesSelector{}, ctx)
		if result.HasMatches() {
			t.Errorf("expected no matches")
		}
		if !result.IsNotFound() {
			t.Errorf("expected IsNotFound() to be true")
		}
	})
}

func TestBackwardCompatibility(t *testing.T) {
	// Ensure existing selector types still work correctly
	placeholders := []model.ShapeInfo{
		{ID: 1, Name: "Title 1"},
		{ID: 2, Name: "Body 1"},
	}

	placeholderKeys := map[int]string{
		1: "title",
		2: "body:0",
	}

	placeholderRoles := map[int]string{
		1: "title",
		2: "body",
	}

	ctx := NewResolutionContext(placeholders, placeholders, placeholderKeys, placeholderRoles, 10)

	t.Run("PlaceholderKeySelector still works", func(t *testing.T) {
		result := ResolveForShape(&PlaceholderKeySelector{Key: "title"}, ctx)
		if len(result.Matches) != 1 || result.Matches[0] != 1 {
			t.Errorf("expected match ID 1, got %v", result.Matches)
		}
	})

	t.Run("PlaceholderTypeSelector still works", func(t *testing.T) {
		result := ResolveForShape(&PlaceholderTypeSelector{Role: "body"}, ctx)
		if len(result.Matches) != 1 || result.Matches[0] != 2 {
			t.Errorf("expected match ID 2, got %v", result.Matches)
		}
	})
}
