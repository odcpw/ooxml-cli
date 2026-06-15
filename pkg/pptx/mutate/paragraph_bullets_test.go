package mutate

import (
	"testing"

	"github.com/beevik/etree"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// Helper to create a basic paragraph element for testing
func createTestParagraph() *etree.Element {
	// Create a basic p element with minimal structure
	p := etree.NewElement("a:p")
	p.Space = "a" // Set namespace prefix
	// Add namespace declaration so xmlx can find children
	p.CreateAttr("xmlns:a", ns.NsA)

	// Create and add a text run
	r := etree.NewElement("a:r")
	r.Space = "a"
	t := etree.NewElement("a:t")
	t.Space = "a"
	t.SetText("Test paragraph")
	r.AddChild(t)
	p.AddChild(r)

	return p
}

// ============================================================================
// Paragraph Level Tests
// ============================================================================

func TestSetParagraphLevel(t *testing.T) {
	tests := []struct {
		name      string
		level     int32
		shouldErr bool
	}{
		{"Level 0", 0, false},
		{"Level 1", 1, false},
		{"Level 5", 5, false},
		{"Level 8", 8, false},
		{"Level -1 (invalid)", -1, true},
		{"Level 9 (invalid)", 9, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetParagraphLevel(p, tt.level)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetParagraphLevel(%d): got error %v, want error %v", tt.level, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the level was set
				level, getErr := GetParagraphLevel(p)
				if getErr != nil {
					t.Errorf("GetParagraphLevel: got error %v", getErr)
				}
				if level == nil || *level != tt.level {
					t.Errorf("GetParagraphLevel: got %v, want %v", level, tt.level)
				}
			}
		})
	}
}

// ============================================================================
// Paragraph Alignment Tests
// ============================================================================

func TestSetParagraphAlignment(t *testing.T) {
	tests := []struct {
		name      string
		alignment string
		shouldErr bool
	}{
		{"Left", "l", false},
		{"Center", "ctr", false},
		{"Right", "r", false},
		{"Justified", "just", false},
		{"Distributed", "dist", false},
		{"Invalid", "invalid", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetParagraphAlignment(p, tt.alignment)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetParagraphAlignment(%s): got error %v, want error %v", tt.alignment, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the alignment was set
				alignment, getErr := GetParagraphAlignment(p)
				if getErr != nil {
					t.Errorf("GetParagraphAlignment: got error %v", getErr)
				}
				if alignment != tt.alignment {
					t.Errorf("GetParagraphAlignment: got %s, want %s", alignment, tt.alignment)
				}
			}
		})
	}
}

// ============================================================================
// Paragraph Spacing Tests
// ============================================================================

func TestSetParagraphSpacing(t *testing.T) {
	tests := []struct {
		name        string
		spaceBefore *int64
		spaceAfter  *int64
	}{
		{"Both values", int64Ptr(1000), int64Ptr(2000)},
		{"Only before", int64Ptr(1500), nil},
		{"Only after", nil, int64Ptr(2500)},
		{"Both nil", nil, nil},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetParagraphSpacing(p, tt.spaceBefore, tt.spaceAfter)

			if err != nil {
				t.Errorf("SetParagraphSpacing: got error %v", err)
			}

			// Verify spacing was set
			before, after, getErr := GetParagraphSpacing(p)
			if getErr != nil && (tt.spaceBefore != nil || tt.spaceAfter != nil) {
				t.Errorf("GetParagraphSpacing: got error %v", getErr)
			}

			if tt.spaceBefore != nil && (before == nil || *before != *tt.spaceBefore) {
				t.Errorf("spaceBefore: got %v, want %v", before, tt.spaceBefore)
			}

			if tt.spaceAfter != nil && (after == nil || *after != *tt.spaceAfter) {
				t.Errorf("spaceAfter: got %v, want %v", after, tt.spaceAfter)
			}
		})
	}
}

// ============================================================================
// Paragraph Line Spacing Tests
// ============================================================================

