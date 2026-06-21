package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// NewSlideImageFill describes one image placeholder fill for a newly created slide.
// Target can be:
//   - A placeholder key (e.g., "pic:12", "body:1") for placeholder-based insertion
//   - "coords" for coordinate-based insertion (requires CoordinateX, CoordinateY, CoordinateWidth, CoordinateHeight)
//   - "slot:<slotKey>" for normalized layout/master slot targeting, including authored picture placeholders
type NewSlideImageFill struct {
	Target           string
	ImageData        []byte
	ContentType      string
	FitMode          FitMode
	CoordinateX      int64
	CoordinateY      int64
	CoordinateWidth  int64
	CoordinateHeight int64
}

// NewSlideRichTextFill describes one rich-text placeholder fill for a newly created slide.
type NewSlideRichTextFill struct {
	Target   string
	RichText *model.TextBlockInfo
}

// NewSlideFromLayoutRequest describes a package-level new-slide-from-layout mutation.
type NewSlideFromLayoutRequest struct {
	Package       opc.PackageSession
	LayoutPartURI string
	InsertAfter   int
	SetTexts      map[string]string
	SetRichTexts  []NewSlideRichTextFill
	SetImages     []NewSlideImageFill
	// Optional paragraph/bullet mutation options to apply to all filled text
	ParagraphOptions *ParagraphMutationOptions
	BulletOptions    *BulletMutationOptions
}

// NewSlideFromLayoutResult describes the inserted slide.
type NewSlideFromLayoutResult struct {
	NewSlideNumber int
	NewSlideID     uint32
	NewSlideURI    string
}

// NewSlideFromLayout creates a new slide bound to an existing layout.
func NewSlideFromLayout(req *NewSlideFromLayoutRequest) (*NewSlideFromLayoutResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("new-slide-from-layout requires an open package session")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}

	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}
	insertAfter := req.InsertAfter
	if insertAfter == 0 {
		insertAfter = len(graph.Slides)
	}
	if insertAfter < 0 || insertAfter > len(graph.Slides) {
		return nil, fmt.Errorf("insert-after %d out of range", insertAfter)
	}

	layoutExists := false
	masterURI := ""
	for _, layout := range graph.Layouts {
		if layout.PartURI == req.LayoutPartURI {
			layoutExists = true
			masterURI = layout.MasterPartURI
			break
		}
	}
	if !layoutExists {
		return nil, fmt.Errorf("layout %s not found", req.LayoutPartURI)
	}

	var result *NewSlideFromLayoutResult
	if templateSlide := findTemplateSlide(graph, req.LayoutPartURI); templateSlide != nil {
		cloned, err := CloneSlide(&CloneSlideRequest{
			Package:     req.Package,
			SlideNumber: templateSlide.SlideNumber,
			InsertAfter: insertAfter,
			NotesPolicy: NotesDrop,
		})
		if err != nil {
			return nil, err
		}
		result = &NewSlideFromLayoutResult{NewSlideNumber: cloned.NewSlideNumber, NewSlideID: cloned.NewSlideID, NewSlideURI: cloned.NewSlideURI}
	} else {
		created, err := createSlideFromLayout(req.Package, req.LayoutPartURI, insertAfter)
		if err != nil {
			return nil, err
		}
		result = created
	}

	if err := resetSlideText(req.Package, result.NewSlideURI); err != nil {
		return nil, err
	}

	if err := applyTextFillsToSlide(req.Package, result.NewSlideURI, req.LayoutPartURI, masterURI, req.SetTexts, req.ParagraphOptions, req.BulletOptions); err != nil {
		return nil, err
	}

	if len(req.SetRichTexts) > 0 {
		if err := applyRichTextFillsToSlide(req.Package, result.NewSlideURI, req.LayoutPartURI, masterURI, req.SetRichTexts, req.ParagraphOptions, req.BulletOptions); err != nil {
			return nil, err
		}
	}

	if len(req.SetImages) > 0 {
		newSlideRef := inspect.SlideRef{PartURI: result.NewSlideURI, SlideNumber: result.NewSlideNumber, LayoutPartURI: req.LayoutPartURI}
		for _, fill := range req.SetImages {
			if err := applyImageFillToSlide(req.Package, &newSlideRef, fill); err != nil {
				return nil, err
			}
		}
	}

	return result, nil
}

