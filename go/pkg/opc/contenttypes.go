package opc

import (
	"encoding/xml"
	"fmt"
	"sort"
)

// ContentTypesRegistry holds the content types for an OPC package.
type ContentTypesRegistry struct {
	// defaults maps file extensions to default content types (e.g., "rels" -> "application/vnd.openxmlformats-package.relationships+xml")
	defaults map[string]string
	// overrides maps full part URIs to specific content types
	overrides map[string]string
}

// ctRoot is the XML structure of [Content_Types].xml.
type ctRoot struct {
	XMLName   xml.Name     `xml:"http://schemas.openxmlformats.org/package/2006/content-types Types"`
	Defaults  []ctDefault  `xml:"http://schemas.openxmlformats.org/package/2006/content-types Default"`
	Overrides []ctOverride `xml:"http://schemas.openxmlformats.org/package/2006/content-types Override"`
}

type ctDefault struct {
	Extension   string `xml:"Extension,attr"`
	ContentType string `xml:"ContentType,attr"`
}

type ctOverride struct {
	PartName    string `xml:"PartName,attr"`
	ContentType string `xml:"ContentType,attr"`
}

// NewContentTypesRegistry creates a new content types registry with standard defaults.
func NewContentTypesRegistry() *ContentTypesRegistry {
	return &ContentTypesRegistry{
		defaults: map[string]string{
			"rels": "application/vnd.openxmlformats-package.relationships+xml",
			"xml":  "application/xml",
		},
		overrides: make(map[string]string),
	}
}

// ParseContentTypes parses the [Content_Types].xml file and returns a registry.
func ParseContentTypes(data []byte) (*ContentTypesRegistry, error) {
	registry := NewContentTypesRegistry()

	var root ctRoot
	if err := xml.Unmarshal(data, &root); err != nil {
		return nil, fmt.Errorf("failed to parse [Content_Types].xml: %w", err)
	}

	// Clear defaults and add from XML
	registry.defaults = make(map[string]string)
	for _, d := range root.Defaults {
		registry.defaults[d.Extension] = d.ContentType
	}

	// Add overrides
	for _, o := range root.Overrides {
		registry.overrides[o.PartName] = o.ContentType
	}

	return registry, nil
}

// GetContentType returns the content type for a given part URI.
func (r *ContentTypesRegistry) GetContentType(partURI string) string {
	// Check overrides first
	if ct, ok := r.overrides[partURI]; ok {
		return ct
	}

	// Extract extension from URI and check defaults
	lastDot := -1
	for i := len(partURI) - 1; i >= 0; i-- {
		if partURI[i] == '.' {
			lastDot = i
			break
		}
		if partURI[i] == '/' {
			break
		}
	}

	if lastDot > 0 {
		ext := partURI[lastDot+1:]
		if ct, ok := r.defaults[ext]; ok {
			return ct
		}
	}

	// Default fallback for unknown types
	return "application/octet-stream"
}

// SetOverride sets the content type for a specific part.
func (r *ContentTypesRegistry) SetOverride(partURI, contentType string) {
	r.overrides[partURI] = contentType
}

// RemoveOverride removes a part-specific content type override.
func (r *ContentTypesRegistry) RemoveOverride(partURI string) {
	delete(r.overrides, partURI)
}

// SetDefault sets the default content type for a file extension.
func (r *ContentTypesRegistry) SetDefault(extension, contentType string) {
	r.defaults[extension] = contentType
}

// SerializeXML serializes the content types to XML bytes.
func (r *ContentTypesRegistry) SerializeXML() ([]byte, error) {
	root := ctRoot{
		XMLName:   xml.Name{Space: "http://schemas.openxmlformats.org/package/2006/content-types", Local: "Types"},
		Defaults:  make([]ctDefault, 0, len(r.defaults)),
		Overrides: make([]ctOverride, 0, len(r.overrides)),
	}

	defaultKeys := make([]string, 0, len(r.defaults))
	for ext := range r.defaults {
		defaultKeys = append(defaultKeys, ext)
	}
	sort.Strings(defaultKeys)
	for _, ext := range defaultKeys {
		root.Defaults = append(root.Defaults, ctDefault{
			Extension:   ext,
			ContentType: r.defaults[ext],
		})
	}

	overrideKeys := make([]string, 0, len(r.overrides))
	for partName := range r.overrides {
		overrideKeys = append(overrideKeys, partName)
	}
	sort.Strings(overrideKeys)
	for _, partName := range overrideKeys {
		root.Overrides = append(root.Overrides, ctOverride{
			PartName:    partName,
			ContentType: r.overrides[partName],
		})
	}

	data, err := xml.MarshalIndent(root, "", "  ")
	if err != nil {
		return nil, err
	}

	// Add XML declaration
	result := append([]byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>`+"\n"), data...)
	return result, nil
}

// IsXML returns true if the content type indicates an XML part.
func IsXML(contentType string) bool {
	if len(contentType) == 0 {
		return false
	}
	// Check for +xml suffix (e.g., "application/vnd.openxmlformats-officedocument.presentationml.slide+xml")
	if len(contentType) >= 4 && contentType[len(contentType)-4:] == "+xml" {
		return true
	}
	// Check for specific XML content types
	return contentType == "application/xml" || contentType == "text/xml"
}
