package mutate

import (
	"fmt"
	"regexp"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

const notesMasterRelationshipType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster"

const notesMasterContentType = "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml"

var notesMasterPartNamePattern = regexp.MustCompile(`^/ppt/notesMasters/notesMaster(\d+)\.xml$`)

// SetNotesRequest sets (replacing) the speaker notes text for a slide.
type SetNotesRequest struct {
	Package     opc.PackageSession
	SlideNumber int
	// Text is the plain-text notes body. Embedded "\n" separates paragraphs.
	Text string
}

// ClearNotesRequest clears the speaker notes text for a slide while preserving
// the notesSlide part and its relationship.
type ClearNotesRequest struct {
	Package     opc.PackageSession
	SlideNumber int
}

// SetNotesResult describes the outcome of a notes mutation.
type SetNotesResult struct {
	Slide               int    `json:"slide"`
	SlidePartURI        string `json:"slidePartUri"`
	NotesURI            string `json:"notesUri"`
	Text                string `json:"text"`
	CreatedPart         bool   `json:"createdPart"`
	CreatedRelationship bool   `json:"createdRelationship"`
}

// SetNotesForSlide replaces the speaker notes text for the targeted slide,
// creating the notesSlide part, its relationship, and content-type override when
// absent.
func SetNotesForSlide(req *SetNotesRequest) (*SetNotesResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("set-notes requires an open package session")
	}
	return applyNotesText(req.Package, req.SlideNumber, req.Text)
}

// ClearNotesForSlide empties the speaker notes text for the targeted slide. The
// notesSlide part and its relationship are created if absent (resulting in an
// empty notes body) so the readback contract is uniform.
func ClearNotesForSlide(req *ClearNotesRequest) (*SetNotesResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("clear-notes requires an open package session")
	}
	return applyNotesText(req.Package, req.SlideNumber, "")
}

func applyNotesText(session opc.PackageSession, slideNumber int, text string) (*SetNotesResult, error) {
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		return nil, fmt.Errorf("failed to parse presentation: %w", err)
	}
	if slideNumber < 1 || slideNumber > len(graph.Slides) {
		return nil, fmt.Errorf("slide %d not found (presentation has %d slides)", slideNumber, len(graph.Slides))
	}
	slide := graph.Slides[slideNumber-1]

	notesURI, createdPart, createdRel, err := ensureNotesPartForSlide(session, graph, slide)
	if err != nil {
		return nil, err
	}

	notesDoc, err := session.ReadXMLPart(notesURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read notes part %s: %w", notesURI, err)
	}
	if err := setNotesBodyText(notesDoc, text); err != nil {
		return nil, err
	}
	if err := session.ReplaceXMLPart(notesURI, notesDoc); err != nil {
		return nil, fmt.Errorf("failed to replace notes part %s: %w", notesURI, err)
	}

	return &SetNotesResult{
		Slide:               slideNumber,
		SlidePartURI:        slide.PartURI,
		NotesURI:            notesURI,
		Text:                text,
		CreatedPart:         createdPart,
		CreatedRelationship: createdRel,
	}, nil
}