func createSlideFromLayout(session opc.PackageSession, layoutPartURI string, insertAfter int) (*NewSlideFromLayoutResult, error) {
	layoutDoc, err := session.ReadXMLPart(layoutPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read layout: %w", err)
	}
	layoutRoot := layoutDoc.Root()
	if layoutRoot == nil {
		return nil, fmt.Errorf("layout root element not found")
	}
	cSld := layoutRoot.FindElement("{" + namespaces.NsP + "}cSld")
	if cSld == nil {
		cSld = layoutRoot.FindElement("cSld")
	}
	if cSld == nil {
		return nil, fmt.Errorf("layout common slide data not found")
	}

	newSlideURI, err := AllocateSlidePartName(session)
	if err != nil {
		return nil, err
	}
	newSlideDoc := etree.NewDocument()
	root := etree.NewElement("p:sld")
	root.CreateAttr("xmlns:p", namespaces.NsP)
	root.CreateAttr("xmlns:a", namespaces.NsA)
	root.CreateAttr("xmlns:r", namespaces.NsR)
	root.AddChild(cSld.Copy())
	clrMapOvr := etree.NewElement("p:clrMapOvr")
	clrMapOvr.AddChild(etree.NewElement("a:masterClrMapping"))
	root.AddChild(clrMapOvr)
	newSlideDoc.SetRoot(root)

	slideXML, err := writeXML(newSlideDoc)
	if err != nil {
		return nil, fmt.Errorf("failed to serialize new slide XML: %w", err)
	}
	if err := session.AddPart(newSlideURI, slideXML, slideContentType, nil); err != nil {
		return nil, err
	}

	layoutTarget, err := relationshipTarget(newSlideURI, layoutPartURI)
	if err != nil {
		return nil, err
	}
	relsXML, err := BuildRelationshipsXML([]opc.RelationshipInfo{{ID: "rId1", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout", Target: layoutTarget}})
	if err != nil {
		return nil, err
	}
	if err := session.AddPart(relsURIForPart(newSlideURI), relsXML, relationshipsContentType, nil); err != nil {
		return nil, err
	}

	presentationDoc, err := session.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, err
	}
	newSlideID, err := AllocateSlideID(presentationDoc)
	if err != nil {
		return nil, err
	}
	inserted, err := InsertSlideReference(&InsertSlideReferenceRequest{
		PresentationDoc:           presentationDoc,
		PresentationRelationships: session.ListRelationships("/ppt/presentation.xml"),
		SlidePartURI:              newSlideURI,
		SlideID:                   newSlideID,
		Position:                  insertAfter,
	})
	if err != nil {
		return nil, err
	}
	if err := session.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		return nil, err
	}
	presentationRels := append(session.ListRelationships("/ppt/presentation.xml"), inserted.Relationship)
	presentationRelsXML, err := BuildRelationshipsXML(presentationRels)
	if err != nil {
		return nil, err
	}
	if err := session.ReplaceRawPart("/ppt/_rels/presentation.xml.rels", presentationRelsXML, relationshipsContentType); err != nil {
		return nil, err
	}

	return &NewSlideFromLayoutResult{NewSlideNumber: insertAfter + 1, NewSlideID: newSlideID, NewSlideURI: newSlideURI}, nil
}

func findTemplateSlide(graph *inspect.PresentationGraph, layoutURI string) *inspect.SlideRef {
	for i := range graph.Slides {
		if graph.Slides[i].LayoutPartURI == layoutURI {
			return &graph.Slides[i]
		}
	}
	return nil
}

func resetSlideText(session opc.PackageSession, slideURI string) error {
	doc, err := session.ReadXMLPart(slideURI)
	if err != nil {
		return err
	}
	spTree := doc.Root().FindElement("//p:spTree")
	if spTree == nil {
		return nil
	}
	shapes := spTree.FindElements("sp")
	if len(shapes) == 0 {
		shapes = spTree.FindElements("{" + namespaces.NsP + "}sp")
	}
	for _, shape := range shapes {
		txBody := shape.FindElement("txBody")
		if txBody == nil {
			txBody = shape.FindElement("{" + namespaces.NsP + "}txBody")
		}
		if txBody == nil {
			if !shapeHasPlaceholder(shape) {
				continue
			}
			shape.AddChild(newEmptyTextBody())
			continue
		}
		if err := replaceShapeText(shape, ""); err != nil {
			return err
		}
	}
	return session.ReplaceXMLPart(slideURI, doc)
}

func shapeHasPlaceholder(shape *etree.Element) bool {
	nvSpPr := shape.FindElement("nvSpPr")
	if nvSpPr == nil {
		nvSpPr = shape.FindElement("{" + namespaces.NsP + "}nvSpPr")
	}
	if nvSpPr == nil {
		return false
	}
	nvPr := nvSpPr.FindElement("nvPr")
	if nvPr == nil {
		nvPr = nvSpPr.FindElement("{" + namespaces.NsP + "}nvPr")
	}
	if nvPr == nil {
		return false
	}
	ph := nvPr.FindElement("ph")
	if ph == nil {
		ph = nvPr.FindElement("{" + namespaces.NsP + "}ph")
	}
	return ph != nil
}

func newEmptyTextBody() *etree.Element {
	txBody := etree.NewElement("p:txBody")
	txBody.AddChild(etree.NewElement("a:bodyPr"))
	txBody.AddChild(etree.NewElement("a:lstStyle"))
	txBody.AddChild(etree.NewElement("a:p"))
	return txBody
}

func applyTextFillsToSlide(session opc.PackageSession, slideURI string, layoutURI string, masterURI string, setTexts map[string]string, paraOpts *ParagraphMutationOptions, bulletOpts *BulletMutationOptions) error {
	if len(setTexts) == 0 {
		return nil
	}
	slideDoc, err := session.ReadXMLPart(slideURI)
	if err != nil {
		return err
	}
	layoutDoc, err := session.ReadXMLPart(layoutURI)
	if err != nil {
		return err
	}
	var masterRoot *etree.Element
	if masterURI != "" {
		masterDoc, err := session.ReadXMLPart(masterURI)
		if err != nil {
			return err
		}
		masterRoot = masterDoc.Root()
	}
	for target, text := range setTexts {
		shape, err := findTargetShape(slideDoc.Root(), layoutDoc.Root(), masterRoot, target)
		if err != nil {
			return err
		}
		if shape == nil {
			return fmt.Errorf("target not found: %s", target)
		}
		if err := replaceShapeText(shape, text); err != nil {
			return err
		}
		// Apply paragraph/bullet options to all paragraphs in the target shape
		if paraOpts != nil || bulletOpts != nil {
			if err := applyParagraphBulletOptionsToShape(shape, paraOpts, bulletOpts); err != nil {
				return err
			}
		}
	}
	return session.ReplaceXMLPart(slideURI, slideDoc)
}

func applyRichTextFillsToSlide(session opc.PackageSession, slideURI string, layoutURI string, masterURI string, setRichTexts []NewSlideRichTextFill, paraOpts *ParagraphMutationOptions, bulletOpts *BulletMutationOptions) error {
	if len(setRichTexts) == 0 {
		return nil
	}
	slideDoc, err := session.ReadXMLPart(slideURI)
	if err != nil {
		return err
	}
	layoutDoc, err := session.ReadXMLPart(layoutURI)
	if err != nil {
		return err
	}
	var masterRoot *etree.Element
	if masterURI != "" {
		masterDoc, err := session.ReadXMLPart(masterURI)
		if err != nil {
			return err
		}
		masterRoot = masterDoc.Root()
	}
	for _, fill := range setRichTexts {
		shape, err := findTargetShape(slideDoc.Root(), layoutDoc.Root(), masterRoot, fill.Target)
		if err != nil {
			return err
		}
		if shape == nil {
			return fmt.Errorf("target not found: %s", fill.Target)
		}
		if err := applyRichTextToShape(shape, fill.RichText); err != nil {
			return err
		}
		// Apply paragraph/bullet options to all paragraphs in the target shape
		if paraOpts != nil || bulletOpts != nil {
			if err := applyParagraphBulletOptionsToShape(shape, paraOpts, bulletOpts); err != nil {
				return err
			}
		}
	}
	return session.ReplaceXMLPart(slideURI, slideDoc)
}

// applyParagraphBulletOptionsToShape applies paragraph and bullet options to all paragraphs in a shape
func applyParagraphBulletOptionsToShape(shape *etree.Element, paraOpts *ParagraphMutationOptions, bulletOpts *BulletMutationOptions) error {
	if paraOpts == nil && bulletOpts == nil {
		return nil
	}

	// Find the text body
	var txBody *etree.Element
	if shape.Tag == "sp" || strings.HasSuffix(shape.Tag, "}sp") {
		txBody = shape.FindElement("txBody")
	}

	if txBody == nil {
		// Shape may not have text (e.g., picture), so just return success
		return nil
	}

	// Apply options to all paragraphs in the text body
	paragraphs := txBody.FindElements("p")
	for _, p := range paragraphs {
		if paraOpts != nil {
			if err := ApplyParagraphOptions(p, paraOpts); err != nil {
				return err
			}
		}
		if bulletOpts != nil {
			if err := ApplyBulletOptions(p, bulletOpts); err != nil {
				return err
			}
		}
	}

	return nil
}

// applyImageFillToSlide handles image insertion with different modes:
// - Placeholder-based: Target is a selector key
// - Coordinate-based: Target is "coords" with explicit EMU coordinates
// - Slot-based: Target is "slot:<key>" for normalized slot targeting
func applyImageFillToSlide(session opc.PackageSession, slideRef *inspect.SlideRef, fill NewSlideImageFill) error {
	if fill.Target == "coords" {
		if fill.CoordinateWidth <= 0 || fill.CoordinateHeight <= 0 {
			return fmt.Errorf("coordinate-based image insertion requires positive width and height")
		}
		if _, err := InsertImage(&InsertImageRequest{
			Package:     session,
			SlideRef:    slideRef,
			ImageData:   fill.ImageData,
			ContentType: fill.ContentType,
			FitMode:     fill.FitMode,
			X:           fill.CoordinateX,
			Y:           fill.CoordinateY,
			CX:          fill.CoordinateWidth,
			CY:          fill.CoordinateHeight,
		}); err != nil {
			return fmt.Errorf("failed to insert image at coordinates: %w", err)
		}
		return nil
	}

	target := strings.TrimSpace(fill.Target)
	allowDefaultFallback := false
	if strings.HasPrefix(target, "slot:") {
		target = strings.TrimSpace(strings.TrimPrefix(target, "slot:"))
		allowDefaultFallback = true
		if target == "" {
			return fmt.Errorf("slot key cannot be empty")
		}
	}

	return applyImageFillToTarget(session, slideRef, target, fill, allowDefaultFallback)
}

func applyImageFillToTarget(session opc.PackageSession, slideRef *inspect.SlideRef, target string, fill NewSlideImageFill, allowDefaultFallback bool) error {
	if strings.TrimSpace(target) == "" {
		return fmt.Errorf("image target cannot be empty")
	}

	slideDoc, layoutDoc, masterRoot, err := loadSlideImageFillContext(session, slideRef)
	if err != nil {
		return err
	}

	targetShape, err := findTargetShape(slideDoc.Root(), layoutDoc.Root(), masterRoot, target)
	if err != nil {
		return err
	}
	if targetShape == nil {
		if allowDefaultFallback {
			return insertImageWithDefaultPosition(session, slideRef, fill)
		}
		return fmt.Errorf("target not found: %s", target)
	}

	targetShapeID, _ := extractShapeID(targetShape)
	if targetShapeID > 0 && pictureShapeHasEmbeddedImage(targetShape) {
		if _, err := ReplaceImage(&selectors.ShapeIDSelector{ID: targetShapeID}, slideRef, session, ImageReplaceOptions{
			FitMode:             fill.FitMode,
			NewImageData:        fill.ImageData,
			NewImageContentType: fill.ContentType,
		}); err != nil {
			return err
		}
		return nil
	}

	x, y, cx, cy, err := extractTargetShapeBounds(targetShape)
	if err != nil {
		if allowDefaultFallback {
			return insertImageWithDefaultPosition(session, slideRef, fill)
		}
		return err
	}
	if targetShapeID <= 0 {
		if allowDefaultFallback {
			return insertImageWithDefaultPosition(session, slideRef, fill)
		}
		return fmt.Errorf("target shape %q is missing a shape ID", target)
	}

	if _, err := InsertImage(&InsertImageRequest{
		Package:       session,
		SlideRef:      slideRef,
		ImageData:     fill.ImageData,
		ContentType:   fill.ContentType,
		FitMode:       fill.FitMode,
		X:             x,
		Y:             y,
		CX:            cx,
		CY:            cy,
		InsertAfterID: targetShapeID,
	}); err != nil {
		return fmt.Errorf("failed to insert image into target bounds: %w", err)
	}

	if err := removeShapeByID(session, slideRef.PartURI, targetShapeID); err != nil {
		return fmt.Errorf("failed to replace placeholder target %q with image: %w", target, err)
	}
	return nil
}

func loadSlideImageFillContext(session opc.PackageSession, slideRef *inspect.SlideRef) (*etree.Document, *etree.Document, *etree.Element, error) {
	slideDoc, err := session.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return nil, nil, nil, err
	}
	layoutDoc, err := session.ReadXMLPart(slideRef.LayoutPartURI)
	if err != nil {
		return nil, nil, nil, err
	}
	masterURI := findLayoutMasterURI(slideRef.LayoutPartURI, session.ListRelationships(slideRef.LayoutPartURI))
	if masterURI == "" {
		return slideDoc, layoutDoc, nil, nil
	}
	masterDoc, err := session.ReadXMLPart(masterURI)
	if err != nil {
		return nil, nil, nil, err
	}
	return slideDoc, layoutDoc, masterDoc.Root(), nil
}

