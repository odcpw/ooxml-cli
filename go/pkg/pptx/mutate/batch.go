package mutate

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// BatchTextReplaceRequest holds parameters for batch text replacement across multiple slides
type BatchTextReplaceRequest struct {
	// Package session
	Package opc.PackageSession

	// Slide numbers (1-based). If empty, the replacement is single-slide behavior.
	// If populated, replacement is applied to each slide that has the target.
	SlideNumbers []int

	// Target selector (e.g., "title", "shape:5", "~MyShape", "@body")
	Target string

	// New text content (used for plain-text and preserve-format modes)
	NewText string

	// Replacement mode: "plain-text" (default), "preserve-format", or "rich-text"
	Mode string

	// Paragraph mutation options (optional)
	ParagraphOptions *ParagraphMutationOptions
	BulletOptions    *BulletMutationOptions
}

// BatchTextReplaceResult represents the aggregated result of a batch text replacement
type BatchTextReplaceResult struct {
	// Total slides requested
	TotalSlides int

	// Slides where replacement succeeded
	SuccessCount int

	// Slides where target was not found (but replacement didn't fail)
	NotFoundCount int

	// Slides with errors
	ErrorCount int

	// Per-slide results
	Results []TextReplaceSlideResult

	// Aggregated error message if the operation failed fatally
	FatalError string
}

// TextReplaceSlideResult represents the result for a single slide
type TextReplaceSlideResult struct {
	SlideNumber int
	Success     bool
	NotFound    bool
	Error       string
}

// BatchTextReplace replaces text across multiple slides with aggregated reporting
func BatchTextReplace(req *BatchTextReplaceRequest) *BatchTextReplaceResult {
	result := &BatchTextReplaceResult{
		TotalSlides: len(req.SlideNumbers),
		Results:     make([]TextReplaceSlideResult, len(req.SlideNumbers)),
	}

	if req.Target == "" {
		result.FatalError = "target selector cannot be empty"
		return result
	}

	// Default to plain-text mode
	if req.Mode == "" {
		req.Mode = "plain-text"
	}

	// Parse the presentation to get slide info once
	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		result.FatalError = fmt.Sprintf("failed to parse presentation: %v", err)
		return result
	}

	// Process each slide
	for i, slideNumber := range req.SlideNumbers {
		slideResult := TextReplaceSlideResult{
			SlideNumber: slideNumber,
		}

		// Validate slide number
		if slideNumber < 1 || slideNumber > len(graph.Slides) {
			slideResult.Error = fmt.Sprintf("slide %d not found (presentation has %d slides)", slideNumber, len(graph.Slides))
			slideResult.NotFound = true
			result.NotFoundCount++
			result.Results[i] = slideResult
			continue
		}

		// Perform replacement on this slide
		textReq := &ReplaceTextRequest{
			Package:          req.Package,
			SlideNumber:      slideNumber,
			Target:           req.Target,
			NewText:          req.NewText,
			Mode:             req.Mode,
			ParagraphOptions: req.ParagraphOptions,
			BulletOptions:    req.BulletOptions,
		}

		err := ReplaceText(textReq)
		if err != nil {
			// Check if it's a "target not found" error
			if fmt.Sprintf("%v", err) == fmt.Sprintf("target not found: %s", req.Target) {
				slideResult.NotFound = true
				result.NotFoundCount++
			} else {
				slideResult.Error = fmt.Sprintf("%v", err)
				result.ErrorCount++
			}
		} else {
			slideResult.Success = true
			result.SuccessCount++
		}

		result.Results[i] = slideResult
	}

	return result
}

// BatchImageReplaceRequest holds parameters for batch image replacement across multiple slides
type BatchImageReplaceRequest struct {
	// Package session
	Package opc.PackageSession

	// Slide numbers (1-based)
	SlideNumbers []int

	// Target selector (e.g., "shape:2", "~Picture 1")
	Target selectors.Selector

	// New image data and metadata
	NewImageData        []byte
	NewImageContentType string

	// Fit mode for the image (contain or cover)
	FitMode FitMode
}

// BatchImageReplaceResult represents the aggregated result of a batch image replacement
type BatchImageReplaceResult struct {
	// Total slides requested
	TotalSlides int

	// Slides where replacement succeeded
	SuccessCount int

	// Slides where target was not found
	NotFoundCount int

	// Slides with errors
	ErrorCount int

	// Per-slide results
	Results []ImageReplaceSlideResult

	// Aggregated error message if the operation failed fatally
	FatalError string
}

