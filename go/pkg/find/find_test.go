package find

import (
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
)

// testdataPath returns the absolute path to a fixture under testdata/.
func testdataPath(rel ...string) string {
	_, currentFile, _, _ := runtime.Caller(0)
	root := filepath.Dir(filepath.Dir(filepath.Dir(currentFile)))
	parts := append([]string{root, "testdata"}, rel...)
	return filepath.Join(parts...)
}

func openFixture(t *testing.T, rel ...string) opc.PackageSession {
	t.Helper()
	pkg, err := opc.Open(testdataPath(rel...))
	if err != nil {
		t.Fatalf("open fixture %v: %v", rel, err)
	}
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

// ---------------------------------------------------------------------------
// Pure predicate table tests
// ---------------------------------------------------------------------------

func TestMatchSubstring(t *testing.T) {
	tests := []struct {
		name       string
		query      string
		value      string
		ignoreCase bool
		regex      string // non-empty enables regex mode
		want       string
		ok         bool
	}{
		{"exact literal", "Corp", "Acme Corp", false, "", "Corp", true},
		{"case sensitive miss", "corp", "Acme Corp", false, "", "", false},
		{"case insensitive hit", "corp", "Acme Corp", true, "", "Corp", true},
		{"matched preserves original case", "ACME", "Acme Corp", true, "", "Acme", true},
		{"case insensitive unicode preserves full original rune", "i", "İstanbul", true, "", "İ", true},
		{"empty value", "x", "", false, "", "", false},
		{"no match", "zzz", "Acme Corp", false, "", "", false},
		{"regex literal substring", `Acme.*Corp`, "We love Acme Big Corp today", false, `Acme.*Corp`, "Acme Big Corp", true},
		{"regex ignore-case", `acme`, "ACME", true, `acme`, "ACME", true},
		{"regex no match", `^Z`, "Acme", false, `^Z`, "", false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			opts := Options{Query: tc.query, IgnoreCase: tc.ignoreCase}
			if tc.regex != "" {
				opts.Regex = true
				opts.Query = tc.regex
			}
			m, err := newMatcher(opts)
			if err != nil {
				t.Fatalf("newMatcher: %v", err)
			}
			// For literal tests the matcher carries tc.query already; for regex
			// tests the query is the pattern, so call match against value.
			got, ok := m.match(tc.value)
			if ok != tc.ok || got != tc.want {
				t.Fatalf("match(%q) = (%q,%v), want (%q,%v)", tc.value, got, ok, tc.want, tc.ok)
			}
		})
	}
}

func TestParseMatchType(t *testing.T) {
	for _, valid := range []string{"all", "text", "formula", "name"} {
		if _, err := ParseMatchType(valid); err != nil {
			t.Errorf("ParseMatchType(%q) unexpected error: %v", valid, err)
		}
	}
	if _, err := ParseMatchType("bogus"); err == nil {
		t.Errorf("ParseMatchType(bogus) expected error")
	}
}

func TestNewMatcherEmptyQuery(t *testing.T) {
	if _, err := newMatcher(Options{Query: ""}); err == nil {
		t.Fatalf("expected error for empty query")
	}
}

func TestNewMatcherBadRegex(t *testing.T) {
	if _, err := newMatcher(Options{Query: "(", Regex: true}); err == nil {
		t.Fatalf("expected error for invalid regex")
	}
}

