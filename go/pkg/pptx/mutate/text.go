package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/normalize"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// ReplaceTextRequest holds parameters for text replacement
type ReplaceTextRequest struct {
	// Package session
	Package opc.PackageSession

	// Slide number (1-based)
	SlideNumber int

	// Target selector (e.g., "title", "shape:5", "~MyShape", "@body")
	Target string

	// New text content (used for plain-text and preserve-format modes)
	NewText string

	// Replacement mode: "plain-text" (default), "preserve-format", or "rich-text"
	Mode string

	// Rich text input (used when Mode is "rich-text")
	RichText *model.TextBlockInfo

	// Paragraph mutation options (optional)
	ParagraphOptions *ParagraphMutationOptions
	BulletOptions    *BulletMutationOptions
}

// ReplaceText replaces the text content of a targeted shape/placeholder
// Supports three modes: plain-text (default), preserve-format, and rich-text
//
// Target accepts every legacy selector (placeholder key, shape:<id>, ~name,
// @type, #idx) AND, as an additive first branch, a stable shape handle
// (H:pptx/s:<sldId>/shape:n:<cNvPr id>). When a handle is supplied it is
// authoritative for its own slide scope: the handle's sldId selects the slide
// (by SEARCHING for the native id), so req.SlideNumber is ignored for
// resolution and the edit lands on the same shape even after unrelated slides
// have been inserted, deleted, or reordered.
func ReplaceText(req *ReplaceTextRequest) error {
	if req.Target == "" {
		return fmt.Errorf("target selector cannot be empty")
	}

	// Default to plain-text mode
	if req.Mode == "" {
		req.Mode = "plain-text"
	}

	var (
		catalog        *selectors.SlideCatalog
		resolvedTarget *selectors.SlideSelectorTarget
		shapeElem      *etree.Element
		err            error
	)

	if handle.IsHandle(req.Target) {
		// Handle path: scope comes from the handle, not from req.SlideNumber.
		catalog, resolvedTarget, shapeElem, err = selectors.ResolvePPTXShapeHandle(req.Package, req.Target)
		if err != nil {
			return err
		}
	} else {
		if req.SlideNumber < 1 {
			return fmt.Errorf("invalid slide number: %d (must be >= 1)", req.SlideNumber)
		}
		// Build the published selector catalog for this slide and resolve the target through it.
		catalog, err = selectors.BuildSlideCatalog(req.Package, req.SlideNumber)
		if err != nil {
			return err
		}
		resolvedTarget, shapeElem, err = catalog.ResolveTargetElement(req.Target)
		if err != nil {
			return err
		}
	}
	if resolvedTarget == nil || shapeElem == nil {
		return fmt.Errorf("target not found: %s", req.Target)
	}
	if !resolvedTarget.TextCapable {
		return fmt.Errorf("target %s resolves to a non-text %s shape", req.Target, resolvedTarget.TargetKind)
	}

	slideURI := catalog.SlidePartURI
	slideDoc := catalog.SlideDocument()

	// Apply text replacement based on mode
	switch req.Mode {
	case "plain-text":
		// Plain text mode: replace all content with a single bare paragraph
		if err := replaceShapeText(shapeElem, req.NewText); err != nil {
			return err
		}
	case "preserve-format":
		// Preserve-format mode: update text while keeping structure/formatting
		if err := replaceShapeTextPreserveFormat(shapeElem, req.NewText); err != nil {
			return err
		}
	case "rich-text":
		// Rich text mode: apply full structured text with properties
		if req.RichText == nil {
			return fmt.Errorf("rich-text mode requires RichText field to be set")
		}
		if err := applyRichTextToShape(shapeElem, req.RichText); err != nil {
			return err
		}
	default:
		return fmt.Errorf("unknown replacement mode: %s (must be 'plain-text', 'preserve-format', or 'rich-text')", req.Mode)
	}

	// Apply paragraph options (level, alignment, spacing) if provided
	if req.ParagraphOptions != nil || req.BulletOptions != nil {
		txBody := shapeElem.FindElement("txBody")
		if txBody != nil {
			// Apply options to all paragraphs in the shape
			paragraphs := txBody.FindElements("p")
			for _, p := range paragraphs {
				if req.ParagraphOptions != nil {
					if err := ApplyParagraphOptions(p, req.ParagraphOptions); err != nil {
						return fmt.Errorf("failed to apply paragraph options: %w", err)
					}
				}
				if req.BulletOptions != nil {
					if err := ApplyBulletOptions(p, req.BulletOptions); err != nil {
						return fmt.Errorf("failed to apply bullet options: %w", err)
					}
				}
			}
		}
	}

	// Save the modified slide
	if err := req.Package.ReplaceXMLPart(slideURI, slideDoc); err != nil {
		return fmt.Errorf("failed to save slide: %w", err)
	}

	return nil
}

