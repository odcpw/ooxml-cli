package mutate

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// createDrawingMLElement creates an element in the DrawingML namespace
func createDrawingMLElement(localName string) *etree.Element {
	elem := etree.NewElement(localName)
	elem.Space = "a"
	return elem
}

// ParagraphMutationOptions holds options for modifying paragraph properties
type ParagraphMutationOptions struct {
	Level       *int32  // Paragraph indent level (0-8)
	Alignment   *string // Alignment: "l", "ctr", "r", "just", "dist"
	SpaceBefore *int64  // Spacing before paragraph in EMU
	SpaceAfter  *int64  // Spacing after paragraph in EMU
	LineSpacing *int64  // Line spacing in EMU
}

// SetParagraphLevel sets the indent level of a paragraph
// Level must be between 0 and 8
func SetParagraphLevel(pElem *etree.Element, level int32) error {
	if level < 0 || level > 8 {
		return fmt.Errorf("invalid paragraph level: %d (must be 0-8)", level)
	}

	// Get or create pPr element
	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Set the lvl attribute
	pPr.CreateAttr("lvl", strconv.FormatInt(int64(level), 10))
	return nil
}

// GetParagraphLevel gets the indent level of a paragraph
// Returns the level (0-8) or an error if not set
func GetParagraphLevel(pElem *etree.Element) (*int32, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	if level, ok := xmlx.GetAttr(pPr, "lvl"); ok {
		if val, err := strconv.Atoi(level); err == nil {
			val32 := int32(val)
			return &val32, nil
		}
	}
	return nil, fmt.Errorf("paragraph level not found")
}

// SetParagraphAlignment sets the text alignment of a paragraph
// Alignment can be: "l" (left), "ctr" (center), "r" (right), "just" (justified), "dist" (distributed)
func SetParagraphAlignment(pElem *etree.Element, alignment string) error {
	validAlignments := map[string]bool{
		"l":    true,
		"ctr":  true,
		"r":    true,
		"just": true,
		"dist": true,
	}

	if !validAlignments[alignment] {
		return fmt.Errorf("invalid alignment: %s (must be 'l', 'ctr', 'r', 'just', or 'dist')", alignment)
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	pPr.CreateAttr("algn", alignment)
	return nil
}

// GetParagraphAlignment gets the text alignment of a paragraph
func GetParagraphAlignment(pElem *etree.Element) (string, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return "", fmt.Errorf("paragraph has no properties element")
	}

	if alignment, ok := xmlx.GetAttr(pPr, "algn"); ok {
		return alignment, nil
	}
	return "", fmt.Errorf("paragraph alignment not found")
}

// SetParagraphSpacing sets spacing before and/or after a paragraph
// Values are in EMU (English Metric Units)
func SetParagraphSpacing(pElem *etree.Element, spaceBefore, spaceAfter *int64) error {
	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Remove existing spacing elements
	for {
		spcBef := xmlx.FindChild(pPr, ns.NsA, "spcBef")
		if spcBef == nil {
			break
		}
		pPr.RemoveChild(spcBef)
	}
	for {
		spcAft := xmlx.FindChild(pPr, ns.NsA, "spcAft")
		if spcAft == nil {
			break
		}
		pPr.RemoveChild(spcAft)
	}

	// Add spaceBefore if specified
	if spaceBefore != nil {
		spcBef := createDrawingMLElement("spcBef")
		spcElem := createDrawingMLElement("spcPts")
		spcElem.CreateAttr("val", strconv.FormatInt(*spaceBefore, 10))
		spcBef.AddChild(spcElem)
		pPr.AddChild(spcBef)
	}

	// Add spaceAfter if specified
	if spaceAfter != nil {
		spcAft := createDrawingMLElement("spcAft")
		spcElem := createDrawingMLElement("spcPts")
		spcElem.CreateAttr("val", strconv.FormatInt(*spaceAfter, 10))
		spcAft.AddChild(spcElem)
		pPr.AddChild(spcAft)
	}

	return nil
}

