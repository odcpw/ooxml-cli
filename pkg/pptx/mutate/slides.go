package mutate

import (
	"bytes"
	"fmt"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

const (
	slideLayoutRelationshipType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"
	slideMasterRelationshipType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster"
	themeRelationshipType       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme"
)

// ImportSlideRequest describes a cross-deck slide import operation.
type ImportSlideRequest struct {
	// TargetPackage is the destination presentation package (will be modified)
	TargetPackage opc.PackageSession
	// SourcePackage is the source presentation package (will not be modified)
	SourcePackage opc.PackageSession
	// SourceSlideNumber is the 1-based slide number in the source presentation
	SourceSlideNumber int
	// InsertAfter is the 1-based position in the target presentation after which to insert
	InsertAfter int
	// LayoutPolicy determines how layouts should be handled: "reuse" or "import"
	LayoutPolicy string
	// ThemePolicy determines how themes should be handled: "reuse" or "import"
	ThemePolicy string
	// NotesPolicy determines how notes should be handled
	NotesPolicy NotesClonePolicy
}

// ImportSlideResult describes the newly imported slide in the target package.
type ImportSlideResult struct {
	NewSlideURI    string
	NewSlideID     uint32
	NewSlideNumber int
	NotesURI       string
}

// ImportSlide imports a slide from a source presentation into a target presentation.
// It handles media copying, layout/master/theme reconciliation, and relationship remapping.
func ImportSlide(req *ImportSlideRequest) (*ImportSlideResult, error) {
	if req == nil || req.TargetPackage == nil || req.SourcePackage == nil {
		return nil, fmt.Errorf("import-slide requires open source and target packages")
	}
	if req.SourceSlideNumber < 1 {
		return nil, fmt.Errorf("source slide number must be >= 1")
	}

	// Parse both presentations
	sourceGraph, err := inspect.ParsePresentation(req.SourcePackage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse source presentation: %w", err)
	}
	if req.SourceSlideNumber > len(sourceGraph.Slides) {
		return nil, fmt.Errorf("source slide %d not found", req.SourceSlideNumber)
	}

	targetGraph, err := inspect.ParsePresentation(req.TargetPackage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse target presentation: %w", err)
	}

	insertAfter := req.InsertAfter
	if insertAfter == 0 {
		insertAfter = len(targetGraph.Slides)
	}
	if insertAfter < 0 || insertAfter > len(targetGraph.Slides) {
		return nil, fmt.Errorf("insert-after %d out of range for target with %d slides", insertAfter, len(targetGraph.Slides))
	}

	layoutPolicy := req.LayoutPolicy
	if layoutPolicy == "" {
		layoutPolicy = "reuse"
	}
	if layoutPolicy != "reuse" && layoutPolicy != "import" {
		return nil, fmt.Errorf("unknown layout policy: %s", layoutPolicy)
	}

	themePolicy := req.ThemePolicy
	if themePolicy == "" {
		themePolicy = "reuse"
	}
	if themePolicy != "reuse" && themePolicy != "import" {
		return nil, fmt.Errorf("unknown theme policy: %s", themePolicy)
	}

	notesPolicy := req.NotesPolicy
	if notesPolicy == "" {
		notesPolicy = NotesClone
	}

	sourceSlide := sourceGraph.Slides[req.SourceSlideNumber-1]

	// Allocate a new slide URI in the target package
	newSlideURI, err := AllocateSlidePartName(req.TargetPackage)
	if err != nil {
		return nil, err
	}

	// Copy the slide content
	sourceSlideDoc, err := req.SourcePackage.ReadXMLPart(sourceSlide.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source slide: %w", err)
	}

	slideXML, err := writeXML(sourceSlideDoc)
	if err != nil {
		return nil, fmt.Errorf("failed to serialize imported slide XML: %w", err)
	}
	if err := req.TargetPackage.AddPart(newSlideURI, slideXML, slideContentType, copyZipMeta(req.SourcePackage.GetZipMeta(sourceSlide.PartURI))); err != nil {
		return nil, fmt.Errorf("failed to add imported slide part: %w", err)
	}

	// Get source slide relationships.
	sourceSlideRels := req.SourcePackage.ListRelationships(sourceSlide.PartURI)
	newSlideRels := make([]opc.RelationshipInfo, 0, len(sourceSlideRels))
	importedNotesURI := ""
	importCtx := &partImportContext{
		SourcePackage: req.SourcePackage,
		TargetPackage: req.TargetPackage,
		Imported:      map[string]string{},
	}

	for _, rel := range sourceSlideRels {
		if IsNoteSlideRelationship(rel) {
			notesResult, err := importNotes(&importNotesRequest{
				SourcePackage:            req.SourcePackage,
				TargetPackage:            req.TargetPackage,
				SourceNotesURI:           sourceSlide.NotesPartURI,
				DestinationSlideURI:      newSlideURI,
				DestinationRelationships: newSlideRels,
				Policy:                   notesPolicy,
				ImportContext:            importCtx,
			})
			if err != nil {
				return nil, err
			}
			if notesResult != nil && notesResult.NewNotesURI != "" {
				importedNotesURI = notesResult.NewNotesURI
				newSlideRels = append(newSlideRels, notesResult.NotesRelationship)
			}
			continue
		}

		newRelID := AllocateRelationshipID(newSlideRels)
		if rel.TargetMode == "External" {
			newSlideRels = append(newSlideRels, opc.RelationshipInfo{
				SourceURI:  newSlideURI,
				ID:         newRelID,
				Type:       rel.Type,
				Target:     rel.Target,
				TargetMode: rel.TargetMode,
			})
			continue
		}

		sourceTargetURI := opc.ResolveRelationshipTarget(sourceSlide.PartURI, rel.Target)
		var targetURI string
		switch rel.Type {
		case slideLayoutRelationshipType:
			targetURI, err = resolveSlideLayoutTarget(req.SourcePackage, req.TargetPackage, sourceGraph, targetGraph, sourceSlide.LayoutPartURI, layoutPolicy, themePolicy)
			if err != nil {
				return nil, err
			}
		case slideMasterRelationshipType, themeRelationshipType:
			return nil, fmt.Errorf("direct slide %s relationships are not supported; import the layout chain instead", rel.Type)
		default:
			if IsMediaRelationship(rel) {
				targetURI, err = copyMedia(req.SourcePackage, req.TargetPackage, sourceTargetURI)
			} else {
				targetURI, err = importCtx.copyDependencyTree(sourceTargetURI)
			}
			if err != nil {
				return nil, fmt.Errorf("failed to copy dependency %s: %w", rel.Target, err)
			}
		}

		newTarget, err := relationshipTarget(newSlideURI, targetURI)
		if err != nil {
			return nil, err
		}
		newSlideRels = append(newSlideRels, opc.RelationshipInfo{
			SourceURI: newSlideURI,
			ID:        newRelID,
			Type:      rel.Type,
			Target:    newTarget,
		})
	}

	// Write slide relationships
	relsXML, err := BuildRelationshipsXML(newSlideRels)
	if err != nil {
		return nil, err
	}
	newSlideRelsURI := relsURIForPart(newSlideURI)
	if err := req.TargetPackage.AddPart(newSlideRelsURI, relsXML, relationshipsContentType, nil); err != nil {
		return nil, fmt.Errorf("failed to add imported slide relationships: %w", err)
	}

	// Update target presentation.xml
	presentationDoc, err := req.TargetPackage.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read target presentation.xml: %w", err)
	}

	newSlideID, err := AllocateSlideID(presentationDoc)
	if err != nil {
		return nil, err
	}

	inserted, err := InsertSlideReference(&InsertSlideReferenceRequest{
		PresentationDoc:           presentationDoc,
		PresentationRelationships: req.TargetPackage.ListRelationships("/ppt/presentation.xml"),
		SlidePartURI:              newSlideURI,
		SlideID:                   newSlideID,
		Position:                  insertAfter,
	})
	if err != nil {
		return nil, err
	}

	if err := req.TargetPackage.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		return nil, fmt.Errorf("failed to update target presentation.xml: %w", err)
	}

	// Update target presentation relationships
	presentationRels := req.TargetPackage.ListRelationships("/ppt/presentation.xml")
	presentationRels = append(presentationRels, inserted.Relationship)
	presentationRelsXML, err := BuildRelationshipsXML(presentationRels)
	if err != nil {
		return nil, err
	}
	if err := req.TargetPackage.ReplaceRawPart("/ppt/_rels/presentation.xml.rels", presentationRelsXML, relationshipsContentType); err != nil {
		return nil, fmt.Errorf("failed to update target presentation relationships: %w", err)
	}

	result := &ImportSlideResult{
		NewSlideURI:    newSlideURI,
		NewSlideID:     newSlideID,
		NewSlideNumber: insertAfter + 1,
		NotesURI:       importedNotesURI,
	}

	return result, nil
}

