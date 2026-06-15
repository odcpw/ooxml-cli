package render

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

type fakeRunner struct {
	lookups map[string]error
	runFn   func(ctx context.Context, name string, args []string) (*RunResult, error)
}

func (f *fakeRunner) LookPath(name string) (string, error) {
	if err, ok := f.lookups[name]; ok {
		return "", err
	}
	return "/usr/bin/" + name, nil
}

func (f *fakeRunner) Run(ctx context.Context, name string, args []string) (*RunResult, error) {
	if f.runFn == nil {
		return &RunResult{}, nil
	}
	return f.runFn(ctx, name, args)
}

func TestRenderToPDF_MissingTool(t *testing.T) {
	tools := &Tools{
		Runner: &fakeRunner{lookups: map[string]error{
			"soffice":     errors.New("missing"),
			"libreoffice": errors.New("missing"),
		}},
	}

	_, err := tools.RenderToPDF("deck.pptx", t.TempDir())
	require.Error(t, err)
	var missing *MissingDependencyError
	assert.ErrorAs(t, err, &missing)
}

func TestRenderToPDF_Success(t *testing.T) {
	outDir := t.TempDir()
	pptxPath := filepath.Join(t.TempDir(), "deck.pptx")
	require.NoError(t, os.WriteFile(pptxPath, []byte("fake"), 0o644))

	tools := &Tools{
		Runner: &fakeRunner{runFn: func(ctx context.Context, name string, args []string) (*RunResult, error) {
			pdfPath := filepath.Join(argAfter(t, args, "--outdir"), "deck.pdf")
			require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))
			return &RunResult{}, nil
		}},
	}

	pdfPath, err := tools.RenderToPDF(pptxPath, outDir)
	require.NoError(t, err)
	assert.Equal(t, filepath.Join(outDir, "deck.pdf"), pdfPath)
}

func TestRenderToPDF_SurfacesSofficeDiagnosticsWhenNoPDFIsProduced(t *testing.T) {
	outDir := t.TempDir()
	pptxPath := filepath.Join(t.TempDir(), "deck.pptx")
	require.NoError(t, os.WriteFile(pptxPath, []byte("fake"), 0o644))

	tools := &Tools{
		Runner: &fakeRunner{runFn: func(ctx context.Context, name string, args []string) (*RunResult, error) {
			return &RunResult{Stderr: "Error: source file could not be loaded"}, nil
		}},
	}

	_, err := tools.RenderToPDF(pptxPath, outDir)
	require.Error(t, err)
	var toolFailure *ToolFailureError
	assert.ErrorAs(t, err, &toolFailure)
	assert.Contains(t, err.Error(), "source file could not be loaded")
}

func TestRasterizeToImages_MissingTool(t *testing.T) {
	tools := &Tools{Runner: &fakeRunner{lookups: map[string]error{"pdftoppm": errors.New("missing")}}}
	_, err := tools.RasterizeToImages("deck.pdf", t.TempDir(), RasterizeOptions{})
	require.Error(t, err)
	var missing *MissingDependencyError
	assert.ErrorAs(t, err, &missing)
}

func TestRasterizeToImages_PageSelection(t *testing.T) {
	outDir := t.TempDir()
	pdfPath := filepath.Join(t.TempDir(), "deck.pdf")
	require.NoError(t, os.WriteFile(pdfPath, []byte("pdf"), 0o644))

	tools := &Tools{
		Runner: &fakeRunner{runFn: func(ctx context.Context, name string, args []string) (*RunResult, error) {
			prefix := args[len(args)-1]
			ext := ".png"
			if contains(args, "-jpeg") {
				ext = ".jpg"
			}
			page := 1
			if idx := indexOf(args, "-f"); idx >= 0 {
				parsed, err := strconv.Atoi(args[idx+1])
				require.NoError(t, err)
				page = parsed
			}
			imagePath := prefix + "-" + strconv.Itoa(page) + ext
			require.NoError(t, os.WriteFile(imagePath, []byte("img"), 0o644))
			return &RunResult{}, nil
		}},
	}

	images, err := tools.RasterizeToImages(pdfPath, outDir, RasterizeOptions{Format: ImageFormatPNG, DPI: 110, Pages: []int{3, 1, 3}})
	require.NoError(t, err)
	require.Len(t, images, 2)
	assert.Equal(t, filepath.Join(outDir, "slide-1.png"), images[0])
	assert.Equal(t, filepath.Join(outDir, "slide-3.png"), images[1])
}

func TestRasterizeToImages_InvalidFormat(t *testing.T) {
	tools := &Tools{Runner: &fakeRunner{}}
	_, err := tools.RasterizeToImages("deck.pdf", t.TempDir(), RasterizeOptions{Format: "gif"})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "unsupported image format")
}

func contains(values []string, want string) bool {
	return indexOf(values, want) >= 0
}

func indexOf(values []string, want string) int {
	for i, value := range values {
		if value == want {
			return i
		}
	}
	return -1
}

func argAfter(t *testing.T, values []string, want string) string {
	t.Helper()
	idx := indexOf(values, want)
	if idx < 0 || idx+1 >= len(values) {
		t.Fatalf("missing %s argument in %v", want, values)
	}
	return values[idx+1]
}