// ImageReplaceSlideResult represents the result for a single slide
type ImageReplaceSlideResult struct {
	SlideNumber int
	Success     bool
	NotFound    bool
	Error       string
	Result      *ReplaceImageResult // populated on success
}

// BatchImageReplace replaces images across multiple slides with aggregated reporting
func BatchImageReplace(req *BatchImageReplaceRequest) *BatchImageReplaceResult {
	result := &BatchImageReplaceResult{
		TotalSlides: len(req.SlideNumbers),
		Results:     make([]ImageReplaceSlideResult, len(req.SlideNumbers)),
	}

	if req.NewImageData == nil || len(req.NewImageData) == 0 {
		result.FatalError = "new image data is empty"
		return result
	}

	// Parse the presentation to get slide info once
	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		result.FatalError = fmt.Sprintf("failed to parse presentation: %v", err)
		return result
	}

	// Process each slide
	for i, slideNumber := range req.SlideNumbers {
		slideResult := ImageReplaceSlideResult{
			SlideNumber: slideNumber,
		}

		// Validate slide number
		if slideNumber < 1 || slideNumber > len(graph.Slides) {
			slideResult.Error = fmt.Sprintf("slide %d not found (presentation has %d slides)", slideNumber, len(graph.Slides))
			slideResult.NotFound = true
			result.NotFoundCount++
			result.Results[i] = slideResult
			continue
		}

		slideRef := graph.Slides[slideNumber-1]

		// Read the slide XML
		slideDoc, err := req.Package.ReadXMLPart(slideRef.PartURI)
		if err != nil {
			slideResult.Error = fmt.Sprintf("failed to read slide: %v", err)
			result.ErrorCount++
			result.Results[i] = slideResult
			continue
		}

		// Get the shape tree
		spTree := slideDoc.FindElement(".//spTree")
		if spTree == nil {
			slideResult.Error = "shape tree not found in slide"
			result.ErrorCount++
			result.Results[i] = slideResult
			continue
		}

		// Perform replacement on this slide
		opts := ImageReplaceOptions{
			FitMode:             req.FitMode,
			NewImageData:        req.NewImageData,
			NewImageContentType: req.NewImageContentType,
		}

		replaceResult, err := ReplaceImage(req.Target, &slideRef, req.Package, opts)
		if err != nil {
			if isImageReplaceNotFoundError(err) {
				slideResult.NotFound = true
				result.NotFoundCount++
			} else {
				slideResult.Error = fmt.Sprintf("%v", err)
				result.ErrorCount++
			}
		} else {
			slideResult.Success = true
			slideResult.Result = replaceResult
			result.SuccessCount++
		}

		result.Results[i] = slideResult
	}

	return result
}

func isImageReplaceNotFoundError(err error) bool {
	if err == nil {
		return false
	}
	msg := err.Error()
	return strings.Contains(msg, "not found on slide") ||
		strings.Contains(msg, "no picture shape found matching selector")
}

// SummarizeBatchResult provides a human-readable summary of batch operation results
func SummarizeBatchResult(operation string, result interface{}) string {
	switch r := result.(type) {
	case *BatchTextReplaceResult:
		if r.FatalError != "" {
			return fmt.Sprintf("%s failed: %s", operation, r.FatalError)
		}
		summary := fmt.Sprintf("%s: %d/%d slides succeeded", operation, r.SuccessCount, r.TotalSlides)
		if r.NotFoundCount > 0 {
			summary += fmt.Sprintf(" (%d not found)", r.NotFoundCount)
		}
		if r.ErrorCount > 0 {
			summary += fmt.Sprintf(" (%d errors)", r.ErrorCount)
		}
		return summary

	case *BatchImageReplaceResult:
		if r.FatalError != "" {
			return fmt.Sprintf("%s failed: %s", operation, r.FatalError)
		}
		summary := fmt.Sprintf("%s: %d/%d slides succeeded", operation, r.SuccessCount, r.TotalSlides)
		if r.NotFoundCount > 0 {
			summary += fmt.Sprintf(" (%d not found)", r.NotFoundCount)
		}
		if r.ErrorCount > 0 {
			summary += fmt.Sprintf(" (%d errors)", r.ErrorCount)
		}
		return summary

	default:
		return fmt.Sprintf("%s: unknown result type", operation)
	}
}
