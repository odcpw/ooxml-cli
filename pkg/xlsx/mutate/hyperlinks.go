package mutate

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Hyperlink describes one worksheet hyperlink.
type Hyperlink struct {
	Ref      string `json:"ref"`
	URL      string `json:"url,omitempty"`
	Location string `json:"location,omitempty"`
	Display  string `json:"display,omitempty"`
	Tooltip  string `json:"tooltip,omitempty"`
	RelID    string `json:"relId,omitempty"`
	Broken   bool   `json:"broken,omitempty"`
}

// HyperlinkMutationResult reports a hyperlink mutation outcome.
type HyperlinkMutationResult struct {
	Ref       string `json:"ref"`
	Hyperlink Hyperlink
}

// AddHyperlinkRequest adds or replaces a hyperlink on a cell or range ref.
type AddHyperlinkRequest struct {
	Package  opc.PackageSession
	SheetRef model.SheetRef
	Ref      string
	URL      string
	Location string
	Display  string
	Tooltip  string
	Replace  bool // allow overwriting an existing hyperlink on the same ref
}

// UpdateHyperlinkRequest mutates an existing hyperlink on a ref.
type UpdateHyperlinkRequest struct {
	Package        opc.PackageSession
	SheetRef       model.SheetRef
	Ref            string
	URL            string
	Location       string
	Display        string
	Tooltip        string
	SetURL         bool
	SetLocation    bool
	SetDisplay     bool
	SetTooltip     bool
	ExpectURL      string
	ExpectLocation string
	HasExpectURL   bool
	HasExpectLoc   bool
}

// DeleteHyperlinkRequest removes a hyperlink from a ref.
type DeleteHyperlinkRequest struct {
	Package        opc.PackageSession
	SheetRef       model.SheetRef
	Ref            string
	ExpectURL      string
	ExpectLocation string
	HasExpectURL   bool
	HasExpectLoc   bool
}

// NormalizeHyperlinkRef canonicalizes an A1 cell or range reference.
func NormalizeHyperlinkRef(ref string) (string, error) {
	ref = strings.TrimSpace(ref)
	if ref == "" {
		return "", fmt.Errorf("hyperlink ref cannot be empty")
	}
	if strings.Contains(ref, ":") {
		parsed, err := address.ParseRange(ref)
		if err != nil {
			return "", err
		}
		return parsed.String(), nil
	}
	parsed, err := address.ParseCell(ref)
	if err != nil {
		return "", err
	}
	return parsed.String(), nil
}

// ListHyperlinks returns the worksheet's hyperlinks with resolved targets.
func ListHyperlinks(session opc.PackageSession, sheet model.SheetRef) ([]Hyperlink, error) {
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return nil, err
	}
	relTargets, relModes := hyperlinkRelMaps(session, sheet)
	var out []Hyperlink
	hyperlinks := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "hyperlinks")
	if hyperlinks == nil {
		return out, nil
	}
	for _, hl := range namespaces.FindChildren(hyperlinks, namespaces.NsSpreadsheetML, "hyperlink") {
		entry := Hyperlink{
			Ref:      hl.SelectAttrValue("ref", ""),
			Location: hl.SelectAttrValue("location", ""),
			Display:  hl.SelectAttrValue("display", ""),
			Tooltip:  hl.SelectAttrValue("tooltip", ""),
			RelID:    hyperlinkRelAttr(hl),
		}
		if entry.RelID != "" {
			if target, ok := relTargets[entry.RelID]; ok {
				entry.URL = target
				_ = relModes
			} else {
				entry.Broken = true
			}
		}
		out = append(out, entry)
	}
	return out, nil
}

func hyperlinkRelMaps(session opc.PackageSession, sheet model.SheetRef) (map[string]string, map[string]string) {
	targets := map[string]string{}
	modes := map[string]string{}
	for _, rel := range session.ListRelationships(sheet.PartURI) {
		if rel.Type != namespaces.RelHyperlink {
			continue
		}
		targets[rel.ID] = rel.Target
		modes[rel.ID] = rel.TargetMode
	}
	return targets, modes
}