// copyMedia copies a media file from source to target package, returning the new URI.
// If the same media already exists in target (exact bytes + content type), reuse it.
func copyMedia(sourcePackage, targetPackage opc.PackageSession, sourceMediaURI string) (string, error) {
	if sourcePackage == nil || targetPackage == nil {
		return "", fmt.Errorf("packages cannot be nil")
	}

	sourceData, err := sourcePackage.ReadRawPart(sourceMediaURI)
	if err != nil {
		return "", fmt.Errorf("failed to read source media: %w", err)
	}
	contentType := sourcePackage.GetContentType(sourceMediaURI)

	for _, part := range targetPackage.ListParts() {
		if !strings.HasPrefix(part.URI, "/ppt/media/") {
			continue
		}
		if targetPackage.GetContentType(part.URI) != contentType {
			continue
		}
		targetData, err := targetPackage.ReadRawPart(part.URI)
		if err != nil {
			continue
		}
		if bytes.Equal(sourceData, targetData) {
			return part.URI, nil
		}
	}

	newMediaURI, err := allocateMediaPartNameLikeSource(targetPackage, sourceMediaURI)
	if err != nil {
		return "", err
	}

	if err := targetPackage.AddPart(newMediaURI, sourceData, contentType, copyZipMeta(sourcePackage.GetZipMeta(sourceMediaURI))); err != nil {
		return "", fmt.Errorf("failed to add media part: %w", err)
	}

	return newMediaURI, nil
}

