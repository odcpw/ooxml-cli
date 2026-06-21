package opc

import (
	"archive/zip"
)

// ZipEntryMeta is defined in types.go

// NewZipEntryMetaFromFileHeader creates a ZipEntryMeta from a zip.FileHeader.
func NewZipEntryMetaFromFileHeader(fh *zip.FileHeader) *ZipEntryMeta {
	return &ZipEntryMeta{
		Method:       fh.Method,
		ModifiedTime: fh.Modified,
		Comment:      fh.Comment,
	}
}

// ApplyToFileHeader applies the metadata to a zip.FileHeader.
func (m *ZipEntryMeta) ApplyToFileHeader(fh *zip.FileHeader) {
	fh.Method = m.Method
	fh.Modified = m.ModifiedTime
	fh.Comment = m.Comment
}
