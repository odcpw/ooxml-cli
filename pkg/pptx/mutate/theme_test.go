package mutate

import (
	"fmt"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

const (
	testDrawingMLNS = "http://schemas.openxmlformats.org/drawingml/2006/main"
)

// mockThemePackageSession is a minimal mock for testing theme mutations
type mockThemePackageSession struct {
	xmlParts     map[string][]byte
	contentTypes map[string]string
}

func newMockThemePackageSession() *mockThemePackageSession {
	return &mockThemePackageSession{
		xmlParts:     make(map[string][]byte),
		contentTypes: make(map[string]string),
	}
}

func (m *mockThemePackageSession) ListParts() []opc.PartInfo {
	return nil
}

func (m *mockThemePackageSession) ListRelationships(sourceURI string) []opc.RelationshipInfo {
	return nil
}

func (m *mockThemePackageSession) ReadRawPart(uri string) ([]byte, error) {
	return nil, fmt.Errorf("not implemented")
}

func (m *mockThemePackageSession) ReadXMLPart(uri string) (*etree.Document, error) {
	data, ok := m.xmlParts[uri]
	if !ok {
		return nil, fmt.Errorf("xml part not found: %s", uri)
	}
	doc := etree.NewDocument()
	if err := doc.ReadFromBytes(data); err != nil {
		return nil, err
	}
	return doc, nil
}

func (m *mockThemePackageSession) GetContentType(uri string) string {
	return m.contentTypes[uri]
}

func (m *mockThemePackageSession) GetZipMeta(uri string) *opc.ZipEntryMeta {
	return nil
}

func (m *mockThemePackageSession) ReplaceRawPart(uri string, data []byte, contentType string) error {
	return fmt.Errorf("not implemented")
}

func (m *mockThemePackageSession) ReplaceXMLPart(uri string, doc *etree.Document) error {
	data, err := doc.WriteToBytes()
	if err != nil {
		return err
	}
	m.xmlParts[uri] = data
	m.contentTypes[uri] = "application/vnd.openxmlformats-officedocument.theme+xml"
	return nil
}

func (m *mockThemePackageSession) AddPart(uri string, data []byte, contentType string, meta *opc.ZipEntryMeta) error {
	return fmt.Errorf("not implemented")
}

func (m *mockThemePackageSession) RemovePart(uri string) error {
	return fmt.Errorf("not implemented")
}

func (m *mockThemePackageSession) SaveAs(path string) error {
	return fmt.Errorf("not implemented")
}

func (m *mockThemePackageSession) Close() error {
	return nil
}

func (m *mockThemePackageSession) IsDirty() bool {
	return false
}

func (m *mockThemePackageSession) Warnings() []string {
	return nil
}

// Helper to create a mock theme XML for testing
func createTestThemeXML() *etree.Document {
	doc := etree.NewDocument()
	// Create root element with namespace using the proper format
	root := doc.CreateElement("a:theme")
	root.CreateAttr("xmlns:a", testDrawingMLNS)
	root.CreateAttr("name", "Office Theme")

	// Create children using the proper namespace format (prefix:local)
	// We need to set up a prefix for all child elements to ensure consistent serialization
	// The key is to create all children as elements with the "a:" prefix to match the namespace
	themeElements := root.CreateElement("a:themeElements")

	clrScheme := themeElements.CreateElement("a:clrScheme")
	clrScheme.CreateAttr("name", "Office")

	// Add test colors with RGB values
	colorNames := []string{"dk1", "lt1", "dk2", "lt2",
		"accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
		"hlink", "folHlink"}

	testHexValues := map[string]string{
		"dk1":      "000000",
		"lt1":      "FFFFFF",
		"dk2":      "1F497D",
		"lt2":      "EEECE1",
		"accent1":  "4F81BD",
		"accent2":  "C0504D",
		"accent3":  "9BBB59",
		"accent4":  "8064A2",
		"accent5":  "4BACC6",
		"accent6":  "F79646",
		"hlink":    "0000FF",
		"folHlink": "800080",
	}

	for _, name := range colorNames {
		colorElem := clrScheme.CreateElement("a:" + name)
		srgbClr := colorElem.CreateElement("a:srgbClr")
		srgbClr.CreateAttr("val", testHexValues[name])
	}

	// Create fontScheme
	fontScheme := themeElements.CreateElement("a:fontScheme")
	fontScheme.CreateAttr("name", "Office")

	// Create majorFont
	majorFont := fontScheme.CreateElement("a:majorFont")
	majorLatin := majorFont.CreateElement("a:latin")
	majorLatin.CreateAttr("typeface", "Calibri")

	// Create minorFont
	minorFont := fontScheme.CreateElement("a:minorFont")
	minorLatin := minorFont.CreateElement("a:latin")
	minorLatin.CreateAttr("typeface", "Calibri")

	return doc
}

func TestUpdateThemeColor(t *testing.T) {
	tests := []struct {
		name          string
		colorName     string
		hexValue      string
		expectedError bool
	}{
		{
			name:          "Update accent1 color",
			colorName:     "accent1",
			hexValue:      "FF0000",
			expectedError: false,
		},
		{
			name:          "Update dk1 color",
			colorName:     "dk1",
			hexValue:      "AABBCC",
			expectedError: false,
		},
		{
			name:          "Update hlink color",
			colorName:     "hlink",
			hexValue:      "00FF00",
			expectedError: false,
		},
		{
			name:          "Invalid color name",
			colorName:     "invalid",
			hexValue:      "FF0000",
			expectedError: true,
		},
		{
			name:          "Invalid hex value - too short",
			colorName:     "accent1",
			hexValue:      "FF00",
			expectedError: true,
		},
		{
			name:          "Invalid hex value - too long",
			colorName:     "accent1",
			hexValue:      "FF0000FF",
			expectedError: true,
		},
		{
			name:          "Invalid hex value - non-hex characters",
			colorName:     "accent1",
			hexValue:      "GGGGGG",
			expectedError: true,
		},
		{
			name:          "Empty color name",
			colorName:     "",
			hexValue:      "FF0000",
			expectedError: true,
		},
		{
			name:          "Empty hex value",
			colorName:     "accent1",
			hexValue:      "",
			expectedError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Create an in-memory package session
			themeDoc := createTestThemeXML()
			session := newMockThemePackageSession()

			// Add theme to session
			session.ReplaceXMLPart("/ppt/theme/theme1.xml", themeDoc)

			req := &UpdateThemeColorRequest{
				Package:   session,
				ThemeURI:  "/ppt/theme/theme1.xml",
				ColorName: tt.colorName,
				HexValue:  tt.hexValue,
			}

			err := UpdateThemeColor(req)

			if (err != nil) != tt.expectedError {
				t.Errorf("UpdateThemeColor() error = %v, expectedError = %v", err, tt.expectedError)
			}

			// If no error, verify the color was updated
			if err == nil && !tt.expectedError {
				updatedDoc, _ := session.ReadXMLPart("/ppt/theme/theme1.xml")
				root := updatedDoc.Root()
				themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
				clrScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "clrScheme")
				colorElem := xmlx.FindChild(clrScheme, testDrawingMLNS, tt.colorName)
				srgbClr := xmlx.FindChild(colorElem, testDrawingMLNS, "srgbClr")

				if srgbClr == nil {
					t.Errorf("srgbClr element not found after update")
				} else {
					actualHex := srgbClr.SelectAttrValue("val", "")
					if actualHex != tt.hexValue {
						t.Errorf("Expected hex value %s, got %s", tt.hexValue, actualHex)
					}
				}
			}
		})
	}
}

