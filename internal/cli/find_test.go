package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/spf13/pflag"

	findpkg "github.com/ooxml-cli/ooxml-cli/pkg/find"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	xlsxmutate "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
)

// resetFindFlags restores find's command-local flags. The shared root-flag
// reset helper does not touch subcommand flags, and cobra retains flag values
// across in-process Execute() calls, so each test must reset explicitly.
func resetFindFlags() {
	findType = "all"
	findIgnoreCase = false
	findRegex = false
	findMax = 0
	findToOps = false
	findReplace = ""
	findApply = false

	// find now carries composition + mutation flags whose Changed state drives
	// flag-combo validation. Cobra retains both values and Changed across
	// in-process Execute() calls, so reset every find flag explicitly.
	findCmd.Flags().VisitAll(func(f *pflag.Flag) {
		_ = findCmd.Flags().Set(f.Name, f.DefValue)
		f.Changed = false
	})
}

func runFind(t *testing.T, args ...string) (string, error) {
	t.Helper()
	resetFindFlags()
	return executeRootForXLSXTest(t, args...)
}

func decodeFindResult(t *testing.T, out string) findpkg.Result {
	t.Helper()
	var res findpkg.Result
	if err := json.Unmarshal([]byte(out), &res); err != nil {
		t.Fatalf("failed to decode find JSON: %v\noutput: %s", err, out)
	}
	return res
}

const (
	xlsxFindFixture = "../../testdata/xlsx/types-and-formulas/workbook.xlsx"
	docxFindFixture = "../../testdata/docx/mixed-blocks/document.docx"
	pptxFindFixture = "../../testdata/pptx/chart-simple/presentation.pptx"
)

func TestFindXLSXValueJSON(t *testing.T) {
	out, err := runFind(t, "--json", "find", "Revenue", xlsxFindFixture)
	if err != nil {
		t.Fatalf("find returned error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.ContractVersion != findpkg.ContractVersion {
		t.Errorf("contractVersion = %q", res.ContractVersion)
	}
	if res.PackageType != "xlsx" {
		t.Errorf("packageType = %q", res.PackageType)
	}
	if res.TotalHits != 1 {
		t.Fatalf("want 1 hit, got %d", res.TotalHits)
	}
	if res.Hits[0].Kind != findpkg.KindXLSXValue {
		t.Errorf("kind = %q", res.Hits[0].Kind)
	}
	if !strings.Contains(res.Hits[0].MutationCommand, "xlsx cells set") {
		t.Errorf("mutationCommand = %q", res.Hits[0].MutationCommand)
	}
	if !strings.HasPrefix(res.Hits[0].MutationCommand, "ooxml --json ") {
		t.Errorf("mutationCommand should be JSON-first, got %q", res.Hits[0].MutationCommand)
	}
}

func TestFindXLSXFormulaType(t *testing.T) {
	out, err := runFind(t, "--json", "find", "CONCAT", xlsxFindFixture, "--type", "formula")
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.TotalHits != 1 || res.Hits[0].Kind != findpkg.KindXLSXFormula {
		t.Fatalf("want 1 formula hit, got %d", res.TotalHits)
	}
}

func TestFindDOCXText(t *testing.T) {
	out, err := runFind(t, "--json", "find", "Bold heading", docxFindFixture)
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.PackageType != "docx" || res.TotalHits == 0 {
		t.Fatalf("expected docx hits, got type=%s hits=%d", res.PackageType, res.TotalHits)
	}
	if !strings.Contains(res.Hits[0].MutationCommand, "docx replace") {
		t.Errorf("mutationCommand = %q", res.Hits[0].MutationCommand)
	}
}

func TestFindPPTXTextWithMax(t *testing.T) {
	out, err := runFind(t, "--json", "find", "a", pptxFindFixture, "--ignore-case", "--max", "1")
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.PackageType != "pptx" {
		t.Errorf("packageType = %q", res.PackageType)
	}
	if res.TotalHits > 1 {
		t.Fatalf("--max 1 not honored, got %d hits", res.TotalHits)
	}
}

func TestFindRegexIgnoreCase(t *testing.T) {
	out, err := runFind(t, "--json", "find", "rev.*nue", xlsxFindFixture, "--regex", "--ignore-case")
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.TotalHits == 0 {
		t.Fatalf("expected regex hit for rev.*nue")
	}
	if res.Hits[0].MatchedValue != "Revenue" {
		t.Errorf("regex matchedValue should be the literal substring, got %q", res.Hits[0].MatchedValue)
	}
}

func TestFindZeroHitsExitZero(t *testing.T) {
	out, err := runFind(t, "--json", "find", "definitely-not-present-xyz", xlsxFindFixture)
	if err != nil {
		t.Fatalf("zero hits must not be an error, got %v", err)
	}
	res := decodeFindResult(t, out)
	if res.TotalHits != 0 {
		t.Errorf("want 0 hits, got %d", res.TotalHits)
	}
	if !strings.Contains(out, "\"hits\":[]") {
		t.Errorf("expected empty hits array in JSON, got: %s", out)
	}
}

func TestFindFileNotFound(t *testing.T) {
	_, err := runFind(t, "find", "x", "../../testdata/does-not-exist.xlsx")
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T: %v", err, err)
	}
	if cliErr.ExitCode != ExitFileNotFound {
		t.Errorf("exit code = %d, want %d", cliErr.ExitCode, ExitFileNotFound)
	}
}

