package inspect

import (
	"os"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// addDocxSeed reads a seed file relative to the package directory and registers
// it with the fuzzer. Missing files are ignored so the harness still runs when
// a fixture is absent in some checkout.
func addDocxSeed(f *testing.F, relPath string) {
	f.Helper()
	data, err := os.ReadFile(relPath)
	if err != nil {
		return
	}
	f.Add(data)
}

// FuzzDocx fuzzes the DOCX ingest/parse surface: raw untrusted bytes are first
// opened as an OPC/ZIP package (opc.OpenBytes), then driven through the full
// pkg/docx document-model parse layer — main-part discovery, document parsing,
// summarization, body counting, styles parsing, and the extract/text +
// extract/blocks readers.
//
// A returned error is CORRECT behavior for malformed input; the harness ignores
// all returned errors and never asserts on outputs. Only a panic or hang in the
// parse path is a bug, which Go's fuzzer surfaces as a crash.
func FuzzDocx(f *testing.F) {
	// Real DOCX seeds — small, well-formed packages that exercise distinct
	// parse paths (tables, comments, media, styles, fields, headers, dup
	// paraIds, default namespace, split runs, hyperlinks).
	addDocxSeed(f, "../../../testdata/docx/minimal/document.docx")
	addDocxSeed(f, "../../../testdata/docx/table/document.docx")
	addDocxSeed(f, "../../../testdata/docx/with-comments/document.docx")
	addDocxSeed(f, "../../../testdata/docx/with-image/document.docx")
	addDocxSeed(f, "../../../testdata/docx/with-media/document.docx")
	addDocxSeed(f, "../../../testdata/docx/styles-catalog/document.docx")
	addDocxSeed(f, "../../../testdata/docx/with-fields/document.docx")
	addDocxSeed(f, "../../../testdata/docx/headers/document.docx")
	addDocxSeed(f, "../../../testdata/docx/hyperlink/document.docx")
	addDocxSeed(f, "../../../testdata/docx/paraid-dup/document.docx")
	addDocxSeed(f, "../../../testdata/docx/default-ns/document.docx")
	addDocxSeed(f, "../../../testdata/docx/split-runs/document.docx")
	addDocxSeed(f, "../../../testdata/docx/merged-table/document.docx")
	addDocxSeed(f, "../../../testdata/docx/styled-headings/document.docx")
	addDocxSeed(f, "../../../testdata/docx/space-preserve/document.docx")

	// Adversarial seed: a real package with the main document part removed.
	addDocxSeed(f, "../../../testdata/docx/corrupted-missing-document/document.docx")

	// Degenerate seeds that do not depend on fixtures.
	f.Add([]byte(""))
	f.Add([]byte("PK\x03\x04"))
	f.Add([]byte("not a zip at all"))

	f.Fuzz(func(t *testing.T, data []byte) {
		// Stage 1: OPC/ZIP ingest. A malformed payload simply errors.
		session, err := opc.OpenBytes(data)
		if err != nil {
			return
		}
		defer func() { _ = session.Close() }()

		// Stage 2: DOCX main-part discovery + document-model parse. Each of
		// these takes the untrusted package and walks its XML; errors are the
		// expected outcome for adversarial input and are deliberately ignored.
		documentURI, partErr := FindMainDocumentPart(session)

		if _, err := ParseDocument(session); err == nil {
			_ = err
		}
		if _, err := SummarizeDocument(session); err == nil {
			_ = err
		}

		// Stage 3: body + styles + extract parsers, only when a main document
		// part was located (they require a document URI).
		if partErr == nil && documentURI != "" {
			_, _, _, _, _ = CountBody(session, documentURI)

			if _, err := extract.ExtractText(&extract.ExtractTextRequest{
				Session:     session,
				DocumentURI: documentURI,
			}); err == nil {
				_ = err
			}
			if _, err := extract.ExtractBlocks(&extract.ExtractBlocksRequest{
				Session:     session,
				DocumentURI: documentURI,
				IncludeRuns: true,
			}); err == nil {
				_ = err
			}
		}

		// Styles parsing keyed off the document's resolved styles relationship.
		if doc, err := ParseDocument(session); err == nil && doc != nil && doc.StylesURI != "" {
			_, _ = ParseStyles(session, doc.StylesURI)
		}
	})
}