func TestUpdateThemeColorPreservesOtherColors(t *testing.T) {
	// Create a theme with multiple colors
	themeDoc := createTestThemeXML()
	session := newMockThemePackageSession()
	session.ReplaceXMLPart("/ppt/theme/theme1.xml", themeDoc)

	// Update one color
	req := &UpdateThemeColorRequest{
		Package:   session,
		ThemeURI:  "/ppt/theme/theme1.xml",
		ColorName: "accent1",
		HexValue:  "AABBCC",
	}

	err := UpdateThemeColor(req)
	if err != nil {
		t.Fatalf("UpdateThemeColor() error = %v", err)
	}

	// Verify other colors are unchanged
	updatedDoc, _ := session.ReadXMLPart("/ppt/theme/theme1.xml")
	root := updatedDoc.Root()
	themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
	clrScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "clrScheme")

	// Check that accent2 is still its original value
	accent2 := xmlx.FindChild(clrScheme, testDrawingMLNS, "accent2")
	srgbClr := xmlx.FindChild(accent2, testDrawingMLNS, "srgbClr")
	actualHex := srgbClr.SelectAttrValue("val", "")

	if actualHex != "C0504D" {
		t.Errorf("accent2 color was unexpectedly modified from C0504D to %s", actualHex)
	}
}