func TestSetParagraphLineSpacing(t *testing.T) {
	p := createTestParagraph()
	lineSpacing := int64(2400) // 24pt

	err := SetParagraphLineSpacing(p, lineSpacing)
	if err != nil {
		t.Errorf("SetParagraphLineSpacing: got error %v", err)
	}

	retrieved, getErr := GetParagraphLineSpacing(p)
	if getErr != nil {
		t.Errorf("GetParagraphLineSpacing: got error %v", getErr)
	}

	if retrieved == nil || *retrieved != lineSpacing {
		t.Errorf("GetParagraphLineSpacing: got %v, want %v", retrieved, lineSpacing)
	}
}

// ============================================================================
// Bullet Mode Tests
// ============================================================================

func TestSetBulletMode(t *testing.T) {
	tests := []struct {
		name      string
		mode      string
		shouldErr bool
	}{
		{"No bullets", "buNone", false},
		{"Character bullets", "buChar", false},
		{"Auto-numbered", "buAutoNum", false},
		{"Invalid", "invalid", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetBulletMode(p, tt.mode)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetBulletMode(%s): got error %v, want error %v", tt.mode, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the mode was set
				mode, getErr := GetBulletMode(p)
				if getErr != nil {
					t.Errorf("GetBulletMode: got error %v", getErr)
				}
				if mode != tt.mode {
					t.Errorf("GetBulletMode: got %s, want %s", mode, tt.mode)
				}
			}
		})
	}
}

// ============================================================================
// Bullet Character Tests
// ============================================================================

func TestSetBulletCharacter(t *testing.T) {
	tests := []struct {
		name      string
		character string
		shouldErr bool
	}{
		{"Bullet point", "•", false},
		{"Dash", "-", false},
		{"Asterisk", "*", false},
		{"Empty (invalid)", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			// First set bullet mode to buChar
			_ = SetBulletMode(p, "buChar")

			err := SetBulletCharacter(p, tt.character)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetBulletCharacter(%s): got error %v, want error %v", tt.character, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the character was set
				char, getErr := GetBulletCharacter(p)
				if getErr != nil {
					t.Errorf("GetBulletCharacter: got error %v", getErr)
				}
				if char == nil || *char != tt.character {
					t.Errorf("GetBulletCharacter: got %v, want %s", char, tt.character)
				}
			}
		})
	}
}

// ============================================================================
// Auto-Numbering Scheme Tests
// ============================================================================

func TestSetAutoNumberingScheme(t *testing.T) {
	tests := []struct {
		name      string
		scheme    string
		shouldErr bool
	}{
		{"Standard", "stdAutoNum", false},
		{"Roman lowercase", "romanLcParenR", false},
		{"Custom", "arabicPeriod", false},
		{"Empty (invalid)", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			// First set bullet mode to buAutoNum
			_ = SetBulletMode(p, "buAutoNum")

			err := SetAutoNumberingScheme(p, tt.scheme)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetAutoNumberingScheme(%s): got error %v, want error %v", tt.scheme, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the scheme was set
				scheme, getErr := GetAutoNumberingScheme(p)
				if getErr != nil {
					t.Errorf("GetAutoNumberingScheme: got error %v", getErr)
				}
				if scheme == nil || *scheme != tt.scheme {
					t.Errorf("GetAutoNumberingScheme: got %v, want %s", scheme, tt.scheme)
				}
			}
		})
	}
}

// ============================================================================
// Bullet Font Size Tests
// ============================================================================

