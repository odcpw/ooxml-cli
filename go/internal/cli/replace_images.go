package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

var (
	replaceImageTarget    string
	replaceImageFile      string
	replaceImageFitMode   string
	replaceImageSlide     int
	replaceImageForSlides string
)

var replaceImagesCmd = &cobra.Command{
	Use:     "images <file>",
	Aliases: []string{"image"},
	Short:   "Replace images in a presentation",
	Long: `Replace an image in a PPTX presentation with a new image file.

Usage:
  ooxml pptx replace images <file> --target <selector-or-handle> --image <path> [--slide <n> | --for-slides <spec>] [--fit-mode <mode>] [--out <output>] [--in-place] [--backup <backup>]

Target Selectors:
  shape:<id>            - Replace image in shape with ID <id>
  ~<name>              - Replace image in shape with name <name>
  H:pptx/s:<sldId>/shape:n:<id>
                       - Replace image by stable shape handle; supplies slide scope

Slide Targeting:
  --slide <n>          Single slide (1-based number)
  --for-slides <spec>  Multiple slides: "1,3,5-7" (ranges and lists supported)
  --slide or --for-slides is required unless --target is a stable shape handle.
  Shape handles cannot be combined with --slide or --for-slides.

Fit Modes:
  contain (default)    - Fit the image within the shape bounds (stretch to fit)
  cover                - Tile the image to cover the shape (crop as needed)

Output Options:
  --out <path>         - Write to output file (mutually exclusive with --in-place)
  --in-place           - Modify the input file directly (mutually exclusive with --out)
  --backup <path>      - Create backup when using --in-place (optional)

Flags:
  --no-validate        - Skip validation after writing (default: validate)`,
	Args: cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]

		// Check if file exists
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}

		// Validate mutation flags
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}

		// Validate target selector
		if replaceImageTarget == "" {
			return InvalidArgsError("--target must be specified")
		}

		// Validate image file
		if replaceImageFile == "" {
			return InvalidArgsError("--image must be specified")
		}

		if _, err := os.Stat(replaceImageFile); err != nil {
			return FileNotFoundError(replaceImageFile)
		}

		// --target additionally accepts a stable shape handle
		// (H:pptx/s:<sldId>/shape:n:<id>). When supplied the handle's sldId
		// selects the slide (--slide / --for-slides are rejected with it).
		if pptxhandle.IsHandle(replaceImageTarget) {
			h, herr := pptxhandle.Parse(replaceImageTarget)
			if herr != nil {
				return mapPPTXHandleError(herr)
			}
			if cmd.Flags().Lookup("slide").Changed || cmd.Flags().Lookup("for-slides").Changed {
				return InvalidArgsError("--slide / --for-slides cannot be combined with a handle target")
			}
			fitMode, ferr := mutate.ParseFitMode(replaceImageFitMode)
			if ferr != nil {
				return InvalidArgsError(ferr.Error())
			}
			imageData, ierr := os.ReadFile(replaceImageFile)
			if ierr != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read image file: %v", ierr)
			}
			contentType, cerr := getImageContentType(replaceImageFile)
			if cerr != nil {
				return cerr
			}
			result, rerr := performReplaceImageHandle(filePath, replaceImageTarget, h, imageData, contentType, fitMode, mutOpts)
			if rerr != nil {
				return rerr
			}
			config := GetGlobalConfig(cmd)
			if config.Format == "json" {
				return outputReplaceImageJSON(cmd, result)
			}
			return outputReplaceImageText(cmd, result)
		}

		// Parse selector
		selector, err := selectors.Parse(replaceImageTarget)
		if err != nil {
			return InvalidArgsError(fmt.Sprintf("invalid target selector: %v", err))
		}

		// Parse fit mode
		fitMode, err := mutate.ParseFitMode(replaceImageFitMode)
		if err != nil {
			return InvalidArgsError(err.Error())
		}

		// Get global config
		config := GetGlobalConfig(cmd)

		// Read the new image data
		imageData, err := os.ReadFile(replaceImageFile)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read image file: %v", err)
		}

		// Determine content type based on file extension
		contentType, err := getImageContentType(replaceImageFile)
		if err != nil {
			return err
		}

		// Check slide targeting options
		slideSpecified := cmd.Flags().Lookup("slide").Changed
		forSlidesSpecified := cmd.Flags().Lookup("for-slides").Changed

		if slideSpecified && forSlidesSpecified {
			return InvalidArgsError("cannot specify both --slide and --for-slides")
		}

		// Batch operation
		if forSlidesSpecified {
			return performBatchReplaceImage(cmd, filePath, replaceImageForSlides, selector, imageData, contentType, fitMode, mutOpts)
		}

		// Single-slide or all-slides operation (default to all slides if no slide specified)
		result, err := performReplaceImage(filePath, replaceImageTarget, selector, imageData, contentType, fitMode, replaceImageSlide, slideSpecified, mutOpts)
		if err != nil {
			return err
		}

		// Output the result
		if config.Format == "json" {
			return outputReplaceImageJSON(cmd, result)
		}

		return outputReplaceImageText(cmd, result)
	},
}

