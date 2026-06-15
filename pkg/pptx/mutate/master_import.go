package mutate

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"path/filepath"
	"sort"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

const masterContentType = "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"
const themeContentType = "application/vnd.openxmlformats-officedocument.presentationml.theme+xml"

// ImportMasterRequest describes importing a source slide master and its linked layouts.
type ImportMasterRequest struct {
	TargetPackage   opc.PackageSession
	SourcePackage   opc.PackageSession
	SourceMasterURI string
	ThemePolicy     string
}

// ImportedLayoutMapping reports a source->target imported layout mapping.
type ImportedLayoutMapping struct {
	SourceLayoutURI string
	TargetLayoutURI string
	Name            string
}

// ImportMasterResult describes the imported or reused master chain.
type ImportMasterResult struct {
	SourceMasterURI string
	TargetMasterURI string
	ThemeURI        string
	MasterID        uint32
	RelationshipID  string
	Imported        bool
	Layouts         []ImportedLayoutMapping
}

type partImportContext struct {
	SourcePackage opc.PackageSession
	TargetPackage opc.PackageSession
	Imported      map[string]string
}

// ImportMaster imports a source slide master, its linked layouts, and their dependent parts.
// Existing target masters are reused only when the source and target master chains are exact-compatible.
func ImportMaster(req *ImportMasterRequest) (*ImportMasterResult, error) {
	if req == nil {
		return nil, fmt.Errorf("request cannot be nil")
	}
	if req.SourcePackage == nil || req.TargetPackage == nil {
		return nil, fmt.Errorf("source and target packages are required")
	}
	if req.SourceMasterURI == "" {
		return nil, fmt.Errorf("source master URI cannot be empty")
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

	sourceMaster := findMasterRefByURI(sourceGraph.Masters, req.SourceMasterURI)
	if sourceMaster == nil {
		return nil, fmt.Errorf("source master %s not found", req.SourceMasterURI)
	}

	if reused, err := findCompatibleTargetMasterExact(req.SourcePackage, req.TargetPackage, sourceGraph, targetGraph, sourceMaster); err != nil {
		return nil, err
	} else if reused != nil {
		return reused, nil
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
			matched, err := findMatchingThemeByBytes(req.SourcePackage, req.TargetPackage, targetGraph, sourceMaster.ThemeURI)
			if err != nil {
				return nil, err
			}
			if matched != "" {
				importedThemeURI = matched
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

	layoutMap, importedLayouts, err := importMasterLayouts(ctx, sourceMaster, newMasterURI, sourceGraph)
	if err != nil {
		return nil, err
	}

	masterData, err := req.SourcePackage.ReadRawPart(req.SourceMasterURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read source master: %w", err)
	}
	if err := req.TargetPackage.AddPart(newMasterURI, masterData, contentTypeOrDefault(req.SourcePackage, req.SourceMasterURI, masterContentType), copyZipMeta(req.SourcePackage.GetZipMeta(req.SourceMasterURI))); err != nil {
		return nil, fmt.Errorf("failed to add imported master: %w", err)
	}

	masterRels, err := buildImportedMasterRelationships(ctx, sourceMaster, newMasterURI, layoutMap, importedThemeURI)
	if err != nil {
		return nil, err
	}
	if len(masterRels) > 0 {
		relsXML, err := BuildRelationshipsXML(masterRels)
		if err != nil {
			return nil, fmt.Errorf("failed to build imported master relationships: %w", err)
		}
		if err := req.TargetPackage.AddPart(relsURIForPart(newMasterURI), relsXML, relationshipsContentType, copyZipMeta(req.SourcePackage.GetZipMeta(relsURIForPart(req.SourceMasterURI)))); err != nil {
			return nil, fmt.Errorf("failed to add imported master relationships: %w", err)
		}
	}

	registration, err := registerImportedMaster(req.TargetPackage, newMasterURI)
	if err != nil {
		return nil, err
	}

	return &ImportMasterResult{
		SourceMasterURI: req.SourceMasterURI,
		TargetMasterURI: newMasterURI,
		ThemeURI:        importedThemeURI,
		MasterID:        registration.MasterID,
		RelationshipID:  registration.RelationshipID,
		Imported:        true,
		Layouts:         importedLayouts,
	}, nil
}

type masterRegistrationResult struct {
	MasterID       uint32
	RelationshipID string
}

func registerImportedMaster(targetPackage opc.PackageSession, masterURI string) (*masterRegistrationResult, error) {
	presentationDoc, err := targetPackage.ReadXMLPart("/ppt/presentation.xml")
	if err != nil {
		return nil, fmt.Errorf("failed to read presentation.xml: %w", err)
	}
	presentationRels := targetPackage.ListRelationships("/ppt/presentation.xml")

	root := presentationDoc.Root()
	if root == nil {
		return nil, fmt.Errorf("presentation.xml root element not found")
	}
	ensureNamespace(root, "r", namespaces.NsR)

	masterIDList := namespaces.FindChild(root, namespaces.NsP, "sldMasterIdLst")
	if masterIDList == nil {
		masterIDList = etree.NewElement("p:sldMasterIdLst")
		if slideIDList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst"); slideIDList != nil {
			root.InsertChildAt(slideIDList.Index(), masterIDList)
		} else {
			root.AddChild(masterIDList)
		}
	}

	newRelID := AllocateRelationshipID(presentationRels)
	masterID, err := allocateMasterID(presentationDoc)
	if err != nil {
		return nil, err
	}
	masterTarget, err := relationshipTarget("/ppt/presentation.xml", masterURI)
	if err != nil {
		return nil, err
	}

	masterIDElem := etree.NewElement("p:sldMasterId")
	masterIDElem.CreateAttr("id", strconv.FormatUint(uint64(masterID), 10))
	masterIDElem.CreateAttr("r:id", newRelID)
	masterIDList.AddChild(masterIDElem)

	if err := targetPackage.ReplaceXMLPart("/ppt/presentation.xml", presentationDoc); err != nil {
		return nil, fmt.Errorf("failed to update presentation.xml: %w", err)
	}

	presentationRels = append(presentationRels, opc.RelationshipInfo{
		SourceURI: "/ppt/presentation.xml",
		ID:        newRelID,
		Type:      slideMasterRelationshipType,
		Target:    masterTarget,
	})
	presentationRelsXML, err := BuildRelationshipsXML(presentationRels)
	if err != nil {
		return nil, err
	}
	if err := targetPackage.ReplaceRawPart("/ppt/_rels/presentation.xml.rels", presentationRelsXML, relationshipsContentType); err != nil {
		return nil, fmt.Errorf("failed to update presentation.xml.rels: %w", err)
	}

	return &masterRegistrationResult{MasterID: masterID, RelationshipID: newRelID}, nil
}

func allocateMasterID(presentationDoc *etree.Document) (uint32, error) {
	if presentationDoc == nil || presentationDoc.Root() == nil {
		return 0, fmt.Errorf("presentation.xml root element not found")
	}
	const base uint32 = 2147483648
	root := presentationDoc.Root()
	masterIDList := namespaces.FindChild(root, namespaces.NsP, "sldMasterIdLst")
	if masterIDList == nil {
		return base, nil
	}
	maxID := uint32(base - 1)
	for _, masterID := range namespaces.FindChildren(masterIDList, namespaces.NsP, "sldMasterId") {
		parsed, err := strconv.ParseUint(masterID.SelectAttrValue("id", ""), 10, 32)
		if err != nil {
			continue
		}
		if uint32(parsed) > maxID {
			maxID = uint32(parsed)
		}
	}
	if maxID < base {
		return base, nil
	}
	return maxID + 1, nil
}

func importMasterLayouts(ctx *partImportContext, sourceMaster *inspect.MasterRef, newMasterURI string, sourceGraph *inspect.PresentationGraph) (map[string]string, []ImportedLayoutMapping, error) {
	layoutMap := make(map[string]string, len(sourceMaster.LinkedLayoutURIs))
	imported := make([]ImportedLayoutMapping, 0, len(sourceMaster.LinkedLayoutURIs))
	for _, sourceLayoutURI := range sourceMaster.LinkedLayoutURIs {
		newLayoutURI, err := allocateNumberedPartName(ctx.TargetPackage, layoutPartNamePattern, "/ppt/slideLayouts/slideLayout%d.xml")
		if err != nil {
			return nil, nil, err
		}
		layoutMap[sourceLayoutURI] = newLayoutURI

		layoutData, err := ctx.SourcePackage.ReadRawPart(sourceLayoutURI)
		if err != nil {
			return nil, nil, fmt.Errorf("failed to read source layout %s: %w", sourceLayoutURI, err)
		}
		if err := ctx.TargetPackage.AddPart(newLayoutURI, layoutData, contentTypeOrDefault(ctx.SourcePackage, sourceLayoutURI, layoutContentType), copyZipMeta(ctx.SourcePackage.GetZipMeta(sourceLayoutURI))); err != nil {
			return nil, nil, fmt.Errorf("failed to add imported layout %s: %w", sourceLayoutURI, err)
		}

		layoutRels, err := buildImportedLayoutRelationships(ctx, sourceLayoutURI, newLayoutURI, newMasterURI)
		if err != nil {
			return nil, nil, err
		}
		if len(layoutRels) > 0 {
			relsXML, err := BuildRelationshipsXML(layoutRels)
			if err != nil {
				return nil, nil, fmt.Errorf("failed to build imported layout relationships: %w", err)
			}
			if err := ctx.TargetPackage.AddPart(relsURIForPart(newLayoutURI), relsXML, relationshipsContentType, copyZipMeta(ctx.SourcePackage.GetZipMeta(relsURIForPart(sourceLayoutURI)))); err != nil {
				return nil, nil, fmt.Errorf("failed to add imported layout relationships: %w", err)
			}
		}

		layoutRef := findLayoutRefByURI(sourceGraph.Layouts, sourceLayoutURI)
		layoutName := ""
		if layoutRef != nil {
			layoutName = layoutRef.Name
		}
		imported = append(imported, ImportedLayoutMapping{
			SourceLayoutURI: sourceLayoutURI,
			TargetLayoutURI: newLayoutURI,
			Name:            layoutName,
		})
	}
	return layoutMap, imported, nil
}

func buildImportedLayoutRelationships(ctx *partImportContext, sourceLayoutURI, newLayoutURI, newMasterURI string) ([]opc.RelationshipInfo, error) {
	sourceRels := ctx.SourcePackage.ListRelationships(sourceLayoutURI)
	newRels := make([]opc.RelationshipInfo, 0, len(sourceRels))
	for _, rel := range sourceRels {
		if rel.TargetMode == "External" {
			newRels = append(newRels, opc.RelationshipInfo{
				SourceURI:  newLayoutURI,
				ID:         rel.ID,
				Type:       rel.Type,
				Target:     rel.Target,
				TargetMode: rel.TargetMode,
			})
			continue
		}

		var targetURI string
		switch rel.Type {
		case slideMasterRelationshipType:
			targetURI = newMasterURI
		default:
			resolved := opc.ResolveRelationshipTarget(sourceLayoutURI, rel.Target)
			copied, err := ctx.copyDependencyTree(resolved)
			if err != nil {
				return nil, err
			}
			targetURI = copied
		}

		newTarget, err := relationshipTarget(newLayoutURI, targetURI)
		if err != nil {
			return nil, err
		}
		newRels = append(newRels, opc.RelationshipInfo{
			SourceURI: newLayoutURI,
			ID:        rel.ID,
			Type:      rel.Type,
			Target:    newTarget,
		})
	}
	return newRels, nil
}

func buildImportedMasterRelationships(ctx *partImportContext, sourceMaster *inspect.MasterRef, newMasterURI string, layoutMap map[string]string, importedThemeURI string) ([]opc.RelationshipInfo, error) {
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
			mapped := layoutMap[resolved]
			if mapped == "" {
				return nil, fmt.Errorf("missing imported layout mapping for %s", resolved)
			}
			targetURI = mapped
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

func (ctx *partImportContext) copyDependencyTree(sourceURI string) (string, error) {
	sourceURI = opc.NormalizeURI(sourceURI)
	if sourceURI == "" {
		return "", fmt.Errorf("source URI cannot be empty")
	}
	if existing := ctx.Imported[sourceURI]; existing != "" {
		return existing, nil
	}
	if reused, ok, err := findExactPartMatch(ctx.SourcePackage, ctx.TargetPackage, sourceURI); err != nil {
		return "", err
	} else if ok {
		ctx.Imported[sourceURI] = reused
		return reused, nil
	}

	data, err := ctx.SourcePackage.ReadRawPart(sourceURI)
	if err != nil {
		return "", fmt.Errorf("failed to read dependent part %s: %w", sourceURI, err)
	}
	contentType := contentTypeOrDefault(ctx.SourcePackage, sourceURI, "application/octet-stream")
	newURI, err := allocateImportedPartName(ctx.TargetPackage, sourceURI)
	if err != nil {
		return "", err
	}
	ctx.Imported[sourceURI] = newURI
	if err := ctx.TargetPackage.AddPart(newURI, data, contentType, copyZipMeta(ctx.SourcePackage.GetZipMeta(sourceURI))); err != nil {
		return "", fmt.Errorf("failed to add dependent part %s: %w", sourceURI, err)
	}

	sourceRels := ctx.SourcePackage.ListRelationships(sourceURI)
	if len(sourceRels) == 0 {
		return newURI, nil
	}

	newRels := make([]opc.RelationshipInfo, 0, len(sourceRels))
	for _, rel := range sourceRels {
		if rel.TargetMode == "External" {
			newRels = append(newRels, opc.RelationshipInfo{
				SourceURI:  newURI,
				ID:         rel.ID,
				Type:       rel.Type,
				Target:     rel.Target,
				TargetMode: rel.TargetMode,
			})
			continue
		}
		resolved := opc.ResolveRelationshipTarget(sourceURI, rel.Target)
		copiedTarget, err := ctx.copyDependencyTree(resolved)
		if err != nil {
			return "", err
		}
		newTarget, err := relationshipTarget(newURI, copiedTarget)
		if err != nil {
			return "", err
		}
		newRels = append(newRels, opc.RelationshipInfo{
			SourceURI: newURI,
			ID:        rel.ID,
			Type:      rel.Type,
			Target:    newTarget,
		})
	}

	relsXML, err := BuildRelationshipsXML(newRels)
	if err != nil {
		return "", err
	}
	if err := ctx.TargetPackage.AddPart(relsURIForPart(newURI), relsXML, relationshipsContentType, copyZipMeta(ctx.SourcePackage.GetZipMeta(relsURIForPart(sourceURI)))); err != nil {
		return "", fmt.Errorf("failed to add dependent relationships for %s: %w", sourceURI, err)
	}
	return newURI, nil
}

func findExactPartMatch(sourcePackage, targetPackage opc.PackageSession, sourceURI string) (string, bool, error) {
	contentType := sourcePackage.GetContentType(sourceURI)
	sourceData, err := sourcePackage.ReadRawPart(sourceURI)
	if err != nil {
		return "", false, err
	}
	for _, part := range targetPackage.ListParts() {
		if contentType != "" && targetPackage.GetContentType(part.URI) != contentType {
			continue
		}
		targetData, err := targetPackage.ReadRawPart(part.URI)
		if err != nil {
			continue
		}
		if bytes.Equal(sourceData, targetData) {
			return part.URI, true, nil
		}
	}
	return "", false, nil
}

func findMatchingThemeByBytes(sourcePackage, targetPackage opc.PackageSession, targetGraph *inspect.PresentationGraph, sourceThemeURI string) (string, error) {
	sourceData, err := sourcePackage.ReadRawPart(sourceThemeURI)
	if err != nil {
		return "", fmt.Errorf("failed to read source theme: %w", err)
	}
	seen := make(map[string]struct{})
	for _, master := range targetGraph.Masters {
		if master.ThemeURI == "" {
			continue
		}
		if _, ok := seen[master.ThemeURI]; ok {
			continue
		}
		seen[master.ThemeURI] = struct{}{}
		targetData, err := targetPackage.ReadRawPart(master.ThemeURI)
		if err != nil {
			continue
		}
		if bytes.Equal(sourceData, targetData) {
			return master.ThemeURI, nil
		}
	}
	return "", nil
}

func findCompatibleTargetMasterExact(sourcePackage, targetPackage opc.PackageSession, sourceGraph, targetGraph *inspect.PresentationGraph, sourceMaster *inspect.MasterRef) (*ImportMasterResult, error) {
	sourceMasterFingerprint, err := partFingerprint(sourcePackage, sourceMaster.PartURI, map[string]bool{
		slideLayoutRelationshipType: true,
		themeRelationshipType:       true,
	})
	if err != nil {
		return nil, err
	}
	sourceThemeData, err := maybeReadPart(sourcePackage, sourceMaster.ThemeURI)
	if err != nil {
		return nil, err
	}
	sourceLayouts, err := layoutFingerprintMap(sourcePackage, sourceMaster.LinkedLayoutURIs)
	if err != nil {
		return nil, err
	}

	for _, targetMaster := range targetGraph.Masters {
		targetMasterFingerprint, err := partFingerprint(targetPackage, targetMaster.PartURI, map[string]bool{
			slideLayoutRelationshipType: true,
			themeRelationshipType:       true,
		})
		if err != nil || sourceMasterFingerprint != targetMasterFingerprint {
			continue
		}
		targetThemeData, err := maybeReadPart(targetPackage, targetMaster.ThemeURI)
		if err != nil {
			return nil, err
		}
		if !bytes.Equal(sourceThemeData, targetThemeData) {
			continue
		}
		targetLayouts, err := layoutFingerprintMap(targetPackage, targetMaster.LinkedLayoutURIs)
		if err != nil {
			return nil, err
		}
		layoutMappings, ok := matchExactLayouts(sourceGraph.Layouts, sourceMaster.LinkedLayoutURIs, sourceLayouts, targetGraph.Layouts, targetMaster.LinkedLayoutURIs, targetLayouts)
		if !ok {
			continue
		}
		return &ImportMasterResult{
			SourceMasterURI: sourceMaster.PartURI,
			TargetMasterURI: targetMaster.PartURI,
			ThemeURI:        targetMaster.ThemeURI,
			Imported:        false,
			Layouts:         layoutMappings,
		}, nil
	}
	return nil, nil
}

func matchExactLayouts(sourceLayoutRefs []inspect.LayoutRef, sourceLayoutURIs []string, sourceLayouts map[string]string, targetLayoutRefs []inspect.LayoutRef, targetLayoutURIs []string, targetLayouts map[string]string) ([]ImportedLayoutMapping, bool) {
	if len(sourceLayoutURIs) != len(targetLayoutURIs) {
		return nil, false
	}
	used := make(map[string]struct{}, len(targetLayoutURIs))
	mappings := make([]ImportedLayoutMapping, 0, len(sourceLayoutURIs))
	for _, sourceLayoutURI := range sourceLayoutURIs {
		sourceFingerprint := sourceLayouts[sourceLayoutURI]
		matchedTargetURI := ""
		for _, targetLayoutURI := range targetLayoutURIs {
			if _, seen := used[targetLayoutURI]; seen {
				continue
			}
			if sourceFingerprint == targetLayouts[targetLayoutURI] {
				matchedTargetURI = targetLayoutURI
				used[targetLayoutURI] = struct{}{}
				break
			}
		}
		if matchedTargetURI == "" {
			return nil, false
		}
		layoutRef := findLayoutRefByURI(sourceLayoutRefs, sourceLayoutURI)
		mappings = append(mappings, ImportedLayoutMapping{
			SourceLayoutURI: sourceLayoutURI,
			TargetLayoutURI: matchedTargetURI,
			Name:            firstLayoutName(layoutRef),
		})
	}
	sort.Slice(mappings, func(i, j int) bool {
		return mappings[i].SourceLayoutURI < mappings[j].SourceLayoutURI
	})
	return mappings, true
}

func firstLayoutName(layoutRef *inspect.LayoutRef) string {
	if layoutRef == nil {
		return ""
	}
	return layoutRef.Name
}

func layoutFingerprintMap(pkg opc.PackageSession, layoutURIs []string) (map[string]string, error) {
	out := make(map[string]string, len(layoutURIs))
	for _, layoutURI := range layoutURIs {
		fingerprint, err := partFingerprint(pkg, layoutURI, map[string]bool{slideMasterRelationshipType: true})
		if err != nil {
			return nil, fmt.Errorf("failed to fingerprint layout %s: %w", layoutURI, err)
		}
		out[layoutURI] = fingerprint
	}
	return out, nil
}

func maybeReadPart(pkg opc.PackageSession, uri string) ([]byte, error) {
	if strings.TrimSpace(uri) == "" {
		return nil, nil
	}
	data, err := pkg.ReadRawPart(uri)
	if err != nil {
		return nil, fmt.Errorf("failed to read %s: %w", uri, err)
	}
	return data, nil
}

func partFingerprint(pkg opc.PackageSession, partURI string, ignoreTypes map[string]bool) (string, error) {
	memo := map[string]string{}
	return partFingerprintWithMemo(pkg, opc.NormalizeURI(partURI), ignoreTypes, memo)
}

func partFingerprintWithMemo(pkg opc.PackageSession, partURI string, ignoreTypes map[string]bool, memo map[string]string) (string, error) {
	if cached, ok := memo[partURI]; ok {
		return cached, nil
	}
	data, err := pkg.ReadRawPart(partURI)
	if err != nil {
		return "", fmt.Errorf("failed to read %s: %w", partURI, err)
	}
	rels := pkg.ListRelationships(partURI)
	relFingerprints := make([]string, 0, len(rels))
	for _, rel := range rels {
		if ignoreTypes != nil && ignoreTypes[rel.Type] {
			continue
		}
		if rel.TargetMode == "External" {
			relFingerprints = append(relFingerprints, fmt.Sprintf("external|%s|%s|%s", rel.Type, rel.TargetMode, rel.Target))
			continue
		}
		resolved := opc.ResolveRelationshipTarget(partURI, rel.Target)
		depFingerprint, err := partFingerprintWithMemo(pkg, resolved, nil, memo)
		if err != nil {
			return "", err
		}
		relFingerprints = append(relFingerprints, fmt.Sprintf("internal|%s|%s", rel.Type, depFingerprint))
	}
	sort.Strings(relFingerprints)

	hasher := sha256.New()
	hasher.Write(data)
	hasher.Write([]byte("\x00"))
	hasher.Write([]byte(pkg.GetContentType(partURI)))
	for _, relFingerprint := range relFingerprints {
		hasher.Write([]byte("\x00"))
		hasher.Write([]byte(relFingerprint))
	}
	fingerprint := hex.EncodeToString(hasher.Sum(nil))
	memo[partURI] = fingerprint
	return fingerprint, nil
}

func resolveSlideLayoutTarget(sourcePackage, targetPackage opc.PackageSession, sourceGraph, targetGraph *inspect.PresentationGraph, sourceLayoutURI, layoutPolicy, themePolicy string) (string, error) {
	sourceLayoutURI = opc.NormalizeURI(sourceLayoutURI)
	if sourceLayoutURI == "" {
		return "", fmt.Errorf("source slide is missing a layout relationship")
	}

	switch layoutPolicy {
	case "reuse":
		matched, err := findCompatibleTargetLayoutExact(sourcePackage, targetPackage, sourceGraph, targetGraph, sourceLayoutURI)
		if err != nil {
			return "", err
		}
		if matched == "" {
			return "", fmt.Errorf("layout-policy reuse requires an explicit compatible target layout; no exact match found for %s", sourceLayoutURI)
		}
		return matched, nil
	case "import":
		sourceLayout := findLayoutRefByURI(sourceGraph.Layouts, sourceLayoutURI)
		if sourceLayout == nil {
			return "", fmt.Errorf("source layout %s not found", sourceLayoutURI)
		}
		masterResult, err := ImportMaster(&ImportMasterRequest{
			TargetPackage:   targetPackage,
			SourcePackage:   sourcePackage,
			SourceMasterURI: sourceLayout.MasterPartURI,
			ThemePolicy:     themePolicy,
		})
		if err != nil {
			return "", err
		}
		for _, layout := range masterResult.Layouts {
			if layout.SourceLayoutURI == sourceLayoutURI {
				return layout.TargetLayoutURI, nil
			}
		}
		return "", fmt.Errorf("imported master %s did not provide layout %s", masterResult.TargetMasterURI, sourceLayoutURI)
	default:
		return "", fmt.Errorf("unknown layout policy: %s", layoutPolicy)
	}
}

func findCompatibleTargetLayoutExact(sourcePackage, targetPackage opc.PackageSession, sourceGraph, targetGraph *inspect.PresentationGraph, sourceLayoutURI string) (string, error) {
	sourceLayout := findLayoutRefByURI(sourceGraph.Layouts, sourceLayoutURI)
	if sourceLayout == nil {
		return "", fmt.Errorf("source layout %s not found", sourceLayoutURI)
	}
	sourceLayoutFingerprint, err := partFingerprint(sourcePackage, sourceLayoutURI, map[string]bool{slideMasterRelationshipType: true})
	if err != nil {
		return "", err
	}
	sourceThemeData, err := themeBytesForLayout(sourcePackage, sourceGraph, sourceLayoutURI)
	if err != nil {
		return "", err
	}

	for _, targetLayout := range targetGraph.Layouts {
		targetLayoutFingerprint, err := partFingerprint(targetPackage, targetLayout.PartURI, map[string]bool{slideMasterRelationshipType: true})
		if err != nil {
			continue
		}
		if sourceLayoutFingerprint != targetLayoutFingerprint {
			continue
		}
		targetThemeData, err := themeBytesForLayout(targetPackage, targetGraph, targetLayout.PartURI)
		if err != nil {
			return "", err
		}
		if bytes.Equal(sourceThemeData, targetThemeData) {
			return targetLayout.PartURI, nil
		}
	}
	return "", nil
}

func themeBytesForLayout(pkg opc.PackageSession, graph *inspect.PresentationGraph, layoutURI string) ([]byte, error) {
	layout := findLayoutRefByURI(graph.Layouts, layoutURI)
	if layout == nil {
		return nil, fmt.Errorf("layout %s not found", layoutURI)
	}
	master := findMasterRefByURI(graph.Masters, layout.MasterPartURI)
	if master == nil || strings.TrimSpace(master.ThemeURI) == "" {
		return nil, nil
	}
	return maybeReadPart(pkg, master.ThemeURI)
}

func findMasterRefByURI(masters []inspect.MasterRef, uri string) *inspect.MasterRef {
	for i := range masters {
		if masters[i].PartURI == uri {
			return &masters[i]
		}
	}
	return nil
}

func findLayoutRefByURI(layouts []inspect.LayoutRef, uri string) *inspect.LayoutRef {
	for i := range layouts {
		if layouts[i].PartURI == uri {
			return &layouts[i]
		}
	}
	return nil
}

func allocateImportedPartName(session opc.PackageSession, sourceURI string) (string, error) {
	sourceURI = opc.NormalizeURI(sourceURI)
	switch {
	case layoutPartNamePattern.MatchString(sourceURI):
		return allocateNumberedPartName(session, layoutPartNamePattern, "/ppt/slideLayouts/slideLayout%d.xml")
	case masterPartNamePattern.MatchString(sourceURI):
		return allocateNumberedPartName(session, masterPartNamePattern, "/ppt/slideMasters/slideMaster%d.xml")
	case themePartNamePattern.MatchString(sourceURI):
		return allocateNumberedPartName(session, themePartNamePattern, "/ppt/theme/theme%d.xml")
	case strings.HasPrefix(sourceURI, "/ppt/media/"):
		return allocateMediaPartNameLikeSource(session, sourceURI)
	default:
		if !partExists(session, sourceURI) {
			return sourceURI, nil
		}
		return allocateSiblingPartName(session, sourceURI)
	}
}

func partExists(session opc.PackageSession, uri string) bool {
	uri = opc.NormalizeURI(uri)
	for _, part := range session.ListParts() {
		if part.URI == uri {
			return true
		}
	}
	return false
}

func allocateSiblingPartName(session opc.PackageSession, sourceURI string) (string, error) {
	sourceURI = opc.NormalizeURI(sourceURI)
	dir := opc.GetDirectory(sourceURI)
	base := filepath.Base(sourceURI)
	ext := filepath.Ext(base)
	stem := strings.TrimSuffix(base, ext)
	for i := 1; i < 10000; i++ {
		candidate := opc.NormalizeURI(filepath.ToSlash(filepath.Join(dir, fmt.Sprintf("%s-import%d%s", stem, i, ext))))
		if !partExists(session, candidate) {
			return candidate, nil
		}
	}
	return "", fmt.Errorf("unable to allocate imported part name for %s", sourceURI)
}
