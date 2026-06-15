package validate

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

func TestValidatePPTXPresentationSlideIDWrongTargetType(t *testing.T) {
	session := newPPTXValidationSession()
	session.parts = append(session.parts, xmlPart("/ppt/slideLayouts/slideLayout1.xml", pptxContentTypeSlideLayout))
	session.relationships["/ppt/presentation.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/ppt/presentation.xml",
			ID:        "rId1",
			Type:      pptxRelTypeSlide,
			Target:    "slideLayouts/slideLayout1.xml",
		},
	}
	session.xmlParts["/ppt/slideLayouts/slideLayout1.xml"] = `<p:sldLayout xmlns:p="` + namespaces.NsP + `"><p:cSld><p:spTree/></p:cSld></p:sldLayout>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_SLIDE_ID_WRONG_TARGET_TYPE")
}

func TestValidatePPTXSlideXMLMissingImageRelationship(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBody(`
		<p:pic>
			<p:nvPicPr><p:cNvPr id="3" name="Picture 1"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
			<p:blipFill><a:blip r:embed="rIdImage"/></p:blipFill>
			<p:spPr/>
		</p:pic>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_MISSING_SLIDE_RELATIONSHIP")
}

func TestValidatePPTXSlideHyperlinkWrongRelationshipType(t *testing.T) {
	session := newPPTXValidationSession()
	session.relationships["/ppt/slides/slide1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI:  "/ppt/slides/slide1.xml",
			ID:         "rId2",
			Type:       pptxRelTypeImage,
			Target:     "https://example.com",
			TargetMode: "External",
		},
	}
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBody(`
		<p:sp>
			<p:nvSpPr>
				<p:cNvPr id="2" name="Linked Shape"><a:hlinkClick r:id="rId2"/></p:cNvPr>
				<p:cNvSpPr/><p:nvPr/>
			</p:nvSpPr>
			<p:spPr/>
		</p:sp>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_WRONG_SLIDE_RELATIONSHIP_TYPE")
}

func TestValidatePPTXNotesMissingSlideBacklink(t *testing.T) {
	session := newPPTXValidationSession()
	session.parts = append(session.parts, xmlPart("/ppt/notesSlides/notesSlide1.xml", pptxContentTypeNotesSlide))
	session.relationships["/ppt/slides/slide1.xml"] = []opc.RelationshipInfo{
		{
			SourceURI: "/ppt/slides/slide1.xml",
			ID:        "rIdNotes",
			Type:      pptxRelTypeNotesSlide,
			Target:    "../notesSlides/notesSlide1.xml",
		},
	}
	session.xmlParts["/ppt/notesSlides/notesSlide1.xml"] = `<p:notesSlide xmlns:p="` + namespaces.NsP + `"><p:cSld><p:spTree/></p:cSld></p:notesSlide>`

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_NOTES_MISSING_SLIDE_BACKLINK")
}

func TestValidatePPTXAnimationMissingShapeTarget(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBodyAndTail("", animationTimingXML("99"))

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_STALE_ANIMATION_TARGET")
	assertDiagnosticSeverity(t, diags, "PPTX_STALE_ANIMATION_TARGET", result.Warning)
}

func TestValidatePPTXAnimationMissingBuildTargetIsWarning(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBodyAndTail("", `<p:timing>
		<p:bldLst>
			<p:bldP spid="99" grpId="0" build="p"/>
		</p:bldLst>
	</p:timing>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertDiagnosticSeverity(t, diags, "PPTX_STALE_ANIMATION_BUILD_TARGET", result.Warning)
}