// performReplaceImage performs the image replacement mutation
func performReplaceImage(
	filePath string,
	targetSelector string,
	selector selectors.Selector,
	imageData []byte,
	contentType string,
	fitMode mutate.FitMode,
	slideNumber int,
	slideSpecified bool,
	mutOpts *MutationOptions,
) (*replaceImageResult, error) {
	// Create mutation writer
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *replaceImageResult
	var destination *PPTXShapeDestination
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)

	// Perform the mutation
	err = writer.Write(func(pkg opc.PackageSession) error {
		// Parse presentation to get slide references
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}

		if slideSpecified && (slideNumber < 1 || slideNumber > len(graph.Slides)) {
			return fmt.Errorf("invalid slide number %d (presentation has %d slides)", slideNumber, len(graph.Slides))
		}

		var found bool

		for slideIdx, slideRef := range graph.Slides {
			if slideSpecified && slideIdx+1 != slideNumber {
				continue
			}
			// Try to replace the image on this slide
			opts := mutate.ImageReplaceOptions{
				FitMode:             fitMode,
				NewImageData:        imageData,
				NewImageContentType: contentType,
			}

			replaceResult, err := mutate.ReplaceImage(selector, &slideRef, pkg, opts)
			if err != nil {
				if isReplaceImageSearchMiss(err) {
					continue
				}
				return err
			}

			// Success! Store the result
			found = true
			destination, err = collectPPTXShapeDestination(pkg, slideIdx+1, targetSelector, destinationFile, false, true)
			if err != nil {
				return err
			}
			result = &replaceImageResult{
				File:           filePath,
				Output:         destinationFile,
				DryRun:         mutOpts.DryRun,
				Target:         targetSelector,
				FitMode:        string(fitMode),
				SlideNumber:    slideIdx + 1,
				ShapeID:        replaceResult.ShapeID,
				ShapeName:      replaceResult.ShapeName,
				OldTargetURI:   replaceResult.OldTargetURI,
				OldContentType: replaceResult.OldContentType,
				NewTargetURI:   replaceResult.NewTargetURI,
				NewContentType: replaceResult.NewContentType,
				RelID:          replaceResult.RelationshipID,
				Destination:    destination,
			}
			result.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, false, true)
			break
		}

		if !found {
			return replaceImageTargetNotFoundError(pkg, graph, targetSelector, slideNumber, slideSpecified)
		}

		return nil
	})

	if err != nil {
		return nil, mutationWriteError(err, "failed to replace image")
	}

	return result, nil
}

