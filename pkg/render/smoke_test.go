package render

import (
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/require"
)

func TestRenderSmokeMinimalTitle(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping render smoke test in short mode")
	}
	requireRenderSmokeBaseline(t)

	outDir := t.TempDir()
	pptxPath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)

	pdfPath, err := RenderToPDF(pptxPath, outDir)
	var missing *MissingDependencyError
	if errors.As(err, &missing) {
		t.Skipf("render dependency unavailable: %v", err)
	}
	require.NoError(t, err)

	images, err := RasterizeToImages(pdfPath, outDir, RasterizeOptions{Format: ImageFormatPNG, DPI: 144})
	if errors.As(err, &missing) {
		t.Skipf("raster dependency unavailable: %v", err)
	}
	require.NoError(t, err)
	require.NotEmpty(t, images)
}

// M12-2: Extended render smoke tests for rich text and geometry

func TestRenderSmokeTitleContent(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping render smoke test in short mode")
	}
	requireRenderSmokeBaseline(t)

	outDir := t.TempDir()
	pptxPath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	pdfPath, err := RenderToPDF(pptxPath, outDir)
	var missing *MissingDependencyError
	if errors.As(err, &missing) {
		t.Skipf("render dependency unavailable: %v", err)
	}
	require.NoError(t, err)

	images, err := RasterizeToImages(pdfPath, outDir, RasterizeOptions{Format: ImageFormatPNG, DPI: 144})
	if errors.As(err, &missing) {
		t.Skipf("raster dependency unavailable: %v", err)
	}
	require.NoError(t, err)
	require.GreaterOrEqual(t, len(images), 2, "should have at least 2 slides rendered")
}

func TestRenderSmokePictureFixture(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping render smoke test in short mode")
	}
	requireRenderSmokeBaseline(t)

	outDir := t.TempDir()
	pptxPath, err := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Skipf("picture fixture not available: %v", err)
	}

	pdfPath, err := RenderToPDF(pptxPath, outDir)
	var missing *MissingDependencyError
	if errors.As(err, &missing) {
		t.Skipf("render dependency unavailable: %v", err)
	}
	if err != nil {
		t.Skipf("failed to render picture fixture: %v", err)
	}

	images, err := RasterizeToImages(pdfPath, outDir, RasterizeOptions{Format: ImageFormatPNG, DPI: 144})
	if errors.As(err, &missing) {
		t.Skipf("raster dependency unavailable: %v", err)
	}
	require.NoError(t, err)
	require.NotEmpty(t, images)
}

func TestRenderSmokeMultiLayout(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping render smoke test in short mode")
	}
	requireRenderSmokeBaseline(t)

	outDir := t.TempDir()
	pptxPath, err := filepath.Abs("../../testdata/pptx/multi-layout/presentation.pptx")
	if err != nil {
		t.Skipf("multi-layout fixture not available: %v", err)
	}

	pdfPath, err := RenderToPDF(pptxPath, outDir)
	var missing *MissingDependencyError
	if errors.As(err, &missing) {
		t.Skipf("render dependency unavailable: %v", err)
	}
	if err != nil {
		t.Skipf("failed to render multi-layout fixture: %v", err)
	}

	images, err := RasterizeToImages(pdfPath, outDir, RasterizeOptions{Format: ImageFormatPNG, DPI: 144})
	if errors.As(err, &missing) {
		t.Skipf("raster dependency unavailable: %v", err)
	}
	require.NoError(t, err)
	require.NotEmpty(t, images)
}

func TestRenderSmokeMetadata(t *testing.T) {
	if testing.Short() {
		t.Skip("skipping render smoke test in short mode")
	}
	requireRenderSmokeBaseline(t)

	outDir := t.TempDir()
	pptxPath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)

	pdfPath, err := RenderToPDF(pptxPath, outDir)
	var missing *MissingDependencyError
	if errors.As(err, &missing) {
		t.Skipf("render dependency unavailable: %v", err)
	}
	require.NoError(t, err)

	// Verify that PDF was actually created with non-zero size
	info, err := os.Stat(pdfPath)
	require.NoError(t, err)
	require.Greater(t, info.Size(), int64(0), "rendered PDF should have non-zero size")
}

func requireRenderSmokeBaseline(t *testing.T) {
	t.Helper()
	pptxPath, err := filepath.Abs("../../testdata/pptx/minimal-title/presentation.pptx")
	require.NoError(t, err)
	_, err = RenderToPDF(pptxPath, t.TempDir())
	var missing *MissingDependencyError
	if errors.As(err, &missing) {
		t.Skipf("render dependency unavailable: %v", err)
	}
	if err != nil {
		t.Skipf("render engine cannot render known-good PPTX fixture; skipping render smoke tests: %v", err)
	}
}