func TestFindInvalidType(t *testing.T) {
	_, err := runFind(t, "find", "x", xlsxFindFixture, "--type", "bogus")
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T: %v", err, err)
	}
	if cliErr.ExitCode != ExitInvalidArgs {
		t.Errorf("exit code = %d, want %d", cliErr.ExitCode, ExitInvalidArgs)
	}
}

func TestFindEmptyQuery(t *testing.T) {
	_, err := runFind(t, "find", "", xlsxFindFixture)
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T: %v", err, err)
	}
	if cliErr.ExitCode != ExitInvalidArgs {
		t.Errorf("exit code = %d, want %d", cliErr.ExitCode, ExitInvalidArgs)
	}
}

func TestFindBadRegex(t *testing.T) {
	_, err := runFind(t, "find", "(", xlsxFindFixture, "--regex")
	cliErr, ok := AsCLIError(err)
	if !ok {
		t.Fatalf("expected CLIError, got %T: %v", err, err)
	}
	if cliErr.ExitCode != ExitInvalidArgs {
		t.Errorf("exit code = %d, want %d", cliErr.ExitCode, ExitInvalidArgs)
	}
}

func TestFindTextOutput(t *testing.T) {
	out, err := runFind(t, "find", "Revenue", xlsxFindFixture)
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	if !strings.Contains(out, "Revenue") || !strings.Contains(out, "xlsx cells set") {
		t.Errorf("text output missing expected content: %s", out)
	}
}

// writeNamedWorkbook builds a temp workbook with one defined name and returns
// its path (no committed fixture carries a defined name).
func writeNamedWorkbook(t *testing.T, name, ref string) string {
	t.Helper()
	src, err := opc.Open(xlsxFindFixture)
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
	return out
}

func TestFindXLSXDefinedNameJSON(t *testing.T) {
	path := writeNamedWorkbook(t, "MyTotal", "Types!$B$2")
	out, err := runFind(t, "--json", "find", "MyTotal", path, "--type", "name")
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.TotalHits != 1 || res.Hits[0].Kind != findpkg.KindXLSXName {
		t.Fatalf("want 1 defined-name hit, got %d", res.TotalHits)
	}
	if !strings.Contains(res.Hits[0].MutationCommand, "names update") {
		t.Errorf("mutationCommand = %q", res.Hits[0].MutationCommand)
	}
}