func replaceImageTargetNotFoundError(pkg opc.PackageSession, graph *inspect.PresentationGraph, targetSelector string, slideNumber int, slideSpecified bool) error {
	if slideSpecified {
		catalog, err := selectors.BuildSlideCatalog(pkg, slideNumber)
		if err == nil {
			candidates := BuildSelectorCandidates(replaceImageSelectorCandidates(catalog), targetSelector, maxSelectorCandidates)
			discovery := fmt.Sprintf("ooxml --json pptx shapes show <file> --slide %d", slideNumber)
			return SelectorNotFoundError("picture shape", targetSelector, candidates, discovery)
		}
		return TargetNotFoundError(fmt.Sprintf("picture shape not found: %s on slide %d", targetSelector, slideNumber))
	}

	var candidates []SelectorCandidate
	if graph != nil {
		for _, slide := range graph.Slides {
			catalog, err := selectors.BuildSlideCatalog(pkg, slide.SlideNumber)
			if err != nil {
				continue
			}
			candidates = append(candidates, replaceImageSelectorCandidates(catalog)...)
		}
	}
	discovery := "ooxml --json pptx slides show <file> --include-bounds"
	return SelectorNotFoundError("picture shape", targetSelector, BuildSelectorCandidates(candidates, targetSelector, maxSelectorCandidates), discovery)
}

func replaceImageSelectorCandidates(catalog *selectors.SlideCatalog) []SelectorCandidate {
	if catalog == nil {
		return nil
	}
	out := make([]SelectorCandidate, 0, len(catalog.Targets))
	for _, target := range catalog.Targets {
		if target.TargetKind != "picture" {
			continue
		}
		out = append(out, SelectorCandidate{Primary: target.PrimarySelector, Selectors: target.Selectors})
	}
	return out
}

// performReplaceImageHandle replaces an image addressed by a stable shape
// handle. The handle's sldId selects the slide (by SEARCH, surviving slide
// reorder/insert/delete) and its native cNvPr id selects the shape within that
// slide (by SEARCH, surviving shape reorder/insert/delete).
func performReplaceImageHandle(
	filePath string,
	targetSelector string,
	h pptxhandle.Handle,
	imageData []byte,
	contentType string,
	fitMode mutate.FitMode,
	mutOpts *MutationOptions,
) (*replaceImageResult, error) {
	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return nil, err
	}

	var result *replaceImageResult
	var destination *PPTXShapeDestination
	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	selector := &selectors.ShapeIDSelector{ID: h.ShapeID}

	err = writer.Write(func(pkg opc.PackageSession) error {
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return fmt.Errorf("failed to parse presentation: %w", err)
		}
		// Route through the shared scope resolver so a duplicate sldId errors
		// CodeAmbiguous instead of silently first-winning on a duplicate.
		resolvedRef, rerr := selectors.ResolveSlideRefForHandle(graph, h)
		if rerr != nil {
			return mapPPTXHandleError(rerr)
		}
		slideRefCopy := *resolvedRef
		slideRef := &slideRefCopy
		slideNumber := slideRef.SlideNumber

		opts := mutate.ImageReplaceOptions{
			FitMode:             fitMode,
			NewImageData:        imageData,
			NewImageContentType: contentType,
		}
		replaceResult, err := mutate.ReplaceImage(selector, slideRef, pkg, opts)
		if err != nil {
			if isReplaceImageSearchMiss(err) {
				return mapPPTXHandleError(&pptxhandle.Error{
					Code:    pptxhandle.CodeStale,
					Handle:  pptxhandle.Format(h),
					Message: fmt.Sprintf("shape cNvPr id %d not found on slide sldId %d", h.ShapeID, h.SlideID),
				})
			}
			return err
		}

		destination, err = collectPPTXShapeDestination(pkg, slideNumber, targetSelector, destinationFile, false, true)
		if err != nil {
			return err
		}
		result = &replaceImageResult{
			File:           filePath,
			Output:         destinationFile,
			DryRun:         mutOpts.DryRun,
			Target:         targetSelector,
			FitMode:        string(fitMode),
			SlideNumber:    slideNumber,
			ShapeID:        replaceResult.ShapeID,
			ShapeName:      replaceResult.ShapeName,
			OldTargetURI:   replaceResult.OldTargetURI,
			OldContentType: replaceResult.OldContentType,
			NewTargetURI:   replaceResult.NewTargetURI,
			NewContentType: replaceResult.NewContentType,
			RelID:          replaceResult.RelationshipID,
			Destination:    destination,
		}
		result.PPTXBridgeReadbackCommands = pptxShapeMutationReadbackCommands(destination, false, true)
		return nil
	})
	if err != nil {
		return nil, mutationWriteError(err, "failed to replace image")
	}
	return result, nil
}

