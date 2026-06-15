package template

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// CompileError represents an error that occurred during compilation
type CompileError struct {
	SlideIndex int
	SlotID     string
	Message    string
	Err        error
}

// Error implements the error interface
func (ce *CompileError) Error() string {
	if ce.SlotID != "" {
		return fmt.Sprintf("slide %d, slot %s: %s", ce.SlideIndex, ce.SlotID, ce.Message)
	}
	return fmt.Sprintf("slide %d: %s", ce.SlideIndex, ce.Message)
}

// CompileResult represents the result of a compilation operation
type CompileResult struct {
	OutputPath     string
	SlideCount     int
	Errors         []*CompileError
	SlotsAttempted int
	SlotsSucceeded int
	StartedAt      time.Time
	CompletedAt    time.Time
}

// CompileOptions contains options for the compilation process
type CompileOptions struct {
	ArchetypePath   string
	OutputPath      string
	PreserveLayouts bool
	StrictMode      bool
	ContinueOnError bool
	ImageBaseDir    string
	ThemeOptions    *ThemeMutationOptions
}

// ThemeMutationOptions contains options for applying theme mutations
type ThemeMutationOptions struct {
	Colors    map[string]string
	MajorFont string
	MinorFont string
}

// CompilerEngine coordinates the template compilation process
type CompilerEngine struct {
	manifest         *TemplateManifest
	spec             *CompilationSpec
	options          CompileOptions
	errors           []*CompileError
	outputSession    opc.PackageSession
	imageCache       map[string][]byte
	seedSlideCount   int // Number of pristine archetype slides kept as clone sources during compilation
	currentLastSlide int // Track the last slide number in the output
}

// NewCompilerEngine creates a new compiler engine
func NewCompilerEngine(manifest *TemplateManifest, spec *CompilationSpec, options CompileOptions) *CompilerEngine {
	return &CompilerEngine{
		manifest:   manifest,
		spec:       spec,
		options:    options,
		errors:     []*CompileError{},
		imageCache: make(map[string][]byte),
	}
}

// Compile executes the complete compilation process
func (ce *CompilerEngine) Compile() (*CompileResult, error) {
	startTime := time.Now()

	// Validate inputs
	if err := ce.validateInputs(); err != nil {
		return nil, fmt.Errorf("validation failed: %w", err)
	}

	// Create output presentation by cloning the archetype
	outputSession, err := ce.initializeOutputPresentation()
	if err != nil {
		return nil, fmt.Errorf("failed to initialize output: %w", err)
	}
	defer outputSession.Close()
	ce.outputSession = outputSession

	// Open archetype to get the base structure
	archetypeSession, err := opc.Open(ce.options.ArchetypePath)
	if err != nil {
		return nil, fmt.Errorf("failed to open archetype: %w", err)
	}
	defer archetypeSession.Close()

	// Parse the archetype presentation
	archetypeGraph, err := inspect.ParsePresentation(archetypeSession)
	if err != nil {
		return nil, fmt.Errorf("failed to parse archetype: %w", err)
	}

	// Keep the copied archetype slides pristine during compilation and clone from
	// them into appended output slides. We'll remove these seed slides once all
	// requested spec slides have been materialized.
	ce.seedSlideCount = len(archetypeGraph.Slides)
	ce.currentLastSlide = ce.seedSlideCount

	// Compile each slide from the spec
	var slotsAttempted, slotsSucceeded int
	for i, slideSpec := range ce.spec.Slides {
		attempted, succeeded, err := ce.compileSlide(i, &slideSpec, archetypeGraph)
		slotsAttempted += attempted
		slotsSucceeded += succeeded

		if err != nil && !ce.options.ContinueOnError {
			return nil, fmt.Errorf("compilation failed at slide %d: %w", i, err)
		}
	}

	// Remove the pristine seed slides now that all compiled slides exist.
	if err := ce.cleanupSeedSlides(); err != nil {
		return nil, fmt.Errorf("failed to finalize output slides: %w", err)
	}

	// Apply theme overrides if specified
	if ce.spec.ThemeOverrides != nil || ce.options.ThemeOptions != nil {
		if err := ce.applyThemeOverrides(); err != nil {
			ce.addError(&CompileError{
				Message: fmt.Sprintf("theme overrides failed: %v", err),
				Err:     err,
			})
			if ce.options.StrictMode {
				return nil, err
			}
		}
	}

	// Validate the output presentation
	if err := ce.validateOutput(); err != nil {
		ce.addError(&CompileError{
			Message: fmt.Sprintf("output validation failed: %v", err),
			Err:     err,
		})
		if ce.options.StrictMode {
			return nil, err
		}
	}

	// Persist the output presentation.
	if err := ce.outputSession.SaveAs(ce.options.OutputPath); err != nil {
		return nil, fmt.Errorf("failed to save output: %w", err)
	}

	// Return success with results
	slideCount := len(ce.spec.Slides)
	if outputGraph, err := inspect.ParsePresentation(ce.outputSession); err == nil {
		slideCount = len(outputGraph.Slides)
	}
	result := &CompileResult{
		OutputPath:     ce.options.OutputPath,
		SlideCount:     slideCount,
		Errors:         ce.errors,
		SlotsAttempted: slotsAttempted,
		SlotsSucceeded: slotsSucceeded,
		StartedAt:      startTime,
		CompletedAt:    time.Now(),
	}

	return result, nil
}