func TestUpdateThemeFont(t *testing.T) {
	tests := []struct {
		name          string
		majorFont     string
		minorFont     string
		expectedError bool
	}{
		{
			name:          "Update major font only",
			majorFont:     "Arial",
			minorFont:     "",
			expectedError: false,
		},
		{
			name:          "Update minor font only",
			majorFont:     "",
			minorFont:     "Times New Roman",
			expectedError: false,
		},
		{
			name:          "Update both fonts",
			majorFont:     "Arial",
			minorFont:     "Times New Roman",
			expectedError: false,
		},
		{
			name:          "No font provided",
			majorFont:     "",
			minorFont:     "",
			expectedError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			themeDoc := createTestThemeXML()
			session := newMockThemePackageSession()
			session.ReplaceXMLPart("/ppt/theme/theme1.xml", themeDoc)

			req := &UpdateThemeFontRequest{
				Package:   session,
				ThemeURI:  "/ppt/theme/theme1.xml",
				MajorFont: tt.majorFont,
				MinorFont: tt.minorFont,
			}

			err := UpdateThemeFont(req)

			if (err != nil) != tt.expectedError {
				t.Errorf("UpdateThemeFont() error = %v, expectedError = %v", err, tt.expectedError)
			}

			// If no error, verify the fonts were updated
			if err == nil && !tt.expectedError {
				updatedDoc, _ := session.ReadXMLPart("/ppt/theme/theme1.xml")
				root := updatedDoc.Root()
				themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
				fontScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "fontScheme")

				if tt.majorFont != "" {
					majorFont := xmlx.FindChild(fontScheme, testDrawingMLNS, "majorFont")
					majorLatin := xmlx.FindChild(majorFont, testDrawingMLNS, "latin")
					actualFont := majorLatin.SelectAttrValue("typeface", "")
					if actualFont != tt.majorFont {
						t.Errorf("Expected major font %s, got %s", tt.majorFont, actualFont)
					}
				}

				if tt.minorFont != "" {
					minorFont := xmlx.FindChild(fontScheme, testDrawingMLNS, "minorFont")
					minorLatin := xmlx.FindChild(minorFont, testDrawingMLNS, "latin")
					actualFont := minorLatin.SelectAttrValue("typeface", "")
					if actualFont != tt.minorFont {
						t.Errorf("Expected minor font %s, got %s", tt.minorFont, actualFont)
					}
				}
			}
		})
	}
}

func TestUpdateThemeFontPreservesOtherContent(t *testing.T) {
	// Create a theme with additional font elements
	themeDoc := createTestThemeXML()
	session := newMockThemePackageSession()
	session.ReplaceXMLPart("/ppt/theme/theme1.xml", themeDoc)

	// Add an EA (East Asian) font to the major font
	root := themeDoc.Root()
	themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
	fontScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "fontScheme")
	majorFont := xmlx.FindChild(fontScheme, testDrawingMLNS, "majorFont")
	// Use "a:" prefix to match the document's namespace convention
	eaFont := majorFont.CreateElement("a:ea")
	eaFont.CreateAttr("typeface", "SimSun")

	session.ReplaceXMLPart("/ppt/theme/theme1.xml", themeDoc)

	// Update the major font
	req := &UpdateThemeFontRequest{
		Package:   session,
		ThemeURI:  "/ppt/theme/theme1.xml",
		MajorFont: "Helvetica",
	}

	err := UpdateThemeFont(req)
	if err != nil {
		t.Fatalf("UpdateThemeFont() error = %v", err)
	}

	// Verify EA font is still present
	updatedDoc, _ := session.ReadXMLPart("/ppt/theme/theme1.xml")
	root = updatedDoc.Root()
	themeElements = xmlx.FindChild(root, testDrawingMLNS, "themeElements")
	fontScheme = xmlx.FindChild(themeElements, testDrawingMLNS, "fontScheme")
	majorFont = xmlx.FindChild(fontScheme, testDrawingMLNS, "majorFont")
	eaFont = xmlx.FindChild(majorFont, testDrawingMLNS, "ea")

	if eaFont == nil {
		t.Error("EA font element was unexpectedly removed")
	} else {
		eaTypeface := eaFont.SelectAttrValue("typeface", "")
		if eaTypeface != "SimSun" {
			t.Errorf("EA font was modified from SimSun to %s", eaTypeface)
		}
	}
}