func isReplaceImageSearchMiss(err error) bool {
	if err == nil {
		return false
	}
	msg := err.Error()
	return strings.Contains(msg, "not found on slide") ||
		strings.Contains(msg, "no picture shape found matching selector")
}

// replaceImageResult holds the result of a successful image replacement
type replaceImageResult struct {
	File           string                `json:"file"`
	Output         string                `json:"output,omitempty"`
	DryRun         bool                  `json:"dryRun"`
	Target         string                `json:"target"`
	FitMode        string                `json:"fitMode"`
	SlideNumber    int                   `json:"slideNumber"`
	ShapeID        int                   `json:"shapeId"`
	ShapeName      string                `json:"shapeName"`
	OldTargetURI   string                `json:"oldTargetUri"`
	OldContentType string                `json:"oldContentType"`
	NewTargetURI   string                `json:"newTargetUri"`
	NewContentType string                `json:"newContentType"`
	RelID          string                `json:"relationshipId"`
	Destination    *PPTXShapeDestination `json:"destination,omitempty"`
	PPTXBridgeReadbackCommands
}

// getImageContentType determines content type from a supported image file path.
func getImageContentType(filePath string) (string, error) {
	contentType, ok := imagex.ContentTypeFromPath(filePath)
	if !ok {
		return "", NewCLIErrorf(ExitUnsupportedType, "unsupported image type for %s; supported extensions are .png, .jpg, .jpeg, .gif, .bmp, .tif, .tiff, .webp, and .svg", filePath)
	}
	return contentType, nil
}

// outputReplaceImageJSON outputs the result in JSON format
func outputReplaceImageJSON(cmd *cobra.Command, result *replaceImageResult) error {
	jsonData, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeCLIOutput(cmd, jsonData)
}