// validateInputs validates the inputs to compilation
func (ce *CompilerEngine) validateInputs() error {
	if ce.manifest == nil {
		return fmt.Errorf("manifest is nil")
	}

	if ce.spec == nil {
		return fmt.Errorf("spec is nil")
	}

	if ce.options.ArchetypePath == "" {
		return fmt.Errorf("archetype path is empty")
	}

	if ce.options.OutputPath == "" {
		return fmt.Errorf("output path is empty")
	}

	if _, err := os.Stat(ce.options.ArchetypePath); err != nil {
		return fmt.Errorf("archetype not found: %w", err)
	}

	// Validate spec against manifest
	if err := ValidateCompilationSpec(ce.spec, ce.manifest); err != nil {
		return fmt.Errorf("spec validation failed: %w", err)
	}

	return nil
}

// initializeOutputPresentation creates the output presentation
func (ce *CompilerEngine) initializeOutputPresentation() (opc.PackageSession, error) {
	// Copy the archetype file to the output location
	src, err := os.Open(ce.options.ArchetypePath)
	if err != nil {
		return nil, fmt.Errorf("failed to open archetype: %w", err)
	}
	defer src.Close()

	out, err := os.Create(ce.options.OutputPath)
	if err != nil {
		return nil, fmt.Errorf("failed to create output file: %w", err)
	}
	defer out.Close()

	if _, err := io.Copy(out, src); err != nil {
		return nil, fmt.Errorf("failed to copy archetype: %w", err)
	}

	// Open the output file as a session
	session, err := opc.Open(ce.options.OutputPath)
	if err != nil {
		return nil, fmt.Errorf("failed to open output session: %w", err)
	}

	return session, nil
}

// cleanupSeedSlides removes the pristine archetype slides after all compiled
// slides have been cloned and filled.
func (ce *CompilerEngine) cleanupSeedSlides() error {
	if ce.outputSession == nil {
		return fmt.Errorf("output session is not initialized")
	}

	for slideNum := ce.seedSlideCount; slideNum >= 1; slideNum-- {
		if _, err := mutate.DeleteSlide(&mutate.DeleteSlideRequest{
			Package:     ce.outputSession,
			SlideNumber: slideNum,
		}); err != nil {
			return fmt.Errorf("failed to delete seed slide %d: %w", slideNum, err)
		}
	}

	return nil
}

