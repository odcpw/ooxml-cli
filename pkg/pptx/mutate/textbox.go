package mutate

import (
	"fmt"
	"time"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

// InsertTextBoxRequest holds parameters for inserting a new text box on a slide
type InsertTextBoxRequest struct {
	// OPC package session
	Package opc.PackageSession

	// Slide reference
	SlideRef *inspect.SlideRef

	// Rich text content (paragraphs, runs, formatting)
	RichText *model.TextBlockInfo

	// Position and size in EMUs (English Metric Units)
	X  int64 // Left position
	Y  int64 // Top position
	CX int64 // Width
	CY int64 // Height

	// Optional: insert after this shape ID (0 = append to end)
	InsertAfterID int

	// Optional: body properties (margins, anchor, word wrap, etc.)
	BodyProperties *model.TextBodyProperties

	// Optional: shape name (if not provided, uses "TextBox_N")
	ShapeName string
}

// InsertTextBoxResult holds the result of a successful text box insertion
type InsertTextBoxResult struct {
	// Unique shape ID assigned to the text box
	ShapeID int

	// Shape name (either provided or auto-generated)
	ShapeName string

	// Timestamp when the text box was created
	CreatedAt time.Time
}

// InsertTextBox creates a new text box shape on a slide at specific EMU coordinates.
// It handles:
// - Creating a shape element with text body and rich text content
// - Allocating a unique shape ID and name
// - Setting geometry (position and size)
// - Applying body properties and default styling
// - Inserting into the shape tree at the specified position
// - Preserving unrelated shape state
func InsertTextBox(req *InsertTextBoxRequest) (*InsertTextBoxResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference cannot be nil")
	}
	if req.RichText == nil {
		return nil, fmt.Errorf("rich text content cannot be nil")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("text box dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	// Read the slide XML
	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	// Get the shape tree
	spTree := slideDoc.FindElement(".//p:spTree")
	if spTree == nil {
		spTree = slideDoc.FindElement(".//spTree")
		if spTree == nil {
			return nil, fmt.Errorf("shape tree not found in slide")
		}
	}

	// Determine the new shape ID across every existing cNvPr in the tree.
	newShapeID := nextSpTreeShapeID(spTree)

	// Determine shape name
	shapeName := req.ShapeName
	if shapeName == "" {
		shapeName = fmt.Sprintf("TextBox %d", newShapeID)
	}

	// Create the shape element
	shapeElem := createTextBoxElement(
		newShapeID,
		shapeName,
		req.X, req.Y, req.CX, req.CY,
		req.RichText,
		req.BodyProperties,
	)

	// Insert into shape tree (after specified shape or before the schema-tail extLst).
	insertSpTreeChildAfterShapeID(spTree, shapeElem, req.InsertAfterID)

	// Write the slide back
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &InsertTextBoxResult{
		ShapeID:   newShapeID,
		ShapeName: shapeName,
		CreatedAt: time.Now().UTC(),
	}, nil
}

// createTextBoxElement creates a shape element (p:sp) with text body and rich text content
func createTextBoxElement(
	shapeID int,
	shapeName string,
	x, y, cx, cy int64,
	richText *model.TextBlockInfo,
	bodyProps *model.TextBodyProperties,
) *etree.Element {
	// Create the main shape element
	shape := etree.NewElement("p:sp")

	// Create nvSpPr (non-visual shape properties)
	nvSpPr := etree.NewElement("p:nvSpPr")
	shape.AddChild(nvSpPr)

	// cNvPr: common non-visual properties
	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", fmt.Sprintf("%d", shapeID))
	cNvPr.CreateAttr("name", shapeName)
	nvSpPr.AddChild(cNvPr)

	// cNvSpPr: common non-visual shape properties
	cNvSpPr := etree.NewElement("p:cNvSpPr")
	nvSpPr.AddChild(cNvSpPr)

	// nvPr: non-visual properties (no placeholder for text boxes)
	nvPr := etree.NewElement("p:nvPr")
	nvSpPr.AddChild(nvPr)

	// Create spPr (shape properties with transform and geometry)
	spPr := etree.NewElement("p:spPr")
	shape.AddChild(spPr)

	// xfrm: transform with position and size
	xfrm := etree.NewElement("a:xfrm")
	spPr.AddChild(xfrm)

	// off: offset (position)
	off := etree.NewElement("a:off")
	off.CreateAttr("x", fmt.Sprintf("%d", x))
	off.CreateAttr("y", fmt.Sprintf("%d", y))
	xfrm.AddChild(off)

	// ext: extension (size)
	ext := etree.NewElement("a:ext")
	ext.CreateAttr("cx", fmt.Sprintf("%d", cx))
	ext.CreateAttr("cy", fmt.Sprintf("%d", cy))
	xfrm.AddChild(ext)

	// prstGeom: preset geometry (rectangle)
	prstGeom := etree.NewElement("a:prstGeom")
	prstGeom.CreateAttr("prst", "rect")
	spPr.AddChild(prstGeom)

	// avLst: adjust value list (empty for rect)
	avLst := etree.NewElement("a:avLst")
	prstGeom.AddChild(avLst)

	// Create txBody (text body)
	txBody := etree.NewElement("p:txBody")
	shape.AddChild(txBody)

	// bodyPr: body properties
	bodyPr := etree.NewElement("a:bodyPr")
	// Set default values for text box
	bodyPr.CreateAttr("anchor", "t")        // Top anchor
	bodyPr.CreateAttr("anchorCtr", "false") // Don't center
	bodyPr.CreateAttr("wrap", "square")     // Square text wrapping
	bodyPr.CreateAttr("rtlCol", "false")    // Not RTL

	// Apply custom body properties if provided
	if bodyProps != nil {
		if bodyProps.Anchor != "" {
			bodyPr.RemoveAttr("anchor")
			bodyPr.CreateAttr("anchor", bodyProps.Anchor)
		}
		if bodyProps.Wrap != "" {
			bodyPr.RemoveAttr("wrap")
			bodyPr.CreateAttr("wrap", bodyProps.Wrap)
		}
	}

	txBody.AddChild(bodyPr)

	// lstStyle: list style (empty list for default)
	lstStyle := etree.NewElement("a:lstStyle")
	txBody.AddChild(lstStyle)

	// Add paragraphs from rich text content
	if richText != nil && len(richText.Paragraphs) > 0 {
		for _, para := range richText.Paragraphs {
			pElem := createParagraphElement(&para)
			txBody.AddChild(pElem)
		}
	} else {
		// Create empty paragraph if no text provided
		pElem := etree.NewElement("a:p")
		txBody.AddChild(pElem)
	}

	return shape
}

// createParagraphElement creates an a:p element from a model.Paragraph
func createParagraphElement(para *model.Paragraph) *etree.Element {
	p := etree.NewElement("a:p")

	// Add paragraph properties if present
	if para.Properties != nil {
		pPr := createParagraphPropertiesElement(para.Properties)
		p.AddChild(pPr)
	}

	// Add runs
	for _, run := range para.Runs {
		switch r := run.(type) {
		case *model.TextRun:
			if r != nil {
				rElem := createTextRunElement(r)
				p.AddChild(rElem)
			}
		case model.TextRun:
			rElem := createTextRunElement(&r)
			p.AddChild(rElem)
		case *model.Break:
			if r != nil {
				br := etree.NewElement("a:br")
				if r.Properties != nil {
					rPr := createRunPropertiesElement(r.Properties)
					br.InsertChildAt(0, rPr)
				}
				p.AddChild(br)
			}
		case model.Break:
			br := etree.NewElement("a:br")
			if r.Properties != nil {
				rPr := createRunPropertiesElement(r.Properties)
				br.InsertChildAt(0, rPr)
			}
			p.AddChild(br)
		}
	}

	// Ensure at least one run or end paragraph run if empty
	if len(p.ChildElements()) == 0 {
		rElem := etree.NewElement("a:r")
		rElem.AddChild(etree.NewElement("a:rPr"))
		t := etree.NewElement("a:t")
		t.SetText("")
		rElem.AddChild(t)
		p.AddChild(rElem)
	}

	// Add end paragraph run (a:endParaRPr)
	endParaRPr := etree.NewElement("a:endParaRPr")
	endParaRPr.CreateAttr("lang", "en-US")
	endParaRPr.CreateAttr("sz", "1800")
	p.AddChild(endParaRPr)

	return p
}

// createTextRunElement creates an a:r element from a model.TextRun
func createTextRunElement(run *model.TextRun) *etree.Element {
	r := etree.NewElement("a:r")

	// Add run properties if present
	if run.Properties != nil {
		rPr := createRunPropertiesElement(run.Properties)
		r.AddChild(rPr)
	} else {
		// Add empty run properties
		r.AddChild(etree.NewElement("a:rPr"))
	}

	// Add text
	t := etree.NewElement("a:t")
	t.SetText(run.Text)
	r.AddChild(t)

	return r
}

// createRunPropertiesElement creates an a:rPr element from model.RunProperties
func createRunPropertiesElement(props *model.RunProperties) *etree.Element {
	rPr := etree.NewElement("a:rPr")

	// Set language
	lang := "en-US"
	if props.Language != "" {
		lang = props.Language
	}
	rPr.CreateAttr("lang", lang)

	// Set font size (in hundredths of a point)
	if props.FontSize != nil {
		sz := int32(*props.FontSize * 100)
		rPr.CreateAttr("sz", fmt.Sprintf("%d", sz))
	} else {
		rPr.CreateAttr("sz", "1800") // Default 18pt
	}

	// Set bold
	if props.Bold != nil && *props.Bold {
		rPr.CreateAttr("b", "1")
	}

	// Set italic
	if props.Italic != nil && *props.Italic {
		rPr.CreateAttr("i", "1")
	}

	// Set font family
	if props.FontFamily != "" {
		latin := etree.NewElement("a:latin")
		latin.CreateAttr("typeface", props.FontFamily)
		insertRPrChild(rPr, latin)
	}

	// Set color if provided
	if props.Color != "" {
		solidFill := etree.NewElement("a:solidFill")
		srgbClr := etree.NewElement("a:srgbClr")
		srgbClr.CreateAttr("val", props.Color)
		solidFill.AddChild(srgbClr)
		insertRPrChild(rPr, solidFill)
	}

	return rPr
}

// createParagraphPropertiesElement creates an a:pPr element from model.ParagraphProperties
func createParagraphPropertiesElement(props *model.ParagraphProperties) *etree.Element {
	pPr := etree.NewElement("a:pPr")

	// Set alignment if provided
	if props.Alignment != "" {
		pPr.CreateAttr("algn", props.Alignment)
	}

	// Set level (indent)
	if props.Level != nil {
		pPr.CreateAttr("lvl", fmt.Sprintf("%d", *props.Level))
	}

	return pPr
}