// ensureNotesPartForSlide returns the notesSlide URI for the slide, creating the
// part (with a body placeholder text body), its content-type override, and the
// slide->notesSlide relationship when they do not yet exist. When the deck has a
// notesMaster, the new notesSlide part links to it.
func ensureNotesPartForSlide(session opc.PackageSession, graph *inspect.PresentationGraph, slide inspect.SlideRef) (string, bool, bool, error) {
	if slide.NotesPartURI != "" {
		return slide.NotesPartURI, false, false, nil
	}

	notesURI, err := allocateNumberedPartName(session, notesPartNamePattern, "/ppt/notesSlides/notesSlide%d.xml")
	if err != nil {
		return "", false, false, err
	}

	notesDoc := createNotesSlideDocument()
	notesXML, err := writeXML(notesDoc)
	if err != nil {
		return "", false, false, fmt.Errorf("failed to serialize notes slide XML: %w", err)
	}
	if err := session.AddPart(notesURI, notesXML, notesContentType, nil); err != nil {
		return "", false, false, fmt.Errorf("failed to add notes part %s: %w", notesURI, err)
	}

	// A notesSlide whose clrMapOvr inherits the master color map requires a
	// notesMaster to inherit from. When the deck has none, synthesize a minimal
	// notesMaster part and its presentation-level relationship so the package
	// stays structurally canonical (real PowerPoint always pairs a notesSlide
	// with a notesMaster).
	if findNotesMasterURI(session) == "" {
		if err := synthesizeNotesMaster(session); err != nil {
			return "", false, false, err
		}
	}

	// Link the new notesSlide back to the notesMaster (if one exists) and to the
	// owning slide, mirroring real PowerPoint notesSlide relationships.
	notesRels := []opc.RelationshipInfo{}
	if notesMasterURI := findNotesMasterURI(session); notesMasterURI != "" {
		notesMasterTarget, err := relationshipTarget(notesURI, notesMasterURI)
		if err != nil {
			return "", false, false, err
		}
		notesRels = append(notesRels, opc.RelationshipInfo{
			SourceURI: notesURI,
			ID:        AllocateRelationshipID(notesRels),
			Type:      notesMasterRelationshipType,
			Target:    notesMasterTarget,
		})
	}
	slideTarget, err := relationshipTarget(notesURI, slide.PartURI)
	if err != nil {
		return "", false, false, err
	}
	notesRels = append(notesRels, opc.RelationshipInfo{
		SourceURI: notesURI,
		ID:        AllocateRelationshipID(notesRels),
		Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide",
		Target:    slideTarget,
	})
	if err := opc.WriteRelationships(session, notesURI, notesRels); err != nil {
		return "", false, false, fmt.Errorf("failed to write notes relationships: %w", err)
	}

	// Add the forward slide->notesSlide relationship.
	slideRels := session.ListRelationships(slide.PartURI)
	notesTarget, err := relationshipTarget(slide.PartURI, notesURI)
	if err != nil {
		return "", false, false, err
	}
	slideRels = append(slideRels, opc.RelationshipInfo{
		SourceURI: slide.PartURI,
		ID:        opc.AllocateRelationshipID(slideRels),
		Type:      notesRelationshipType,
		Target:    notesTarget,
	})
	if err := opc.WriteRelationships(session, slide.PartURI, slideRels); err != nil {
		return "", false, false, fmt.Errorf("failed to write slide notes relationship: %w", err)
	}

	return notesURI, true, true, nil
}

// findNotesMasterURI resolves the notesMaster part URI from presentation.xml
// relationships, returning "" when no notesMaster exists.
func findNotesMasterURI(session opc.PackageSession) string {
	for _, rel := range session.ListRelationships("/ppt/presentation.xml") {
		if rel.Type == notesMasterRelationshipType {
			return opc.ResolveRelationshipTarget("/ppt/presentation.xml", rel.Target)
		}
	}
	return ""
}

// synthesizeNotesMaster creates a minimal notesMaster part and the
// presentation-level notesMaster relationship for a deck that has none, so that
// a newly created notesSlide has a master to inherit its color map from. The
// notesMaster reuses the existing presentation theme; no notesMasterIdLst is
// added (canonical fixtures omit it).
func synthesizeNotesMaster(session opc.PackageSession) error {
	notesMasterURI, err := allocateNumberedPartName(session, notesMasterPartNamePattern, "/ppt/notesMasters/notesMaster%d.xml")
	if err != nil {
		return err
	}

	masterDoc := createNotesMasterDocument()
	masterXML, err := writeXML(masterDoc)
	if err != nil {
		return fmt.Errorf("failed to serialize notes master XML: %w", err)
	}
	if err := session.AddPart(notesMasterURI, masterXML, notesMasterContentType, nil); err != nil {
		return fmt.Errorf("failed to add notes master part %s: %w", notesMasterURI, err)
	}

	// Link the notesMaster to an existing presentation theme when one is present
	// (a theme part may legitimately be the target of multiple relationships).
	if themeURI := findPresentationThemeURI(session); themeURI != "" {
		themeTarget, err := relationshipTarget(notesMasterURI, themeURI)
		if err != nil {
			return err
		}
		masterRels := []opc.RelationshipInfo{{
			SourceURI: notesMasterURI,
			ID:        "rId1",
			Type:      themeRelationshipType,
			Target:    themeTarget,
		}}
		if err := opc.WriteRelationships(session, notesMasterURI, masterRels); err != nil {
			return fmt.Errorf("failed to write notes master relationships: %w", err)
		}
	}

	// Add the presentation-level notesMaster relationship.
	presURI := "/ppt/presentation.xml"
	presRels := session.ListRelationships(presURI)
	masterTarget, err := relationshipTarget(presURI, notesMasterURI)
	if err != nil {
		return err
	}
	presRels = append(presRels, opc.RelationshipInfo{
		SourceURI: presURI,
		ID:        opc.AllocateRelationshipID(presRels),
		Type:      notesMasterRelationshipType,
		Target:    masterTarget,
	})
	if err := opc.WriteRelationships(session, presURI, presRels); err != nil {
		return fmt.Errorf("failed to write presentation notes master relationship: %w", err)
	}

	return nil
}

