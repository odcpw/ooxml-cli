package opc

import (
	"archive/zip"
	"bytes"
	"compress/flate"
	"fmt"
	"hash/crc32"
	"io"
	"os"
	"strings"
	"time"

	"github.com/beevik/etree"
)

var defaultZipModifiedTime = time.Date(1980, time.January, 1, 0, 0, 0, 0, time.UTC)

const (
	zipVersion20 = 20
	zipVersion45 = 45
)

// Package represents an open OPC package (implementation of PackageSession).
type Package struct {
	// File path
	path string

	// Zip reader
	reader *zip.Reader

	// Content types registry
	contentTypes *ContentTypesRegistry

	// Parts: original raw bytes and metadata
	parts map[string][]byte
	metas map[string]*ZipEntryMeta

	// Dirty tracking: which parts have been modified
	dirty   map[string]bool
	removed map[string]bool
	added   map[string]*partData

	// Stable insertion order for added parts.
	addedOrder []string

	// Part info cache
	partInfos map[string]*PartInfo

	// Relationships cache: sourceURI -> []RelationshipInfo
	relationships map[string][]RelationshipInfo

	// File ordering from original zip (for preservation)
	fileOrder []string

	// XML document cache (parsed docs that haven't been modified)
	xmlCache map[string]*etree.Document

	// Tracks whether [Content_Types].xml must be regenerated.
	contentTypesDirty bool

	// Non-fatal package warnings discovered while loading.
	warnings []string
}

type partData struct {
	data        []byte
	contentType string
	meta        *ZipEntryMeta
}

// Open opens an OPC package from the given path.
func Open(path string) (*Package, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("failed to open package: %w", err)
	}
	defer file.Close()

	// Get file size for zip.Reader
	info, err := file.Stat()
	if err != nil {
		return nil, fmt.Errorf("failed to stat package: %w", err)
	}

	// Create zip reader
	zipReader, err := zip.NewReader(file, info.Size())
	if err != nil {
		return nil, fmt.Errorf("failed to read zip: %w", err)
	}
	pkg, err := openZipReader(zipReader)
	if err != nil {
		return nil, err
	}
	pkg.path = path
	return pkg, nil
}

// OpenBytes opens an OPC package from an in-memory ZIP payload.
func OpenBytes(data []byte) (*Package, error) {
	return OpenReader(bytes.NewReader(data), int64(len(data)))
}

// OpenReader opens an OPC package from an arbitrary ZIP reader.
func OpenReader(reader io.ReaderAt, size int64) (*Package, error) {
	zipReader, err := zip.NewReader(reader, size)
	if err != nil {
		return nil, fmt.Errorf("failed to read zip: %w", err)
	}
	return openZipReader(zipReader)
}

func openZipReader(zipReader *zip.Reader) (*Package, error) {
	// Create package
	pkg := &Package{
		reader:            zipReader,
		parts:             make(map[string][]byte),
		metas:             make(map[string]*ZipEntryMeta),
		dirty:             make(map[string]bool),
		removed:           make(map[string]bool),
		added:             make(map[string]*partData),
		addedOrder:        make([]string, 0),
		partInfos:         make(map[string]*PartInfo),
		relationships:     make(map[string][]RelationshipInfo),
		fileOrder:         make([]string, 0),
		xmlCache:          make(map[string]*etree.Document),
		contentTypesDirty: false,
		warnings:          make([]string, 0),
	}

	// Load all parts from zip
	if err := pkg.loadParts(); err != nil {
		return nil, err
	}

	// Parse content types
	if err := pkg.loadContentTypes(); err != nil {
		return nil, err
	}

	// Parse relationships
	if err := pkg.loadRelationships(); err != nil {
		return nil, err
	}

	return pkg, nil
}

// loadParts loads all parts from the zip archive.
const (
	// Guards against decompression ("zip bomb") attacks: a small compressed
	// entry can inflate to many GB and exhaust memory. These ceilings are far
	// above any legitimate Office part/package.
	maxPartUncompressedBytes  = 256 << 20 // 256 MiB per part
	maxTotalUncompressedBytes = 512 << 20 // 512 MiB per package
)