// Helper type for notes import
type importNotesRequest struct {
	SourcePackage            opc.PackageSession
	TargetPackage            opc.PackageSession
	SourceNotesURI           string
	DestinationSlideURI      string
	DestinationRelationships []opc.RelationshipInfo
	Policy                   NotesClonePolicy
	ImportContext            *partImportContext
}

type importNotesResult struct {
	NewNotesURI       string
	NotesRelationship opc.RelationshipInfo
}

// importNotes copies notes from source to target package
func importNotes(req *importNotesRequest) (*importNotesResult, error) {
	if req == nil || req.Policy == NotesDrop || req.SourceNotesURI == "" {
		return &importNotesResult{}, nil
	}
	if req.Policy != NotesClone {
		return nil, fmt.Errorf("unknown notes policy: %s", req.Policy)
	}

	sourceDoc, err := req.SourcePackage.ReadXMLPart(req.SourceNotesURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source notes: %w", err)
	}

	newNotesURI, err := allocateNumberedPartName(req.TargetPackage, notesPartNamePattern, "/ppt/notesSlides/notesSlide%d.xml")
	if err != nil {
		return nil, err
	}

	data, err := sourceDoc.WriteToBytes()
	if err != nil {
		return nil, fmt.Errorf("failed to serialize notes: %w", err)
	}

	if err := req.TargetPackage.AddPart(newNotesURI, data, notesContentType, nil); err != nil {
		return nil, fmt.Errorf("failed to add notes part: %w", err)
	}

	// Copy notes relationships
	sourceNotesRels := req.SourcePackage.ListRelationships(req.SourceNotesURI)
	if len(sourceNotesRels) > 0 {
		importCtx := req.ImportContext
		if importCtx == nil {
			importCtx = &partImportContext{
				SourcePackage: req.SourcePackage,
				TargetPackage: req.TargetPackage,
				Imported:      map[string]string{},
			}
		}
		newNotesRels := make([]opc.RelationshipInfo, 0, len(sourceNotesRels))
		for _, rel := range sourceNotesRels {
			if rel.TargetMode == "External" {
				newNotesRels = append(newNotesRels, opc.RelationshipInfo{
					SourceURI:  newNotesURI,
					ID:         rel.ID,
					Type:       rel.Type,
					Target:     rel.Target,
					TargetMode: rel.TargetMode,
				})
				continue
			}

			var targetURI string
			if rel.Type == slideRelationshipType {
				targetURI = req.DestinationSlideURI
			} else {
				sourceTargetURI := opc.ResolveRelationshipTarget(req.SourceNotesURI, rel.Target)
				copied, err := importCtx.copyDependencyTree(sourceTargetURI)
				if err != nil {
					return nil, fmt.Errorf("failed to copy notes dependency %s: %w", rel.Target, err)
				}
				targetURI = copied
			}
			newTarget, err := relationshipTarget(newNotesURI, targetURI)
			if err != nil {
				return nil, err
			}
			newNotesRels = append(newNotesRels, opc.RelationshipInfo{
				SourceURI: newNotesURI,
				ID:        rel.ID,
				Type:      rel.Type,
				Target:    newTarget,
			})
		}

		relsXML, err := BuildRelationshipsXML(newNotesRels)
		if err != nil {
			return nil, fmt.Errorf("failed to build notes relationships XML: %w", err)
		}
		notesRelsURI := opc.GetDirectory(newNotesURI) + "/_rels/" + opc.GetFileName(newNotesURI) + ".rels"
		if err := req.TargetPackage.AddPart(notesRelsURI, relsXML, relationshipsContentType, nil); err != nil {
			return nil, fmt.Errorf("failed to add notes relationships: %w", err)
		}
	}

	relTarget, err := relationshipTarget(req.DestinationSlideURI, newNotesURI)
	if err != nil {
		return nil, err
	}

	newRelID := AllocateRelationshipID(req.DestinationRelationships)

	return &importNotesResult{
		NewNotesURI: newNotesURI,
		NotesRelationship: opc.RelationshipInfo{
			SourceURI: req.DestinationSlideURI,
			ID:        newRelID,
			Type:      notesRelationshipType,
			Target:    relTarget,
		},
	}, nil
}

