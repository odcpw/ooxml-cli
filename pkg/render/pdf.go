package render

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// ImageFormat is the raster output format.
type ImageFormat string

const (
	ImageFormatPNG ImageFormat = "png"
	ImageFormatJPG ImageFormat = "jpg"
)

// RasterizeOptions controls PDF->image rasterization.
type RasterizeOptions struct {
	Format ImageFormat
	DPI    int
	Pages  []int
	Prefix string
}

// RasterizeToImages rasterizes a PDF into slide images via pdftoppm.
func RasterizeToImages(pdfPath string, outDir string, opts RasterizeOptions) ([]string, error) {
	return NewTools().RasterizeToImages(pdfPath, outDir, opts)
}

// RasterizeToImages rasterizes a PDF into slide images via pdftoppm.
func (t *Tools) RasterizeToImages(pdfPath string, outDir string, opts RasterizeOptions) ([]string, error) {
	if t == nil {
		t = NewTools()
	}
	if t.Runner == nil {
		t.Runner = ExecRunner{}
	}
	if t.Timeout <= 0 {
		t.Timeout = defaultTimeout
	}
	if pdfPath == "" {
		return nil, fmt.Errorf("pdf path cannot be empty")
	}
	if outDir == "" {
		return nil, fmt.Errorf("output directory cannot be empty")
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return nil, fmt.Errorf("failed to create output directory: %w", err)
	}

	format, err := normalizeImageFormat(opts.Format)
	if err != nil {
		return nil, err
	}
	dpi := opts.DPI
	if dpi <= 0 {
		dpi = 144
	}
	prefix := opts.Prefix
	if prefix == "" {
		prefix = "slide"
	}

	if _, err := t.Runner.LookPath("pdftoppm"); err != nil {
		return nil, &MissingDependencyError{Tool: "pdftoppm"}
	}

	pages := uniqueSortedPages(opts.Pages)
	if len(pages) == 0 {
		pages = []int{0}
	}

	prefixPath := filepath.Join(outDir, prefix)
	generated := []string{}
	for _, page := range pages {
		ctx, cancel := context.WithTimeout(context.Background(), t.Timeout)
		args := []string{"-r", fmt.Sprintf("%d", dpi)}
		switch format {
		case ImageFormatPNG:
			args = append(args, "-png")
		case ImageFormatJPG:
			args = append(args, "-jpeg")
		}
		if page > 0 {
			args = append(args, "-f", fmt.Sprintf("%d", page), "-l", fmt.Sprintf("%d", page))
		}
		args = append(args, pdfPath, prefixPath)

		result, runErr := t.Runner.Run(ctx, "pdftoppm", args)
		cancel()
		if runErr != nil {
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				return nil, fmt.Errorf("pdftoppm rasterization timed out")
			}
			stderr := ""
			if result != nil {
				stderr = result.Stderr
			}
			return nil, &ToolFailureError{Tool: "pdftoppm", Args: args, Stderr: stderr, Cause: runErr}
		}
	}

	ext := ".png"
	if format == ImageFormatJPG {
		ext = ".jpg"
	}
	matches, err := filepath.Glob(filepath.Join(outDir, prefix+"-*"+ext))
	if err != nil {
		return nil, fmt.Errorf("failed to enumerate rasterized images: %w", err)
	}
	sort.Strings(matches)
	if len(matches) == 0 {
		return nil, fmt.Errorf("pdftoppm produced no %s images", strings.TrimPrefix(ext, "."))
	}
	generated = append(generated, matches...)
	return generated, nil
}

func normalizeImageFormat(format ImageFormat) (ImageFormat, error) {
	switch format {
	case "", ImageFormatPNG:
		return ImageFormatPNG, nil
	case ImageFormatJPG:
		return ImageFormatJPG, nil
	default:
		return "", fmt.Errorf("unsupported image format %q", format)
	}
}

func uniqueSortedPages(pages []int) []int {
	if len(pages) == 0 {
		return nil
	}
	seen := make(map[int]struct{}, len(pages))
	result := make([]int, 0, len(pages))
	for _, page := range pages {
		if page <= 0 {
			continue
		}
		if _, exists := seen[page]; exists {
			continue
		}
		seen[page] = struct{}{}
		result = append(result, page)
	}
	sort.Ints(result)
	return result
}
