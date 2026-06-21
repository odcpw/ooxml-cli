package xmlx

import (
	"testing"

	"github.com/beevik/etree"
)

func TestFindChild(t *testing.T) {
	doc := etree.NewDocument()
	root := doc.CreateElement("root")
	root.CreateElement("child1")
	root.CreateElement("child2")
	root.CreateElement("child3")

	// Test finding existing child
	child := FindChild(root, "", "child2")
	if child == nil {
		t.Errorf("FindChild failed to find child2")
	}
	if child.Tag != "child2" {
		t.Errorf("Expected tag child2, got %s", child.Tag)
	}

	// Test finding non-existent child
	notFound := FindChild(root, "", "nonexistent")
	if notFound != nil {
		t.Errorf("FindChild should return nil for non-existent child")
	}

	// Test with nil element
	result := FindChild(nil, "", "child1")
	if result != nil {
		t.Errorf("FindChild should return nil for nil element")
	}
}

func TestFindChildren(t *testing.T) {
	doc := etree.NewDocument()
	root := doc.CreateElement("root")
	root.CreateElement("child")
	root.CreateElement("child")
	root.CreateElement("child")
	root.CreateElement("other")

	children := FindChildren(root, "", "child")
	if len(children) != 3 {
		t.Errorf("Expected 3 children, got %d", len(children))
	}

	for _, child := range children {
		if child.Tag != "child" {
			t.Errorf("Expected tag child, got %s", child.Tag)
		}
	}
}

func TestFindDescendants(t *testing.T) {
	doc := etree.NewDocument()
	root := doc.CreateElement("root")
	level1 := root.CreateElement("level1")
	level1.CreateElement("target")
	level2 := level1.CreateElement("level2")
	level2.CreateElement("target")
	level2.CreateElement("target")

	descendants := FindDescendants(root, "", "target")
	if len(descendants) != 3 {
		t.Errorf("Expected 3 descendants, got %d", len(descendants))
	}
}

func TestElementMatches(t *testing.T) {
	doc := etree.NewDocument()
	elem := doc.CreateElement("test")

	// Test matching element
	if !ElementMatches(elem, "", "test") {
		t.Errorf("ElementMatches failed for matching element")
	}

	// Test non-matching element
	if ElementMatches(elem, "", "other") {
		t.Errorf("ElementMatches should return false for non-matching element")
	}

	// Test with nil element
	if ElementMatches(nil, "", "test") {
		t.Errorf("ElementMatches should return false for nil element")
	}
}

func TestGetAttr(t *testing.T) {
	doc := etree.NewDocument()
	elem := doc.CreateElement("test")
	elem.CreateAttr("name", "value")

	// Test getting existing attribute
	val, found := GetAttr(elem, "name")
	if !found {
		t.Errorf("GetAttr failed to find attribute")
	}
	if val != "value" {
		t.Errorf("Expected value 'value', got %q", val)
	}

	// Test getting non-existent attribute
	val, found = GetAttr(elem, "nonexistent")
	if found {
		t.Errorf("GetAttr should return false for non-existent attribute")
	}
	if val != "" {
		t.Errorf("Expected empty string for non-existent attribute, got %q", val)
	}

	// Test with nil element
	val, found = GetAttr(nil, "name")
	if found {
		t.Errorf("GetAttr should return false for nil element")
	}
}

func TestSetAttr(t *testing.T) {
	doc := etree.NewDocument()
	elem := doc.CreateElement("test")

	// Set attribute
	SetAttr(elem, "name", "value")

	attr := elem.SelectAttr("name")
	if attr == nil {
		t.Errorf("SetAttr failed to set attribute")
	}
	if attr.Value != "value" {
		t.Errorf("Expected attribute value 'value', got %q", attr.Value)
	}

	// Test setting with nil element (should not panic)
	SetAttr(nil, "name", "value")
}

