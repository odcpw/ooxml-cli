package normalize

import (
	"testing"
)

func TestCanonicalRole(t *testing.T) {
	tests := []struct {
		name     string
		phType   string
		expected string
	}{
		// Mapped types
		{
			name:     "title type",
			phType:   "title",
			expected: "title",
		},
		{
			name:     "ctrTitle type (center title)",
			phType:   "ctrTitle",
			expected: "title",
		},
		{
			name:     "subTitle type",
			phType:   "subTitle",
			expected: "subtitle",
		},
		{
			name:     "body type",
			phType:   "body",
			expected: "body",
		},
		{
			name:     "pic type",
			phType:   "pic",
			expected: "pic",
		},
		{
			name:     "tbl type (table)",
			phType:   "tbl",
			expected: "table",
		},
		{
			name:     "chart type",
			phType:   "chart",
			expected: "chart",
		},
		{
			name:     "obj type (object)",
			phType:   "obj",
			expected: "object",
		},
		{
			name:     "dt type (date)",
			phType:   "dt",
			expected: "date",
		},
		{
			name:     "ftr type (footer)",
			phType:   "ftr",
			expected: "footer",
		},
		{
			name:     "sldNum type (slide number)",
			phType:   "sldNum",
			expected: "slideNumber",
		},
		// Unknown types (preserved literally)
		{
			name:     "unknown type (preserved)",
			phType:   "customType",
			expected: "customType",
		},
		{
			name:     "empty string",
			phType:   "",
			expected: "",
		},
		{
			name:     "unknown vendor type",
			phType:   "vendorSpecific",
			expected: "vendorSpecific",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := CanonicalRole(tt.phType)
			if result != tt.expected {
				t.Errorf("CanonicalRole(%q) = %q, want %q", tt.phType, result, tt.expected)
			}
		})
	}
}

func TestCanonicalRoleStability(t *testing.T) {
	// Test that calling CanonicalRole multiple times returns the same result
	phType := "body"
	first := CanonicalRole(phType)
	second := CanonicalRole(phType)

	if first != second {
		t.Errorf("CanonicalRole(%q) not stable: got %q then %q", phType, first, second)
	}
}

func TestCanonicalRoleCaseSensitive(t *testing.T) {
	// Verify that role mapping is case-sensitive
	tests := []struct {
		phType   string
		expected string
	}{
		{"Title", "Title"}, // Capital T → not matched, preserved
		{"TITLE", "TITLE"}, // All caps → not matched, preserved
		{"title", "title"}, // lowercase → matched
		{"Body", "Body"},   // Capital B → not matched, preserved
		{"body", "body"},   // lowercase → matched
	}

	for _, tt := range tests {
		result := CanonicalRole(tt.phType)
		if result != tt.expected {
			t.Errorf("CanonicalRole(%q) = %q, want %q", tt.phType, result, tt.expected)
		}
	}
}
