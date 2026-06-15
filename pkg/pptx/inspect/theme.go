package inspect

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// ParseTheme parses a theme XML and extracts font and color scheme information
func ParseTheme(session opc.PackageSession, themeURI string) (*model.ThemeInfo, error) {
	if themeURI == "" {
		return nil, nil
	}

	// Read the theme XML
	themeDoc, err := session.ReadXMLPart(themeURI)
	if err != nil {
		// If theme doesn't exist, return nil instead of error
		return nil, nil
	}

	root := themeDoc.Root()
	if root == nil {
		return nil, fmt.Errorf("theme XML root element not found in %s", themeURI)
	}

	theme := &model.ThemeInfo{
		Name: root.SelectAttrValue("name", ""),
	}

	// Find themeElements
	themeElements := xmlx.FindChild(root, "http://schemas.openxmlformats.org/drawingml/2006/main", "themeElements")
	if themeElements != nil {
		// Parse color scheme
		if colorScheme, err := parseColorScheme(themeElements); err == nil && colorScheme != nil {
			theme.ColorScheme = colorScheme
		}

		// Parse font scheme
		if fontScheme, err := parseFontScheme(themeElements); err == nil && fontScheme != nil {
			theme.FontScheme = fontScheme
		}
	}

	return theme, nil
}

// parseColorScheme extracts color scheme information from themeElements
func parseColorScheme(themeElements *etree.Element) (*model.ColorScheme, error) {
	// Find clrScheme element
	clrScheme := xmlx.FindChild(themeElements, "http://schemas.openxmlformats.org/drawingml/2006/main", "clrScheme")
	if clrScheme == nil {
		return nil, nil
	}

	colorScheme := &model.ColorScheme{
		Name: clrScheme.SelectAttrValue("name", ""),
	}

	// Extract dark and light colors
	if color := extractColorFromElement(clrScheme, "dk1"); color != "" {
		colorScheme.Dark1 = color
	}
	if color := extractColorFromElement(clrScheme, "lt1"); color != "" {
		colorScheme.Light1 = color
	}
	if color := extractColorFromElement(clrScheme, "dk2"); color != "" {
		colorScheme.Dark2 = color
	}
	if color := extractColorFromElement(clrScheme, "lt2"); color != "" {
		colorScheme.Light2 = color
	}

	// Extract accent colors (accent1 through accent6)
	accentNames := []string{"accent1", "accent2", "accent3", "accent4", "accent5", "accent6"}
	accentFields := []*string{&colorScheme.Accent1, &colorScheme.Accent2, &colorScheme.Accent3, &colorScheme.Accent4, &colorScheme.Accent5, &colorScheme.Accent6}

	for i, accentName := range accentNames {
		if color := extractColorFromElement(clrScheme, accentName); color != "" {
			*accentFields[i] = color
		}
	}

	// Extract hyperlink colors
	if color := extractColorFromElement(clrScheme, "hlink"); color != "" {
		colorScheme.HypLink = color
	}
	if color := extractColorFromElement(clrScheme, "folHlink"); color != "" {
		colorScheme.FolLink = color
	}

	return colorScheme, nil
}

// parseFontScheme extracts font scheme information from themeElements
func parseFontScheme(themeElements *etree.Element) (*model.FontScheme, error) {
	// Find fontScheme element
	fontScheme := xmlx.FindChild(themeElements, "http://schemas.openxmlformats.org/drawingml/2006/main", "fontScheme")
	if fontScheme == nil {
		return nil, nil
	}

	scheme := &model.FontScheme{
		Name: fontScheme.SelectAttrValue("name", ""),
	}

	// Extract major font (Latin)
	if majorFont := extractFontFromElement(fontScheme, "majorFont", "latin"); majorFont != "" {
		scheme.MajorFont = majorFont
	}

	// Extract minor font (Latin)
	if minorFont := extractFontFromElement(fontScheme, "minorFont", "latin"); minorFont != "" {
		scheme.MinorFont = minorFont
	}

	// Extract East Asian major font
	if eaMajor := extractFontFromElement(fontScheme, "majorFont", "ea"); eaMajor != "" {
		scheme.EastAsianMajorFont = eaMajor
	}

	// Extract East Asian minor font
	if eaMinor := extractFontFromElement(fontScheme, "minorFont", "ea"); eaMinor != "" {
		scheme.EastAsianMinorFont = eaMinor
	}

	// Extract Complex Script major font
	if csMajor := extractFontFromElement(fontScheme, "majorFont", "cs"); csMajor != "" {
		scheme.ComplexScriptMajorFont = csMajor
	}

	// Extract Complex Script minor font
	if csMinor := extractFontFromElement(fontScheme, "minorFont", "cs"); csMinor != "" {
		scheme.ComplexScriptMinorFont = csMinor
	}

	return scheme, nil
}

