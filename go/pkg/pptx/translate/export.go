package translate

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// ExportTranslationRequest holds parameters for translation manifest export
type ExportTranslationRequest struct {
	// OPC session for reading package contents
	Session opc.PackageSession

	// Presentation graph (parsed from the OPC package)
	Graph *inspect.PresentationGraph

	// Slide numbers to include in the manifest (1-based, empty = all slides)
	SlideNumbers []int

	// Source language code (e.g., "en-US")
	SourceLanguage string

	// Target language code for translation (optional)
	TargetLanguage string

	// Deck name/identifier for metadata
	DeckName string

	// Include notes in the manifest
	IncludeNotes bool

	// Optional notes about this export
	Notes string
}

// ExportTranslation creates a translation manifest from a presentation
// Extracts all translatable text with stable IDs, context, and formatting information
func ExportTranslation(req *ExportTranslationRequest) (*TranslationManifest, error) {
	if req == nil || req.Session == nil || req.Graph == nil {
		return nil, fmt.Errorf("invalid export request: nil session or graph")
	}

	manifest := NewManifest()

	// Set metadata
	manifest.Metadata.SourceLanguage = req.SourceLanguage
	manifest.Metadata.TargetLanguage = req.TargetLanguage
	manifest.Metadata.DeckName = req.DeckName
	manifest.Metadata.SlideCount = len(req.Graph.Slides)
	manifest.Metadata.Notes = req.Notes

	// Determine which slides to export
	slidesToExport := req.SlideNumbers
	if len(slidesToExport) == 0 {
		// Export all slides
		for i := 1; i <= len(req.Graph.Slides); i++ {
			slidesToExport = append(slidesToExport, i)
		}
	}

	// Extract text from all requested slides
	textReq := &extract.ExtractTextRequest{
		Session:      req.Session,
		Graph:        req.Graph,
		SlideNumbers: slidesToExport,
	}

	textResult, err := extract.ExtractText(textReq)
	if err != nil {
		return nil, fmt.Errorf("failed to extract text: %w", err)
	}

	// Process each extracted slide
	for _, extractedSlide := range textResult.Slides {
		slideIdx := extractedSlide.Slide - 1 // Convert to 0-based

		// Process each shape in the slide
		for _, shape := range extractedSlide.Shapes {
			if shape.Text == nil || len(shape.Text.Paragraphs) == 0 {
				continue
			}

			// Process paragraphs and runs
			entries := processShape(slideIdx, extractedSlide.Slide, shape)
			manifest.Entries = append(manifest.Entries, entries...)
		}

		// Extract notes if requested
		if req.IncludeNotes && slideIdx < len(req.Graph.Slides) {
			slideRef := req.Graph.Slides[slideIdx]
			notesEntries, err := extractNotesEntries(req.Session, slideRef)
			if err == nil {
				manifest.Entries = append(manifest.Entries, notesEntries...)
			}
		}
	}

	// Set entry count
	manifest.Metadata.EntryCount = len(manifest.Entries)

	return manifest, nil
}

// processShape extracts translation entries from a shape's text
func processShape(slideIdx, slideNumber int, shape extract.ExtractedShape) []TranslationEntry {
	var entries []TranslationEntry

	if shape.Text == nil {
		return entries
	}

	// Determine entry type based on placeholder key
	entryType := determineEntryType(shape.Key)

	// Process each paragraph
	for paragraphIdx, paragraph := range shape.Text.Paragraphs {
		// Process each run in the paragraph
		for runIndex, run := range paragraph.Runs {
			var sourceText string
			var segmentType string
			var runProps *model.RunProperties

			// Extract text and properties based on run type
			switch r := run.(type) {
			case model.TextRun:
				if r.Text == "" {
					continue
				}
				sourceText = r.Text
				segmentType = model.SegmentText
				runProps = r.Properties

			case *model.TextRun:
				if r == nil || r.Text == "" {
					continue
				}
				sourceText = r.Text
				segmentType = model.SegmentText
				runProps = r.Properties

			case model.Break:
				sourceText = "\n"
				segmentType = model.SegmentBreak
				runProps = r.Properties

			case *model.Break:
				if r == nil {
					continue
				}
				sourceText = "\n"
				segmentType = model.SegmentBreak
				runProps = r.Properties

			case model.Tab:
				sourceText = "\t"
				segmentType = model.SegmentTab
				runProps = r.Properties

			case *model.Tab:
				if r == nil {
					continue
				}
				sourceText = "\t"
				segmentType = model.SegmentTab
				runProps = r.Properties

			case model.Field:
				sourceText = r.Text
				segmentType = model.SegmentField
				runProps = r.Properties

			case *model.Field:
				if r == nil || r.Text == "" {
					continue
				}
				sourceText = r.Text
				segmentType = model.SegmentField
				runProps = r.Properties

			default:
				// Skip unknown run types
				continue
			}

			// Skip empty text
			if sourceText == "" {
				continue
			}

			// Generate stable ID
			id := GenerateEntryID(slideIdx, shape.Key, paragraphIdx, runIndex)

			// Create entry
			entry := TranslationEntry{
				ID:             id,
				Type:           entryType,
				SourceText:     sourceText,
				SlideID:        slideIdx,
				SlideName:      "",
				SlideNumber:    slideNumber,
				PlaceholderKey: shape.Key,
				ShapeID:        shape.ID,
				ShapeName:      shape.Name,
				ParagraphIndex: paragraphIdx,
				RunIndex:       runIndex,
				SegmentType:    segmentType,
			}

			// Add bullet metadata if available
			if paragraph.Properties != nil {
				entry.BulletInfo = &BulletMetadata{
					Level:               paragraph.Properties.Level,
					BulletMode:          paragraph.Properties.BulletMode,
					BulletCharacter:     paragraph.Properties.BulletCharacter,
					AutoNumberingScheme: paragraph.Properties.AutoNumberingScheme,
					BulletFontFamily:    paragraph.Properties.BulletFontFamily,
					BulletFontSize:      paragraph.Properties.BulletFontSize,
					BulletColor:         paragraph.Properties.BulletColor,
				}
			}

			// Add run formatting if available
			if runProps != nil {
				entry.RunFormat = &RunFormatting{
					FontFamily: runProps.FontFamily,
					FontSize:   runProps.FontSize,
					Bold:       runProps.Bold,
					Italic:     runProps.Italic,
					Underline:  runProps.Underline,
					Strike:     runProps.Strike,
					Color:      runProps.Color,
					ThemeColor: runProps.ThemeColor,
					Language:   runProps.Language,
				}
			}

			entries = append(entries, entry)
		}
	}

	return entries
}

