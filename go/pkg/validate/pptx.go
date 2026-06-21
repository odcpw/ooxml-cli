package validate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

const (
	pptxContentTypePresentation = "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"
	pptxContentTypeSlide        = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
	pptxContentTypeSlideLayout  = "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"
	pptxContentTypeNotesSlide   = "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"

	pptxRelTypeSlide       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"
	pptxRelTypeSlideLayout = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"
	pptxRelTypeNotesSlide  = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide"
	pptxRelTypeHyperlink   = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"
	pptxRelTypeImage       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
	pptxRelTypeVideo       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video"
	pptxRelTypeAudio       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio"
	pptxRelTypeMedia       = "http://schemas.microsoft.com/office/2007/relationships/media"
)

// validatePPTXSemantics validates Stage 3: PPTX structure, slide/layout/master hierarchy.
// This validates that slides reference valid layouts, layouts reference valid masters, etc.
func validatePPTXSemantics(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	// Build a map of all parts before graph parsing so preflight diagnostics can
	// report broken presentation slide IDs even when ParsePresentation aborts.
	parts := session.ListParts()
	partMap := make(map[string]bool)
	partByURI := make(map[string]opc.PartInfo)
	for _, part := range parts {
		partMap[part.URI] = true
		partByURI[part.URI] = part
	}

	diags = append(diags, validatePresentationSlideIDRelationships(session, partByURI)...)

	// Try to parse the presentation graph
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		// Presentation parsing failed, but this is an error to report, not a blocker
		diags = append(diags, diag.Error(
			"PPTX_PARSE_ERROR",
			"failed to parse presentation structure: "+err.Error(),
		))
		return diags, nil
	}

	// Validate masters
	for i, master := range graph.Masters {
		if !partMap[master.PartURI] {
			diags = append(diags, diag.Error(
				"PPTX_MISSING_MASTER",
				fmt.Sprintf("master %d part not found: %s", i+1, master.PartURI),
			))
			continue
		}

		// Validate theme reference
		if master.ThemeURI != "" && !partMap[master.ThemeURI] {
			diags = append(diags, diag.Warning(
				"PPTX_MISSING_THEME",
				fmt.Sprintf("master %d theme not found: %s", i+1, master.ThemeURI),
			))
		}

		// Validate layout references
		for _, layoutURI := range master.LinkedLayoutURIs {
			if !partMap[layoutURI] {
				diags = append(diags, diag.Error(
					"PPTX_MISSING_LAYOUT",
					fmt.Sprintf("master %d references missing layout: %s", i+1, layoutURI),
				))
			}
		}
	}

	// Validate layouts
	for i, layout := range graph.Layouts {
		if !partMap[layout.PartURI] {
			diags = append(diags, diag.Error(
				"PPTX_MISSING_LAYOUT",
				fmt.Sprintf("layout %d part not found: %s", i+1, layout.PartURI),
			))
			continue
		}

		// Validate master reference
		if layout.MasterPartURI != "" && !partMap[layout.MasterPartURI] {
			diags = append(diags, diag.Error(
				"PPTX_DANGLING_LAYOUT",
				fmt.Sprintf("layout %d references missing master: %s", i+1, layout.MasterPartURI),
			))
		}
	}

	// Validate slides
	for _, slide := range graph.Slides {
		if !partMap[slide.PartURI] {
			diags = append(diags, diag.Error(
				"PPTX_MISSING_SLIDE",
				fmt.Sprintf("slide %d part not found: %s", slide.SlideNumber, slide.PartURI),
			))
			continue
		}

		// Validate layout reference
		if slide.LayoutPartURI != "" && !partMap[slide.LayoutPartURI] {
			diags = append(diags, diag.Error(
				"PPTX_DANGLING_LAYOUT",
				fmt.Sprintf("slide %d references missing layout: %s", slide.SlideNumber, slide.LayoutPartURI),
			))
		}

		// Validate notes reference if present
		if slide.NotesPartURI != "" && !partMap[slide.NotesPartURI] {
			diags = append(diags, diag.Warning(
				"PPTX_MISSING_NOTES",
				fmt.Sprintf("slide %d references missing notes: %s", slide.SlideNumber, slide.NotesPartURI),
			))
		}

		// Validate media relationships
		slideRels := session.ListRelationships(slide.PartURI)
		for _, rel := range slideRels {
			// Check for image/media relationships
			if isMediaRelationshipType(rel.Type) && rel.TargetMode != "External" {
				targetURI := opc.ResolveRelationshipTarget(slide.PartURI, rel.Target)
				if !partMap[targetURI] {
					diags = append(diags, diag.Warning(
						"PPTX_MISSING_MEDIA",
						fmt.Sprintf("slide %d references missing media: %s", slide.SlideNumber, targetURI),
					))
				}
			}
		}
		diags = append(diags, validateSlideRelationshipGraph(session, &slide, partByURI)...)

		// M12-1: Validate shape and geometry within slide
		slideDiags, err := validateSlideShapeStructure(session, &slide)
		if err != nil {
			diags = append(diags, diag.Warning(
				"PPTX_SHAPE_VALIDATION_ERROR",
				fmt.Sprintf("slide %d shape validation error: %v", slide.SlideNumber, err),
			))
		} else {
			diags = append(diags, slideDiags...)
		}
	}

	animationDiags, err := validatePPTXAnimationGraph(session)
	if err != nil {
		diags = append(diags, diag.Warning(
			"PPTX_ANIMATION_VALIDATION_ERROR",
			"failed to validate animation graph: "+err.Error(),
		))
	} else {
		diags = append(diags, animationDiags...)
	}

	diags = append(diags, validateChartParts(session)...)
	return diags, nil
}

