package normalize

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// LayoutContext provides context-dependent information about a layout,
// used during key generation to determine role uniqueness.
// Defined in this package; must be implemented by consumers.
type LayoutContext interface {
	// IsRoleUniqueInLayout returns true if the role appears exactly once
	// in this layout.
	IsRoleUniqueInLayout(role string) bool
}

// GenerateKey generates a stable, semantic key for a placeholder following
// the four-priority algorithm defined in docs/placeholder-key-rules.md:
//
// Priority 1: Unique canonical role → {role}
// Priority 2: Non-unique role with index → {role}:{idx}
// Priority 3: No type, has index → ph:{idx}
// Priority 4: No metadata → shape:{shapeId}
func GenerateKey(resolved model.ResolvedPlaceholder, layoutCtx LayoutContext) string {
	// Priority 1: Unique canonical role
	if resolved.Role != "" && layoutCtx.IsRoleUniqueInLayout(resolved.Role) {
		return resolved.Role
	}

	// Priority 2: Non-unique role with index
	if resolved.Role != "" && resolved.Raw.Idx >= 0 {
		return fmt.Sprintf("%s:%d", resolved.Role, resolved.Raw.Idx)
	}

	// Priority 3: No type, but has index
	if resolved.Raw.Idx >= 0 {
		return fmt.Sprintf("ph:%d", resolved.Raw.Idx)
	}

	// Priority 4: Shape ID fallback
	return fmt.Sprintf("shape:%d", resolved.ShapeID)
}

// SimpleLayoutContext is a minimal implementation of LayoutContext
// for testing and simple use cases. It takes a set of roles and their counts.
type SimpleLayoutContext struct {
	roleCounts map[string]int
}

// NewSimpleLayoutContext creates a new SimpleLayoutContext with the given role counts.
func NewSimpleLayoutContext(roleCounts map[string]int) *SimpleLayoutContext {
	return &SimpleLayoutContext{
		roleCounts: roleCounts,
	}
}

// IsRoleUniqueInLayout returns true if the role appears exactly once.
func (c *SimpleLayoutContext) IsRoleUniqueInLayout(role string) bool {
	count, exists := c.roleCounts[role]
	return exists && count == 1
}

// BuildSimpleLayoutContext is a helper that counts roles in a list
// of resolved placeholders to build a SimpleLayoutContext.
func BuildSimpleLayoutContext(placeholders []model.ResolvedPlaceholder) *SimpleLayoutContext {
	roleCounts := make(map[string]int)
	for _, ph := range placeholders {
		if ph.Role != "" {
			roleCounts[ph.Role]++
		}
	}
	return NewSimpleLayoutContext(roleCounts)
}
