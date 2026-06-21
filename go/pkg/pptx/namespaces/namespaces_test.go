package namespaces

import (
	"testing"

	"github.com/beevik/etree"
)

func TestNamespaceConstants(t *testing.T) {
	tests := []struct {
		name     string
		constant string
		expected string
	}{
		{"NsP", NsP, "http://schemas.openxmlformats.org/presentationml/2006/main"},
		{"NsA", NsA, "http://schemas.openxmlformats.org/drawingml/2006/main"},
		{"NsR", NsR, "http://schemas.openxmlformats.org/officeDocument/2006/relationships"},
		{"NsMC", NsMC, "http://schemas.openxmlformats.org/markup-compatibility/2006"},
		{"NsC", NsC, "http://schemas.openxmlformats.org/drawingml/2006/chart"},
		{"NsDgm", NsDgm, "http://schemas.openxmlformats.org/drawingml/2006/diagram"},
	}

	for _, tt := range tests {
		if tt.constant != tt.expected {
			t.Errorf("%s constant mismatch: got %q, expected %q", tt.name, tt.constant, tt.expected)
		}
	}
}

func TestFindChild(t *testing.T) {
	// Use XML parsing to properly create elements with namespaces
	xml := `<?xml version="1.0"?>
<p:spTree xmlns:p="` + NsP + `">
  <p:nvGrpSpPr/>
  <p:grpSpPr/>
  <p:sp id="2"/>
  <p:sp id="3"/>
</p:spTree>`

	doc := etree.NewDocument()
	err := doc.ReadFromString(xml)
	if err != nil {
		t.Fatalf("Failed to parse XML: %v", err)
	}

	spTree := doc.Root()

	// Test finding existing shape child
	found := FindChild(spTree, NsP, "sp")
	if found == nil {
		t.Errorf("FindChild failed to find p:sp child")
	}
	if found.SelectAttr("id").Value != "2" {
		t.Errorf("FindChild returned wrong shape")
	}

	// Test finding non-existent child
	notFound := FindChild(spTree, NsP, "grpSp")
	if notFound != nil {
		t.Errorf("FindChild should return nil for non-existent child")
	}

	// Test with wrong namespace
	notFound = FindChild(spTree, NsA, "sp")
	if notFound != nil {
		t.Errorf("FindChild should return nil for wrong namespace")
	}

	// Test with nil element
	result := FindChild(nil, NsP, "sp")
	if result != nil {
		t.Errorf("FindChild should return nil for nil element")
	}
}

func TestFindChildren(t *testing.T) {
	xml := `<?xml version="1.0"?>
<p:spTree xmlns:p="` + NsP + `">
  <p:sp/>
  <p:sp/>
  <p:sp/>
  <p:grpSp/>
</p:spTree>`

	doc := etree.NewDocument()
	err := doc.ReadFromString(xml)
	if err != nil {
		t.Fatalf("Failed to parse XML: %v", err)
	}

	spTree := doc.Root()

	// Test finding all shape children
	shapes := FindChildren(spTree, NsP, "sp")
	if len(shapes) != 3 {
		t.Errorf("Expected 3 shapes, got %d", len(shapes))
	}

	for _, shape := range shapes {
		if shape.Tag != "sp" {
			t.Errorf("Expected sp tag, got %s", shape.Tag)
		}
	}

	// Test finding different element type
	grpShapes := FindChildren(spTree, NsP, "grpSp")
	if len(grpShapes) != 1 {
		t.Errorf("Expected 1 group shape, got %d", len(grpShapes))
	}

	// Test with empty result
	notFound := FindChildren(spTree, NsP, "nonexistent")
	if notFound != nil {
		t.Errorf("FindChildren should return nil for no matches, got %v", notFound)
	}
}

func TestFindDescendants(t *testing.T) {
	xml := `<?xml version="1.0"?>
<p:sld xmlns:p="` + NsP + `" xmlns:a="` + NsA + `">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:spPr>
          <a:xfrm>
            <a:ext/>
          </a:xfrm>
        </p:spPr>
      </p:sp>
      <p:sp>
        <p:spPr>
          <a:xfrm>
            <a:ext/>
          </a:xfrm>
        </p:spPr>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>`

	doc := etree.NewDocument()
	err := doc.ReadFromString(xml)
	if err != nil {
		t.Fatalf("Failed to parse XML: %v", err)
	}

	slide := doc.Root()

	// Find all ext elements (descendants in DrawingML namespace)
	exts := FindDescendants(slide, NsA, "ext")
	if len(exts) != 2 {
		t.Errorf("Expected 2 ext descendants, got %d", len(exts))
	}

	// Find all sp elements
	shapes := FindDescendants(slide, NsP, "sp")
	if len(shapes) != 2 {
		t.Errorf("Expected 2 sp descendants, got %d", len(shapes))
	}

	// Find non-existent descendants
	notFound := FindDescendants(slide, NsP, "nonexistent")
	if notFound != nil {
		t.Errorf("FindDescendants should return nil for no matches")
	}
}

