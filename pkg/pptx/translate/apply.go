package translate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/normalize"
)

// StaleEntryMode defines how to handle manifest entries with stale source text
type StaleEntryMode string

const (
	// StaleEntrySkip: Skip stale entries without applying translations
	StaleEntrySkip StaleEntryMode = "skip"

	// StaleEntryWarn: Apply translations to stale entries but log warnings
	StaleEntryWarn StaleEntryMode = "warn"

	// StaleEntryError: Treat stale entries as errors and fail application
	StaleEntryError StaleEntryMode = "error"
)

// ApplyTranslationRequest holds parameters for applying a translation manifest to a presentation
type ApplyTranslationRequest struct {
	// OPC session for reading and writing package contents
	Package opc.PackageSession

	// The translation manifest to apply
	Manifest *TranslationManifest

	// How to handle entries with stale source text (default: StaleEntrySkip)
	StaleEntryMode StaleEntryMode

	// Optional callback for reporting stale entries or other warnings
	OnWarning func(message string)
}

// ApplyTranslationResult contains the outcome of applying a translation manifest
type ApplyTranslationResult struct {
	// Total number of entries processed
	EntriesProcessed int

	// Number of entries successfully applied
	EntriesApplied int

	// Number of entries skipped (stale or other reasons)
	EntriesSkipped int

	// List of warnings encountered
	Warnings []string

	// First error if application failed completely (nil if successful)
	Error error
}

// ApplyTranslation applies a translation manifest to a presentation.
//
// Loads the manifest, verifies source-text freshness, and updates deck text
// using the rich-text writer path. Surfaces stale-manifest mismatches clearly.
//
// Parameters:
//   - req: ApplyTranslationRequest with package, manifest, and options
//
// Returns: ApplyTranslationResult with outcome information
func ApplyTranslation(req *ApplyTranslationRequest) *ApplyTranslationResult {
	result := &ApplyTranslationResult{
		Warnings: []string{},
	}

	// Validate request
	if req == nil {
		result.Error = fmt.Errorf("ApplyTranslation: request is nil")
		return result
	}

	// Set default stale entry mode early
	if req.StaleEntryMode == "" {
		req.StaleEntryMode = StaleEntrySkip
	}

	if req.Package == nil {
		result.Error = fmt.Errorf("ApplyTranslation: package is nil")
		return result
	}

	if req.Manifest == nil {
		result.Error = fmt.Errorf("ApplyTranslation: manifest is nil")
		return result
	}

	// Validate manifest before applying
	valResult := ValidateManifest(req.Manifest)
	if !valResult.IsValid() {
		var errMsgs []string
		for _, err := range valResult.Errors {
			errMsgs = append(errMsgs, err.Error())
		}
		result.Error = fmt.Errorf("manifest validation failed: %v", strings.Join(errMsgs, "; "))
		return result
	}

	// Parse the presentation to get slide and graph info
	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		result.Error = fmt.Errorf("failed to parse presentation: %w", err)
		return result
	}

	// Extract current text for freshness verification
	textReq := &extract.ExtractTextRequest{
		Session: req.Package,
		Graph:   graph,
	}

	textResult, err := extract.ExtractText(textReq)
	if err != nil {
		result.Error = fmt.Errorf("failed to extract current text: %w", err)
		return result
	}

	// Build a map of entry IDs to current text for freshness checking
	currentTextMap := buildCurrentTextMap(textResult, graph)

	// Apply each entry
	for _, entry := range req.Manifest.Entries {
		result.EntriesProcessed++

		// Parse the entry ID to get location
		slideID, shapeKey, paraIdx, runIdx, err := ParseID(entry.ID)
		if err != nil {
			warning := fmt.Sprintf("entry %s: invalid ID format - %v", entry.ID, err)
			result.Warnings = append(result.Warnings, warning)
			req.reportWarning(warning)
			result.EntriesSkipped++
			continue
		}

		// Convert slide ID (0-based) to slide number (1-based)
		slideNumber := slideID + 1

		// Validate slide exists
		if slideID < 0 || slideID >= len(graph.Slides) {
			warning := fmt.Sprintf("entry %s: slide %d out of bounds", entry.ID, slideID)
			result.Warnings = append(result.Warnings, warning)
			req.reportWarning(warning)
			result.EntriesSkipped++
			continue
		}

		// Verify source text freshness
		if entry.SourceText != "" {
			currentText, exists := currentTextMap[entry.ID]
			if !exists {
				warning := fmt.Sprintf("entry %s: text location not found in current presentation", entry.ID)
				result.Warnings = append(result.Warnings, warning)
				req.reportWarning(warning)

				if req.StaleEntryMode == StaleEntryError {
					result.Error = fmt.Errorf(warning)
					return result
				}
				result.EntriesSkipped++
				continue
			}

			if currentText != entry.SourceText {
				// Source text has changed - entry is stale
				warning := fmt.Sprintf(
					"entry %s: source text mismatch (expected %q, found %q) - entry is stale",
					entry.ID, entry.SourceText, currentText)
				result.Warnings = append(result.Warnings, warning)
				req.reportWarning(warning)

				if req.StaleEntryMode == StaleEntryError {
					result.Error = fmt.Errorf(warning)
					return result
				}
				if req.StaleEntryMode == StaleEntrySkip {
					result.EntriesSkipped++
					continue
				}
				// If Warn mode, continue and apply anyway
			}
		}

		// Skip entries with no target text (nothing to apply)
		if entry.TargetText == "" {
			result.EntriesSkipped++
			continue
		}

		// Apply the translation to this entry
		if err := applyEntryToSlide(
			req.Package,
			graph,
			slideNumber,
			shapeKey,
			paraIdx,
			runIdx,
			entry.TargetText,
		); err != nil {
			warning := fmt.Sprintf("entry %s: failed to apply translation - %v", entry.ID, err)
			result.Warnings = append(result.Warnings, warning)
			req.reportWarning(warning)

			if req.StaleEntryMode == StaleEntryError {
				result.Error = err
				return result
			}
			result.EntriesSkipped++
			continue
		}

		result.EntriesApplied++
	}

	return result
}

