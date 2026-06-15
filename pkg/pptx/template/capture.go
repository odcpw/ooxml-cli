package template

import (
	"fmt"
	"time"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// CaptureOptions contains options for template capture
type CaptureOptions struct {
	// Template name
	Name string

	// Template description
	Description string

	// Author name
	Author string

	// Organization
	Organization string

	// Template version
	Version *Version

	// Slide numbers to capture (1-indexed, or empty for all)
	SlideNumbers []int

	// Whether to be strict about static shape preservation
	StrictStaticShapes bool

	// Notes about the template
	Notes string
}

// CaptureEngine analyzes slides and produces a template manifest
type CaptureEngine struct {
	session opc.PackageSession
	graph   *inspect.PresentationGraph
	options CaptureOptions
}

// NewCaptureEngine creates a new capture engine for a presentation
func NewCaptureEngine(session opc.PackageSession, graph *inspect.PresentationGraph, options CaptureOptions) *CaptureEngine {
	return &CaptureEngine{
		session: session,
		graph:   graph,
		options: options,
	}
}

// Capture analyzes selected slides and produces a template manifest
func (ce *CaptureEngine) Capture() (*TemplateManifest, error) {
	if ce.session == nil || ce.graph == nil {
		return nil, fmt.Errorf("capture engine not properly initialized")
	}

	// Determine which slides to capture
	slidesToCapture := ce.determineSlidesToCapture()
	if len(slidesToCapture) == 0 {
		return nil, fmt.Errorf("no slides selected for capture")
	}

	// Create manifest structure
	manifest := &TemplateManifest{
		ManifestVersion: "1.0.0",
		Name:            ce.options.Name,
		Description:     ce.options.Description,
		CreatedAt:       time.Now(),
		ModifiedAt:      time.Now(),
		Author:          ce.options.Author,
		Organization:    ce.options.Organization,
		Notes:           ce.options.Notes,
		Archetypes:      []Archetype{},
	}

	// Set version
	if ce.options.Version != nil {
		manifest.Version = ce.options.Version
	} else {
		manifest.Version = &Version{
			Major:     1,
			Minor:     0,
			Patch:     0,
			CreatedAt: time.Now(),
		}
	}

	// Capture each slide as an archetype
	for i, slideNumber := range slidesToCapture {
		if slideNumber < 1 || slideNumber > len(ce.graph.Slides) {
			return nil, fmt.Errorf("slide number %d out of range", slideNumber)
		}

		slideRef := ce.graph.Slides[slideNumber-1]
		archetype, err := ce.captureSlide(&slideRef, i)
		if err != nil {
			return nil, fmt.Errorf("failed to capture slide %d: %w", slideNumber, err)
		}

		manifest.Archetypes = append(manifest.Archetypes, *archetype)
	}

	// Validate manifest
	if err := manifest.ValidateManifest(); err != nil {
		return nil, fmt.Errorf("captured manifest is invalid: %w", err)
	}

	return manifest, nil
}

// determineSlidesToCapture determines which slides to capture
func (ce *CaptureEngine) determineSlidesToCapture() []int {
	if len(ce.options.SlideNumbers) > 0 {
		return ce.options.SlideNumbers
	}

	// Capture all slides
	result := make([]int, len(ce.graph.Slides))
	for i := range ce.graph.Slides {
		result[i] = i + 1
	}
	return result
}

// captureSlide analyzes a single slide and produces an Archetype
func (ce *CaptureEngine) captureSlide(slideRef *inspect.SlideRef, index int) (*Archetype, error) {
	// Read slide XML
	slideDoc, err := ce.session.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide XML: %w", err)
	}

	// Get slide content tree
	spTree := slideDoc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
	if spTree == nil {
		return nil, fmt.Errorf("slide has no spTree element")
	}

	// Get layout information
	layoutName := ""
	layoutRef := slideRef.LayoutPartURI
	for _, layout := range ce.graph.Layouts {
		if layout.PartURI == layoutRef {
			layoutName = layout.Name
			break
		}
	}

	// Get master information
	masterName := ""
	if layoutRef != "" {
		for _, layout := range ce.graph.Layouts {
			if layout.PartURI == layoutRef {
				masterRef := layout.MasterPartURI
				if masterRef != "" {
					masterName = masterRef
				}
				break
			}
		}
	}

	// Enumerate shapes
	shapeList := inspect.EnumerateShapes(spTree)

	// Build archetype
	archetype := &Archetype{
		ID:                fmt.Sprintf("archetype-%d", slideRef.SlideNumber),
		Name:              fmt.Sprintf("Slide %d", slideRef.SlideNumber),
		LayoutName:        layoutName,
		MasterName:        masterName,
		SourceSlideNumber: slideRef.SlideNumber,
		Slots:             []Slot{},
		StaticShapes:      []StaticShape{},
	}

	// Process shapes to identify slots and static shapes
	slotIndex := 0
	for _, shape := range shapeList {
		slot, isStatic, err := ce.classifyShape(&shape, slideDoc, spTree)
		if err != nil {
			// Log but continue
			continue
		}

		if isStatic {
			// This is a static shape
			staticShape := StaticShape{
				ID:       fmt.Sprintf("shape-%d", shape.ID),
				Name:     shape.Name,
				Type:     string(shape.Type),
				Bounds:   convertBounds(shape.Bounds),
				Preserve: true,
			}
			archetype.StaticShapes = append(archetype.StaticShapes, staticShape)
		} else if slot != nil {
			// This is a fillable slot
			if slot.ID == "" {
				slot.ID = fmt.Sprintf("slot-%d", slotIndex)
				slotIndex++
			}
			if slot.Name == "" {
				slot.Name = shape.Name
			}
			if slot.Bounds == nil {
				slot.Bounds = convertBounds(shape.Bounds)
			}

			archetype.Slots = append(archetype.Slots, *slot)
		}
	}

	// If no slots were found, mark all shapes as static
	if len(archetype.Slots) == 0 {
		archetype.StaticShapes = []StaticShape{}
		for _, shape := range shapeList {
			if shape.Type != model.ShapeTypeSP || !shape.IsPlaceholder {
				staticShape := StaticShape{
					ID:       fmt.Sprintf("shape-%d", shape.ID),
					Name:     shape.Name,
					Type:     string(shape.Type),
					Bounds:   convertBounds(shape.Bounds),
					Preserve: true,
				}
				archetype.StaticShapes = append(archetype.StaticShapes, staticShape)
			}
		}

		// Still need at least one slot for a valid archetype
		if len(archetype.Slots) == 0 {
			return nil, fmt.Errorf("slide has no fillable slots or placeholders")
		}
	}

	return archetype, nil
}

