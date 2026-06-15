package validate

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/vba"
)

type vbaRelationshipFinding struct {
	sourceURI string
	rel       opc.RelationshipInfo
	targetURI string
}

func validateVBAPackageConsistency(session opc.PackageSession) ([]result.Diagnostic, error) {
	spec, mainURI := detectVBAValidationFamily(session)
	if spec == nil {
		return nil, nil
	}

	parts := session.ListParts()
	partMap := make(map[string]opc.PartInfo, len(parts))
	for _, part := range parts {
		partMap[part.URI] = part
	}

	var diags []result.Diagnostic
	mainContentType := session.GetContentType(mainURI)

	vbaParts := findVBAProjectPartCandidates(parts, spec)
	vbaRels := findVBARelationships(session, parts, spec)
	mainVBAProjectRels := filterVBARelationshipsFromSource(vbaRels, mainURI, true)
	validMainTargets := map[string]bool{}
	reportedWrongContentType := map[string]bool{}
	reportedEmptyProject := map[string]bool{}

	for _, finding := range vbaRels {
		if finding.rel.TargetMode == "External" {
			diags = append(diags, diag.Error(
				"VBA_REL_EXTERNAL_TARGET",
				fmt.Sprintf("VBA project relationship from %s (id=%s) targets an external resource", finding.sourceURI, finding.rel.ID),
			))
			continue
		}

		if finding.rel.Type != vba.RelationshipTypeVBAProject {
			diags = append(diags, diag.Error(
				"VBA_REL_WRONG_TYPE",
				fmt.Sprintf("relationship from %s (id=%s) points to VBA project candidate %s but uses type %s", finding.sourceURI, finding.rel.ID, finding.targetURI, finding.rel.Type),
			))
		}

		if finding.rel.Type == vba.RelationshipTypeVBAProject && finding.sourceURI != mainURI {
			diags = append(diags, diag.Error(
				"VBA_REL_WRONG_SOURCE",
				fmt.Sprintf("VBA project relationship must come from %s, found source %s (id=%s)", mainURI, finding.sourceURI, finding.rel.ID),
			))
		}

		targetPart, exists := partMap[finding.targetURI]
		if !exists {
			diags = append(diags, diag.Error(
				"VBA_REL_MISSING_TARGET",
				fmt.Sprintf("VBA project relationship from %s (id=%s) points to missing part %s", finding.sourceURI, finding.rel.ID, finding.targetURI),
			))
			continue
		}

		if !strings.EqualFold(targetPart.ContentType, vba.ContentTypeVBAProject) && !reportedWrongContentType[finding.targetURI] {
			diags = append(diags, diag.Error(
				"VBA_PART_WRONG_CONTENT_TYPE",
				fmt.Sprintf("VBA project part %s has content type %q, want %q", finding.targetURI, targetPart.ContentType, vba.ContentTypeVBAProject),
			))
			reportedWrongContentType[finding.targetURI] = true
		}
		if targetPart.SizeBytes == 0 && !reportedEmptyProject[finding.targetURI] {
			diags = append(diags, diag.Error(
				"VBA_PROJECT_EMPTY",
				fmt.Sprintf("VBA project part %s is empty", finding.targetURI),
			))
			reportedEmptyProject[finding.targetURI] = true
		}

		if finding.sourceURI == mainURI && finding.rel.Type == vba.RelationshipTypeVBAProject {
			validMainTargets[finding.targetURI] = true
		}
	}

	if len(mainVBAProjectRels) > 1 {
		diags = append(diags, diag.Error(
			"VBA_MULTIPLE_RELATIONSHIPS",
			fmt.Sprintf("main part %s has %d VBA project relationships; at most one is allowed", mainURI, len(mainVBAProjectRels)),
		))
	}

	vbaPartCount := 0
	for uri, part := range vbaParts {
		if strings.EqualFold(part.ContentType, vba.ContentTypeVBAProject) {
			vbaPartCount++
		}
		if !strings.EqualFold(part.ContentType, vba.ContentTypeVBAProject) && !reportedWrongContentType[uri] {
			diags = append(diags, diag.Error(
				"VBA_PART_WRONG_CONTENT_TYPE",
				fmt.Sprintf("VBA project candidate %s has content type %q, want %q", uri, part.ContentType, vba.ContentTypeVBAProject),
			))
			reportedWrongContentType[uri] = true
		}
		if part.SizeBytes == 0 && !reportedEmptyProject[uri] {
			diags = append(diags, diag.Error(
				"VBA_PROJECT_EMPTY",
				fmt.Sprintf("VBA project part %s is empty", uri),
			))
			reportedEmptyProject[uri] = true
		}
		if !validMainTargets[uri] {
			diags = append(diags, diag.Error(
				"VBA_ORPHAN_PART",
				fmt.Sprintf("VBA project part %s is not targeted by a VBA relationship from %s", uri, mainURI),
			))
		}
	}
	if vbaPartCount > 1 {
		diags = append(diags, diag.Error(
			"VBA_MULTIPLE_PROJECT_PARTS",
			fmt.Sprintf("package contains %d VBA project parts; at most one is allowed", vbaPartCount),
		))
	}
	diags = append(diags, validateVBAProjectOutgoingRelationships(session, spec, vbaParts)...)

	hasProjectEvidence := len(vbaParts) > 0 || len(mainVBAProjectRels) > 0
	hasValidExistingProject := false
	for uri := range validMainTargets {
		if _, exists := partMap[uri]; exists {
			hasValidExistingProject = true
			break
		}
	}

	if strings.EqualFold(mainContentType, spec.MacroMainContentType) && !hasValidExistingProject {
		diags = append(diags, diag.Warning(
			"VBA_MAIN_MACRO_WITHOUT_PROJECT",
			fmt.Sprintf("main part %s is macro-enabled but no valid VBA project relationship targets an existing project", mainURI),
		))
	}
	if hasProjectEvidence && strings.EqualFold(mainContentType, spec.NonMacroMainContentType) {
		diags = append(diags, diag.Error(
			"VBA_MAIN_NOT_MACRO_ENABLED",
			fmt.Sprintf("VBA project artifacts are present but main part %s is not macro-enabled", mainURI),
		))
	}

	for _, artifact := range vba.FindSignatureArtifacts(session) {
		diags = append(diags, diag.Warning(
			"VBA_SIGNATURE_ARTIFACT",
			formatVBASignatureArtifact(artifact),
		))
	}
	if project, err := vba.InspectSourceProject(session); err == nil {
		for _, warning := range vba.HostCompatibilityWarnings(project) {
			diags = append(diags, diag.Warning(warning.Code, warning.Message))
		}
	}

	return diags, nil
}