func insertImageWithDefaultPosition(session opc.PackageSession, slideRef *inspect.SlideRef, fill NewSlideImageFill) error {
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		return fmt.Errorf("failed to parse presentation for fallback image insertion: %w", err)
	}

	const emuPerInch = int64(914400)
	defaultWidth := int64(3) * emuPerInch
	defaultHeight := int64(2) * emuPerInch
	defaultX := (graph.SlideSize.CX - defaultWidth) / 2
	defaultY := (graph.SlideSize.CY - defaultHeight) / 2

	if _, err := InsertImage(&InsertImageRequest{
		Package:     session,
		SlideRef:    slideRef,
		ImageData:   fill.ImageData,
		ContentType: fill.ContentType,
		FitMode:     fill.FitMode,
		X:           defaultX,
		Y:           defaultY,
		CX:          defaultWidth,
		CY:          defaultHeight,
	}); err != nil {
		return fmt.Errorf("failed to insert image with default positioning: %w", err)
	}
	return nil
}

func pictureShapeHasEmbeddedImage(shape *etree.Element) bool {
	if shape == nil {
		return false
	}
	tag := shape.Tag
	if tag != "pic" && !strings.HasSuffix(tag, "}pic") && !strings.HasSuffix(tag, ":pic") {
		return false
	}
	blipFill := shape.FindElement("blipFill")
	if blipFill == nil {
		return false
	}
	blip := blipFill.FindElement("blip")
	if blip == nil {
		return false
	}
	for _, attr := range blip.Attr {
		if attr.Space == "r" && attr.Key == "embed" && strings.TrimSpace(attr.Value) != "" {
			return true
		}
	}
	return blip.SelectAttrValue("embed", "") != ""
}

