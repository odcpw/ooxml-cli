package find

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// readSeed loads a fixture's raw bytes for use as a fuzz seed. A missing seed is
// skipped (not fatal) so the harness still builds and runs on a trimmed tree.
func readSeed(f *testing.F, rel ...string) {
	f.Helper()
	data, err := os.ReadFile(testdataPath(rel...))
	if err != nil {
		f.Logf("seed %v unavailable: %v", rel, err)
		return
	}
	f.Add(data)
}

// FuzzFind drives the OOXML text-search ingest surface: open untrusted package
// bytes via opc.OpenBytes, detect the package type, and run find.Search with a
// fixed query. The query is held constant so the fuzzer mutates ONLY the package
// bytes (the parse/ingest surface under test). All returned errors are ignored:
// a returned error on garbage input is correct behavior. Only a panic or a hang
// is a bug.
func FuzzFind(f *testing.F) {
	// Real, small seeds — one per supported package type — give coverage-guided
	// fuzzing a valid skeleton to mutate from, so it reaches deep parse paths fast.
	readSeed(f, "pptx", "minimal-title", "presentation.pptx")
	readSeed(f, "docx", "mixed-blocks", "document.docx")
	readSeed(f, "xlsx", "types-and-formulas", "workbook.xlsx")

	// Adversarial seeds: structurally-broken real packages exercise the
	// error/degradation paths inside the searchers (missing media/parts, dangling
	// relationships) without the fuzzer having to rediscover ZIP+OOXML framing.
	readSeed(f, "pptx", "corrupted-missing-media", "presentation.pptx")
	readSeed(f, "pptx", "corrupted-dangling-layout", "presentation.pptx")
	readSeed(f, "docx", "corrupted-missing-document", "document.docx")

	// A couple of non-package seeds to probe the OpenBytes ZIP-framing guard.
	f.Add([]byte("not a zip at all"))
	f.Add([]byte("PK\x03\x04 truncated"))

	f.Fuzz(func(t *testing.T, data []byte) {
		pkg, err := opc.OpenBytes(data)
		if err != nil {
			return // not a valid OPC/ZIP payload — correct rejection.
		}
		defer pkg.Close()

		ptype := opc.DetectType(pkg).String()

		// A fixed query string keeps the search surface (not the matcher) the
		// thing being fuzzed. Run the default search plus regex and ignore-case
		// variants so all three matchSubstring branches see the parsed package.
		base := Options{Query: "a"}
		for _, opts := range []Options{
			base,
			{Query: "a", IgnoreCase: true},
			{Query: "a.*b", Regex: true, IgnoreCase: true},
			{Query: "a", Type: MatchText, Max: 5},
			{Query: "a", Type: MatchFormula},
			{Query: "a", Type: MatchName},
		} {
			// Ignore the result and any error: a returned error is acceptable
			// behavior on adversarial input. We only care that Search does not
			// panic or hang. Nil-safe: Search never returns a nil pointer it then
			// dereferences here because we discard the result.
			_, _ = Search(pkg, ptype, opts)
		}
	})
}

// guard against unused import of filepath if testdataPath ever changes form.
var _ = filepath.Join