func detectVBAValidationFamily(session opc.PackageSession) (*vba.FamilySpec, string) {
	packageType := opc.DetectType(session)
	specs := vba.SupportedFamilySpecs()
	for i := range specs {
		spec := &specs[i]
		if spec.PackageType != packageType {
			continue
		}
		if mainURI := findVBAValidationMainPart(session, spec); mainURI != "" {
			return spec, mainURI
		}
	}

	for i := range specs {
		spec := &specs[i]
		if mainURI := findVBAValidationMainPart(session, spec); mainURI != "" {
			return spec, mainURI
		}
	}

	return nil, ""
}

func findVBAValidationMainPart(session opc.PackageSession, spec *vba.FamilySpec) string {
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

	for _, part := range session.ListParts() {
		if part.URI == spec.DefaultMainPartURI ||
			strings.EqualFold(part.ContentType, spec.NonMacroMainContentType) ||
			strings.EqualFold(part.ContentType, spec.MacroMainContentType) {
			return part.URI
		}
	}
	return ""
}

func findVBAProjectPartCandidates(parts []opc.PartInfo, spec *vba.FamilySpec) map[string]opc.PartInfo {
	candidates := map[string]opc.PartInfo{}
	for _, part := range parts {
		if part.URI == spec.DefaultVBAProjectPartURI ||
			strings.EqualFold(part.ContentType, vba.ContentTypeVBAProject) {
			candidates[part.URI] = part
		}
	}
	return candidates
}

func findVBARelationships(session opc.PackageSession, parts []opc.PartInfo, spec *vba.FamilySpec) []vbaRelationshipFinding {
	sources := make([]string, 0, len(parts)+1)
	sources = append(sources, "/")
	for _, part := range parts {
		sources = append(sources, part.URI)
	}

	var findings []vbaRelationshipFinding
	for _, sourceURI := range sources {
		for _, rel := range session.ListRelationships(sourceURI) {
			targetURI := opc.ResolveRelationshipTarget(sourceURI, rel.Target)
			if isVBARelationshipCandidate(session, rel, targetURI, spec) {
				findings = append(findings, vbaRelationshipFinding{
					sourceURI: sourceURI,
					rel:       rel,
					targetURI: targetURI,
				})
			}
		}
	}
	return findings
}

func isVBARelationshipCandidate(session opc.PackageSession, rel opc.RelationshipInfo, targetURI string, spec *vba.FamilySpec) bool {
	if rel.Type == vba.RelationshipTypeVBAProject {
		return true
	}
	if targetURI == spec.DefaultVBAProjectPartURI {
		return true
	}
	return strings.EqualFold(session.GetContentType(targetURI), vba.ContentTypeVBAProject)
}

func validateVBAProjectOutgoingRelationships(session opc.PackageSession, spec *vba.FamilySpec, vbaParts map[string]opc.PartInfo) []result.Diagnostic {
	var diags []result.Diagnostic
	sources := map[string]bool{}
	if spec != nil && spec.DefaultVBAProjectPartURI != "" {
		sources[spec.DefaultVBAProjectPartURI] = true
	}
	for uri := range vbaParts {
		sources[uri] = true
	}
	for uri := range sources {
		for _, rel := range session.ListRelationships(uri) {
			target := rel.Target
			if rel.TargetMode != "External" {
				target = opc.ResolveRelationshipTarget(uri, rel.Target)
			}
			message := fmt.Sprintf("VBA project part %s must not have relationships; found id=%s type=%s target=%s", uri, rel.ID, rel.Type, target)
			if rel.TargetMode != "" {
				message += fmt.Sprintf(" targetMode=%s", rel.TargetMode)
			}
			diags = append(diags, diag.Error("VBA_PROJECT_UNEXPECTED_RELATIONSHIP", message))
		}
	}
	return diags
}

func filterVBARelationshipsFromSource(findings []vbaRelationshipFinding, sourceURI string, requireVBAType bool) []vbaRelationshipFinding {
	var filtered []vbaRelationshipFinding
	for _, finding := range findings {
		if finding.sourceURI != sourceURI {
			continue
		}
		if requireVBAType && finding.rel.Type != vba.RelationshipTypeVBAProject {
			continue
		}
		filtered = append(filtered, finding)
	}
	return filtered
}

func formatVBASignatureArtifact(artifact vba.SignatureArtifact) string {
	if artifact.Kind == "relationship" {
		return fmt.Sprintf("known VBA/package signature relationship from %s (id=%s) targets %s", artifact.SourceURI, artifact.RelationshipID, artifact.PartURI)
	}
	return fmt.Sprintf("known VBA/package signature part is present: %s", artifact.PartURI)
}
