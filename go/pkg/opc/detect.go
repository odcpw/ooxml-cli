package opc

import (
	"strings"
)

// PackageType is defined in types.go

// DetectType detects the type of an OPC package.
func DetectType(session PackageSession) PackageType {
	// Look at the root relationships to determine the package type
	rootRels := session.ListRelationships("/")

	for _, rel := range rootRels {
		targetURI := ResolveRelationshipTarget("/", rel.Target)
		targetContentType := session.GetContentType(targetURI)

		// Check for presentation relationships (PPTX)
		if strings.Contains(rel.Type, "presentationml.presentation") {
			return PackageTypePPTX
		}
		if strings.Contains(targetContentType, "presentationml.presentation") || strings.HasPrefix(targetURI, "/ppt/") {
			return PackageTypePPTX
		}

		// Check for document relationships (DOCX)
		if strings.Contains(rel.Type, "wordprocessingml.document") {
			return PackageTypeDOCX
		}
		if strings.Contains(targetContentType, "wordprocessingml.document") || strings.HasPrefix(targetURI, "/word/") {
			return PackageTypeDOCX
		}

		// Check for workbook relationships (XLSX)
		if strings.Contains(rel.Type, "spreadsheetml.sheet") {
			return PackageTypeXLSX
		}
		if strings.Contains(targetContentType, "spreadsheetml.sheet") || strings.HasPrefix(targetURI, "/xl/") {
			return PackageTypeXLSX
		}
	}

	// Fallback: check content types
	parts := session.ListParts()
	for _, part := range parts {
		contentType := part.ContentType
		if strings.Contains(contentType, "presentationml") {
			return PackageTypePPTX
		}
		if strings.Contains(contentType, "wordprocessingml") {
			return PackageTypeDOCX
		}
		if strings.Contains(contentType, "spreadsheetml") {
			return PackageTypeXLSX
		}
	}

	return PackageTypeUnknown
}
