package opc

import (
	"time"

	"github.com/beevik/etree"
)

// PartInfo describes an OPC part (zip entry).
type PartInfo struct {
	URI         string
	ContentType string
	SizeBytes   int64
	IsXML       bool
}

// Copy returns a deep copy of PartInfo.
func (p *PartInfo) Copy() *PartInfo {
	if p == nil {
		return nil
	}
	return &PartInfo{
		URI:         p.URI,
		ContentType: p.ContentType,
		SizeBytes:   p.SizeBytes,
		IsXML:       p.IsXML,
	}
}

// RelationshipInfo describes an OPC relationship.
type RelationshipInfo struct {
	SourceURI  string
	ID         string
	Type       string
	Target     string
	TargetMode string // "External" or ""
}

// ZipEntryMeta holds metadata about a zip entry that should be preserved.
type ZipEntryMeta struct {
	Method       uint16 // zip.Store or zip.Deflate
	ModifiedTime time.Time
	Comment      string
}

// PackageType represents the type of OOXML package.
type PackageType string

const (
	PackageTypePPTX    PackageType = "pptx"
	PackageTypeDOCX    PackageType = "docx"
	PackageTypeXLSX    PackageType = "xlsx"
	PackageTypeUnknown PackageType = "unknown"
)

// String returns the string representation of the PackageType.
func (pt PackageType) String() string {
	return string(pt)
}

// PackageSession represents an open OPC package for reading and writing.
// Important: ReadXMLPart does NOT mark the part as dirty. Only ReplaceXMLPart,
// ReplaceRawPart, AddPart, and RemovePart mark parts as changed.
type PackageSession interface {
	// Read-side operations

	// ListParts returns all parts in the package.
	ListParts() []PartInfo

	// ListRelationships returns all relationships from a source URI.
	// sourceURI is typically "/" for package-level relationships.
	ListRelationships(sourceURI string) []RelationshipInfo

	// ReadRawPart returns the raw bytes of a part.
	ReadRawPart(uri string) ([]byte, error)

	// ReadXMLPart returns a parsed XML document for a part.
	// IMPORTANT: This does NOT mark the part as dirty.
	ReadXMLPart(uri string) (*etree.Document, error)

	// GetContentType returns the content type for a part.
	GetContentType(uri string) string

	// GetZipMeta returns zip-level metadata for a part (compression, timestamp, etc.).
	GetZipMeta(uri string) *ZipEntryMeta

	// Write-side operations

	// ReplaceRawPart replaces a part with raw bytes, marking it dirty.
	ReplaceRawPart(uri string, data []byte, contentType string) error

	// ReplaceXMLPart replaces a part with an XML document, marking it dirty.
	ReplaceXMLPart(uri string, doc *etree.Document) error

	// AddPart adds a new part to the package.
	AddPart(uri string, data []byte, contentType string, meta *ZipEntryMeta) error

	// RemovePart marks a part for removal.
	RemovePart(uri string) error

	// SaveAs saves the package to a new file, preserving untouched parts as raw bytes.
	SaveAs(path string) error

	// Close closes the session and releases resources.
	Close() error

	// IsDirty returns true if any parts have been modified.
	IsDirty() bool

	// Warnings returns non-fatal package warnings discovered while loading.
	Warnings() []string
}
