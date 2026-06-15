package render

import (
	"errors"
	"testing"
)

func TestBuildThumbnailManifest(t *testing.T) {
	dims := map[string][2]int{
		"/out/thumb-1.png": {800, 600},
		"/out/thumb-2.png": {640, 480},
	}
	dimFn := func(path string) (int, int, error) {
		d, ok := dims[path]
		if !ok {
			return 0, 0, errors.New("unknown path")
		}
		return d[0], d[1], nil
	}

	images := []string{"/out/thumb-1.png", "/out/thumb-2.png"}
	manifest, err := BuildThumbnailManifest("/in/deck.pptx", "/out", "png", 96, images, dimFn)
	if err != nil {
		t.Fatalf("BuildThumbnailManifest returned error: %v", err)
	}

	if manifest.SchemaVersion != ThumbnailManifestSchemaVersion {
		t.Errorf("schemaVersion = %d, want %d", manifest.SchemaVersion, ThumbnailManifestSchemaVersion)
	}
	if manifest.SourceFile != "/in/deck.pptx" {
		t.Errorf("sourceFile = %q", manifest.SourceFile)
	}
	if manifest.OutputDir != "/out" {
		t.Errorf("outputDir = %q", manifest.OutputDir)
	}
	if manifest.ImageFormat != "png" {
		t.Errorf("imageFormat = %q", manifest.ImageFormat)
	}
	if manifest.DPI != 96 {
		t.Errorf("dpi = %d", manifest.DPI)
	}
	if len(manifest.Thumbnails) != 2 {
		t.Fatalf("got %d thumbnails, want 2", len(manifest.Thumbnails))
	}

	cases := []struct {
		index  int
		path   string
		width  int
		height int
	}{
		{1, "/out/thumb-1.png", 800, 600},
		{2, "/out/thumb-2.png", 640, 480},
	}
	for i, want := range cases {
		got := manifest.Thumbnails[i]
		if got.Index != want.index || got.Path != want.path || got.Width != want.width || got.Height != want.height {
			t.Errorf("thumbnail[%d] = %+v, want %+v", i, got, want)
		}
	}
}

func TestBuildThumbnailManifestEmpty(t *testing.T) {
	manifest, err := BuildThumbnailManifest("/in/deck.pptx", "/out", "png", 96, nil, func(string) (int, int, error) {
		return 0, 0, errors.New("should not be called")
	})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if manifest.Thumbnails == nil {
		t.Error("thumbnails should be a non-nil empty slice for stable JSON")
	}
	if len(manifest.Thumbnails) != 0 {
		t.Errorf("got %d thumbnails, want 0", len(manifest.Thumbnails))
	}
}

func TestBuildThumbnailManifestDimError(t *testing.T) {
	dimFn := func(string) (int, int, error) { return 0, 0, errors.New("boom") }
	_, err := BuildThumbnailManifest("/in/deck.pptx", "/out", "png", 96, []string{"/out/thumb-1.png"}, dimFn)
	if err == nil {
		t.Fatal("expected error when dimFn fails")
	}
}