func (p *Package) loadParts() error {
	var totalBytes int64
	for _, file := range p.reader.File {
		uri := NormalizeURI("/" + file.Name)

		// Skip directories
		if strings.HasSuffix(file.Name, "/") {
			continue
		}

		// Reject parts whose declared uncompressed size already blows the cap,
		// before spending any work decompressing them.
		if file.UncompressedSize64 > maxPartUncompressedBytes {
			return fmt.Errorf("zip entry %s is too large (%d bytes uncompressed)", file.Name, file.UncompressedSize64)
		}

		// Read raw bytes with a hard ceiling, in case the central-directory size
		// (attacker-controlled) understates the real decompressed length.
		rc, err := file.Open()
		if err != nil {
			return fmt.Errorf("failed to open zip entry %s: %w", file.Name, err)
		}

		data, err := io.ReadAll(io.LimitReader(rc, maxPartUncompressedBytes+1))
		rc.Close()
		if err != nil {
			return fmt.Errorf("failed to read zip entry %s: %w", file.Name, err)
		}
		if int64(len(data)) > maxPartUncompressedBytes {
			return fmt.Errorf("zip entry %s exceeds the %d byte uncompressed limit", file.Name, int64(maxPartUncompressedBytes))
		}
		totalBytes += int64(len(data))
		if totalBytes > maxTotalUncompressedBytes {
			return fmt.Errorf("package exceeds the %d byte total uncompressed limit", int64(maxTotalUncompressedBytes))
		}

		p.parts[uri] = cloneBytes(data)
		p.metas[uri] = cloneZipMeta(NewZipEntryMetaFromFileHeader(&file.FileHeader))
		p.fileOrder = append(p.fileOrder, uri)
	}

	return nil
}

// loadContentTypes loads and parses the [Content_Types].xml file.
func (p *Package) loadContentTypes() error {
	ctData, exists := p.parts["/[Content_Types].xml"]
	if !exists {
		// Create a default content types registry
		p.contentTypes = NewContentTypesRegistry()
		return nil
	}

	var err error
	p.contentTypes, err = ParseContentTypes(ctData)
	return err
}

// loadRelationships loads and parses all .rels files.
func (p *Package) loadRelationships() error {
	for uri, data := range p.parts {
		if !IsRelsFile(uri) {
			continue
		}

		sourceURI := extractSourceURIFromRelsPath(uri)
		rels, err := ParseRelationships(sourceURI, data)
		if err != nil {
			p.warnings = append(p.warnings, fmt.Sprintf("failed to parse relationships part %s: %v", uri, err))
			continue
		}

		p.relationships[sourceURI] = cloneRelationships(rels)
	}

	return nil
}

// extractSourceURIFromRelsPath converts a .rels path to its source URI.
// e.g., "/ppt/slides/_rels/slide1.xml.rels" -> "/ppt/slides/slide1.xml"
func extractSourceURIFromRelsPath(relsPath string) string {
	dir := GetDirectory(relsPath)
	if strings.HasSuffix(dir, "/_rels") {
		dir = dir[:len(dir)-6] // Remove "/_rels"
	}

	fileName := GetFileName(relsPath)
	if strings.HasSuffix(fileName, ".rels") {
		fileName = fileName[:len(fileName)-5] // Remove ".rels"
	}

	return JoinPaths(dir, fileName)
}

// ListParts returns all parts in the package (excluding removed parts).
func (p *Package) ListParts() []PartInfo {
	result := make([]PartInfo, 0)

	for _, uri := range p.fileOrder {
		if p.removed[uri] {
			continue
		}
		result = append(result, *p.getOrCreatePartInfo(uri))
	}

	for _, uri := range p.addedOrder {
		if p.removed[uri] {
			continue
		}
		if _, exists := p.added[uri]; !exists {
			continue
		}
		result = append(result, *p.getOrCreatePartInfo(uri))
	}

	return result
}

// getOrCreatePartInfo gets or creates a PartInfo for a URI.
func (p *Package) getOrCreatePartInfo(uri string) *PartInfo {
	if info, exists := p.partInfos[uri]; exists {
		return info
	}

	var size int64
	contentType := p.GetContentType(uri)

	if data, exists := p.parts[uri]; exists {
		size = int64(len(data))
	} else if partData, exists := p.added[uri]; exists {
		size = int64(len(partData.data))
	}

	info := &PartInfo{
		URI:         uri,
		ContentType: contentType,
		SizeBytes:   size,
		IsXML:       IsXML(contentType),
	}

	p.partInfos[uri] = info
	return info
}