func TestUpdateThemeColorMultiple(t *testing.T) {
	themeDoc := createTestThemeXML()
	session := newMockThemePackageSession()
	session.ReplaceXMLPart("/ppt/theme/theme1.xml", themeDoc)

	// Update first color
	req1 := &UpdateThemeColorRequest{
		Package:   session,
		ThemeURI:  "/ppt/theme/theme1.xml",
		ColorName: "accent1",
		HexValue:  "FF0000",
	}
	if err := UpdateThemeColor(req1); err != nil {
		t.Fatalf("First UpdateThemeColor() error = %v", err)
	}

	// Update second color
	req2 := &UpdateThemeColorRequest{
		Package:   session,
		ThemeURI:  "/ppt/theme/theme1.xml",
		ColorName: "accent2",
		HexValue:  "00FF00",
	}
	if err := UpdateThemeColor(req2); err != nil {
		t.Fatalf("Second UpdateThemeColor() error = %v", err)
	}

	// Verify both colors
	updatedDoc, _ := session.ReadXMLPart("/ppt/theme/theme1.xml")
	root := updatedDoc.Root()
	themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
	clrScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "clrScheme")

	accent1 := xmlx.FindChild(clrScheme, testDrawingMLNS, "accent1")
	srgb1 := xmlx.FindChild(accent1, testDrawingMLNS, "srgbClr")
	hex1 := srgb1.SelectAttrValue("val", "")

	if hex1 != "FF0000" {
		t.Errorf("accent1: expected FF0000, got %s", hex1)
	}

	accent2 := xmlx.FindChild(clrScheme, testDrawingMLNS, "accent2")
	srgb2 := xmlx.FindChild(accent2, testDrawingMLNS, "srgbClr")
	hex2 := srgb2.SelectAttrValue("val", "")

	if hex2 != "00FF00" {
		t.Errorf("accent2: expected 00FF00, got %s", hex2)
	}
}

func TestIsValidHexColor(t *testing.T) {
	tests := []struct {
		hex   string
		valid bool
	}{
		{"FF0000", true},
		{"ffffff", true},
		{"AABBCC", true},
		{"123456", true},
		{"ff0000", true},
		{"", false},
		{"FF00", false},     // too short
		{"FF00FF00", false}, // too long
		{"GGGGGG", false},   // invalid characters
		{"FF00G0", false},   // invalid character
		{"FF 000", false},   // space
	}

	for _, tt := range tests {
		result := isValidHexColor(tt.hex)
		if result != tt.valid {
			t.Errorf("isValidHexColor(%q) = %v, want %v", tt.hex, result, tt.valid)
		}
	}
}

func TestIsValidColorName(t *testing.T) {
	validNames := []string{"dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3",
		"accent4", "accent5", "accent6", "hlink", "folHlink"}
	invalidNames := []string{"invalid", "accent7", "accent", "color", ""}

	for _, name := range validNames {
		if !isValidColorName(name) {
			t.Errorf("isValidColorName(%q) = false, want true", name)
		}
	}

	for _, name := range invalidNames {
		if isValidColorName(name) {
			t.Errorf("isValidColorName(%q) = true, want false", name)
		}
	}
}

