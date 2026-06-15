package inspect

import (
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// PresentationGraph represents the complete structure of a presentation
// including all masters, layouts, and slides with their relationships.
type PresentationGraph struct {
	// SlideSize represents the dimensions of slides in EMU
	SlideSize SlideSizeInfo
	// Masters contains all slide masters in the presentation
	Masters []MasterRef
	// Layouts contains all slide layouts in the presentation
	Layouts []LayoutRef
	// Slides contains all slides in presentation order
	Slides []SlideRef
}

// SlideSizeInfo represents slide dimensions in EMU (English Metric Units)
type SlideSizeInfo struct {
	CX int64 // Width in EMU
	CY int64 // Height in EMU
}

// MasterRef represents a slide master reference
type MasterRef struct {
	PartURI          string   // e.g., "/ppt/slideMasters/slideMaster1.xml"
	LinkedLayoutURIs []string // URIs of layouts linked to this master
	ThemeURI         string   // e.g., "/ppt/theme/theme1.xml"
}

// LayoutRef represents a slide layout reference
type LayoutRef struct {
	PartURI       string // e.g., "/ppt/slideLayouts/slideLayout1.xml"
	Name          string // e.g., "Title Slide", "Title and Content"
	MasterPartURI string // e.g., "/ppt/slideMasters/slideMaster1.xml"
}

// SlideRef represents a slide reference
type SlideRef struct {
	PartURI        string // e.g., "/ppt/slides/slide1.xml"
	SlideNumber    int    // Position in presentation order (1-based)
	SlideID        uint32 // p:sldId@id from /ppt/presentation.xml
	RelationshipID string // presentation.xml relationship ID, e.g. rId2
	LayoutPartURI  string // e.g., "/ppt/slideLayouts/slideLayout1.xml"
	NotesPartURI   string // e.g., "/ppt/notesSlides/notesSlide1.xml" (empty if no notes)
}

// ParsePresentation parses a presentation and builds the master/layout/slide graph.
func ParsePresentation(session opc.PackageSession) (*PresentationGraph, error) {
	graph := &PresentationGraph{
		Masters: make([]MasterRef, 0),
		Layouts: make([]LayoutRef, 0),
		Slides:  make([]SlideRef, 0),
	}

	// Read presentation.xml
	presentationDoc, err := session.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read presentation.xml: %w", err)
	}

	// Parse slide size
	if err := parseSlideSizeFromXML(presentationDoc, graph); err != nil {
		return nil, err
	}

	// Get presentation.xml relationships
	presentationRels := session.ListRelationships("/ppt/presentation.xml")

	// Build a map of rId -> target URI for presentation.xml relationships
	relMap := make(map[string]string)
	for _, rel := range presentationRels {
		targetURI := opc.ResolveRelationshipTarget("/ppt/presentation.xml", rel.Target)
		relMap[rel.ID] = targetURI
	}

	// Parse slide masters from p:sldMasterIdLst
	if err := parseMasters(presentationDoc, relMap, session, graph); err != nil {
		return nil, err
	}

	// Parse layouts from all masters
	if err := parseLayouts(session, graph); err != nil {
		return nil, err
	}

	// Parse slides from p:sldIdLst
	if err := parseSlides(presentationDoc, relMap, session, graph); err != nil {
		return nil, err
	}

	return graph, nil
}

// parseSlideSizeFromXML extracts slide dimensions from p:sldSz element
func parseSlideSizeFromXML(doc *etree.Document, graph *PresentationGraph) error {
	root := doc.Root()
	if root == nil {
		return fmt.Errorf("presentation.xml root element not found")
	}

	sldSz := xmlx.FindChild(root, namespaces.NsP, "sldSz")
	if sldSz == nil {
		return fmt.Errorf("p:sldSz element not found in presentation.xml")
	}

	cx, err := strconv.ParseInt(sldSz.SelectAttrValue("cx", ""), 10, 64)
	if err != nil {
		return fmt.Errorf("invalid cx attribute in p:sldSz: %w", err)
	}

	cy, err := strconv.ParseInt(sldSz.SelectAttrValue("cy", ""), 10, 64)
	if err != nil {
		return fmt.Errorf("invalid cy attribute in p:sldSz: %w", err)
	}

	graph.SlideSize = SlideSizeInfo{CX: cx, CY: cy}
	return nil
}

// parseMasters extracts all slide masters from the presentation
func parseMasters(doc *etree.Document, relMap map[string]string, session opc.PackageSession, graph *PresentationGraph) error {
	root := doc.Root()
	if root == nil {
		return fmt.Errorf("presentation.xml root element not found")
	}

	// Find p:sldMasterIdLst element
	masterIdList := xmlx.FindChild(root, namespaces.NsP, "sldMasterIdLst")
	if masterIdList == nil {
		// No masters is unusual but not an error
		return nil
	}

	// Find all p:sldMasterId children
	masterIds := xmlx.FindChildren(masterIdList, namespaces.NsP, "sldMasterId")

	for _, masterId := range masterIds {
		// Get the r:id attribute (relationships namespace prefix is "r")
		rid := masterId.SelectAttrValue("r:id", "")
		if rid == "" {
			continue
		}

		// Resolve the relationship target
		masterPartURI, exists := relMap[rid]
		if !exists {
			return fmt.Errorf("relationship %s not found in presentation.xml.rels", rid)
		}

		// Get layouts for this master
		layoutURIs, themeURI, err := getMasterLayoutsAndTheme(session, masterPartURI)
		if err != nil {
			return err
		}

		master := MasterRef{
			PartURI:          masterPartURI,
			LinkedLayoutURIs: layoutURIs,
			ThemeURI:         themeURI,
		}
		graph.Masters = append(graph.Masters, master)
	}

	return nil
}