func TestGetText(t *testing.T) {
	doc := etree.NewDocument()
	elem := doc.CreateElement("test")
	elem.SetText("Hello, World!")

	text := GetText(elem)
	if text != "Hello, World!" {
		t.Errorf("Expected 'Hello, World!', got %q", text)
	}

	// Test with nil element
	text = GetText(nil)
	if text != "" {
		t.Errorf("Expected empty string for nil element, got %q", text)
	}
}

func TestSetText(t *testing.T) {
	doc := etree.NewDocument()
	elem := doc.CreateElement("test")

	SetText(elem, "New text")

	text := GetText(elem)
	if text != "New text" {
		t.Errorf("Expected 'New text', got %q", text)
	}

	// Test setting with nil element (should not panic)
	SetText(nil, "text")
}

func TestAppendChild(t *testing.T) {
	doc := etree.NewDocument()
	parent := doc.CreateElement("parent")
	child := etree.NewElement("child")

	result := AppendChild(parent, child)
	if result != child {
		t.Errorf("AppendChild should return the child element")
	}

	if len(parent.Child) != 1 {
		t.Errorf("Expected 1 child, got %d", len(parent.Child))
	}
}

func TestCreateChild(t *testing.T) {
	doc := etree.NewDocument()
	parent := doc.CreateElement("parent")

	child := CreateChild(parent, "child")
	if child == nil {
		t.Errorf("CreateChild failed to create element")
	}
	if child.Tag != "child" {
		t.Errorf("Expected tag 'child', got %s", child.Tag)
	}

	if len(parent.Child) != 1 {
		t.Errorf("Expected 1 child, got %d", len(parent.Child))
	}

	// Test with nil parent
	result := CreateChild(nil, "child")
	if result != nil {
		t.Errorf("CreateChild should return nil for nil parent")
	}
}

func TestRemoveChild(t *testing.T) {
	doc := etree.NewDocument()
	parent := doc.CreateElement("parent")
	child := parent.CreateElement("child")

	if len(parent.Child) != 1 {
		t.Errorf("Expected 1 child before removal, got %d", len(parent.Child))
	}

	RemoveChild(parent, child)

	if len(parent.Child) != 0 {
		t.Errorf("Expected 0 children after removal, got %d", len(parent.Child))
	}
}

func TestGetParent(t *testing.T) {
	doc := etree.NewDocument()
	parent := doc.CreateElement("parent")
	child := parent.CreateElement("child")

	p := GetParent(child)
	if p != parent {
		t.Errorf("GetParent failed to return parent")
	}

	// Test with nil element
	result := GetParent(nil)
	if result != nil {
		t.Errorf("GetParent should return nil for nil element")
	}
}

func TestPath(t *testing.T) {
	doc := etree.NewDocument()
	root := doc.CreateElement("root")
	level1 := root.CreateElement("level1")
	level2 := level1.CreateElement("level2")

	path := Path(level2)
	if len(path) < 2 {
		t.Errorf("Expected path with at least 2 elements, got %d", len(path))
	}

	// Verify the path ends with the element names
	if path[len(path)-2] != "level1" || path[len(path)-1] != "level2" {
		t.Errorf("Path doesn't match expected sequence")
	}

	// Test with nil element
	result := Path(nil)
	if result != nil {
		t.Errorf("Path should return nil for nil element")
	}
}

func TestAttrHelper(t *testing.T) {
	attr := Attr("name", "value")
	if attr.Key != "name" {
		t.Errorf("Expected key 'name', got %q", attr.Key)
	}
	if attr.Value != "value" {
		t.Errorf("Expected value 'value', got %q", attr.Value)
	}
}

func TestAttrNSHelper(t *testing.T) {
	attr := AttrNS("http://example.com", "local", "value")
	if attr.Space != "http://example.com" {
		t.Errorf("Expected space 'http://example.com', got %q", attr.Space)
	}
	if attr.Key != "local" {
		t.Errorf("Expected key 'local', got %q", attr.Key)
	}
	if attr.Value != "value" {
		t.Errorf("Expected value 'value', got %q", attr.Value)
	}
}