// TestFindSurfacesHandleFieldUniformly is the cross-format consistency proof:
// find surfaces a stable handle under the SAME json field name ("handle") for
// PPTX (slide handle) and XLSX (cell handle) hits, and for a XLSX defined name
// (workbook-scoped name handle). The field is additive next to the unchanged
// primarySelector/selectors. This keeps an agent's handle extraction identical
// across formats.
func TestFindSurfacesHandleFieldUniformly(t *testing.T) {
	cases := []struct {
		name       string
		args       []string
		wantPrefix string
	}{
		{"pptx slide handle", []string{"--json", "find", "a", pptxFindFixture, "--ignore-case", "--max", "1"}, "H:pptx/s:"},
		{"xlsx cell handle", []string{"--json", "find", "Revenue", xlsxFindFixture}, "H:xlsx/ws:"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			out, err := runFind(t, tc.args...)
			if err != nil {
				t.Fatalf("find error: %v", err)
			}
			res := decodeFindResult(t, out)
			if res.TotalHits == 0 {
				t.Fatalf("expected at least one hit")
			}
			h := res.Hits[0]
			if h.Handle == "" || !strings.HasPrefix(h.Handle, tc.wantPrefix) {
				t.Fatalf("handle = %q, want prefix %q", h.Handle, tc.wantPrefix)
			}
			// The handle field name in the wire JSON must be exactly "handle".
			var raw struct {
				Hits []map[string]json.RawMessage `json:"hits"`
			}
			if err := json.Unmarshal([]byte(out), &raw); err != nil {
				t.Fatalf("raw decode: %v", err)
			}
			if _, ok := raw.Hits[0]["handle"]; !ok {
				t.Fatalf("hit JSON missing the canonical 'handle' field: %v", raw.Hits[0])
			}
		})
	}

	// XLSX workbook-scoped defined name also surfaces a name handle.
	path := writeNamedWorkbook(t, "MyTotal", "Types!$B$2")
	out, err := runFind(t, "--json", "find", "MyTotal", path, "--type", "name")
	if err != nil {
		t.Fatalf("find name error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.TotalHits != 1 || !strings.HasPrefix(res.Hits[0].Handle, "H:xlsx/wb/name:") {
		t.Fatalf("defined-name handle = %q", res.Hits[0].Handle)
	}
}

// TestFindOmitsHandleWhenAbsent confirms the omitempty contract is uniform: a
// DOCX text hit (no pre-existing w14:paraId; find is read-only and never injects)
// surfaces NO handle field at all, rather than an empty string.
func TestFindOmitsHandleWhenAbsent(t *testing.T) {
	out, err := runFind(t, "--json", "find", "Bold heading", docxFindFixture)
	if err != nil {
		t.Fatalf("find error: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.TotalHits == 0 {
		t.Fatalf("expected docx hits")
	}
	if res.Hits[0].Handle != "" {
		t.Fatalf("docx text hit unexpectedly carries a handle: %q", res.Hits[0].Handle)
	}
	var raw struct {
		Hits []map[string]json.RawMessage `json:"hits"`
	}
	if err := json.Unmarshal([]byte(out), &raw); err != nil {
		t.Fatalf("raw decode: %v", err)
	}
	if _, present := raw.Hits[0]["handle"]; present {
		t.Fatalf("absent handle must be omitted (omitempty), but field is present: %v", raw.Hits[0])
	}
}

func TestFindCapabilities(t *testing.T) {
	out, err := runFind(t, "--json", "find", "capabilities")
	if err != nil {
		t.Fatalf("capabilities error: %v", err)
	}
	var caps findCapabilities
	if err := json.Unmarshal([]byte(out), &caps); err != nil {
		t.Fatalf("decode capabilities: %v\n%s", err, out)
	}
	if caps.ContractVersion != findpkg.ContractVersion {
		t.Errorf("capabilities contractVersion = %q", caps.ContractVersion)
	}
	if len(caps.HitKinds) == 0 || len(caps.ExitCodes) == 0 {
		t.Errorf("capabilities missing hitKinds/exitCodes")
	}
}

func TestFindReservedQueryTeachesDelimiter(t *testing.T) {
	_, err := runFind(t, "--json", "find", "capabilities", pptxFindFixture)
	if err == nil {
		t.Fatal("expected reserved query without -- delimiter to fail")
	}
	if !strings.Contains(err.Error(), "ooxml --json find -- capabilities <file>") {
		t.Fatalf("reserved query error should teach the -- delimiter, got: %v", err)
	}

	out, err := runFind(t, "--json", "find", "--", "capabilities", pptxFindFixture)
	if err != nil {
		t.Fatalf("reserved query with -- delimiter failed: %v", err)
	}
	res := decodeFindResult(t, out)
	if res.Query != "capabilities" {
		t.Fatalf("query = %q, want capabilities", res.Query)
	}
}

func TestFindRobotDocs(t *testing.T) {
	out, err := runFind(t, "find", "robot-docs")
	if err != nil {
		t.Fatalf("robot-docs error: %v", err)
	}
	if !strings.Contains(out, "semantic cross-object") {
		t.Errorf("robot-docs missing expected handbook content")
	}
}
