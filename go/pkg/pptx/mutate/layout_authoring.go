package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

const layoutContentType = "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"

// CloneLayoutRequest describes cloning an existing layout under the same master.
type CloneLayoutRequest struct {
	Package       opc.PackageSession
	LayoutPartURI string
	NewName       string
}

// CloneLayoutResult describes the newly cloned layout.
type CloneLayoutResult struct {
	SourceLayoutURI string
	NewLayoutURI    string
	MasterPartURI   string
	OldName         string
	NewName         string
	RelationshipID  string
	LayoutID        uint32
}

// RenameLayoutRequest describes renaming an existing layout.
type RenameLayoutRequest struct {
	Package       opc.PackageSession
	LayoutPartURI string
	NewName       string
}

// RenameLayoutResult describes a layout rename.
type RenameLayoutResult struct {
	LayoutPartURI string
	OldName       string
	NewName       string
}

// DeleteLayoutShapeRequest describes deleting a shape from a layout.
type DeleteLayoutShapeRequest struct {
	Package       opc.PackageSession
	LayoutPartURI string
	Target        string
}

// DeleteLayoutShapeResult describes a deleted layout shape.
type DeleteLayoutShapeResult struct {
	LayoutPartURI string
	ShapeID       int
	ShapeName     string
}

// SetLayoutShapeBoundsRequest describes updating a layout shape's bounds.
type SetLayoutShapeBoundsRequest struct {
	Package       opc.PackageSession
	LayoutPartURI string
	Target        string
	X             int64
	Y             int64
	CX            int64
	CY            int64
}

// SetLayoutShapeBoundsResult describes a layout shape bounds update.
type SetLayoutShapeBoundsResult struct {
	LayoutPartURI string
	ShapeID       int
	ShapeName     string
	OldX          int64
	OldY          int64
	OldCX         int64
	OldCY         int64
	NewX          int64
	NewY          int64
	NewCX         int64
	NewCY         int64
}

// CloneLayout clones an existing layout, keeps it attached to the same master,
// and optionally assigns a new layout name.
func CloneLayout(req *CloneLayoutRequest) (*CloneLayoutResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}

	layoutDoc, err := req.Package.ReadXMLPart(req.LayoutPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read layout: %w", err)
	}

	oldName := layoutName(layoutDoc.Root())
	newName := strings.TrimSpace(req.NewName)
	if newName == "" {
		newName = oldName
	}
	if err := setLayoutName(layoutDoc.Root(), newName); err != nil {
		return nil, err
	}

	sourceRels := req.Package.ListRelationships(req.LayoutPartURI)
	masterPartURI := findLayoutMasterURI(req.LayoutPartURI, sourceRels)
	if masterPartURI == "" {
		return nil, fmt.Errorf("layout %s is missing a slideMaster relationship", req.LayoutPartURI)
	}

	newLayoutURI, err := allocateNumberedPartName(req.Package, layoutPartNamePattern, "/ppt/slideLayouts/slideLayout%d.xml")
	if err != nil {
		return nil, err
	}

	layoutXML, err := writeXML(layoutDoc)
	if err != nil {
		return nil, fmt.Errorf("failed to serialize layout XML: %w", err)
	}
	if err := req.Package.AddPart(newLayoutURI, layoutXML, contentTypeOrDefault(req.Package, req.LayoutPartURI, layoutContentType), copyZipMeta(req.Package.GetZipMeta(req.LayoutPartURI))); err != nil {
		return nil, fmt.Errorf("failed to add cloned layout part: %w", err)
	}

	if len(sourceRels) > 0 {
		relsXML, err := BuildRelationshipsXML(sourceRels)
		if err != nil {
			return nil, fmt.Errorf("failed to build cloned layout relationships: %w", err)
		}
		if err := req.Package.AddPart(relsURIForPart(newLayoutURI), relsXML, relationshipsContentType, copyZipMeta(req.Package.GetZipMeta(relsURIForPart(req.LayoutPartURI)))); err != nil {
			return nil, fmt.Errorf("failed to add cloned layout relationships: %w", err)
		}
	}

	masterRels := req.Package.ListRelationships(masterPartURI)
	newRelID := AllocateRelationshipID(masterRels)
	newTarget, err := relationshipTarget(masterPartURI, newLayoutURI)
	if err != nil {
		return nil, fmt.Errorf("failed to compute master relationship target: %w", err)
	}
	masterRels = append(masterRels, opc.RelationshipInfo{
		SourceURI: masterPartURI,
		ID:        newRelID,
		Type:      slideLayoutRelationshipType,
		Target:    newTarget,
	})
	masterRelsXML, err := BuildRelationshipsXML(masterRels)
	if err != nil {
		return nil, fmt.Errorf("failed to serialize master relationships: %w", err)
	}
	if err := req.Package.ReplaceRawPart(relsURIForPart(masterPartURI), masterRelsXML, relationshipsContentType); err != nil {
		return nil, fmt.Errorf("failed to update master relationships: %w", err)
	}

	masterDoc, err := req.Package.ReadXMLPart(masterPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read master: %w", err)
	}
	layoutID, err := appendMasterLayoutReference(masterDoc.Root(), newRelID)
	if err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart(masterPartURI, masterDoc); err != nil {
		return nil, fmt.Errorf("failed to update master XML: %w", err)
	}

	return &CloneLayoutResult{
		SourceLayoutURI: req.LayoutPartURI,
		NewLayoutURI:    newLayoutURI,
		MasterPartURI:   masterPartURI,
		OldName:         oldName,
		NewName:         newName,
		RelationshipID:  newRelID,
		LayoutID:        layoutID,
	}, nil
}

