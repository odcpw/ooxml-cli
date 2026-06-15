package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestRenderCommand_RequiresOut(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "deck.pptx")
	require.NoError(t, os.WriteFile(inputPath, []byte("dummy"), 0o644))

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "render", inputPath})
	err := cmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "out")
}

func TestRenderCommand_JSONManifest(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "deck.pptx")
	outDir := filepath.Join(t.TempDir(), "rendered")
	jsonPath := filepath.Join(t.TempDir(), "manifest.json")
	require.NoError(t, os.WriteFile(inputPath, []byte("dummy"), 0o644))

	origRender := renderToPDFFn
	origRaster := rasterizeFn
	defer func() {
		renderToPDFFn = origRender
		rasterizeFn = origRaster
	}()

	renderToPDFFn = func(pptxPath string, outDir string) (string, error) {
		require.NoError(t, os.MkdirAll(outDir, 0o755))
		pdfPath := filepath.Join(outDir, "deck.pdf")
		require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))
		return pdfPath, nil
	}
	rasterizeFn = func(pdfPath string, outDir string, opts pkgrender.RasterizeOptions) ([]string, error) {
		paths := []string{filepath.Join(outDir, "slide-1.png"), filepath.Join(outDir, "slide-2.png")}
		for _, path := range paths {
			require.NoError(t, os.WriteFile(path, []byte("img"), 0o644))
		}
		return paths, nil
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "render", inputPath, "--out", outDir, "--format", "json", "-o", jsonPath})
	require.NoError(t, cmd.Execute())

	manifestData, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var manifest pkgrender.Manifest
	require.NoError(t, json.Unmarshal(manifestData, &manifest))
	assert.Equal(t, inputPath, manifest.SourceFile)
	assert.Equal(t, filepath.Join(outDir, "deck.pdf"), manifest.PDFPath)
	require.Len(t, manifest.Slides, 2)

	writtenManifest, err := os.ReadFile(filepath.Join(outDir, "render-manifest.json"))
	require.NoError(t, err)
	assert.Contains(t, string(writtenManifest), "slide-1.png")
}

func TestRenderCommand_Thumbnails(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "deck.pptx")
	outDir := filepath.Join(t.TempDir(), "rendered")
	jsonPath := filepath.Join(t.TempDir(), "thumbs.json")
	require.NoError(t, os.WriteFile(inputPath, []byte("dummy"), 0o644))

	origRender := renderToPDFFn
	origRaster := rasterizeFn
	origDims := imageDimsFn
	defer func() {
		renderToPDFFn = origRender
		rasterizeFn = origRaster
		imageDimsFn = origDims
	}()

	renderToPDFFn = func(pptxPath string, outDir string) (string, error) {
		require.NoError(t, os.MkdirAll(outDir, 0o755))
		pdfPath := filepath.Join(outDir, "deck.pdf")
		require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))
		return pdfPath, nil
	}
	var capturedOpts pkgrender.RasterizeOptions
	rasterizeFn = func(pdfPath string, outDir string, opts pkgrender.RasterizeOptions) ([]string, error) {
		capturedOpts = opts
		paths := []string{filepath.Join(outDir, "thumb-1.png"), filepath.Join(outDir, "thumb-2.png")}
		for _, path := range paths {
			require.NoError(t, os.WriteFile(path, []byte("img"), 0o644))
		}
		return paths, nil
	}
	imageDimsFn = func(path string) (int, int, error) {
		return 320, 240, nil
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "render", inputPath, "--out", outDir, "--thumbnails", "--thumbnail-dpi", "72", "--format", "json", "-o", jsonPath})
	require.NoError(t, cmd.Execute())

	// Single rasterization pass at thumbnail DPI, PNG only.
	assert.Equal(t, 72, capturedOpts.DPI)
	assert.Equal(t, pkgrender.ImageFormatPNG, capturedOpts.Format)

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var manifest pkgrender.ThumbnailManifest
	require.NoError(t, json.Unmarshal(data, &manifest))
	assert.Equal(t, pkgrender.ThumbnailManifestSchemaVersion, manifest.SchemaVersion)
	assert.Equal(t, inputPath, manifest.SourceFile)
	assert.Equal(t, 72, manifest.DPI)
	require.Len(t, manifest.Thumbnails, 2)
	assert.Equal(t, 1, manifest.Thumbnails[0].Index)
	assert.Equal(t, filepath.Join(outDir, "thumb-1.png"), manifest.Thumbnails[0].Path)
	assert.Equal(t, 320, manifest.Thumbnails[0].Width)
	assert.Equal(t, 240, manifest.Thumbnails[0].Height)
	assert.Equal(t, 2, manifest.Thumbnails[1].Index)

	// Manifest also persisted to disk in the output directory.
	onDisk, err := os.ReadFile(filepath.Join(outDir, "thumbnails-manifest.json"))
	require.NoError(t, err)
	assert.Contains(t, string(onDisk), "thumb-1.png")
}

func TestRenderCommand_ThumbnailsMissingDependency(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "deck.pptx")
	outDir := filepath.Join(t.TempDir(), "rendered")
	require.NoError(t, os.WriteFile(inputPath, []byte("dummy"), 0o644))

	origRender := renderToPDFFn
	origRaster := rasterizeFn
	defer func() {
		renderToPDFFn = origRender
		rasterizeFn = origRaster
	}()

	renderToPDFFn = func(pptxPath string, outDir string) (string, error) {
		require.NoError(t, os.MkdirAll(outDir, 0o755))
		pdfPath := filepath.Join(outDir, "deck.pdf")
		require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))
		return pdfPath, nil
	}
	rasterizeFn = func(pdfPath string, outDir string, opts pkgrender.RasterizeOptions) ([]string, error) {
		return nil, &pkgrender.MissingDependencyError{Tool: "pdftoppm"}
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "render", inputPath, "--out", outDir, "--thumbnails"})
	err := cmd.Execute()
	require.Error(t, err)
	var cliErr *CLIError
	require.ErrorAs(t, err, &cliErr)
	assert.Equal(t, ExitRenderFailed, cliErr.ExitCode)
	assert.Contains(t, err.Error(), "pdftoppm")
	assert.Contains(t, err.Error(), "ooxml doctor")
}

func TestRenderCommand_ParsesSlidesFilter(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "deck.pptx")
	outDir := filepath.Join(t.TempDir(), "rendered")
	require.NoError(t, os.WriteFile(inputPath, []byte("dummy"), 0o644))

	origRender := renderToPDFFn
	origRaster := rasterizeFn
	defer func() {
		renderToPDFFn = origRender
		rasterizeFn = origRaster
	}()

	renderToPDFFn = func(pptxPath string, outDir string) (string, error) {
		require.NoError(t, os.MkdirAll(outDir, 0o755))
		pdfPath := filepath.Join(outDir, "deck.pdf")
		require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))
		return pdfPath, nil
	}
	var capturedPages []int
	rasterizeFn = func(pdfPath string, outDir string, opts pkgrender.RasterizeOptions) ([]string, error) {
		capturedPages = append([]int{}, opts.Pages...)
		path := filepath.Join(outDir, "slide-1.png")
		require.NoError(t, os.WriteFile(path, []byte("img"), 0o644))
		return []string{path}, nil
	}

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "render", inputPath, "--out", outDir, "--slides", "1,3-4"})
	require.NoError(t, cmd.Execute())
	assert.Equal(t, []int{1, 3, 4}, capturedPages)
}