// isMediaRelationshipType checks if a relationship type points to media content.
func isMediaRelationshipType(relType string) bool {
	mediaTypes := []string{
		pptxRelTypeImage,
		pptxRelTypeVideo,
		pptxRelTypeAudio,
		pptxRelTypeMedia,
	}

	for _, mt := range mediaTypes {
		if relType == mt {
			return true
		}
	}
	return false
}

func validatePresentationSlideIDRelationships(session opc.PackageSession, partByURI map[string]opc.PartInfo) []result.Diagnostic {
	var diags []result.Diagnostic

	doc, err := session.ReadXMLPart("/ppt/presentation.xml")
	if err != nil || doc == nil || doc.Root() == nil {
		return diags
	}

	relByID := relationshipMap(session.ListRelationships("/ppt/presentation.xml"))
	slideIDList := namespaces.FindChild(doc.Root(), namespaces.NsP, "sldIdLst")
	for i, slideID := range namespaces.FindChildren(slideIDList, namespaces.NsP, "sldId") {
		slideNumber := i + 1
		rid := relationshipIDAttr(slideID, "id")
		if rid == "" {
			diags = append(diags, diag.Error(
				"PPTX_SLIDE_ID_MISSING_REL_ID",
				fmt.Sprintf("presentation slide %d has no relationship id", slideNumber),
			))
			continue
		}

		rel, ok := relByID[rid]
		if !ok {
			diags = append(diags, diag.Error(
				"PPTX_SLIDE_ID_MISSING_RELATIONSHIP",
				fmt.Sprintf("presentation slide %d references missing relationship %s", slideNumber, rid),
			))
			continue
		}
		if rel.TargetMode == "External" {
			diags = append(diags, diag.Error(
				"PPTX_SLIDE_ID_EXTERNAL_TARGET",
				fmt.Sprintf("presentation slide %d relationship %s points to an external target", slideNumber, rid),
			))
			continue
		}
		if rel.Type != pptxRelTypeSlide {
			diags = append(diags, diag.Error(
				"PPTX_SLIDE_ID_WRONG_REL_TYPE",
				fmt.Sprintf("presentation slide %d relationship %s has type %s, want %s", slideNumber, rid, rel.Type, pptxRelTypeSlide),
			))
		}

		targetURI := opc.ResolveRelationshipTarget("/ppt/presentation.xml", rel.Target)
		part, ok := partByURI[targetURI]
		if !ok {
			diags = append(diags, diag.Error(
				"PPTX_SLIDE_ID_MISSING_TARGET",
				fmt.Sprintf("presentation slide %d relationship %s points to missing slide part: %s", slideNumber, rid, targetURI),
			))
			continue
		}
		if !strings.HasPrefix(targetURI, "/ppt/slides/") || part.ContentType != pptxContentTypeSlide {
			diags = append(diags, diag.Error(
				"PPTX_SLIDE_ID_WRONG_TARGET_TYPE",
				fmt.Sprintf("presentation slide %d relationship %s points to non-slide part %s with content type %s", slideNumber, rid, targetURI, part.ContentType),
			))
		}
	}

	return diags
}

