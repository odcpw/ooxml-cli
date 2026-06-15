package mutate

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/imagex"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

// FitMode represents the image fit mode (contain or cover)
type FitMode string

const (
	FitModeContain FitMode = "contain"
	FitModeCover   FitMode = "cover"
)

// ParseFitMode parses a fit mode string
func ParseFitMode(mode string) (FitMode, error) {
	switch strings.ToLower(mode) {
	case "contain", "fit":
		return FitModeContain, nil
	case "cover", "crop":
		return FitModeCover, nil
	default:
		return "", fmt.Errorf("invalid fit mode %q (must be 'contain' or 'cover')", mode)
	}
}

// ImageReplaceOptions contains options for ReplaceImage
type ImageReplaceOptions struct {
	// FitMode is the fit mode for the image (contain or cover)
	FitMode FitMode

	// NewImageData is the raw bytes of the new image
	NewImageData []byte

	// NewImageContentType is the content type of the new image (e.g., "image/png")
	NewImageContentType string
}

// ReplaceImageResult contains the result of a successful image replacement
type ReplaceImageResult struct {
	// ShapeID is the ID of the shape that was modified
	ShapeID int

	// ShapeName is the name of the shape that was modified
	ShapeName string

	// OldTargetURI is the original target URI for the image
	OldTargetURI string

	// OldContentType is the original content type
	OldContentType string

	// NewTargetURI is the new target URI for the image
	NewTargetURI string

	// NewContentType is the new content type
	NewContentType string

	// RelationshipID is the relationship ID used
	RelationshipID string
}