// hyperlinkRelAttr returns the r:id value regardless of namespace prefix.
func hyperlinkRelAttr(hl *etree.Element) string {
	for _, attr := range hl.Attr {
		if attr.Key == "id" && (attr.Space == "r" || attr.NamespaceURI() == namespaces.NsR) {
			return attr.Value
		}
	}
	return ""
}

func findHyperlinkElem(hyperlinks *etree.Element, normRef string) *etree.Element {
	for _, hl := range namespaces.FindChildren(hyperlinks, namespaces.NsSpreadsheetML, "hyperlink") {
		existing, err := NormalizeHyperlinkRef(hl.SelectAttrValue("ref", ""))
		if err == nil && existing == normRef {
			return hl
		}
	}
	return nil
}

func ensureRelationshipsNamespace(root *etree.Element) {
	if root.SelectAttr("xmlns:r") == nil {
		root.CreateAttr("xmlns:r", namespaces.NsR)
	}
}

func allocateWorksheetRelID(session opc.PackageSession, sheet model.SheetRef) string {
	return opc.AllocateRelationshipID(session.ListRelationships(sheet.PartURI))
}

// AddHyperlink creates a hyperlink (external URL or internal location) on a ref.
func AddHyperlink(req *AddHyperlinkRequest) (*HyperlinkMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add hyperlink request is nil")
	}
	if (req.URL == "") == (req.Location == "") {
		return nil, fmt.Errorf("specify exactly one of url or location")
	}
	normRef, err := NormalizeHyperlinkRef(req.Ref)
	if err != nil {
		return nil, err
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	prefix := root.Space
	hyperlinks := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "hyperlinks")
	if hyperlinks == nil {
		hyperlinks = newElement(prefix, "hyperlinks")
		insertWorksheetChild(root, hyperlinks, "hyperlinks")
	}
	if existing := findHyperlinkElem(hyperlinks, normRef); existing != nil {
		if !req.Replace {
			return nil, fmt.Errorf("a hyperlink already exists on %s (use update)", normRef)
		}
		if err := removeHyperlinkRel(req.Package, req.SheetRef, hyperlinkRelAttr(existing), existing); err != nil {
			return nil, err
		}
		hyperlinks.RemoveChild(existing)
	}

	hl := newElement(prefix, "hyperlink")
	hl.CreateAttr("ref", normRef)
	result := &HyperlinkMutationResult{Ref: normRef, Hyperlink: Hyperlink{Ref: normRef, Display: req.Display, Tooltip: req.Tooltip}}
	if req.URL != "" {
		ensureRelationshipsNamespace(root)
		relID := allocateWorksheetRelID(req.Package, req.SheetRef)
		rels := req.Package.ListRelationships(req.SheetRef.PartURI)
		rels = append(rels, opc.RelationshipInfo{
			SourceURI:  req.SheetRef.PartURI,
			ID:         relID,
			Type:       namespaces.RelHyperlink,
			Target:     req.URL,
			TargetMode: "External",
		})
		if err := opc.WriteRelationships(req.Package, req.SheetRef.PartURI, rels); err != nil {
			return nil, fmt.Errorf("failed to write worksheet relationships: %w", err)
		}
		hl.CreateAttr("r:id", relID)
		result.Hyperlink.URL = req.URL
		result.Hyperlink.RelID = relID
	} else {
		hl.CreateAttr("location", req.Location)
		result.Hyperlink.Location = req.Location
	}
	if req.Display != "" {
		hl.CreateAttr("display", req.Display)
	}
	if req.Tooltip != "" {
		hl.CreateAttr("tooltip", req.Tooltip)
	}
	hyperlinks.AddChild(hl)
	updateHyperlinkCount(hyperlinks)

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return result, nil
}