func validateSlideRelationshipGraph(session opc.PackageSession, slide *inspect.SlideRef, partByURI map[string]opc.PartInfo) []result.Diagnostic {
	var diags []result.Diagnostic

	rels := session.ListRelationships(slide.PartURI)
	relByID := relationshipMap(rels)
	for _, rel := range rels {
		if rel.TargetMode == "External" {
			continue
		}
		switch rel.Type {
		case pptxRelTypeSlideLayout:
			diags = append(diags, validateTypedRelationshipTarget(slide, rel, pptxContentTypeSlideLayout, "/ppt/slideLayouts/", partByURI, "PPTX_WRONG_LAYOUT_TARGET")...)
		case pptxRelTypeNotesSlide:
			diags = append(diags, validateTypedRelationshipTarget(slide, rel, pptxContentTypeNotesSlide, "/ppt/notesSlides/", partByURI, "PPTX_WRONG_NOTES_TARGET")...)
			diags = append(diags, validateNotesSlideBacklink(session, slide, rel, partByURI)...)
		case namespaces.RelComments:
			diags = append(diags, validateTypedRelationshipTarget(slide, rel, namespaces.ContentTypeComments, "/ppt/comments/", partByURI, "PPTX_WRONG_COMMENTS_TARGET")...)
		case pptxRelTypeImage:
			diags = append(diags, validateMediaLikeRelationshipTarget(slide, rel, partByURI, "PPTX_WRONG_IMAGE_TARGET")...)
		case pptxRelTypeVideo, pptxRelTypeAudio, pptxRelTypeMedia:
			diags = append(diags, validateMediaLikeRelationshipTarget(slide, rel, partByURI, "PPTX_WRONG_MEDIA_TARGET")...)
		}
	}

	doc, err := session.ReadXMLPart(slide.PartURI)
	if err != nil || doc == nil || doc.Root() == nil {
		return diags
	}
	for _, blip := range namespaces.FindDescendants(doc.Root(), namespaces.NsA, "blip") {
		if rid := relationshipIDAttr(blip, "embed"); rid != "" {
			diags = append(diags, validateSlideXMLRelationshipReference(slide, rid, relByID, []string{pptxRelTypeImage}, "image embed")...)
		}
		if rid := relationshipIDAttr(blip, "link"); rid != "" {
			diags = append(diags, validateSlideXMLRelationshipReference(slide, rid, relByID, []string{pptxRelTypeImage}, "linked image")...)
		}
	}
	for _, hlink := range append(namespaces.FindDescendants(doc.Root(), namespaces.NsA, "hlinkClick"), namespaces.FindDescendants(doc.Root(), namespaces.NsA, "hlinkMouseOver")...) {
		if rid := relationshipIDAttr(hlink, "id"); rid != "" {
			diags = append(diags, validateSlideXMLRelationshipReference(slide, rid, relByID, []string{pptxRelTypeHyperlink}, "hyperlink")...)
		}
	}
	for _, media := range namespaces.FindDescendants(doc.Root(), namespaces.Np14, "media") {
		if rid := relationshipIDAttr(media, "embed"); rid != "" {
			diags = append(diags, validateSlideXMLRelationshipReference(slide, rid, relByID, []string{pptxRelTypeMedia}, "embedded media")...)
		}
	}
	for _, video := range namespaces.FindDescendants(doc.Root(), namespaces.NsA, "videoFile") {
		if rid := relationshipIDAttr(video, "link"); rid != "" {
			diags = append(diags, validateSlideXMLRelationshipReference(slide, rid, relByID, []string{pptxRelTypeVideo, pptxRelTypeMedia}, "video media")...)
		}
	}
	for _, audio := range namespaces.FindDescendants(doc.Root(), namespaces.NsA, "audioFile") {
		if rid := relationshipIDAttr(audio, "link"); rid != "" {
			diags = append(diags, validateSlideXMLRelationshipReference(slide, rid, relByID, []string{pptxRelTypeAudio, pptxRelTypeMedia}, "audio media")...)
		}
	}

	return diags
}

