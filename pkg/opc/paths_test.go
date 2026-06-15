package opc

import (
	"testing"
)

func TestNormalizeURI(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"", "/"},
		{"/", "/"},
		{"ppt/slides/slide1.xml", "/ppt/slides/slide1.xml"},
		{"/ppt/slides/slide1.xml", "/ppt/slides/slide1.xml"},
		{"/ppt/slides/./slide1.xml", "/ppt/slides/slide1.xml"},
		{"/ppt/slides/../slide1.xml", "/ppt/slide1.xml"},
		{"/ppt/slides/../../slide1.xml", "/slide1.xml"},
		{"/ppt/slides/", "/ppt/slides"},
		{"/", "/"},
	}

	for _, tt := range tests {
		result := NormalizeURI(tt.input)
		if result != tt.expected {
			t.Errorf("NormalizeURI(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestGetFileExtension(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"/ppt/slides/slide1.xml", "xml"},
		{"/ppt/slides/slide1", ""},
		{"/ppt/slides/.rels", "rels"},
		{"/file.tar.gz", "gz"},
	}

	for _, tt := range tests {
		result := GetFileExtension(tt.input)
		if result != tt.expected {
			t.Errorf("GetFileExtension(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestGetDirectory(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"/ppt/slides/slide1.xml", "/ppt/slides"},
		{"/ppt/slides/", "/ppt/slides"},
		{"/file.xml", "/"},
		{"/", "/"},
		{"ppt/slides/slide1.xml", "/ppt/slides"},
	}

	for _, tt := range tests {
		result := GetDirectory(tt.input)
		if result != tt.expected {
			t.Errorf("GetDirectory(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestGetFileName(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{"/ppt/slides/slide1.xml", "slide1.xml"},
		{"/ppt/slides/", ""},
		{"/file.xml", "file.xml"},
		{"file.xml", "file.xml"},
	}

	for _, tt := range tests {
		result := GetFileName(tt.input)
		if result != tt.expected {
			t.Errorf("GetFileName(%q) = %q, want %q", tt.input, result, tt.expected)
		}
	}
}

func TestJoinPaths(t *testing.T) {
	tests := []struct {
		base     string
		relative string
		expected string
	}{
		{"/ppt/slides", "../slideLayouts/slideLayout1.xml", "/ppt/slideLayouts/slideLayout1.xml"},
		{"/ppt/slides/slide1.xml", "../slideLayouts/slideLayout1.xml", "/ppt/slideLayouts/slideLayout1.xml"},
		{"/", "ppt/slides/slide1.xml", "/ppt/slides/slide1.xml"},
		{"/ppt", "./slides/slide1.xml", "/ppt/slides/slide1.xml"},
	}

	for _, tt := range tests {
		result := JoinPaths(tt.base, tt.relative)
		if result != tt.expected {
			t.Errorf("JoinPaths(%q, %q) = %q, want %q", tt.base, tt.relative, result, tt.expected)
		}
	}
}

func TestIsRelsFile(t *testing.T) {
	tests := []struct {
		uri      string
		expected bool
	}{
		{"/ppt/slides/_rels/slide1.xml.rels", true},
		{"/ppt/slides/slide1.xml", false},
		{"/.rels", true},
		{"/ppt/slides/.rels", true},
	}

	for _, tt := range tests {
		result := IsRelsFile(tt.uri)
		if result != tt.expected {
			t.Errorf("IsRelsFile(%q) = %v, want %v", tt.uri, result, tt.expected)
		}
	}
}
