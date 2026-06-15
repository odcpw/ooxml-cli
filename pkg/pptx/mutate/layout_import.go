package mutate

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// ImportLayoutRequest describes importing a single source slide layout into a target deck.
type ImportLayoutRequest struct {
	TargetPackage   opc.PackageSession
	SourcePackage   opc.PackageSession
	SourceLayoutURI string
	ThemePolicy     string
}

// ImportLayoutResult describes the imported or reused layout.
type ImportLayoutResult struct {
	SourceLayoutURI string
	TargetLayoutURI string
	TargetMasterURI string
	ThemeURI        string
	Name            string
	Imported        bool
	MasterImported  bool
}

// ImportLayout imports one source layout and the minimum required master chain to keep it registered.
func ImportLayout(req *ImportLayoutRequest) (*ImportLayoutResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.SourcePackage == nil || req.TargetPackage == nil {
		return nil, fmt.Errorf("source and target packages are required")
	}
	if strings.TrimSpace(req.SourceLayoutURI) == "" {
		return nil, fmt.Errorf("source layout URI cannot be empty")
	}

	themePolicy := strings.TrimSpace(req.ThemePolicy)
	if themePolicy == "" {
		themePolicy = "reuse"
	}
	if themePolicy != "reuse" && themePolicy != "import" {
		return nil, fmt.Errorf("unknown theme policy: %s", themePolicy)
	}

	sourceGraph, err := inspect.ParsePresentation(req.SourcePackage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse source presentation: %w", err)
	}
	targetGraph, err := inspect.ParsePresentation(req.TargetPackage)
	if err != nil {
		return nil, fmt.Errorf("failed to parse target presentation: %w", err)
	}

	sourceLayout := findLayoutRefByURI(sourceGraph.Layouts, req.SourceLayoutURI)
	if sourceLayout == nil {
		return nil, fmt.Errorf("source layout %s not found", req.SourceLayoutURI)
	}

	if matched, err := findCompatibleTargetLayoutExact(req.SourcePackage, req.TargetPackage, sourceGraph, targetGraph, req.SourceLayoutURI); err != nil {
		return nil, err
	} else if matched != "" {
		targetLayout := findLayoutRefByURI(targetGraph.Layouts, matched)
		result := &ImportLayoutResult{
			SourceLayoutURI: req.SourceLayoutURI,
			TargetLayoutURI: matched,
			Name:            sourceLayout.Name,
			Imported:        false,
		}
		if targetLayout != nil {
			result.TargetMasterURI = targetLayout.MasterPartURI
			master := findMasterRefByURI(targetGraph.Masters, targetLayout.MasterPartURI)
			if master != nil {
				result.ThemeURI = master.ThemeURI
			}
		}
		return result, nil
	}

	sourceMaster := findMasterRefByURI(sourceGraph.Masters, sourceLayout.MasterPartURI)
	if sourceMaster == nil {
		return nil, fmt.Errorf("source layout master %s not found", sourceLayout.MasterPartURI)
	}

	ctx := &partImportContext{
		SourcePackage: req.SourcePackage,
		TargetPackage: req.TargetPackage,
		Imported:      map[string]string{},
	}

	importedThemeURI := ""
	if sourceMaster.ThemeURI != "" {
		switch themePolicy {
		case "reuse":
			matchedTheme, err := findMatchingThemeByBytes(req.SourcePackage, req.TargetPackage, targetGraph, sourceMaster.ThemeURI)
			if err != nil {
				return nil, err
			}
			if matchedTheme != "" {
				importedThemeURI = matchedTheme
			} else {
				importedThemeURI, err = ctx.copyDependencyTree(sourceMaster.ThemeURI)
				if err != nil {
					return nil, err
				}
			}
		case "import":
			importedThemeURI, err = ctx.copyDependencyTree(sourceMaster.ThemeURI)
			if err != nil {
				return nil, err
			}
		}
	}

	newMasterURI, err := allocateNumberedPartName(req.TargetPackage, masterPartNamePattern, "/ppt/slideMasters/slideMaster%d.xml")
	if err != nil {
		return nil, err
	}
	newLayoutURI, err := allocateNumberedPartName(req.TargetPackage, layoutPartNamePattern, "/ppt/slideLayouts/slideLayout%d.xml")
	if err != nil {
		return nil, err
	}

	layoutData, err := req.SourcePackage.ReadRawPart(req.SourceLayoutURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source layout: %w", err)
	}
	if err := req.TargetPackage.AddPart(newLayoutURI, layoutData, contentTypeOrDefault(req.SourcePackage, req.SourceLayoutURI, layoutContentType), copyZipMeta(req.SourcePackage.GetZipMeta(req.SourceLayoutURI))); err != nil {
		return nil, fmt.Errorf("failed to add imported layout: %w", err)
	}

	layoutRels, err := buildImportedLayoutRelationships(ctx, req.SourceLayoutURI, newLayoutURI, newMasterURI)
	if err != nil {
		return nil, err
	}
	if len(layoutRels) > 0 {
		relsXML, err := BuildRelationshipsXML(layoutRels)
		if err != nil {
			return nil, fmt.Errorf("failed to build imported layout relationships: %w", err)
		}
		if err := req.TargetPackage.AddPart(relsURIForPart(newLayoutURI), relsXML, relationshipsContentType, copyZipMeta(req.SourcePackage.GetZipMeta(relsURIForPart(req.SourceLayoutURI)))); err != nil {
			return nil, fmt.Errorf("failed to add imported layout relationships: %w", err)
		}
	}

	newMasterData, selectedLayoutRelID, err := buildImportedSingleLayoutMaster(req.SourcePackage, sourceMaster.PartURI, req.SourceLayoutURI)
	if err != nil {
		return nil, err
	}
	if err := req.TargetPackage.AddPart(newMasterURI, newMasterData, contentTypeOrDefault(req.SourcePackage, sourceMaster.PartURI, masterContentType), copyZipMeta(req.SourcePackage.GetZipMeta(sourceMaster.PartURI))); err != nil {
		return nil, fmt.Errorf("failed to add imported master: %w", err)
	}

	masterRels, err := buildImportedMasterRelationshipsSubset(ctx, sourceMaster, newMasterURI, req.SourceLayoutURI, newLayoutURI, importedThemeURI)
	if err != nil {
		return nil, err
	}
	if len(masterRels) == 0 {
		return nil, fmt.Errorf("imported layout master %s has no relationships", sourceMaster.PartURI)
	}
	if !relationshipIDExists(masterRels, selectedLayoutRelID) {
		return nil, fmt.Errorf("selected layout relationship %s was not preserved on imported master", selectedLayoutRelID)
	}
	masterRelsXML, err := BuildRelationshipsXML(masterRels)
	if err != nil {
		return nil, fmt.Errorf("failed to build imported master relationships: %w", err)
	}
	if err := req.TargetPackage.AddPart(relsURIForPart(newMasterURI), masterRelsXML, relationshipsContentType, copyZipMeta(req.SourcePackage.GetZipMeta(relsURIForPart(sourceMaster.PartURI)))); err != nil {
		return nil, fmt.Errorf("failed to add imported master relationships: %w", err)
	}

	if _, err := registerImportedMaster(req.TargetPackage, newMasterURI); err != nil {
		return nil, err
	}

	return &ImportLayoutResult{
		SourceLayoutURI: req.SourceLayoutURI,
		TargetLayoutURI: newLayoutURI,
		TargetMasterURI: newMasterURI,
		ThemeURI:        importedThemeURI,
		Name:            sourceLayout.Name,
		Imported:        true,
		MasterImported:  true,
	}, nil
}