// getMasterLayoutsAndTheme retrieves the layouts and theme for a master
func getMasterLayoutsAndTheme(session opc.PackageSession, masterPartURI string) ([]string, string, error) {
	layoutURIs := make([]string, 0)
	themeURI := ""

	// Get relationships for this master
	masterRels := session.ListRelationships(masterPartURI)

	for _, rel := range masterRels {
		// Check if it's a slideLayout relationship
		if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" {
			targetURI := opc.ResolveRelationshipTarget(masterPartURI, rel.Target)
			layoutURIs = append(layoutURIs, targetURI)
		}
		// Check if it's a theme relationship
		if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" {
			themeURI = opc.ResolveRelationshipTarget(masterPartURI, rel.Target)
		}
	}

	return layoutURIs, themeURI, nil
}

// parseLayouts extracts all slide layouts from the masters
func parseLayouts(session opc.PackageSession, graph *PresentationGraph) error {
	for _, master := range graph.Masters {
		for _, layoutURI := range master.LinkedLayoutURIs {
			// Read the layout XML
			layoutDoc, err := session.ReadXMLPart(layoutURI)
			if err != nil {
				return fmt.Errorf("failed to read layout %s: %w", layoutURI, err)
			}

			layoutRoot := layoutDoc.Root()
			if layoutRoot == nil {
				return fmt.Errorf("layout XML root element not found in %s", layoutURI)
			}

			// Extract the layout name from p:cSld@name
			cSld := xmlx.FindChild(layoutRoot, namespaces.NsP, "cSld")
			layoutName := ""
			if cSld != nil {
				layoutName = cSld.SelectAttrValue("name", "")
			}

			// Get the master reference from the layout's relationships
			masterPartURI := ""
			layoutRels := session.ListRelationships(layoutURI)
			for _, rel := range layoutRels {
				if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" {
					masterPartURI = opc.ResolveRelationshipTarget(layoutURI, rel.Target)
					break
				}
			}

			layout := LayoutRef{
				PartURI:       layoutURI,
				Name:          layoutName,
				MasterPartURI: masterPartURI,
			}
			graph.Layouts = append(graph.Layouts, layout)
		}
	}

	return nil
}

// parseSlides extracts all slides from the presentation
func parseSlides(doc *etree.Document, relMap map[string]string, session opc.PackageSession, graph *PresentationGraph) error {
	root := doc.Root()
	if root == nil {
		return fmt.Errorf("presentation.xml root element not found")
	}

	// Find p:sldIdLst element. Some otherwise-valid decks omit an empty slide list;
	// normalize that to an empty presentation instead of treating it as a hard error.
	slideIdList := xmlx.FindChild(root, namespaces.NsP, "sldIdLst")
	if slideIdList == nil {
		return nil
	}

	// Find all p:sldId children
	slideIds := xmlx.FindChildren(slideIdList, namespaces.NsP, "sldId")

	for slideNumber, slideId := range slideIds {
		// Get the r:id attribute (relationships namespace prefix is "r")
		rid := slideId.SelectAttrValue("r:id", "")
		if rid == "" {
			continue
		}
		slideID := uint32(0)
		if rawID := slideId.SelectAttrValue("id", ""); rawID != "" {
			parsedID, err := strconv.ParseUint(rawID, 10, 32)
			if err != nil {
				return fmt.Errorf("invalid p:sldId id %q: %w", rawID, err)
			}
			slideID = uint32(parsedID)
		}

		// Resolve the relationship target
		slidePartURI, exists := relMap[rid]
		if !exists {
			return fmt.Errorf("relationship %s not found in presentation.xml.rels", rid)
		}

		// Get layout and notes for this slide
		layoutPartURI, notesPartURI, err := getSlideLayoutAndNotes(session, slidePartURI)
		if err != nil {
			return err
		}

		slide := SlideRef{
			PartURI:        slidePartURI,
			SlideNumber:    slideNumber + 1, // 1-based numbering
			SlideID:        slideID,
			RelationshipID: rid,
			LayoutPartURI:  layoutPartURI,
			NotesPartURI:   notesPartURI,
		}
		graph.Slides = append(graph.Slides, slide)
	}

	return nil
}

// getSlideLayoutAndNotes retrieves the layout and optional notes for a slide
func getSlideLayoutAndNotes(session opc.PackageSession, slidePartURI string) (string, string, error) {
	layoutPartURI := ""
	notesPartURI := ""

	// Get relationships for this slide
	slideRels := session.ListRelationships(slidePartURI)

	for _, rel := range slideRels {
		// Check if it's a slideLayout relationship
		if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" {
			layoutPartURI = opc.ResolveRelationshipTarget(slidePartURI, rel.Target)
		}
		// Check if it's a notesSlide relationship
		if rel.Type == "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" {
			notesPartURI = opc.ResolveRelationshipTarget(slidePartURI, rel.Target)
		}
	}

	return layoutPartURI, notesPartURI, nil
}