// extractColorFromElement extracts RGB value from a color element (srgbClr or sysClr)
func extractColorFromElement(parent *etree.Element, colorName string) string {
	// Find the color element
	colorElem := xmlx.FindChild(parent, "http://schemas.openxmlformats.org/drawingml/2006/main", colorName)
	if colorElem == nil {
		return ""
	}

	// Try srgbClr first (RGB color)
	srgbClr := xmlx.FindChild(colorElem, "http://schemas.openxmlformats.org/drawingml/2006/main", "srgbClr")
	if srgbClr != nil {
		rgb := srgbClr.SelectAttrValue("val", "")
		if rgb != "" {
			return rgb
		}
	}

	// Try sysClr (system color)
	sysClr := xmlx.FindChild(colorElem, "http://schemas.openxmlformats.org/drawingml/2006/main", "sysClr")
	if sysClr != nil {
		// Use lastClr attribute which contains the computed RGB
		rgb := sysClr.SelectAttrValue("lastClr", "")
		if rgb != "" {
			return rgb
		}
	}

	return ""
}

// extractFontFromElement extracts the typeface from a font element
// fontType should be "majorFont" or "minorFont"
// scriptType should be "latin", "ea", or "cs"
func extractFontFromElement(parent *etree.Element, fontType, scriptType string) string {
	// Find the font element (majorFont or minorFont)
	fontElem := xmlx.FindChild(parent, "http://schemas.openxmlformats.org/drawingml/2006/main", fontType)
	if fontElem == nil {
		return ""
	}

	// Look for the specific script typeface
	script := xmlx.FindChild(fontElem, "http://schemas.openxmlformats.org/drawingml/2006/main", scriptType)
	if script != nil {
		typeface := script.SelectAttrValue("typeface", "")
		if typeface != "" {
			return typeface
		}
	}

	// If no specific script found and scriptType is "latin", look for first available font element
	if scriptType == "latin" {
		fonts := xmlx.FindChildren(fontElem, "http://schemas.openxmlformats.org/drawingml/2006/main", "font")
		if len(fonts) > 0 {
			typeface := fonts[0].SelectAttrValue("typeface", "")
			if typeface != "" {
				return typeface
			}
		}
	}

	return ""
}

// ExtractDefaultTextStyleInfo extracts default text style information
// for a master or layout given its theme reference
func ExtractDefaultTextStyleInfo(session opc.PackageSession, themeURI string) *model.DefaultTextStyleInfo {
	if themeURI == "" {
		return nil
	}

	theme, err := ParseTheme(session, themeURI)
	if err != nil || theme == nil {
		return nil
	}

	info := &model.DefaultTextStyleInfo{
		ThemeName: theme.Name,
	}

	if theme.FontScheme != nil {
		info.MajorFont = theme.FontScheme.MajorFont
		info.MinorFont = theme.FontScheme.MinorFont
	}

	if theme.ColorScheme != nil {
		// Collect accent colors
		colors := []string{}
		if theme.ColorScheme.Accent1 != "" {
			colors = append(colors, theme.ColorScheme.Accent1)
		}
		if theme.ColorScheme.Accent2 != "" {
			colors = append(colors, theme.ColorScheme.Accent2)
		}
		if theme.ColorScheme.Accent3 != "" {
			colors = append(colors, theme.ColorScheme.Accent3)
		}
		if theme.ColorScheme.Accent4 != "" {
			colors = append(colors, theme.ColorScheme.Accent4)
		}
		if theme.ColorScheme.Accent5 != "" {
			colors = append(colors, theme.ColorScheme.Accent5)
		}
		if theme.ColorScheme.Accent6 != "" {
			colors = append(colors, theme.ColorScheme.Accent6)
		}
		if len(colors) > 0 {
			info.AccentColors = colors
		}
	}

	return info
}