// outputReplaceImageText outputs the result in text format
func outputReplaceImageText(cmd *cobra.Command, result *replaceImageResult) error {
	var builder strings.Builder
	builder.WriteString("Image replaced successfully!\n\n")
	builder.WriteString(fmt.Sprintf("Slide:                %d\n", result.SlideNumber))
	builder.WriteString(fmt.Sprintf("Shape:                %s (ID: %d)\n", result.ShapeName, result.ShapeID))
	builder.WriteString(fmt.Sprintf("Relationship ID:      %s\n", result.RelID))
	if result.Output != "" {
		builder.WriteString(fmt.Sprintf("Output:               %s\n", result.Output))
	}
	if result.Destination != nil {
		builder.WriteString(fmt.Sprintf("Selector:             %s\n", result.Destination.PrimarySelector))
	}
	builder.WriteString("\nOld Image:\n")
	builder.WriteString(fmt.Sprintf("  URI:                %s\n", result.OldTargetURI))
	builder.WriteString(fmt.Sprintf("  Content Type:       %s\n\n", result.OldContentType))
	builder.WriteString("New Image:\n")
	builder.WriteString(fmt.Sprintf("  URI:                %s\n", result.NewTargetURI))
	builder.WriteString(fmt.Sprintf("  Content Type:       %s\n", result.NewContentType))
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func performBatchReplaceImage(cmd *cobra.Command, filePath string, forSlides string, selector selectors.Selector, imageData []byte, contentType string, fitMode mutate.FitMode, mutOpts *MutationOptions) error {
	// Parse slide specification
	slideNums, err := parseSlideSpec(forSlides)
	if err != nil {
		return InvalidArgsError(fmt.Sprintf("invalid slide specification: %v", err))
	}

	if len(slideNums) == 0 {
		return InvalidArgsError("no valid slides specified in --for-slides")
	}

	writer, err := NewMutationWriter(filePath, mutOpts)
	if err != nil {
		return err
	}

	var batchResult *mutate.BatchImageReplaceResult

	// Perform the batch mutation
	err = writer.Write(func(pkg opc.PackageSession) error {
		request := &mutate.BatchImageReplaceRequest{
			Package:             pkg,
			SlideNumbers:        slideNums,
			Target:              selector,
			NewImageData:        imageData,
			NewImageContentType: contentType,
			FitMode:             fitMode,
		}

		batchResult = mutate.BatchImageReplace(request)
		if batchResult.FatalError != "" {
			return fmt.Errorf(batchResult.FatalError)
		}

		return nil
	})

	if err != nil {
		return err
	}

	// Output results
	config := GetGlobalConfig(cmd)
	if config != nil && config.Format == "json" {
		return outputBatchReplaceImageJSON(cmd, batchResult, selector)
	}
	return outputBatchReplaceImageText(cmd, batchResult, selector)
}

func outputBatchReplaceImageJSON(cmd *cobra.Command, result *mutate.BatchImageReplaceResult, selector selectors.Selector) error {
	config := GetGlobalConfig(cmd)

	batchOutput := map[string]interface{}{
		"target":        selector.String(),
		"totalSlides":   result.TotalSlides,
		"successCount":  result.SuccessCount,
		"notFoundCount": result.NotFoundCount,
		"errorCount":    result.ErrorCount,
		"results":       result.Results,
	}

	data, err := marshalWithConfig(config, batchOutput)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputBatchReplaceImageText(cmd *cobra.Command, result *mutate.BatchImageReplaceResult, selector selectors.Selector) error {
	var message string
	message = fmt.Sprintf("Batch image replacement on target %s\n", selector.String())
	message += fmt.Sprintf("Total slides: %d\n", result.TotalSlides)
	message += fmt.Sprintf("Succeeded: %d\n", result.SuccessCount)
	message += fmt.Sprintf("Not found: %d\n", result.NotFoundCount)
	message += fmt.Sprintf("Errors: %d\n", result.ErrorCount)

	if result.ErrorCount > 0 {
		message += "\nErrors:\n"
		for _, r := range result.Results {
			if r.Error != "" {
				message += fmt.Sprintf("  Slide %d: %s\n", r.SlideNumber, r.Error)
			}
		}
	}

	return writeCLIOutput(cmd, []byte(message))
}

// init registers the replace images command
func init() {
	replaceImagesCmd.Flags().IntVar(
		&replaceImageSlide,
		"slide",
		0,
		"1-based slide number (use with single-slide operations)",
	)

	replaceImagesCmd.Flags().StringVar(
		&replaceImageForSlides,
		"for-slides",
		"",
		"slide specification for batch operations (e.g., '1-3,5,7-9')",
	)

	replaceImagesCmd.Flags().StringVar(
		&replaceImageTarget,
		"target",
		"",
		"target selector or stable shape handle (e.g., 'shape:2', '~Picture 1', or 'H:pptx/s:<sldId>/shape:n:<id>')",
	)
	replaceImagesCmd.MarkFlagRequired("target")

	replaceImagesCmd.Flags().StringVar(
		&replaceImageFile,
		"image",
		"",
		"path to the replacement image file",
	)
	replaceImagesCmd.MarkFlagRequired("image")

	replaceImagesCmd.Flags().StringVar(
		&replaceImageFitMode,
		"fit-mode",
		"contain",
		"fit mode: 'contain' (fit within bounds, stretch as needed) or 'cover' (tile to cover bounds)",
	)

	// Add mutation flags
	AddMutationFlags(replaceImagesCmd)

	replaceCmd.AddCommand(replaceImagesCmd)
}