func validateTypedRelationshipTarget(slide *inspect.SlideRef, rel opc.RelationshipInfo, contentType, pathPrefix string, partByURI map[string]opc.PartInfo, code string) []result.Diagnostic {
	targetURI := opc.ResolveRelationshipTarget(slide.PartURI, rel.Target)
	part, ok := partByURI[targetURI]
	if !ok {
		return nil
	}
	if !strings.HasPrefix(targetURI, pathPrefix) || part.ContentType != contentType {
		return []result.Diagnostic{diag.Error(
			code,
			fmt.Sprintf("slide %d relationship %s points to %s with content type %s", slide.SlideNumber, rel.ID, targetURI, part.ContentType),
		)}
	}
	return nil
}

func validateMediaLikeRelationshipTarget(slide *inspect.SlideRef, rel opc.RelationshipInfo, partByURI map[string]opc.PartInfo, code string) []result.Diagnostic {
	targetURI := opc.ResolveRelationshipTarget(slide.PartURI, rel.Target)
	part, ok := partByURI[targetURI]
	if !ok {
		return nil
	}
	if !strings.HasPrefix(targetURI, "/ppt/media/") {
		return []result.Diagnostic{diag.Error(
			code,
			fmt.Sprintf("slide %d relationship %s points outside /ppt/media/: %s", slide.SlideNumber, rel.ID, targetURI),
		)}
	}
	if rel.Type == pptxRelTypeImage && !strings.HasPrefix(part.ContentType, "image/") {
		return []result.Diagnostic{diag.Error(
			code,
			fmt.Sprintf("slide %d image relationship %s targets non-image part %s with content type %s", slide.SlideNumber, rel.ID, targetURI, part.ContentType),
		)}
	}
	return nil
}

func validateNotesSlideBacklink(session opc.PackageSession, slide *inspect.SlideRef, notesRel opc.RelationshipInfo, partByURI map[string]opc.PartInfo) []result.Diagnostic {
	targetURI := opc.ResolveRelationshipTarget(slide.PartURI, notesRel.Target)
	if _, ok := partByURI[targetURI]; !ok {
		return nil
	}
	for _, rel := range session.ListRelationships(targetURI) {
		if rel.TargetMode == "External" || rel.Type != pptxRelTypeSlide {
			continue
		}
		if opc.ResolveRelationshipTarget(targetURI, rel.Target) == slide.PartURI {
			return nil
		}
	}
	return []result.Diagnostic{diag.Error(
		"PPTX_NOTES_MISSING_SLIDE_BACKLINK",
		fmt.Sprintf("slide %d notes part %s does not link back to %s", slide.SlideNumber, targetURI, slide.PartURI),
	)}
}

func validateSlideXMLRelationshipReference(slide *inspect.SlideRef, rid string, relByID map[string]opc.RelationshipInfo, allowedTypes []string, description string) []result.Diagnostic {
	rel, ok := relByID[rid]
	if !ok {
		return []result.Diagnostic{diag.Error(
			"PPTX_MISSING_SLIDE_RELATIONSHIP",
			fmt.Sprintf("slide %d %s references missing relationship %s", slide.SlideNumber, description, rid),
		)}
	}
	for _, allowed := range allowedTypes {
		if rel.Type == allowed {
			return nil
		}
	}
	return []result.Diagnostic{diag.Error(
		"PPTX_WRONG_SLIDE_RELATIONSHIP_TYPE",
		fmt.Sprintf("slide %d %s relationship %s has type %s", slide.SlideNumber, description, rid, rel.Type),
	)}
}

