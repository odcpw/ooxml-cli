package inspect

import (
	"os"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// addGraphSeed reads a seed .pptx relative to this package directory and
// registers its raw bytes with the fuzzer. Missing fixtures are skipped so the
// harness still runs if a file is absent.
func addGraphSeed(f *testing.F, relPath string) {
	f.Helper()
	data, err := os.ReadFile(relPath)
	if err != nil {
		return
	}
	f.Add(data)
}

// FuzzPptxGraph fuzzes the deep OOXML slide-graph ingest path: raw untrusted
// bytes are opened as an OPC package via opc.OpenBytes and then handed to
// inspect.ParsePresentation, which walks presentation.xml, all slide masters,
// slide layouts and slides, resolving relationships into the master/layout/slide
// graph. To maximize coverage of the deep XML parsers that hang off that graph,
// the harness also traverses the resulting graph through theme parsing.
//
// A returned error is correct behavior for malformed input; the harness ignores
// all return values and only treats a panic or hang as a bug. No outputs are
// asserted.
func FuzzPptxGraph(f *testing.F) {
	// Real PPTX seeds covering distinct structural features (layouts, notes,
	// tables, themes, charts, large decks, rich text) plus adversarial
	// deliberately-corrupted packages. Each is a full ZIP/OPC payload.
	seeds := []string{
		"../../../testdata/pptx/minimal-title/presentation.pptx",
		"../../../testdata/pptx/multi-layout/presentation.pptx",
		"../../../testdata/pptx/notes-slide/presentation.pptx",
		"../../../testdata/pptx/table-slide/presentation.pptx",
		"../../../testdata/pptx/theme-custom-colors/presentation.pptx",
		"../../../testdata/pptx/chart-simple/presentation.pptx",
		"../../../testdata/pptx/edge-large-deck/presentation.pptx",
		"../../../testdata/pptx/edge-nested-groups/presentation.pptx",
		"../../../testdata/pptx/corrupted-dangling-layout/presentation.pptx",
		"../../../testdata/pptx/corrupted-missing-media/presentation.pptx",
	}
	for _, s := range seeds {
		addGraphSeed(f, s)
	}

	// Degenerate seeds: not-a-zip, empty, and a bare local-file header so the
	// fuzzer has cheap mutation starting points that fail early in OpenBytes.
	f.Add([]byte(""))
	f.Add([]byte("PK\x03\x04"))
	f.Add([]byte("not a pptx"))

	f.Fuzz(func(t *testing.T, data []byte) {
		pkg, err := opc.OpenBytes(data)
		if err != nil {
			return
		}

		// Primary deep ingest entry: build the master/layout/slide graph.
		graph, err := ParsePresentation(pkg)
		if err != nil || graph == nil {
			return
		}

		// Walk the graph to drive the deeper per-part XML parsers. All errors
		// are intentionally ignored; only panics/hangs are bugs.
		for _, m := range graph.Masters {
			if m.ThemeURI != "" {
				_, _ = ParseTheme(pkg, m.ThemeURI)
				_ = ExtractDefaultTextStyleInfo(pkg, m.ThemeURI)
			}
		}
	})
}
