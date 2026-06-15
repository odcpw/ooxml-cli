package opc

import (
	"archive/zip"
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestPackageReadXMLPartReturnsIndependentCopies(t *testing.T) {
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	doc1, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read XML part: %v", err)
	}
	doc1.Root().CreateAttr("demo", "changed")

	if pkg.IsDirty() {
		t.Fatal("ReadXMLPart mutation should not mark package dirty")
	}

	doc2, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to reread XML part: %v", err)
	}
	if got := doc2.Root().SelectAttrValue("demo", ""); got != "" {
		t.Fatalf("expected cached XML read to be isolated, got demo=%q", got)
	}
}

func TestPackageReadRawPartReturnsIndependentCopies(t *testing.T) {
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	raw1, err := pkg.ReadRawPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read raw part: %v", err)
	}
	raw1[0] = 'X'

	if pkg.IsDirty() {
		t.Fatal("ReadRawPart mutation should not mark package dirty")
	}

	raw2, err := pkg.ReadRawPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to reread raw part: %v", err)
	}
	if raw2[0] == 'X' {
		t.Fatal("expected raw part reads to return defensive copies")
	}
}

func TestPackageListRelationshipsReturnsIndependentCopies(t *testing.T) {
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	rels1 := pkg.ListRelationships("/ppt/slides/slide1.xml")
	if len(rels1) == 0 {
		t.Fatal("expected slide relationships")
	}
	rels1[0].Target = "mutated"

	rels2 := pkg.ListRelationships("/ppt/slides/slide1.xml")
	if rels2[0].Target == "mutated" {
		t.Fatal("expected ListRelationships to return a defensive copy")
	}
}

