package mutate

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// Note: createDrawingMLElement is defined in paragraph.go

// BulletMutationOptions holds options for modifying bullet properties
type BulletMutationOptions struct {
	Mode                string  // "buNone", "buChar", "buAutoNum"
	Character           *string // Character if mode is "buChar"
	AutoNumberingScheme *string // Scheme if mode is "buAutoNum" (e.g., "stdAutoNum")
	FontFamily          *string // Font family for the bullet
	FontSize            *int32  // Font size in points * 100
	Color               *string // Bullet color as RGB hex
}

// SetBulletMode sets the bullet mode for a paragraph
// Mode can be: "buNone" (no bullets), "buChar" (character bullet), "buAutoNum" (auto-numbering)
func SetBulletMode(pElem *etree.Element, mode string) error {
	validModes := map[string]bool{
		"buNone":    true,
		"buChar":    true,
		"buAutoNum": true,
	}

	if !validModes[mode] {
		return fmt.Errorf("invalid bullet mode: %s (must be 'buNone', 'buChar', or 'buAutoNum')", mode)
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Remove existing bullet elements
	removeBulletElements(pPr)

	// Add the appropriate bullet element based on mode
	switch mode {
	case "buNone":
		buNone := createDrawingMLElement("buNone")
		pPr.AddChild(buNone)
	case "buChar":
		// buChar requires a character; add placeholder
		buChar := createDrawingMLElement("buChar")
		buChar.CreateAttr("char", "•") // Default bullet character
		pPr.AddChild(buChar)
	case "buAutoNum":
		// buAutoNum requires a type; add placeholder
		buAutoNum := createDrawingMLElement("buAutoNum")
		buAutoNum.CreateAttr("type", "stdAutoNum") // Default scheme
		pPr.AddChild(buAutoNum)
	}

	return nil
}

// SetBulletCharacter sets the bullet character for a paragraph
// Requires the bullet mode to be "buChar"
func SetBulletCharacter(pElem *etree.Element, character string) error {
	if character == "" {
		return fmt.Errorf("bullet character cannot be empty")
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Find or create buChar element
	buChar := xmlx.FindChild(pPr, ns.NsA, "buChar")
	if buChar == nil {
		// Create new buChar element
		buChar = createDrawingMLElement("buChar")
		pPr.AddChild(buChar)
	}

	buChar.CreateAttr("char", character)
	return nil
}

// GetBulletCharacter gets the bullet character from a paragraph
func GetBulletCharacter(pElem *etree.Element) (*string, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	buChar := xmlx.FindChild(pPr, ns.NsA, "buChar")
	if buChar == nil {
		return nil, fmt.Errorf("no bullet character found")
	}

	if char, ok := xmlx.GetAttr(buChar, "char"); ok {
		return &char, nil
	}
	return nil, fmt.Errorf("bullet character attribute not found")
}

// SetAutoNumberingScheme sets the auto-numbering scheme for a paragraph
// Requires the bullet mode to be "buAutoNum"
func SetAutoNumberingScheme(pElem *etree.Element, scheme string) error {
	if scheme == "" {
		return fmt.Errorf("auto-numbering scheme cannot be empty")
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Find or create buAutoNum element
	buAutoNum := xmlx.FindChild(pPr, ns.NsA, "buAutoNum")
	if buAutoNum == nil {
		// Create new buAutoNum element
		buAutoNum = createDrawingMLElement("buAutoNum")
		pPr.AddChild(buAutoNum)
	}

	buAutoNum.CreateAttr("type", scheme)
	return nil
}

// GetAutoNumberingScheme gets the auto-numbering scheme from a paragraph
func GetAutoNumberingScheme(pElem *etree.Element) (*string, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	buAutoNum := xmlx.FindChild(pPr, ns.NsA, "buAutoNum")
	if buAutoNum == nil {
		return nil, fmt.Errorf("no auto-numbering element found")
	}

	if scheme, ok := xmlx.GetAttr(buAutoNum, "type"); ok {
		return &scheme, nil
	}
	return nil, fmt.Errorf("auto-numbering scheme attribute not found")
}

// SetBulletFontSize sets the font size of the bullet character
// Size is in points * 100 (e.g., 2400 = 24pt)
func SetBulletFontSize(pElem *etree.Element, sizeInHundredths int32) error {
	if sizeInHundredths < 0 {
		return fmt.Errorf("invalid font size: %d (must be non-negative)", sizeInHundredths)
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Remove existing bullet size elements (buSzPts and buSzPct)
	for {
		buSzPts := xmlx.FindChild(pPr, ns.NsA, "buSzPts")
		if buSzPts == nil {
			break
		}
		pPr.RemoveChild(buSzPts)
	}
	for {
		buSzPct := xmlx.FindChild(pPr, ns.NsA, "buSzPct")
		if buSzPct == nil {
			break
		}
		pPr.RemoveChild(buSzPct)
	}

	if sizeInHundredths > 0 {
		// Add new font size specification
		// Note: OOXML uses buSzPcts (percentage) or buSzPts (points)
		buSzPts := createDrawingMLElement("buSzPts")
		buSzPts.CreateAttr("val", strconv.FormatInt(int64(sizeInHundredths), 10))
		pPr.AddChild(buSzPts)
	}

	return nil
}

// GetBulletFontSize gets the font size of the bullet character
func GetBulletFontSize(pElem *etree.Element) (*int32, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	// Check for buSzPts (font size in points)
	if buSzPts := xmlx.FindChild(pPr, ns.NsA, "buSzPts"); buSzPts != nil {
		if val, ok := xmlx.GetAttr(buSzPts, "val"); ok {
			if parsed, err := strconv.ParseInt(val, 10, 32); err == nil {
				val32 := int32(parsed)
				return &val32, nil
			}
		}
	}

	// Check for buSzPct (font size in percentage)
	if buSzPct := xmlx.FindChild(pPr, ns.NsA, "buSzPct"); buSzPct != nil {
		if val, ok := xmlx.GetAttr(buSzPct, "val"); ok {
			if parsed, err := strconv.ParseInt(val, 10, 32); err == nil {
				val32 := int32(parsed)
				return &val32, nil
			}
		}
	}

	return nil, fmt.Errorf("bullet font size not found")
}

// SetBulletFontFamily sets the font family for the bullet character
func SetBulletFontFamily(pElem *etree.Element, fontFamily string) error {
	if fontFamily == "" {
		return fmt.Errorf("font family cannot be empty")
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Remove existing buFont element
	for {
		buFont := xmlx.FindChild(pPr, ns.NsA, "buFont")
		if buFont == nil {
			break
		}
		pPr.RemoveChild(buFont)
	}

	// Add new buFont element
	buFont := createDrawingMLElement("buFont")
	buFont.CreateAttr("typeface", fontFamily)
	pPr.AddChild(buFont)

	return nil
}

// GetBulletFontFamily gets the font family of the bullet character
func GetBulletFontFamily(pElem *etree.Element) (*string, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	buFont := xmlx.FindChild(pPr, ns.NsA, "buFont")
	if buFont == nil {
		return nil, fmt.Errorf("no bullet font found")
	}

	if typeface, ok := xmlx.GetAttr(buFont, "typeface"); ok {
		return &typeface, nil
	}
	return nil, fmt.Errorf("bullet font typeface attribute not found")
}

// SetBulletColor sets the color of the bullet character
// Color should be an RGB hex value (e.g., "FF0000" for red)
func SetBulletColor(pElem *etree.Element, color string) error {
	if color == "" {
		return fmt.Errorf("color cannot be empty")
	}

	pPr := getParagraphProperties(pElem)
	if pPr == nil {
		return fmt.Errorf("paragraph has no properties element")
	}

	// Remove existing buClr element
	for {
		buClr := xmlx.FindChild(pPr, ns.NsA, "buClr")
		if buClr == nil {
			break
		}
		pPr.RemoveChild(buClr)
	}

	// Add new buClr element with solid fill
	buClr := createDrawingMLElement("buClr")
	solidFill := createDrawingMLElement("solidFill")
	srgbClr := createDrawingMLElement("srgbClr")
	srgbClr.CreateAttr("val", color)
	solidFill.AddChild(srgbClr)
	buClr.AddChild(solidFill)
	pPr.AddChild(buClr)

	return nil
}

// GetBulletColor gets the color of the bullet character
func GetBulletColor(pElem *etree.Element) (*string, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return nil, fmt.Errorf("paragraph has no properties element")
	}

	buClr := xmlx.FindChild(pPr, ns.NsA, "buClr")
	if buClr == nil {
		return nil, fmt.Errorf("no bullet color found")
	}

	// Look for solid fill with srgbClr
	solidFill := xmlx.FindChild(buClr, ns.NsA, "solidFill")
	if solidFill == nil {
		return nil, fmt.Errorf("no solid fill in bullet color")
	}

	srgbClr := xmlx.FindChild(solidFill, ns.NsA, "srgbClr")
	if srgbClr == nil {
		return nil, fmt.Errorf("no srgbClr in bullet color")
	}

	if val, ok := xmlx.GetAttr(srgbClr, "val"); ok {
		return &val, nil
	}
	return nil, fmt.Errorf("bullet color value attribute not found")
}

// GetBulletMode gets the current bullet mode from a paragraph
func GetBulletMode(pElem *etree.Element) (string, error) {
	pPr := xmlx.FindChild(pElem, ns.NsA, "pPr")
	if pPr == nil {
		return "", fmt.Errorf("paragraph has no properties element")
	}

	if xmlx.FindChild(pPr, ns.NsA, "buNone") != nil {
		return "buNone", nil
	}
	if xmlx.FindChild(pPr, ns.NsA, "buChar") != nil {
		return "buChar", nil
	}
	if xmlx.FindChild(pPr, ns.NsA, "buAutoNum") != nil {
		return "buAutoNum", nil
	}

	return "", fmt.Errorf("bullet mode not found")
}

// ApplyBulletOptions applies multiple bullet options at once
func ApplyBulletOptions(pElem *etree.Element, opts *BulletMutationOptions) error {
	if opts == nil {
		return nil
	}

	// Set bullet mode first
	if opts.Mode != "" {
		if err := SetBulletMode(pElem, opts.Mode); err != nil {
			return err
		}
	}

	// Set character if provided
	if opts.Character != nil {
		if err := SetBulletCharacter(pElem, *opts.Character); err != nil {
			return err
		}
	}

	// Set auto-numbering scheme if provided
	if opts.AutoNumberingScheme != nil {
		if err := SetAutoNumberingScheme(pElem, *opts.AutoNumberingScheme); err != nil {
			return err
		}
	}

	// Set font family if provided
	if opts.FontFamily != nil {
		if err := SetBulletFontFamily(pElem, *opts.FontFamily); err != nil {
			return err
		}
	}

	// Set font size if provided
	if opts.FontSize != nil {
		if err := SetBulletFontSize(pElem, *opts.FontSize); err != nil {
			return err
		}
	}

	// Set color if provided
	if opts.Color != nil {
		if err := SetBulletColor(pElem, *opts.Color); err != nil {
			return err
		}
	}

	return nil
}

// removeBulletElements removes all bullet-related elements from a paragraph properties element
func removeBulletElements(pPr *etree.Element) {
	// Remove all bullet-related elements
	bulletElements := []string{"buNone", "buChar", "buAutoNum"}
	for _, elemName := range bulletElements {
		for {
			elem := xmlx.FindChild(pPr, ns.NsA, elemName)
			if elem == nil {
				break
			}
			pPr.RemoveChild(elem)
		}
	}
}
