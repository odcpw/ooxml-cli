package cli

import (
	"errors"
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

type PPTXShapesResult struct {
	File          string           `json:"file"`
	Slide         int              `json:"slide"`
	PartURI       string           `json:"partUri"`
	LayoutName    string           `json:"layoutName,omitempty"`
	LayoutPartURI string           `json:"layoutPartUri,omitempty"`
	Shapes        []PPTXShapeEntry `json:"shapes"`
}

type PPTXShapeEntry struct {
	Order           int                                    `json:"order"`
	ShapeID         int                                    `json:"shapeId"`
	ShapeName       string                                 `json:"shapeName,omitempty"`
	ShapeType       model.ShapeType                        `json:"shapeType"`
	TargetKind      string                                 `json:"targetKind"`
	PrimarySelector string                                 `json:"primarySelector"`
	Handle          string                                 `json:"handle,omitempty"`
	Selectors       []string                               `json:"selectors"`
	TextCapable     bool                                   `json:"textCapable"`
	TextPreview     string                                 `json:"textPreview,omitempty"`
	Placeholder     *pptselectors.SlideSelectorPlaceholder `json:"placeholder,omitempty"`
	Bounds          *model.Bounds                          `json:"bounds,omitempty"`
	Geometry        *model.Geometry                        `json:"geometry,omitempty"`
	ImageRef        *model.ImageRef                        `json:"imageRef,omitempty"`
	TableInfo       *model.TableInfo                       `json:"tableInfo,omitempty"`
}

type PPTXShapeDestination struct {
	File            string          `json:"file,omitempty"`
	Slide           int             `json:"slide"`
	Target          string          `json:"target,omitempty"`
	ShapeID         int             `json:"shapeId"`
	ShapeName       string          `json:"shapeName,omitempty"`
	TargetKind      string          `json:"targetKind"`
	PrimarySelector string          `json:"primarySelector"`
	Handle          string          `json:"handle,omitempty"`
	Selectors       []string        `json:"selectors"`
	TextPreview     string          `json:"textPreview,omitempty"`
	Bounds          *model.Bounds   `json:"bounds,omitempty"`
	Geometry        *model.Geometry `json:"geometry,omitempty"`
	ImageRef        *model.ImageRef `json:"imageRef,omitempty"`
}

func collectPPTXShapeEntries(pkg opc.PackageSession, catalog *pptselectors.SlideCatalog, includeText, includeBounds bool) ([]PPTXShapeEntry, error) {
	if catalog == nil {
		return nil, fmt.Errorf("selector catalog is nil")
	}
	shapeInfoByID := map[int]model.ShapeInfo{}
	spTree := findPPTXShapeTree(catalog.SlideDocument().Root())
	if spTree != nil {
		shapeInfos := inspect.EnumerateShapes(spTree)
		if includeText {
			attachPPTXSlideText(spTree, shapeInfos)
		}
		attachPPTXSlideImageRefs(pkg, catalog.SlidePartURI, shapeInfos)
		for _, shapeInfo := range shapeInfos {
			shapeInfoByID[shapeInfo.ID] = shapeInfo
		}
	}
	_ = pkg

	entries := make([]PPTXShapeEntry, 0, len(catalog.Targets))
	for _, target := range catalog.Targets {
		shapeInfo := shapeInfoByID[target.ShapeID]
		entry := PPTXShapeEntry{
			Order:           target.Order,
			ShapeID:         target.ShapeID,
			ShapeName:       target.ShapeName,
			ShapeType:       target.ShapeType,
			TargetKind:      target.TargetKind,
			PrimarySelector: target.PrimarySelector,
			Handle:          pptxShapeHandle(catalog, target.ShapeID),
			Selectors:       append([]string{}, target.Selectors...),
			TextCapable:     target.TextCapable,
			Placeholder:     target.Placeholder,
			ImageRef:        shapeInfo.ImageRef,
			TableInfo:       shapeInfo.TableInfo,
		}
		if includeText {
			entry.TextPreview = nonEmpty(target.TextPreview, strings.TrimSpace(shapeInfo.TextContent))
		}
		if includeBounds {
			entry.Bounds = shapeInfo.Bounds
			entry.Geometry = shapeInfo.Geometry
		}
		entries = append(entries, entry)
	}
	return entries, nil
}

func collectPPTXShapeDestination(pkg opc.PackageSession, slide int, targetSelector string, destinationFile string, includeText, includeBounds bool) (*PPTXShapeDestination, error) {
	var (
		catalog *pptselectors.SlideCatalog
		target  *pptselectors.SlideSelectorTarget
		err     error
	)
	if pptxhandle.IsHandle(targetSelector) {
		// Handle target: the handle's sldId selects the slide, not the slide arg.
		catalog, target, _, err = pptselectors.ResolvePPTXShapeHandle(pkg, targetSelector)
		if err != nil {
			return nil, mapPPTXHandleError(err)
		}
		slide = catalog.SlideNumber
	} else {
		catalog, err = pptselectors.BuildSlideCatalog(pkg, slide)
		if err != nil {
			return nil, mapPPTXShapeCatalogError(err)
		}
		target, err = catalog.ResolveTarget(targetSelector)
		if err != nil {
			return nil, mapPPTXShapeResolveError(err, catalog, targetSelector, slide)
		}
	}
	entries, err := collectPPTXShapeEntries(pkg, catalog, includeText, includeBounds)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to collect slide shapes: %v", err)
	}
	for _, entry := range entries {
		if entry.ShapeID != target.ShapeID {
			continue
		}
		return &PPTXShapeDestination{
			File:            destinationFile,
			Slide:           slide,
			Target:          targetSelector,
			ShapeID:         entry.ShapeID,
			ShapeName:       entry.ShapeName,
			TargetKind:      entry.TargetKind,
			PrimarySelector: entry.PrimarySelector,
			Handle:          entry.Handle,
			Selectors:       append([]string{}, entry.Selectors...),
			TextPreview:     entry.TextPreview,
			Bounds:          entry.Bounds,
			Geometry:        entry.Geometry,
			ImageRef:        entry.ImageRef,
		}, nil
	}
	return nil, TargetNotFoundError(targetSelector)
}