// compileSlide compiles a single slide from the spec
func (ce *CompilerEngine) compileSlide(slideIndex int, slideSpec *SlideSpec, archetypeGraph *inspect.PresentationGraph) (int, int, error) {
	var slotsAttempted, slotsSucceeded int

	// Find the archetype
	var archetype *Archetype
	for i := range ce.manifest.Archetypes {
		if ce.manifest.Archetypes[i].ID == slideSpec.Archetype {
			archetype = &ce.manifest.Archetypes[i]
			break
		}
	}

	if archetype == nil {
		err := fmt.Errorf("unknown archetype: %s", slideSpec.Archetype)
		ce.addError(&CompileError{
			SlideIndex: slideIndex,
			Message:    err.Error(),
			Err:        err,
		})
		return 0, 0, err
	}

	if len(archetypeGraph.Slides) == 0 {
		err := fmt.Errorf("archetype presentation has no slides")
		ce.addError(&CompileError{
			SlideIndex: slideIndex,
			Message:    err.Error(),
			Err:        err,
		})
		return 0, 0, err
	}

	sourceSlideNumber := archetype.SourceSlideNumber
	if sourceSlideNumber == 0 {
		sourceSlideNumber = 1
	}
	if sourceSlideNumber > len(archetypeGraph.Slides) {
		err := fmt.Errorf("archetype source slide %d out of range (source has %d slides)", sourceSlideNumber, len(archetypeGraph.Slides))
		ce.addError(&CompileError{
			SlideIndex: slideIndex,
			Message:    err.Error(),
			Err:        err,
		})
		return 0, 0, err
	}

	cloneResult, err := mutate.CloneSlide(&mutate.CloneSlideRequest{
		Package:     ce.outputSession,
		SlideNumber: sourceSlideNumber,
		InsertAfter: ce.currentLastSlide,
		NotesPolicy: mutate.NotesClone,
	})
	if err != nil {
		err := fmt.Errorf("failed to clone archetype slide: %w", err)
		ce.addError(&CompileError{
			SlideIndex: slideIndex,
			Message:    err.Error(),
			Err:        err,
		})
		return 0, 0, err
	}

	slideNumber := cloneResult.NewSlideNumber
	ce.currentLastSlide = slideNumber

	// Fill in the slots
	for _, slot := range archetype.Slots {
		slotsAttempted++

		// Get the content for this slot
		content, ok := slideSpec.Content[slot.ID]
		if !ok {
			// Try by slot name as fallback
			content, ok = slideSpec.Content[slot.Name]
		}

		if !ok {
			if slot.Required {
				err := fmt.Errorf("required slot %q not provided", slot.ID)
				ce.addError(&CompileError{
					SlideIndex: slideIndex,
					SlotID:     slot.ID,
					Message:    err.Error(),
					Err:        err,
				})
				if !ce.options.ContinueOnError {
					return slotsAttempted, slotsSucceeded, err
				}
			}
			continue
		}

		// Fill the slot based on its kind
		if err := ce.fillSlot(slideNumber, &slot, content); err != nil {
			ce.addError(&CompileError{
				SlideIndex: slideIndex,
				SlotID:     slot.ID,
				Message:    fmt.Sprintf("failed to fill slot: %v", err),
				Err:        err,
			})
			if !ce.options.ContinueOnError {
				return slotsAttempted, slotsSucceeded, err
			}
		} else {
			slotsSucceeded++
		}
	}

	// Process slide-level notes if provided
	if slideSpec.Notes != "" {
		if err := ce.fillNotesSlide(slideNumber, slideSpec.Notes); err != nil {
			ce.addError(&CompileError{
				SlideIndex: slideIndex,
				Message:    fmt.Sprintf("failed to fill notes: %v", err),
				Err:        err,
			})
			if !ce.options.ContinueOnError {
				return slotsAttempted, slotsSucceeded, err
			}
		}
	}

	return slotsAttempted, slotsSucceeded, nil
}

// fillSlot fills a single slot with the provided content
func (ce *CompilerEngine) fillSlot(slideNumber int, slot *Slot, content interface{}) error {
	if content == nil {
		return nil
	}

	// Convert content to appropriate format based on slot kind
	switch slot.Kind {
	case SlotKindText, SlotKindRichText:
		return ce.fillTextSlot(slideNumber, slot, content)

	case SlotKindBullets:
		return ce.fillBulletsSlot(slideNumber, slot, content)

	case SlotKindImage:
		return ce.fillImageSlot(slideNumber, slot, content)

	case SlotKindTable:
		return ce.fillTableSlot(slideNumber, slot, content)

	case SlotKindNotes:
		return ce.fillNotesSlot(slideNumber, slot, content)

	default:
		return fmt.Errorf("unsupported slot kind: %s", slot.Kind)
	}
}

// fillTextSlot fills a text or rich-text slot
func (ce *CompilerEngine) fillTextSlot(slideNumber int, slot *Slot, content interface{}) error {
	// Convert content to string
	text := ce.contentToString(content)
	if text == "" {
		return nil
	}

	// Use PlaceholderKey if available, otherwise use slot ID
	target := slot.PlaceholderKey
	if target == "" {
		target = slot.ID
	}

	// Determine mode based on slot kind
	mode := "plain-text"
	if slot.Kind == SlotKindRichText {
		mode = "rich-text"
	}

	// Replace text using the actual mutate API
	req := &mutate.ReplaceTextRequest{
		Package:     ce.outputSession,
		SlideNumber: slideNumber,
		Target:      target,
		NewText:     text,
		Mode:        mode,
	}

	return mutate.ReplaceText(req)
}