// Helper type for layout/master import
type importLayoutOrMasterRequest struct {
	SourcePackage          opc.PackageSession
	TargetPackage          opc.PackageSession
	NewSlideURI            string
	SourceSlideLayoutURI   string
	SourceRel              opc.RelationshipInfo
	LayoutPolicy           string
	ThemePolicy            string
	SourceMasters          []inspect.MasterRef
	SourceLayouts          []inspect.LayoutRef
	TargetMasters          []inspect.MasterRef
	TargetLayouts          []inspect.LayoutRef
	SourcePresentationRels []opc.RelationshipInfo
	TargetPresentationRels []opc.RelationshipInfo
}

type importLayoutOrMasterResult struct {
	NewRelationship opc.RelationshipInfo
}

// importLayoutOrMaster handles layout, master, and theme relationships
func importLayoutOrMaster(req *importLayoutOrMasterRequest) (*importLayoutOrMasterResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}

	// Resolve the target URI from the relationship
	sourceTargetURI := opc.ResolveRelationshipTarget(req.SourceSlideLayoutURI, req.SourceRel.Target)

	// For now, implement a simple reuse-based strategy:
	// - Try to find an equivalent layout/master/theme in the target
	// - If not found, import it
	// - In "reuse" mode, don't import duplicates; in "import" mode, import everything

	switch {
	case strings.Contains(req.SourceRel.Type, "slideLayout"):
		return importLayout(req, sourceTargetURI)
	case strings.Contains(req.SourceRel.Type, "slideMaster"):
		return importMaster(req, sourceTargetURI)
	case strings.Contains(req.SourceRel.Type, "theme"):
		return importTheme(req, sourceTargetURI)
	default:
		// For other relationships, keep as-is with new slide URI
		return &importLayoutOrMasterResult{
			NewRelationship: opc.RelationshipInfo{
				SourceURI:  req.NewSlideURI,
				ID:         AllocateRelationshipID([]opc.RelationshipInfo{}),
				Type:       req.SourceRel.Type,
				Target:     req.SourceRel.Target,
				TargetMode: req.SourceRel.TargetMode,
			},
		}, nil
	}
}

// importLayout handles slide layout relationships
func importLayout(req *importLayoutOrMasterRequest, sourceLayoutURI string) (*importLayoutOrMasterResult, error) {
	if req.LayoutPolicy == "reuse" {
		// In reuse mode, look for an existing layout
		// For now, use the first available layout in the target
		if len(req.TargetLayouts) > 0 {
			targetLayout := req.TargetLayouts[0]
			newTarget, err := relationshipTarget(req.NewSlideURI, targetLayout.PartURI)
			if err != nil {
				return nil, err
			}
			return &importLayoutOrMasterResult{
				NewRelationship: opc.RelationshipInfo{
					SourceURI: req.NewSlideURI,
					ID:        AllocateRelationshipID([]opc.RelationshipInfo{}),
					Type:      req.SourceRel.Type,
					Target:    newTarget,
				},
			}, nil
		}
	}

	// In import mode or if no target layout exists, copy the layout
	return copyLayoutAndMaster(req, sourceLayoutURI)
}

// importMaster handles slide master relationships
func importMaster(req *importLayoutOrMasterRequest, sourceMasterURI string) (*importLayoutOrMasterResult, error) {
	if req.LayoutPolicy == "reuse" {
		// In reuse mode, look for an existing master
		if len(req.TargetMasters) > 0 {
			targetMaster := req.TargetMasters[0]
			newTarget, err := relationshipTarget(req.NewSlideURI, targetMaster.PartURI)
			if err != nil {
				return nil, err
			}
			return &importLayoutOrMasterResult{
				NewRelationship: opc.RelationshipInfo{
					SourceURI: req.NewSlideURI,
					ID:        AllocateRelationshipID([]opc.RelationshipInfo{}),
					Type:      req.SourceRel.Type,
					Target:    newTarget,
				},
			}, nil
		}
	}

	// In import mode or if no target master exists, copy the master
	return copyLayoutAndMaster(req, sourceMasterURI)
}

// importTheme handles theme relationships
func importTheme(req *importLayoutOrMasterRequest, sourceThemeURI string) (*importLayoutOrMasterResult, error) {
	if req.ThemePolicy == "reuse" {
		// In reuse mode, look for an existing theme
		if len(req.TargetMasters) > 0 && req.TargetMasters[0].ThemeURI != "" {
			newTarget, err := relationshipTarget(req.NewSlideURI, req.TargetMasters[0].ThemeURI)
			if err != nil {
				return nil, err
			}
			return &importLayoutOrMasterResult{
				NewRelationship: opc.RelationshipInfo{
					SourceURI: req.NewSlideURI,
					ID:        AllocateRelationshipID([]opc.RelationshipInfo{}),
					Type:      req.SourceRel.Type,
					Target:    newTarget,
				},
			}, nil
		}
	}

	// In import mode or if no target theme exists, copy the theme
	return copyLayoutAndMaster(req, sourceThemeURI)
}

