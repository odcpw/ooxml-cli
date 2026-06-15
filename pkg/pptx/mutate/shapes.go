package mutate

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

type SetSlideShapeBoundsRequest struct {
	Package     opc.PackageSession
	SlideNumber int
	Target      string
	X           int64
	Y           int64
	CX          int64
	CY          int64
}

type SetSlideShapeBoundsResult struct {
	Slide     int             `json:"slide"`
	PartURI   string          `json:"partUri"`
	ShapeID   int             `json:"shapeId"`
	ShapeName string          `json:"shapeName"`
	ShapeType model.ShapeType `json:"shapeType"`
	Target    string          `json:"target"`
	OldX      int64           `json:"oldX"`
	OldY      int64           `json:"oldY"`
	OldCX     int64           `json:"oldCx"`
	OldCY     int64           `json:"oldCy"`
	NewX      int64           `json:"newX"`
	NewY      int64           `json:"newY"`
	NewCX     int64           `json:"newCx"`
	NewCY     int64           `json:"newCy"`
}

type DeleteSlideShapeRequest struct {
	Package     opc.PackageSession
	SlideNumber int
	Target      string
}

type DeleteSlideShapeResult struct {
	Slide     int             `json:"slide"`
	PartURI   string          `json:"partUri"`
	ShapeID   int             `json:"shapeId"`
	ShapeName string          `json:"shapeName"`
	ShapeType model.ShapeType `json:"shapeType"`
	Target    string          `json:"target"`
}

func SetSlideShapeBounds(req *SetSlideShapeBoundsRequest) (*SetSlideShapeBoundsResult, error) {
	if req == nil {
		return nil, fmt.Errorf("set slide shape bounds request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SlideNumber < 1 {
		return nil, fmt.Errorf("slide must be >= 1")
	}
	if req.Target == "" {
		return nil, fmt.Errorf("target selector cannot be empty")
	}
	if req.CX <= 0 || req.CY <= 0 {
		return nil, fmt.Errorf("bounds width and height must be positive")
	}

	catalog, err := selectors.BuildSlideCatalog(req.Package, req.SlideNumber)
	if err != nil {
		return nil, err
	}
	target, shapeElem, err := catalog.ResolveTargetElement(req.Target)
	if err != nil {
		return nil, err
	}
	if target.ShapeType == model.ShapeTypeGroup {
		return nil, fmt.Errorf("group shape bounds mutation is not supported in this slice: shape:%d", target.ShapeID)
	}

	oldX, oldY, oldCX, oldCY, err := setShapeBounds(shapeElem, req.X, req.Y, req.CX, req.CY)
	if err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart(catalog.SlidePartURI, catalog.SlideDocument()); err != nil {
		return nil, fmt.Errorf("failed to replace slide %s: %w", catalog.SlidePartURI, err)
	}

	return &SetSlideShapeBoundsResult{
		Slide:     catalog.SlideNumber,
		PartURI:   catalog.SlidePartURI,
		ShapeID:   target.ShapeID,
		ShapeName: target.ShapeName,
		ShapeType: target.ShapeType,
		Target:    target.PrimarySelector,
		OldX:      oldX,
		OldY:      oldY,
		OldCX:     oldCX,
		OldCY:     oldCY,
		NewX:      req.X,
		NewY:      req.Y,
		NewCX:     req.CX,
		NewCY:     req.CY,
	}, nil
}

func DeleteSlideShape(req *DeleteSlideShapeRequest) (*DeleteSlideShapeResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete slide shape request is nil")
	}
	if req.Package == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.SlideNumber < 1 {
		return nil, fmt.Errorf("slide must be >= 1")
	}
	if req.Target == "" {
		return nil, fmt.Errorf("target selector cannot be empty")
	}

	catalog, err := selectors.BuildSlideCatalog(req.Package, req.SlideNumber)
	if err != nil {
		return nil, err
	}
	target, shapeElem, err := catalog.ResolveTargetElement(req.Target)
	if err != nil {
		return nil, err
	}
	if target.ShapeType == model.ShapeTypeGroup {
		return nil, fmt.Errorf("group shape deletion is not supported in this slice: shape:%d", target.ShapeID)
	}
	parent := shapeElem.Parent()
	if parent == nil {
		return nil, fmt.Errorf("shape:%d has no parent shape tree", target.ShapeID)
	}
	parent.RemoveChild(shapeElem)
	if err := req.Package.ReplaceXMLPart(catalog.SlidePartURI, catalog.SlideDocument()); err != nil {
		return nil, fmt.Errorf("failed to replace slide %s: %w", catalog.SlidePartURI, err)
	}

	return &DeleteSlideShapeResult{
		Slide:     catalog.SlideNumber,
		PartURI:   catalog.SlidePartURI,
		ShapeID:   target.ShapeID,
		ShapeName: target.ShapeName,
		ShapeType: target.ShapeType,
		Target:    target.PrimarySelector,
	}, nil
}
