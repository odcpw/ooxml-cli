package vba

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

const (
	ContentTypeVBAProject      = "application/vnd.ms-office.vbaProject"
	RelationshipTypeVBAProject = "http://schemas.microsoft.com/office/2006/relationships/vbaProject"
)

type FamilySpec struct {
	Family                   string
	PackageType              opc.PackageType
	DefaultMainPartURI       string
	DefaultVBAProjectPartURI string
	NonMacroMainContentType  string
	MacroMainContentType     string
	NonMacroExtension        string
	MacroExtension           string
}

var familySpecs = []FamilySpec{
	{
		Family:                   "pptx",
		PackageType:              opc.PackageTypePPTX,
		DefaultMainPartURI:       "/ppt/presentation.xml",
		DefaultVBAProjectPartURI: "/ppt/vbaProject.bin",
		NonMacroMainContentType:  "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
		MacroMainContentType:     "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml",
		NonMacroExtension:        ".pptx",
		MacroExtension:           ".pptm",
	},
	{
		Family:                   "xlsx",
		PackageType:              opc.PackageTypeXLSX,
		DefaultMainPartURI:       "/xl/workbook.xml",
		DefaultVBAProjectPartURI: "/xl/vbaProject.bin",
		NonMacroMainContentType:  "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
		MacroMainContentType:     "application/vnd.ms-excel.sheet.macroEnabled.main+xml",
		NonMacroExtension:        ".xlsx",
		MacroExtension:           ".xlsm",
	},
}

// SupportedFamilySpecs returns the OOXML families currently supported by the
// package-level VBA operations.
func SupportedFamilySpecs() []FamilySpec {
	specs := make([]FamilySpec, len(familySpecs))
	copy(specs, familySpecs)
	return specs
}

// Info describes package-level macro state without parsing the opaque VBA binary.
type Info struct {
	Family             string              `json:"family"`
	PackageType        string              `json:"packageType"`
	MacroEnabled       bool                `json:"macroEnabled"`
	HasVBAProject      bool                `json:"hasVbaProject"`
	MainPartURI        string              `json:"mainPartUri"`
	MainContentType    string              `json:"mainContentType"`
	VBAProject         *ProjectInfo        `json:"vbaProject,omitempty"`
	NonMacroExtension  string              `json:"nonMacroExtension"`
	MacroExtension     string              `json:"macroExtension"`
	SignatureArtifacts []SignatureArtifact `json:"signatureArtifacts,omitempty"`
	Warnings           []string            `json:"warnings,omitempty"`
}

// ProjectInfo describes the opaque vbaProject.bin part and its host relationship.
type ProjectInfo struct {
	PartURI            string `json:"partUri"`
	ContentType        string `json:"contentType"`
	Exists             bool   `json:"exists"`
	SizeBytes          int64  `json:"sizeBytes,omitempty"`
	SHA256             string `json:"sha256,omitempty"`
	RelationshipID     string `json:"relationshipId,omitempty"`
	RelationshipType   string `json:"relationshipType,omitempty"`
	RelationshipTarget string `json:"relationshipTarget,omitempty"`
}

// SignatureArtifact describes known package or VBA signature artifacts.
type SignatureArtifact struct {
	Kind           string `json:"kind"`
	PartURI        string `json:"partUri,omitempty"`
	SourceURI      string `json:"sourceUri,omitempty"`
	RelationshipID string `json:"relationshipId,omitempty"`
	Type           string `json:"type,omitempty"`
	Target         string `json:"target,omitempty"`
}

// MutationResult describes an opaque VBA package mutation.
type MutationResult struct {
	Action        string         `json:"action"`
	Family        string         `json:"family"`
	MainPartURI   string         `json:"mainPartUri"`
	VBAPartURI    string         `json:"vbaPartUri,omitempty"`
	MacroEnabled  bool           `json:"macroEnabled"`
	SourceProject *SourceProject `json:"sourceProject,omitempty"`
}

// AttachOptions controls opaque VBA attachment safety checks.
type AttachOptions struct {
	AllowHostFamilyRisk bool
}