// copyLayoutAndMaster copies a layout, master, or theme from source to target
func copyLayoutAndMaster(req *importLayoutOrMasterRequest, sourceURI string) (*importLayoutOrMasterResult, error) {
	sourceDoc, err := req.SourcePackage.ReadXMLPart(sourceURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source layout/master/theme: %w", err)
	}

	// Allocate new part name based on type
	var newURI string
	var contentType string
	var err2 error

	if strings.Contains(req.SourceRel.Type, "slideLayout") {
		newURI, err2 = allocateNumberedPartName(req.TargetPackage, layoutPartNamePattern, "/ppt/slideLayouts/slideLayout%d.xml")
		contentType = "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"
	} else if strings.Contains(req.SourceRel.Type, "slideMaster") {
		newURI, err2 = allocateNumberedPartName(req.TargetPackage, masterPartNamePattern, "/ppt/slideMasters/slideMaster%d.xml")
		contentType = "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"
	} else if strings.Contains(req.SourceRel.Type, "theme") {
		newURI, err2 = allocateNumberedPartName(req.TargetPackage, themePartNamePattern, "/ppt/theme/theme%d.xml")
		contentType = "application/vnd.openxmlformats-officedocument.presentationml.theme+xml"
	} else {
		return nil, fmt.Errorf("unknown layout/master/theme type: %s", req.SourceRel.Type)
	}

	if err2 != nil {
		return nil, err2
	}

	sourceXML, err := writeXML(sourceDoc)
	if err != nil {
		return nil, fmt.Errorf("failed to serialize layout/master/theme XML: %w", err)
	}
	if err := req.TargetPackage.AddPart(newURI, sourceXML, contentType, copyZipMeta(req.SourcePackage.GetZipMeta(sourceURI))); err != nil {
		return nil, fmt.Errorf("failed to add layout/master/theme part: %w", err)
	}

	// Copy relationships if present
	sourceRels := req.SourcePackage.ListRelationships(sourceURI)
	if len(sourceRels) > 0 {
		relsXML, err := BuildRelationshipsXML(sourceRels)
		if err != nil {
			return nil, fmt.Errorf("failed to build layout/master/theme relationships XML: %w", err)
		}
		newRelsURI := relsURIForPart(newURI)
		if err := req.TargetPackage.AddPart(newRelsURI, relsXML, relationshipsContentType, nil); err != nil {
			return nil, fmt.Errorf("failed to add layout/master/theme relationships: %w", err)
		}
	}

	newTarget, err := relationshipTarget(req.NewSlideURI, newURI)
	if err != nil {
		return nil, err
	}

	return &importLayoutOrMasterResult{
		NewRelationship: opc.RelationshipInfo{
			SourceURI: req.NewSlideURI,
			ID:        AllocateRelationshipID([]opc.RelationshipInfo{}),
			Type:      req.SourceRel.Type,
			Target:    newTarget,
		},
	}, nil
}

// Helper functions

func extractMediaType(uri string) string {
	if strings.Contains(uri, "/image") {
		return "image"
	}
	if strings.Contains(uri, "/video") {
		return "video"
	}
	if strings.Contains(uri, "/audio") {
		return "audio"
	}
	return "media"
}

func allocateMediaPartNameLikeSource(session opc.PackageSession, sourceURI string) (string, error) {
	if session == nil {
		return "", fmt.Errorf("session cannot be nil")
	}

	mediaType := extractMediaType(sourceURI)
	ext := strings.ToLower(filepath.Ext(sourceURI))
	if ext == "" {
		switch mediaType {
		case "image":
			ext = ".png"
		case "video":
			ext = ".mp4"
		case "audio":
			ext = ".m4a"
		default:
			ext = ".bin"
		}
	}
	if ext == ".jpeg" {
		ext = ".jpg"
	}

	var pattern *regexp.Regexp
	var format string
	switch mediaType {
	case "image":
		pattern = imagePartNamePattern
		format = "/ppt/media/image%d%s"
	case "video":
		pattern = videoPartNamePattern
		format = "/ppt/media/video%d%s"
	case "audio":
		pattern = audioPartNamePattern
		format = "/ppt/media/audio%d%s"
	default:
		pattern = mediaPartNamePattern
		format = "/ppt/media/media%d%s"
	}

	max := 0
	for _, part := range session.ListParts() {
		matches := pattern.FindStringSubmatch(part.URI)
		if matches == nil {
			continue
		}
		parsed, err := strconv.Atoi(matches[1])
		if err != nil {
			continue
		}
		if parsed > max {
			max = parsed
		}
	}

	return fmt.Sprintf(format, max+1, ext), nil
}