// findTargetShape finds the shape element matching the target selector
func findTargetShape(slideRoot, layoutRoot, masterRoot *etree.Element, target string) (*etree.Element, error) {
	// Parse the target selector
	selector, err := selectors.Parse(target)
	if err != nil {
		return nil, fmt.Errorf("invalid selector: %w", err)
	}

	// Get the shape tree from the slide
	spTree := slideRoot.FindElement("//p:spTree")
	if spTree == nil {
		return nil, fmt.Errorf("no shape tree found on slide")
	}

	// Resolve the selector to find matching shapes
	switch sel := selector.(type) {
	case *selectors.ShapeIDSelector:
		// Match by shape ID
		return findShapeByID(spTree, sel.ID), nil

	case *selectors.ShapeNameSelector:
		// Match by shape name
		return findShapeByName(spTree, sel.Name), nil

	case *selectors.PlaceholderKeySelector:
		// Match placeholder by normalized key
		return findPlaceholderByKey(slideRoot, layoutRoot, masterRoot, spTree, sel.Key)

	case *selectors.PlaceholderTypeSelector:
		// Match placeholder by type/role
		return findPlaceholderByType(slideRoot, layoutRoot, spTree, sel.Role)

	case *selectors.PlaceholderIndexSelector:
		// Match placeholder by raw index
		return findPlaceholderByIndex(spTree, sel.Index), nil

	default:
		return nil, fmt.Errorf("unsupported selector type: %T", selector)
	}
}

// findShapeByID finds a shape by its ID attribute
func findShapeByID(spTree *etree.Element, id int) *etree.Element {
	idStr := strconv.Itoa(id)

	// Search p:sp elements
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr != nil {
			cNvPr := nvSpPr.FindElement("cNvPr")
			if cNvPr != nil && cNvPr.SelectAttrValue("id", "") == idStr {
				return sp
			}
		}
	}

	// Search p:pic elements
	for _, pic := range spTree.FindElements("pic") {
		nvPicPr := pic.FindElement("nvPicPr")
		if nvPicPr != nil {
			cNvPr := nvPicPr.FindElement("cNvPr")
			if cNvPr != nil && cNvPr.SelectAttrValue("id", "") == idStr {
				return pic
			}
		}
	}

	// Search p:graphicFrame elements
	for _, gf := range spTree.FindElements("graphicFrame") {
		nvGraphicFramePr := gf.FindElement("nvGraphicFramePr")
		if nvGraphicFramePr != nil {
			cNvPr := nvGraphicFramePr.FindElement("cNvPr")
			if cNvPr != nil && cNvPr.SelectAttrValue("id", "") == idStr {
				return gf
			}
		}
	}

	return nil
}