// ReplaceImage replaces the image in a picture shape with a new image.
// It handles:
// - Resolving the selector to a specific picture shape
// - Validating that the target is a picture (not a master/layout image reference)
// - Replacing the image part in the package
// - Updating the fit mode (contain/cover)
// - Returning details about what was changed
func ReplaceImage(
	selector selectors.Selector,
	slideRef *inspect.SlideRef,
	session opc.PackageSession,
	opts ImageReplaceOptions,
) (*ReplaceImageResult, error) {
	if opts.NewImageData == nil || len(opts.NewImageData) == 0 {
		return nil, fmt.Errorf("new image data is empty")
	}
	contentType, err := validateImagePayload(opts.NewImageContentType, opts.NewImageData)
	if err != nil {
		return nil, err
	}

	// Read the slide XML
	slideDoc, err := session.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	// Get the shape tree
	spTree := slideDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in slide")
	}

	// Resolve the selector to a picture shape
	pic, picInfo, err := resolvePictureShape(selector, slideRef, session, spTree)
	if err != nil {
		return nil, err
	}

	// Find the blip element (the actual image reference)
	blipFill := pic.FindElement("blipFill")
	if blipFill == nil {
		return nil, fmt.Errorf("blip fill not found in picture shape")
	}

	blip := blipFill.FindElement("blip")
	if blip == nil {
		return nil, fmt.Errorf("blip element not found in picture shape")
	}

	// Get the current relationship ID
	relID := ""
	for _, attr := range blip.Attr {
		if attr.Key == "embed" && attr.Space == "r" {
			relID = attr.Value
			break
		}
	}
	if relID == "" {
		relID = blip.SelectAttrValue("embed", "")
	}

	if relID == "" {
		return nil, fmt.Errorf("no relationship ID found in blip element")
	}

	// Get the old image URI to determine where to place the new image
	oldImageURI := ""
	relationships := session.ListRelationships(slideRef.PartURI)
	for _, rel := range relationships {
		if rel.ID == relID {
			oldImageURI = opc.ResolveRelationshipTarget(slideRef.PartURI, rel.Target)
			break
		}
	}

	if oldImageURI == "" {
		return nil, fmt.Errorf("could not resolve relationship %s", relID)
	}

	oldContentType := session.GetContentType(oldImageURI)

	// Determine the new image URI
	// Try to preserve the extension if content type changes
	newImageURI := oldImageURI
	if contentType != oldContentType {
		// Content type changed - need to update the filename
		newExt, err := getExtensionForContentType(contentType)
		if err != nil {
			return nil, err
		}
		oldExt := filepath.Ext(oldImageURI)
		if newExt != oldExt {
			// Replace the extension
			base := oldImageURI[:len(oldImageURI)-len(oldExt)]
			newImageURI = base + newExt
		}
	}

	// Write the new image part. If the URI changes to a part that does not yet exist,
	// this is effectively an AddPart rather than an in-place replacement.
	if newImageURI != oldImageURI && !packagePartExists(session, newImageURI) {
		err = session.AddPart(newImageURI, opts.NewImageData, contentType, nil)
	} else {
		err = session.ReplaceRawPart(newImageURI, opts.NewImageData, contentType)
	}
	if err != nil {
		return nil, fmt.Errorf("failed to write image part: %w", err)
	}

	if newImageURI != oldImageURI {
		updatedTarget, err := relationshipTarget(slideRef.PartURI, newImageURI)
		if err != nil {
			return nil, fmt.Errorf("failed to relativize updated image target: %w", err)
		}
		for i := range relationships {
			if relationships[i].ID == relID {
				relationships[i].Target = updatedTarget
				break
			}
		}
		relsXML, err := BuildRelationshipsXML(relationships)
		if err != nil {
			return nil, fmt.Errorf("failed to rebuild slide relationships: %w", err)
		}
		relsURI := opc.GetDirectory(slideRef.PartURI) + "/_rels/" + opc.GetFileName(slideRef.PartURI) + ".rels"
		relsContentType := session.GetContentType(relsURI)
		if relsContentType == "" {
			relsContentType = "application/vnd.openxmlformats-package.relationships+xml"
		}
		if err := session.ReplaceRawPart(relsURI, relsXML, relsContentType); err != nil {
			return nil, fmt.Errorf("failed to update slide relationships: %w", err)
		}

		// Keep the old media part in place. It may still be referenced by other slides,
		// and preserving untouched parts is preferable to guessing ownership here.
	}

	// Update the blip fill's fit mode based on the options
	updateBlipFillFitMode(blipFill, opts.FitMode)

	// Write the slide back
	err = session.ReplaceXMLPart(slideRef.PartURI, slideDoc)
	if err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &ReplaceImageResult{
		ShapeID:        picInfo.ShapeID,
		ShapeName:      picInfo.ShapeName,
		OldTargetURI:   oldImageURI,
		OldContentType: oldContentType,
		NewTargetURI:   newImageURI,
		NewContentType: contentType,
		RelationshipID: relID,
	}, nil
}

// InsertImageRequest holds parameters for inserting a new image on a slide
type InsertImageRequest struct {
	Package       opc.PackageSession
	SlideRef      *inspect.SlideRef
	ImageData     []byte
	ContentType   string
	FitMode       FitMode
	X             int64 // Position X in EMUs
	Y             int64 // Position Y in EMUs
	CX            int64 // Width in EMUs
	CY            int64 // Height in EMUs
	Name          string
	InsertAfterID int // Shape ID to insert after (0 = append)
}

// InsertImageResult holds the result of a successful image insertion
type InsertImageResult struct {
	ShapeID        int
	ShapeName      string
	TargetURI      string
	ContentType    string
	RelationshipID string
}