// ListRelationships returns all relationships from a source URI.
func (p *Package) ListRelationships(sourceURI string) []RelationshipInfo {
	sourceURI = NormalizeURI(sourceURI)
	if rels, exists := p.relationships[sourceURI]; exists {
		return cloneRelationships(rels)
	}
	return []RelationshipInfo{}
}

// ReadRawPart returns the raw bytes of a part.
func (p *Package) ReadRawPart(uri string) ([]byte, error) {
	uri = NormalizeURI(uri)

	if p.removed[uri] {
		return nil, fmt.Errorf("part %s has been removed", uri)
	}

	if partData, exists := p.added[uri]; exists {
		return cloneBytes(partData.data), nil
	}

	if data, exists := p.parts[uri]; exists {
		return cloneBytes(data), nil
	}

	return nil, fmt.Errorf("part %s not found", uri)
}

// ReadXMLPart returns a parsed XML document for a part.
// IMPORTANT: This does NOT mark the part as dirty.
func (p *Package) ReadXMLPart(uri string) (*etree.Document, error) {
	uri = NormalizeURI(uri)

	if doc, exists := p.xmlCache[uri]; exists {
		return doc.Copy(), nil
	}

	raw, err := p.ReadRawPart(uri)
	if err != nil {
		return nil, err
	}

	doc := etree.NewDocument()
	if err := doc.ReadFromBytes(raw); err != nil {
		return nil, fmt.Errorf("failed to parse XML part %s: %w", uri, err)
	}

	p.xmlCache[uri] = doc
	return doc.Copy(), nil
}

// GetContentType returns the content type for a part.
func (p *Package) GetContentType(uri string) string {
	uri = NormalizeURI(uri)

	if partData, exists := p.added[uri]; exists {
		return partData.contentType
	}

	return p.contentTypes.GetContentType(uri)
}

// GetZipMeta returns zip-level metadata for a part.
func (p *Package) GetZipMeta(uri string) *ZipEntryMeta {
	uri = NormalizeURI(uri)
	if meta, exists := p.metas[uri]; exists {
		return cloneZipMeta(meta)
	}
	if partData, exists := p.added[uri]; exists {
		return cloneZipMeta(partData.meta)
	}
	return nil
}

// ReplaceRawPart replaces a part with raw bytes, marking it dirty.
func (p *Package) ReplaceRawPart(uri string, data []byte, contentType string) error {
	uri = NormalizeURI(uri)
	currentContentType := p.GetContentType(uri)

	if _, exists := p.parts[uri]; !exists {
		if _, addedExists := p.added[uri]; !addedExists {
			return p.AddPart(uri, data, contentType, nil)
		}
	}

	if p.removed[uri] {
		delete(p.removed, uri)
	}

	if added, exists := p.added[uri]; exists {
		added.data = cloneBytes(data)
		added.contentType = contentType
		if currentContentType != contentType {
			p.contentTypesDirty = true
		}
		p.contentTypes.SetOverride(uri, contentType)
		delete(p.xmlCache, uri)
		delete(p.partInfos, uri)
		return p.updateRelationshipsCacheForPart(uri, data)
	}

	p.dirty[uri] = true
	p.parts[uri] = cloneBytes(data)
	if currentContentType != contentType {
		p.contentTypesDirty = true
	}
	p.contentTypes.SetOverride(uri, contentType)

	delete(p.xmlCache, uri)
	delete(p.partInfos, uri)

	return p.updateRelationshipsCacheForPart(uri, data)
}

// ReplaceXMLPart replaces a part with an XML document, marking it dirty.
func (p *Package) ReplaceXMLPart(uri string, doc *etree.Document) error {
	uri = NormalizeURI(uri)

	data, err := doc.WriteToBytes()
	if err != nil {
		return fmt.Errorf("failed to serialize XML: %w", err)
	}

	contentType := p.GetContentType(uri)
	if !IsXML(contentType) {
		contentType = "application/xml"
	}

	if err := p.ReplaceRawPart(uri, data, contentType); err != nil {
		return err
	}

	p.xmlCache[uri] = doc.Copy()
	return nil
}

