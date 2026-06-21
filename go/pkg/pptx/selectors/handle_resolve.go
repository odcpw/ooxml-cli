package selectors

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// ResolveSlideNumberForHandle resolves a handle's slide scope to a 1-based slide
// number by SEARCHING graph.Slides for the matching native p:sldId@id. This is
// what delivers cross-slide structural-edit survival: the slide number is
// derived from the durable sldId, not from any positional --slide value, so a
// handle keeps pointing at the same slide after other slides are inserted,
// deleted, or reordered.
//
// A scope that no longer exists yields a typed CodeScopeStale error; a sldId
// shared by more than one slide yields a typed CodeAmbiguous error.
func ResolveSlideNumberForHandle(graph *inspect.PresentationGraph, h handle.Handle) (int, error) {
	ref, err := ResolveSlideRefForHandle(graph, h)
	if err != nil {
		return 0, err
	}
	return ref.SlideNumber, nil
}

// ResolveSlideRefForHandle resolves a handle's slide scope to the single
// SlideRef whose native p:sldId@id matches. It is the shared scope-resolution
// primitive: every handle path (shape resolution, animations add, image
// replace) routes through it so the duplicate-sldId ambiguity contract is
// enforced uniformly and NEVER silently first-wins on a duplicate.
//
// A scope that no longer exists yields a typed CodeScopeStale error; a sldId
// shared by MORE THAN ONE slide yields a typed CodeAmbiguous error.
func ResolveSlideRefForHandle(graph *inspect.PresentationGraph, h handle.Handle) (*inspect.SlideRef, error) {
	if graph == nil {
		return nil, fmt.Errorf("presentation graph cannot be nil")
	}
	var (
		matches []*inspect.SlideRef
	)
	for i := range graph.Slides {
		if graph.Slides[i].SlideID == h.SlideID {
			matches = append(matches, &graph.Slides[i])
		}
	}
	switch len(matches) {
	case 0:
		return nil, &handle.Error{
			Code:    handle.CodeScopeStale,
			Handle:  handle.Format(h),
			Message: fmt.Sprintf("no slide with sldId %d in presentation", h.SlideID),
		}
	case 1:
		return matches[0], nil
	default:
		return nil, &handle.Error{
			Code:    handle.CodeAmbiguous,
			Handle:  handle.Format(h),
			Message: fmt.Sprintf("sldId %d is not unique in presentation (%d slides share it); cannot resolve to a single slide", h.SlideID, len(matches)),
		}
	}
}

// ResolvePPTXShapeHandle decodes a shape handle string, locates its scope slide
// by sldId, builds that slide's catalog, and resolves the shape by its native
// cNvPr@id. The returned catalog is the mutable slide catalog (use
// catalog.SlideDocument() and persist via the package session) and the element
// is the backing shape XML.
//
// The handle is authoritative for its own scope: any --slide value the caller
// may also hold is irrelevant here and must NOT be used for resolution.
//
// Errors are typed handle.Error values: CodeMalformed (bad envelope or a
// slide-only handle), CodeScopeStale (slide gone), CodeStale (shape gone).
func ResolvePPTXShapeHandle(pkg opc.PackageSession, handleStr string) (*SlideCatalog, *SlideSelectorTarget, *etree.Element, error) {
	h, err := handle.Parse(handleStr)
	if err != nil {
		return nil, nil, nil, err
	}
	if h.Kind != handle.KindShape {
		return nil, nil, nil, &handle.Error{Code: handle.CodeMalformed, Handle: handleStr, Message: "expected a shape handle, got a slide handle"}
	}
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		return nil, nil, nil, fmt.Errorf("failed to parse presentation: %w", err)
	}
	slideNumber, err := ResolveSlideNumberForHandle(graph, h)
	if err != nil {
		return nil, nil, nil, err
	}
	catalog, err := BuildSlideCatalogFromGraph(pkg, graph, slideNumber)
	if err != nil {
		return nil, nil, nil, err
	}
	target, elem, err := catalog.ResolveHandleShape(h)
	if err != nil {
		return nil, nil, nil, err
	}
	return catalog, target, elem, nil
}