// fillBulletsSlot fills a bullets slot
func (ce *CompilerEngine) fillBulletsSlot(slideNumber int, slot *Slot, content interface{}) error {
	var bullets []string

	// Handle different content formats
	switch v := content.(type) {
	case string:
		// Parse bullet list from string
		bullets = ce.parseBulletList(v)

	case []interface{}:
		// Array of strings
		for _, item := range v {
			if s, ok := item.(string); ok {
				bullets = append(bullets, s)
			}
		}

	case map[string]interface{}:
		// Structured bullet content
		if items, ok := v["items"].([]interface{}); ok {
			for _, item := range items {
				if s, ok := item.(string); ok {
					bullets = append(bullets, s)
				}
			}
		}
	}

	if len(bullets) == 0 {
		return nil
	}

	// Use PlaceholderKey if available, otherwise use slot ID
	target := slot.PlaceholderKey
	if target == "" {
		target = slot.ID
	}

	// Join bullets with newlines for text replacement
	bulletText := strings.Join(bullets, "\n")

	// Apply text replacement
	req := &mutate.ReplaceTextRequest{
		Package:     ce.outputSession,
		SlideNumber: slideNumber,
		Target:      target,
		NewText:     bulletText,
		Mode:        "plain-text",
	}

	return mutate.ReplaceText(req)
}

// fillImageSlot fills an image slot by replacing an existing image in a picture placeholder
// or inserting a new image at the slot bounds if no existing picture is found.
func (ce *CompilerEngine) fillImageSlot(slideNumber int, slot *Slot, content interface{}) error {
	// Extract image path from content
	var imagePath string
	var fitMode mutate.FitMode = mutate.FitModeContain // default

	switch v := content.(type) {
	case string:
		imagePath = v

	case map[string]interface{}:
		if path, ok := v["path"].(string); ok {
			imagePath = path
		}
		// Check for fit mode override
		if fitModeStr, ok := v["fitMode"].(string); ok {
			if parsed, err := mutate.ParseFitMode(fitModeStr); err == nil {
				fitMode = parsed
			}
		}
	}

	if imagePath == "" {
		return fmt.Errorf("image path is empty")
	}

	// Resolve image path
	if ce.options.ImageBaseDir != "" && !filepath.IsAbs(imagePath) {
		imagePath = filepath.Join(ce.options.ImageBaseDir, imagePath)
	}

	// Load image data
	imageData, err := ce.loadImageData(imagePath)
	if err != nil {
		return fmt.Errorf("failed to load image: %w", err)
	}

	if len(imageData) == 0 {
		return fmt.Errorf("image data is empty")
	}

	// Determine content type
	contentType, err := ce.detectImageContentType(imagePath)
	if err != nil {
		return err
	}

	// Parse the output presentation to get SlideRef
	outputGraph, err := inspect.ParsePresentation(ce.outputSession)
	if err != nil {
		return fmt.Errorf("failed to parse output presentation: %w", err)
	}

	if slideNumber < 1 || slideNumber > len(outputGraph.Slides) {
		return fmt.Errorf("slide number %d out of range (presentation has %d slides)", slideNumber, len(outputGraph.Slides))
	}

	slideRef := outputGraph.Slides[slideNumber-1]

	// Determine the selector
	var selector selectors.Selector

	if slot.PlaceholderKey != "" {
		// Parse the placeholder key as selector
		var err error
		selector, err = selectors.Parse(slot.PlaceholderKey)
		if err != nil {
			return fmt.Errorf("failed to parse placeholder key %q: %w", slot.PlaceholderKey, err)
		}
	} else if slot.Name != "" {
		// Use slot name as shape name selector
		selector = &selectors.ShapeNameSelector{Name: slot.Name}
	} else {
		return fmt.Errorf("slot has neither PlaceholderKey nor Name")
	}

	// Read the slide to check for existing picture shapes
	slideDoc, err := ce.outputSession.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return fmt.Errorf("failed to read slide: %w", err)
	}

	spTree := slideDoc.FindElement(".//spTree")
	if spTree == nil {
		// If no shape tree and we have bounds, insert new picture
		if slot.Bounds != nil {
			return ce.insertImageViaSlotBounds(slideRef, slot, imageData, contentType, fitMode)
		}
		return fmt.Errorf("no shape tree found and no bounds specified for image insertion")
	}

	// Try to find an existing picture shape to replace
	pictures := spTree.FindElements("pic")

	// Try to resolve selector to a picture and replace it
	for _, pic := range pictures {
		// Extract picture info
		nvPicPr := pic.FindElement("nvPicPr")
		if nvPicPr == nil {
			continue
		}
		cNvPr := nvPicPr.FindElement("cNvPr")
		if cNvPr == nil {
			continue
		}

		picName := cNvPr.SelectAttrValue("name", "")

		// Check if this picture matches our selector
		if shapeNameSel, ok := selector.(*selectors.ShapeNameSelector); ok {
			if picName == shapeNameSel.Name {
				// Replace this picture
				result, err := mutate.ReplaceImage(selector, &slideRef, ce.outputSession, mutate.ImageReplaceOptions{
					FitMode:             fitMode,
					NewImageData:        imageData,
					NewImageContentType: contentType,
				})
				if err != nil {
					return fmt.Errorf("failed to replace image: %w", err)
				}
				_ = result // Use the result if logging is needed
				return nil
			}
		}

		// For placeholder key selector, try to match
		if placeKeySel, ok := selector.(*selectors.PlaceholderKeySelector); ok {
			if picName == placeKeySel.Key {
				result, err := mutate.ReplaceImage(selector, &slideRef, ce.outputSession, mutate.ImageReplaceOptions{
					FitMode:             fitMode,
					NewImageData:        imageData,
					NewImageContentType: contentType,
				})
				if err != nil {
					return fmt.Errorf("failed to replace image: %w", err)
				}
				_ = result
				return nil
			}
		}
	}

	// No existing picture found
	// If we have bounds, insert a new picture
	if slot.Bounds != nil {
		return ce.insertImageViaSlotBounds(slideRef, slot, imageData, contentType, fitMode)
	}

	// No picture to replace and no bounds to insert at
	return fmt.Errorf("no existing picture found for selector and no bounds specified for insertion")
}