// InsertImage creates a new picture shape on a slide at specific EMU coordinates.
// It handles:
// - Creating a new picture element with geometry (x, y, cx, cy)
// - Adding the image part to the package
// - Creating the relationship
// - Inserting into the shape tree
// - Preserving unrelated shape state
func InsertImage(req *InsertImageRequest) (*InsertImageResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference cannot be nil")
	}
	if req.ImageData == nil || len(req.ImageData) == 0 {
		return nil, fmt.Errorf("image data is empty")
	}
	contentType, err := validateImagePayload(req.ContentType, req.ImageData)
	if err != nil {
		return nil, err
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("image dimensions must be positive: cx=%d, cy=%d", req.CX, req.CY)
	}

	// Read the slide XML
	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	// Get the shape tree
	spTree := slideDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in slide")
	}

	// Determine the new shape ID across every existing cNvPr in the tree.
	newShapeID := nextSpTreeShapeID(spTree)

	// Allocate image part name and URI
	imageFileName := fmt.Sprintf("image%d", newShapeID)
	imageExt, err := getExtensionForContentType(contentType)
	if err != nil {
		return nil, err
	}
	imageURI := "/ppt/media/" + imageFileName + imageExt

	// Check for existing image with this name and increment if needed
	counter := 1
	for packagePartExists(req.Package, imageURI) {
		imageURI = fmt.Sprintf("/ppt/media/%s_%d%s", imageFileName, counter, imageExt)
		counter++
	}

	// Add the image part to the package
	if err := req.Package.AddPart(imageURI, req.ImageData, contentType, nil); err != nil {
		return nil, fmt.Errorf("failed to add image part: %w", err)
	}

	// Create a relationship from the slide to the image
	currentRels := req.Package.ListRelationships(req.SlideRef.PartURI)
	newRelID := AllocateRelationshipID(currentRels)
	imageTarget, err := relationshipTarget(req.SlideRef.PartURI, imageURI)
	if err != nil {
		return nil, fmt.Errorf("failed to relativize image target: %w", err)
	}

	newRel := opc.RelationshipInfo{
		SourceURI: req.SlideRef.PartURI,
		ID:        newRelID,
		Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
		Target:    imageTarget,
	}

	// Add the relationship
	currentRels = append(currentRels, newRel)
	relsXML, err := BuildRelationshipsXML(currentRels)
	if err != nil {
		return nil, fmt.Errorf("failed to build relationships XML: %w", err)
	}
	relsURI := opc.GetDirectory(req.SlideRef.PartURI) + "/_rels/" + opc.GetFileName(req.SlideRef.PartURI) + ".rels"
	relsContentType := req.Package.GetContentType(relsURI)
	if relsContentType == "" {
		relsContentType = "application/vnd.openxmlformats-package.relationships+xml"
	}
	if err := req.Package.ReplaceRawPart(relsURI, relsXML, relsContentType); err != nil {
		return nil, fmt.Errorf("failed to update slide relationships: %w", err)
	}

	// Create the picture element
	shapeName := req.Name
	if shapeName == "" {
		shapeName = imageFileName
	}
	pic := createPictureElement(newShapeID, shapeName, newRelID, req.X, req.Y, req.CX, req.CY, req.FitMode)

	// Insert into shape tree (after specified shape or before the schema-tail extLst).
	insertSpTreeChildAfterShapeID(spTree, pic, req.InsertAfterID)

	// Write the slide back
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &InsertImageResult{
		ShapeID:        newShapeID,
		ShapeName:      shapeName,
		TargetURI:      imageURI,
		ContentType:    contentType,
		RelationshipID: newRelID,
	}, nil
}

// RepositionShapeRequest holds parameters for repositioning/resizing a shape
type RepositionShapeRequest struct {
	Package  opc.PackageSession
	SlideRef *inspect.SlideRef
	Selector selectors.Selector
	X        *int64 // New X position (nil = no change)
	Y        *int64 // New Y position (nil = no change)
	CX       *int64 // New width (nil = no change)
	CY       *int64 // New height (nil = no change)
}

// RepositionShapeResult holds the result of repositioning
type RepositionShapeResult struct {
	ShapeID   int
	ShapeName string
	OldX      int64
	OldY      int64
	OldCX     int64
	OldCY     int64
	NewX      int64
	NewY      int64
	NewCX     int64
	NewCY     int64
}

