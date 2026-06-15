package opc

import (
	"testing"
)

func TestContentTypesRegistry(t *testing.T) {
	registry := NewContentTypesRegistry()

	// Test default content types
	tests := []struct {
		uri      string
		expected string
	}{
		{"/ppt/slides/slide1.xml", "application/xml"},
		{"/word/document.xml", "application/xml"},
		{"/xl/workbook.xml", "application/xml"},
		{"/ppt/_rels/slide1.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"},
		{"/media/image1.png", "application/octet-stream"},
		{"/xl/workbook.xlsx", "application/octet-stream"}, // .xlsx is a file, not a standard extension
	}

	for _, tt := range tests {
		result := registry.GetContentType(tt.uri)
		if result != tt.expected {
			t.Errorf("GetContentType(%q) = %q, want %q", tt.uri, result, tt.expected)
		}
	}
}

func TestParseContentTypes(t *testing.T) {
	xmlData := []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
</Types>`)

	registry, err := ParseContentTypes(xmlData)
	if err != nil {
		t.Fatalf("ParseContentTypes failed: %v", err)
	}

	// Test the parsed content types
	if ct := registry.GetContentType("/.rels"); ct != "application/vnd.openxmlformats-package.relationships+xml" {
		t.Errorf("got %q for .rels, want application/vnd.openxmlformats-package.relationships+xml", ct)
	}

	if ct := registry.GetContentType("/ppt/presentation.xml"); ct != "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml" {
		t.Errorf("got %q for /ppt/presentation.xml, want application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml", ct)
	}
}

func TestIsXML(t *testing.T) {
	tests := []struct {
		contentType string
		expected    bool
	}{
		{"application/xml", true},
		{"application/vnd.openxmlformats-officedocument.presentationml.slide+xml", true},
		{"image/png", false},
		{"application/octet-stream", false},
		{"", false},
	}

	for _, tt := range tests {
		result := IsXML(tt.contentType)
		if result != tt.expected {
			t.Errorf("IsXML(%q) = %v, want %v", tt.contentType, result, tt.expected)
		}
	}
}

func TestSetOverride(t *testing.T) {
	registry := NewContentTypesRegistry()
	registry.SetOverride("/ppt/slides/slide1.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml")

	result := registry.GetContentType("/ppt/slides/slide1.xml")
	expected := "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"

	if result != expected {
		t.Errorf("GetContentType after SetOverride = %q, want %q", result, expected)
	}
}

func TestSerializeContentTypes(t *testing.T) {
	registry := NewContentTypesRegistry()
	registry.SetOverride("/ppt/presentation.xml", "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml")

	data, err := registry.SerializeXML()
	if err != nil {
		t.Fatalf("SerializeXML failed: %v", err)
	}

	if len(data) == 0 {
		t.Fatal("SerializeXML returned empty data")
	}

	// Verify it can be parsed back
	parsed, err := ParseContentTypes(data)
	if err != nil {
		t.Fatalf("Failed to parse serialized XML: %v", err)
	}

	if ct := parsed.GetContentType("/ppt/presentation.xml"); ct != "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml" {
		t.Errorf("Round-trip failed: got %q", ct)
	}
}

func TestRemoveOverride(t *testing.T) {
	registry := NewContentTypesRegistry()
	registry.SetOverride("/ppt/slides/slide1.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml")
	registry.RemoveOverride("/ppt/slides/slide1.xml")

	if ct := registry.GetContentType("/ppt/slides/slide1.xml"); ct != "application/xml" {
		t.Fatalf("expected fallback XML content type after RemoveOverride, got %q", ct)
	}
}

func TestSerializeContentTypesDeterministic(t *testing.T) {
	registry := NewContentTypesRegistry()
	registry.SetDefault("png", "image/png")
	registry.SetOverride("/z.xml", "application/xml")
	registry.SetOverride("/a.xml", "application/xml")

	first, err := registry.SerializeXML()
	if err != nil {
		t.Fatalf("SerializeXML failed: %v", err)
	}
	second, err := registry.SerializeXML()
	if err != nil {
		t.Fatalf("SerializeXML failed: %v", err)
	}
	if string(first) != string(second) {
		t.Fatal("SerializeXML should be deterministic across calls")
	}
}