// findPresentationThemeURI resolves the first theme part referenced from
// presentation.xml, returning "" when none exists.
func findPresentationThemeURI(session opc.PackageSession) string {
	for _, rel := range session.ListRelationships("/ppt/presentation.xml") {
		if rel.Type == themeRelationshipType {
			return opc.ResolveRelationshipTarget("/ppt/presentation.xml", rel.Target)
		}
	}
	return ""
}

// createNotesMasterDocument builds a minimal, schema-ordered notesMaster part:
// an empty group shape tree followed by the standard color map. Optional
// placeholders and notesStyle are omitted.
func createNotesMasterDocument() *etree.Document {
	doc := etree.NewDocument()
	root := etree.NewElement("p:notesMaster")
	root.CreateAttr("xmlns:a", namespaces.NsA)
	root.CreateAttr("xmlns:p", namespaces.NsP)
	root.CreateAttr("xmlns:r", namespaces.NsR)

	cSld := etree.NewElement("p:cSld")
	spTree := etree.NewElement("p:spTree")

	nvGrpSpPr := etree.NewElement("p:nvGrpSpPr")
	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", "1")
	cNvPr.CreateAttr("name", "")
	nvGrpSpPr.AddChild(cNvPr)
	nvGrpSpPr.AddChild(etree.NewElement("p:cNvGrpSpPr"))
	nvGrpSpPr.AddChild(etree.NewElement("p:nvPr"))
	spTree.AddChild(nvGrpSpPr)
	spTree.AddChild(etree.NewElement("p:grpSpPr"))

	cSld.AddChild(spTree)
	root.AddChild(cSld)

	clrMap := etree.NewElement("p:clrMap")
	clrMap.CreateAttr("bg1", "lt1")
	clrMap.CreateAttr("tx1", "dk1")
	clrMap.CreateAttr("bg2", "lt2")
	clrMap.CreateAttr("tx2", "dk2")
	clrMap.CreateAttr("accent1", "accent1")
	clrMap.CreateAttr("accent2", "accent2")
	clrMap.CreateAttr("accent3", "accent3")
	clrMap.CreateAttr("accent4", "accent4")
	clrMap.CreateAttr("accent5", "accent5")
	clrMap.CreateAttr("accent6", "accent6")
	clrMap.CreateAttr("hlink", "hlink")
	clrMap.CreateAttr("folHlink", "folHlink")
	root.AddChild(clrMap)

	doc.SetRoot(root)
	return doc
}

