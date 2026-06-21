package validate

import (
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/vba"
)

func TestValidateVBAAttachedXLSXNoVBADiagnostics(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoVBADiagnostics(t, diags)
	assertNoErrorDiagnostics(t, diags)
}

func TestValidateVBAProjectWithNonMacroMainContentType(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_MAIN_NOT_MACRO_ENABLED")
}

func TestValidateVBAMacroMainWithoutProject(t *testing.T) {
	session := newXLSXValidationSession()
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_MAIN_MACRO_WITHOUT_PROJECT")
}

func TestValidateVBAWrongContentType(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", "application/octet-stream")
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_PART_WRONG_CONTENT_TYPE")
}

func TestValidateVBAOrphanProject(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_ORPHAN_PART")
	assertHasDiagnosticCode(t, diags, "VBA_MAIN_MACRO_WITHOUT_PROJECT")
}

func TestValidateVBADanglingRelationship(t *testing.T) {
	session := newXLSXValidationSession()
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "REL_DANGLING_TARGET")
	assertHasDiagnosticCode(t, diags, "VBA_REL_MISSING_TARGET")
}

func TestValidateVBAWrongSourceRelationship(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/worksheets/sheet1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/worksheets/sheet1.xml",
			ID:        "rId1",
			Type:      vba.RelationshipTypeVBAProject,
			Target:    "../vbaProject.bin",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_REL_WRONG_SOURCE")
	assertHasDiagnosticCode(t, diags, "VBA_ORPHAN_PART")
}

func TestValidateVBAMultipleRelationships(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	addVBAProjectPart(session, "/xl/vbaProject2.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"],
		opc.RelationshipInfo{
			SourceURI: "/xl/workbook.xml",
			ID:        "rId2",
			Type:      vba.RelationshipTypeVBAProject,
			Target:    "vbaProject.bin",
		},
		opc.RelationshipInfo{
			SourceURI: "/xl/workbook.xml",
			ID:        "rId3",
			Type:      vba.RelationshipTypeVBAProject,
			Target:    "vbaProject2.bin",
		},
	)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_MULTIPLE_RELATIONSHIPS")
	assertHasDiagnosticCode(t, diags, "VBA_MULTIPLE_PROJECT_PARTS")
}

func TestValidateVBAWrongRelationshipType(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml",
		Target:    "vbaProject.bin",
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_REL_WRONG_TYPE")
	assertHasDiagnosticCode(t, diags, "VBA_ORPHAN_PART")
}

func TestValidateVBASignatureArtifactsWarn(t *testing.T) {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, opc.PartInfo{
		URI:         "/xl/vbaProjectSignature.bin",
		ContentType: "application/vnd.ms-office.vbaProjectSignature",
		IsXML:       false,
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_SIGNATURE_ARTIFACT")
}

func TestValidateVBAEmptyProject(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPartWithSize(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject, 0)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_PROJECT_EMPTY")
}

func TestValidateVBAProjectOutgoingRelationship(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})
	session.relationships["/xl/vbaProject.bin"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/vbaProject.bin",
			ID:        "rId1",
			Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
			Target:    "media/image1.png",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticDetail(t, diags, "VBA_PROJECT_UNEXPECTED_RELATIONSHIP", result.Error,
		"/xl/vbaProject.bin",
		"rId1",
		"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
		"/xl/media/image1.png",
	)
}

func TestValidateVBAProjectExternalOutgoingRelationship(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})
	session.relationships["/xl/vbaProject.bin"] = []opc.RelationshipInfo{
		{
			SourceURI:  "/xl/vbaProject.bin",
			ID:         "rId1",
			Type:       "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
			Target:     "https://example.invalid/macro",
			TargetMode: "External",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticDetail(t, diags, "VBA_PROJECT_UNEXPECTED_RELATIONSHIP", result.Error,
		"/xl/vbaProject.bin",
		"rId1",
		"https://example.invalid/macro",
		"targetMode=External",
	)
}

func TestValidateVBAProjectSignatureOutgoingRelationship(t *testing.T) {
	session := newXLSXValidationSession()
	addVBAProjectPart(session, "/xl/vbaProject.bin", vba.ContentTypeVBAProject)
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/workbook.xml"] = append(session.relationships["/xl/workbook.xml"], opc.RelationshipInfo{
		SourceURI: "/xl/workbook.xml",
		ID:        "rId2",
		Type:      vba.RelationshipTypeVBAProject,
		Target:    "vbaProject.bin",
	})
	session.relationships["/xl/vbaProject.bin"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/vbaProject.bin",
			ID:        "rId1",
			Type:      "http://schemas.microsoft.com/office/2006/relationships/vbaProjectSignature",
			Target:    "vbaProjectSignature.bin",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_PROJECT_UNEXPECTED_RELATIONSHIP")
	assertHasDiagnosticCode(t, diags, "VBA_SIGNATURE_ARTIFACT")
}