// RepositionShape updates the position and/or size of an existing shape while
// preserving unrelated properties like image content, crop, rotation, etc.
func RepositionShape(req *RepositionShapeRequest) (*RepositionShapeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.SlideRef == nil {
		return nil, fmt.Errorf("slide reference cannot be nil")
	}
	if req.Selector == nil {
		return nil, fmt.Errorf("selector cannot be nil")
	}
	if req.X == nil && req.Y == nil && req.CX == nil && req.CY == nil {
		return nil, fmt.Errorf("at least one dimension must be specified")
	}

	// Read the slide XML
	slideDoc, err := req.Package.ReadXMLPart(req.SlideRef.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read slide: %w", err)
	}

	// Get the shape tree
	spTree := slideDoc.FindElement(".//spTree")
	if spTree == nil {
		return nil, fmt.Errorf("shape tree not found in slide")
	}

	// Find the target shape
	var targetShape *etree.Element
	var targetID int
	var targetName string

	// Try to resolve the selector to find the shape
	if shapeIDSel, ok := req.Selector.(*selectors.ShapeIDSelector); ok {
		targetID = shapeIDSel.ID
		for _, sp := range spTree.FindElements("sp") {
			if id, _ := extractShapeID(sp); id == targetID {
				targetShape = sp
				targetName, _ = extractShapeName(sp)
				break
			}
		}
		if targetShape == nil {
			for _, pic := range spTree.FindElements("pic") {
				if id, _ := extractShapeID(pic); id == targetID {
					targetShape = pic
					targetName, _ = extractShapeName(pic)
					break
				}
			}
		}
	} else if shapeNameSel, ok := req.Selector.(*selectors.ShapeNameSelector); ok {
		targetName = shapeNameSel.Name
		for _, sp := range spTree.FindElements("sp") {
			if name, _ := extractShapeName(sp); name == targetName {
				targetShape = sp
				targetID, _ = extractShapeID(sp)
				break
			}
		}
		if targetShape == nil {
			for _, pic := range spTree.FindElements("pic") {
				if name, _ := extractShapeName(pic); name == targetName {
					targetShape = pic
					targetID, _ = extractShapeID(pic)
					break
				}
			}
		}
	}

	if targetShape == nil {
		return nil, fmt.Errorf("shape not found matching selector %v", req.Selector)
	}

	// Get shape properties element
	spPr := targetShape.FindElement("spPr")
	if spPr == nil {
		return nil, fmt.Errorf("shape properties not found for shape %d", targetID)
	}

	// Get or create transform element
	xfrm := spPr.FindElement("xfrm")
	if xfrm == nil {
		xfrm = etree.NewElement("a:xfrm")
		// Insert transform as first child of spPr
		if len(spPr.ChildElements()) > 0 {
			spPr.InsertChildAt(0, xfrm)
		} else {
			spPr.AddChild(xfrm)
		}
	}

	// Get current geometry
	off := xfrm.FindElement("off")
	ext := xfrm.FindElement("ext")

	oldX, oldY := int64(0), int64(0)
	oldCX, oldCY := int64(0), int64(0)

	if off != nil {
		if xStr := off.SelectAttrValue("x", ""); xStr != "" {
			fmt.Sscanf(xStr, "%d", &oldX)
		}
		if yStr := off.SelectAttrValue("y", ""); yStr != "" {
			fmt.Sscanf(yStr, "%d", &oldY)
		}
	}
	if ext != nil {
		if cxStr := ext.SelectAttrValue("cx", ""); cxStr != "" {
			fmt.Sscanf(cxStr, "%d", &oldCX)
		}
		if cyStr := ext.SelectAttrValue("cy", ""); cyStr != "" {
			fmt.Sscanf(cyStr, "%d", &oldCY)
		}
	}

	// Determine new values (use old if not specified)
	newX := oldX
	if req.X != nil {
		newX = *req.X
	}
	newY := oldY
	if req.Y != nil {
		newY = *req.Y
	}
	newCX := oldCX
	if req.CX != nil {
		newCX = *req.CX
	}
	newCY := oldCY
	if req.CY != nil {
		newCY = *req.CY
	}

	// Validate new dimensions
	if newCX <= 0 || newCY <= 0 {
		return nil, fmt.Errorf("invalid dimensions: cx=%d, cy=%d (must be positive)", newCX, newCY)
	}

	// Update or create offset element
	if off == nil {
		off = etree.NewElement("a:off")
		xfrm.InsertChildAt(0, off)
	}
	off.CreateAttr("x", fmt.Sprintf("%d", newX))
	off.CreateAttr("y", fmt.Sprintf("%d", newY))

	// Update or create extent element
	if ext == nil {
		ext = etree.NewElement("a:ext")
		xfrm.InsertChildAt(1, ext)
	}
	ext.CreateAttr("cx", fmt.Sprintf("%d", newCX))
	ext.CreateAttr("cy", fmt.Sprintf("%d", newCY))

	// Write the slide back
	if err := req.Package.ReplaceXMLPart(req.SlideRef.PartURI, slideDoc); err != nil {
		return nil, fmt.Errorf("failed to write slide: %w", err)
	}

	return &RepositionShapeResult{
		ShapeID:   targetID,
		ShapeName: targetName,
		OldX:      oldX,
		OldY:      oldY,
		OldCX:     oldCX,
		OldCY:     oldCY,
		NewX:      newX,
		NewY:      newY,
		NewCX:     newCX,
		NewCY:     newCY,
	}, nil
}