// insertImageViaSlotBounds inserts a new image at the slot bounds
func (ce *CompilerEngine) insertImageViaSlotBounds(slideRef inspect.SlideRef, slot *Slot, imageData []byte, contentType string, fitMode mutate.FitMode) error {
	req := &mutate.InsertImageRequest{
		Package:     ce.outputSession,
		SlideRef:    &slideRef,
		ImageData:   imageData,
		ContentType: contentType,
		FitMode:     fitMode,
		X:           slot.Bounds.X,
		Y:           slot.Bounds.Y,
		CX:          slot.Bounds.CX,
		CY:          slot.Bounds.CY,
	}

	result, err := mutate.InsertImage(req)
	if err != nil {
		return fmt.Errorf("failed to insert image: %w", err)
	}

	_ = result // Use the result if logging is needed
	return nil
}

// fillTableSlot fills a table slot by inserting a new table on the slide
func (ce *CompilerEngine) fillTableSlot(slideNumber int, slot *Slot, content interface{}) error {
	// Extract table data and options from content
	var tableData [][]interface{}
	var hasHeader bool
	var hasBandedRows bool
	var headerFillColor string
	var bandFill1Color string
	var bandFill2Color string
	var defaultFontSize int

	switch v := content.(type) {
	case map[string]interface{}:
		// Extract data from map
		if data, ok := v["data"].([]interface{}); ok {
			for _, row := range data {
				if rowData, ok := row.([]interface{}); ok {
					tableData = append(tableData, rowData)
				}
			}
		}

		// Extract table options from map
		if header, ok := v["hasHeaders"].(bool); ok {
			hasHeader = header
		}
		if banded, ok := v["bandedRows"].(bool); ok {
			hasBandedRows = banded
		}
		if color, ok := v["headerFillColor"].(string); ok {
			headerFillColor = color
		}
		if color, ok := v["bandFill1Color"].(string); ok {
			bandFill1Color = color
		}
		if color, ok := v["bandFill2Color"].(string); ok {
			bandFill2Color = color
		}
		if fontSize, ok := v["defaultFontSize"].(int); ok {
			defaultFontSize = fontSize
		}

	case []interface{}:
		// Handle array of rows (YAML unmarshaling produces this)
		for _, row := range v {
			if rowData, ok := row.([]interface{}); ok {
				tableData = append(tableData, rowData)
			}
		}

	case [][]interface{}:
		tableData = v
	}

	if len(tableData) == 0 {
		return fmt.Errorf("table data is empty")
	}

	// Convert table data from [][]interface{} to [][]string
	stringData := make([][]string, len(tableData))
	for i, row := range tableData {
		stringData[i] = make([]string, len(row))
		for j, cell := range row {
			stringData[i][j] = fmt.Sprint(cell)
		}
	}

	// Parse the output presentation to get SlideRef
	outputGraph, err := inspect.ParsePresentation(ce.outputSession)
	if err != nil {
		return fmt.Errorf("failed to parse output presentation: %w", err)
	}

	if slideNumber < 1 || slideNumber > len(outputGraph.Slides) {
		return fmt.Errorf("slide number %d out of range (presentation has %d slides)", slideNumber, len(outputGraph.Slides))
	}

	slideRef := outputGraph.Slides[slideNumber-1]

	// Determine table bounds
	var x, y, width, height int64

	if slot.Bounds != nil {
		x = slot.Bounds.X
		y = slot.Bounds.Y
		width = slot.Bounds.CX
		height = slot.Bounds.CY
	} else {
		// Use default bounds: centered on slide, 80% width
		// Standard PowerPoint slide: 9144000 EMUs wide (10 inches), 7620000 EMUs tall (7.5 inches)
		const slideWidth = int64(9144000)
		const slideHeight = int64(7620000)
		const margin = int64(457200) // 0.5 inches

		x = margin
		y = 2 * margin // Leave space for title
		width = slideWidth - (2 * margin)
		height = 0 // Auto-calculate height based on data
	}

	// Validate dimensions
	if width <= 0 {
		return fmt.Errorf("invalid table width: %d", width)
	}

	// Set default font size if not provided
	if defaultFontSize <= 0 {
		defaultFontSize = 18
	}

	if slot.TableID != nil {
		rows := len(stringData)
		cols := 0
		if rows > 0 {
			cols = len(stringData[0])
		}
		if slot.TableRows != nil && *slot.TableRows != rows {
			return fmt.Errorf("table slot %q target table has %d rows, content has %d rows", slot.ID, *slot.TableRows, rows)
		}
		if slot.TableCols != nil && *slot.TableCols != cols {
			return fmt.Errorf("table slot %q target table has %d columns, content has %d columns", slot.ID, *slot.TableCols, cols)
		}
		_, err := mutate.SetTableTextMatrix(&mutate.SetTableTextMatrixRequest{
			Package:  ce.outputSession,
			SlideRef: &slideRef,
			TableID:  *slot.TableID,
			Data:     stringData,
		})
		if err != nil {
			return fmt.Errorf("failed to fill table slot %q: %w", slot.ID, err)
		}
		return nil
	}

	// Create and insert the table
	req := &mutate.InsertTableRequest{
		Package:         ce.outputSession,
		SlideRef:        &slideRef,
		Data:            stringData,
		X:               x,
		Y:               y,
		Width:           width,
		Height:          height,
		HasHeader:       hasHeader,
		HasBandedRows:   hasBandedRows,
		HeaderFillColor: headerFillColor,
		BandFill1Color:  bandFill1Color,
		BandFill2Color:  bandFill2Color,
		DefaultFontSize: defaultFontSize,
	}

	result, err := mutate.InsertTable(req)
	if err != nil {
		return fmt.Errorf("failed to insert table: %w", err)
	}

	_ = result // Use the result if logging is needed
	return nil
}

