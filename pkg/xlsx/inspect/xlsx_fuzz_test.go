package inspect

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/sheet"
)

// FuzzXlsx fuzzes the XLSX ingest/parse surface end to end, starting from
// untrusted package bytes. It drives:
//
//   - opc.OpenBytes        (zip/OPC container ingest)
//   - inspect.ParseWorkbook / SummarizeWorkbook / ListSheets / ListDefinedNames /
//     ReadWorkbookMetadata (workbook.xml + relationship parsing)
//   - sheet.LoadContext    (shared strings via sst.ParseBytes, styles via styles.ParseBytes)
//   - sheet.Read           (worksheet cell parsing for every sheet)
//
// Returned errors are correct behavior on malformed input and are intentionally
// ignored: only a panic or a hang is treated as a bug. No assertions on output.
func FuzzXlsx(f *testing.F) {
	// Seed with the small real + corrupted XLSX fixtures shipped under testdata.
	seeds := []string{
		"../../../testdata/xlsx/minimal-workbook/workbook.xlsx",
		"../../../testdata/xlsx/corrupted-missing-worksheet/workbook.xlsx",
		"../../../testdata/xlsx/shared-strings/workbook.xlsx",
		"../../../testdata/xlsx/shared-string-runs/workbook.xlsx",
		"../../../testdata/xlsx/types-and-formulas/workbook.xlsx",
		"../../../testdata/xlsx/shared-formula/workbook.xlsx",
		"../../../testdata/xlsx/used-range/workbook.xlsx",
		"../../../testdata/xlsx/chart-workbook/workbook.xlsx",
	}
	for _, path := range seeds {
		data, err := os.ReadFile(filepath.Clean(path))
		if err != nil {
			continue
		}
		f.Add(data)
	}
	// A non-zip seed and an empty seed so the mutator explores the
	// "not a valid OPC container" branch too.
	f.Add([]byte{})
	f.Add([]byte("PK\x03\x04 not really a zip"))

	f.Fuzz(func(t *testing.T, data []byte) {
		pkg, err := opc.OpenBytes(data)
		if err != nil {
			// Bad container bytes: correct rejection, nothing further to parse.
			return
		}
		defer func() { _ = pkg.Close() }()

		// Workbook-level parsing surface. All errors are acceptable.
		_, _ = SummarizeWorkbook(pkg)
		_, _ = ReadWorkbookMetadata(pkg)
		_, _ = ListDefinedNames(pkg)

		workbook, err := ParseWorkbook(pkg)
		if err != nil || workbook == nil {
			return
		}

		// Shared-strings + styles parsing (sst.ParseBytes / styles.ParseBytes).
		ctx, err := sheet.LoadContext(pkg, workbook)
		if err != nil {
			return
		}

		// Worksheet cell parsing for every declared sheet, with data extraction on.
		opts := sheet.ReadOptions{
			IncludeData:  true,
			IncludeEmpty: true,
			MaxRows:      64,
			MaxCells:     256,
		}
		for _, ref := range workbook.Sheets {
			_, _ = sheet.Read(pkg, ref, ctx, opts)
			_, _ = ListComments(pkg, ref)
		}
	})
}