// Add pattern variables
var (
	layoutPartNamePattern = regexp.MustCompile(`^/ppt/slideLayouts/slideLayout(\d+)\.xml$`)
	masterPartNamePattern = regexp.MustCompile(`^/ppt/slideMasters/slideMaster(\d+)\.xml$`)
	themePartNamePattern  = regexp.MustCompile(`^/ppt/theme/theme(\d+)\.xml$`)
	imagePartNamePattern  = regexp.MustCompile(`^/ppt/media/image(\d+)\.`)
	videoPartNamePattern  = regexp.MustCompile(`^/ppt/media/video(\d+)\.`)
	audioPartNamePattern  = regexp.MustCompile(`^/ppt/media/audio(\d+)\.`)
	mediaPartNamePattern  = regexp.MustCompile(`^/ppt/media/media(\d+)\.`)
)

// DeleteSlideRequest holds parameters for deleting a slide.
type DeleteSlideRequest struct {
	Package     opc.PackageSession
	SlideNumber int // 1-based slide number
}

// DeleteSlideResult describes what was deleted.
type DeleteSlideResult struct {
	DeletedSlideURI   string
	DeletedNotesURI   string
	RemovedRelationID string
}

// DeleteSlide removes a slide from a presentation, cleaning up all references.
// This includes:
// - Removing the slide reference from presentation.xml
// - Removing the relationship from presentation.xml.rels
// - Removing the slide part and its relationships
// - Removing the notes slide if present
// - Updating content type overrides
func DeleteSlide(req *DeleteSlideRequest) (*DeleteSlideResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("delete-slide requires an open package session")
	}

	// Parse the presentation to get slide information
	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	// Validate slide number
	if req.SlideNumber < 1 || req.SlideNumber > len(graph.Slides) {
		return nil, fmt.Errorf("slide number %d out of range (presentation has %d slides)", req.SlideNumber, len(graph.Slides))
	}

	// Get the slide to delete (convert to 0-based index)
	slideToDelete := graph.Slides[req.SlideNumber-1]
	result := &DeleteSlideResult{
		DeletedSlideURI: slideToDelete.PartURI,
		DeletedNotesURI: slideToDelete.NotesPartURI,
	}

	// Read presentation.xml to find and remove the slide reference
	presentationDoc, err := req.Package.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read presentation.xml: %w", err)
	}

	// Find and remove the p:sldId entry
	root := presentationDoc.Root()
	sldIDList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst")
	if sldIDList == nil {
		return nil, fmt.Errorf("no slide list found in presentation.xml")
	}

	sldIds := namespaces.FindChildren(sldIDList, namespaces.NsP, "sldId")
	if req.SlideNumber-1 >= len(sldIds) {
		return nil, fmt.Errorf("slide reference at position %d not found", req.SlideNumber)
	}

	// Get the relationship ID from the slide we're removing
	sldIdElement := sldIds[req.SlideNumber-1]
	relID := sldIdElement.SelectAttrValue("{"+namespaces.NsR+"}id", "")
	if relID == "" {
		relID = sldIdElement.SelectAttrValue("r:id", "")
	}
	if relID == "" {
		return nil, fmt.Errorf("no relationship ID found for slide %d", req.SlideNumber)
	}
	result.RemovedRelationID = relID

	// Remove the p:sldId element
	sldIDList.RemoveChildAt(sldIdElement.Index())

	// Write presentation.xml back
	if err := req.Package.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		return nil, fmt.Errorf("failed to update presentation.xml: %w", err)
	}

	// Update presentation.xml.rels
	presentationRels := req.Package.ListRelationships("/ppt/presentation.xml")
	newPresentationRels := []opc.RelationshipInfo{}
	for _, rel := range presentationRels {
		if rel.ID != relID {
			newPresentationRels = append(newPresentationRels, rel)
		}
	}

	presentationRelsXML, err := BuildRelationshipsXML(newPresentationRels)
	if err != nil {
		return nil, fmt.Errorf("failed to build relationships XML: %w", err)
	}
	if err := req.Package.ReplaceRawPart("/ppt/_rels/presentation.xml.rels", presentationRelsXML, relationshipsContentType); err != nil {
		return nil, fmt.Errorf("failed to update presentation.xml.rels: %w", err)
	}

	// Remove the slide part
	relsURI := opc.GetDirectory(slideToDelete.PartURI) + "/_rels/" + opc.GetFileName(slideToDelete.PartURI) + ".rels"
	if err := req.Package.RemovePart(slideToDelete.PartURI); err != nil {
		return nil, fmt.Errorf("failed to remove slide part %s: %w", slideToDelete.PartURI, err)
	}

	// Remove the slide's relationships part
	if err := req.Package.RemovePart(relsURI); err != nil {
		// This is not critical, but log it
		_ = err
	}

	// Remove notes slide if present
	if slideToDelete.NotesPartURI != "" {
		notesRelsURI := opc.GetDirectory(slideToDelete.NotesPartURI) + "/_rels/" + opc.GetFileName(slideToDelete.NotesPartURI) + ".rels"

		if err := req.Package.RemovePart(slideToDelete.NotesPartURI); err != nil {
			// This is not critical
			_ = err
		}

		if err := req.Package.RemovePart(notesRelsURI); err != nil {
			// This is not critical
			_ = err
		}

		// Remove notes-related relationships from notes.xml.rels if the notes part exists
		// and update any slide relationships that reference notes
	}

	return result, nil
}