// fillNotesSlide fills notes directly (from SlideSpec.Notes field)
func (ce *CompilerEngine) fillNotesSlide(slideNumber int, notesText string) error {
	return ce.fillNotesText(slideNumber, notesText)
}

// fillNotesSlot fills the notes slide
func (ce *CompilerEngine) fillNotesSlot(slideNumber int, slot *Slot, content interface{}) error {
	// Convert content to string
	notesText := ce.contentToString(content)
	if notesText == "" {
		return nil
	}
	return ce.fillNotesText(slideNumber, notesText)
}

func (ce *CompilerEngine) fillNotesText(slideNumber int, notesText string) error {
	// Parse the output presentation to get the slide's notes URI
	outputGraph, err := inspect.ParsePresentation(ce.outputSession)
	if err != nil {
		return fmt.Errorf("failed to parse output presentation: %w", err)
	}

	if slideNumber < 1 || slideNumber > len(outputGraph.Slides) {
		return fmt.Errorf("slide number %d out of range", slideNumber)
	}

	slideRef := outputGraph.Slides[slideNumber-1]

	if slideRef.NotesPartURI == "" {
		return fmt.Errorf("slide has no notes part; cannot fill notes")
	}

	// Read the notes slide XML
	notesDoc, err := ce.outputSession.ReadXMLPart(slideRef.NotesPartURI)
	if err != nil {
		return fmt.Errorf("failed to read notes slide: %w", err)
	}

	// Find the text body in the notes slide
	// Structure: p:notes -> p:cSld -> p:spTree -> p:sp (with ph type="body") -> p:txBody
	root := notesDoc.Root()
	if root == nil {
		return fmt.Errorf("notes document has no root")
	}

	// Find p:cSld
	cSld := xmlx.FindChild(root, namespaces.NsP, "cSld")
	if cSld == nil {
		return fmt.Errorf("notes slide has no cSld element")
	}

	// Find p:spTree
	spTree := xmlx.FindChild(cSld, namespaces.NsP, "spTree")
	if spTree == nil {
		return fmt.Errorf("notes slide has no spTree element")
	}

	// Find the p:sp with ph type="body"
	shapes := xmlx.FindChildren(spTree, namespaces.NsP, "sp")
	var targetShape *etree.Element
	for _, sp := range shapes {
		nvSpPr := xmlx.FindChild(sp, namespaces.NsP, "nvSpPr")
		if nvSpPr == nil {
			continue
		}

		nvPr := xmlx.FindChild(nvSpPr, namespaces.NsP, "nvPr")
		if nvPr == nil {
			continue
		}

		ph := xmlx.FindChild(nvPr, namespaces.NsP, "ph")
		if ph == nil {
			continue
		}

		// Check if this placeholder is of type "body"
		phType := ph.SelectAttrValue("type", "")
		if phType == "body" {
			targetShape = sp
			break
		}
	}

	if targetShape == nil {
		return fmt.Errorf("notes slide has no body placeholder")
	}

	// Find p:txBody in the target shape
	txBody := xmlx.FindChild(targetShape, namespaces.NsP, "txBody")
	if txBody == nil {
		return fmt.Errorf("notes placeholder has no text body")
	}

	// Clear existing paragraphs and create a new one with the notes text
	// Remove all a:p (paragraph) children
	children := txBody.Child
	for i := len(children) - 1; i >= 0; i-- {
		child := children[i]
		// Only remove elements that are paragraphs (a:p)
		if elem, ok := child.(*etree.Element); ok {
			if elem.Tag == "a:p" || (elem.Space == namespaces.NsA && elem.Tag == "p") {
				txBody.RemoveChild(elem)
			}
		}
	}

	// Create a new paragraph with the notes text
	newPara := etree.NewElement("a:p")
	newPara.Space = namespaces.NsA

	// Create run (a:r)
	run := etree.NewElement("a:r")
	run.Space = namespaces.NsA

	// Create text (a:t)
	text := etree.NewElement("a:t")
	text.Space = namespaces.NsA
	text.SetText(notesText)

	run.AddChild(text)
	newPara.AddChild(run)
	txBody.AddChild(newPara)

	// Write the modified notes back
	if err := ce.outputSession.ReplaceXMLPart(slideRef.NotesPartURI, notesDoc); err != nil {
		return fmt.Errorf("failed to write notes slide: %w", err)
	}

	return nil
}

