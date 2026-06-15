package normalize

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// MockLayoutContext is a test implementation of LayoutContext
type MockLayoutContext struct {
	uniqueRoles map[string]bool
}

func NewMockLayoutContext(uniqueRoles ...string) *MockLayoutContext {
	unique := make(map[string]bool)
	for _, role := range uniqueRoles {
		unique[role] = true
	}
	return &MockLayoutContext{uniqueRoles: unique}
}

func (m *MockLayoutContext) IsRoleUniqueInLayout(role string) bool {
	return m.uniqueRoles[role]
}

func TestGenerateKeyPriority1_UniqueRole(t *testing.T) {
	// Priority 1: Unique canonical role → {role}
	tests := []struct {
		name     string
		resolved model.ResolvedPlaceholder
		ctxRoles []string
		expected string
	}{
		{
			name: "title type, unique",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "title", Idx: -1},
				Role:    "title",
				ShapeID: 5,
			},
			ctxRoles: []string{"title"},
			expected: "title",
		},
		{
			name: "subtitle type, unique",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "subTitle", Idx: -1},
				Role:    "subtitle",
				ShapeID: 6,
			},
			ctxRoles: []string{"subtitle"},
			expected: "subtitle",
		},
		{
			name: "ctrTitle mapped to title, unique",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "ctrTitle", Idx: -1},
				Role:    "title",
				ShapeID: 7,
			},
			ctxRoles: []string{"title"},
			expected: "title",
		},
		{
			name: "footer type, unique",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "ftr", Idx: -1},
				Role:    "footer",
				ShapeID: 8,
			},
			ctxRoles: []string{"footer"},
			expected: "footer",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := NewMockLayoutContext(tt.ctxRoles...)
			result := GenerateKey(tt.resolved, ctx)
			if result != tt.expected {
				t.Errorf("GenerateKey() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestGenerateKeyPriority2_NonUniqueRoleWithIndex(t *testing.T) {
	// Priority 2: Non-unique role with index → {role}:{idx}
	tests := []struct {
		name     string
		resolved model.ResolvedPlaceholder
		expected string
	}{
		{
			name: "body with idx=0, non-unique",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "body", Idx: 0},
				Role:    "body",
				ShapeID: 10,
			},
			expected: "body:0",
		},
		{
			name: "body with idx=3, non-unique",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "body", Idx: 3},
				Role:    "body",
				ShapeID: 11,
			},
			expected: "body:3",
		},
		{
			name: "pic with idx=1",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "pic", Idx: 1},
				Role:    "pic",
				ShapeID: 12,
			},
			expected: "pic:1",
		},
		{
			name: "pic with idx=12",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "pic", Idx: 12},
				Role:    "pic",
				ShapeID: 13,
			},
			expected: "pic:12",
		},
		{
			name: "chart with idx=2",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "chart", Idx: 2},
				Role:    "chart",
				ShapeID: 14,
			},
			expected: "chart:2",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Use empty context so role is never unique
			ctx := NewMockLayoutContext()
			result := GenerateKey(tt.resolved, ctx)
			if result != tt.expected {
				t.Errorf("GenerateKey() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestGenerateKeyPriority3_NoTypeButHasIndex(t *testing.T) {
	// Priority 3: No type, has index → ph:{idx}
	tests := []struct {
		name     string
		resolved model.ResolvedPlaceholder
		expected string
	}{
		{
			name: "no type, idx=0",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "", Idx: 0},
				Role:    "",
				ShapeID: 20,
			},
			expected: "ph:0",
		},
		{
			name: "no type, idx=1",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "", Idx: 1},
				Role:    "",
				ShapeID: 21,
			},
			expected: "ph:1",
		},
		{
			name: "no type, idx=11",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "", Idx: 11},
				Role:    "",
				ShapeID: 22,
			},
			expected: "ph:11",
		},
		{
			name: "unknown type, idx=5",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "unknownType", Idx: 5},
				Role:    "unknownType", // Preserved literally
				ShapeID: 23,
			},
			expected: "unknownType:5", // Non-unique (empty context)
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := NewMockLayoutContext()
			result := GenerateKey(tt.resolved, ctx)
			if result != tt.expected {
				t.Errorf("GenerateKey() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestGenerateKeyPriority4_ShapeIDFallback(t *testing.T) {
	// Priority 4: No metadata → shape:{shapeId}
	tests := []struct {
		name     string
		resolved model.ResolvedPlaceholder
		expected string
	}{
		{
			name: "no type, no index, shapeId=4",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "", Idx: -1},
				Role:    "",
				ShapeID: 4,
			},
			expected: "shape:4",
		},
		{
			name: "no type, no index, shapeId=123",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "", Idx: -1},
				Role:    "",
				ShapeID: 123,
			},
			expected: "shape:123",
		},
		{
			name: "no type, no index, shapeId=0",
			resolved: model.ResolvedPlaceholder{
				Raw:     model.RawPlaceholder{Type: "", Idx: -1},
				Role:    "",
				ShapeID: 0,
			},
			expected: "shape:0",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ctx := NewMockLayoutContext()
			result := GenerateKey(tt.resolved, ctx)
			if result != tt.expected {
				t.Errorf("GenerateKey() = %q, want %q", result, tt.expected)
			}
		})
	}
}

