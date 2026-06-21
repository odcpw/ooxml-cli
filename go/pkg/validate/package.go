package validate

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// validatePackageIntegrity validates Stage 1: zip structure, required parts, content types.
// Returns diagnostics (not errors) so partial validation can continue.
func validatePackageIntegrity(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	parts := session.ListParts()
	if len(parts) == 0 {
		diags = append(diags, diag.Error(
			"PKG_EMPTY",
			"package contains no parts",
		))
		return diags, nil
	}

	// Check for required root parts
	hasContentTypes := false
	hasRels := false

	for _, part := range parts {
		if part.URI == "/[Content_Types].xml" {
			hasContentTypes = true
		}
		if part.URI == "/_rels/.rels" {
			hasRels = true
		}
	}

	if !hasContentTypes {
		diags = append(diags, diag.Error(
			"PKG_MISSING_CONTENT_TYPES",
			"[Content_Types].xml not found in package root",
		))
	}

	if !hasRels {
		diags = append(diags, diag.Error(
			"PKG_MISSING_PACKAGE_RELS",
			"/_rels/.rels not found in package",
		))
	}

	// Surface non-fatal package warnings captured while opening the package.
	for _, warning := range session.Warnings() {
		diags = append(diags, diag.Warning(
			"PKG_WARNING",
			warning,
		))
	}

	// Validate content types are readable
	for _, part := range parts {
		ct := session.GetContentType(part.URI)
		if ct == "" && !isKnownPartWithoutContentType(part.URI) {
			diags = append(diags, diag.Warning(
				"PKG_NO_CONTENT_TYPE",
				"part "+part.URI+" has no content type",
			))
		}
	}

	return diags, nil
}

// isKnownPartWithoutContentType checks if a part is known to not have a content type.
func isKnownPartWithoutContentType(uri string) bool {
	// [Content_Types].xml and .rels files typically don't have explicit content types
	switch uri {
	case "/[Content_Types].xml", "/_rels/.rels":
		return true
	}
	// .rels files anywhere in the package tree
	if len(uri) > 5 {
		if uri[len(uri)-5:] == ".rels" {
			return true
		}
	}
	return false
}
