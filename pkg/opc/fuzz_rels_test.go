package opc

import (
	"testing"
)

// FuzzParseRelationships fuzzes the .rels relationship parser. The sourceURI is
// fixed to a realistic package-root value; only the untrusted XML payload is
// fuzzed. A returned error is correct behavior; only a panic/hang is a bug, so
// outputs are ignored. (Distinct from FuzzResolveRelationshipTargetNormalization
// in rels_fuzz_test.go, which fuzzes the target-resolution logic.)
func FuzzParseRelationships(f *testing.F) {
	const sourceURI = "/"

	// Crafted minimal valid .rels part.
	f.Add([]byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
		`<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
		`<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>` +
		`<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>` +
		`</Relationships>`))

	// Real .rels parts extracted from genuine packages.
	if real := extractPartFromPackage("../../testdata/pptx/minimal-title/presentation.pptx", "/_rels/.rels"); real != nil {
		f.Add(real)
	}
	if real := extractPartFromPackage("../../testdata/docx/minimal/document.docx", "/_rels/.rels"); real != nil {
		f.Add(real)
	}
	if real := extractPartFromPackage("../../testdata/xlsx/minimal-workbook/workbook.xlsx", "/_rels/.rels"); real != nil {
		f.Add(real)
	}

	// Degenerate and malformed seeds.
	f.Add([]byte(""))
	f.Add([]byte(`<Relationships></Relationships>`))
	f.Add([]byte(`<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship/></Relationships>`))
	f.Add([]byte(`<Relationships><Relationship Id="" Type="" Target="../../../etc/passwd" TargetMode="External"/></Relationships>`))
	f.Add([]byte(`<?xml version="1.0"?><not-rels/>`))

	f.Fuzz(func(t *testing.T, data []byte) {
		rels, err := ParseRelationships(sourceURI, data)
		if err != nil {
			return
		}
		// Lightly exercise each parsed relationship without asserting on outputs.
		for i := range rels {
			_ = ResolveRelationshipTarget(rels[i].SourceURI, rels[i].Target)
		}
	})
}