// RenameLayout updates p:cSld@name on a layout.
func RenameLayout(req *RenameLayoutRequest) (*RenameLayoutResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}
	if strings.TrimSpace(req.NewName) == "" {
		return nil, fmt.Errorf("new layout name cannot be empty")
	}

	layoutDoc, err := req.Package.ReadXMLPart(req.LayoutPartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read layout: %w", err)
	}

	oldName := layoutName(layoutDoc.Root())
	if err := setLayoutName(layoutDoc.Root(), req.NewName); err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart(req.LayoutPartURI, layoutDoc); err != nil {
		return nil, fmt.Errorf("failed to write layout: %w", err)
	}

	return &RenameLayoutResult{
		LayoutPartURI: req.LayoutPartURI,
		OldName:       oldName,
		NewName:       req.NewName,
	}, nil
}

// DeleteLayoutShape removes a shape from a layout's shape tree.
func DeleteLayoutShape(req *DeleteLayoutShapeRequest) (*DeleteLayoutShapeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}
	if strings.TrimSpace(req.Target) == "" {
		return nil, fmt.Errorf("target cannot be empty")
	}

	layoutDoc, targetShape, err := resolveLayoutTarget(req.Package, req.LayoutPartURI, req.Target)
	if err != nil {
		return nil, err
	}
	if targetShape == nil {
		return nil, fmt.Errorf("target not found: %s", req.Target)
	}

	shapeID, _ := extractShapeID(targetShape)
	shapeName, _ := extractShapeName(targetShape)
	parent := targetShape.Parent()
	if parent == nil {
		return nil, fmt.Errorf("target shape has no parent")
	}
	parent.RemoveChild(targetShape)

	if err := req.Package.ReplaceXMLPart(req.LayoutPartURI, layoutDoc); err != nil {
		return nil, fmt.Errorf("failed to write layout: %w", err)
	}

	return &DeleteLayoutShapeResult{
		LayoutPartURI: req.LayoutPartURI,
		ShapeID:       shapeID,
		ShapeName:     shapeName,
	}, nil
}

// SetLayoutShapeBounds updates an existing layout shape's position and size.
func SetLayoutShapeBounds(req *SetLayoutShapeBoundsRequest) (*SetLayoutShapeBoundsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session cannot be nil")
	}
	if req.LayoutPartURI == "" {
		return nil, fmt.Errorf("layout part URI cannot be empty")
	}
	if strings.TrimSpace(req.Target) == "" {
		return nil, fmt.Errorf("target cannot be empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("bounds must be positive: cx=%d cy=%d", req.CX, req.CY)
	}

	layoutDoc, targetShape, err := resolveLayoutTarget(req.Package, req.LayoutPartURI, req.Target)
	if err != nil {
		return nil, err
	}
	if targetShape == nil {
		return nil, fmt.Errorf("target not found: %s", req.Target)
	}

	shapeID, _ := extractShapeID(targetShape)
	shapeName, _ := extractShapeName(targetShape)
	oldX, oldY, oldCX, oldCY, err := setShapeBounds(targetShape, req.X, req.Y, req.CX, req.CY)
	if err != nil {
		return nil, err
	}

	if err := req.Package.ReplaceXMLPart(req.LayoutPartURI, layoutDoc); err != nil {
		return nil, fmt.Errorf("failed to write layout: %w", err)
	}

	return &SetLayoutShapeBoundsResult{
		LayoutPartURI: req.LayoutPartURI,
		ShapeID:       shapeID,
		ShapeName:     shapeName,
		OldX:          oldX,
		OldY:          oldY,
		OldCX:         oldCX,
		OldCY:         oldCY,
		NewX:          req.X,
		NewY:          req.Y,
		NewCX:         req.CX,
		NewCY:         req.CY,
	}, nil
}

func resolveLayoutTarget(session opc.PackageSession, layoutPartURI, target string) (*etree.Document, *etree.Element, error) {
	layoutDoc, err := session.ReadXMLPart(layoutPartURI)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to read layout: %w", err)
	}
	root := layoutDoc.Root()
	if root == nil {
		return nil, nil, fmt.Errorf("layout root not found")
	}

	var masterRoot *etree.Element
	masterPartURI := findLayoutMasterURI(layoutPartURI, session.ListRelationships(layoutPartURI))
	if masterPartURI != "" {
		masterDoc, err := session.ReadXMLPart(masterPartURI)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to read layout master: %w", err)
		}
		masterRoot = masterDoc.Root()
	}

	targetShape, err := findTargetShape(root, root, masterRoot, target)
	if err != nil {
		return nil, nil, err
	}
	return layoutDoc, targetShape, nil
}