func validatePPTXAnimationGraph(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	report, err := inspect.ReadAnimations(session)
	if err != nil {
		return nil, err
	}
	for _, slide := range report.Slides {
		for _, effect := range slide.Effects {
			if !effect.Stale {
				continue
			}
			diags = append(diags, diag.Warning(
				"PPTX_STALE_ANIMATION_TARGET",
				fmt.Sprintf("slide %d animation effect %d targets stale shape %d: %s", slide.Slide, effect.EffectID, effect.Spid, effect.StaleReason),
			))
		}
		for _, build := range slide.Builds {
			if !build.Stale {
				continue
			}
			diags = append(diags, diag.Warning(
				"PPTX_STALE_ANIMATION_BUILD_TARGET",
				fmt.Sprintf("slide %d animation build targets stale shape %d: %s", slide.Slide, build.Spid, build.StaleReason),
			))
		}
		for _, media := range slide.Media {
			if !media.Stale {
				continue
			}
			diags = append(diags, diag.Warning(
				"PPTX_STALE_MEDIA_REFERENCE",
				fmt.Sprintf("slide %d media shape %d has stale media reference: %s", slide.Slide, media.Spid, media.StaleReason),
			))
		}
	}
	return diags, nil
}

func relationshipMap(rels []opc.RelationshipInfo) map[string]opc.RelationshipInfo {
	byID := make(map[string]opc.RelationshipInfo, len(rels))
	for _, rel := range rels {
		byID[rel.ID] = rel
	}
	return byID
}

func relationshipIDAttr(elem *etree.Element, local string) string {
	if elem == nil {
		return ""
	}
	if value := elem.SelectAttrValue("r:"+local, ""); value != "" {
		return value
	}
	for _, attr := range elem.Attr {
		if attr.Key == local && (attr.Space == "r" || attr.Space == namespaces.NsR) {
			return attr.Value
		}
	}
	return ""
}

// validateSlideShapeStructure validates shape IDs, text bodies, and placeholders within a slide.
// M12-1 Extensions: Duplicate shape IDs, orphaned media, placeholder sanity, text body structure
func validateSlideShapeStructure(session opc.PackageSession, slide *inspect.SlideRef) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	doc, err := session.ReadXMLPart(slide.PartURI)
	if err != nil || doc == nil || doc.Root() == nil {
		return diags, fmt.Errorf("failed to read slide XML: %w", err)
	}

	// Find the shape tree (p:cSld/p:spTree)
	spTree := doc.Root().FindElement("{" + namespaces.NsP + "}cSld/{" + namespaces.NsP + "}spTree")
	if spTree == nil {
		spTree = doc.Root().FindElement("cSld/spTree")
	}
	if spTree == nil {
		return diags, nil
	}

	// M12-1: Check for duplicate shape IDs across all non-visual drawing props.
	diags = append(diags, validateShapeTreeCNvPrIDs(spTree, slide.SlideNumber)...)

	// M12-1: Validate text body structure for shapes with text
	for _, sp := range spTree.FindElements("{" + namespaces.NsP + "}sp") {
		txBody := sp.FindElement("{" + namespaces.NsP + "}txBody")
		if txBody == nil {
			txBody = sp.FindElement("txBody")
		}
		if txBody != nil {
			if textBodyDiags := validateTextBodyStructure(txBody, slide.SlideNumber, getShapeName(sp)); textBodyDiags != nil {
				diags = append(diags, textBodyDiags...)
			}
		}
	}

	// M12-1: Validate placeholder structure
	for _, sp := range spTree.FindElements("{" + namespaces.NsP + "}sp") {
		if isPlaceholder(sp) {
			if phDiags := validatePlaceholderStructure(sp, slide.SlideNumber); phDiags != nil {
				diags = append(diags, phDiags...)
			}
		}
	}

	return diags, nil
}

