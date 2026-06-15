package normalize

import (
	"testing"

	"github.com/beevik/etree"
)

// TestParseSimplePlaceholder tests parsing a basic placeholder with type and idx.
func TestParseSimplePlaceholder(t *testing.T) {
	// Create a minimal shape with placeholder
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="2" name="Title 1"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph type="title" idx="0"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph == nil {
		t.Fatal("ParsePlaceholder returned nil")
	}

	if ph.Type != "title" {
		t.Errorf("expected type=title, got %q", ph.Type)
	}

	if ph.Idx != 0 {
		t.Errorf("expected idx=0, got %d", ph.Idx)
	}
}

// TestParseBodyPlaceholder tests parsing a body placeholder.
func TestParseBodyPlaceholder(t *testing.T) {
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="3" name="Content Placeholder 2"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph type="body" idx="1"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph == nil {
		t.Fatal("ParsePlaceholder returned nil")
	}

	if ph.Type != "body" {
		t.Errorf("expected type=body, got %q", ph.Type)
	}

	if ph.Idx != 1 {
		t.Errorf("expected idx=1, got %d", ph.Idx)
	}
}

// TestParsePlaceholderWithoutType tests parsing a placeholder with idx but no type.
func TestParsePlaceholderWithoutType(t *testing.T) {
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="3" name="Content Placeholder 2"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph idx="1"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph == nil {
		t.Fatal("ParsePlaceholder returned nil")
	}

	if ph.Type != "" {
		t.Errorf("expected empty type, got %q", ph.Type)
	}

	if ph.Idx != 1 {
		t.Errorf("expected idx=1, got %d", ph.Idx)
	}
}

// TestParseNonPlaceholder tests that non-placeholder shapes return nil.
func TestParseNonPlaceholder(t *testing.T) {
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="2" name="Regular Shape"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<!-- No p:ph element -->
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph != nil {
		t.Errorf("expected nil for non-placeholder, got %+v", ph)
	}
}

// TestParsePlaceholderWithSizeAndOrient tests parsing all p:ph attributes.
func TestParsePlaceholderWithSizeAndOrient(t *testing.T) {
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="2" name="Title"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph type="title" idx="0" sz="full" orient="horz"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph == nil {
		t.Fatal("ParsePlaceholder returned nil")
	}

	if ph.Sz != "full" {
		t.Errorf("expected sz=full, got %q", ph.Sz)
	}

	if ph.Orient != "horz" {
		t.Errorf("expected orient=horz, got %q", ph.Orient)
	}
}

// TestParseNilShape tests that ParsePlaceholder handles nil input gracefully.
func TestParseNilShape(t *testing.T) {
	ph := ParsePlaceholder(nil)
	if ph != nil {
		t.Errorf("expected nil for nil input, got %+v", ph)
	}
}

// TestExtractPlaceholdersFromShapes tests filtering placeholders from a shape list.
func TestExtractPlaceholdersFromShapes(t *testing.T) {
	// Create a shape with placeholder
	phXML := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="2" name="Title"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph type="title" idx="0"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	// Create a non-placeholder shape
	nonPhXML := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="3" name="Regular"/>
			<p:cNvSpPr/>
			<p:nvPr/>
		</p:nvSpPr>
	</p:sp>`

	phDoc := etree.NewDocument()
	if err := phDoc.ReadFromString(phXML); err != nil {
		t.Fatalf("failed to parse placeholder XML: %v", err)
	}

	nonPhDoc := etree.NewDocument()
	if err := nonPhDoc.ReadFromString(nonPhXML); err != nil {
		t.Fatalf("failed to parse non-placeholder XML: %v", err)
	}

	shapes := []*etree.Element{
		phDoc.Root(),
		nonPhDoc.Root(),
	}

	result := ExtractPlaceholdersFromShapes(shapes)

	if len(result) != 1 {
		t.Errorf("expected 1 placeholder, got %d", len(result))
	}

	if ParsePlaceholder(result[0]).Type != "title" {
		t.Errorf("expected title placeholder, got something else")
	}
}

// TestParsePlaceholderLargeIdx tests parsing placeholders with large index values.
func TestParsePlaceholderLargeIdx(t *testing.T) {
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="10" name="Content"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph type="body" idx="999"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph == nil {
		t.Fatal("ParsePlaceholder returned nil")
	}

	if ph.Idx != 999 {
		t.Errorf("expected idx=999, got %d", ph.Idx)
	}
}

// TestParsePlaceholderInvalidIdx tests parsing placeholder with non-numeric idx.
func TestParsePlaceholderInvalidIdx(t *testing.T) {
	xml := `<p:sp xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
		<p:nvSpPr>
			<p:cNvPr id="2" name="Title"/>
			<p:cNvSpPr/>
			<p:nvPr>
				<p:ph type="title" idx="invalid"/>
			</p:nvPr>
		</p:nvSpPr>
	</p:sp>`

	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("failed to parse XML: %v", err)
	}

	shape := doc.Root()
	ph := ParsePlaceholder(shape)

	if ph == nil {
		t.Fatal("ParsePlaceholder returned nil")
	}

	// idx should be -1 when it can't be parsed
	if ph.Idx != -1 {
		t.Errorf("expected idx=-1 for invalid idx, got %d", ph.Idx)
	}
}