func extractTargetShapeBounds(shape *etree.Element) (int64, int64, int64, int64, error) {
	xfrm, err := targetShapeTransformElement(shape)
	if err != nil {
		return 0, 0, 0, 0, err
	}
	off := xfrm.FindElement("off")
	ext := xfrm.FindElement("ext")
	if off == nil || ext == nil {
		return 0, 0, 0, 0, fmt.Errorf("target shape is missing transform bounds")
	}

	parse := func(elem *etree.Element, attr string) (int64, error) {
		value := strings.TrimSpace(elem.SelectAttrValue(attr, ""))
		if value == "" {
			return 0, fmt.Errorf("missing %s attribute", attr)
		}
		parsed, err := strconv.ParseInt(value, 10, 64)
		if err != nil {
			return 0, fmt.Errorf("invalid %s attribute %q: %w", attr, value, err)
		}
		return parsed, nil
	}

	x, err := parse(off, "x")
	if err != nil {
		return 0, 0, 0, 0, err
	}
	y, err := parse(off, "y")
	if err != nil {
		return 0, 0, 0, 0, err
	}
	cx, err := parse(ext, "cx")
	if err != nil {
		return 0, 0, 0, 0, err
	}
	cy, err := parse(ext, "cy")
	if err != nil {
		return 0, 0, 0, 0, err
	}
	if cx <= 0 || cy <= 0 {
		return 0, 0, 0, 0, fmt.Errorf("target shape has non-positive bounds: cx=%d cy=%d", cx, cy)
	}
	return x, y, cx, cy, nil
}