// TestAttrGetAttribute tests getting attribute values with namespace
func TestAttr(t *testing.T) {
	doc := etree.NewDocument()

	// Create a blip element with r:embed attribute
	blip := doc.CreateElement("{" + NsA + "}blip")
	blip.Attr = append(blip.Attr, etree.Attr{
		Space: NsR,
		Key:   "embed",
		Value: "rId1",
	})

	// Test getting r:embed attribute
	val, found := Attr(blip, NsR, "embed")
	if !found {
		t.Errorf("Attr failed to find r:embed attribute")
	}
	if val != "rId1" {
		t.Errorf("Expected rId1, got %q", val)
	}

	// Test getting non-existent attribute
	val, found = Attr(blip, NsR, "link")
	if found {
		t.Errorf("Attr should return false for non-existent attribute")
	}
	if val != "" {
		t.Errorf("Expected empty string for non-existent attribute, got %q", val)
	}

	// Test with wrong namespace
	val, found = Attr(blip, NsA, "embed")
	if found {
		t.Errorf("Attr should return false for wrong namespace")
	}

	// Test with nil element
	val, found = Attr(nil, NsR, "embed")
	if found {
		t.Errorf("Attr should return false for nil element")
	}
}

// TestHasChild tests checking for child element existence
func TestHasChild(t *testing.T) {
	xml := `<?xml version="1.0"?>
<p:nvPr xmlns:p="` + NsP + `">
  <p:ph/>
</p:nvPr>`

	doc := etree.NewDocument()
	err := doc.ReadFromString(xml)
	if err != nil {
		t.Fatalf("Failed to parse XML: %v", err)
	}

	nvPr := doc.Root()

	// Test that HasChild finds the placeholder
	if !HasChild(nvPr, NsP, "ph") {
		t.Errorf("HasChild failed to find p:ph child")
	}

	// Test that HasChild returns false for non-existent child
	if HasChild(nvPr, NsP, "noexist") {
		t.Errorf("HasChild should return false for non-existent child")
	}

	// Test with wrong namespace
	if HasChild(nvPr, NsA, "ph") {
		t.Errorf("HasChild should return false for wrong namespace")
	}

	// Test with nil element
	if HasChild(nil, NsP, "ph") {
		t.Errorf("HasChild should return false for nil element")
	}

	// Test with element that has no children
	xml2 := `<?xml version="1.0"?>
<p:empty xmlns:p="` + NsP + `"/>`

	doc2 := etree.NewDocument()
	err = doc2.ReadFromString(xml2)
	if err != nil {
		t.Fatalf("Failed to parse XML: %v", err)
	}

	empty := doc2.Root()
	if HasChild(empty, NsP, "ph") {
		t.Errorf("HasChild should return false for element with no children")
	}
}

// TestIntegration tests a more realistic PPTX shape structure
func TestIntegration(t *testing.T) {
	xml := `<?xml version="1.0"?>
<p:sld xmlns:p="` + NsP + `" xmlns:a="` + NsA + `">
  <p:cSld>
    <p:spTree>
      <p:sp>
        <p:nvSpPr>
          <p:cNvPr/>
          <p:cNvSpPr/>
          <p:nvPr>
            <p:ph/>
          </p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm/>
          <a:prstGeom>
            <a:avLst/>
          </a:prstGeom>
        </p:spPr>
      </p:sp>
    </p:spTree>
  </p:cSld>
</p:sld>`

	doc := etree.NewDocument()
	err := doc.ReadFromString(xml)
	if err != nil {
		t.Fatalf("Failed to parse XML: %v", err)
	}

	sld := doc.Root()
	cSld := FindChild(sld, NsP, "cSld")
	spTree := FindChild(cSld, NsP, "spTree")
	sp := FindChild(spTree, NsP, "sp")
	nvSpPr := FindChild(sp, NsP, "nvSpPr")
	nvPr := FindChild(nvSpPr, NsP, "nvPr")
	spPr := FindChild(sp, NsP, "spPr")

	// Test: FindChild(spTree, NsP, "sp") should find the shape
	found := FindChild(spTree, NsP, "sp")
	if found == nil {
		t.Errorf("Integration test: FindChild failed to find shape in spTree")
	}

	// Test: HasChild(nvPr, NsP, "ph") should return true
	if !HasChild(nvPr, NsP, "ph") {
		t.Errorf("Integration test: HasChild failed to find placeholder in nvPr")
	}

	// Test: FindChild(spPr, NsA, "xfrm") should find transform
	xfrm := FindChild(spPr, NsA, "xfrm")
	if xfrm == nil {
		t.Errorf("Integration test: FindChild failed to find xfrm in spPr")
	}

	// Test: FindDescendants should find all descendants
	descendants := FindDescendants(sp, NsA, "xfrm")
	if len(descendants) != 1 {
		t.Errorf("Integration test: Expected 1 xfrm descendant, got %d", len(descendants))
	}

	// Test: FindChildren should find direct children only (not descendant)
	children := FindChildren(spPr, NsA, "avLst")
	if len(children) != 0 {
		t.Errorf("Integration test: Expected 0 direct children (avLst is nested), got %d", len(children))
	}

	// But it should be found as a descendant
	descendants = FindDescendants(spPr, NsA, "avLst")
	if len(descendants) != 1 {
		t.Errorf("Integration test: Expected 1 avLst descendant, got %d", len(descendants))
	}
}

// TestAttrWithRelationshipID tests a realistic relationship attribute scenario
func TestAttrWithRelationshipID(t *testing.T) {
	doc := etree.NewDocument()

	// Simulate an image element with relationship:
	// <a:blip r:embed="rId4"/>
	blip := doc.CreateElement("{" + NsA + "}blip")
	blip.Attr = append(blip.Attr, etree.Attr{
		Space: NsR,
		Key:   "embed",
		Value: "rId4",
	})

	// Test getting the relationship ID
	rId, found := Attr(blip, NsR, "embed")
	if !found {
		t.Errorf("Failed to get relationship ID from blip")
	}
	if rId != "rId4" {
		t.Errorf("Expected rId4, got %q", rId)
	}
}
