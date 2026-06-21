package opc

import (
	"os"
	"testing"
)

// extractPartFromPackage opens an OPC package fixture and returns the raw bytes
// of the named part, or nil if anything fails. Used to seed fuzzers with real
// part contents pulled out of genuine Office files.
func extractPartFromPackage(relPath, partURI string) []byte {
	data, err := os.ReadFile(relPath)
	if err != nil {
		return nil
	}
	pkg, err := OpenBytes(data)
	if err != nil {
		return nil
	}
	raw, err := pkg.ReadRawPart(partURI)
	if err != nil {
		return nil
	}
	return raw
}

// FuzzParseContentTypes fuzzes the [Content_Types].xml parser. A returned error
// is correct behavior; only a panic/hang is a bug, so outputs are ignored.
func FuzzParseContentTypes(f *testing.F) {
	// Crafted minimal valid content-types part.
	f.Add([]byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
		`<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">` +
		`<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
		`<Default Extension="xml" ContentType="application/xml"/>` +
		`<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>` +
		`</Types>`))

	// Real [Content_Types].xml parts extracted from genuine packages.
	if real := extractPartFromPackage("../../testdata/pptx/minimal-title/presentation.pptx", "/[Content_Types].xml"); real != nil {
		f.Add(real)
	}
	if real := extractPartFromPackage("../../testdata/docx/minimal/document.docx", "/[Content_Types].xml"); real != nil {
		f.Add(real)
	}
	if real := extractPartFromPackage("../../testdata/xlsx/minimal-workbook/workbook.xlsx", "/[Content_Types].xml"); real != nil {
		f.Add(real)
	}

	// Degenerate and malformed seeds.
	f.Add([]byte(""))
	f.Add([]byte(`<Types></Types>`))
	f.Add([]byte(`<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default/></Types>`))
	f.Add([]byte(`<?xml version="1.0"?><not-types/>`))
	f.Add([]byte(`<Types><Override PartName=""/></Types>`))

	f.Fuzz(func(t *testing.T, data []byte) {
		reg, err := ParseContentTypes(data)
		if err != nil {
			return
		}
		// Lightly exercise the registry without asserting on outputs.
		_ = reg.GetContentType("/ppt/presentation.xml")
	})
}