// findShapeByName finds a shape by its name attribute
func findShapeByName(spTree *etree.Element, name string) *etree.Element {
	// Search p:sp elements
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr != nil {
			cNvPr := nvSpPr.FindElement("cNvPr")
			if cNvPr != nil && cNvPr.SelectAttrValue("name", "") == name {
				return sp
			}
		}
	}

	// Search p:pic elements
	for _, pic := range spTree.FindElements("pic") {
		nvPicPr := pic.FindElement("nvPicPr")
		if nvPicPr != nil {
			cNvPr := nvPicPr.FindElement("cNvPr")
			if cNvPr != nil && cNvPr.SelectAttrValue("name", "") == name {
				return pic
			}
		}
	}

	// Search p:graphicFrame elements
	for _, gf := range spTree.FindElements("graphicFrame") {
		nvGraphicFramePr := gf.FindElement("nvGraphicFramePr")
		if nvGraphicFramePr != nil {
			cNvPr := nvGraphicFramePr.FindElement("cNvPr")
			if cNvPr != nil && cNvPr.SelectAttrValue("name", "") == name {
				return gf
			}
		}
	}

	return nil
}

// findPlaceholderByIndex finds a placeholder by its raw index (p:ph@idx)
func findPlaceholderByIndex(spTree *etree.Element, idx int) *etree.Element {
	idxStr := strconv.Itoa(idx)

	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr != nil {
			nvPr := nvSpPr.FindElement("nvPr")
			if nvPr != nil {
				ph := nvPr.FindElement("ph")
				if ph != nil && ph.SelectAttrValue("idx", "") == idxStr {
					return sp
				}
			}
		}
	}

	return nil
}

// parseShapesForRawPlaceholders extracts raw placeholder attributes from shape elements.
func parseShapesForRawPlaceholders(shapes []*etree.Element) []*model.RawPlaceholder {
	var placeholders []*model.RawPlaceholder
	for _, shape := range shapes {
		if ph := normalize.ParsePlaceholder(shape); ph != nil {
			placeholders = append(placeholders, ph)
		}
	}
	return placeholders
}