// Inspect returns package-level VBA state for supported PPTX/PPTM and XLSX/XLSM packages.
func Inspect(session opc.PackageSession) (*Info, error) {
	spec, mainURI, err := detectFamily(session)
	if err != nil {
		return nil, err
	}

	info := &Info{
		Family:             spec.Family,
		PackageType:        spec.PackageType.String(),
		MainPartURI:        mainURI,
		MainContentType:    session.GetContentType(mainURI),
		NonMacroExtension:  spec.NonMacroExtension,
		MacroExtension:     spec.MacroExtension,
		SignatureArtifacts: FindSignatureArtifacts(session),
	}

	project := inspectProject(session, mainURI, spec)
	if project != nil {
		info.VBAProject = project
		info.HasVBAProject = project.Exists
		if !project.Exists {
			info.Warnings = append(info.Warnings, "VBA relationship points to a missing vbaProject.bin part")
		}
	}

	info.MacroEnabled = strings.EqualFold(info.MainContentType, spec.MacroMainContentType) || info.HasVBAProject || projectHasRelationship(project)
	if strings.EqualFold(info.MainContentType, spec.MacroMainContentType) && !info.HasVBAProject {
		info.Warnings = append(info.Warnings, "main part is macro-enabled but no VBA project part was found")
	}
	if !strings.EqualFold(info.MainContentType, spec.MacroMainContentType) && info.HasVBAProject {
		info.Warnings = append(info.Warnings, "VBA project exists but main content type is not macro-enabled")
	}
	if len(info.SignatureArtifacts) > 0 {
		info.Warnings = append(info.Warnings, "known signature artifacts are present; attach/remove refuses to mutate signed macro packages")
	}

	return info, nil
}

// ExtractBin reads the opaque vbaProject.bin bytes.
func ExtractBin(session opc.PackageSession) ([]byte, *Info, error) {
	info, err := Inspect(session)
	if err != nil {
		return nil, nil, err
	}
	if info.VBAProject == nil || !info.VBAProject.Exists {
		return nil, info, fmt.Errorf("package has no vbaProject.bin part")
	}
	data, err := session.ReadRawPart(info.VBAProject.PartURI)
	if err != nil {
		return nil, info, err
	}
	return data, info, nil
}

// Attach adds or replaces the opaque vbaProject.bin part and macro relationship.
func Attach(session opc.PackageSession, projectData []byte) (*MutationResult, error) {
	return AttachWithOptions(session, projectData, AttachOptions{})
}

// AttachWithOptions adds or replaces the opaque vbaProject.bin part and macro
// relationship, refusing parseable host-family mismatches unless explicitly
// allowed. Unparseable payloads remain opaque and are attached unchanged.
func AttachWithOptions(session opc.PackageSession, projectData []byte, opts AttachOptions) (*MutationResult, error) {
	if len(projectData) == 0 {
		return nil, fmt.Errorf("vbaProject.bin is empty")
	}
	info, err := Inspect(session)
	if err != nil {
		return nil, err
	}
	if len(info.SignatureArtifacts) > 0 {
		return nil, fmt.Errorf("refusing to attach VBA project because known signature artifacts are present")
	}

	spec, err := specByFamily(info.Family)
	if err != nil {
		return nil, err
	}
	sourceProject, err := ParseSourceProjectForFamily(projectData, spec.Family)
	if err == nil {
		if sourceProject.OfficeCompatibility != nil &&
			sourceProject.OfficeCompatibility.Status == "risk" &&
			!opts.AllowHostFamilyRisk {
			return nil, fmt.Errorf("VBA host-family risk refused for %s attachment: %s; rerun with --allow-host-family-risk only if you accept that Office may repair or reject the VBA project", spec.MacroExtension, strings.Join(sourceProjectHostWarningMessages(sourceProject), "; "))
		}
		sourceProject = SummarizeSourceProject(sourceProject)
	}
	targetPartURI := spec.DefaultVBAProjectPartURI
	if info.VBAProject != nil && info.VBAProject.PartURI != "" {
		targetPartURI = info.VBAProject.PartURI
	}

	mainData, err := session.ReadRawPart(info.MainPartURI)
	if err != nil {
		return nil, err
	}
	if err := session.ReplaceRawPart(info.MainPartURI, mainData, spec.MacroMainContentType); err != nil {
		return nil, err
	}
	if err := session.ReplaceRawPart(targetPartURI, projectData, ContentTypeVBAProject); err != nil {
		return nil, err
	}
	if err := upsertVBAProjectRelationship(session, info.MainPartURI, targetPartURI); err != nil {
		return nil, err
	}

	return &MutationResult{
		Action:        "attach",
		Family:        spec.Family,
		MainPartURI:   info.MainPartURI,
		VBAPartURI:    targetPartURI,
		MacroEnabled:  true,
		SourceProject: sourceProject,
	}, nil
}

func sourceProjectHostWarningMessages(project *SourceProject) []string {
	if project == nil || len(project.HostCompatibilityWarnings) == 0 {
		return []string{"host-family compatibility status is risk"}
	}
	messages := make([]string, 0, len(project.HostCompatibilityWarnings))
	for _, warning := range project.HostCompatibilityWarnings {
		if strings.TrimSpace(warning.Message) != "" {
			messages = append(messages, warning.Message)
		}
	}
	if len(messages) == 0 {
		return []string{"host-family compatibility status is risk"}
	}
	return messages
}