// AddPart adds a new part to the package.
func (p *Package) AddPart(uri string, data []byte, contentType string, meta *ZipEntryMeta) error {
	uri = NormalizeURI(uri)
	currentContentType := p.GetContentType(uri)

	if meta == nil {
		meta = &ZipEntryMeta{Method: zip.Deflate}
	}
	meta = cloneZipMeta(meta)

	// Re-adding or replacing an original part should update that original slot rather than
	// staging a duplicate added entry.
	if _, exists := p.parts[uri]; exists {
		delete(p.removed, uri)
		p.dirty[uri] = true
		p.parts[uri] = cloneBytes(data)
		p.metas[uri] = cloneZipMeta(meta)
		if currentContentType != contentType {
			p.contentTypesDirty = true
		}
		p.contentTypes.SetOverride(uri, contentType)
		delete(p.xmlCache, uri)
		delete(p.partInfos, uri)
		return p.updateRelationshipsCacheForPart(uri, data)
	}

	if _, exists := p.added[uri]; !exists {
		p.addedOrder = append(p.addedOrder, uri)
	}
	p.added[uri] = &partData{
		data:        cloneBytes(data),
		contentType: contentType,
		meta:        meta,
	}
	delete(p.removed, uri)

	p.contentTypesDirty = true
	p.contentTypes.SetOverride(uri, contentType)
	delete(p.xmlCache, uri)
	delete(p.partInfos, uri)
	return p.updateRelationshipsCacheForPart(uri, data)
}

// RemovePart marks a part for removal.
func (p *Package) RemovePart(uri string) error {
	uri = NormalizeURI(uri)

	if _, exists := p.added[uri]; exists {
		delete(p.added, uri)
		p.removeAddedOrder(uri)
		p.contentTypesDirty = true
		p.contentTypes.RemoveOverride(uri)
		delete(p.xmlCache, uri)
		delete(p.partInfos, uri)
		if IsRelsFile(uri) {
			delete(p.relationships, extractSourceURIFromRelsPath(uri))
		}
		return nil
	}

	if _, exists := p.parts[uri]; !exists {
		return nil
	}
	if p.removed[uri] {
		return nil
	}

	p.removed[uri] = true
	delete(p.dirty, uri)
	delete(p.xmlCache, uri)
	delete(p.partInfos, uri)
	p.contentTypesDirty = true
	p.contentTypes.RemoveOverride(uri)
	if IsRelsFile(uri) {
		delete(p.relationships, extractSourceURIFromRelsPath(uri))
	}

	return nil
}

// SaveAs saves the package to a new file.
func (p *Package) SaveAs(path string) error {
	outFile, err := os.Create(path)
	if err != nil {
		return fmt.Errorf("failed to create output file: %w", err)
	}

	zipWriter := zip.NewWriter(outFile)
	if err := p.writeToZip(zipWriter); err != nil {
		_ = outFile.Close()
		return err
	}

	if err := outFile.Close(); err != nil {
		return fmt.Errorf("failed to close output file: %w", err)
	}

	return nil
}