func TestGenerateKeyRealWorldScenarios(t *testing.T) {
	// Title layout: [title, subtitle, body]
	t.Run("title layout", func(t *testing.T) {
		placeholders := []model.ResolvedPlaceholder{
			{
				Raw:     model.RawPlaceholder{Type: "title", Idx: -1},
				Role:    "title",
				ShapeID: 1,
			},
			{
				Raw:     model.RawPlaceholder{Type: "subTitle", Idx: -1},
				Role:    "subtitle",
				ShapeID: 2,
			},
			{
				Raw:     model.RawPlaceholder{Type: "body", Idx: 0},
				Role:    "body",
				ShapeID: 3,
			},
		}
		ctx := BuildSimpleLayoutContext(placeholders)

		tests := []struct {
			ph       model.ResolvedPlaceholder
			expected string
		}{
			{placeholders[0], "title"},
			{placeholders[1], "subtitle"},
			{placeholders[2], "body"}, // body is unique because idx=0 is the only body
		}

		for i, tt := range tests {
			result := GenerateKey(tt.ph, ctx)
			if result != tt.expected {
				t.Errorf("placeholder[%d]: GenerateKey() = %q, want %q", i, result, tt.expected)
			}
		}
	})

	// Content layout with multiple bodies: [title, body:0, body:1, body:2]
	t.Run("content layout with multiple bodies", func(t *testing.T) {
		placeholders := []model.ResolvedPlaceholder{
			{
				Raw:     model.RawPlaceholder{Type: "title", Idx: -1},
				Role:    "title",
				ShapeID: 10,
			},
			{
				Raw:     model.RawPlaceholder{Type: "body", Idx: 0},
				Role:    "body",
				ShapeID: 11,
			},
			{
				Raw:     model.RawPlaceholder{Type: "body", Idx: 1},
				Role:    "body",
				ShapeID: 12,
			},
			{
				Raw:     model.RawPlaceholder{Type: "body", Idx: 2},
				Role:    "body",
				ShapeID: 13,
			},
		}
		ctx := BuildSimpleLayoutContext(placeholders)

		tests := []struct {
			ph       model.ResolvedPlaceholder
			expected string
		}{
			{placeholders[0], "title"},
			{placeholders[1], "body:0"},
			{placeholders[2], "body:1"},
			{placeholders[3], "body:2"},
		}

		for i, tt := range tests {
			result := GenerateKey(tt.ph, ctx)
			if result != tt.expected {
				t.Errorf("placeholder[%d]: GenerateKey() = %q, want %q", i, result, tt.expected)
			}
		}
	})
}

func TestSimpleLayoutContext(t *testing.T) {
	t.Run("single role is unique", func(t *testing.T) {
		ctx := NewSimpleLayoutContext(map[string]int{
			"title": 1,
		})
		if !ctx.IsRoleUniqueInLayout("title") {
			t.Error("expected title to be unique")
		}
	})

	t.Run("multiple roles not unique", func(t *testing.T) {
		ctx := NewSimpleLayoutContext(map[string]int{
			"body": 3,
		})
		if ctx.IsRoleUniqueInLayout("body") {
			t.Error("expected body (count=3) to not be unique")
		}
	})

	t.Run("missing role not unique", func(t *testing.T) {
		ctx := NewSimpleLayoutContext(map[string]int{})
		if ctx.IsRoleUniqueInLayout("title") {
			t.Error("expected missing role to not be unique")
		}
	})
}

func TestBuildSimpleLayoutContext(t *testing.T) {
	placeholders := []model.ResolvedPlaceholder{
		{Role: "title", ShapeID: 1},
		{Role: "body", ShapeID: 2},
		{Role: "body", ShapeID: 3},
		{Role: "body", ShapeID: 4},
	}

	ctx := BuildSimpleLayoutContext(placeholders)

	tests := []struct {
		role     string
		expected bool
	}{
		{"title", true},   // count=1
		{"body", false},   // count=3
		{"footer", false}, // not in placeholders
	}

	for _, tt := range tests {
		result := ctx.IsRoleUniqueInLayout(tt.role)
		if result != tt.expected {
			t.Errorf("IsRoleUniqueInLayout(%q) = %v, want %v", tt.role, result, tt.expected)
		}
	}
}