// createPictureElement creates a new picture element with geometry
func createPictureElement(shapeID int, shapeName string, relID string, x, y, cx, cy int64, fitMode FitMode) *etree.Element {
	pic := etree.NewElement("p:pic")

	// Non-visual properties
	nvPicPr := etree.NewElement("p:nvPicPr")
	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", fmt.Sprintf("%d", shapeID))
	cNvPr.CreateAttr("name", shapeName)
	nvPicPr.AddChild(cNvPr)

	cNvPicPr := etree.NewElement("p:cNvPicPr")
	nvPicPr.AddChild(cNvPicPr)

	nvPr := etree.NewElement("p:nvPr")
	nvPicPr.AddChild(nvPr)

	pic.AddChild(nvPicPr)

	// Blip fill (image reference)
	blipFill := etree.NewElement("p:blipFill")
	blip := etree.NewElement("a:blip")
	blip.CreateAttr("r:embed", relID)
	blipFill.AddChild(blip)

	// Set fit mode
	updateBlipFillFitMode(blipFill, fitMode)

	pic.AddChild(blipFill)

	// Shape properties (geometry)
	spPr := etree.NewElement("p:spPr")

	xfrm := etree.NewElement("a:xfrm")
	off := etree.NewElement("a:off")
	off.CreateAttr("x", fmt.Sprintf("%d", x))
	off.CreateAttr("y", fmt.Sprintf("%d", y))
	xfrm.AddChild(off)

	ext := etree.NewElement("a:ext")
	ext.CreateAttr("cx", fmt.Sprintf("%d", cx))
	ext.CreateAttr("cy", fmt.Sprintf("%d", cy))
	xfrm.AddChild(ext)

	spPr.AddChild(xfrm)

	// Add prstGeom (preset geometry)
	prstGeom := etree.NewElement("a:prstGeom")
	prstGeom.CreateAttr("prst", "rect")
	prstGeom.AddChild(etree.NewElement("a:avLst"))
	spPr.AddChild(prstGeom)

	pic.AddChild(spPr)

	return pic
}