// findPlaceholderByKey finds a placeholder by its normalized key.
// This uses the placeholder normalization pipeline so keys like body:1 match
// the same contract exposed by extract/show commands.
func findPlaceholderByKey(slideRoot, layoutRoot, masterRoot *etree.Element, spTree *etree.Element, key string) (*etree.Element, error) {
	if strings.HasPrefix(key, "shape:") {
		idStr := strings.TrimPrefix(key, "shape:")
		id, err := strconv.Atoi(idStr)
		if err != nil {
			return nil, fmt.Errorf("invalid shape ID in key: %s", key)
		}
		return findShapeByID(spTree, id), nil
	}

	if strings.HasPrefix(key, "ph:") {
		idxStr := strings.TrimPrefix(key, "ph:")
		idx, err := strconv.Atoi(idxStr)
		if err != nil {
			return nil, fmt.Errorf("invalid placeholder index in key: %s", key)
		}
		return findPlaceholderByIndex(spTree, idx), nil
	}

	slideShapes := spTree.FindElements("sp")
	if len(slideShapes) == 0 {
		slideShapes = spTree.FindElements("{" + slideRoot.NamespaceURI() + "}sp")
	}

	layoutShapes := []*etree.Element{}
	if layoutRoot != nil {
		layoutSpTree := layoutRoot.FindElement("//p:spTree")
		if layoutSpTree == nil {
			layoutSpTree = layoutRoot.FindElement("{" + layoutRoot.NamespaceURI() + "}cSld/{" + layoutRoot.NamespaceURI() + "}spTree")
		}
		if layoutSpTree != nil {
			layoutShapes = layoutSpTree.FindElements("sp")
			if len(layoutShapes) == 0 {
				layoutShapes = layoutSpTree.FindElements("{" + layoutRoot.NamespaceURI() + "}sp")
			}
		}
	}

	masterShapes := []*etree.Element{}
	if masterRoot != nil {
		masterSpTree := masterRoot.FindElement("//p:spTree")
		if masterSpTree == nil {
			masterSpTree = masterRoot.FindElement("{" + masterRoot.NamespaceURI() + "}cSld/{" + masterRoot.NamespaceURI() + "}spTree")
		}
		if masterSpTree != nil {
			masterShapes = masterSpTree.FindElements("sp")
			if len(masterShapes) == 0 {
				masterShapes = masterSpTree.FindElements("{" + masterRoot.NamespaceURI() + "}sp")
			}
		}
	}

	// Build roleCounts from resolved (not raw) layout placeholder types
	// by resolving through master if layout placeholder lacks a type attribute
	layoutPlaceholders := parseShapesForRawPlaceholders(layoutShapes)
	masterPlaceholders := parseShapesForRawPlaceholders(masterShapes)

	roleCounts := make(map[string]int)
	for _, layoutPh := range layoutPlaceholders {
		if layoutPh == nil {
			continue
		}

		// Start with the layout placeholder's type
		resolvedType := layoutPh.Type

		// If layout type is missing and we have an idx, try to inherit from master
		if resolvedType == "" && layoutPh.Idx >= 0 {
			for _, masterPh := range masterPlaceholders {
				if masterPh.Idx == layoutPh.Idx && masterPh.Type != "" {
					resolvedType = masterPh.Type
					break
				}
			}
		}

		role := normalize.CanonicalRole(resolvedType)
		if role != "" {
			roleCounts[role]++
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

	if literal := findPlaceholderByLiteralKey(slideShapes, key); literal != nil {
		return literal, nil
	}

	if !strings.Contains(key, ":") {
		return findPlaceholderByType(slideRoot, layoutRoot, spTree, key)
	}

	return nil, nil
}

// findPlaceholderByType finds a placeholder by its type/role
func findPlaceholderByType(slideRoot, layoutRoot *etree.Element, spTree *etree.Element, role string) (*etree.Element, error) {
	// Search for a placeholder with matching canonical role or literal type.
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr != nil {
			nvPr := nvSpPr.FindElement("nvPr")
			if nvPr != nil {
				ph := nvPr.FindElement("ph")
				if ph != nil {
					phType := ph.SelectAttrValue("type", "")
					canonicalRole := normalize.CanonicalRole(phType)
					if canonicalRole == role || phType == role {
						return sp, nil
					}
				}
			}
		}
	}

	return nil, nil
}

func findPlaceholderByLiteralKey(shapes []*etree.Element, key string) *etree.Element {
	for _, shape := range shapes {
		raw := normalize.ParsePlaceholder(shape)
		if raw == nil {
			continue
		}
		literalKey := raw.Type
		if raw.Idx >= 0 {
			literalKey = fmt.Sprintf("%s:%d", raw.Type, raw.Idx)
		}
		if literalKey == key {
			return shape
		}
	}
	return nil
}

// replaceShapeText replaces the text content of a shape element
func replaceShapeText(shapeElem *etree.Element, newText string) error {
	// Find the text body element (p:txBody for shapes, etc.)
	var txBody *etree.Element

	// For p:sp (shapes)
	if shapeElem.Tag == "sp" || strings.HasSuffix(shapeElem.Tag, "}sp") {
		txBody = shapeElem.FindElement("txBody")
	}

	// If txBody not found, can't modify text
	if txBody == nil {
		return fmt.Errorf("shape does not have a text body")
	}

	// Clear existing paragraphs and create a new one
	// First, remove all existing a:p elements
	for {
		p := txBody.FindElement("p")
		if p == nil {
			break
		}
		txBody.RemoveChild(p)
	}

	// Create a new paragraph with the new text
	newP := etree.NewElement("a:p")

	// Create a text run
	r := etree.NewElement("a:r")

	// Create the text element with the new content
	t := etree.NewElement("a:t")
	t.SetText(newText)

	// Assemble: a:p -> a:r -> a:t
	r.AddChild(t)
	newP.AddChild(r)

	// Add to text body
	txBody.AddChild(newP)

	return nil
}

// replaceShapeTextPreserveFormat replaces text while preserving paragraph structure and formatting
func replaceShapeTextPreserveFormat(shapeElem *etree.Element, newText string) error {
	// Find the text body element
	var txBody *etree.Element

	if shapeElem.Tag == "sp" || strings.HasSuffix(shapeElem.Tag, "}sp") {
		txBody = shapeElem.FindElement("txBody")
	}

	if txBody == nil {
		return fmt.Errorf("shape does not have a text body")
	}

	// Get all existing paragraphs
	paragraphs := txBody.FindElements("p")
	if len(paragraphs) == 0 {
		// No paragraphs exist, fall back to plain-text mode
		return replaceShapeText(shapeElem, newText)
	}

	// Split newText into lines (one per paragraph if multiple paragraphs exist)
	textLines := strings.Split(newText, "\n")

	// Update existing paragraphs with the new text while preserving structure
	for i, p := range paragraphs {
		if i < len(textLines) {
			// Update this paragraph with the corresponding text line
			if err := updateParagraphTextPreserveFormatting(p, textLines[i]); err != nil {
				return err
			}
		} else {
			// Remove extra paragraphs beyond the input lines
			txBody.RemoveChild(p)
		}
	}

	// If there are more text lines than paragraphs, add new paragraphs
	if len(textLines) > len(paragraphs) {
		for i := len(paragraphs); i < len(textLines); i++ {
			newP := etree.NewElement("a:p")
			r := etree.NewElement("a:r")
			t := etree.NewElement("a:t")
			t.SetText(textLines[i])
			r.AddChild(t)
			newP.AddChild(r)
			txBody.AddChild(newP)
		}
	}

	return nil
}

// updateParagraphTextPreserveFormatting updates the text content of a paragraph while preserving formatting
func updateParagraphTextPreserveFormatting(p *etree.Element, newText string) error {
	// Find all text runs (a:r) in the paragraph
	runs := p.FindElements("r")

	if len(runs) == 0 {
		// No runs exist, recreate with plain text
		clearParagraphText(p)
		r := etree.NewElement("a:r")
		t := etree.NewElement("a:t")
		t.SetText(newText)
		r.AddChild(t)
		p.AddChild(r)
		return nil
	}

	// If only one run exists, just update its text
	if len(runs) == 1 {
		run := runs[0]
		// Clear existing text elements
		for {
			t := run.FindElement("t")
			if t == nil {
				break
			}
			run.RemoveChild(t)
		}
		// Add new text
		t := etree.NewElement("a:t")
		t.SetText(newText)
		run.AddChild(t)
		return nil
	}

	// Multiple runs exist: distribute new text among them while preserving properties
	// Strategy: put all text in first run, clear others
	firstRun := runs[0]

	// Clear text in first run
	for {
		t := firstRun.FindElement("t")
		if t == nil {
			break
		}
		firstRun.RemoveChild(t)
	}

	// Add new text to first run
	t := etree.NewElement("a:t")
	t.SetText(newText)
	firstRun.AddChild(t)

	// Clear text from other runs (but keep them for structure)
	for i := 1; i < len(runs); i++ {
		for {
			t := runs[i].FindElement("t")
			if t == nil {
				break
			}
			runs[i].RemoveChild(t)
		}
	}

	return nil
}

// clearParagraphText removes all text elements from a paragraph while preserving structure
func clearParagraphText(p *etree.Element) {
	for {
		t := p.FindElement("t")
		if t == nil {
			break
		}
		parent := t.Parent()
		if parent != nil {
			parent.RemoveChild(t)
		}
	}
}

// applyRichTextToShape applies rich, structured text to a shape
func applyRichTextToShape(shapeElem *etree.Element, richText *model.TextBlockInfo) error {
	// Find the text body element
	var txBody *etree.Element

	if shapeElem.Tag == "sp" || strings.HasSuffix(shapeElem.Tag, "}sp") {
		txBody = shapeElem.FindElement("txBody")
	}

	if txBody == nil {
		return fmt.Errorf("shape does not have a text body")
	}

	// Clear all existing paragraphs
	for {
		p := txBody.FindElement("p")
		if p == nil {
			break
		}
		txBody.RemoveChild(p)
	}

	// Write each paragraph from richText
	for _, para := range richText.Paragraphs {
		pElem := writeParagraphFromModel(&para)
		if pElem != nil {
			txBody.AddChild(pElem)
		}
	}

	// If no paragraphs were provided, create a default one
	if len(richText.Paragraphs) == 0 && richText.PlainText != "" {
		newP := etree.NewElement("a:p")
		r := etree.NewElement("a:r")
		t := etree.NewElement("a:t")
		t.SetText(richText.PlainText)
		r.AddChild(t)
		newP.AddChild(r)
		txBody.AddChild(newP)
	}

	return nil
}

// writeParagraphFromModel creates an a:p element from a Paragraph model
func writeParagraphFromModel(para *model.Paragraph) *etree.Element {
	pElem := etree.NewElement("a:p")

	// Write paragraph properties if present
	if para.Properties != nil {
		writeParagraphPropertiesElement(pElem, para.Properties)
	}

	// Write each segment from Segments field if available
	if len(para.Segments) > 0 {
		for _, seg := range para.Segments {
			elemFromSegment := writeSegmentElement(&seg)
			if elemFromSegment != nil {
				pElem.AddChild(elemFromSegment)
			}
		}
	} else if len(para.Runs) > 0 {
		// Fallback to legacy Runs field
		for _, runIface := range para.Runs {
			if run, ok := runIface.(model.TextRun); ok {
				rElem := writeTextRunElement(&run)
				pElem.AddChild(rElem)
			}
		}
	} else if para.Text != "" {
		// Create a simple run with the paragraph text
		r := etree.NewElement("a:r")
		t := etree.NewElement("a:t")
		t.SetText(para.Text)
		r.AddChild(t)
		pElem.AddChild(r)
	}

	return pElem
}

// writeParagraphPropertiesElement creates a:pPr element from ParagraphProperties
func writeParagraphPropertiesElement(pElem *etree.Element, props *model.ParagraphProperties) {
	pPr := etree.NewElement("a:pPr")

	// Set level if present
	if props.Level != nil {
		pPr.CreateAttr("lvl", strconv.FormatInt(int64(*props.Level), 10))
	}

	// Set alignment if present
	if props.Alignment != "" {
		pPr.CreateAttr("algn", props.Alignment)
	}

	// Set bullet properties
	if props.BulletMode == "buChar" && props.BulletCharacter != "" {
		buChar := etree.NewElement("a:buChar")
		buChar.CreateAttr("char", props.BulletCharacter)
		pPr.AddChild(buChar)
	} else if props.BulletMode == "buAutoNum" && props.AutoNumberingScheme != "" {
		buAutoNum := etree.NewElement("a:buAutoNum")
		buAutoNum.CreateAttr("type", props.AutoNumberingScheme)
		pPr.AddChild(buAutoNum)
	} else if props.BulletMode == "buNone" {
		buNone := etree.NewElement("a:buNone")
		pPr.AddChild(buNone)
	}

	// Add to paragraph if it has content
	if pPr.ChildElements() != nil || pPr.Attr != nil {
		children := pElem.ChildElements()
		if len(children) > 0 {
			pElem.InsertChildAt(children[0].Index(), pPr)
		} else {
			pElem.AddChild(pPr)
		}
	}
}

// writeSegmentElement creates an XML element from a TextSegment
func writeSegmentElement(seg *model.TextSegment) *etree.Element {
	switch seg.Type {
	case model.SegmentText:
		return writeTextRunElement(&model.TextRun{
			Type:       "text",
			Text:       seg.Text,
			Properties: seg.Properties,
		})
	case model.SegmentBreak:
		brElem := etree.NewElement("a:br")
		if seg.Properties != nil {
			rPr := writeRunPropertiesElement(seg.Properties)
			brElem.AddChild(rPr)
		}
		return brElem
	case model.SegmentTab:
		tabElem := etree.NewElement("a:tab")
		if seg.Properties != nil {
			rPr := writeRunPropertiesElement(seg.Properties)
			tabElem.AddChild(rPr)
		}
		return tabElem
	case model.SegmentField:
		fldElem := etree.NewElement("a:fld")
		tElem := etree.NewElement("a:t")
		tElem.SetText(seg.Text)
		fldElem.AddChild(tElem)
		if seg.Properties != nil {
			rPr := writeRunPropertiesElement(seg.Properties)
			fldElem.AddChild(rPr)
		}
		return fldElem
	}
	return nil
}

// writeTextRunElement creates an a:r element from a TextRun
func writeTextRunElement(run *model.TextRun) *etree.Element {
	rElem := etree.NewElement("a:r")

	// Add run properties if present
	if run.Properties != nil {
		rPr := writeRunPropertiesElement(run.Properties)
		rElem.AddChild(rPr)
	}

	// Add text element
	tElem := etree.NewElement("a:t")
	tElem.SetText(run.Text)
	rElem.AddChild(tElem)

	return rElem
}

// writeRunPropertiesElement creates an a:rPr element from RunProperties
func writeRunPropertiesElement(props *model.RunProperties) *etree.Element {
	rPr := etree.NewElement("a:rPr")

	// Set font size if present
	if props.FontSize != nil {
		rPr.CreateAttr("sz", strconv.FormatInt(int64(*props.FontSize), 10))
	}

	// Set bold/italic
	if props.Bold != nil && *props.Bold {
		rPr.CreateAttr("b", "1")
	}
	if props.Italic != nil && *props.Italic {
		rPr.CreateAttr("i", "1")
	}

	// Set underline
	if props.Underline != "" {
		rPr.CreateAttr("u", props.Underline)
	}

	// Set strike
	if props.Strike != "" {
		rPr.CreateAttr("strike", props.Strike)
	}

	// Set baseline (super/subscript)
	if props.Baseline != "" {
		rPr.CreateAttr("baseline", props.Baseline)
	}

	// Set language
	if props.Language != "" {
		rPr.CreateAttr("lang", props.Language)
	}

	// Font family
	if props.FontFamily != "" {
		latin := etree.NewElement("a:latin")
		latin.CreateAttr("typeface", props.FontFamily)
		rPr.AddChild(latin)
	}

	// Color handling
	if props.Color != "" {
		// RGB color
		solidFill := etree.NewElement("a:solidFill")
		srgbClr := etree.NewElement("a:srgbClr")
		srgbClr.CreateAttr("val", props.Color)
		solidFill.AddChild(srgbClr)
		rPr.AddChild(solidFill)
	} else if props.ThemeColor != "" {
		// Theme color
		solidFill := etree.NewElement("a:solidFill")
		schemeClr := etree.NewElement("a:schemeClr")
		schemeClr.CreateAttr("val", props.ThemeColor)
		if props.ThemeShade != nil {
			lumMod := etree.NewElement("a:lumMod")
			lumMod.CreateAttr("val", strconv.FormatInt(int64(*props.ThemeShade), 10))
			schemeClr.AddChild(lumMod)
		}
		if props.ThemeTint != nil {
			lumOff := etree.NewElement("a:lumOff")
			lumOff.CreateAttr("val", strconv.FormatInt(int64(*props.ThemeTint), 10))
			schemeClr.AddChild(lumOff)
		}
		solidFill.AddChild(schemeClr)
		rPr.AddChild(solidFill)
	}

	return rPr
}