// Helper functions

// contentToString converts content to a string
func (ce *CompilerEngine) contentToString(content interface{}) string {
	switch v := content.(type) {
	case string:
		return v
	case map[string]interface{}:
		if text, ok := v["text"].(string); ok {
			return text
		}
	}
	return ""
}

// parseBulletList parses a bullet list from a string
func (ce *CompilerEngine) parseBulletList(text string) []string {
	var bullets []string
	lines := strings.Split(text, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line != "" {
			// Remove bullet markers if present
			line = strings.TrimPrefix(line, "• ")
			line = strings.TrimPrefix(line, "- ")
			line = strings.TrimPrefix(line, "* ")
			if line != "" {
				bullets = append(bullets, line)
			}
		}
	}
	return bullets
}

// loadImageData loads image data from a file
func (ce *CompilerEngine) loadImageData(imagePath string) ([]byte, error) {
	// Check cache first
	if data, ok := ce.imageCache[imagePath]; ok {
		return data, nil
	}

	// Load from file
	data, err := os.ReadFile(imagePath)
	if err != nil {
		return nil, fmt.Errorf("failed to read image file: %w", err)
	}

	// Cache for future use
	ce.imageCache[imagePath] = data

	return data, nil
}

// detectImageContentType detects supported image content types from the file extension.
func (ce *CompilerEngine) detectImageContentType(imagePath string) (string, error) {
	contentType, ok := imagex.ContentTypeFromPath(imagePath)
	if !ok {
		return "", fmt.Errorf("unsupported image type for %s; supported extensions are .png, .jpg, .jpeg, .gif, .bmp, .tif, .tiff, .webp, and .svg", imagePath)
	}
	return contentType, nil
}