// extractShapeID extracts the shape ID from a shape element
func extractShapeID(elem *etree.Element) (int, error) {
	if elem == nil {
		return 0, fmt.Errorf("element is nil")
	}

	var nvPr *etree.Element
	tag := elem.Tag
	if tag == "p:sp" || tag == "sp" {
		nvPr = elem.FindElement("nvSpPr")
	} else if tag == "p:pic" || tag == "pic" {
		nvPr = elem.FindElement("nvPicPr")
	} else if tag == "p:grpSp" || tag == "grpSp" {
		nvPr = elem.FindElement("nvGrpSpPr")
	} else {
		nvPr = elem.FindElement("nvSpPr")
	}

	if nvPr == nil {
		return 0, fmt.Errorf("non-visual properties not found")
	}

	cNvPr := nvPr.FindElement("cNvPr")
	if cNvPr == nil {
		return 0, fmt.Errorf("cNvPr not found")
	}

	idStr := cNvPr.SelectAttrValue("id", "")
	if idStr == "" {
		return 0, fmt.Errorf("shape id attribute not found")
	}

	var id int
	if _, err := fmt.Sscanf(idStr, "%d", &id); err != nil {
		return 0, fmt.Errorf("invalid shape id: %w", err)
	}

	return id, nil
}

// extractShapeName extracts the shape name from a shape element
func extractShapeName(elem *etree.Element) (string, error) {
	if elem == nil {
		return "", fmt.Errorf("element is nil")
	}

	var nvPr *etree.Element
	tag := elem.Tag
	if tag == "p:sp" || tag == "sp" {
		nvPr = elem.FindElement("nvSpPr")
	} else if tag == "p:pic" || tag == "pic" {
		nvPr = elem.FindElement("nvPicPr")
	} else if tag == "p:grpSp" || tag == "grpSp" {
		nvPr = elem.FindElement("nvGrpSpPr")
	} else {
		nvPr = elem.FindElement("nvSpPr")
	}

	if nvPr == nil {
		return "", fmt.Errorf("non-visual properties not found")
	}

	cNvPr := nvPr.FindElement("cNvPr")
	if cNvPr == nil {
		return "", fmt.Errorf("cNvPr not found")
	}

	return cNvPr.SelectAttrValue("name", ""), nil
}

// resolvePictureShape resolves a selector to a picture shape in a slide
func resolvePictureShape(
	selector selectors.Selector,
	slideRef *inspect.SlideRef,
	session opc.PackageSession,
	spTree *etree.Element,
) (*etree.Element, *picInfo, error) {
	// Get all picture elements and their info
	pictures := make(map[int]*etree.Element) // shapeID -> pic element
	var picInfos []*picInfo

	relationships := session.ListRelationships(slideRef.PartURI)
	relMap := make(map[string]opc.RelationshipInfo)
	for _, rel := range relationships {
		relMap[rel.ID] = rel
	}

	for _, pic := range spTree.FindElements("pic") {
		info := extractPictureInfo(slideRef.PartURI, pic, relMap, session)
		if info != nil {
			pictures[info.ShapeID] = pic
			picInfos = append(picInfos, info)
		}
	}

	// Resolve the selector against the pictures
	// Try shape ID selector
	if shapeIDSel, ok := selector.(*selectors.ShapeIDSelector); ok {
		pic, exists := pictures[shapeIDSel.ID]
		if !exists {
			return nil, nil, fmt.Errorf("picture with ID %d not found on slide", shapeIDSel.ID)
		}
		// Find the info
		for _, info := range picInfos {
			if info.ShapeID == shapeIDSel.ID {
				return pic, info, nil
			}
		}
	}

	// Try shape name selector
	if shapeNameSel, ok := selector.(*selectors.ShapeNameSelector); ok {
		for _, info := range picInfos {
			if info.ShapeName == shapeNameSel.Name {
				pic := pictures[info.ShapeID]
				return pic, info, nil
			}
		}
		return nil, nil, fmt.Errorf("picture with name %q not found on slide", shapeNameSel.Name)
	}

	// Unsupported selector type for images
	return nil, nil, fmt.Errorf("selector type %T is not supported for image replacement (use shape ID or name)", selector)
}

// picInfo represents basic information about a picture shape
type picInfo struct {
	ShapeID     int
	ShapeName   string
	HasImageRef bool // Whether this picture has an image reference (not just a placeholder shape)
}