func TestPackageSaveAsPreservesCompressionAndAvoidsDataDescriptors(t *testing.T) {
	inputPath := fixturePath("..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	outPath := tempPackagePath(t)
	defer os.Remove(outPath)

	if err := pkg.SaveAs(outPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	inputMethods, err := zipMethodsByName(inputPath)
	if err != nil {
		t.Fatalf("failed to inspect input zip: %v", err)
	}
	outputInfo, err := zipInfoByName(outPath)
	if err != nil {
		t.Fatalf("failed to inspect output zip: %v", err)
	}

	for name, method := range inputMethods {
		info, ok := outputInfo[name]
		if !ok {
			t.Fatalf("missing output zip entry %s", name)
		}
		if info.Method != method {
			t.Fatalf("zip method mismatch for %s: input=%d output=%d", name, method, info.Method)
		}
		if info.Flags&0x8 != 0 {
			t.Fatalf("zip entry %s unexpectedly uses data descriptor flag: 0x%x", name, info.Flags)
		}
	}

	inputStat, err := os.Stat(inputPath)
	if err != nil {
		t.Fatalf("failed to stat input package: %v", err)
	}
	outputStat, err := os.Stat(outPath)
	if err != nil {
		t.Fatalf("failed to stat output package: %v", err)
	}
	if outputStat.Size() > inputStat.Size()*2 {
		t.Fatalf("output package unexpectedly bloated: input=%d output=%d", inputStat.Size(), outputStat.Size())
	}
}

func TestPackageOpenBytesAndWriteToBytesRoundTrip(t *testing.T) {
	inputPath := fixturePath("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	raw, err := os.ReadFile(inputPath)
	if err != nil {
		t.Fatalf("failed to read fixture: %v", err)
	}
	pkg, err := OpenBytes(raw)
	if err != nil {
		t.Fatalf("OpenBytes failed: %v", err)
	}
	defer pkg.Close()

	if err := pkg.ReplaceRawPart("/custom/agent.txt", []byte("updated"), "text/plain"); err != nil {
		t.Fatalf("ReplaceRawPart failed: %v", err)
	}
	rewritten, err := pkg.WriteToBytes()
	if err != nil {
		t.Fatalf("WriteToBytes failed: %v", err)
	}

	roundTrip, err := OpenBytes(rewritten)
	if err != nil {
		t.Fatalf("OpenBytes(rewritten) failed: %v", err)
	}
	defer roundTrip.Close()
	got, err := roundTrip.ReadRawPart("/custom/agent.txt")
	if err != nil {
		t.Fatalf("failed to read rewritten custom part: %v", err)
	}
	if string(got) != "updated" {
		t.Fatalf("custom part = %q, want updated", got)
	}
	if roundTrip.GetContentType("/custom/agent.txt") != "text/plain" {
		t.Fatalf("custom part content type = %q", roundTrip.GetContentType("/custom/agent.txt"))
	}
}

func TestPackageSaveAsPreservesAddedPartOrder(t *testing.T) {
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	inserted := []string{
		"/custom/a.txt",
		"/custom/b.txt",
		"/custom/c.txt",
		"/custom/d.txt",
		"/custom/e.txt",
	}
	for _, uri := range inserted {
		if err := pkg.AddPart(uri, []byte(uri), "text/plain", nil); err != nil {
			t.Fatalf("failed to add %s: %v", uri, err)
		}
	}

	outPath := tempPackagePath(t)
	defer os.Remove(outPath)
	if err := pkg.SaveAs(outPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	zr, err := zip.OpenReader(outPath)
	if err != nil {
		t.Fatalf("failed to open output zip: %v", err)
	}
	defer zr.Close()

	var actual []string
	for _, f := range zr.File {
		name := "/" + strings.TrimPrefix(f.Name, "/")
		if strings.HasPrefix(name, "/custom/") {
			actual = append(actual, name)
		}
	}

	if len(actual) != len(inserted) {
		t.Fatalf("custom entry count mismatch: got %d want %d (%v)", len(actual), len(inserted), actual)
	}
	for i := range inserted {
		if actual[i] != inserted[i] {
			t.Fatalf("custom entry order mismatch at %d: got %s want %s (all=%v)", i, actual[i], inserted[i], actual)
		}
	}
}

func TestPackageSaveAsUsesValidDefaultTimestampForAddedParts(t *testing.T) {
	pkg := openPackageFixture(t, "..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	defer pkg.Close()

	if err := pkg.AddPart("/xl/charts/chart1.xml", []byte("<chart/>"), "application/vnd.openxmlformats-officedocument.drawingml.chart+xml", nil); err != nil {
		t.Fatalf("failed to add chart part: %v", err)
	}

	outPath := tempPackagePath(t)
	defer os.Remove(outPath)
	if err := pkg.SaveAs(outPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	zr, err := zip.OpenReader(outPath)
	if err != nil {
		t.Fatalf("failed to open output zip: %v", err)
	}
	defer zr.Close()

	var chart *zip.File
	for _, f := range zr.File {
		if f.Name == "xl/charts/chart1.xml" {
			chart = f
			break
		}
	}
	if chart == nil {
		t.Fatal("added chart part missing from output zip")
	}
	minValid := time.Date(1980, time.January, 1, 0, 0, 0, 0, time.UTC)
	if chart.Modified.Before(minValid) {
		t.Fatalf("added part timestamp = %s, want at least %s", chart.Modified, minValid)
	}
}

func TestPackageWarningsCaptureMalformedRelationships(t *testing.T) {
	path := tempPackagePath(t)
	defer os.Remove(path)

	if err := writeMalformedRelationshipsPackage(path); err != nil {
		t.Fatalf("failed to write malformed package: %v", err)
	}

	pkg, err := Open(path)
	if err != nil {
		t.Fatalf("failed to open malformed package: %v", err)
	}
	defer pkg.Close()

	warnings := pkg.Warnings()
	if len(warnings) == 0 {
		t.Fatal("expected malformed .rels warning")
	}
	if !strings.Contains(warnings[0], "/_rels/.rels") {
		t.Fatalf("expected warning to mention .rels path, got %q", warnings[0])
	}
}

func TestPackageIsDirtyTransitionsForAddedPartLifecycle(t *testing.T) {
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	if pkg.IsDirty() {
		t.Fatal("freshly opened package should not be dirty")
	}
	if err := pkg.AddPart("/custom/temp.txt", []byte("temp"), "text/plain", nil); err != nil {
		t.Fatalf("failed to add part: %v", err)
	}
	if !pkg.IsDirty() {
		t.Fatal("package should be dirty after AddPart")
	}
	if err := pkg.RemovePart("/custom/temp.txt"); err != nil {
		t.Fatalf("failed to remove added part: %v", err)
	}
	if pkg.IsDirty() {
		t.Fatal("package should be clean after add/remove of the same staged part")
	}
}

func openPackageFixture(t *testing.T, parts ...string) *Package {
	t.Helper()
	path := fixturePath(parts...)
	pkg, err := Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture package %s: %v", path, err)
	}
	return pkg
}

func fixturePath(parts ...string) string {
	return filepath.Join(parts...)
}

func tempPackagePath(t *testing.T) string {
	t.Helper()
	file, err := os.CreateTemp("", "opc-package-*.pptx")
	if err != nil {
		t.Fatalf("failed to create temp package: %v", err)
	}
	path := file.Name()
	if err := file.Close(); err != nil {
		t.Fatalf("failed to close temp package: %v", err)
	}
	return path
}

type zipEntryInfo struct {
	Method uint16
	Flags  uint16
}

func zipMethodsByName(path string) (map[string]uint16, error) {
	zr, err := zip.OpenReader(path)
	if err != nil {
		return nil, err
	}
	defer zr.Close()

	result := make(map[string]uint16, len(zr.File))
	for _, f := range zr.File {
		result["/"+strings.TrimPrefix(f.Name, "/")] = f.Method
	}
	return result, nil
}

func zipInfoByName(path string) (map[string]zipEntryInfo, error) {
	zr, err := zip.OpenReader(path)
	if err != nil {
		return nil, err
	}
	defer zr.Close()

	result := make(map[string]zipEntryInfo, len(zr.File))
	for _, f := range zr.File {
		result["/"+strings.TrimPrefix(f.Name, "/")] = zipEntryInfo{Method: f.Method, Flags: f.Flags}
	}
	return result, nil
}

func writeMalformedRelationshipsPackage(path string) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	zw := zip.NewWriter(file)
	defer zw.Close()

	entries := map[string][]byte{
		"[Content_Types].xml": []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>`),
		"_rels/.rels": []byte(`<Relationships><Relationship`),
	}

	for name, data := range entries {
		h := &zip.FileHeader{Name: name, Method: zip.Deflate}
		w, err := zw.CreateHeader(h)
		if err != nil {
			return err
		}
		if _, err := w.Write(data); err != nil {
			return err
		}
	}

	return zw.Close()
}

func TestPackageNoOpRoundtripPreservesPartBytes(t *testing.T) {
	inputPath := fixturePath("..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	pkg := openPackageFixture(t, "..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	defer pkg.Close()

	outPath := tempPackagePath(t)
	defer os.Remove(outPath)
	if err := pkg.SaveAs(outPath); err != nil {
		t.Fatalf("failed to save package: %v", err)
	}

	outputPkg, err := Open(outPath)
	if err != nil {
		t.Fatalf("failed to open output package: %v", err)
	}
	defer outputPkg.Close()

	for _, part := range pkg.ListParts() {
		before, err := pkg.ReadRawPart(part.URI)
		if err != nil {
			t.Fatalf("failed to read original part %s: %v", part.URI, err)
		}
		after, err := outputPkg.ReadRawPart(part.URI)
		if err != nil {
			t.Fatalf("failed to read round-tripped part %s: %v", part.URI, err)
		}
		if !bytes.Equal(before, after) {
			t.Fatalf("raw part bytes changed for %s", part.URI)
		}
	}

	inputMethods, err := zipMethodsByName(inputPath)
	if err != nil {
		t.Fatalf("failed to inspect input zip: %v", err)
	}
	outputMethods, err := zipMethodsByName(outPath)
	if err != nil {
		t.Fatalf("failed to inspect output zip: %v", err)
	}
	for name, method := range inputMethods {
		if outputMethods[name] != method {
			t.Fatalf("compression method changed for %s: input=%d output=%d", name, method, outputMethods[name])
		}
	}
}