func targetShapeTransformElement(shape *etree.Element) (*etree.Element, error) {
	if shape == nil {
		return nil, fmt.Errorf("target shape cannot be nil")
	}
	tag := shape.Tag
	if tag == "graphicFrame" || strings.HasSuffix(tag, "}graphicFrame") || strings.HasSuffix(tag, ":graphicFrame") {
		xfrm := shape.FindElement("xfrm")
		if xfrm == nil {
			return nil, fmt.Errorf("graphicFrame is missing xfrm")
		}
		return xfrm, nil
	}
	spPr := shape.FindElement("spPr")
	if spPr == nil {
		return nil, fmt.Errorf("target shape is missing spPr")
	}
	xfrm := spPr.FindElement("xfrm")
	if xfrm == nil {
		return nil, fmt.Errorf("target shape is missing xfrm")
	}
	return xfrm, nil
}

func removeShapeByID(session opc.PackageSession, slideURI string, shapeID int) error {
	slideDoc, err := session.ReadXMLPart(slideURI)
	if err != nil {
		return err
	}
	spTree := slideDoc.Root().FindElement("//p:spTree")
	if spTree == nil {
		return fmt.Errorf("no shape tree found on slide")
	}
	shape := findShapeByID(spTree, shapeID)
	if shape == nil {
		return nil
	}
	parent := shape.Parent()
	if parent == nil {
		return fmt.Errorf("target shape %d has no parent", shapeID)
	}
	parent.RemoveChild(shape)
	return session.ReplaceXMLPart(slideURI, slideDoc)
}