// UpdateHyperlink mutates an existing hyperlink on a ref.
func UpdateHyperlink(req *UpdateHyperlinkRequest) (*HyperlinkMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("update hyperlink request is nil")
	}
	if req.SetURL && req.SetLocation {
		// A hyperlink is either external (r:id) or internal (location), never
		// both; reject rather than silently letting one overwrite the other.
		return nil, fmt.Errorf("specify only one of url or location")
	}
	normRef, err := NormalizeHyperlinkRef(req.Ref)
	if err != nil {
		return nil, err
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	hyperlinks := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "hyperlinks")
	var hl *etree.Element
	if hyperlinks != nil {
		hl = findHyperlinkElem(hyperlinks, normRef)
	}
	if hl == nil {
		return nil, fmt.Errorf("no hyperlink found on %s", normRef)
	}
	current := hyperlinkFromElem(req.Package, req.SheetRef, hl)
	if err := checkHyperlinkGuards(current, req.HasExpectURL, req.ExpectURL, req.HasExpectLoc, req.ExpectLocation); err != nil {
		return nil, err
	}

	if req.SetURL {
		// switch to external; drop any internal location
		hl.RemoveAttr("location")
		ensureRelationshipsNamespace(root)
		relID := hyperlinkRelAttr(hl)
		rels := req.Package.ListRelationships(req.SheetRef.PartURI)
		if relID == "" {
			relID = opc.AllocateRelationshipID(rels)
			rels = append(rels, opc.RelationshipInfo{SourceURI: req.SheetRef.PartURI, ID: relID, Type: namespaces.RelHyperlink, Target: req.URL, TargetMode: "External"})
			hl.CreateAttr("r:id", relID)
		} else {
			for i := range rels {
				if rels[i].ID == relID && rels[i].Type == namespaces.RelHyperlink {
					rels[i].Target = req.URL
					rels[i].TargetMode = "External"
				}
			}
		}
		if err := opc.WriteRelationships(req.Package, req.SheetRef.PartURI, rels); err != nil {
			return nil, fmt.Errorf("failed to write worksheet relationships: %w", err)
		}
	}
	if req.SetLocation {
		// switch to internal; drop any external relationship
		if relID := hyperlinkRelAttr(hl); relID != "" {
			if err := removeHyperlinkRel(req.Package, req.SheetRef, relID, hl); err != nil {
				return nil, err
			}
			removeHyperlinkRelAttr(hl)
		}
		hl.CreateAttr("location", req.Location)
	}
	if req.SetDisplay {
		if req.Display == "" {
			hl.RemoveAttr("display")
		} else {
			hl.CreateAttr("display", req.Display)
		}
	}
	if req.SetTooltip {
		if req.Tooltip == "" {
			hl.RemoveAttr("tooltip")
		} else {
			hl.CreateAttr("tooltip", req.Tooltip)
		}
	}

	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	updated := hyperlinkFromElem(req.Package, req.SheetRef, hl)
	return &HyperlinkMutationResult{Ref: normRef, Hyperlink: updated}, nil
}