// buildCurrentTextMap creates a map from entry ID to current text for freshness checking
func buildCurrentTextMap(textResult *extract.TextExtractionResult, graph *inspect.PresentationGraph) map[string]string {
	textMap := make(map[string]string)

	if textResult == nil {
		return textMap
	}

	for _, extractedSlide := range textResult.Slides {
		slideIdx := extractedSlide.Slide - 1 // Convert to 0-based

		for _, shape := range extractedSlide.Shapes {
			if shape.Text == nil {
				continue
			}

			for paraIdx, paragraph := range shape.Text.Paragraphs {
				for runIdx, run := range paragraph.Runs {
					var text string

					// Extract text based on run type
					// Runs can be TextRun, Break, Tab, Field, etc.
					switch r := run.(type) {
					case *model.TextRun:
						if r != nil {
							text = r.Text
						}
					case model.TextRun:
						text = r.Text
					case *model.Break:
						text = "\n"
					case model.Break:
						text = "\n"
					case *model.Tab:
						text = "\t"
					case model.Tab:
						text = "\t"
					default:
						// Unknown run type, skip
						continue
					}

					if text != "" {
						id := GenerateEntryID(slideIdx, shape.Key, paraIdx, runIdx)
						textMap[id] = text
					}
				}
			}
		}
	}

	return textMap
}

// applyEntryToSlide updates a specific text run in a slide with the translated text
func applyEntryToSlide(
	pkg opc.PackageSession,
	graph *inspect.PresentationGraph,
	slideNumber int,
	shapeKey string,
	paraIdx, runIdx int,
	newText string,
) error {
	// Validate slide number
	if slideNumber < 1 || slideNumber > len(graph.Slides) {
		return fmt.Errorf("slide %d out of bounds", slideNumber)
	}

	slideRef := graph.Slides[slideNumber-1]

	// Handle notes translation separately
	if shapeKey == "notes" {
		return applyTextToNotes(pkg, slideRef, paraIdx, runIdx, newText)
	}

	slideURI := slideRef.PartURI

	// Read the slide XML
	slideDoc, err := pkg.ReadXMLPart(slideURI)
	if err != nil {
		return fmt.Errorf("failed to read slide: %w", err)
	}

	// Get layout for context
	layoutURI := slideRef.LayoutPartURI
	layoutDoc, err := pkg.ReadXMLPart(layoutURI)
	if err != nil {
		return fmt.Errorf("failed to read layout: %w", err)
	}

	var masterRoot *etree.Element
	for _, layoutRef := range graph.Layouts {
		if layoutRef.PartURI != layoutURI || layoutRef.MasterPartURI == "" {
			continue
		}
		masterDoc, err := pkg.ReadXMLPart(layoutRef.MasterPartURI)
		if err != nil {
			return fmt.Errorf("failed to read layout master: %w", err)
		}
		masterRoot = masterDoc.Root()
		break
	}

	// Find the target shape
	shapeElem, err := findShapeByKeyOnSlide(slideDoc.Root(), layoutDoc.Root(), masterRoot, shapeKey)
	if err != nil {
		return fmt.Errorf("failed to find shape: %w", err)
	}

	if shapeElem == nil {
		return fmt.Errorf("shape with key %q not found on slide", shapeKey)
	}

	// Navigate to the text body
	txBody := shapeElem.FindElement(".//p:txBody")
	if txBody == nil {
		return fmt.Errorf("shape %q has no text body", shapeKey)
	}

	// Get all paragraphs
	paragraphs := txBody.FindElements("a:p")
	if paraIdx < 0 || paraIdx >= len(paragraphs) {
		return fmt.Errorf(
			"paragraph index %d out of bounds (shape %q has %d paragraphs)",
			paraIdx, shapeKey, len(paragraphs))
	}

	// Get the target paragraph
	paragraph := paragraphs[paraIdx]

	// Get all runs/segments in the paragraph in document order.
	var runs []*etree.Element
	for _, child := range paragraph.ChildElements() {
		switch localTag(child.Tag) {
		case "r", "br", "fld", "tab":
			runs = append(runs, child)
		}
	}

	if runIdx < 0 || runIdx >= len(runs) {
		return fmt.Errorf(
			"run index %d out of bounds (paragraph %d has %d runs)",
			runIdx, paraIdx, len(runs))
	}

	// Get the target run
	run := runs[runIdx]

	// Update the text content based on run type
	switch localTag(run.Tag) {
	case "r":
		t := findChildByLocalName(run, "t")
		if t == nil {
			t = etree.NewElement("a:t")
			run.AddChild(t)
		}
		t.SetText(newText)
	case "br":
		return fmt.Errorf("cannot apply translation to break element (run %d)", runIdx)
	case "fld", "tab":
		if t := findChildByLocalName(run, "t"); t != nil {
			t.SetText(newText)
		}
	}

	// Save the modified slide
	if err := pkg.ReplaceXMLPart(slideURI, slideDoc); err != nil {
		return fmt.Errorf("failed to save slide: %w", err)
	}

	return nil
}