func validateShapeTreeCNvPrIDs(spTree *etree.Element, slideNumber int) []result.Diagnostic {
	shapeIDMap := make(map[int]string)
	var diags []result.Diagnostic
	for _, cNvPr := range namespaces.FindDescendants(spTree, namespaces.NsP, "cNvPr") {
		rawID := strings.TrimSpace(cNvPr.SelectAttrValue("id", ""))
		if rawID == "" {
			continue
		}
		shapeID, err := strconv.Atoi(rawID)
		if err != nil || shapeID < 0 {
			diags = append(diags, diag.Error(
				"PPTX_SHAPE_ID",
				fmt.Sprintf("slide %d: invalid shape ID %q", slideNumber, rawID),
			))
			continue
		}
		shapeName := strings.TrimSpace(cNvPr.SelectAttrValue("name", ""))
		if existing, exists := shapeIDMap[shapeID]; exists {
			diags = append(diags, diag.Error(
				"PPTX_DUPLICATE_SHAPE_ID",
				fmt.Sprintf("slide %d: duplicate shape ID %d (names: %q, %q)", slideNumber, shapeID, existing, shapeName),
			))
		} else {
			shapeIDMap[shapeID] = shapeName
		}
	}
	return diags
}

// validateTextBodyStructure checks that text bodies have proper structure.
// Should have at least one paragraph.
func validateTextBodyStructure(txBody *etree.Element, slideNum int, shapeName string) []result.Diagnostic {
	var diags []result.Diagnostic

	if txBody == nil {
		return diags
	}

	// Count paragraphs
	paragraphs := txBody.FindElements("{" + namespaces.NsA + "}p")
	if len(paragraphs) == 0 {
		paragraphs = txBody.FindElements("p")
	}

	if len(paragraphs) == 0 {
		diags = append(diags, diag.Warning(
			"PPTX_TEXT_BODY_EMPTY",
			fmt.Sprintf("slide %d shape %q: text body has no paragraphs", slideNum, shapeName),
		))
	}

	// Validate each paragraph has proper structure
	for _, para := range paragraphs {
		// Check for proper paragraph content (should have at least one run or break)
		runs := para.FindElements("{" + namespaces.NsA + "}r")
		if len(runs) == 0 {
			runs = para.FindElements("r")
		}
		breaks := para.FindElements("{" + namespaces.NsA + "}br")
		if len(breaks) == 0 {
			breaks = para.FindElements("br")
		}

		if len(runs) == 0 && len(breaks) == 0 {
			// Empty paragraph is allowed in PPTX (represents empty line)
			// So we only warn, don't error
		}
	}

	return diags
}

// validatePlaceholderStructure checks that placeholders have proper structure.
func validatePlaceholderStructure(sp *etree.Element, slideNum int) []result.Diagnostic {
	var diags []result.Diagnostic

	shapeName := getShapeName(sp)

	// Placeholders should have text bodies (unless they're picture placeholders)
	txBody := sp.FindElement("{" + namespaces.NsP + "}txBody")
	if txBody == nil {
		txBody = sp.FindElement("txBody")
	}

	// Picture placeholders don't need text bodies
	spPr := sp.FindElement("{" + namespaces.NsP + "}spPr")
	if spPr == nil {
		spPr = sp.FindElement("spPr")
	}

	// Check if this is a picture or media placeholder
	if spPr != nil {
		// If it has a picture element (a:pic), it's a picture placeholder
		pic := spPr.FindElement("{" + namespaces.NsA + "}pic")
		if pic != nil {
			// Picture placeholders don't need text bodies
			return diags
		}
	}

	// Check if placeholder type is picture
	nvSpPr := sp.FindElement("{" + namespaces.NsP + "}nvSpPr")
	if nvSpPr == nil {
		nvSpPr = sp.FindElement("nvSpPr")
	}
	if nvSpPr != nil {
		nvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}nvPr")
		if nvPr == nil {
			nvPr = nvSpPr.FindElement("nvPr")
		}
		if nvPr != nil {
			ph := nvPr.FindElement("{" + namespaces.NsP + "}ph")
			if ph == nil {
				ph = nvPr.FindElement("ph")
			}
			if ph != nil {
				phType := ph.SelectAttrValue("type", "")
				// Picture, media, and table placeholders don't need text bodies
				if phType == "pic" || phType == "tbl" || phType == "media" {
					return diags
				}
			}
		}
	}

	// Non-picture placeholders should have text bodies
	if txBody == nil {
		diags = append(diags, diag.Warning(
			"PPTX_PLACEHOLDER_NO_TEXT_BODY",
			fmt.Sprintf("slide %d placeholder %q: placeholder has no text body", slideNum, shapeName),
		))
	}

	return diags
}

