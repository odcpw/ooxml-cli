package cli

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

var (
	extractImagesSlide         int
	extractImagesOut           string
	extractImagesIncludeLayout bool
)

var extractImagesCmd = &cobra.Command{
	Use:   "images <file>",
	Short: "Extract images from a presentation",
	Long: `Extract images from a PPTX presentation and save them to a directory.

Usage:
  ooxml pptx extract images <file> [flags]

Flags:
  --out <dir>           Output directory for extracted images (default: current directory)
  --slide <n>           Extract images from a specific slide (1-indexed)
  --include-layout-images  Include images from slide layouts in addition to slide images

Output:
  - Extracted image files in the output directory
  - A manifest.json file with metadata about each extracted image`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Determine output directory
		outDir := extractImagesOut
		if outDir == "" {
			outDir = "."
		}

		// Create output directory if it doesn't exist
		if err := os.MkdirAll(outDir, 0755); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output directory: %v", err)
		}

		// Open the package
		session, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer session.Close()

		// Parse presentation
		graph, err := inspect.ParsePresentation(session)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Determine which slides to process
		var slidesToProcess []int
		if extractImagesSlide > 0 {
			if extractImagesSlide < 1 || extractImagesSlide > len(graph.Slides) {
				return NewCLIErrorf(ExitInvalidArgs, "slide number %d is out of range (1-%d)", extractImagesSlide, len(graph.Slides))
			}
			slidesToProcess = append(slidesToProcess, extractImagesSlide)
		} else {
			// Process all slides
			for _, slideRef := range graph.Slides {
				slidesToProcess = append(slidesToProcess, slideRef.SlideNumber)
			}
		}

		// Extract images
		var extractedImages []model.ExtractedImageInfo
		fileCounter := make(map[string]int) // Track duplicate filenames

		for _, slideNumber := range slidesToProcess {
			slideRef := graph.Slides[slideNumber-1]

			// Read slide XML
			slideDoc, err := session.ReadXMLPart(slideRef.PartURI)
			if err != nil {
				continue
			}

			// Get shapes from slide
			spTree := slideDoc.FindElement(".//spTree")
			if spTree != nil {
				// Extract images from slide
				images := inspect.EnumerateImageRelationships(slideRef.PartURI, session, spTree)
				extractedImages = append(extractedImages, images...)
			}

			// Extract images from layout if requested
			if extractImagesIncludeLayout && slideRef.LayoutPartURI != "" {
				layoutDoc, err := session.ReadXMLPart(slideRef.LayoutPartURI)
				if err == nil {
					layoutSpTree := layoutDoc.FindElement(".//spTree")
					if layoutSpTree != nil {
						images := inspect.EnumerateImageRelationships(slideRef.LayoutPartURI, session, layoutSpTree)
						for i := range images {
							images[i].IsLayoutImage = true
						}
						extractedImages = append(extractedImages, images...)
					}
				}
			}
		}

		// Write extracted images to disk
		filepathMap := make(map[string]string) // Maps extracted file path to unique filename
		for i := range extractedImages {
			img := &extractedImages[i]

			// Read the image data
			data, err := session.ReadRawPart(img.TargetURI)
			if err != nil {
				continue
			}

			// Determine output filename (handle duplicates)
			filename := filepath.Base(img.TargetURI)
			if count, exists := fileCounter[filename]; exists {
				// Duplicate filename, add counter
				ext := filepath.Ext(filename)
				base := filename[:len(filename)-len(ext)]
				filename = fmt.Sprintf("%s_%d%s", base, count, ext)
				fileCounter[filename] = count + 1
			} else {
				fileCounter[filename] = 1
			}

			// Write file
			outPath := filepath.Join(outDir, filename)
			if err := os.WriteFile(outPath, data, 0644); err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to write image file: %v", err)
			}

			img.FilePath = filename
			filepathMap[img.TargetURI] = filename
		}

		// Create manifest
		manifest := &model.ExtractImagesManifest{
			File:            filePath,
			SlideNumber:     extractImagesSlide,
			OutputDirectory: outDir,
			IncludeLayout:   extractImagesIncludeLayout,
			ImagesCount:     len(extractedImages),
			Images:          extractedImages,
		}

		// Write manifest
		if config.Format == "json" {
			return outputExtractImagesJSON(cmd, manifest)
		}

		// Default to text output with manifest
		return outputExtractImagesText(cmd, manifest)
	},
}

// outputExtractImagesJSON outputs the extract images result in JSON format
func outputExtractImagesJSON(cmd *cobra.Command, manifest *model.ExtractImagesManifest) error {
	config := GetGlobalConfig(cmd)

	var jsonData []byte
	var err error
	if config.Pretty {
		jsonData, err = json.MarshalIndent(manifest, "", "  ")
	} else {
		jsonData, err = json.Marshal(manifest)
	}

	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	fmt.Fprintf(outFile, "%s\n", string(jsonData))
	return nil
}

// outputExtractImagesText outputs the extract images result in text format
func outputExtractImagesText(cmd *cobra.Command, manifest *model.ExtractImagesManifest) error {
	config := GetGlobalConfig(cmd)

	var outFile io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		outFile = file
	} else {
		outFile = cmd.OutOrStdout()
	}

	// Print header
	fmt.Fprintf(outFile, "Extracted %d image(s) to: %s\n\n", manifest.ImagesCount, manifest.OutputDirectory)

	if manifest.ImagesCount == 0 {
		fmt.Fprintf(outFile, "No images found.\n")
		return nil
	}

	// Print image details
	fmt.Fprintf(outFile, "Images:\n")
	for i, img := range manifest.Images {
		fmt.Fprintf(outFile, "  [%d] %s (from %s, shape ID %d)\n",
			i+1, img.FilePath, img.ShapeName, img.ShapeID)
		fmt.Fprintf(outFile, "      Source: %s\n", img.SourcePartURI)
		fmt.Fprintf(outFile, "      Size: %d bytes\n", img.FileSize)
		if img.IsLayoutImage {
			fmt.Fprintf(outFile, "      Source: Layout\n")
		}
		if img.Geometry != nil {
			fmt.Fprintf(outFile, "      Geometry:\n")
			if img.Geometry.Rotation != 0 {
				degrees := float64(img.Geometry.Rotation) / 60000.0
				fmt.Fprintf(outFile, "        Rotation: %v°\n", degrees)
			}
			if img.Geometry.FlipH {
				fmt.Fprintf(outFile, "        Flip H: true\n")
			}
			if img.Geometry.FlipV {
				fmt.Fprintf(outFile, "        Flip V: true\n")
			}
			if img.Geometry.Crop != nil {
				fmt.Fprintf(outFile, "        Crop: L=%d T=%d R=%d B=%d\n",
					img.Geometry.Crop.Left, img.Geometry.Crop.Top,
					img.Geometry.Crop.Right, img.Geometry.Crop.Bottom)
			}
		}
	}

	return nil
}

// init registers the extract images command
func init() {
	extractImagesCmd.Flags().StringVarP(
		&extractImagesOut,
		"out",
		"",
		"",
		"output directory for extracted images (default: current directory)",
	)

	extractImagesCmd.Flags().IntVar(
		&extractImagesSlide,
		"slide",
		0,
		"extract images from a specific slide (1-indexed, default: all slides)",
	)

	extractImagesCmd.Flags().BoolVar(
		&extractImagesIncludeLayout,
		"include-layout-images",
		false,
		"include images from slide layouts in addition to slide images",
	)

	extractCmd.AddCommand(extractImagesCmd)
}