func buildImportedSingleLayoutMaster(sourcePackage opc.PackageSession, sourceMasterURI, selectedLayoutURI string) ([]byte, string, error) {
	masterDoc, err := sourcePackage.ReadXMLPart(sourceMasterURI)
	if err != nil {
		return nil, "", fmt.Errorf("failed to read source master: %w", err)
	}
	relID, layoutID, err := findMasterLayoutReference(sourcePackage, sourceMasterURI, selectedLayoutURI, masterDoc)
	if err != nil {
		return nil, "", err
	}
	if err := pruneMasterLayoutList(masterDoc.Root(), relID, layoutID); err != nil {
		return nil, "", err
	}
	masterData, err := masterDoc.WriteToBytes()
	if err != nil {
		return nil, "", fmt.Errorf("failed to serialize imported master: %w", err)
	}
	return masterData, relID, nil
}

func buildImportedMasterRelationshipsSubset(ctx *partImportContext, sourceMaster *inspect.MasterRef, newMasterURI, selectedSourceLayoutURI, newLayoutURI, importedThemeURI string) ([]opc.RelationshipInfo, error) {
	sourceRels := ctx.SourcePackage.ListRelationships(sourceMaster.PartURI)
	newRels := make([]opc.RelationshipInfo, 0, len(sourceRels))
	for _, rel := range sourceRels {
		if rel.TargetMode == "External" {
			newRels = append(newRels, opc.RelationshipInfo{
				SourceURI:  newMasterURI,
				ID:         rel.ID,
				Type:       rel.Type,
				Target:     rel.Target,
				TargetMode: rel.TargetMode,
			})
			continue
		}

		var targetURI string
		switch rel.Type {
		case slideLayoutRelationshipType:
			resolved := opc.ResolveRelationshipTarget(sourceMaster.PartURI, rel.Target)
			if resolved != selectedSourceLayoutURI {
				continue
			}
			targetURI = newLayoutURI
		case themeRelationshipType:
			if importedThemeURI == "" {
				return nil, fmt.Errorf("missing imported theme for master %s", sourceMaster.PartURI)
			}
			targetURI = importedThemeURI
		default:
			resolved := opc.ResolveRelationshipTarget(sourceMaster.PartURI, rel.Target)
			copied, err := ctx.copyDependencyTree(resolved)
			if err != nil {
				return nil, err
			}
			targetURI = copied
		}

		newTarget, err := relationshipTarget(newMasterURI, targetURI)
		if err != nil {
			return nil, err
		}
		newRels = append(newRels, opc.RelationshipInfo{
			SourceURI: newMasterURI,
			ID:        rel.ID,
			Type:      rel.Type,
			Target:    newTarget,
		})
	}
	return newRels, nil
}