// Remove deletes opaque VBA package artifacts and restores the non-macro main content type.
func Remove(session opc.PackageSession) (*MutationResult, error) {
	info, err := Inspect(session)
	if err != nil {
		return nil, err
	}
	if len(info.SignatureArtifacts) > 0 {
		return nil, fmt.Errorf("refusing to remove VBA project because known signature artifacts are present")
	}

	spec, err := specByFamily(info.Family)
	if err != nil {
		return nil, err
	}
	mainData, err := session.ReadRawPart(info.MainPartURI)
	if err != nil {
		return nil, err
	}
	if err := session.ReplaceRawPart(info.MainPartURI, mainData, spec.NonMacroMainContentType); err != nil {
		return nil, err
	}

	removedPart := ""
	for _, uri := range candidateVBAProjectParts(session, spec, info) {
		if removedPart == "" {
			removedPart = uri
		}
		if err := session.RemovePart(uri); err != nil {
			return nil, err
		}
		_ = session.RemovePart(opc.RelsURIForPart(uri))
	}
	if err := removeVBAProjectRelationships(session, info.MainPartURI); err != nil {
		return nil, err
	}

	return &MutationResult{
		Action:       "remove",
		Family:       spec.Family,
		MainPartURI:  info.MainPartURI,
		VBAPartURI:   removedPart,
		MacroEnabled: false,
	}, nil
}

// FindSignatureArtifacts returns known VBA/package signature relationships and parts.
func FindSignatureArtifacts(session opc.PackageSession) []SignatureArtifact {
	var artifacts []SignatureArtifact
	seen := map[string]bool{}
	add := func(artifact SignatureArtifact) {
		key := artifact.Kind + "|" + artifact.PartURI + "|" + artifact.SourceURI + "|" + artifact.RelationshipID + "|" + artifact.Target
		if seen[key] {
			return
		}
		seen[key] = true
		artifacts = append(artifacts, artifact)
	}

	sources := []string{"/"}
	for _, part := range session.ListParts() {
		sources = append(sources, part.URI)
		lowerURI := strings.ToLower(part.URI)
		lowerCT := strings.ToLower(part.ContentType)
		if strings.Contains(lowerURI, "_xmlsignatures") ||
			strings.Contains(lowerURI, "vbaprojectsignature") ||
			strings.Contains(lowerCT, "digital-signature") ||
			strings.Contains(lowerCT, "vbaprojectsignature") {
			add(SignatureArtifact{Kind: "part", PartURI: part.URI})
		}
	}

	for _, sourceURI := range sources {
		for _, rel := range session.ListRelationships(sourceURI) {
			lowerType := strings.ToLower(rel.Type)
			if strings.Contains(lowerType, "digital-signature") || strings.Contains(lowerType, "vbaprojectsignature") {
				add(SignatureArtifact{
					Kind:           "relationship",
					SourceURI:      sourceURI,
					RelationshipID: rel.ID,
					Type:           rel.Type,
					Target:         rel.Target,
					PartURI:        opc.ResolveRelationshipTarget(sourceURI, rel.Target),
				})
			}
		}
	}

	return artifacts
}

func detectFamily(session opc.PackageSession) (*FamilySpec, string, error) {
	packageType := opc.DetectType(session)
	spec, err := specByPackageType(packageType)
	if err != nil {
		return nil, "", err
	}
	mainURI := findMainPartURI(session, spec)
	if mainURI == "" {
		return nil, "", fmt.Errorf("could not locate %s main part", spec.Family)
	}
	return spec, mainURI, nil
}

func specByPackageType(packageType opc.PackageType) (*FamilySpec, error) {
	for i := range familySpecs {
		if familySpecs[i].PackageType == packageType {
			return &familySpecs[i], nil
		}
	}
	return nil, fmt.Errorf("VBA package operations support PPTX/PPTM and XLSX/XLSM only (detected: %s)", packageType)
}

func specByFamily(family string) (*FamilySpec, error) {
	for i := range familySpecs {
		if familySpecs[i].Family == family {
			return &familySpecs[i], nil
		}
	}
	return nil, fmt.Errorf("unsupported VBA family: %s", family)
}

