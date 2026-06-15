package vba

import (
	"archive/zip"
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestAttachExtractRemovePPTX(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "input.pptx")
	writeSyntheticPackage(t, inputPath, "pptx", nil, false)

	projectData := []byte("opaque ppt vba project")
	pkg := openPackage(t, inputPath)
	result, err := Attach(pkg, projectData)
	if err != nil {
		t.Fatalf("Attach failed: %v", err)
	}
	if result.Family != "pptx" || !result.MacroEnabled || result.VBAPartURI != "/ppt/vbaProject.bin" {
		t.Fatalf("unexpected attach result: %+v", result)
	}

	attachedPath := filepath.Join(t.TempDir(), "attached.pptm")
	if err := pkg.SaveAs(attachedPath); err != nil {
		t.Fatalf("SaveAs attached failed: %v", err)
	}
	pkg.Close()

	attached := openPackage(t, attachedPath)
	extracted, info, err := ExtractBin(attached)
	if err != nil {
		t.Fatalf("ExtractBin failed: %v", err)
	}
	if !bytes.Equal(extracted, projectData) {
		t.Fatalf("extracted bytes = %q, want %q", extracted, projectData)
	}
	if !info.MacroEnabled || !info.HasVBAProject || info.VBAProject.RelationshipID == "" {
		t.Fatalf("unexpected attached info: %+v", info)
	}

	removeResult, err := Remove(attached)
	if err != nil {
		t.Fatalf("Remove failed: %v", err)
	}
	if removeResult.MacroEnabled {
		t.Fatalf("unexpected remove result: %+v", removeResult)
	}

	removedPath := filepath.Join(t.TempDir(), "removed.pptx")
	if err := attached.SaveAs(removedPath); err != nil {
		t.Fatalf("SaveAs removed failed: %v", err)
	}
	attached.Close()

	removed := openPackage(t, removedPath)
	defer removed.Close()
	removedInfo, err := Inspect(removed)
	if err != nil {
		t.Fatalf("Inspect removed failed: %v", err)
	}
	if removedInfo.MacroEnabled || removedInfo.HasVBAProject || removedInfo.VBAProject != nil {
		t.Fatalf("expected macro-free package, got %+v", removedInfo)
	}
	if ct := removed.GetContentType("/ppt/presentation.xml"); ct != "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml" {
		t.Fatalf("main content type = %q", ct)
	}
	if _, err := removed.ReadRawPart("/ppt/vbaProject.bin"); err == nil {
		t.Fatal("expected vbaProject.bin to be removed")
	}
	for _, rel := range removed.ListRelationships("/ppt/presentation.xml") {
		if rel.Type == RelationshipTypeVBAProject {
			t.Fatalf("unexpected VBA relationship after remove: %+v", rel)
		}
	}
}

func TestInspectXLSM(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "input.xlsm")
	projectData := []byte("opaque xls vba project")
	writeSyntheticPackage(t, inputPath, "xlsx", projectData, false)

	pkg := openPackage(t, inputPath)
	defer pkg.Close()
	info, err := Inspect(pkg)
	if err != nil {
		t.Fatalf("Inspect failed: %v", err)
	}
	if info.Family != "xlsx" || !info.MacroEnabled || !info.HasVBAProject {
		t.Fatalf("unexpected info: %+v", info)
	}
	if info.VBAProject == nil || info.VBAProject.PartURI != "/xl/vbaProject.bin" || info.VBAProject.SizeBytes != int64(len(projectData)) {
		t.Fatalf("unexpected VBA project info: %+v", info.VBAProject)
	}
}

func TestAttachRejectsSignatureArtifacts(t *testing.T) {
	inputPath := filepath.Join(t.TempDir(), "signed.xlsx")
	writeSyntheticPackage(t, inputPath, "xlsx", nil, true)

	pkg := openPackage(t, inputPath)
	defer pkg.Close()
	_, err := Attach(pkg, []byte("macro"))
	if err == nil {
		t.Fatal("expected Attach to reject signed package")
	}
	if !strings.Contains(err.Error(), "signature artifacts") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func openPackage(t *testing.T, path string) *opc.Package {
	t.Helper()
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open package %s: %v", path, err)
	}
	return pkg
}

func writeSyntheticPackage(t *testing.T, path, family string, vbaData []byte, signed bool) {
	t.Helper()

	spec := familySpecs[0]
	if family == "xlsx" {
		spec = familySpecs[1]
	}
	macro := len(vbaData) > 0

	files := map[string][]byte{
		"[Content_Types].xml": []byte(contentTypesXML(spec, macro, signed)),
		"_rels/.rels":         []byte(rootRelsXML(spec, signed)),
		strings.TrimPrefix(spec.DefaultMainPartURI, "/"): []byte(mainPartXML(family)),
	}
	if macro {
		files[strings.TrimPrefix(spec.DefaultVBAProjectPartURI, "/")] = vbaData
		rels, err := opc.BuildRelationshipsXML([]opc.RelationshipInfo{
			{
				ID:     "rId1",
				Type:   RelationshipTypeVBAProject,
				Target: opc.RelationshipTarget(spec.DefaultMainPartURI, spec.DefaultVBAProjectPartURI),
			},
		})
		if err != nil {
			t.Fatalf("failed to build main rels: %v", err)
		}
		files[strings.TrimPrefix(opc.RelsURIForPart(spec.DefaultMainPartURI), "/")] = rels
	}
	if signed {
		files["_xmlsignatures/sig1.xml"] = []byte(`<Signature xmlns="http://www.w3.org/2000/09/xmldsig#"/>`)
	}

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("failed to create temp dir: %v", err)
	}
	out, err := os.Create(path)
	if err != nil {
		t.Fatalf("failed to create package: %v", err)
	}
	defer out.Close()

	zw := zip.NewWriter(out)
	defer zw.Close()
	for name, data := range files {
		w, err := zw.Create(name)
		if err != nil {
			t.Fatalf("failed to create zip part %s: %v", name, err)
		}
		if _, err := w.Write(data); err != nil {
			t.Fatalf("failed to write zip part %s: %v", name, err)
		}
	}
}

func contentTypesXML(spec FamilySpec, macro bool, signed bool) string {
	mainCT := spec.NonMacroMainContentType
	if macro {
		mainCT = spec.MacroMainContentType
	}
	extra := ""
	if macro {
		extra += `<Override PartName="` + spec.DefaultVBAProjectPartURI + `" ContentType="` + ContentTypeVBAProject + `"/>`
	}
	if signed {
		extra += `<Override PartName="/_xmlsignatures/sig1.xml" ContentType="application/vnd.openxmlformats-package.digital-signature-xmlsignature+xml"/>`
	}
	return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="` + spec.DefaultMainPartURI + `" ContentType="` + mainCT + `"/>` + extra + `
</Types>`
}

func rootRelsXML(spec FamilySpec, signed bool) string {
	extra := ""
	if signed {
		extra = `<Relationship Id="rIdSig" Type="http://schemas.openxmlformats.org/package/2006/relationships/digital-signature/origin" Target="_xmlsignatures/sig1.xml"/>`
	}
	return `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="` + strings.TrimPrefix(spec.DefaultMainPartURI, "/") + `"/>` + extra + `
</Relationships>`
}

func mainPartXML(family string) string {
	if family == "xlsx" {
		return `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`
	}
	return `<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`
}
