package translate

import (
	"testing"
)

// TestGenerateEntryID verifies the deterministic ID generation
func TestGenerateEntryID(t *testing.T) {
	tests := []struct {
		name           string
		slideID        int
		placeholderKey string
		paragraphIndex int
		runIndex       int
		expectedID     string
	}{
		{
			name:           "title on slide 0",
			slideID:        0,
			placeholderKey: "title",
			paragraphIndex: 0,
			runIndex:       0,
			expectedID:     "slide:0_title_p0_r0",
		},
		{
			name:           "body:0 on slide 1",
			slideID:        1,
			placeholderKey: "body:0",
			paragraphIndex: 1,
			runIndex:       2,
			expectedID:     "slide:1_body:0_p1_r2",
		},
		{
			name:           "subtitle on slide 0",
			slideID:        0,
			placeholderKey: "subtitle",
			paragraphIndex: 0,
			runIndex:       0,
			expectedID:     "slide:0_subtitle_p0_r0",
		},
		{
			name:           "shape fallback on slide 2",
			slideID:        2,
			placeholderKey: "shape:5",
			paragraphIndex: 0,
			runIndex:       0,
			expectedID:     "slide:2_shape:5_p0_r0",
		},
		{
			name:           "multiple bodies",
			slideID:        3,
			placeholderKey: "body:5",
			paragraphIndex: 10,
			runIndex:       15,
			expectedID:     "slide:3_body:5_p10_r15",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			id := GenerateEntryID(tt.slideID, tt.placeholderKey, tt.paragraphIndex, tt.runIndex)
			if id != tt.expectedID {
				t.Errorf("expected %s, got %s", tt.expectedID, id)
			}
		})
	}
}

// TestGenerateEntryIDDeterminism verifies that the same input always produces the same ID
func TestGenerateEntryIDDeterminism(t *testing.T) {
	slideID := 5
	placeholderKey := "body:2"
	paragraphIndex := 3
	runIndex := 4

	id1 := GenerateEntryID(slideID, placeholderKey, paragraphIndex, runIndex)
	id2 := GenerateEntryID(slideID, placeholderKey, paragraphIndex, runIndex)
	id3 := GenerateEntryID(slideID, placeholderKey, paragraphIndex, runIndex)

	if id1 != id2 || id2 != id3 {
		t.Errorf("IDs are not deterministic: %s, %s, %s", id1, id2, id3)
	}
}

// TestGenerateContextHash verifies context hash generation
func TestGenerateContextHash(t *testing.T) {
	hash1 := GenerateContextHash("preceding", "current", "following")
	hash2 := GenerateContextHash("preceding", "current", "following")
	hash3 := GenerateContextHash("different", "current", "following")

	// Same input should produce same hash
	if hash1 != hash2 {
		t.Errorf("same input produced different hashes: %s != %s", hash1, hash2)
	}

	// Different input should produce different hash
	if hash1 == hash3 {
		t.Errorf("different input produced same hash: %s", hash1)
	}

	// Hash should be valid hex
	if len(hash1) != 64 { // SHA256 hex is 64 chars
		t.Errorf("expected 64 char hex hash, got %d chars: %s", len(hash1), hash1)
	}

	// Verify all chars are hex
	for _, c := range hash1 {
		if !((c >= '0' && c <= '9') || (c >= 'a' && c <= 'f')) {
			t.Errorf("invalid hex character: %c", c)
		}
	}
}

// TestGenerateContextHashEmpty verifies context hash with empty strings
func TestGenerateContextHashEmpty(t *testing.T) {
	hash := GenerateContextHash("", "", "")
	if len(hash) != 64 {
		t.Errorf("expected 64 char hash, got %d", len(hash))
	}
}

// TestValidateID verifies ID format validation
func TestValidateID(t *testing.T) {
	tests := []struct {
		name    string
		id      string
		isValid bool
	}{
		{
			name:    "valid title ID",
			id:      "slide:0_title_p0_r0",
			isValid: true,
		},
		{
			name:    "valid body with index",
			id:      "slide:1_body:0_p2_r3",
			isValid: true,
		},
		{
			name:    "valid shape fallback",
			id:      "slide:2_shape:5_p0_r0",
			isValid: true,
		},
		{
			name:    "valid large indices",
			id:      "slide:10_body:15_p100_r200",
			isValid: true,
		},
		{
			name:    "missing slide prefix",
			id:      "0_title_p0_r0",
			isValid: false,
		},
		{
			name:    "invalid slide prefix",
			id:      "slides:0_title_p0_r0",
			isValid: false,
		},
		{
			name:    "missing shape key",
			id:      "slide:0__p0_r0",
			isValid: false,
		},
		{
			name:    "missing paragraph prefix",
			id:      "slide:0_title_0_r0",
			isValid: false,
		},
		{
			name:    "missing run prefix",
			id:      "slide:0_title_p0_0",
			isValid: false,
		},
		{
			name:    "too many parts",
			id:      "slide:0_title_p0_r0_extra",
			isValid: false,
		},
		{
			name:    "too few parts",
			id:      "slide:0_title_p0",
			isValid: false,
		},
		{
			name:    "empty ID",
			id:      "",
			isValid: false,
		},
		{
			name:    "non-numeric slide",
			id:      "slide:a_title_p0_r0",
			isValid: false,
		},
		{
			name:    "non-numeric paragraph",
			id:      "slide:0_title_px_r0",
			isValid: false,
		},
		{
			name:    "non-numeric run",
			id:      "slide:0_title_p0_ry",
			isValid: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			valid := ValidateID(tt.id)
			if valid != tt.isValid {
				t.Errorf("expected valid=%v, got %v for ID: %s", tt.isValid, valid, tt.id)
			}
		})
	}
}