func TestSetBulletFontSize(t *testing.T) {
	tests := []struct {
		name      string
		size      int32
		shouldErr bool
	}{
		{"12pt (1200)", 1200, false},
		{"24pt (2400)", 2400, false},
		{"0pt", 0, false},
		{"Negative (invalid)", -100, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetBulletFontSize(p, tt.size)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetBulletFontSize(%d): got error %v, want error %v", tt.size, err != nil, tt.shouldErr)
			}

			if err == nil && tt.size > 0 {
				// Verify the size was set
				size, getErr := GetBulletFontSize(p)
				if getErr != nil {
					t.Errorf("GetBulletFontSize: got error %v", getErr)
				}
				if size == nil || *size != tt.size {
					t.Errorf("GetBulletFontSize: got %v, want %d", size, tt.size)
				}
			}
		})
	}
}

// ============================================================================
// Bullet Font Family Tests
// ============================================================================

func TestSetBulletFontFamily(t *testing.T) {
	tests := []struct {
		name      string
		family    string
		shouldErr bool
	}{
		{"Arial", "Arial", false},
		{"Wingdings", "Wingdings", false},
		{"Symbol", "Symbol", false},
		{"Empty (invalid)", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetBulletFontFamily(p, tt.family)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetBulletFontFamily(%s): got error %v, want error %v", tt.family, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the family was set
				family, getErr := GetBulletFontFamily(p)
				if getErr != nil {
					t.Errorf("GetBulletFontFamily: got error %v", getErr)
				}
				if family == nil || *family != tt.family {
					t.Errorf("GetBulletFontFamily: got %v, want %s", family, tt.family)
				}
			}
		})
	}
}

// ============================================================================
// Bullet Color Tests
// ============================================================================

func TestSetBulletColor(t *testing.T) {
	tests := []struct {
		name      string
		color     string
		shouldErr bool
	}{
		{"Red", "FF0000", false},
		{"Green", "00FF00", false},
		{"Blue", "0000FF", false},
		{"Empty (invalid)", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			p := createTestParagraph()
			err := SetBulletColor(p, tt.color)

			if (err != nil) != tt.shouldErr {
				t.Errorf("SetBulletColor(%s): got error %v, want error %v", tt.color, err != nil, tt.shouldErr)
			}

			if err == nil {
				// Verify the color was set
				color, getErr := GetBulletColor(p)
				if getErr != nil {
					t.Errorf("GetBulletColor: got error %v", getErr)
				}
				if color == nil || *color != tt.color {
					t.Errorf("GetBulletColor: got %v, want %s", color, tt.color)
				}
			}
		})
	}
}

// ============================================================================
// Integration Tests
// ============================================================================

func TestApplyParagraphOptions(t *testing.T) {
	p := createTestParagraph()

	level := int32(1)
	alignment := "ctr"
	spaceBefore := int64(1000)
	spaceAfter := int64(2000)

	opts := &ParagraphMutationOptions{
		Level:       &level,
		Alignment:   &alignment,
		SpaceBefore: &spaceBefore,
		SpaceAfter:  &spaceAfter,
	}

	err := ApplyParagraphOptions(p, opts)
	if err != nil {
		t.Errorf("ApplyParagraphOptions: got error %v", err)
	}

	// Verify all options were applied
	retrievedLevel, err := GetParagraphLevel(p)
	if err != nil || retrievedLevel == nil || *retrievedLevel != level {
		t.Errorf("Level verification failed: got %v, want %d", retrievedLevel, level)
	}

	retrievedAlignment, err := GetParagraphAlignment(p)
	if err != nil || retrievedAlignment != alignment {
		t.Errorf("Alignment verification failed: got %s, want %s", retrievedAlignment, alignment)
	}
}

