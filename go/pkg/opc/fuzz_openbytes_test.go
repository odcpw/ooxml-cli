package opc

import (
	"archive/zip"
	"bytes"
	"os"
	"testing"
)

// addFileSeed reads a seed file relative to the package directory and registers
// it with the fuzzer. Missing files are ignored so the harness still runs when
// a fixture is absent.
func addFileSeed(f *testing.F, relPath string) {
	f.Helper()
	data, err := os.ReadFile(relPath)
	if err != nil {
		return
	}
	f.Add(data)
}

// minimalZip builds a tiny valid OPC-ish package as a deterministic seed that
// does not depend on external fixtures.
func minimalZip(tb testing.TB) []byte {
	tb.Helper()
	var buf bytes.Buffer
	zw := zip.NewWriter(&buf)
	entries := map[string]string{
		"[Content_Types].xml": `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
			`<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">` +
			`<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>` +
			`<Default Extension="xml" ContentType="application/xml"/>` +
			`</Types>`,
		"_rels/.rels": `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
			`<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">` +
			`<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="doc.xml"/>` +
			`</Relationships>`,
		"doc.xml": `<?xml version="1.0"?><root/>`,
	}
	for name, body := range entries {
		w, err := zw.Create(name)
		if err != nil {
			tb.Fatalf("zip create %s: %v", name, err)
		}
		if _, err := w.Write([]byte(body)); err != nil {
			tb.Fatalf("zip write %s: %v", name, err)
		}
	}
	if err := zw.Close(); err != nil {
		tb.Fatalf("zip close: %v", err)
	}
	return buf.Bytes()
}

// FuzzOpenBytes fuzzes the OPC/ZIP package loader — the primary untrusted Office
// file ingestion entry point. A returned error is correct behavior; only a panic
// or hang is a bug, so the return values are intentionally ignored.
func FuzzOpenBytes(f *testing.F) {
	// Synthetic minimal package seed (no fixture dependency).
	f.Add(minimalZip(f))

	// Real Office package seeds — exercise the full zip + content-types + rels path.
	addFileSeed(f, "../../testdata/pptx/minimal-title/presentation.pptx")
	addFileSeed(f, "../../testdata/docx/minimal/document.docx")
	addFileSeed(f, "../../testdata/xlsx/minimal-workbook/workbook.xlsx")
	// Adversarial seeds: deliberately corrupted real packages.
	addFileSeed(f, "../../testdata/pptx/corrupted-missing-media/presentation.pptx")
	addFileSeed(f, "../../testdata/docx/corrupted-missing-document/document.docx")
	addFileSeed(f, "../../testdata/xlsx/corrupted-missing-worksheet/workbook.xlsx")

	// Degenerate seeds.
	f.Add([]byte(""))
	f.Add([]byte("PK\x03\x04"))
	f.Add([]byte("not a zip at all"))

	f.Fuzz(func(t *testing.T, data []byte) {
		pkg, err := OpenBytes(data)
		if err != nil {
			return
		}
		// Lightly exercise the loaded package without asserting on outputs.
		_ = pkg.ListParts()
	})
}