// MoveSlideRequest holds parameters for moving a slide.
type MoveSlideRequest struct {
	Package     opc.PackageSession
	SlideNumber int // Current 1-based position
	NewPosition int // Target 1-based position
}

// MoveSlideResult describes the result of a move operation.
type MoveSlideResult struct {
	SlideURI    string
	OldPosition int
	NewPosition int
}

// MoveSlide moves a slide within a presentation by reordering presentation.xml references.
// Parts are not renamed; only the slide list order changes.
func MoveSlide(req *MoveSlideRequest) (*MoveSlideResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("move-slide requires an open package session")
	}

	// Parse the presentation to get slide information
	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	// Validate slide numbers
	if req.SlideNumber < 1 || req.SlideNumber > len(graph.Slides) {
		return nil, fmt.Errorf("slide number %d out of range (presentation has %d slides)", req.SlideNumber, len(graph.Slides))
	}

	if req.NewPosition < 1 || req.NewPosition > len(graph.Slides) {
		return nil, fmt.Errorf("new position %d out of range (valid range: 1-%d)", req.NewPosition, len(graph.Slides))
	}

	// No-op check
	if req.SlideNumber == req.NewPosition {
		return &MoveSlideResult{
			SlideURI:    graph.Slides[req.SlideNumber-1].PartURI,
			OldPosition: req.SlideNumber,
			NewPosition: req.NewPosition,
		}, nil
	}

	// Read presentation.xml
	presentationDoc, err := req.Package.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read presentation.xml: %w", err)
	}

	root := presentationDoc.Root()
	sldIDList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst")
	if sldIDList == nil {
		return nil, fmt.Errorf("no slide list found in presentation.xml")
	}

	sldIds := namespaces.FindChildren(sldIDList, namespaces.NsP, "sldId")
	if req.SlideNumber-1 >= len(sldIds) {
		return nil, fmt.Errorf("slide reference at position %d not found", req.SlideNumber)
	}

	// Get the slide element at the current position
	slideElement := sldIds[req.SlideNumber-1]
	slideURI := graph.Slides[req.SlideNumber-1].PartURI

	// Make a copy of the element we want to move
	slideElementCopy := slideElement.Copy()

	// Remove from current position
	sldIDList.RemoveChild(slideElement)

	// Re-fetch the children after removal to get the correct insertion point
	sldIds = namespaces.FindChildren(sldIDList, namespaces.NsP, "sldId")

	// Convert to 0-based indices
	newIdx := req.NewPosition - 1

	// Insert at new position
	if newIdx >= len(sldIds) {
		// Append at the end
		sldIDList.AddChild(slideElementCopy)
	} else {
		// Insert before the element at newIdx
		sldIDList.InsertChildAt(sldIds[newIdx].Index(), slideElementCopy)
	}

	// Write presentation.xml back
	if err := req.Package.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		return nil, fmt.Errorf("failed to update presentation.xml: %w", err)
	}

	return &MoveSlideResult{
		SlideURI:    slideURI,
		OldPosition: req.SlideNumber,
		NewPosition: req.NewPosition,
	}, nil
}

// ReorderSlideRequest holds parameters for reordering slides with a full permutation.
type ReorderSlideRequest struct {
	Package opc.PackageSession
	Order   string // Comma-separated list like "3,1,2,4"
}

// ReorderSlideResult describes the result of a reorder operation.
type ReorderSlideResult struct {
	NewOrder []int // The resulting order of slides (1-based positions)
}