// extractPictureInfo extracts information from a p:pic element
func extractPictureInfo(
	sourcePartURI string,
	pic *etree.Element,
	relMap map[string]opc.RelationshipInfo,
	session opc.PackageSession,
) *picInfo {
	info := &picInfo{
		ShapeID:     0,
		HasImageRef: false,
	}

	// Get picture ID and name from p:nvPicPr/p:cNvPr
	nvPicPr := pic.FindElement("nvPicPr")
	if nvPicPr != nil {
		cNvPr := nvPicPr.FindElement("cNvPr")
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if idStr != "" {
				if id, err := stringToInt(idStr); err == nil {
					info.ShapeID = id
				}
			}
			info.ShapeName = cNvPr.SelectAttrValue("name", "")
		}
	}

	// Get image reference from p:blipFill/a:blip@r:embed
	blipFill := pic.FindElement("blipFill")
	if blipFill == nil {
		return info
	}

	blip := blipFill.FindElement("blip")
	if blip == nil {
		return info
	}

	// Extract the r:embed attribute
	relID := ""
	for _, attr := range blip.Attr {
		if attr.Key == "embed" && attr.Space == "r" {
			relID = attr.Value
			break
		}
	}

	if relID == "" {
		relID = blip.SelectAttrValue("embed", "")
	}

	if relID == "" {
		return info
	}

	// Resolve the relationship to verify it exists
	_, exists := relMap[relID]
	if exists {
		info.HasImageRef = true
	}

	return info
}

// updateBlipFillFitMode updates the blip fill element's fit mode
func updateBlipFillFitMode(blipFill *etree.Element, fitMode FitMode) {
	// Remove existing stretch or tile elements
	for _, child := range blipFill.ChildElements() {
		if child.Tag == "{http://schemas.openxmlformats.org/drawingml/2006/main}stretch" ||
			child.Tag == "{http://schemas.openxmlformats.org/drawingml/2006/main}tile" {
			blipFill.RemoveChild(child)
		}
	}

	// Add the appropriate fit mode element
	switch fitMode {
	case FitModeContain:
		// Add stretch with fillRect (standard "fit" mode)
		stretch := etree.NewElement("a:stretch")
		fillRect := etree.NewElement("a:fillRect")
		stretch.AddChild(fillRect)
		blipFill.AddChild(stretch)

	case FitModeCover:
		// Add tile mode for cover
		tile := etree.NewElement("a:tile")
		tile.CreateAttr("tx", "0")
		tile.CreateAttr("ty", "0")
		tile.CreateAttr("sx", "100000")
		tile.CreateAttr("sy", "100000")
		tile.CreateAttr("flip", "none")
		tile.CreateAttr("algn", "ctr")
		blipFill.AddChild(tile)
	}
}

func validateImagePayload(contentType string, raw []byte) (string, error) {
	normalized := imagex.NormalizedContentType(contentType)
	if !imagex.IsContentType(normalized) {
		return "", fmt.Errorf("unsupported image content type %q", contentType)
	}
	if _, ok := imagex.ExtensionForContentType(normalized); !ok {
		return "", fmt.Errorf("unsupported image content type %q", contentType)
	}
	if imagex.HasKnownSignature(normalized) && !imagex.PayloadMatchesContentType(normalized, raw) {
		return "", fmt.Errorf("image payload does not match content type %s", normalized)
	}
	return normalized, nil
}

// getExtensionForContentType returns the file extension for a supported image content type.
func getExtensionForContentType(contentType string) (string, error) {
	ext, ok := imagex.ExtensionForContentType(contentType)
	if !ok {
		return "", fmt.Errorf("unsupported image content type %q", contentType)
	}
	return ext, nil
}

// stringToInt is a helper to convert a string to int
func stringToInt(s string) (int, error) {
	var result int
	_, err := fmt.Sscanf(s, "%d", &result)
	return result, err
}

func packagePartExists(session opc.PackageSession, uri string) bool {
	for _, part := range session.ListParts() {
		if part.URI == uri {
			return true
		}
	}
	return false
}
