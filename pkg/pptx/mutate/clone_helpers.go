package mutate

import (
	"encoding/xml"
	"fmt"
	"path"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

const (
	slideRelationshipType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"
	notesRelationshipType = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide"
	notesContentType      = "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml"
)

var (
	slidePartNamePattern = regexp.MustCompile(`^/ppt/slides/slide(\d+)\.xml$`)
	notesPartNamePattern = regexp.MustCompile(`^/ppt/notesSlides/notesSlide(\d+)\.xml$`)
	relationshipIDPatten = regexp.MustCompile(`^rId(\d+)$`)
)

// RelationshipDecision describes whether a relationship should be shared or duplicated.
type RelationshipDecision string

const (
	// RelDecisionShared means reuse the same target (for layouts, masters, themes, media, etc.).
	RelDecisionShared RelationshipDecision = "shared"
	// RelDecisionDuplicated means create a new copy (for notes and other explicitly duplicated parts).
	RelDecisionDuplicated RelationshipDecision = "duplicated"
)

// RemappedRelationship describes a relationship after shared/duplicated remapping.
type RemappedRelationship struct {
	OldID      string
	NewID      string
	OldType    string
	OldTarget  string
	NewTarget  string
	TargetMode string
	Decision   RelationshipDecision
}

// RemapRelationshipsRequest holds the parameters for relationship remapping.
type RemapRelationshipsRequest struct {
	SourceRelationships []opc.RelationshipInfo
	Decisions           map[string]RelationshipDecision
	TargetMapper        func(oldTarget string, relType string) (string, error)
}

// NotesClonePolicy determines how notes should be handled during cloning.
type NotesClonePolicy string

const (
	// NotesClone duplicates the notes slide for the cloned slide.
	NotesClone NotesClonePolicy = "clone"
	// NotesDrop omits notes from the cloned slide.
	NotesDrop NotesClonePolicy = "drop"
)

// CloneNotesRequest holds parameters for notes cloning.
type CloneNotesRequest struct {
	Session                  opc.PackageSession
	SourceNotesURI           string
	DestinationSlideURI      string
	DestinationRelationships []opc.RelationshipInfo
	Policy                   NotesClonePolicy
}

// CloneNotesResult holds the result of notes cloning.
type CloneNotesResult struct {
	NewNotesURI       string
	NotesRelationship opc.RelationshipInfo
}

// InsertSlideReferenceRequest holds parameters for inserting a slide reference.
type InsertSlideReferenceRequest struct {
	PresentationDoc           *etree.Document
	PresentationRelationships []opc.RelationshipInfo
	SlidePartURI              string
	SlideID                   uint32
	Position                  int // 0-based position in the slide list; -1 means append
}

// InsertSlideReferenceResult describes the created presentation relationship.
type InsertSlideReferenceResult struct {
	RelationshipID string
	Relationship   opc.RelationshipInfo
}

// AllocateSlidePartName finds an unused slide part name in the package.
// Returns a path like /ppt/slides/slide13.xml.
func AllocateSlidePartName(session opc.PackageSession) (string, error) {
	return allocateNumberedPartName(session, slidePartNamePattern, "/ppt/slides/slide%d.xml")
}

// AllocateSlideID finds an unused slide ID in the presentation.
// IDs start at 256 following common PowerPoint conventions.
func AllocateSlideID(presentationDoc *etree.Document) (uint32, error) {
	if presentationDoc == nil || presentationDoc.Root() == nil {
		return 0, fmt.Errorf("presentation.xml root element not found")
	}

	root := presentationDoc.Root()
	sldIDList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst")
	if sldIDList == nil {
		return 256, nil
	}

	maxID := uint32(255)
	for _, sldID := range namespaces.FindChildren(sldIDList, namespaces.NsP, "sldId") {
		parsed, err := strconv.ParseUint(sldID.SelectAttrValue("id", ""), 10, 32)
		if err != nil {
			continue
		}
		if uint32(parsed) > maxID {
			maxID = uint32(parsed)
		}
	}

	return maxID + 1, nil
}

// AllocateRelationshipID finds an unused relationship ID in a relationships list.
func AllocateRelationshipID(rels []opc.RelationshipInfo) string {
	maxID := 0
	for _, rel := range rels {
		matches := relationshipIDPatten.FindStringSubmatch(rel.ID)
		if matches == nil {
			continue
		}
		parsed, err := strconv.Atoi(matches[1])
		if err != nil {
			continue
		}
		if parsed > maxID {
			maxID = parsed
		}
	}
	return fmt.Sprintf("rId%d", maxID+1)
}

// RemapRelationships remaps relationships according to explicit decisions.
// Shared relationships keep the same ID and target. Duplicated relationships receive
// a fresh relationship ID and optionally a mapped target.
func RemapRelationships(req *RemapRelationshipsRequest) ([]RemappedRelationship, error) {
	if req == nil || len(req.SourceRelationships) == 0 {
		return []RemappedRelationship{}, nil
	}

	usedIDs := make(map[string]struct{}, len(req.SourceRelationships))
	for _, rel := range req.SourceRelationships {
		usedIDs[rel.ID] = struct{}{}
	}

	result := make([]RemappedRelationship, 0, len(req.SourceRelationships))
	for _, rel := range req.SourceRelationships {
		decision := RelDecisionShared
		if req.Decisions != nil {
			if explicit, ok := req.Decisions[rel.ID]; ok {
				decision = explicit
			}
		}

		remapped := RemappedRelationship{
			OldID:      rel.ID,
			OldType:    rel.Type,
			OldTarget:  rel.Target,
			NewID:      rel.ID,
			NewTarget:  rel.Target,
			TargetMode: rel.TargetMode,
			Decision:   decision,
		}

		if decision == RelDecisionDuplicated {
			remapped.NewID = allocateRelationshipIDFromUsed(usedIDs)
			if req.TargetMapper != nil {
				newTarget, err := req.TargetMapper(rel.Target, rel.Type)
				if err != nil {
					return nil, fmt.Errorf("failed to map target for %s: %w", rel.ID, err)
				}
				remapped.NewTarget = newTarget
			}
		}

		result = append(result, remapped)
	}

	return result, nil
}

// UpdateContentTypes validates content-type expectations for a part that has already
// been added or replaced through the package session API.
func UpdateContentTypes(session opc.PackageSession, newPartURI string, contentType string) error {
	if session == nil {
		return fmt.Errorf("session cannot be nil")
	}
	if newPartURI == "" {
		return fmt.Errorf("part URI cannot be empty")
	}
	if contentType == "" {
		return fmt.Errorf("content type cannot be empty")
	}

	current := session.GetContentType(newPartURI)
	if current != "" && current != contentType {
		return fmt.Errorf("content type mismatch for %s: have %s want %s", newPartURI, current, contentType)
	}
	return nil
}

// CloneNotes clones or drops notes for a destination slide.
func CloneNotes(req *CloneNotesRequest) (*CloneNotesResult, error) {
	if req == nil {
		return &CloneNotesResult{}, nil
	}
	if req.Policy == NotesDrop || req.SourceNotesURI == "" {
		return &CloneNotesResult{}, nil
	}
	if req.Policy != NotesClone {
		return nil, fmt.Errorf("unknown notes policy: %s", req.Policy)
	}
	if req.Session == nil {
		return nil, fmt.Errorf("session cannot be nil")
	}
	if req.DestinationSlideURI == "" {
		return nil, fmt.Errorf("destination slide URI cannot be empty")
	}

	sourceDoc, err := req.Session.ReadXMLPart(req.SourceNotesURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source notes: %w", err)
	}

	newNotesURI, err := allocateNumberedPartName(req.Session, notesPartNamePattern, "/ppt/notesSlides/notesSlide%d.xml")
	if err != nil {
		return nil, err
	}

	data, err := sourceDoc.WriteToBytes()
	if err != nil {
		return nil, fmt.Errorf("failed to serialize notes: %w", err)
	}
	if err := req.Session.AddPart(newNotesURI, data, notesContentType, nil); err != nil {
		return nil, fmt.Errorf("failed to add notes part: %w", err)
	}
	if err := UpdateContentTypes(req.Session, newNotesURI, notesContentType); err != nil {
		return nil, err
	}

	// Preserve notes-side relationships such as notesMaster/theme links when present,
	// but retarget the notes->slide backlink to the cloned destination slide.
	sourceNotesRels := req.Session.ListRelationships(req.SourceNotesURI)
	if len(sourceNotesRels) > 0 {
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

			targetURI := opc.ResolveRelationshipTarget(req.SourceNotesURI, rel.Target)
			if rel.Type == slideRelationshipType {
				targetURI = req.DestinationSlideURI
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
		if err := req.Session.AddPart(notesRelsURI, relsXML, "application/vnd.openxmlformats-package.relationships+xml", nil); err != nil {
			return nil, fmt.Errorf("failed to add notes relationships part: %w", err)
		}
	}

	relTarget, err := relationshipTarget(req.DestinationSlideURI, newNotesURI)
	if err != nil {
		return nil, err
	}
	newRelID := AllocateRelationshipID(req.DestinationRelationships)

	return &CloneNotesResult{
		NewNotesURI: newNotesURI,
		NotesRelationship: opc.RelationshipInfo{
			SourceURI: req.DestinationSlideURI,
			ID:        newRelID,
			Type:      notesRelationshipType,
			Target:    relTarget,
		},
	}, nil
}

// InsertSlideReference inserts a p:sldId entry into presentation.xml and returns the
// matching relationship entry that should be written to presentation.xml.rels.
func InsertSlideReference(req *InsertSlideReferenceRequest) (*InsertSlideReferenceResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.PresentationDoc == nil || req.PresentationDoc.Root() == nil {
		return nil, fmt.Errorf("presentation.xml root element not found")
	}
	if req.SlidePartURI == "" {
		return nil, fmt.Errorf("slide part URI cannot be empty")
	}
	if req.SlideID == 0 {
		return nil, fmt.Errorf("slide ID must be non-zero")
	}

	root := req.PresentationDoc.Root()
	ensureNamespace(root, "r", namespaces.NsR)

	sldIDList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst")
	if sldIDList == nil {
		sldIDList = etree.NewElement("p:sldIdLst")
		root.AddChild(sldIDList)
	}

	newRelID := AllocateRelationshipID(req.PresentationRelationships)
	newSldID := etree.NewElement("p:sldId")
	newSldID.CreateAttr("id", strconv.FormatUint(uint64(req.SlideID), 10))
	newSldID.CreateAttr("r:id", newRelID)

	existing := namespaces.FindChildren(sldIDList, namespaces.NsP, "sldId")
	if req.Position >= 0 && req.Position < len(existing) {
		sldIDList.InsertChildAt(existing[req.Position].Index(), newSldID)
	} else {
		sldIDList.AddChild(newSldID)
	}

	relTarget, err := relationshipTarget("/ppt/presentation.xml", req.SlidePartURI)
	if err != nil {
		return nil, err
	}

	return &InsertSlideReferenceResult{
		RelationshipID: newRelID,
		Relationship: opc.RelationshipInfo{
			SourceURI: "/ppt/presentation.xml",
			ID:        newRelID,
			Type:      slideRelationshipType,
			Target:    relTarget,
		},
	}, nil
}

// BuildRelationshipsXML serializes a .rels payload from relationship metadata.
func BuildRelationshipsXML(rels []opc.RelationshipInfo) ([]byte, error) {
	type xmlRelationship struct {
		ID         string `xml:"Id,attr"`
		Type       string `xml:"Type,attr"`
		Target     string `xml:"Target,attr"`
		TargetMode string `xml:"TargetMode,attr,omitempty"`
	}
	payload := struct {
		XMLName       xml.Name          `xml:"http://schemas.openxmlformats.org/package/2006/relationships Relationships"`
		Relationships []xmlRelationship `xml:"Relationship"`
	}{
		Relationships: make([]xmlRelationship, 0, len(rels)),
	}

	for _, rel := range rels {
		payload.Relationships = append(payload.Relationships, xmlRelationship{
			ID:         rel.ID,
			Type:       rel.Type,
			Target:     rel.Target,
			TargetMode: rel.TargetMode,
		})
	}

	data, err := xml.MarshalIndent(payload, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("failed to marshal relationships XML: %w", err)
	}
	return append([]byte(xml.Header), append(data, '\n')...), nil
}

// FindNextRId is a convenience wrapper around AllocateRelationshipID.
func FindNextRId(rels []opc.RelationshipInfo) string {
	return AllocateRelationshipID(rels)
}

// IsLayoutOrMasterRelationship reports whether the relationship should be shared.
func IsLayoutOrMasterRelationship(rel opc.RelationshipInfo) bool {
	return strings.Contains(rel.Type, "slideLayout") ||
		strings.Contains(rel.Type, "slideMaster") ||
		strings.Contains(rel.Type, "theme")
}

// IsNoteSlideRelationship reports whether the relationship points at a notes slide.
func IsNoteSlideRelationship(rel opc.RelationshipInfo) bool {
	return strings.Contains(rel.Type, "notesSlide")
}

// IsMediaRelationship reports whether the relationship points at media that should
// normally be shared rather than duplicated.
func IsMediaRelationship(rel opc.RelationshipInfo) bool {
	return strings.Contains(rel.Type, "image") ||
		strings.Contains(rel.Type, "audio") ||
		strings.Contains(rel.Type, "video") ||
		strings.Contains(rel.Type, "embeddedPackage")
}

// BuildDefaultRelationshipDecisions produces the default shared-vs-duplicated policy.
func BuildDefaultRelationshipDecisions(rels []opc.RelationshipInfo) map[string]RelationshipDecision {
	decisions := make(map[string]RelationshipDecision, len(rels))
	for _, rel := range rels {
		if IsNoteSlideRelationship(rel) {
			decisions[rel.ID] = RelDecisionDuplicated
			continue
		}
		decisions[rel.ID] = RelDecisionShared
	}
	return decisions
}

func allocateNumberedPartName(session opc.PackageSession, pattern *regexp.Regexp, format string) (string, error) {
	if session == nil {
		return "", fmt.Errorf("session cannot be nil")
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

	return fmt.Sprintf(format, max+1), nil
}

func allocateRelationshipIDFromUsed(used map[string]struct{}) string {
	candidate := 1
	for {
		id := fmt.Sprintf("rId%d", candidate)
		if _, exists := used[id]; !exists {
			used[id] = struct{}{}
			return id
		}
		candidate++
	}
}

func relationshipTarget(sourceURI string, targetURI string) (string, error) {
	if sourceURI == "" {
		return "", fmt.Errorf("source URI cannot be empty")
	}
	if targetURI == "" {
		return "", fmt.Errorf("target URI cannot be empty")
	}
	if strings.Contains(targetURI, "://") {
		return targetURI, nil
	}
	if !strings.HasPrefix(sourceURI, "/") || !strings.HasPrefix(targetURI, "/") {
		return "", fmt.Errorf("source and target URIs must be package-absolute: %s -> %s", sourceURI, targetURI)
	}

	sourceDir := path.Dir(strings.TrimPrefix(sourceURI, "/"))
	targetPath := strings.TrimPrefix(targetURI, "/")
	rel, err := filepath.Rel(sourceDir, targetPath)
	if err != nil {
		return "", fmt.Errorf("failed to relativize %s against %s: %w", targetURI, sourceURI, err)
	}
	return filepath.ToSlash(rel), nil
}

func ensureNamespace(elem *etree.Element, prefix string, uri string) {
	if elem == nil {
		return
	}
	name := "xmlns:" + prefix
	if elem.SelectAttr(name) == nil {
		elem.CreateAttr(name, uri)
	}
}