// classifyShape analyzes a shape and determines if it's a slot or static shape
func (ce *CaptureEngine) classifyShape(shape *model.ShapeInfo, slideDoc *etree.Document, spTree *etree.Element) (*Slot, bool, error) {
	// Find the actual shape element in the DOM
	shapeElement := findShapeElementByID(spTree, shape.ID)
	if shapeElement == nil {
		return nil, false, fmt.Errorf("shape element not found in DOM")
	}

	// Classify based on shape type and content
	switch shape.Type {
	case model.ShapeTypePic:
		// Pictures are always fillable image slots
		slot := &Slot{
			Name:        shape.Name,
			Kind:        SlotKindImage,
			Bounds:      convertBounds(shape.Bounds),
			Required:    false,
			AspectRatio: ce.estimateAspectRatio(shape.Bounds),
		}

		// Check if placeholder
		if shape.IsPlaceholder {
			slot.PlaceholderKey = fmt.Sprintf("pic")
			slot.PlaceholderRole = "picture"
		}

		return slot, false, nil

	case model.ShapeTypeGraphicFrame:
		// Graphic frames can contain tables or other content
		return ce.classifyGraphicFrame(shape, shapeElement)

	case model.ShapeTypeSP:
		// Regular shapes - check if they're placeholders with text
		if shape.IsPlaceholder {
			return ce.classifyPlaceholder(shape, shapeElement)
		}

		// Check if shape has text body
		txBody := shapeElement.FindElement("{" + namespaces.NsP + "}txBody")
		if txBody != nil {
			// Shape with text is a text slot
			slot := &Slot{
				Name:     shape.Name,
				Kind:     SlotKindText,
				Bounds:   convertBounds(shape.Bounds),
				Required: false,
			}
			return slot, false, nil
		}

		// Shape without text or placeholder markers is static
		return nil, true, nil

	case model.ShapeTypeGroup:
		// Group shapes are typically static (decorative)
		return nil, true, nil

	default:
		return nil, true, nil
	}
}

// classifyPlaceholder analyzes a placeholder and determines its slot kind
func (ce *CaptureEngine) classifyPlaceholder(shape *model.ShapeInfo, shapeElement *etree.Element) (*Slot, bool, error) {
	// Find the placeholder element
	nvSpPr := shapeElement.FindElement("{" + namespaces.NsP + "}nvSpPr")
	if nvSpPr == nil {
		return nil, false, fmt.Errorf("placeholder has no nvSpPr")
	}

	nvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}nvPr")
	if nvPr == nil {
		return nil, false, fmt.Errorf("placeholder has no nvPr")
	}

	ph := nvPr.FindElement("{" + namespaces.NsP + "}ph")
	if ph == nil {
		return nil, false, fmt.Errorf("placeholder marker not found")
	}

	// Get placeholder type
	phType := ph.SelectAttrValue("type", "")
	phIdx := ph.SelectAttrValue("idx", "-1")

	// Determine slot kind based on placeholder type
	var kind SlotKind
	var role string

	switch phType {
	case "title":
		kind = SlotKindText
		role = "title"
	case "body":
		kind = SlotKindBullets
		role = "body"
	case "pic":
		kind = SlotKindImage
		role = "picture"
	case "tbl":
		kind = SlotKindTable
		role = "table"
	case "notes":
		kind = SlotKindNotes
		role = "notes"
	default:
		// Default to text for unknown types
		kind = SlotKindText
		role = phType
	}

	// Check text body for hints
	txBody := shapeElement.FindElement("{" + namespaces.NsP + "}txBody")
	if txBody != nil && kind == SlotKindBullets {
		// Check if this is really bulleted text by looking for list properties
		// For now, assume body placeholders are bullets if they have text
		kind = SlotKindBullets
	}

	slot := &Slot{
		Name:             shape.Name,
		Kind:             kind,
		Bounds:           convertBounds(shape.Bounds),
		Required:         true, // Placeholders are typically required
		PlaceholderKey:   role,
		PlaceholderRole:  role,
		PlaceholderIndex: phIdx,
	}

	return slot, false, nil
}

