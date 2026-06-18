package validate

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// seedFiles are small real OOXML packages plus adversarial corrupted fixtures.
// They are loaded relative to the package directory (pkg/validate), so the
// testdata lives two levels up at the repo root.
var seedFiles = []string{
	"../../testdata/pptx/minimal-title/presentation.pptx",
	"../../testdata/pptx/corrupted-dangling-layout/presentation.pptx",
	"../../testdata/pptx/corrupted-missing-media/presentation.pptx",
	"../../testdata/docx/minimal/document.docx",
	"../../testdata/docx/corrupted-missing-document/document.docx",
	"../../testdata/xlsx/minimal-workbook/workbook.xlsx",
	"../../testdata/xlsx/corrupted-missing-worksheet/workbook.xlsx",
}

// FuzzValidate feeds fuzzed bytes through the OOXML ingest/validation surface:
// opc.OpenBytes (ZIP/OPC parse) -> validate.ValidatePackage (5-stage validation).
//
// Both OpenBytes and ValidatePackage are allowed to return errors for malformed
// input; that is correct behavior. Only a panic or a hang is treated as a bug,
// so returned errors and returned diagnostics are deliberately ignored.
func FuzzValidate(f *testing.F) {
	for _, name := range seedFiles {
		data, err := os.ReadFile(filepath.Clean(name))
		if err != nil {
			// A missing seed should not fail the build; just skip it.
			f.Logf("skipping seed %s: %v", name, err)
			continue
		}
		f.Add(data)
	}

	// A couple of tiny synthetic seeds to exercise the non-ZIP / empty paths.
	f.Add([]byte{})
	f.Add([]byte("PK\x03\x04 not really a zip"))

	f.Fuzz(func(t *testing.T, data []byte) {
		// Stage 0: ZIP/OPC ingest. An error here is normal for garbage input.
		session, err := opc.OpenBytes(data)
		if err != nil {
			return
		}
		// Ensure resources are released even if validation panics.
		defer func() { _ = session.Close() }()

		// Run the full validation pipeline. Ignore both the diagnostics and
		// the returned error: neither is a bug. We only care about panics/hangs.
		_, _ = ValidatePackage(session)
	})
}