// WriteToBytes serializes the package to an in-memory ZIP payload.
func (p *Package) WriteToBytes() ([]byte, error) {
	var buf bytes.Buffer
	zipWriter := zip.NewWriter(&buf)
	if err := p.writeToZip(zipWriter); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

func (p *Package) writeToZip(zipWriter *zip.Writer) error {
	if err := p.ensureAndWriteContentTypes(zipWriter); err != nil {
		_ = zipWriter.Close()
		return err
	}

	for _, uri := range p.fileOrder {
		if p.removed[uri] || uri == "/[Content_Types].xml" {
			continue
		}

		if err := writeZipEntry(zipWriter, strings.TrimPrefix(uri, "/"), p.parts[uri], p.metas[uri]); err != nil {
			_ = zipWriter.Close()
			return fmt.Errorf("failed to write zip entry for %s: %w", uri, err)
		}
	}

	for _, uri := range p.addedOrder {
		partData, exists := p.added[uri]
		if !exists || uri == "/[Content_Types].xml" {
			continue
		}

		if err := writeZipEntry(zipWriter, strings.TrimPrefix(uri, "/"), partData.data, partData.meta); err != nil {
			_ = zipWriter.Close()
			return fmt.Errorf("failed to write zip entry for %s: %w", uri, err)
		}
	}

	if err := zipWriter.Close(); err != nil {
		return fmt.Errorf("failed to close zip writer: %w", err)
	}
	return nil
}

func writeZipEntry(zipWriter *zip.Writer, name string, data []byte, meta *ZipEntryMeta) error {
	method := zip.Deflate
	if meta != nil {
		method = meta.Method
	}

	compressedData := data
	switch method {
	case zip.Store:
		compressedData = data
	case zip.Deflate:
		deflated, err := deflateData(data)
		if err != nil {
			return err
		}
		compressedData = deflated
	default:
		return fmt.Errorf("unsupported zip method %d for %s", method, name)
	}

	header := &zip.FileHeader{
		Name:               name,
		Method:             method,
		CreatorVersion:     zipVersion20,
		ReaderVersion:      zipVersion20,
		CRC32:              crc32.ChecksumIEEE(data),
		UncompressedSize64: uint64(len(data)),
		CompressedSize64:   uint64(len(compressedData)),
		Flags:              0,
	}
	if uint64(len(data)) > uint64(^uint32(0)) || uint64(len(compressedData)) > uint64(^uint32(0)) {
		header.ReaderVersion = zipVersion45
	}
	if meta != nil {
		if !meta.ModifiedTime.IsZero() {
			header.SetModTime(meta.ModifiedTime)
		}
		header.Comment = meta.Comment
	}
	if header.Modified.IsZero() {
		header.SetModTime(defaultZipModifiedTime)
	}

	w, err := zipWriter.CreateRaw(header)
	if err != nil {
		return err
	}
	if _, err := w.Write(compressedData); err != nil {
		return err
	}
	return nil
}

func deflateData(data []byte) ([]byte, error) {
	var buf bytes.Buffer
	w, err := flate.NewWriter(&buf, flate.DefaultCompression)
	if err != nil {
		return nil, err
	}
	if _, err := w.Write(data); err != nil {
		_ = w.Close()
		return nil, err
	}
	if err := w.Close(); err != nil {
		return nil, err
	}
	return buf.Bytes(), nil
}

// ensureAndWriteContentTypes ensures [Content_Types].xml is present and writes it.
func (p *Package) ensureAndWriteContentTypes(zipWriter *zip.Writer) error {
	if !p.contentTypesDirty {
		if raw, exists := p.parts["/[Content_Types].xml"]; exists && !p.removed["/[Content_Types].xml"] {
			return writeZipEntry(zipWriter, "[Content_Types].xml", raw, p.metas["/[Content_Types].xml"])
		}
	}

	ctData, err := p.contentTypes.SerializeXML()
	if err != nil {
		return fmt.Errorf("failed to serialize content types: %w", err)
	}

	meta := p.metas["/[Content_Types].xml"]
	if meta == nil {
		meta = &ZipEntryMeta{Method: zip.Deflate}
	}
	return writeZipEntry(zipWriter, "[Content_Types].xml", ctData, meta)
}

// Close closes the session.
func (p *Package) Close() error {
	p.reader = nil
	p.contentTypes = nil
	p.parts = nil
	p.metas = nil
	p.dirty = nil
	p.removed = nil
	p.added = nil
	p.addedOrder = nil
	p.partInfos = nil
	p.xmlCache = nil
	p.relationships = nil
	p.fileOrder = nil
	p.warnings = nil
	return nil
}

// IsDirty returns true if any parts have been modified.
func (p *Package) IsDirty() bool {
	return len(p.dirty) > 0 || len(p.removed) > 0 || len(p.added) > 0
}

// Warnings returns non-fatal load-time package warnings.
func (p *Package) Warnings() []string {
	copied := make([]string, len(p.warnings))
	copy(copied, p.warnings)
	return copied
}

func (p *Package) updateRelationshipsCacheForPart(uri string, data []byte) error {
	if !IsRelsFile(uri) {
		return nil
	}
	sourceURI := extractSourceURIFromRelsPath(uri)
	rels, err := ParseRelationships(sourceURI, data)
	if err != nil {
		return err
	}
	p.relationships[sourceURI] = cloneRelationships(rels)
	return nil
}

func (p *Package) removeAddedOrder(uri string) {
	for i, candidate := range p.addedOrder {
		if candidate == uri {
			p.addedOrder = append(p.addedOrder[:i], p.addedOrder[i+1:]...)
			return
		}
	}
}

func cloneBytes(data []byte) []byte {
	if data == nil {
		return nil
	}
	copied := make([]byte, len(data))
	copy(copied, data)
	return copied
}

func cloneZipMeta(meta *ZipEntryMeta) *ZipEntryMeta {
	if meta == nil {
		return nil
	}
	copied := *meta
	return &copied
}

func cloneRelationships(rels []RelationshipInfo) []RelationshipInfo {
	if len(rels) == 0 {
		return []RelationshipInfo{}
	}
	copied := make([]RelationshipInfo, len(rels))
	copy(copied, rels)
	return copied
}
