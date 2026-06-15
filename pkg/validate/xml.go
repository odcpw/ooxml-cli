package validate

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/core/diag"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// validateModifiedXML validates Stage 4: XML well-formedness of modified parts.
// This ensures that any parts that have been modified can still be parsed as valid XML.
func validateModifiedXML(session opc.PackageSession) ([]result.Diagnostic, error) {
	var diags []result.Diagnostic

	// Get all parts
	parts := session.ListParts()

	// Check each part
	for _, part := range parts {
		// Skip non-XML parts
		if !isXMLPart(part.URI, part.ContentType) {
			continue
		}

		// Try to parse the XML
		_, err := session.ReadXMLPart(part.URI)
		if err != nil {
			diags = append(diags, diag.Error(
				"XML_PARSE_ERROR",
				"failed to parse XML part "+part.URI+": "+err.Error(),
			))
		}
	}

	return diags, nil
}

// isXMLPart checks if a part is an XML part based on its URI or content type
func isXMLPart(uri string, contentType string) bool {
	// Check content type first
	if contentType != "" {
		// Content types that contain "xml" are XML
		switch contentType {
		case
			"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml",
			"application/vnd.openxmlformats-officedocument.presentationml.slide+xml",
			"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml",
			"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml",
			"application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml",
			"application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml",
			"application/vnd.openxmlformats-officedocument.presentationml.handoutMaster+xml",
			"application/vnd.openxmlformats-officedocument.theme+xml",
			"application/vnd.openxmlformats-package.relationships+xml",
			"application/vnd.openxmlformats-officedocument.custom-properties+xml",
			"application/vnd.openxmlformats-officedocument.core-properties+xml":
			return true
		}

		// Check for generic XML content type
		if contentType == "application/xml" || contentType == "text/xml" {
			return true
		}
	}

	// Check URI for common XML file patterns
	xmlExtensions := []string{".xml", ".rels"}
	for _, ext := range xmlExtensions {
		// Simple check for file ending
		if len(uri) >= len(ext) && uri[len(uri)-len(ext):] == ext {
			return true
		}
	}

	return false
}