func attachPPTXSlideImageRefs(pkg opc.PackageSession, slidePartURI string, shapeInfos []model.ShapeInfo) {
	if pkg == nil || slidePartURI == "" {
		return
	}
	relationships := pkg.ListRelationships(slidePartURI)
	if len(relationships) == 0 {
		return
	}
	relByID := make(map[string]opc.RelationshipInfo, len(relationships))
	for _, rel := range relationships {
		relByID[rel.ID] = rel
	}
	for i := range shapeInfos {
		imageRef := shapeInfos[i].ImageRef
		if imageRef == nil || imageRef.RelID == "" {
			continue
		}
		rel, ok := relByID[imageRef.RelID]
		if !ok {
			continue
		}
		targetURI := opc.NormalizeURI(opc.ResolveRelationshipTarget(slidePartURI, rel.Target))
		imageRef.TargetURI = targetURI
		imageRef.ContentType = pkg.GetContentType(targetURI)
	}
}

func outputPPTXShapesJSON(cmd *cobra.Command, result *PPTXShapesResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal shapes JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputPPTXShapesText(cmd *cobra.Command, result *PPTXShapesResult) error {
	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("Slide %d: %s\n", result.Slide, result.PartURI))
	if result.LayoutName != "" {
		builder.WriteString(fmt.Sprintf("Layout: %s\n", result.LayoutName))
	}
	for _, shape := range result.Shapes {
		builder.WriteString(fmt.Sprintf("  [%d] %s", shape.Order, shape.PrimarySelector))
		if shape.ShapeName != "" {
			builder.WriteString(fmt.Sprintf(" (%s)", shape.ShapeName))
		}
		builder.WriteString(fmt.Sprintf(" id=%d type=%s kind=%s\n", shape.ShapeID, shape.ShapeType, shape.TargetKind))
		if shape.Bounds != nil {
			builder.WriteString(fmt.Sprintf("      bounds: x=%d y=%d cx=%d cy=%d\n", shape.Bounds.X, shape.Bounds.Y, shape.Bounds.CX, shape.Bounds.CY))
		}
		if shape.TextPreview != "" {
			builder.WriteString(fmt.Sprintf("      text: %q\n", shape.TextPreview))
		}
	}
	return writeCLIOutput(cmd, []byte(builder.String()))
}