// TestThemeColorMutationWithRealPresentation tests color updates with a real presentation if available
func TestThemeColorMutationWithRealPresentation(t *testing.T) {
	// Load a real presentation from testdata
	session, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Skipf("Could not load test presentation: %v", err)
	}
	defer session.Close()

	// Parse to find theme URI
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Skipf("Could not parse presentation: %v", err)
	}

	// Presentation should have a theme (get it from first master)
	if len(graph.Masters) == 0 {
		t.Skipf("Test presentation has no masters")
	}
	themeURI := graph.Masters[0].ThemeURI
	if themeURI == "" {
		t.Skipf("Test presentation has no theme")
	}

	// Update a color
	req := &UpdateThemeColorRequest{
		Package:   session,
		ThemeURI:  themeURI,
		ColorName: "accent1",
		HexValue:  "FF5500",
	}

	err = UpdateThemeColor(req)
	if err != nil {
		t.Fatalf("UpdateThemeColor() with real presentation failed: %v", err)
	}

	// Verify the color was updated
	doc, err := session.ReadXMLPart(themeURI)
	if err != nil {
		t.Fatalf("Failed to read updated theme: %v", err)
	}

	root := doc.Root()
	themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
	clrScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "clrScheme")
	accent1 := xmlx.FindChild(clrScheme, testDrawingMLNS, "accent1")
	srgbClr := xmlx.FindChild(accent1, testDrawingMLNS, "srgbClr")

	if srgbClr == nil {
		t.Error("srgbClr not found in updated theme")
	} else {
		hex := srgbClr.SelectAttrValue("val", "")
		if hex != "FF5500" {
			t.Errorf("Expected accent1 color FF5500, got %s", hex)
		}
	}
}

// TestThemeFontMutationWithRealPresentation tests font updates with a real presentation if available
func TestThemeFontMutationWithRealPresentation(t *testing.T) {
	// Load a real presentation from testdata
	session, err := opc.Open("../../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Skipf("Could not load test presentation: %v", err)
	}
	defer session.Close()

	// Parse to find theme URI
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Skipf("Could not parse presentation: %v", err)
	}

	// Presentation should have a theme (get it from first master)
	if len(graph.Masters) == 0 {
		t.Skipf("Test presentation has no masters")
	}
	themeURI := graph.Masters[0].ThemeURI
	if themeURI == "" {
		t.Skipf("Test presentation has no theme")
	}

	// Update fonts
	req := &UpdateThemeFontRequest{
		Package:   session,
		ThemeURI:  themeURI,
		MajorFont: "Georgia",
		MinorFont: "Verdana",
	}

	err = UpdateThemeFont(req)
	if err != nil {
		t.Fatalf("UpdateThemeFont() with real presentation failed: %v", err)
	}

	// Verify the fonts were updated
	doc, err := session.ReadXMLPart(themeURI)
	if err != nil {
		t.Fatalf("Failed to read updated theme: %v", err)
	}

	root := doc.Root()
	themeElements := xmlx.FindChild(root, testDrawingMLNS, "themeElements")
	fontScheme := xmlx.FindChild(themeElements, testDrawingMLNS, "fontScheme")

	majorFont := xmlx.FindChild(fontScheme, testDrawingMLNS, "majorFont")
	majorLatin := xmlx.FindChild(majorFont, testDrawingMLNS, "latin")
	if majorLatin == nil {
		t.Error("latin not found in updated majorFont")
	} else {
		typeface := majorLatin.SelectAttrValue("typeface", "")
		if typeface != "Georgia" {
			t.Errorf("Expected major font Georgia, got %s", typeface)
		}
	}

	minorFont := xmlx.FindChild(fontScheme, testDrawingMLNS, "minorFont")
	minorLatin := xmlx.FindChild(minorFont, testDrawingMLNS, "latin")
	if minorLatin == nil {
		t.Error("latin not found in updated minorFont")
	} else {
		typeface := minorLatin.SelectAttrValue("typeface", "")
		if typeface != "Verdana" {
			t.Errorf("Expected minor font Verdana, got %s", typeface)
		}
	}
}