// createNotesSlideDocument builds a minimal, schema-ordered notesSlide part with
// a body placeholder shape ready to receive notes text.
func createNotesSlideDocument() *etree.Document {
	doc := etree.NewDocument()
	root := etree.NewElement("p:notes")
	root.CreateAttr("xmlns:a", namespaces.NsA)
	root.CreateAttr("xmlns:p", namespaces.NsP)
	root.CreateAttr("xmlns:r", namespaces.NsR)

	cSld := etree.NewElement("p:cSld")
	spTree := etree.NewElement("p:spTree")

	nvGrpSpPr := etree.NewElement("p:nvGrpSpPr")
	cNvPr := etree.NewElement("p:cNvPr")
	cNvPr.CreateAttr("id", "1")
	cNvPr.CreateAttr("name", "")
	nvGrpSpPr.AddChild(cNvPr)
	nvGrpSpPr.AddChild(etree.NewElement("p:cNvGrpSpPr"))
	nvGrpSpPr.AddChild(etree.NewElement("p:nvPr"))
	spTree.AddChild(nvGrpSpPr)

	grpSpPr := etree.NewElement("p:grpSpPr")
	spTree.AddChild(grpSpPr)

	// Body placeholder shape that carries the notes text.
	sp := etree.NewElement("p:sp")
	nvSpPr := etree.NewElement("p:nvSpPr")
	spCNvPr := etree.NewElement("p:cNvPr")
	spCNvPr.CreateAttr("id", "2")
	spCNvPr.CreateAttr("name", "Notes Placeholder 1")
	nvSpPr.AddChild(spCNvPr)
	cNvSpPr := etree.NewElement("p:cNvSpPr")
	spLocks := etree.NewElement("a:spLocks")
	spLocks.CreateAttr("noGrp", "1")
	cNvSpPr.AddChild(spLocks)
	nvSpPr.AddChild(cNvSpPr)
	nvPr := etree.NewElement("p:nvPr")
	ph := etree.NewElement("p:ph")
	ph.CreateAttr("type", "body")
	ph.CreateAttr("idx", "1")
	nvPr.AddChild(ph)
	nvSpPr.AddChild(nvPr)
	sp.AddChild(nvSpPr)
	sp.AddChild(etree.NewElement("p:spPr"))
	sp.AddChild(newEmptyTextBody())
	spTree.AddChild(sp)

	cSld.AddChild(spTree)
	root.AddChild(cSld)

	clrMapOvr := etree.NewElement("p:clrMapOvr")
	clrMapOvr.AddChild(etree.NewElement("a:masterClrMapping"))
	root.AddChild(clrMapOvr)

	doc.SetRoot(root)
	return doc
}

// setNotesBodyText locates the body placeholder shape in a notesSlide document
// and replaces its text with text split into paragraphs on "\n". An empty text
// produces a single empty paragraph.
func setNotesBodyText(doc *etree.Document, text string) error {
	if doc == nil || doc.Root() == nil {
		return fmt.Errorf("notes document is empty")
	}
	root := doc.Root()
	cSld := xmlx.FindChild(root, namespaces.NsP, "cSld")
	if cSld == nil {
		return fmt.Errorf("notes slide is missing p:cSld")
	}
	spTree := xmlx.FindChild(cSld, namespaces.NsP, "spTree")
	if spTree == nil {
		return fmt.Errorf("notes slide is missing p:spTree")
	}

	txBody := findNotesBodyTxBody(spTree)
	if txBody == nil {
		return fmt.Errorf("notes slide has no body placeholder shape to hold notes text")
	}

	// Remove existing paragraphs, preserve bodyPr/lstStyle ordering.
	for _, p := range xmlx.FindChildren(txBody, namespaces.NsA, "p") {
		txBody.RemoveChild(p)
	}
	for _, line := range splitNotesParagraphs(text) {
		p := etree.NewElement("a:p")
		if line != "" {
			r := etree.NewElement("a:r")
			t := etree.NewElement("a:t")
			t.SetText(line)
			r.AddChild(t)
			p.AddChild(r)
		}
		txBody.AddChild(p)
	}
	return nil
}

// findNotesBodyTxBody returns the txBody of the body-type placeholder shape, or
// nil when none exists.
func findNotesBodyTxBody(spTree *etree.Element) *etree.Element {
	for _, sp := range xmlx.FindChildren(spTree, namespaces.NsP, "sp") {
		nvSpPr := xmlx.FindChild(sp, namespaces.NsP, "nvSpPr")
		if nvSpPr == nil {
			continue
		}
		nvPr := xmlx.FindChild(nvSpPr, namespaces.NsP, "nvPr")
		if nvPr == nil {
			continue
		}
		ph := xmlx.FindChild(nvPr, namespaces.NsP, "ph")
		if ph == nil || ph.SelectAttrValue("type", "") != "body" {
			continue
		}
		txBody := xmlx.FindChild(sp, namespaces.NsP, "txBody")
		if txBody == nil {
			txBody = newEmptyTextBody()
			sp.AddChild(txBody)
		}
		return txBody
	}
	return nil
}

// splitNotesParagraphs maps notes text to paragraph lines. Empty text yields a
// single empty paragraph so the txBody always holds at least one a:p.
func splitNotesParagraphs(text string) []string {
	if text == "" {
		return []string{""}
	}
	return strings.Split(text, "\n")
}