func mapPPTXShapeCatalogError(err error) error {
	if err == nil {
		return nil
	}
	msg := err.Error()
	if strings.Contains(msg, "not found (presentation has") || strings.Contains(msg, "slide must be") {
		return InvalidArgsError(msg)
	}
	if strings.Contains(msg, "target not found") || strings.Contains(msg, "ambiguous target") {
		return TargetNotFoundError(msg)
	}
	return NewCLIErrorf(ExitUnexpected, "%v", err)
}

// mapPPTXShapeResolveError is like mapPPTXShapeCatalogError but, for the
// "target not found" case, enriches the message with nearby valid shape selectors
// drawn from the slide catalog and a discovery command. The exit code is unchanged.
func mapPPTXShapeResolveError(err error, catalog *pptselectors.SlideCatalog, selector string, slide int) error {
	if err == nil {
		return nil
	}
	if strings.Contains(err.Error(), "target not found") && catalog != nil {
		candidates := BuildSelectorCandidates(pptxShapeSelectorCandidates(catalog), selector, maxSelectorCandidates)
		discovery := fmt.Sprintf("ooxml --json pptx shapes show <file> --slide %d", slide)
		if len(candidates) > 0 {
			return SelectorNotFoundError("shape", selector, candidates, discovery)
		}
		return TargetNotFoundError(err.Error() + "; discover with `" + discovery + "`")
	}
	return mapPPTXShapeCatalogError(err)
}

// pptxShapeHandle mints a stable shape handle (H:pptx/s:<sldId>/shape:n:<id>)
// for a shape on the catalog's slide. It returns "" when the slide has no
// native sldId (handles require a scope id) OR when the cNvPr@id is not unique
// on the slide (a handle for a duplicated id would mis-resolve, so we never mint
// one), so the field is simply omitted.
func pptxShapeHandle(catalog *pptselectors.SlideCatalog, shapeID int) string {
	if catalog == nil || catalog.SlideID == 0 || catalog.IsSlideIDAmbiguous() {
		return ""
	}
	if catalog.IsShapeIDAmbiguous(shapeID) {
		return ""
	}
	return pptxhandle.FormatShape(catalog.SlideID, shapeID)
}

// mapPPTXHandleError maps a typed handle error to a CLI error with the right
// exit code. A stale or scope-stale handle is a "not found" condition (the
// addressed object is gone); a malformed handle is an invalid-args condition.
func mapPPTXHandleError(err error) error {
	if err == nil {
		return nil
	}
	switch {
	case pptxhandle.IsCode(err, pptxhandle.CodeMalformed),
		pptxhandle.IsCode(err, pptxhandle.CodeFormatMismatch):
		return pptxHandleCLIError(err, ExitInvalidArgs)
	case pptxhandle.IsCode(err, pptxhandle.CodeScopeStale),
		pptxhandle.IsCode(err, pptxhandle.CodeStale),
		pptxhandle.IsCode(err, pptxhandle.CodeAmbiguous):
		return pptxHandleCLIError(err, ExitTargetNotFound)
	default:
		return NewCLIErrorf(ExitUnexpected, "%v", err)
	}
}

func pptxHandleCLIError(err error, exitCode int) *CLIError {
	var herr *pptxhandle.Error
	if errors.As(err, &herr) && herr.Code != "" {
		return &CLIError{ExitCode: exitCode, Code: herr.Code, Message: err.Error()}
	}
	return &CLIError{ExitCode: exitCode, Message: err.Error()}
}

func pptxShapeSelectorCandidates(catalog *pptselectors.SlideCatalog) []SelectorCandidate {
	if catalog == nil {
		return nil
	}
	out := make([]SelectorCandidate, 0, len(catalog.Targets))
	for _, target := range catalog.Targets {
		out = append(out, SelectorCandidate{Primary: target.PrimarySelector, Selectors: target.Selectors})
	}
	return out
}
