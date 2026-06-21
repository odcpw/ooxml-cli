package render

import (
	"image"
	"image/png"
	"os"
	"path/filepath"
	"testing"
)

func TestReadImageDimensions(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "img.png")

	img := image.NewRGBA(image.Rect(0, 0, 120, 80))
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("create: %v", err)
	}
	if err := png.Encode(file, img); err != nil {
		file.Close()
		t.Fatalf("encode: %v", err)
	}
	file.Close()

	w, h, err := ReadImageDimensions(path)
	if err != nil {
		t.Fatalf("ReadImageDimensions: %v", err)
	}
	if w != 120 || h != 80 {
		t.Errorf("got %dx%d, want 120x80", w, h)
	}
}

func TestReadImageDimensionsMissing(t *testing.T) {
	if _, _, err := ReadImageDimensions(filepath.Join(t.TempDir(), "nope.png")); err == nil {
		t.Fatal("expected error for missing file")
	}
}

func TestReadImageDimensionsInvalid(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "bad.png")
	if err := os.WriteFile(path, []byte("not an image"), 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	if _, _, err := ReadImageDimensions(path); err == nil {
		t.Fatal("expected error for invalid image")
	}
}