// ReorderSlides reorders slides in a presentation according to a full explicit permutation.
// The permutation must be a complete, valid reordering of all slides.
func ReorderSlides(req *ReorderSlideRequest) (*ReorderSlideResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("reorder-slides requires an open package session")
	}

	// Parse the presentation to get slide information
	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}

	// Parse the order string
	orderStrs := strings.Split(strings.TrimSpace(req.Order), ",")
	if len(orderStrs) != len(graph.Slides) {
		return nil, fmt.Errorf("permutation has %d elements but presentation has %d slides", len(orderStrs), len(graph.Slides))
	}

	newOrder := make([]int, len(orderStrs))
	seen := make(map[int]bool)

	for i, str := range orderStrs {
		str = strings.TrimSpace(str)
		pos, err := strconv.Atoi(str)
		if err != nil {
			return nil, fmt.Errorf("invalid position %q in permutation: %w", str, err)
		}

		if pos < 1 || pos > len(graph.Slides) {
			return nil, fmt.Errorf("slide position %d out of range (valid range: 1-%d)", pos, len(graph.Slides))
		}

		if seen[pos] {
			return nil, fmt.Errorf("duplicate slide position %d in permutation", pos)
		}
		seen[pos] = true
		newOrder[i] = pos
	}

	// Apply the reordering by performing moves
	// We simulate the permutation: move each slide to its target position
	currentOrder := make([]int, len(graph.Slides))
	for i := 0; i < len(currentOrder); i++ {
		currentOrder[i] = i + 1
	}

	// Apply moves to achieve the target order
	for targetIdx, targetSlide := range newOrder {
		// Find where the target slide currently is
		currentIdx := -1
		for i, slide := range currentOrder {
			if slide == targetSlide {
				currentIdx = i
				break
			}
		}

		if currentIdx != targetIdx {
			// Move the slide from currentIdx to targetIdx
			_, err := MoveSlide(&MoveSlideRequest{
				Package:     req.Package,
				SlideNumber: currentIdx + 1,
				NewPosition: targetIdx + 1,
			})
			if err != nil {
				return nil, fmt.Errorf("failed to move slide %d to position %d: %w", currentIdx+1, targetIdx+1, err)
			}

			// Update our currentOrder tracking
			slide := currentOrder[currentIdx]
			currentOrder = append(currentOrder[:currentIdx], currentOrder[currentIdx+1:]...)
			currentOrder = append(currentOrder[:targetIdx], append([]int{slide}, currentOrder[targetIdx:]...)...)
		}
	}

	return &ReorderSlideResult{
		NewOrder: newOrder,
	}, nil
}

// MergeDeckRequest describes a deck merge operation.
type MergeDeckRequest struct {
	// TargetPackage is the destination presentation package (will be modified)
	TargetPackage opc.PackageSession
	// SourcePackage is the source presentation package (will not be modified)
	SourcePackage opc.PackageSession
	// LayoutPolicy determines how layouts should be handled: "reuse" or "import"
	LayoutPolicy string
	// ThemePolicy determines how themes should be handled: "reuse" or "import"
	ThemePolicy string
	// NotesPolicy determines how notes should be handled
	NotesPolicy NotesClonePolicy
}

// MergeDeckResult describes the result of merging two decks.
type MergeDeckResult struct {
	MergedSlideCount int
	ImportedSlides   []ImportSlideResult
}

// MergeDeck merges all slides from a source presentation into a target presentation.
// It reuses the ImportSlide function to handle each slide, ensuring consistent media,
// layout, and master handling.
func MergeDeck(req *MergeDeckRequest) (*MergeDeckResult, error) {
	if req == nil || req.TargetPackage == nil || req.SourcePackage == nil {
		return nil, fmt.Errorf("merge-deck requires open source and target packages")
	}

	// Parse both presentations to get slide counts
	sourceGraph, err := inspect.ParsePresentation(req.SourcePackage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse source presentation: %w", err)
	}

	targetGraph, err := inspect.ParsePresentation(req.TargetPackage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse target presentation: %w", err)
	}

	if len(sourceGraph.Slides) == 0 {
		return &MergeDeckResult{
			MergedSlideCount: 0,
			ImportedSlides:   []ImportSlideResult{},
		}, nil
	}

	layoutPolicy := req.LayoutPolicy
	if layoutPolicy == "" {
		layoutPolicy = "reuse"
	}

	themePolicy := req.ThemePolicy
	if themePolicy == "" {
		themePolicy = "reuse"
	}

	notesPolicy := req.NotesPolicy
	if notesPolicy == "" {
		notesPolicy = NotesClone
	}

	result := &MergeDeckResult{
		ImportedSlides: make([]ImportSlideResult, 0, len(sourceGraph.Slides)),
	}

	// Import each slide from source in order
	for slideNum := 1; slideNum <= len(sourceGraph.Slides); slideNum++ {
		importResult, err := ImportSlide(&ImportSlideRequest{
			TargetPackage:     req.TargetPackage,
			SourcePackage:     req.SourcePackage,
			SourceSlideNumber: slideNum,
			InsertAfter:       len(targetGraph.Slides), // Insert after the last slide in current target
			LayoutPolicy:      layoutPolicy,
			ThemePolicy:       themePolicy,
			NotesPolicy:       notesPolicy,
		})
		if err != nil {
			return nil, fmt.Errorf("failed to import slide %d: %w", slideNum, err)
		}

		result.ImportedSlides = append(result.ImportedSlides, *importResult)

		// Re-parse target to get updated slide list for next iteration
		targetGraph, err = inspect.ParsePresentation(req.TargetPackage)
		if err != nil {
			return nil, fmt.Errorf("failed to re-parse target presentation after importing slide %d: %w", slideNum, err)
		}
	}

	result.MergedSlideCount = len(result.ImportedSlides)

	return result, nil
}