// DeleteHyperlink removes a hyperlink and any orphaned relationship.
func DeleteHyperlink(req *DeleteHyperlinkRequest) (*HyperlinkMutationResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete hyperlink request is nil")
	}
	normRef, err := NormalizeHyperlinkRef(req.Ref)
	if err != nil {
		return nil, err
	}
	doc, root, err := readWorksheetRoot(req.Package, req.SheetRef)
	if err != nil {
		return nil, err
	}
	hyperlinks := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "hyperlinks")
	var hl *etree.Element
	if hyperlinks != nil {
		hl = findHyperlinkElem(hyperlinks, normRef)
	}
	if hl == nil {
		return nil, fmt.Errorf("no hyperlink found on %s", normRef)
	}
	current := hyperlinkFromElem(req.Package, req.SheetRef, hl)
	if err := checkHyperlinkGuards(current, req.HasExpectURL, req.ExpectURL, req.HasExpectLoc, req.ExpectLocation); err != nil {
		return nil, err
	}
	if relID := hyperlinkRelAttr(hl); relID != "" {
		if err := removeHyperlinkRel(req.Package, req.SheetRef, relID, hl); err != nil {
			return nil, err
		}
	}
	hyperlinks.RemoveChild(hl)
	if len(namespaces.FindChildren(hyperlinks, namespaces.NsSpreadsheetML, "hyperlink")) == 0 {
		root.RemoveChild(hyperlinks)
	} else {
		updateHyperlinkCount(hyperlinks)
	}
	if err := req.Package.ReplaceXMLPart(req.SheetRef.PartURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace worksheet %s: %w", req.SheetRef.PartURI, err)
	}
	return &HyperlinkMutationResult{Ref: normRef, Hyperlink: current}, nil
}

func hyperlinkFromElem(session opc.PackageSession, sheet model.SheetRef, hl *etree.Element) Hyperlink {
	entry := Hyperlink{
		Ref:      hl.SelectAttrValue("ref", ""),
		Location: hl.SelectAttrValue("location", ""),
		Display:  hl.SelectAttrValue("display", ""),
		Tooltip:  hl.SelectAttrValue("tooltip", ""),
		RelID:    hyperlinkRelAttr(hl),
	}
	if entry.RelID != "" {
		targets, _ := hyperlinkRelMaps(session, sheet)
		if target, ok := targets[entry.RelID]; ok {
			entry.URL = target
		} else {
			entry.Broken = true
		}
	}
	return entry
}

func checkHyperlinkGuards(current Hyperlink, hasURL bool, expectURL string, hasLoc bool, expectLoc string) error {
	if hasURL && current.URL != expectURL {
		return fmt.Errorf("expected url %q but found %q", expectURL, current.URL)
	}
	if hasLoc && current.Location != expectLoc {
		return fmt.Errorf("expected location %q but found %q", expectLoc, current.Location)
	}
	return nil
}

// removeHyperlinkRel removes the relationship referenced by relID if no other
// hyperlink in the worksheet still uses it.
func removeHyperlinkRel(session opc.PackageSession, sheet model.SheetRef, relID string, exclude *etree.Element) error {
	if relID == "" {
		return nil
	}
	_, root, err := readWorksheetRoot(session, sheet)
	if err != nil {
		return err
	}
	if hyperlinks := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "hyperlinks"); hyperlinks != nil {
		for _, hl := range namespaces.FindChildren(hyperlinks, namespaces.NsSpreadsheetML, "hyperlink") {
			if hyperlinkRelAttr(hl) == relID && hl.SelectAttrValue("ref", "") != exclude.SelectAttrValue("ref", "") {
				return nil // still in use elsewhere
			}
		}
	}
	rels := session.ListRelationships(sheet.PartURI)
	var kept []opc.RelationshipInfo
	for _, rel := range rels {
		if rel.ID == relID && rel.Type == namespaces.RelHyperlink {
			continue
		}
		kept = append(kept, rel)
	}
	if len(kept) == len(rels) {
		return nil
	}
	if err := opc.WriteRelationships(session, sheet.PartURI, kept); err != nil {
		return fmt.Errorf("failed to write worksheet relationships: %w", err)
	}
	return nil
}

func removeHyperlinkRelAttr(hl *etree.Element) {
	for _, attr := range hl.Attr {
		if attr.Key == "id" && (attr.Space == "r" || attr.NamespaceURI() == namespaces.NsR) {
			hl.RemoveAttr(attr.Space + ":" + attr.Key)
			return
		}
	}
}

func updateHyperlinkCount(hyperlinks *etree.Element) {
	// CT_Hyperlinks has no count attribute; nothing to maintain.
	_ = hyperlinks
}