func findMainPartURI(session opc.PackageSession, spec *FamilySpec) string {
	for _, rel := range session.ListRelationships("/") {
		if rel.TargetMode == "External" {
			continue
		}
		targetURI := opc.ResolveRelationshipTarget("/", rel.Target)
		contentType := session.GetContentType(targetURI)
		if targetURI == spec.DefaultMainPartURI ||
			strings.EqualFold(contentType, spec.NonMacroMainContentType) ||
			strings.EqualFold(contentType, spec.MacroMainContentType) {
			return targetURI
		}
	}
	if partExists(session, spec.DefaultMainPartURI) {
		return spec.DefaultMainPartURI
	}
	return ""
}

func inspectProject(session opc.PackageSession, mainURI string, spec *FamilySpec) *ProjectInfo {
	var relInfo *opc.RelationshipInfo
	var partURI string
	for _, rel := range session.ListRelationships(mainURI) {
		targetURI := opc.ResolveRelationshipTarget(mainURI, rel.Target)
		if rel.Type == RelationshipTypeVBAProject || targetURI == spec.DefaultVBAProjectPartURI || strings.EqualFold(session.GetContentType(targetURI), ContentTypeVBAProject) {
			relCopy := rel
			relInfo = &relCopy
			partURI = targetURI
			break
		}
	}
	if partURI == "" {
		partURI = firstVBAProjectPart(session, spec.DefaultVBAProjectPartURI)
	}
	if partURI == "" {
		return nil
	}

	project := &ProjectInfo{
		PartURI:     partURI,
		ContentType: session.GetContentType(partURI),
		Exists:      partExists(session, partURI),
	}
	if relInfo != nil {
		project.RelationshipID = relInfo.ID
		project.RelationshipType = relInfo.Type
		project.RelationshipTarget = relInfo.Target
	}
	if project.Exists {
		for _, part := range session.ListParts() {
			if part.URI == partURI {
				project.SizeBytes = part.SizeBytes
				break
			}
		}
		if data, err := session.ReadRawPart(partURI); err == nil {
			sum := sha256.Sum256(data)
			project.SHA256 = hex.EncodeToString(sum[:])
		}
	}
	return project
}

func firstVBAProjectPart(session opc.PackageSession, defaultURI string) string {
	if partExists(session, defaultURI) {
		return defaultURI
	}
	for _, part := range session.ListParts() {
		if strings.EqualFold(part.ContentType, ContentTypeVBAProject) {
			return part.URI
		}
	}
	return ""
}

func candidateVBAProjectParts(session opc.PackageSession, spec *FamilySpec, info *Info) []string {
	candidates := []string{spec.DefaultVBAProjectPartURI}
	if info.VBAProject != nil && info.VBAProject.PartURI != "" {
		candidates = append(candidates, info.VBAProject.PartURI)
	}
	for _, part := range session.ListParts() {
		if strings.EqualFold(part.ContentType, ContentTypeVBAProject) {
			candidates = append(candidates, part.URI)
		}
	}

	seen := make(map[string]bool, len(candidates))
	result := make([]string, 0, len(candidates))
	for _, uri := range candidates {
		uri = opc.NormalizeURI(uri)
		if seen[uri] || !partExists(session, uri) {
			continue
		}
		seen[uri] = true
		result = append(result, uri)
	}
	return result
}

func upsertVBAProjectRelationship(session opc.PackageSession, mainURI, projectPartURI string) error {
	rels := session.ListRelationships(mainURI)
	target := opc.RelationshipTarget(mainURI, projectPartURI)
	updated := false
	for i := range rels {
		targetURI := opc.ResolveRelationshipTarget(mainURI, rels[i].Target)
		if rels[i].Type == RelationshipTypeVBAProject || targetURI == projectPartURI {
			rels[i].Type = RelationshipTypeVBAProject
			rels[i].Target = target
			rels[i].TargetMode = ""
			updated = true
		}
	}
	if !updated {
		rels = append(rels, opc.RelationshipInfo{
			SourceURI: mainURI,
			ID:        opc.AllocateRelationshipID(rels),
			Type:      RelationshipTypeVBAProject,
			Target:    target,
		})
	}
	return opc.WriteRelationships(session, mainURI, rels)
}

func removeVBAProjectRelationships(session opc.PackageSession, mainURI string) error {
	rels := session.ListRelationships(mainURI)
	filtered := make([]opc.RelationshipInfo, 0, len(rels))
	for _, rel := range rels {
		targetURI := opc.ResolveRelationshipTarget(mainURI, rel.Target)
		if rel.Type == RelationshipTypeVBAProject || strings.EqualFold(session.GetContentType(targetURI), ContentTypeVBAProject) {
			continue
		}
		filtered = append(filtered, rel)
	}
	return opc.WriteRelationships(session, mainURI, filtered)
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

func projectHasRelationship(project *ProjectInfo) bool {
	return project != nil && project.RelationshipID != ""
}
