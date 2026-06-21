package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
)

var (
	renderSlidesArg   string
	renderImageFormat string
	renderDPI         int
	renderThumbnails  bool
	renderThumbDPI    int
)

var (
	renderToPDFFn = pkgrender.RenderToPDF
	rasterizeFn   = pkgrender.RasterizeToImages
	imageDimsFn   = pkgrender.ReadImageDimensions
)

var renderCmd = &cobra.Command{
	Use:   "render <file>",
	Short: "Render a PPTX presentation to PDF and images",
	Long: "Render a PPTX presentation to PDF and images.\n\n" +
		"With --thumbnails, per-slide PNG files are written to --out and a JSON\n" +
		"manifest ([{index,path,width,height}]) is emitted to stdout so a\n" +
		"multimodal agent can load and view each rendered slide. Requires\n" +
		"LibreOffice (soffice) and pdftoppm; run `ooxml doctor` if rendering is\n" +
		"unavailable. DOCX/XLSX thumbnail support is future work.",
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		inputPath := args[0]
		if _, err := os.Stat(inputPath); err != nil {
			return FileNotFoundError(inputPath)
		}

		outDir, err := cmd.Flags().GetString("out")
		if err != nil {
			return err
		}
		if outDir == "" {
			return InvalidArgsError("--out is required")
		}
		pages, err := parseRenderSlides(renderSlidesArg)
		if err != nil {
			return InvalidArgsError(err.Error())
		}

		pdfPath, err := renderToPDFFn(inputPath, outDir)
		if err != nil {
			return mapRenderError(err)
		}

		if renderThumbnails {
			return runThumbnails(cmd, inputPath, outDir, pdfPath, pages)
		}

		images, err := rasterizeFn(pdfPath, outDir, pkgrender.RasterizeOptions{
			Format: pkgrender.ImageFormat(renderImageFormat),
			DPI:    renderDPI,
			Pages:  pages,
			Prefix: "slide",
		})
		if err != nil {
			return mapRenderError(err)
		}

		manifest := buildRenderManifest(inputPath, outDir, pdfPath, renderImageFormat, renderDPI, images)
		manifestPath := filepath.Join(outDir, "render-manifest.json")
		if err := writeManifestFile(manifestPath, manifest); err != nil {
			return err
		}

		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			return outputRenderManifestJSON(cmd, manifest)
		}
		return outputRenderManifestText(cmd, manifest)
	},
}

func buildRenderManifest(inputPath, outDir, pdfPath, imageFormat string, dpi int, images []string) *pkgrender.Manifest {
	manifest := &pkgrender.Manifest{
		SourceFile:  inputPath,
		OutputDir:   outDir,
		PDFPath:     pdfPath,
		ImageFormat: imageFormat,
		DPI:         dpi,
		Slides:      make([]pkgrender.RenderedSlide, 0, len(images)),
	}
	for idx, imagePath := range images {
		manifest.Slides = append(manifest.Slides, pkgrender.RenderedSlide{Slide: idx + 1, ImagePath: imagePath})
	}
	return manifest
}

// runThumbnails rasterizes each slide to a PNG in outDir and emits a thumbnail
// manifest ([{index,path,width,height}]) to stdout so a multimodal agent can
// load and view the rendered slides. PNG bytes are written to disk, not
// embedded, to keep the JSON payload small.
func runThumbnails(cmd *cobra.Command, inputPath, outDir, pdfPath string, pages []int) error {
	images, err := rasterizeFn(pdfPath, outDir, pkgrender.RasterizeOptions{
		Format: pkgrender.ImageFormatPNG,
		DPI:    renderThumbDPI,
		Pages:  pages,
		Prefix: "thumb",
	})
	if err != nil {
		return mapThumbnailError(err)
	}

	manifest, err := pkgrender.BuildThumbnailManifest(inputPath, outDir, string(pkgrender.ImageFormatPNG), renderThumbDPI, images, imageDimsFn)
	if err != nil {
		return NewCLIErrorf(ExitRenderFailed, "failed to build thumbnail manifest: %v", err)
	}

	manifestPath := filepath.Join(outDir, "thumbnails-manifest.json")
	if err := writeThumbnailManifestFile(manifestPath, manifest); err != nil {
		return err
	}

	return outputThumbnailManifestJSON(cmd, manifest)
}