func layoutName(root *etree.Element) string {
	if root == nil {
		return ""
	}
	cSld := findCommonSlideData(root)
	if cSld == nil {
		return ""
	}
	if attr := cSld.SelectAttr("name"); attr != nil {
		return attr.Value
	}
	return ""
}

func setLayoutName(root *etree.Element, name string) error {
	if root == nil {
		return fmt.Errorf("layout root not found")
	}
	cSld := findCommonSlideData(root)
	if cSld == nil {
		return fmt.Errorf("layout common slide data not found")
	}
	if attr := cSld.SelectAttr("name"); attr != nil {
		attr.Value = name
	} else {
		cSld.CreateAttr("name", name)
	}
	return nil
}

func findCommonSlideData(root *etree.Element) *etree.Element {
	if root == nil {
		return nil
	}
	if cSld := root.FindElement(".//p:cSld"); cSld != nil {
		return cSld
	}
	if cSld := root.FindElement(".//cSld"); cSld != nil {
		return cSld
	}
	return nil
}

func findLayoutMasterURI(layoutPartURI string, rels []opc.RelationshipInfo) string {
	for _, rel := range rels {
		if strings.Contains(rel.Type, "slideMaster") {
			return opc.ResolveRelationshipTarget(layoutPartURI, rel.Target)
		}
	}
	return ""
}

func appendMasterLayoutReference(masterRoot *etree.Element, relID string) (uint32, error) {
	if masterRoot == nil {
		return 0, fmt.Errorf("master root not found")
	}
	layoutIDList := masterRoot.FindElement(".//p:sldLayoutIdLst")
	if layoutIDList == nil {
		layoutIDList = masterRoot.FindElement(".//sldLayoutIdLst")
	}
	if layoutIDList == nil {
		layoutIDList = etree.NewElement("p:sldLayoutIdLst")
		masterRoot.AddChild(layoutIDList)
	}

	newID := nextMasterLayoutID(layoutIDList)
	layoutID := etree.NewElement("p:sldLayoutId")
	layoutID.CreateAttr("id", strconv.FormatUint(uint64(newID), 10))
	layoutID.CreateAttr("r:id", relID)
	layoutIDList.AddChild(layoutID)
	return newID, nil
}

func nextMasterLayoutID(layoutIDList *etree.Element) uint32 {
	const base uint32 = 2147483649
	var maxID uint32

	children := layoutIDList.FindElements("p:sldLayoutId")
	if len(children) == 0 {
		children = layoutIDList.FindElements("sldLayoutId")
	}
	for _, child := range children {
		idStr := child.SelectAttrValue("id", "")
		if idStr == "" {
			continue
		}
		parsed, err := strconv.ParseUint(idStr, 10, 32)
		if err != nil {
			continue
		}
		if uint32(parsed) > maxID {
			maxID = uint32(parsed)
		}
	}
	if maxID < base {
		return base
	}
	return maxID + 1
}

func setShapeBounds(shape *etree.Element, x, y, cx, cy int64) (oldX, oldY, oldCX, oldCY int64, err error) {
	xfrm, err := shapeTransformElement(shape)
	if err != nil {
		return 0, 0, 0, 0, err
	}

	off := xfrm.FindElement("off")
	ext := xfrm.FindElement("ext")
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

	if off == nil {
		off = etree.NewElement("a:off")
		xfrm.AddChild(off)
	}
	off.RemoveAttr("x")
	off.RemoveAttr("y")
	off.CreateAttr("x", strconv.FormatInt(x, 10))
	off.CreateAttr("y", strconv.FormatInt(y, 10))

	if ext == nil {
		ext = etree.NewElement("a:ext")
		xfrm.AddChild(ext)
	}
	ext.RemoveAttr("cx")
	ext.RemoveAttr("cy")
	ext.CreateAttr("cx", strconv.FormatInt(cx, 10))
	ext.CreateAttr("cy", strconv.FormatInt(cy, 10))

	return oldX, oldY, oldCX, oldCY, nil
}

func shapeTransformElement(shape *etree.Element) (*etree.Element, error) {
	if shape == nil {
		return nil, fmt.Errorf("shape cannot be nil")
	}
	tag := shape.Tag
	switch {
	case tag == "graphicFrame" || strings.HasSuffix(tag, "}graphicFrame") || strings.HasSuffix(tag, ":graphicFrame"):
		xfrm := shape.FindElement("xfrm")
		if xfrm == nil {
			xfrm = etree.NewElement("p:xfrm")
			shape.InsertChildAt(0, xfrm)
		}
		return xfrm, nil
	default:
		spPr := shape.FindElement("spPr")
		if spPr == nil {
			spPr = etree.NewElement("p:spPr")
			shape.AddChild(spPr)
		}
		xfrm := spPr.FindElement("xfrm")
		if xfrm == nil {
			xfrm = etree.NewElement("a:xfrm")
			if len(spPr.ChildElements()) > 0 {
				spPr.InsertChildAt(0, xfrm)
			} else {
				spPr.AddChild(xfrm)
			}
		}
		return xfrm, nil
	}
}