func TestValidatePPTXStaleMediaReferenceIsWarning(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBody(`
		<p:pic>
			<p:nvPicPr>
				<p:cNvPr id="3" name="Video 1"/>
				<p:cNvPicPr/>
				<p:nvPr><a:videoFile r:link="rIdMissingVideo"/></p:nvPr>
			</p:nvPicPr>
			<p:blipFill/>
			<p:spPr/>
		</p:pic>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertDiagnosticSeverity(t, diags, "PPTX_STALE_MEDIA_REFERENCE", result.Warning)
}

func TestValidatePPTXAnimationNestedShapeTargetAllowed(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBodyAndTail(`
		<p:grpSp>
			<p:nvGrpSpPr><p:cNvPr id="8" name="Group 1"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
			<p:grpSpPr/>
			<p:sp>
				<p:nvSpPr><p:cNvPr id="99" name="Nested Target"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
				<p:spPr/>
			</p:sp>
		</p:grpSp>`, animationTimingXML("99"))

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_STALE_ANIMATION_TARGET")
}

func TestValidatePPTXDuplicateNestedShapeIDs(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBody(`
		<p:grpSp>
			<p:nvGrpSpPr><p:cNvPr id="8" name="Group 1"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
			<p:grpSpPr/>
			<p:sp>
				<p:nvSpPr><p:cNvPr id="8" name="Nested Duplicate"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
				<p:spPr/>
			</p:sp>
		</p:grpSp>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_DUPLICATE_SHAPE_ID")
}

func TestValidatePPTXInvalidShapeID(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBody(`
		<p:cxnSp>
			<p:nvCxnSpPr><p:cNvPr id="not-a-number" name="Bad Connector"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>
			<p:spPr/>
		</p:cxnSp>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "PPTX_SHAPE_ID")
}

func TestValidatePPTXAllowsShapeIDZero(t *testing.T) {
	session := newPPTXValidationSession()
	session.xmlParts["/ppt/slides/slide1.xml"] = slideXMLWithBody(`
		<p:cxnSp>
			<p:nvCxnSpPr><p:cNvPr id="0" name="Zero Connector"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>
			<p:spPr/>
		</p:cxnSp>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_SHAPE_ID")
}

func assertDiagnosticSeverity(t *testing.T, diags []result.Diagnostic, code string, severity result.Severity) {
	t.Helper()
	for _, d := range diags {
		if d.Code != code {
			continue
		}
		if d.Severity != severity {
			t.Fatalf("diagnostic %s severity = %s, want %s", code, d.Severity, severity)
		}
		return
	}
	t.Fatalf("expected diagnostic %s, got %+v", code, diags)
}

func newPPTXValidationSession() *validationTestSession {
	return &validationTestSession{
		parts: []opc.PartInfo{
			xmlPart("/[Content_Types].xml", "application/xml"),
			xmlPart("/_rels/.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			xmlPart("/ppt/presentation.xml", pptxContentTypePresentation),
			xmlPart("/ppt/_rels/presentation.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"),
			xmlPart("/ppt/slides/slide1.xml", pptxContentTypeSlide),
			xmlPart("/ppt/slides/_rels/slide1.xml.rels", "application/vnd.openxmlformats-package.relationships+xml"),
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
					Type:      pptxRelTypeSlide,
					Target:    "slides/slide1.xml",
				},
			},
			"/ppt/slides/slide1.xml": {},
		},
		xmlParts: map[string]string{
			"/[Content_Types].xml":              `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`,
			"/_rels/.rels":                      `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/ppt/_rels/presentation.xml.rels":  `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/ppt/slides/_rels/slide1.xml.rels": `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
			"/ppt/presentation.xml":             `<p:presentation xmlns:p="` + namespaces.NsP + `" xmlns:r="` + namespaces.NsR + `"><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst><p:sldSz cx="9144000" cy="6858000"/></p:presentation>`,
			"/ppt/slides/slide1.xml":            slideXMLWithBody(""),
		},
	}
}

func slideXMLWithBody(body string) string {
	return slideXMLWithBodyAndTail(body, "")
}

func slideXMLWithBodyAndTail(body, tail string) string {
	return `<p:sld xmlns:p="` + namespaces.NsP + `" xmlns:a="` + namespaces.NsA + `" xmlns:r="` + namespaces.NsR + `" xmlns:p14="` + namespaces.Np14 + `">
		<p:cSld>
			<p:spTree>
				<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
				<p:grpSpPr/>
				<p:sp>
					<p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>
					<p:spPr/>
				</p:sp>
				` + body + `
			</p:spTree>
		</p:cSld>
		` + tail + `
	</p:sld>`
}

func animationTimingXML(spid string) string {
	return `<p:timing>
		<p:tnLst>
			<p:par>
				<p:cTn id="1" nodeType="tmRoot">
					<p:childTnLst>
						<p:seq>
							<p:cTn id="2" nodeType="mainSeq">
								<p:childTnLst>
									<p:par>
										<p:cTn id="3" nodeType="clickEffect">
											<p:childTnLst>
												<p:par>
													<p:cTn id="4" presetClass="entr" nodeType="clickEffect">
														<p:childTnLst>
															<p:set>
																<p:cBhvr>
																	<p:cTn id="5"/>
																	<p:tgtEl><p:spTgt spid="` + spid + `"/></p:tgtEl>
																	<p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst>
																</p:cBhvr>
																<p:to><p:strVal val="visible"/></p:to>
															</p:set>
														</p:childTnLst>
													</p:cTn>
												</p:par>
											</p:childTnLst>
										</p:cTn>
									</p:par>
								</p:childTnLst>
							</p:cTn>
						</p:seq>
					</p:childTnLst>
				</p:cTn>
			</p:par>
		</p:tnLst>
	</p:timing>`
}