func TestApplyBulletOptions(t *testing.T) {
	p := createTestParagraph()

	mode := "buChar"
	character := "•"
	fontFamily := "Wingdings"
	fontSize := int32(2000)
	color := "FF0000"

	opts := &BulletMutationOptions{
		Mode:       mode,
		Character:  &character,
		FontFamily: &fontFamily,
		FontSize:   &fontSize,
		Color:      &color,
	}

	err := ApplyBulletOptions(p, opts)
	if err != nil {
		t.Errorf("ApplyBulletOptions: got error %v", err)
	}

	// Verify all options were applied
	retrievedMode, err := GetBulletMode(p)
	if err != nil || retrievedMode != mode {
		t.Errorf("Mode verification failed: got %s, want %s", retrievedMode, mode)
	}

	retrievedCharacter, err := GetBulletCharacter(p)
	if err != nil || retrievedCharacter == nil || *retrievedCharacter != character {
		t.Errorf("Character verification failed: got %v, want %s", retrievedCharacter, character)
	}

	retrievedFamily, err := GetBulletFontFamily(p)
	if err != nil || retrievedFamily == nil || *retrievedFamily != fontFamily {
		t.Errorf("Font family verification failed: got %v, want %s", retrievedFamily, fontFamily)
	}

	retrievedSize, err := GetBulletFontSize(p)
	if err != nil || retrievedSize == nil || *retrievedSize != fontSize {
		t.Errorf("Font size verification failed: got %v, want %d", retrievedSize, fontSize)
	}

	retrievedColor, err := GetBulletColor(p)
	if err != nil || retrievedColor == nil || *retrievedColor != color {
		t.Errorf("Color verification failed: got %v, want %s", retrievedColor, color)
	}
}

// ============================================================================
// Helper Functions
// ============================================================================

func int64Ptr(v int64) *int64 {
	return &v
}

// TestBulletModeSwitching verifies that changing bullet modes removes the old mode
func TestBulletModeSwitching(t *testing.T) {
	p := createTestParagraph()

	// Set to buChar first
	_ = SetBulletMode(p, "buChar")
	_ = SetBulletCharacter(p, "•")

	mode, _ := GetBulletMode(p)
	if mode != "buChar" {
		t.Errorf("Initial mode: got %s, want buChar", mode)
	}

	// Switch to buNone
	_ = SetBulletMode(p, "buNone")
	mode, _ = GetBulletMode(p)
	if mode != "buNone" {
		t.Errorf("Switched mode: got %s, want buNone", mode)
	}

	// Try to get character (should fail)
	_, err := GetBulletCharacter(p)
	if err == nil {
		t.Errorf("GetBulletCharacter after mode switch: expected error, got nil")
	}
}

// TestMultipleParagraphsInTxBody verifies operations on paragraph elements within a text body
func TestMultipleParagraphsInTxBody(t *testing.T) {
	ns := "http://schemas.openxmlformats.org/drawingml/2006/main"

	// Create a text body with multiple paragraphs
	txBody := etree.NewElement("p:txBody")
	txBody.Space = "p"
	txBody.CreateAttr("xmlns:a", ns) // Add namespace declaration

	p1 := etree.NewElement("a:p")
	p1.Space = "a"
	r1 := etree.NewElement("a:r")
	r1.Space = "a"
	t1 := etree.NewElement("a:t")
	t1.Space = "a"
	t1.SetText("Paragraph 1")
	r1.AddChild(t1)
	p1.AddChild(r1)
	txBody.AddChild(p1)

	p2 := etree.NewElement("a:p")
	p2.Space = "a"
	r2 := etree.NewElement("a:r")
	r2.Space = "a"
	t2 := etree.NewElement("a:t")
	t2.Space = "a"
	t2.SetText("Paragraph 2")
	r2.AddChild(t2)
	p2.AddChild(r2)
	txBody.AddChild(p2)

	// Apply different bullet options to each
	_ = SetBulletMode(p1, "buChar")
	_ = SetBulletCharacter(p1, "•")

	_ = SetBulletMode(p2, "buAutoNum")
	_ = SetAutoNumberingScheme(p2, "stdAutoNum")

	// Verify p1
	mode1, _ := GetBulletMode(p1)
	if mode1 != "buChar" {
		t.Errorf("P1 mode: got %s, want buChar", mode1)
	}

	// Verify p2
	mode2, _ := GetBulletMode(p2)
	if mode2 != "buAutoNum" {
		t.Errorf("P2 mode: got %s, want buAutoNum", mode2)
	}
}
