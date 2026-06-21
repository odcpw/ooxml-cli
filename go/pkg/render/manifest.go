package render

import "fmt"

// ThumbnailManifestSchemaVersion is the stable contract version for the
// thumbnail manifest emitted by `render --thumbnails`.
const ThumbnailManifestSchemaVersion = 1

// ThumbnailManifest is the agent-facing readback for `render --thumbnails`.
// It lists per-slide PNG files written to the output directory so a
// multimodal agent can load and view each rendered slide. PNG bytes are
// deliberately not embedded; the manifest carries file paths to keep the
// JSON payload small.
type ThumbnailManifest struct {
	SchemaVersion int         `json:"schemaVersion"`
	SourceFile    string      `json:"sourceFile"`
	OutputDir     string      `json:"outputDir"`
	ImageFormat   string      `json:"imageFormat"`
	DPI           int         `json:"dpi"`
	Thumbnails    []Thumbnail `json:"thumbnails"`
}

// Thumbnail describes one rendered slide image on disk.
type Thumbnail struct {
	Index  int    `json:"index"`
	Path   string `json:"path"`
	Width  int    `json:"width"`
	Height int    `json:"height"`
}

// ImageDimensions reports the pixel dimensions of an image file.
type ImageDimensions func(path string) (width int, height int, err error)

// BuildThumbnailManifest assembles a ThumbnailManifest from rasterized image
// paths. Dimensions are read via dimFn, which is injectable to keep this pure
// and testable. Indices are 1-based to match the slide numbering used by the
// render manifest.
func BuildThumbnailManifest(sourceFile, outputDir, imageFormat string, dpi int, images []string, dimFn ImageDimensions) (*ThumbnailManifest, error) {
	manifest := &ThumbnailManifest{
		SchemaVersion: ThumbnailManifestSchemaVersion,
		SourceFile:    sourceFile,
		OutputDir:     outputDir,
		ImageFormat:   imageFormat,
		DPI:           dpi,
		Thumbnails:    make([]Thumbnail, 0, len(images)),
	}
	for idx, path := range images {
		width, height, err := dimFn(path)
		if err != nil {
			return nil, fmt.Errorf("failed to read dimensions for %s: %w", path, err)
		}
		manifest.Thumbnails = append(manifest.Thumbnails, Thumbnail{
			Index:  idx + 1,
			Path:   path,
			Width:  width,
			Height: height,
		})
	}
	return manifest, nil
}

// Manifest describes the artifacts emitted by a render operation.
type Manifest struct {
	SourceFile  string          `json:"sourceFile"`
	OutputDir   string          `json:"outputDir"`
	PDFPath     string          `json:"pdfPath"`
	ImageFormat string          `json:"imageFormat"`
	DPI         int             `json:"dpi"`
	Slides      []RenderedSlide `json:"slides"`
}

// RenderedSlide describes one rasterized slide artifact.
type RenderedSlide struct {
	Slide     int    `json:"slide"`
	ImagePath string `json:"imagePath"`
}
