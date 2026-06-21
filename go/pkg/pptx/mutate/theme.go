package mutate

import (
	"fmt"
	"regexp"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

const (
	drawingMLNS = "http://schemas.openxmlformats.org/drawingml/2006/main"
)

// UpdateThemeColorRequest holds parameters for updating a theme color
type UpdateThemeColorRequest struct {
	// Package session
	Package opc.PackageSession

	// Theme URI (typically "/ppt/theme/theme1.xml")
	ThemeURI string

	// Color name: dk1, lt1, dk2, lt2, accent1-accent6, hlink, folHlink
	ColorName string

	// Hex color value (6 characters, e.g., "FF0000" for red)
	HexValue string
}

// UpdateThemeFontRequest holds parameters for updating theme fonts
type UpdateThemeFontRequest struct {
	// Package session
	Package opc.PackageSession

	// Theme URI (typically "/ppt/theme/theme1.xml")
	ThemeURI string

	// Major font typeface (optional, leave empty to skip)
	MajorFont string

	// Minor font typeface (optional, leave empty to skip)
	MinorFont string
}

// Valid color names that can be updated in theme
var validColorNames = map[string]bool{
	"dk1":      true,
	"lt1":      true,
	"dk2":      true,
	"lt2":      true,
	"accent1":  true,
	"accent2":  true,
	"accent3":  true,
	"accent4":  true,
	"accent5":  true,
	"accent6":  true,
	"hlink":    true,
	"folHlink": true,
}

// isValidHexColor validates that a color is a valid 6-character hex string
func isValidHexColor(hex string) bool {
	// Must be 6 characters of hex digits
	if len(hex) != 6 {
		return false
	}
	matched, _ := regexp.MatchString("^[0-9A-Fa-f]{6}$", hex)
	return matched
}

// isValidColorName validates that a color name is in the allowed list
func isValidColorName(name string) bool {
	return validColorNames[name]
}

// UpdateThemeColor updates a single color in the theme's color scheme
// Validates both the color name and hex value before modification
func UpdateThemeColor(req *UpdateThemeColorRequest) error {
	if req.Package == nil {
		return fmt.Errorf("package session is required")
	}

	if req.ThemeURI == "" {
		return fmt.Errorf("theme URI is required")
	}

	if req.ColorName == "" {
		return fmt.Errorf("color name is required")
	}

	if !isValidColorName(req.ColorName) {
		return fmt.Errorf("invalid color name '%s'; must be one of: %s",
			req.ColorName, formatColorNames())
	}

	if req.HexValue == "" {
		return fmt.Errorf("hex color value is required")
	}

	if !isValidHexColor(req.HexValue) {
		return fmt.Errorf("invalid hex color '%s'; must be 6 hexadecimal characters (e.g., FF0000)",
			req.HexValue)
	}

	// Read the theme XML
	themeDoc, err := req.Package.ReadXMLPart(req.ThemeURI)
	if err != nil {
		return fmt.Errorf("failed to read theme from %s: %w", req.ThemeURI, err)
	}

	root := themeDoc.Root()
	if root == nil {
		return fmt.Errorf("theme XML root element not found in %s", req.ThemeURI)
	}

	// Find themeElements
	themeElements := xmlx.FindChild(root, drawingMLNS, "themeElements")
	if themeElements == nil {
		return fmt.Errorf("themeElements not found in theme")
	}

	// Find clrScheme
	clrScheme := xmlx.FindChild(themeElements, drawingMLNS, "clrScheme")
	if clrScheme == nil {
		return fmt.Errorf("clrScheme not found in themeElements")
	}

	// Find the color element (e.g., accent1, dk1, etc.)
	colorElem := xmlx.FindChild(clrScheme, drawingMLNS, req.ColorName)
	if colorElem == nil {
		return fmt.Errorf("color '%s' not found in color scheme", req.ColorName)
	}

	// Remove existing color definitions (both srgbClr and sysClr)
	for i := len(colorElem.Child) - 1; i >= 0; i-- {
		if elem, ok := colorElem.Child[i].(*etree.Element); ok {
			if xmlx.ElementMatches(elem, drawingMLNS, "srgbClr") ||
				xmlx.ElementMatches(elem, drawingMLNS, "sysClr") {
				colorElem.RemoveChild(elem)
			}
		}
	}

	// Create new srgbClr element with the new hex value
	// Detect the namespace prefix used in the document by inspecting the color element itself or a sibling
	var prefix string

	// Check the colorElem's tag to detect the prefix (e.g., "a:accent1")
	if idx := strings.Index(colorElem.Tag, ":"); idx > 0 {
		prefix = colorElem.Tag[:idx+1]
	}

	if prefix == "" {
		// Default to "a:" if we can't detect the prefix
		prefix = "a:"
	}

	newColor := colorElem.CreateElement(prefix + "srgbClr")
	if newColor == nil {
		return fmt.Errorf("failed to create srgbClr element")
	}

	newColor.CreateAttr("val", req.HexValue)

	// Write back to package
	if err := req.Package.ReplaceXMLPart(req.ThemeURI, themeDoc); err != nil {
		return fmt.Errorf("failed to write theme: %w", err)
	}

	return nil
}

// UpdateThemeFont updates the major and/or minor Latin fonts in the theme's font scheme.
// East Asian and Complex Script font slots are outside the current command surface.
func UpdateThemeFont(req *UpdateThemeFontRequest) error {
	if req.Package == nil {
		return fmt.Errorf("package session is required")
	}

	if req.ThemeURI == "" {
		return fmt.Errorf("theme URI is required")
	}

	if req.MajorFont == "" && req.MinorFont == "" {
		return fmt.Errorf("at least one of majorFont or minorFont must be provided")
	}

	// Validate font names are not empty if provided
	if req.MajorFont == "" && req.MinorFont != "" {
		// OK - just updating minor
	} else if req.MajorFont != "" && req.MinorFont == "" {
		// OK - just updating major
	}

	// Read the theme XML
	themeDoc, err := req.Package.ReadXMLPart(req.ThemeURI)
	if err != nil {
		return fmt.Errorf("failed to read theme from %s: %w", req.ThemeURI, err)
	}

	root := themeDoc.Root()
	if root == nil {
		return fmt.Errorf("theme XML root element not found in %s", req.ThemeURI)
	}

	// Find themeElements
	themeElements := xmlx.FindChild(root, drawingMLNS, "themeElements")
	if themeElements == nil {
		return fmt.Errorf("themeElements not found in theme")
	}

	// Find fontScheme
	fontScheme := xmlx.FindChild(themeElements, drawingMLNS, "fontScheme")
	if fontScheme == nil {
		return fmt.Errorf("fontScheme not found in themeElements")
	}

	// Update major font if provided
	if req.MajorFont != "" {
		if err := updateFontInScheme(fontScheme, "majorFont", req.MajorFont); err != nil {
			return fmt.Errorf("failed to update major font: %w", err)
		}
	}

	// Update minor font if provided
	if req.MinorFont != "" {
		if err := updateFontInScheme(fontScheme, "minorFont", req.MinorFont); err != nil {
			return fmt.Errorf("failed to update minor font: %w", err)
		}
	}

	// Write back to package
	if err := req.Package.ReplaceXMLPart(req.ThemeURI, themeDoc); err != nil {
		return fmt.Errorf("failed to write theme: %w", err)
	}

	return nil
}

// updateFontInScheme updates either majorFont or minorFont within a fontScheme element
// This preserves all non-latin font definitions (EA, CS, and script-specific fonts)
func updateFontInScheme(fontScheme *etree.Element, fontType string, newTypeface string) error {
	// Find the font element (majorFont or minorFont)
	fontElem := xmlx.FindChild(fontScheme, drawingMLNS, fontType)
	if fontElem == nil {
		return fmt.Errorf("%s element not found in fontScheme", fontType)
	}

	// Find the latin element
	latin := xmlx.FindChild(fontElem, drawingMLNS, "latin")
	if latin == nil {
		// Create latin if it doesn't exist, inserting at the beginning
		// (latin should come first per OOXML spec)
		// Detect the namespace prefix used in the document
		var prefix string
		if idx := strings.Index(fontElem.Tag, ":"); idx > 0 {
			prefix = fontElem.Tag[:idx+1]
		}
		if prefix == "" {
			prefix = "a:"
		}

		latin = fontElem.CreateElement(prefix + "latin")
		// Move it to the beginning by removing and reinserting
		fontElem.RemoveChild(latin)
		fontElem.InsertChildAt(0, latin)
	}

	// Update the typeface attribute
	xmlx.SetAttr(latin, "typeface", newTypeface)

	return nil
}

// formatColorNames returns a formatted list of valid color names for error messages
func formatColorNames() string {
	names := []string{
		"dk1", "lt1", "dk2", "lt2",
		"accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
		"hlink", "folHlink",
	}
	return strings.Join(names, ", ")
}

// SlideColorOverrideRequest holds parameters for applying color overrides to a specific slide
type SlideColorOverrideRequest struct {
	// Package session
	Package opc.PackageSession

	// Slide URI (e.g., "/ppt/slides/slide1.xml")
	SlideURI string

	// Color name: dk1, lt1, dk2, lt2, accent1-accent6, hlink, folHlink
	ColorName string

	// Hex color value (6 characters, e.g., "FF0000" for red)
	HexValue string
}

// ApplySlideColorOverride applies a color override to a specific slide without changing the theme
// This creates a color override in the slide's color scheme that takes precedence over theme colors
func ApplySlideColorOverride(req *SlideColorOverrideRequest) error {
	if req.Package == nil {
		return fmt.Errorf("package session is required")
	}

	if req.SlideURI == "" {
		return fmt.Errorf("slide URI is required")
	}

	if req.ColorName == "" {
		return fmt.Errorf("color name is required")
	}

	if !isValidColorName(req.ColorName) {
		return fmt.Errorf("invalid color name '%s'; must be one of: %s",
			req.ColorName, formatColorNames())
	}

	if req.HexValue == "" {
		return fmt.Errorf("hex color value is required")
	}

	if !isValidHexColor(req.HexValue) {
		return fmt.Errorf("invalid hex color '%s'; must be 6 hexadecimal characters (e.g., FF0000)",
			req.HexValue)
	}

	// Read the slide XML
	slideDoc, err := req.Package.ReadXMLPart(req.SlideURI)
	if err != nil {
		return fmt.Errorf("failed to read slide from %s: %w", req.SlideURI, err)
	}

	root := slideDoc.Root()
	if root == nil {
		return fmt.Errorf("slide XML root element not found in %s", req.SlideURI)
	}

	// Get or create the cSld (common slide data) element
	cSld := xmlx.FindChild(root, drawingMLNS, "cSld")
	if cSld == nil {
		return fmt.Errorf("cSld element not found in slide %s", req.SlideURI)
	}

	// Find the clrMapOvr (color map override) element or create it
	clrMapOvr := xmlx.FindChild(cSld, drawingMLNS, "clrMapOvr")
	if clrMapOvr == nil {
		// Create clrMapOvr at the beginning of cSld (after bg if present)
		var prefix string
		if idx := strings.Index(cSld.Tag, ":"); idx > 0 {
			prefix = cSld.Tag[:idx+1]
		} else {
			prefix = "p:"
		}
		clrMapOvr = cSld.CreateElement(prefix + "clrMapOvr")
		// Move it to proper position (after bg)
		cSld.RemoveChild(clrMapOvr)
		bgElem := xmlx.FindChild(cSld, drawingMLNS, "bg")
		var insertIdx int
		if bgElem != nil {
			for i, child := range cSld.Child {
				if child == bgElem {
					insertIdx = i + 1
					break
				}
			}
		}
		cSld.InsertChildAt(insertIdx, clrMapOvr)
	}

	// Get or create schemeClr element within clrMapOvr
	var nsPrefix string
	if idx := strings.Index(clrMapOvr.Tag, ":"); idx > 0 {
		nsPrefix = clrMapOvr.Tag[:idx+1]
	} else {
		nsPrefix = "a:"
	}

	// Find existing override for this color or create one
	colorOverride := xmlx.FindChild(clrMapOvr, drawingMLNS, req.ColorName)
	if colorOverride == nil {
		colorOverride = clrMapOvr.CreateElement(nsPrefix + req.ColorName)
	} else {
		// Clear existing color values
		for i := len(colorOverride.Child) - 1; i >= 0; i-- {
			if elem, ok := colorOverride.Child[i].(*etree.Element); ok {
				colorOverride.RemoveChild(elem)
			}
		}
	}

	// Add srgbClr element with the override color
	srgbClr := colorOverride.CreateElement(nsPrefix + "srgbClr")
	if srgbClr == nil {
		return fmt.Errorf("failed to create srgbClr element for color override")
	}
	srgbClr.CreateAttr("val", req.HexValue)

	// Write back to package
	if err := req.Package.ReplaceXMLPart(req.SlideURI, slideDoc); err != nil {
		return fmt.Errorf("failed to write slide: %w", err)
	}

	return nil
}

// RemoveSlideColorOverrides removes all color overrides from a slide, reverting to theme colors
func RemoveSlideColorOverrides(pkg opc.PackageSession, slideURI string) error {
	if pkg == nil {
		return fmt.Errorf("package session is required")
	}

	if slideURI == "" {
		return fmt.Errorf("slide URI is required")
	}

	// Read the slide XML
	slideDoc, err := pkg.ReadXMLPart(slideURI)
	if err != nil {
		return fmt.Errorf("failed to read slide from %s: %w", slideURI, err)
	}

	root := slideDoc.Root()
	if root == nil {
		return fmt.Errorf("slide XML root element not found in %s", slideURI)
	}

	// Find and remove the clrMapOvr element
	cSld := xmlx.FindChild(root, drawingMLNS, "cSld")
	if cSld == nil {
		// No color map overrides to remove
		return nil
	}

	clrMapOvr := xmlx.FindChild(cSld, drawingMLNS, "clrMapOvr")
	if clrMapOvr != nil {
		cSld.RemoveChild(clrMapOvr)
	}

	// Write back to package
	if err := pkg.ReplaceXMLPart(slideURI, slideDoc); err != nil {
		return fmt.Errorf("failed to write slide: %w", err)
	}

	return nil
}