// applyTextToNotes applies text translation to notes slides
func applyTextToNotes(
	pkg opc.PackageSession,
	slideRef inspect.SlideRef,
	paraIdx, runIdx int,
	newText string,
) error {
	// Check if slide has notes
	if slideRef.NotesPartURI == "" {
		return fmt.Errorf("slide has no notes part")
	}

	// Read the notes XML
	notesDoc, err := pkg.ReadXMLPart(slideRef.NotesPartURI)
	if err != nil {
		return fmt.Errorf("failed to read notes: %w", err)
	}

	// Find the text body in notes (p:notesSlide/p:cSld/p:spTree/p:sp/p:txBody)
	root := notesDoc.Root()

	// Find the shape tree
	spTree := root.FindElement(".//p:spTree")
	if spTree == nil {
		return fmt.Errorf("shape tree not found in notes")
	}

	// Find the first shape with text body (typically the notes placeholder)
	var txBody *etree.Element
	for _, sp := range spTree.FindElements(".//p:sp") {
		potentialTxBody := sp.FindElement(".//p:txBody")
		if potentialTxBody != nil {
			txBody = potentialTxBody
			break
		}
	}

	if txBody == nil {
		return fmt.Errorf("text body not found in notes")
	}

	// Get all paragraphs
	paragraphs := txBody.FindElements(".//a:p")
	if paraIdx < 0 || paraIdx >= len(paragraphs) {
		return fmt.Errorf(
			"paragraph index %d out of bounds (notes has %d paragraphs)",
			paraIdx, len(paragraphs))
	}

	// Get the target paragraph
	paragraph := paragraphs[paraIdx]

	// Get all runs/segments in the paragraph
	var runs []*etree.Element
	for _, child := range paragraph.ChildElements() {
		switch localTag(child.Tag) {
		case "r", "br", "fld", "tab":
			runs = append(runs, child)
		}
	}

	if runIdx < 0 || runIdx >= len(runs) {
		return fmt.Errorf(
			"run index %d out of bounds (paragraph %d has %d runs)",
			runIdx, paraIdx, len(runs))
	}

	// Get the target run
	run := runs[runIdx]

	// Update the text content based on run type
	switch localTag(run.Tag) {
	case "r":
		t := findChildByLocalName(run, "t")
		if t == nil {
			t = etree.NewElement("a:t")
			run.AddChild(t)
		}
		t.SetText(newText)
	case "br":
		return fmt.Errorf("cannot apply translation to break element in notes")
	case "fld", "tab":
		if t := findChildByLocalName(run, "t"); t != nil {
			t.SetText(newText)
		}
	}

	// Save the modified notes
	if err := pkg.ReplaceXMLPart(slideRef.NotesPartURI, notesDoc); err != nil {
		return fmt.Errorf("failed to save notes: %w", err)
	}

	return nil
}