// extractNotesEntries extracts translation entries from a slide's notes
func extractNotesEntries(session opc.PackageSession, slideRef inspect.SlideRef) ([]TranslationEntry, error) {
	var entries []TranslationEntry

	notesReport, err := extract.ExtractNotesForSlide(session, slideRef)
	if err != nil {
		return nil, err
	}

	if notesReport.Notes == nil || len(notesReport.Notes.Paragraphs) == 0 {
		return entries, nil
	}

	slideIdx := slideRef.SlideNumber - 1

	// Process notes paragraphs
	for paragraphIdx, paragraph := range notesReport.Notes.Paragraphs {
		for runIndex, run := range paragraph.Runs {
			var sourceText string
			var segmentType string
			var runProps *model.RunProperties

			// Extract text and properties based on run type
			switch r := run.(type) {
			case model.TextRun:
				if r.Text == "" {
					continue
				}
				sourceText = r.Text
				segmentType = model.SegmentText
				runProps = r.Properties

			case *model.TextRun:
				if r == nil || r.Text == "" {
					continue
				}
				sourceText = r.Text
				segmentType = model.SegmentText
				runProps = r.Properties

			case model.Break:
				sourceText = "\n"
				segmentType = model.SegmentBreak
				runProps = r.Properties

			case *model.Break:
				if r == nil {
					continue
				}
				sourceText = "\n"
				segmentType = model.SegmentBreak
				runProps = r.Properties

			case model.Tab:
				sourceText = "\t"
				segmentType = model.SegmentTab
				runProps = r.Properties

			case *model.Tab:
				if r == nil {
					continue
				}
				sourceText = "\t"
				segmentType = model.SegmentTab
				runProps = r.Properties

			case model.Field:
				sourceText = r.Text
				segmentType = model.SegmentField
				runProps = r.Properties

			case *model.Field:
				if r == nil || r.Text == "" {
					continue
				}
				sourceText = r.Text
				segmentType = model.SegmentField
				runProps = r.Properties

			default:
				// Skip unknown run types
				continue
			}

			// Skip empty text
			if sourceText == "" {
				continue
			}

			// Generate stable ID for notes
			id := GenerateEntryID(slideIdx, "notes", paragraphIdx, runIndex)

			entry := TranslationEntry{
				ID:             id,
				Type:           "notes",
				SourceText:     sourceText,
				SlideID:        slideIdx,
				SlideNumber:    slideRef.SlideNumber,
				PlaceholderKey: "notes",
				ParagraphIndex: paragraphIdx,
				RunIndex:       runIndex,
				SegmentType:    segmentType,
			}

			// Add formatting info
			if paragraph.Properties != nil {
				entry.BulletInfo = &BulletMetadata{
					Level: paragraph.Properties.Level,
				}
			}

			// Add run formatting if available
			if runProps != nil {
				entry.RunFormat = &RunFormatting{
					FontFamily: runProps.FontFamily,
					FontSize:   runProps.FontSize,
					Bold:       runProps.Bold,
					Italic:     runProps.Italic,
					Underline:  runProps.Underline,
					Strike:     runProps.Strike,
					Color:      runProps.Color,
					ThemeColor: runProps.ThemeColor,
					Language:   runProps.Language,
				}
			}

			entries = append(entries, entry)
		}
	}

	return entries, nil
}

// determineEntryType maps placeholder keys to entry types
func determineEntryType(placeholderKey string) string {
	if strings.HasPrefix(placeholderKey, "title") {
		return "title"
	}
	if strings.HasPrefix(placeholderKey, "body") {
		return "body"
	}
	if strings.HasPrefix(placeholderKey, "subtitle") {
		return "subtitle"
	}
	// Fallback to generic type based on key
	return "body"
}
