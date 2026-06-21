package validate

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// validateRelationshipIntegrity validates Stage 2: no dangling references, all targets exist.
func validateRelationshipIntegrity(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	parts := session.ListParts()
	partMap := make(map[string]bool)
	for _, part := range parts {
		partMap[part.URI] = true
	}

	// Check all relationships
	for _, part := range parts {
		rels := session.ListRelationships(part.URI)

		for _, rel := range rels {
			// Skip external relationships
			if rel.TargetMode == "External" {
				continue
			}

			// Resolve target URI
			targetURI := opc.ResolveRelationshipTarget(part.URI, rel.Target)

			// Check if target exists
			if !partMap[targetURI] {
				diags = append(diags, diag.Error(
					"REL_DANGLING_TARGET",
					"relationship from "+part.URI+" (id="+rel.ID+") points to missing part: "+targetURI,
				))
			}
		}
	}

	// Check package-level relationships
	pkgRels := session.ListRelationships("/")
	for _, rel := range pkgRels {
		if rel.TargetMode == "External" {
			continue
		}

		targetURI := opc.ResolveRelationshipTarget("/", rel.Target)
		if !partMap[targetURI] {
			diags = append(diags, diag.Error(
				"REL_DANGLING_TARGET",
				"package-level relationship (id="+rel.ID+") points to missing part: "+targetURI,
			))
		}
	}

	return diags, nil
}