// findShapeByKeyOnSlide finds a shape on a slide by its normalized placeholder key,
// explicit shape ID, or shape name.
func findShapeByKeyOnSlide(slideRoot, layoutRoot, masterRoot *etree.Element, key string) (*etree.Element, error) {
	if key == "notes" {
		return nil, fmt.Errorf("notes apply not yet implemented via shape lookup")
	}

	spTree := slideRoot.FindElement("//p:spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found on slide")
	}

	if strings.HasPrefix(key, "shape:") {
		idStr := strings.TrimPrefix(key, "shape:")
		id, err := strconv.Atoi(idStr)
		if err != nil {
			return nil, fmt.Errorf("invalid shape ID: %s", idStr)
		}
		return findShapeByID(spTree, id), nil
	}

	if elem := findShapeByName(spTree, key); elem != nil {
		return elem, nil
	}

	slideShapes := spTree.FindElements("sp")
	if len(slideShapes) == 0 {
		slideShapes = spTree.FindElements("{http://schemas.openxmlformats.org/presentationml/2006/main}sp")
	}

	layoutShapes := []*etree.Element{}
	if layoutRoot != nil {
		layoutSpTree := layoutRoot.FindElement("//p:spTree")
		if layoutSpTree != nil {
			layoutShapes = layoutSpTree.FindElements("sp")
			if len(layoutShapes) == 0 {
				layoutShapes = layoutSpTree.FindElements("{http://schemas.openxmlformats.org/presentationml/2006/main}sp")
			}
		}
	}

	masterShapes := []*etree.Element{}
	if masterRoot != nil {
		masterSpTree := masterRoot.FindElement("//p:spTree")
		if masterSpTree != nil {
			masterShapes = masterSpTree.FindElements("sp")
			if len(masterShapes) == 0 {
				masterShapes = masterSpTree.FindElements("{http://schemas.openxmlformats.org/presentationml/2006/main}sp")
			}
		}
	}

	roleCounts := make(map[string]int)
	for _, layoutShape := range layoutShapes {
		if ph := normalize.ParsePlaceholder(layoutShape); ph != nil {
			role := normalize.CanonicalRole(ph.Type)
			if role != "" {
				roleCounts[role]++
			}
		}
	}
	layoutCtx := normalize.NewSimpleLayoutContext(roleCounts)

	for _, slideShape := range slideShapes {
		normalized := normalize.NormalizePlaceholders(&normalize.NormalizePlaceholdersRequest{
			SlideShapes:   []*etree.Element{slideShape},
			LayoutShapes:  layoutShapes,
			MasterShapes:  masterShapes,
			LayoutContext: layoutCtx,
		})
		if len(normalized) > 0 && normalized[0].Key == key {
			return slideShape, nil
		}
	}

	if !strings.Contains(key, ":") {
		for _, slideShape := range slideShapes {
			if ph := normalize.ParsePlaceholder(slideShape); ph != nil && normalize.CanonicalRole(ph.Type) == key {
				return slideShape, nil
			}
		}
	}

	return nil, fmt.Errorf("placeholder %q not found", key)
}

// findShapeByID finds a shape element by its ID attribute
func findShapeByID(spTree *etree.Element, id int) *etree.Element {
	idStr := strconv.Itoa(id)

	for _, sp := range spTree.FindElements(".//p:sp") {
		nvSpPr := sp.FindElement(".//p:nvSpPr")
		if nvSpPr == nil {
			continue
		}

		cNvPr := nvSpPr.FindElement("p:cNvPr")
		if cNvPr == nil {
			continue
		}

		if cNvPr.SelectAttrValue("id", "") == idStr {
			return sp
		}
	}

	return nil
}

// findShapeByName finds a shape element by its name attribute
func findShapeByName(spTree *etree.Element, name string) *etree.Element {
	for _, sp := range spTree.FindElements(".//p:sp") {
		nvSpPr := sp.FindElement(".//p:nvSpPr")
		if nvSpPr == nil {
			continue
		}

		cNvPr := nvSpPr.FindElement("p:cNvPr")
		if cNvPr == nil {
			continue
		}

		if cNvPr.SelectAttrValue("name", "") == name {
			return sp
		}
	}

	return nil
}

func localTag(tag string) string {
	if idx := strings.LastIndex(tag, "}"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	if idx := strings.Index(tag, ":"); idx >= 0 && idx+1 < len(tag) {
		return tag[idx+1:]
	}
	return tag
}

func findChildByLocalName(elem *etree.Element, local string) *etree.Element {
	for _, child := range elem.ChildElements() {
		if localTag(child.Tag) == local {
			return child
		}
	}
	return nil
}

// reportWarning calls the warning callback if set
func (req *ApplyTranslationRequest) reportWarning(message string) {
	if req != nil && req.OnWarning != nil {
		req.OnWarning(message)
	}
}
