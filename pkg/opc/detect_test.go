package opc

import (
	"testing"

	"github.com/beevik/etree"
)

// MockPackageSession is a mock implementation of PackageSession for testing.
type MockPackageSession struct {
	parts         []PartInfo
	relationships map[string][]RelationshipInfo
}

func (m *MockPackageSession) ListParts() []PartInfo {
	return m.parts
}

func (m *MockPackageSession) ListRelationships(sourceURI string) []RelationshipInfo {
	if rels, ok := m.relationships[sourceURI]; ok {
		return rels
	}
	return []RelationshipInfo{}
}

func (m *MockPackageSession) ReadRawPart(uri string) ([]byte, error) {
	return nil, nil
}

func (m *MockPackageSession) ReadXMLPart(uri string) (*etree.Document, error) {
	return nil, nil
}

func (m *MockPackageSession) GetContentType(uri string) string {
	for _, part := range m.parts {
		if part.URI == uri {
			return part.ContentType
		}
	}
	return ""
}

func (m *MockPackageSession) GetZipMeta(uri string) *ZipEntryMeta {
	return nil
}

func (m *MockPackageSession) ReplaceRawPart(uri string, data []byte, contentType string) error {
	return nil
}

func (m *MockPackageSession) ReplaceXMLPart(uri string, doc *etree.Document) error {
	return nil
}

func (m *MockPackageSession) AddPart(uri string, data []byte, contentType string, meta *ZipEntryMeta) error {
	return nil
}

func (m *MockPackageSession) RemovePart(uri string) error {
	return nil
}

func (m *MockPackageSession) SaveAs(path string) error {
	return nil
}

func (m *MockPackageSession) Close() error {
	return nil
}

func (m *MockPackageSession) IsDirty() bool {
	return false
}

func (m *MockPackageSession) Warnings() []string {
	return nil
}

func TestDetectPPTX(t *testing.T) {
	session := &MockPackageSession{
		relationships: map[string][]RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/presentationml.presentation",
					Target:    "ppt/presentation.xml",
				},
			},
		},
	}

	result := DetectType(session)
	if result != PackageTypePPTX {
		t.Errorf("DetectType() = %q, want %q", result, PackageTypePPTX)
	}
}

func TestDetectDOCX(t *testing.T) {
	session := &MockPackageSession{
		relationships: map[string][]RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/wordprocessingml.document",
					Target:    "word/document.xml",
				},
			},
		},
	}

	result := DetectType(session)
	if result != PackageTypeDOCX {
		t.Errorf("DetectType() = %q, want %q", result, PackageTypeDOCX)
	}
}

func TestDetectXLSX(t *testing.T) {
	session := &MockPackageSession{
		relationships: map[string][]RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/spreadsheetml.sheet",
					Target:    "xl/workbook.xml",
				},
			},
		},
	}

	result := DetectType(session)
	if result != PackageTypeXLSX {
		t.Errorf("DetectType() = %q, want %q", result, PackageTypeXLSX)
	}
}

func TestDetectXLSXFromOfficeDocumentRelationship(t *testing.T) {
	session := &MockPackageSession{
		parts: []PartInfo{
			{
				URI:         "/xl/workbook.xml",
				ContentType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
				IsXML:       true,
			},
		},
		relationships: map[string][]RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
					Target:    "xl/workbook.xml",
				},
			},
		},
	}

	result := DetectType(session)
	if result != PackageTypeXLSX {
		t.Errorf("DetectType() = %q, want %q", result, PackageTypeXLSX)
	}
}

func TestDetectByContentType(t *testing.T) {
	session := &MockPackageSession{
		parts: []PartInfo{
			{
				URI:         "/ppt/presentation.xml",
				ContentType: "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
				IsXML:       true,
			},
		},
		relationships: map[string][]RelationshipInfo{},
	}

	result := DetectType(session)
	if result != PackageTypePPTX {
		t.Errorf("DetectType() = %q, want %q", result, PackageTypePPTX)
	}
}

func TestDetectUnknown(t *testing.T) {
	session := &MockPackageSession{
		parts:         []PartInfo{},
		relationships: map[string][]RelationshipInfo{},
	}

	result := DetectType(session)
	if result != PackageTypeUnknown {
		t.Errorf("DetectType() = %q, want %q", result, PackageTypeUnknown)
	}
}