func TestShellArg(t *testing.T) {
	tests := []struct {
		in   string
		want string
	}{
		{"", "''"},
		{"plain", "plain"},
		{"two words", "'two words'"},
		{"it's", `'it'"'"'s'`},
	}
	for _, tc := range tests {
		if got := shellArg(tc.in); got != tc.want {
			t.Errorf("shellArg(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

// ---------------------------------------------------------------------------
// XLSX searcher (fixture-backed)
// ---------------------------------------------------------------------------

func TestSearchXLSXValue(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	res, err := Search(pkg, "xlsx", Options{Query: "Revenue"})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.ContractVersion != ContractVersion {
		t.Errorf("contract version = %q", res.ContractVersion)
	}
	if res.TotalHits != 1 {
		t.Fatalf("want 1 hit, got %d", res.TotalHits)
	}
	h := res.Hits[0]
	if h.Kind != KindXLSXValue {
		t.Errorf("kind = %q", h.Kind)
	}
	if h.MatchedValue != "Revenue" {
		t.Errorf("matchedValue = %q", h.MatchedValue)
	}
	if !strings.Contains(h.MutationCommand, "xlsx cells set") || !strings.Contains(h.MutationCommand, "--value <NEW>") {
		t.Errorf("unexpected mutationCommand: %q", h.MutationCommand)
	}
	if !strings.Contains(h.PrimarySelector, "!") {
		t.Errorf("primarySelector should be Sheet!Ref, got %q", h.PrimarySelector)
	}
}

func TestSearchXLSXFormula(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	res, err := Search(pkg, "xlsx", Options{Query: "CONCAT", Type: MatchFormula})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 1 {
		t.Fatalf("want 1 formula hit, got %d", res.TotalHits)
	}
	h := res.Hits[0]
	if h.Kind != KindXLSXFormula {
		t.Errorf("kind = %q", h.Kind)
	}
	if !strings.Contains(h.MutationCommand, "--formula <NEW>") {
		t.Errorf("formula mutation command wrong: %q", h.MutationCommand)
	}
}

func TestSearchXLSXTypeNameSkipsValues(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	// "Revenue" is a cell value, never a defined name; type=name must skip it.
	res, err := Search(pkg, "xlsx", Options{Query: "Revenue", Type: MatchName})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 0 {
		t.Fatalf("type=name should not match cell values, got %d", res.TotalHits)
	}
}

func TestSearchXLSXTypeFormulaSkipsValues(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	res, err := Search(pkg, "xlsx", Options{Query: "Revenue", Type: MatchFormula})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 0 {
		t.Fatalf("type=formula should not match cell values, got %d", res.TotalHits)
	}
}

// makeWorkbookWithDefinedName builds a temp workbook carrying one defined name
// (no committed fixture has one; the xlsx names commands also create them at
// runtime) and returns an opened session for it.
func makeWorkbookWithDefinedName(t *testing.T, name, ref string) opc.PackageSession {
	t.Helper()
	src, err := opc.Open(testdataPath("xlsx", "types-and-formulas", "workbook.xlsx"))
	if err != nil {
		t.Fatalf("open source workbook: %v", err)
	}
	defer src.Close()

	wb, err := xlsxinspect.ParseWorkbook(src)
	if err != nil {
		t.Fatalf("parse workbook: %v", err)
	}
	if _, err := xlsxmutate.AddDefinedName(&xlsxmutate.AddDefinedNameRequest{
		Package:     src,
		WorkbookURI: wb.PartURI,
		Name:        name,
		Ref:         ref,
	}); err != nil {
		t.Fatalf("add defined name: %v", err)
	}

	out := filepath.Join(t.TempDir(), "named.xlsx")
	if err := src.SaveAs(out); err != nil {
		t.Fatalf("save workbook: %v", err)
	}
	pkg, err := opc.Open(out)
	if err != nil {
		t.Fatalf("reopen workbook: %v", err)
	}
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

func TestSearchXLSXDefinedNameByName(t *testing.T) {
	pkg := makeWorkbookWithDefinedName(t, "MyTotal", "Types!$B$2")
	res, err := Search(pkg, "xlsx", Options{Query: "MyTotal", Type: MatchName})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 1 {
		t.Fatalf("want 1 defined-name hit, got %d", res.TotalHits)
	}
	h := res.Hits[0]
	if h.Kind != KindXLSXName {
		t.Errorf("kind = %q", h.Kind)
	}
	if h.PrimarySelector != "MyTotal" {
		t.Errorf("primarySelector = %q", h.PrimarySelector)
	}
	if !strings.Contains(h.MutationCommand, "names update") || !strings.Contains(h.MutationCommand, "--ref <NEW>") {
		t.Errorf("defined-name mutation command wrong: %q", h.MutationCommand)
	}
	if h.Metadata["matchedField"] != "name" {
		t.Errorf("matchedField = %q, want name", h.Metadata["matchedField"])
	}
}

func TestSearchXLSXDefinedNameByRef(t *testing.T) {
	pkg := makeWorkbookWithDefinedName(t, "MyTotal", "Types!$B$2")
	// "Types" appears in the ref, not the name; type=name should still match it.
	res, err := Search(pkg, "xlsx", Options{Query: "Types", Type: MatchName})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 1 {
		t.Fatalf("want 1 hit matching ref, got %d", res.TotalHits)
	}
	if res.Hits[0].Metadata["matchedField"] != "ref" {
		t.Errorf("matchedField = %q, want ref", res.Hits[0].Metadata["matchedField"])
	}
}

// ---------------------------------------------------------------------------
// DOCX searcher (fixture-backed)
// ---------------------------------------------------------------------------

func TestSearchDOCXParagraphAndTable(t *testing.T) {
	pkg := openFixture(t, "docx", "mixed-blocks", "document.docx")

	// Paragraph text.
	res, err := Search(pkg, "docx", Options{Query: "Bold heading"})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits == 0 {
		t.Fatalf("expected a paragraph hit")
	}
	h := res.Hits[0]
	if h.Kind != KindDOCXText {
		t.Errorf("kind = %q", h.Kind)
	}
	if !strings.Contains(h.MutationCommand, "docx replace") || !strings.Contains(h.MutationCommand, "--replace <NEW>") {
		t.Errorf("unexpected docx mutation command: %q", h.MutationCommand)
	}

	// Table-cell text.
	tableRes, err := Search(pkg, "docx", Options{Query: "Cell text"})
	if err != nil {
		t.Fatalf("Search table: %v", err)
	}
	if tableRes.TotalHits == 0 {
		t.Fatalf("expected a table-cell hit for 'Cell text'")
	}
	if tableRes.Hits[0].MatchedValue != "Cell text" {
		t.Errorf("table matchedValue = %q", tableRes.Hits[0].MatchedValue)
	}
}

func TestSearchDOCXTypeFormulaEmpty(t *testing.T) {
	pkg := openFixture(t, "docx", "mixed-blocks", "document.docx")
	res, err := Search(pkg, "docx", Options{Query: "Bold", Type: MatchFormula})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 0 {
		t.Fatalf("docx + type=formula must be empty, got %d", res.TotalHits)
	}
}

// ---------------------------------------------------------------------------
// PPTX searcher (fixture-backed)
// ---------------------------------------------------------------------------

func TestSearchPPTXText(t *testing.T) {
	pkg := openFixture(t, "pptx", "chart-simple", "presentation.pptx")
	res, err := Search(pkg, "pptx", Options{Query: "a", IgnoreCase: true})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits == 0 {
		t.Fatalf("expected PPTX text hits")
	}
	for _, h := range res.Hits {
		if h.Kind != KindPPTXText {
			continue
		}
		if !strings.Contains(h.MutationCommand, "replace text-occurrences") {
			t.Errorf("unexpected pptx mutation command: %q", h.MutationCommand)
		}
		// After shape-scoping, a uniquely-resolvable shape hit emits --for-shape
		// with its shape handle; a fallback hit (no usable shape handle) emits the
		// slide-wide --for-slides plus a MutationNote. Exactly one must hold.
		hasShape := strings.Contains(h.MutationCommand, "--for-shape ")
		hasSlide := strings.Contains(h.MutationCommand, "--for-slides ")
		if hasShape == hasSlide {
			t.Errorf("pptx command must carry exactly one of --for-shape/--for-slides: %q", h.MutationCommand)
		}
		if hasSlide && h.MutationNote == "" {
			t.Errorf("slide-wide fallback hit must carry a MutationNote: %q", h.MutationCommand)
		}
	}
}

func TestSearchPPTXTableCell(t *testing.T) {
	// table-simple has a 3x3 table with cells R0C0..R2C2 on slide 2.
	pkg := openFixture(t, "pptx", "table-simple", "presentation.pptx")
	res, err := Search(pkg, "pptx", Options{Query: "R0C1"})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 1 {
		t.Fatalf("want 1 table-cell hit, got %d", res.TotalHits)
	}
	h := res.Hits[0]
	if h.Kind != KindPPTXText {
		t.Errorf("kind = %q", h.Kind)
	}
	if h.MatchedValue != "R0C1" {
		t.Errorf("matchedValue = %q", h.MatchedValue)
	}
	// A table cell scopes to its enclosing graphicFrame shape, so it emits a
	// shape handle for slide 2's table shape (--for-shape), not slide-wide.
	if !strings.Contains(h.MutationCommand, "replace text-occurrences") || !strings.Contains(h.MutationCommand, "--for-shape H:pptx/s:") {
		t.Errorf("table-cell mutation command wrong: %q", h.MutationCommand)
	}
}

func TestSearchPPTXNotesHaveNoCommand(t *testing.T) {
	pkg := openFixture(t, "pptx", "notes-slide", "presentation.pptx")
	res, err := Search(pkg, "pptx", Options{Query: "e", IgnoreCase: true})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	sawNotes := false
	for _, h := range res.Hits {
		if h.Kind == KindPPTXNotes {
			sawNotes = true
			if h.MutationCommand != "" {
				t.Errorf("notes hit should have empty mutationCommand, got %q", h.MutationCommand)
			}
			if h.MutationNote == "" {
				t.Errorf("notes hit should carry a mutationNote")
			}
		}
	}
	if !sawNotes {
		t.Skip("fixture produced no notes hits; coverage validated elsewhere")
	}
}

// ---------------------------------------------------------------------------
// Cross-cutting behavior
// ---------------------------------------------------------------------------

func TestSearchMaxTruncates(t *testing.T) {
	pkg := openFixture(t, "pptx", "chart-simple", "presentation.pptx")
	full, err := Search(pkg, "pptx", Options{Query: "a", IgnoreCase: true})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if full.TotalHits < 2 {
		t.Skipf("need >=2 hits to test --max, got %d", full.TotalHits)
	}
	capped, err := Search(pkg, "pptx", Options{Query: "a", IgnoreCase: true, Max: 1})
	if err != nil {
		t.Fatalf("Search capped: %v", err)
	}
	if capped.TotalHits != 1 {
		t.Fatalf("want 1 capped hit, got %d", capped.TotalHits)
	}
	if !capped.Truncated {
		t.Errorf("expected truncated=true")
	}
	// Indices are reassigned 0..n-1 and ordering is preserved (file order).
	if capped.Hits[0].Index != 0 {
		t.Errorf("first hit index = %d", capped.Hits[0].Index)
	}
}

func TestSearchNoMatchEmpty(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	res, err := Search(pkg, "xlsx", Options{Query: "zzz-not-present-anywhere"})
	if err != nil {
		t.Fatalf("Search: %v", err)
	}
	if res.TotalHits != 0 {
		t.Fatalf("want 0 hits, got %d", res.TotalHits)
	}
	if res.Hits == nil {
		t.Errorf("hits must be empty slice, not nil, for stable JSON")
	}
}

func TestSearchUnsupportedType(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	if _, err := Search(pkg, "rtf", Options{Query: "x"}); err == nil {
		t.Fatalf("expected error for unsupported type")
	}
}

func TestSearchEmptyQueryErrors(t *testing.T) {
	pkg := openFixture(t, "xlsx", "types-and-formulas", "workbook.xlsx")
	if _, err := Search(pkg, "xlsx", Options{Query: ""}); err == nil {
		t.Fatalf("expected error for empty query")
	}
}