func TestValidateVBAProjectOutgoingRelationshipWithoutProjectPart(t *testing.T) {
	session := newXLSXValidationSession()
	setPartContentType(session, "/xl/workbook.xml", "application/vnd.ms-excel.sheet.macroEnabled.main+xml")
	session.relationships["/xl/vbaProject.bin"] = []opc.RelationshipInfo{
		{
			SourceURI: "/xl/vbaProject.bin",
			ID:        "rId1",
			Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
			Target:    "media/image1.png",
		},
	}

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "VBA_PROJECT_UNEXPECTED_RELATIONSHIP")
	assertHasDiagnosticCode(t, diags, "VBA_MAIN_MACRO_WITHOUT_PROJECT")
}

func TestValidateVBAProjectOutgoingRelationshipPPTX(t *testing.T) {
	session := newPPTMValidationSession()
	session.relationships["/ppt/vbaProject.bin"] = []opc.RelationshipInfo{
		{
			SourceURI: "/ppt/vbaProject.bin",
			ID:        "rId1",
			Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
			Target:    "media/image1.png",
		},
	}

	diags, err := validateVBAPackageConsistency(session)
	if err != nil {
		t.Fatalf("validateVBAPackageConsistency returned error: %v", err)
	}
	assertHasDiagnosticDetail(t, diags, "VBA_PROJECT_UNEXPECTED_RELATIONSHIP", result.Error,
		"/ppt/vbaProject.bin",
		"rId1",
		"/ppt/media/image1.png",
	)
}

func addVBAProjectPart(session *validationTestSession, uri, contentType string) {
	addVBAProjectPartWithSize(session, uri, contentType, 4)
}

func addVBAProjectPartWithSize(session *validationTestSession, uri, contentType string, sizeBytes int64) {
	session.parts = append(session.parts, opc.PartInfo{
		URI:         uri,
		ContentType: contentType,
		SizeBytes:   sizeBytes,
		IsXML:       false,
	})
}

func setPartContentType(session *validationTestSession, uri, contentType string) {
	for i := range session.parts {
		if session.parts[i].URI == uri {
			session.parts[i].ContentType = contentType
			return
		}
	}
}

func newPPTMValidationSession() *validationTestSession {
	return &validationTestSession{
		parts: []opc.PartInfo{
			xmlPart("/[Content_Types].xml", "application/xml"),
			xmlPart("/_rels/.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			xmlPart("/ppt/presentation.xml", "application/vnd.ms-powerpoint.presentation.macroEnabled.main+xml"),
			xmlPart("/ppt/_rels/presentation.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			{
				URI:         "/ppt/vbaProject.bin",
				ContentType: vba.ContentTypeVBAProject,
				SizeBytes:   4,
				IsXML:       false,
			},
		},
		relationships: map[string][]opc.RelationshipInfo{
			"/": {
				{
					SourceURI: "/",
					ID:        "rId1",
					Type:      "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument",
					Target:    "ppt/presentation.xml",
				},
			},
			"/ppt/presentation.xml": {
				{
					SourceURI: "/ppt/presentation.xml",
					ID:        "rId1",
					Type:      vba.RelationshipTypeVBAProject,
					Target:    "vbaProject.bin",
				},
			},
		},
		xmlParts: map[string]string{
			"/[Content_Types].xml":             `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`,
			"/_rels/.rels":                     `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/ppt/presentation.xml":            `<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
			"/ppt/_rels/presentation.xml.rels": `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
		},
	}
}

func assertNoVBADiagnostics(t *testing.T, diags []result.Diagnostic) {
	t.Helper()
	for _, d := range diags {
		if strings.HasPrefix(d.Code, "VBA_") {
			t.Fatalf("unexpected VBA diagnostic %s: %s", d.Code, d.Message)
		}
	}
}

func assertHasDiagnosticDetail(t *testing.T, diags []result.Diagnostic, code string, severity result.Severity, messageContains ...string) {
	t.Helper()
	for _, d := range diags {
		if d.Code != code {
			continue
		}
		if d.Severity != severity {
			t.Fatalf("diagnostic %s severity = %s, want %s", code, d.Severity, severity)
		}
		for _, snippet := range messageContains {
			if !strings.Contains(d.Message, snippet) {
				t.Fatalf("diagnostic %s message %q missing %q", code, d.Message, snippet)
			}
		}
		return
	}
	t.Fatalf("expected diagnostic %s, got %+v", code, diags)
}