func findMasterLayoutReference(sourcePackage opc.PackageSession, sourceMasterURI, selectedLayoutURI string, masterDoc *etree.Document) (string, uint32, error) {
	for _, rel := range sourcePackage.ListRelationships(sourceMasterURI) {
		if rel.Type != slideLayoutRelationshipType || rel.TargetMode == "External" {
			continue
		}
		if opc.ResolveRelationshipTarget(sourceMasterURI, rel.Target) != selectedLayoutURI {
			continue
		}
		layoutID, err := layoutIDForRelationship(masterDoc.Root(), rel.ID)
		if err != nil {
			return "", 0, err
		}
		return rel.ID, layoutID, nil
	}
	return "", 0, fmt.Errorf("source master %s does not reference layout %s", sourceMasterURI, selectedLayoutURI)
}

func pruneMasterLayoutList(masterRoot *etree.Element, relID string, layoutID uint32) error {
	if masterRoot == nil {
		return fmt.Errorf("master root element not found")
	}
	ensureNamespace(masterRoot, "r", namespaces.NsR)
	layoutIDList := masterRoot.FindElement(".//p:sldLayoutIdLst")
	if layoutIDList == nil {
		layoutIDList = masterRoot.FindElement(".//sldLayoutIdLst")
	}
	if layoutIDList == nil {
		layoutIDList = etree.NewElement("p:sldLayoutIdLst")
		masterRoot.AddChild(layoutIDList)
	}
	for _, child := range append([]*etree.Element{}, layoutIDList.ChildElements()...) {
		layoutIDList.RemoveChild(child)
	}
	layoutElem := etree.NewElement("p:sldLayoutId")
	layoutElem.CreateAttr("id", strconv.FormatUint(uint64(layoutID), 10))
	layoutElem.CreateAttr("r:id", relID)
	layoutIDList.AddChild(layoutElem)
	return nil
}

func layoutIDForRelationship(masterRoot *etree.Element, relID string) (uint32, error) {
	if masterRoot == nil {
		return 0, fmt.Errorf("master root element not found")
	}
	layoutIDList := masterRoot.FindElement(".//p:sldLayoutIdLst")
	if layoutIDList == nil {
		layoutIDList = masterRoot.FindElement(".//sldLayoutIdLst")
	}
	if layoutIDList == nil {
		return 0, fmt.Errorf("master layout list not found")
	}
	for _, child := range layoutIDList.ChildElements() {
		if xmlLocalNameForMasterImport(child.Tag) != "sldLayoutId" {
			continue
		}
		if child.SelectAttrValue("r:id", "") != relID {
			continue
		}
		parsed, err := strconv.ParseUint(child.SelectAttrValue("id", ""), 10, 32)
		if err != nil {
			return 0, fmt.Errorf("invalid master layout id for relationship %s: %w", relID, err)
		}
		return uint32(parsed), nil
	}
	return 0, fmt.Errorf("master layout relationship %s not found in master XML", relID)
}

func relationshipIDExists(rels []opc.RelationshipInfo, relID string) bool {
	for _, rel := range rels {
		if rel.ID == relID {
			return true
		}
	}
	return false
}

func xmlLocalNameForMasterImport(tag string) string {
	if tag == "" {
		return ""
	}
	if idx := strings.LastIndex(tag, "}"); idx >= 0 {
		return tag[idx+1:]
	}
	if idx := strings.Index(tag, ":"); idx >= 0 {
		return tag[idx+1:]
	}
	return tag
}
