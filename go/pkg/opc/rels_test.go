package opc

import (
	"testing"
)

func TestParseRelationships(t *testing.T) {
	xmlData := []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="http://example.com" TargetMode="External"/>
</Relationships>`)

	rels, err := ParseRelationships("/", xmlData)
	if err != nil {
		t.Fatalf("ParseRelationships failed: %v", err)
	}

	if len(rels) != 3 {
		t.Errorf("expected 3 relationships, got %d", len(rels))
	}

	// Check first relationship
	if rels[0].ID != "rId1" {
		t.Errorf("expected rId1, got %s", rels[0].ID)
	}
	if rels[0].Target != "slides/slide1.xml" {
		t.Errorf("expected slides/slide1.xml, got %s", rels[0].Target)
	}

	// Check external relationship
	if rels[2].TargetMode != "External" {
		t.Errorf("expected External, got %s", rels[2].TargetMode)
	}
}

func TestResolveRelationshipTarget(t *testing.T) {
	tests := []struct {
		sourceURI string
		target    string
		expected  string
	}{
		{"/ppt/slides/slide1.xml", "../slideLayouts/slideLayout1.xml", "/ppt/slideLayouts/slideLayout1.xml"},
		{"/ppt/slides/slide1.xml", "../../slideMasters/slideMaster1.xml", "/slideMasters/slideMaster1.xml"},
		{"/ppt/slides/slide1.xml", "../media/image1.jpg", "/ppt/media/image1.jpg"},
		{"/ppt/presentation.xml", "./slides/slide1.xml", "/ppt/slides/slide1.xml"},
		{"/", "docProps/core.xml", "/docProps/core.xml"},
		{"/ppt/slides/slide1.xml", "http://example.com", "http://example.com"},
	}

	for _, tt := range tests {
		result := ResolveRelationshipTarget(tt.sourceURI, tt.target)
		if result != tt.expected {
			t.Errorf("ResolveRelationshipTarget(%q, %q) = %q, want %q", tt.sourceURI, tt.target, result, tt.expected)
		}
	}
}

func TestRelsURIForPart(t *testing.T) {
	tests := []struct {
		source   string
		expected string
	}{
		{"/", "/_rels/.rels"},
		{"/ppt/presentation.xml", "/ppt/_rels/presentation.xml.rels"},
		{"/xl/workbook.xml", "/xl/_rels/workbook.xml.rels"},
		{"/custom.xml", "/_rels/custom.xml.rels"},
	}

	for _, tt := range tests {
		if got := RelsURIForPart(tt.source); got != tt.expected {
			t.Errorf("RelsURIForPart(%q) = %q, want %q", tt.source, got, tt.expected)
		}
	}
}

func TestRelationshipTarget(t *testing.T) {
	tests := []struct {
		source   string
		target   string
		expected string
	}{
		{"/", "/ppt/presentation.xml", "ppt/presentation.xml"},
		{"/ppt/presentation.xml", "/ppt/vbaProject.bin", "vbaProject.bin"},
		{"/ppt/slides/slide1.xml", "/ppt/media/image1.png", "../media/image1.png"},
		{"/xl/workbook.xml", "/xl/worksheets/sheet1.xml", "worksheets/sheet1.xml"},
	}

	for _, tt := range tests {
		if got := RelationshipTarget(tt.source, tt.target); got != tt.expected {
			t.Errorf("RelationshipTarget(%q, %q) = %q, want %q", tt.source, tt.target, got, tt.expected)
		}
	}
}

func TestAllocateRelationshipID(t *testing.T) {
	rels := []RelationshipInfo{
		{ID: "rId1"},
		{ID: "rId3"},
		{ID: "custom"},
	}
	if got := AllocateRelationshipID(rels); got != "rId4" {
		t.Fatalf("AllocateRelationshipID() = %q, want rId4", got)
	}
}

func TestBuildRelationshipsXMLRoundTrip(t *testing.T) {
	input := []RelationshipInfo{
		{
			SourceURI: "/ppt/presentation.xml",
			ID:        "rId1",
			Type:      "http://schemas.microsoft.com/office/2006/relationships/vbaProject",
			Target:    "vbaProject.bin",
		},
		{
			SourceURI:  "/ppt/presentation.xml",
			ID:         "rId2",
			Type:       "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
			Target:     "https://example.com",
			TargetMode: "External",
		},
	}

	data, err := BuildRelationshipsXML(input)
	if err != nil {
		t.Fatalf("BuildRelationshipsXML failed: %v", err)
	}

	parsed, err := ParseRelationships("/ppt/presentation.xml", data)
	if err != nil {
		t.Fatalf("ParseRelationships failed: %v", err)
	}
	if len(parsed) != len(input) {
		t.Fatalf("parsed %d relationships, want %d", len(parsed), len(input))
	}
	for i := range input {
		if parsed[i].ID != input[i].ID || parsed[i].Type != input[i].Type || parsed[i].Target != input[i].Target || parsed[i].TargetMode != input[i].TargetMode {
			t.Fatalf("relationship %d mismatch: got %+v want %+v", i, parsed[i], input[i])
		}
	}
}
