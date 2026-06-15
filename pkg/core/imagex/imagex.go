// Package imagex contains conservative image payload checks for OOXML parts.
package imagex

import (
	"bytes"
	"encoding/binary"
	"image/gif"
	"image/jpeg"
	"image/png"
	"path/filepath"
	"strings"
)

// IsContentType reports whether contentType is an image media type.
func IsContentType(contentType string) bool {
	return strings.HasPrefix(strings.ToLower(strings.TrimSpace(contentType)), "image/")
}

// ContentTypeFromPath returns a supported OOXML image content type from a file path.
func ContentTypeFromPath(path string) (string, bool) {
	return ContentTypeFromExtension(filepath.Ext(path))
}

// ContentTypeFromExtension returns a supported OOXML image content type from an extension.
func ContentTypeFromExtension(ext string) (string, bool) {
	switch strings.ToLower(strings.TrimSpace(ext)) {
	case ".jpg", ".jpeg":
		return "image/jpeg", true
	case ".png":
		return "image/png", true
	case ".gif":
		return "image/gif", true
	case ".bmp":
		return "image/bmp", true
	case ".tif", ".tiff":
		return "image/tiff", true
	case ".webp":
		return "image/webp", true
	case ".svg":
		return "image/svg+xml", true
	case ".emf":
		return "image/x-emf", true
	case ".wmf":
		return "image/x-wmf", true
	default:
		return "", false
	}
}

// ExtensionForContentType returns the OOXML media extension for a supported image content type.
func ExtensionForContentType(contentType string) (string, bool) {
	switch NormalizedContentType(contentType) {
	case "image/png":
		return ".png", true
	case "image/jpeg", "image/jpg", "image/pjpeg":
		return ".jpg", true
	case "image/gif":
		return ".gif", true
	case "image/bmp":
		return ".bmp", true
	case "image/tiff":
		return ".tiff", true
	case "image/webp":
		return ".webp", true
	case "image/svg+xml":
		return ".svg", true
	case "image/x-emf", "image/emf":
		return ".emf", true
	case "image/x-wmf", "image/wmf":
		return ".wmf", true
	default:
		return "", false
	}
}

// HasKnownSignature reports whether PayloadMatchesContentType enforces a
// structural payload check for contentType. Vector or platform-specific formats
// are skipped.
func HasKnownSignature(contentType string) bool {
	switch NormalizedContentType(contentType) {
	case "image/png", "image/jpeg", "image/jpg", "image/pjpeg", "image/gif", "image/bmp", "image/tiff":
		return true
	default:
		return false
	}
}

// PayloadMatchesContentType compares raw bytes with a conservative structural
// check for contentType. Unknown image content types return true so callers can
// avoid false positives for SVG/EMF/WMF/WebP and other formats.
func PayloadMatchesContentType(contentType string, raw []byte) bool {
	switch NormalizedContentType(contentType) {
	case "image/png":
		_, err := png.DecodeConfig(bytes.NewReader(raw))
		return err == nil
	case "image/jpeg", "image/jpg", "image/pjpeg":
		_, err := jpeg.DecodeConfig(bytes.NewReader(raw))
		return err == nil
	case "image/gif":
		_, err := gif.DecodeConfig(bytes.NewReader(raw))
		return err == nil
	case "image/bmp":
		return validBMPHeader(raw)
	case "image/tiff":
		return validTIFFHeader(raw)
	default:
		return true
	}
}

func validBMPHeader(raw []byte) bool {
	if len(raw) < 26 || !bytes.HasPrefix(raw, []byte("BM")) {
		return false
	}
	fileSize := binary.LittleEndian.Uint32(raw[2:6])
	pixelOffset := binary.LittleEndian.Uint32(raw[10:14])
	dibHeaderSize := binary.LittleEndian.Uint32(raw[14:18])
	headerEnd := int64(14) + int64(dibHeaderSize)
	if dibHeaderSize < 12 || headerEnd > int64(len(raw)) {
		return false
	}
	pixelOffset64 := int64(pixelOffset)
	if pixelOffset64 < headerEnd || pixelOffset64 > int64(len(raw)) {
		return false
	}
	return fileSize == 0 || int64(fileSize) <= int64(len(raw))
}

func validTIFFHeader(raw []byte) bool {
	if len(raw) < 8 {
		return false
	}
	var order binary.ByteOrder
	switch {
	case raw[0] == 'I' && raw[1] == 'I':
		order = binary.LittleEndian
	case raw[0] == 'M' && raw[1] == 'M':
		order = binary.BigEndian
	default:
		return false
	}
	magic := order.Uint16(raw[2:4])
	if magic != 42 && magic != 43 {
		return false
	}
	firstIFDOffset := order.Uint32(raw[4:8])
	return firstIFDOffset >= 8 && int64(firstIFDOffset) < int64(len(raw))
}

// NormalizedContentType lowercases, trims, and removes optional parameters.
func NormalizedContentType(contentType string) string {
	contentType = strings.ToLower(strings.TrimSpace(contentType))
	if idx := strings.Index(contentType, ";"); idx >= 0 {
		contentType = strings.TrimSpace(contentType[:idx])
	}
	return contentType
}