// TestParseID verifies ID parsing
func TestParseID(t *testing.T) {
	tests := []struct {
		name             string
		id               string
		expectedSlideID  int
		expectedShapeKey string
		expectedParaIdx  int
		expectedRunIdx   int
		shouldError      bool
	}{
		{
			name:             "simple title ID",
			id:               "slide:0_title_p0_r0",
			expectedSlideID:  0,
			expectedShapeKey: "title",
			expectedParaIdx:  0,
			expectedRunIdx:   0,
			shouldError:      false,
		},
		{
			name:             "body with indices",
			id:               "slide:1_body:0_p2_r3",
			expectedSlideID:  1,
			expectedShapeKey: "body:0",
			expectedParaIdx:  2,
			expectedRunIdx:   3,
			shouldError:      false,
		},
		{
			name:             "shape fallback",
			id:               "slide:5_shape:123_p10_r20",
			expectedSlideID:  5,
			expectedShapeKey: "shape:123",
			expectedParaIdx:  10,
			expectedRunIdx:   20,
			shouldError:      false,
		},
		{
			name:        "invalid ID format",
			id:          "invalid",
			shouldError: true,
		},
		{
			name:        "empty ID",
			id:          "",
			shouldError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			slideID, shapeKey, paraIdx, runIdx, err := ParseID(tt.id)

			if tt.shouldError {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
				if slideID != tt.expectedSlideID {
					t.Errorf("expected slideID %d, got %d", tt.expectedSlideID, slideID)
				}
				if shapeKey != tt.expectedShapeKey {
					t.Errorf("expected shapeKey %s, got %s", tt.expectedShapeKey, shapeKey)
				}
				if paraIdx != tt.expectedParaIdx {
					t.Errorf("expected paraIdx %d, got %d", tt.expectedParaIdx, paraIdx)
				}
				if runIdx != tt.expectedRunIdx {
					t.Errorf("expected runIdx %d, got %d", tt.expectedRunIdx, runIdx)
				}
			}
		})
	}
}

// TestParseIDRoundtrip verifies that generating an ID and parsing it gives back the original components
func TestParseIDRoundtrip(t *testing.T) {
	tests := []struct {
		slideID        int
		placeholderKey string
		paragraphIndex int
		runIndex       int
	}{
		{0, "title", 0, 0},
		{1, "body:0", 2, 3},
		{5, "shape:123", 10, 20},
		{100, "body:99", 0, 0},
	}

	for _, tt := range tests {
		t.Run("roundtrip", func(t *testing.T) {
			// Generate ID
			id := GenerateEntryID(tt.slideID, tt.placeholderKey, tt.paragraphIndex, tt.runIndex)

			// Parse it back
			slideID, shapeKey, paraIdx, runIdx, err := ParseID(id)
			if err != nil {
				t.Fatalf("failed to parse ID: %v", err)
			}

			// Verify all components match
			if slideID != tt.slideID {
				t.Errorf("slideID mismatch: expected %d, got %d", tt.slideID, slideID)
			}
			if shapeKey != tt.placeholderKey {
				t.Errorf("shapeKey mismatch: expected %s, got %s", tt.placeholderKey, shapeKey)
			}
			if paraIdx != tt.paragraphIndex {
				t.Errorf("paraIdx mismatch: expected %d, got %d", tt.paragraphIndex, paraIdx)
			}
			if runIdx != tt.runIndex {
				t.Errorf("runIdx mismatch: expected %d, got %d", tt.runIndex, runIdx)
			}
		})
	}
}

// TestIDCollisionResistance verifies that different entries produce different IDs
func TestIDCollisionResistance(t *testing.T) {
	ids := make(map[string]bool)

	// Generate many IDs with different parameters
	testCases := []struct {
		slideID        int
		placeholderKey string
		paragraphIndex int
		runIndex       int
	}{
		{0, "title", 0, 0},
		{0, "title", 0, 1},
		{0, "title", 1, 0},
		{0, "body:0", 0, 0},
		{1, "title", 0, 0},
		{0, "body:1", 0, 0},
		{0, "shape:5", 0, 0},
	}

	for _, tc := range testCases {
		id := GenerateEntryID(tc.slideID, tc.placeholderKey, tc.paragraphIndex, tc.runIndex)
		if ids[id] {
			t.Errorf("collision detected for ID: %s", id)
		}
		ids[id] = true
	}
}
