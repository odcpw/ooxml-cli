package translate

import (
	"crypto/sha256"
	"fmt"
	"strings"
)

// IDGenerationRules documents the stable ID generation algorithm.
// IDs must be deterministic (same input → same ID) and collision-free.
// This is a permanent contract — IDs must never change once exported.
//
// Format: <slide-id>_<shape-key>_p<para-idx>_r<run-idx>
//
// Components:
//   - slide-id: Zero-based slide index with "slide:" prefix
//   - shape-key: Placeholder key (e.g., "title", "body:0") or "shape:N" fallback
//   - para-idx: Zero-based paragraph index with "p" prefix
//   - run-idx: Zero-based run index within paragraph with "r" prefix
//
// Examples:
//   - "slide:0_title_p0_r0" — first run of first paragraph of title on slide 0
//   - "slide:1_body:0_p1_r2" — third run of second paragraph of first body on slide 1
//   - "slide:2_shape:5_p0_r0" — untypified shape with ID 5 on slide 2
//
// Stability Contract:
//   - A text entry at the same location must always generate the same ID
//   - IDs are immutable once published in a manifest
//   - If the algorithm changes, it requires a version bump in ManifestVersion
type IDGenerationRules struct {
	// This struct exists for documentation purposes only.
	// All functionality is in GenerateEntryID functions.
}

// GenerateEntryID creates a stable, deterministic ID for a translation entry.
//
// Parameters:
//   - slideID: Zero-based slide index
//   - placeholderKey: Placeholder key (e.g., "title", "body:0") or shape fallback
//   - paragraphIndex: Zero-based paragraph index within the shape
//   - runIndex: Zero-based run index within the paragraph
//
// Returns: ID string in format "slide:0_title_p0_r0"
func GenerateEntryID(slideID int, placeholderKey string, paragraphIndex, runIndex int) string {
	return fmt.Sprintf("slide:%d_%s_p%d_r%d", slideID, placeholderKey, paragraphIndex, runIndex)
}

// GenerateContextHash creates a SHA256 hash of surrounding text context
// for freshness validation. This allows detecting if source text has changed.
//
// The context is computed from neighboring paragraphs and runs to provide
// a stable fingerprint of the text's environment.
//
// Parameters:
//   - precedingText: Text from the previous entry (or empty if none)
//   - currentText: The current entry's source text
//   - followingText: Text from the next entry (or empty if none)
//
// Returns: Hex-encoded SHA256 hash
func GenerateContextHash(precedingText, currentText, followingText string) string {
	combined := strings.Join([]string{precedingText, currentText, followingText}, "\n")
	hash := sha256.Sum256([]byte(combined))
	return fmt.Sprintf("%x", hash)
}

// ValidateID checks if an entry ID matches the expected format.
// This is useful for detecting corrupted manifests or manual edits.
//
// Valid format: slide:\d+_[a-zA-Z0-9:]+_p\d+_r\d+
func ValidateID(id string) bool {
	parts := strings.Split(id, "_")
	if len(parts) != 4 {
		return false
	}

	// Check slide: prefix
	if !strings.HasPrefix(parts[0], "slide:") {
		return false
	}
	slidePart := strings.TrimPrefix(parts[0], "slide:")
	if slidePart == "" {
		return false
	}
	// slidePart should be numeric, but we do minimal validation
	for _, c := range slidePart {
		if c < '0' || c > '9' {
			return false
		}
	}

	// Check shape key (should contain alphanumerics and colons)
	shapeKey := parts[1]
	if shapeKey == "" {
		return false
	}

	// Check paragraph: p\d+
	if !strings.HasPrefix(parts[2], "p") {
		return false
	}
	paraPart := strings.TrimPrefix(parts[2], "p")
	if paraPart == "" {
		return false
	}
	for _, c := range paraPart {
		if c < '0' || c > '9' {
			return false
		}
	}

	// Check run: r\d+
	if !strings.HasPrefix(parts[3], "r") {
		return false
	}
	runPart := strings.TrimPrefix(parts[3], "r")
	if runPart == "" {
		return false
	}
	for _, c := range runPart {
		if c < '0' || c > '9' {
			return false
		}
	}

	return true
}

// ParseID extracts components from an entry ID.
// Returns (slideID, shapeKey, paragraphIndex, runIndex, error)
//
// Example: "slide:0_title_p1_r2" → (0, "title", 1, 2, nil)
func ParseID(id string) (slideID int, shapeKey string, paragraphIndex, runIndex int, err error) {
	if !ValidateID(id) {
		return 0, "", 0, 0, fmt.Errorf("invalid ID format: %s", id)
	}

	parts := strings.Split(id, "_")
	slidePart := strings.TrimPrefix(parts[0], "slide:")
	shapeKey = parts[1]
	paraPart := strings.TrimPrefix(parts[2], "p")
	runPart := strings.TrimPrefix(parts[3], "r")

	_, _ = fmt.Sscanf(slidePart, "%d", &slideID)
	_, _ = fmt.Sscanf(paraPart, "%d", &paragraphIndex)
	_, _ = fmt.Sscanf(runPart, "%d", &runIndex)

	return slideID, shapeKey, paragraphIndex, runIndex, nil
}