func writeThumbnailManifestFile(path string, manifest *pkgrender.ThumbnailManifest) error {
	data, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal thumbnail manifest: %v", err)
	}
	if err := os.WriteFile(path, append(data, '\n'), 0o644); err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to write thumbnail manifest: %v", err)
	}
	return nil
}

func outputThumbnailManifestJSON(cmd *cobra.Command, manifest *pkgrender.ThumbnailManifest) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(manifest, "", "  ")
	} else {
		data, err = json.Marshal(manifest)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal thumbnail manifest: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

// mapThumbnailError maps render-tool errors to a render-failed CLI error and
// points the agent at `ooxml doctor` for remediation when a tool is missing.
func mapThumbnailError(err error) error {
	var missing *pkgrender.MissingDependencyError
	var toolFailure *pkgrender.ToolFailureError
	switch {
	case errors.As(err, &missing):
		return RenderFailedError(fmt.Sprintf("%s; run `ooxml doctor` to diagnose render dependencies", missing.Error()))
	case errors.As(err, &toolFailure):
		return RenderFailedError(fmt.Sprintf("%s; run `ooxml doctor` to diagnose render dependencies", toolFailure.Error()))
	default:
		return NewCLIErrorf(ExitUnexpected, "thumbnail render failed: %v", err)
	}
}

func writeManifestFile(path string, manifest *pkgrender.Manifest) error {
	data, err := json.MarshalIndent(manifest, "", "  ")
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal render manifest: %v", err)
	}
	if err := os.WriteFile(path, append(data, '\n'), 0o644); err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to write render manifest: %v", err)
	}
	return nil
}

func outputRenderManifestJSON(cmd *cobra.Command, manifest *pkgrender.Manifest) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(manifest, "", "  ")
	} else {
		data, err = json.Marshal(manifest)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal render manifest: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputRenderManifestText(cmd *cobra.Command, manifest *pkgrender.Manifest) error {
	text := fmt.Sprintf("PDF: %s\nImages: %d\n", manifest.PDFPath, len(manifest.Slides))
	return writeCLIOutput(cmd, []byte(text))
}

func writeCLIOutput(cmd *cobra.Command, data []byte) error {
	config := GetGlobalConfig(cmd)
	var out io.Writer = cmd.OutOrStdout()
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		out = file
	}
	if _, err := out.Write(data); err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to write output: %v", err)
	}
	if config.Output == "" && len(data) > 0 && data[len(data)-1] != '\n' {
		_, _ = out.Write([]byte("\n"))
	}
	return nil
}

func parseRenderSlides(value string) ([]int, error) {
	if value == "" {
		return nil, nil
	}
	selector, err := selectors.Parse(value)
	if err != nil {
		return nil, err
	}
	pages := []int{}
	switch sel := selector.(type) {
	case *selectors.SlideNumberSelector:
		pages = append(pages, sel.Number)
	case *selectors.SlideRangeSelector:
		for _, r := range sel.Ranges {
			for page := r.Start; page <= r.End; page++ {
				pages = append(pages, page)
			}
		}
	default:
		return nil, fmt.Errorf("invalid slide selector %q", value)
	}
	sort.Ints(pages)
	return pages, nil
}

func mapRenderError(err error) error {
	var missing *pkgrender.MissingDependencyError
	var toolFailure *pkgrender.ToolFailureError
	switch {
	case errors.As(err, &missing):
		return RenderFailedError(missing.Error())
	case errors.As(err, &toolFailure):
		return RenderFailedError(toolFailure.Error())
	default:
		return NewCLIErrorf(ExitUnexpected, "render failed: %v", err)
	}
}

func init() {
	renderCmd.Flags().StringVar(&renderSlidesArg, "slides", "", "slide selector (e.g. 1,3-5)")
	renderCmd.Flags().StringVar(&renderImageFormat, "image-format", "png", "rendered image format: png or jpg")
	renderCmd.Flags().IntVar(&renderDPI, "dpi", 144, "rendered image DPI")
	renderCmd.Flags().BoolVar(&renderThumbnails, "thumbnails", false, "emit per-slide PNG thumbnails plus a JSON manifest [{index,path,width,height}] for multimodal agents")
	renderCmd.Flags().IntVar(&renderThumbDPI, "thumbnail-dpi", 96, "DPI for thumbnail rasterization (used with --thumbnails)")
	renderCmd.Flags().String("out", "", "output directory for rendered artifacts")
	renderCmd.MarkFlagRequired("out")
	pptxCmd.AddCommand(renderCmd)
}