// getShapeID extracts the shape ID from a shape element's cNvPr element.
func getShapeID(sp *etree.Element) int {
	nvSpPr := sp.FindElement("{" + namespaces.NsP + "}nvSpPr")
	if nvSpPr == nil {
		nvSpPr = sp.FindElement("nvSpPr")
	}
	if nvSpPr == nil {
		// For pictures: nvPicPr, for graphic frames: nvGraphicFramePr, for groups: nvGrpSpPr
		nvSpPr = sp.FindElement("{" + namespaces.NsP + "}nvPicPr")
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("nvPicPr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("{" + namespaces.NsP + "}nvGraphicFramePr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("nvGraphicFramePr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("{" + namespaces.NsP + "}nvGrpSpPr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("nvGrpSpPr")
		}
	}

	if nvSpPr != nil {
		cNvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}cNvPr")
		if cNvPr == nil {
			cNvPr = nvSpPr.FindElement("cNvPr")
		}
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if id, err := parseID(idStr); err == nil {
				return id
			}
		}
	}
	return 0
}

// getShapeName extracts the shape name from a shape element's cNvPr element.
func getShapeName(sp *etree.Element) string {
	nvSpPr := sp.FindElement("{" + namespaces.NsP + "}nvSpPr")
	if nvSpPr == nil {
		nvSpPr = sp.FindElement("nvSpPr")
	}
	if nvSpPr == nil {
		nvSpPr = sp.FindElement("{" + namespaces.NsP + "}nvPicPr")
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("nvPicPr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("{" + namespaces.NsP + "}nvGraphicFramePr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("nvGraphicFramePr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("{" + namespaces.NsP + "}nvGrpSpPr")
		}
		if nvSpPr == nil {
			nvSpPr = sp.FindElement("nvGrpSpPr")
		}
	}

	if nvSpPr != nil {
		cNvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}cNvPr")
		if cNvPr == nil {
			cNvPr = nvSpPr.FindElement("cNvPr")
		}
		if cNvPr != nil {
			return cNvPr.SelectAttrValue("name", "")
		}
	}
	return ""
}

// isPlaceholder checks if a shape element is a placeholder.
func isPlaceholder(sp *etree.Element) bool {
	nvSpPr := sp.FindElement("{" + namespaces.NsP + "}nvSpPr")
	if nvSpPr == nil {
		nvSpPr = sp.FindElement("nvSpPr")
	}
	if nvSpPr != nil {
		nvPr := nvSpPr.FindElement("{" + namespaces.NsP + "}nvPr")
		if nvPr == nil {
			nvPr = nvSpPr.FindElement("nvPr")
		}
		if nvPr != nil {
			ph := nvPr.FindElement("{" + namespaces.NsP + "}ph")
			if ph == nil {
				ph = nvPr.FindElement("ph")
			}
			return ph != nil
		}
	}
	return false
}

// parseID parses an integer ID from a string.
func parseID(s string) (int, error) {
	if s == "" {
		return 0, fmt.Errorf("empty ID")
	}
	var id int
	_, err := fmt.Sscanf(s, "%d", &id)
	return id, err
}
