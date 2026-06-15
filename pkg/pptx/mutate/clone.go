package mutate

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

const (
	slideContentType         = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
	relationshipsContentType = "application/vnd.openxmlformats-package.relationships+xml"
)

// CloneSlideRequest describes a package-level slide clone operation.
type CloneSlideRequest struct {
	Package     opc.PackageSession
	SlideNumber int
	InsertAfter int
	NotesPolicy NotesClonePolicy
}

// CloneSlideResult describes the newly inserted slide.
type CloneSlideResult struct {
	NewSlideURI    string
	NewSlideID     uint32
	NewSlideNumber int
	NotesURI       string
}

// CloneSlide duplicates an existing slide inside the same package.
func CloneSlide(req *CloneSlideRequest) (*CloneSlideResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("clone-slide requires an open package session")
	}
	if req.SlideNumber < 1 {
		return nil, fmt.Errorf("slide number must be >= 1")
	}

	graph, err := inspect.ParsePresentation(req.Package)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}
	if req.SlideNumber > len(graph.Slides) {
		return nil, fmt.Errorf("slide %d not found", req.SlideNumber)
	}

	insertAfter := req.InsertAfter
	if insertAfter == 0 {
		insertAfter = req.SlideNumber
	}
	if insertAfter < 1 || insertAfter > len(graph.Slides) {
		return nil, fmt.Errorf("insert-after %d out of range", insertAfter)
	}
	notesPolicy := req.NotesPolicy
	if notesPolicy == "" {
		notesPolicy = NotesClone
	}

	sourceSlide := graph.Slides[req.SlideNumber-1]
	newSlideURI, err := AllocateSlidePartName(req.Package)
	if err != nil {
		return nil, err
	}

	slideDoc, err := req.Package.ReadXMLPart(sourceSlide.PartURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source slide: %w", err)
	}
	slideXML, err := writeXML(slideDoc)
	if err != nil {
		return nil, fmt.Errorf("failed to serialize slide XML: %w", err)
	}
	if err := req.Package.AddPart(newSlideURI, slideXML, contentTypeOrDefault(req.Package, sourceSlide.PartURI, slideContentType), copyZipMeta(req.Package.GetZipMeta(sourceSlide.PartURI))); err != nil {
		return nil, fmt.Errorf("failed to add cloned slide part: %w", err)
	}

	sourceRels := req.Package.ListRelationships(sourceSlide.PartURI)
	newSlideRels := make([]opc.RelationshipInfo, 0, len(sourceRels))
	result := &CloneSlideResult{NewSlideURI: newSlideURI}
	for _, rel := range sourceRels {
		if IsNoteSlideRelationship(rel) {
			notesResult, err := CloneNotes(&CloneNotesRequest{
				Session:                  req.Package,
				SourceNotesURI:           sourceSlide.NotesPartURI,
				DestinationSlideURI:      newSlideURI,
				DestinationRelationships: newSlideRels,
				Policy:                   notesPolicy,
			})
			if err != nil {
				return nil, err
			}
			if notesResult != nil && notesResult.NewNotesURI != "" {
				newSlideRels = append(newSlideRels, notesResult.NotesRelationship)
				result.NotesURI = notesResult.NewNotesURI
			}
			continue
		}
		newSlideRels = append(newSlideRels, rel)
	}

	relsXML, err := BuildRelationshipsXML(newSlideRels)
	if err != nil {
		return nil, err
	}
	newSlideRelsURI := relsURIForPart(newSlideURI)
	if err := req.Package.AddPart(newSlideRelsURI, relsXML, relationshipsContentType, copyZipMeta(req.Package.GetZipMeta(relsURIForPart(sourceSlide.PartURI)))); err != nil {
		return nil, fmt.Errorf("failed to add cloned slide relationships: %w", err)
	}

	presentationDoc, err := req.Package.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read presentation.xml: %w", err)
	}
	newSlideID, err := AllocateSlideID(presentationDoc)
	if err != nil {
		return nil, err
	}
	result.NewSlideID = newSlideID

	inserted, err := InsertSlideReference(&InsertSlideReferenceRequest{
		PresentationDoc:           presentationDoc,
		PresentationRelationships: req.Package.ListRelationships("/ppt/presentation.xml"),
		SlidePartURI:              newSlideURI,
		SlideID:                   newSlideID,
		Position:                  insertAfter,
	})
	if err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		return nil, fmt.Errorf("failed to update presentation.xml: %w", err)
	}

	presentationRels := req.Package.ListRelationships("/ppt/presentation.xml")
	presentationRels = append(presentationRels, inserted.Relationship)
	presentationRelsXML, err := BuildRelationshipsXML(presentationRels)
	if err != nil {
		return nil, err
	}
	if err := req.Package.ReplaceRawPart("/ppt/_rels/presentation.xml.rels", presentationRelsXML, relationshipsContentType); err != nil {
		return nil, fmt.Errorf("failed to update presentation relationships: %w", err)
	}

	result.NewSlideNumber = insertAfter + 1
	return result, nil
}

func relsURIForPart(partURI string) string {
	return opc.GetDirectory(partURI) + "/_rels/" + opc.GetFileName(partURI) + ".rels"
}

func writeXML(doc interface{ WriteToBytes() ([]byte, error) }) ([]byte, error) {
	return doc.WriteToBytes()
}

func contentTypeOrDefault(session opc.PackageSession, uri string, fallback string) string {
	if session == nil {
		return fallback
	}
	if contentType := session.GetContentType(uri); contentType != "" {
		return contentType
	}
	return fallback
}

func copyZipMeta(meta *opc.ZipEntryMeta) *opc.ZipEntryMeta {
	if meta == nil {
		return nil
	}
	copied := *meta
	return &copied
}