// classifyGraphicFrame analyzes a graphic frame (table, chart, etc.)
func (ce *CaptureEngine) classifyGraphicFrame(shape *model.ShapeInfo, frameElement *etree.Element) (*Slot, bool, error) {
	// First check if we already identified this as a table in shape inspection
	if shape.TableInfo != nil {
		rows := shape.TableInfo.Rows
		cols := shape.TableInfo.Cols
		tableID := shape.ID

		slot := &Slot{
			Name:      shape.Name,
			Kind:      SlotKindTable,
			Bounds:    convertBounds(shape.Bounds),
			Required:  false,
			TableRows: &rows,
			TableCols: &cols,
			TableID:   &tableID,
		}
		return slot, false, nil
	}

	// Try to find graphicData element directly
	// The structure is p:graphicFrame/p:xfrm/.../a:graphic/a:graphicData
	graphic := frameElement.FindElement("{" + namespaces.NsA + "}graphic")
	if graphic != nil {
		graphicData := graphic.FindElement("{" + namespaces.NsA + "}graphicData")
		if graphicData != nil {
			// Check if it contains a table (a:tbl)
			tbl := graphicData.FindElement("{" + namespaces.NsA + "}tbl")
			if tbl != nil {
				// This is a table
				tableInfo := inspect.ParseTable(tbl)
				if tableInfo != nil {
					tableID := shape.ID
					slot := &Slot{
						Name:      shape.Name,
						Kind:      SlotKindTable,
						Bounds:    convertBounds(shape.Bounds),
						Required:  false,
						TableRows: &tableInfo.Rows,
						TableCols: &tableInfo.Cols,
						TableID:   &tableID,
					}
					return slot, false, nil
				}
			}
		}
	}

	// Default to static if we can't determine what it is
	return nil, true, nil
}

// findShapeElementByID finds a shape element by its ID in the shape tree
func findShapeElementByID(spTree *etree.Element, id int) *etree.Element {
	if spTree == nil {
		return nil
	}

	// Search all shape types
	for _, shapeElem := range spTree.ChildElements() {
		elem := findShapeInTree(shapeElem, id)
		if elem != nil {
			return elem
		}
	}

	return nil
}

// findShapeInTree recursively searches for a shape by ID
func findShapeInTree(elem *etree.Element, id int) *etree.Element {
	if elem == nil {
		return nil
	}

	// Check if this element is a shape with the target ID
	nvSpPr := elem.FindElement("{" + namespaces.NsP + "}nvSpPr")
	if nvSpPr == nil {
		nvSpPr = elem.FindElement("{" + namespaces.NsP + "}nvPicPr")
	}
	if nvSpPr == nil {
		nvSpPr = elem.FindElement("{" + namespaces.NsP + "}nvGraphicFramePr")
	}
	if nvSpPr == nil {
		nvSpPr = elem.FindElement("{" + namespaces.NsP + "}nvGrpSpPr")
	}

	if nvSpPr != nil {
		cNvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}cNvPr")
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			var shapeID int
			_, _ = fmt.Sscanf(idStr, "%d", &shapeID)
			if shapeID == id {
				return elem
			}
		}
	}

	// Recursively search children (for group shapes)
	for _, child := range elem.ChildElements() {
		if found := findShapeInTree(child, id); found != nil {
			return found
		}
	}

	return nil
}

// estimateAspectRatio estimates the aspect ratio from bounds
func (ce *CaptureEngine) estimateAspectRatio(bounds *model.Bounds) *float64 {
	if bounds == nil || bounds.CY == 0 {
		return nil
	}

	ratio := float64(bounds.CX) / float64(bounds.CY)
	return &ratio
}

// convertBounds converts model.Bounds to template.Bounds
func convertBounds(b *model.Bounds) *Bounds {
	if b == nil {
		return nil
	}
	return &Bounds{
		X:  b.X,
		Y:  b.Y,
		CX: b.CX,
		CY: b.CY,
	}
}
