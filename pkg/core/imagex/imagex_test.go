package imagex

import (
	"bytes"
	"image"
	"image/png"
	"testing"
)

func TestPayloadMatchesContentTypeRejectsTruncatedPNG(t *testing.T) {
	headerOnlyPNG := []byte{0x89, 'P', 'N', 'G', '\r', '\n', 0x1a, '\n'}
	if PayloadMatchesContentType("image/png", headerOnlyPNG) {
		t.Fatal("expected truncated PNG header to fail structural validation")
	}
}

func TestPayloadMatchesContentTypeAllowsValidPNGAndUnknownImages(t *testing.T) {
	var buf bytes.Buffer
	if err := png.Encode(&buf, image.NewRGBA(image.Rect(0, 0, 1, 1))); err != nil {
		t.Fatalf("encode png: %v", err)
	}
	if !PayloadMatchesContentType("image/png", buf.Bytes()) {
		t.Fatal("expected valid PNG to pass structural validation")
	}
	if !PayloadMatchesContentType("image/svg+xml", []byte("<svg/>")) {
		t.Fatal("expected unknown/vector image type to be skipped")
	}
}

func TestContentTypeAndExtensionMappings(t *testing.T) {
	for _, tt := range []struct {
		path        string
		contentType string
		extension   string
	}{
		{"image.png", "image/png", ".png"},
		{"image.JPG", "image/jpeg", ".jpg"},
		{"image.tiff", "image/tiff", ".tiff"},
		{"image.svg", "image/svg+xml", ".svg"},
		{"image.webp", "image/webp", ".webp"},
		{"image.emf", "image/x-emf", ".emf"},
		{"image.wmf", "image/x-wmf", ".wmf"},
	} {
		contentType, ok := ContentTypeFromPath(tt.path)
		if !ok || contentType != tt.contentType {
			t.Fatalf("%s content type = %q, %t; want %q, true", tt.path, contentType, ok, tt.contentType)
		}
		extension, ok := ExtensionForContentType(contentType)
		if !ok || extension != tt.extension {
			t.Fatalf("%s extension = %q, %t; want %q, true", contentType, extension, ok, tt.extension)
		}
	}

	if contentType, ok := ContentTypeFromPath("image.bin"); ok || contentType != "" {
		t.Fatalf("unsupported extension should not produce content type: %q %t", contentType, ok)
	}
	if extension, ok := ExtensionForContentType("application/octet-stream"); ok || extension != "" {
		t.Fatalf("unsupported content type should not produce extension: %q %t", extension, ok)
	}
}