// applyThemeOverrides applies theme overrides to the output presentation
func (ce *CompilerEngine) applyThemeOverrides() error {
	// Parse the output presentation to get the theme URI
	outputGraph, err := inspect.ParsePresentation(ce.outputSession)
	if err != nil {
		return fmt.Errorf("failed to parse output presentation: %w", err)
	}

	if len(outputGraph.Masters) == 0 {
		return fmt.Errorf("presentation has no master slides")
	}

	themeURI := outputGraph.Masters[0].ThemeURI
	if themeURI == "" {
		return fmt.Errorf("master slide has no theme URI")
	}

	// Apply colors from spec ThemeOverrides
	if ce.spec.ThemeOverrides != nil && len(ce.spec.ThemeOverrides.Colors) > 0 {
		for colorName, hexValue := range ce.spec.ThemeOverrides.Colors {
			req := &mutate.UpdateThemeColorRequest{
				Package:   ce.outputSession,
				ThemeURI:  themeURI,
				ColorName: colorName,
				HexValue:  hexValue,
			}
			if err := mutate.UpdateThemeColor(req); err != nil {
				return fmt.Errorf("failed to update theme color %q: %w", colorName, err)
			}
		}
	}

	// Apply fonts from spec ThemeOverrides
	if ce.spec.ThemeOverrides != nil && (ce.spec.ThemeOverrides.MajorFont != "" || ce.spec.ThemeOverrides.MinorFont != "") {
		req := &mutate.UpdateThemeFontRequest{
			Package:   ce.outputSession,
			ThemeURI:  themeURI,
			MajorFont: ce.spec.ThemeOverrides.MajorFont,
			MinorFont: ce.spec.ThemeOverrides.MinorFont,
		}
		if err := mutate.UpdateThemeFont(req); err != nil {
			return fmt.Errorf("failed to update theme fonts: %w", err)
		}
	}

	// Apply fonts from CompileOptions.ThemeOptions if provided
	if ce.options.ThemeOptions != nil && (ce.options.ThemeOptions.MajorFont != "" || ce.options.ThemeOptions.MinorFont != "") {
		req := &mutate.UpdateThemeFontRequest{
			Package:   ce.outputSession,
			ThemeURI:  themeURI,
			MajorFont: ce.options.ThemeOptions.MajorFont,
			MinorFont: ce.options.ThemeOptions.MinorFont,
		}
		if err := mutate.UpdateThemeFont(req); err != nil {
			return fmt.Errorf("failed to update theme fonts from options: %w", err)
		}
	}

	// Apply colors from CompileOptions.ThemeOptions if provided
	if ce.options.ThemeOptions != nil && len(ce.options.ThemeOptions.Colors) > 0 {
		for colorName, hexValue := range ce.options.ThemeOptions.Colors {
			req := &mutate.UpdateThemeColorRequest{
				Package:   ce.outputSession,
				ThemeURI:  themeURI,
				ColorName: colorName,
				HexValue:  hexValue,
			}
			if err := mutate.UpdateThemeColor(req); err != nil {
				return fmt.Errorf("failed to update theme color %q from options: %w", colorName, err)
			}
		}
	}

	return nil
}

// validateOutput validates the output presentation
func (ce *CompilerEngine) validateOutput() error {
	// Parse the output presentation
	graph, err := inspect.ParsePresentation(ce.outputSession)
	if err != nil {
		return fmt.Errorf("failed to parse output: %w", err)
	}

	// Basic validation
	if len(graph.Slides) == 0 {
		return fmt.Errorf("output has no slides")
	}
	if ce.spec != nil && len(graph.Slides) != len(ce.spec.Slides) {
		return fmt.Errorf("output slide count (%d) does not match spec (%d)", len(graph.Slides), len(ce.spec.Slides))
	}

	return nil
}

// addError adds an error to the error list
func (ce *CompilerEngine) addError(err *CompileError) {
	if err != nil {
		ce.errors = append(ce.errors, err)
	}
}