// GetParagraphSpacing gets spacing before and after a paragraph
func GetParagraphSpacing(pElem *etree.Element) (spaceBefore, spaceAfter *int64, err error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, nil, fmt.Errorf("paragraph has no properties element")
	}

	// Get space before
	if spcBef := xmlx.FindChild(pPr, ns.NsA, "spcBef"); spcBef != nil {
		if val := extractSpacingValue(spcBef); val != nil {
			spaceBefore = val
		}
	}

	// Get space after
	if spcAft := xmlx.FindChild(pPr, ns.NsA, "spcAft"); spcAft != nil {
		if val := extractSpacingValue(spcAft); val != nil {
			spaceAfter = val
		}
	}

	return spaceBefore, spaceAfter, nil
}

// SetParagraphLineSpacing sets the line spacing for a paragraph
// Value is in EMU
func SetParagraphLineSpacing(pElem *etree.Element, lineSpacing int64) error {
	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Remove existing line spacing
	for {
		lnSpc := xmlx.FindChild(pPr, ns.NsA, "lnSpc")
		if lnSpc == nil {
			break
		}
		pPr.RemoveChild(lnSpc)
	}

	// Add new line spacing
	lnSpc := createDrawingMLElement("lnSpc")
	spcPts := createDrawingMLElement("spcPts")
	spcPts.CreateAttr("val", strconv.FormatInt(lineSpacing, 10))
	lnSpc.AddChild(spcPts)
	pPr.AddChild(lnSpc)

	return nil
}

// GetParagraphLineSpacing gets the line spacing of a paragraph
func GetParagraphLineSpacing(pElem *etree.Element) (*int64, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	if lnSpc := xmlx.FindChild(pPr, ns.NsA, "lnSpc"); lnSpc != nil {
		return extractSpacingValue(lnSpc), nil
	}
	return nil, fmt.Errorf("line spacing not found")
}

// ApplyParagraphOptions applies multiple paragraph options at once
func ApplyParagraphOptions(pElem *etree.Element, opts *ParagraphMutationOptions) error {
	if opts == nil {
		return nil
	}

	if opts.Level != nil {
		if err := SetParagraphLevel(pElem, *opts.Level); err != nil {
			return err
		}
	}

	if opts.Alignment != nil {
		if err := SetParagraphAlignment(pElem, *opts.Alignment); err != nil {
			return err
		}
	}

	if opts.SpaceBefore != nil || opts.SpaceAfter != nil {
		if err := SetParagraphSpacing(pElem, opts.SpaceBefore, opts.SpaceAfter); err != nil {
			return err
		}
	}

	if opts.LineSpacing != nil {
		if err := SetParagraphLineSpacing(pElem, *opts.LineSpacing); err != nil {
			return err
		}
	}

	return nil
}

// getParagraphProperties gets or creates the a:pPr element for a paragraph
func getParagraphProperties(pElem *etree.Element) *etree.Element {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		// Create pPr element at the beginning
		pPr = createDrawingMLElement("pPr")

		// Insert at the beginning by removing and re-adding all children
		children := pElem.ChildElements()
		for _, child := range children {
			pElem.RemoveChild(child)
		}
		pElem.AddChild(pPr)
		for _, child := range children {
			pElem.AddChild(child)
		}
	}
	return pPr
}

// extractSpacingValue extracts the numeric value from a spacing element
func extractSpacingValue(spacingElem *etree.Element) *int64 {
	if spacingElem == nil {
		return nil
	}

	// Check for a:spcPts (space in points)
	if spcPts := xmlx.FindChild(spacingElem, ns.NsA, "spcPts"); spcPts != nil {
		if val, ok := xmlx.GetAttr(spcPts, "val"); ok {
			if parsed, err := strconv.ParseInt(val, 10, 64); err == nil {
				return &parsed
			}
		}
	}

	// Check for a:spcPct (space in percent)
	if spcPct := xmlx.FindChild(spacingElem, ns.NsA, "spcPct"); spcPct != nil {
		if val, ok := xmlx.GetAttr(spcPct, "val"); ok {
			if parsed, err := strconv.ParseInt(val, 10, 64); err == nil {
				return &parsed
			}
		}
	}

	return nil
}
