package cli

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/apply"
)

// stageXLSXFindFixture copies the types-and-formulas workbook into a temp file
// so mutation tests never touch the committed fixture.
func stageXLSXFindFixture(t *testing.T) string {
	t.Helper()
	data, err := os.ReadFile(xlsxFindFixture)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	dst := filepath.Join(t.TempDir(), "input.xlsx")
	if err := os.WriteFile(dst, data, 0o644); err != nil {
		t.Fatalf("stage fixture: %v", err)
	}
	return dst
}

// TestFindToOpsEmitsApplyCompatibleArray verifies --to-ops emits a BARE JSON
// array of {command,args} that apply.ParseOps (apply's own validator) accepts.
func TestFindToOpsEmitsApplyCompatibleArray(t *testing.T) {
	out, err := runFind(t, "--json", "find", "Revenue", xlsxFindFixture, "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops: %v", err)
	}
	// Must be a bare array, and apply must accept it directly.
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("apply.ParseOps rejected --to-ops output: %v\noutput: %s", err, out)
	}
	if len(ops) != 1 {
		t.Fatalf("want 1 op, got %d (%s)", len(ops), out)
	}
	if ops[0].Command != "xlsx cells set" {
		t.Errorf("command = %q", ops[0].Command)
	}
	// Without --replace, the value carries the <NEW> placeholder.
	if got := ops[0].Args["value"].String(); got != "<NEW>" {
		t.Errorf("value arg = %q, want <NEW>", got)
	}
	if ops[0].Args["cell"].String() == "" || ops[0].Args["sheet"].String() == "" {
		t.Errorf("missing structured cell/sheet args: %+v", ops[0])
	}
}

// TestFindToOpsWithReplaceSubstitutes verifies --replace is substituted into the
// replacement argument of the emitted ops while remaining apply-compatible.
func TestFindToOpsWithReplaceSubstitutes(t *testing.T) {
	out, err := runFind(t, "--json", "find", "Revenue", xlsxFindFixture, "--replace", "Income", "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops --replace: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, out)
	}
	if len(ops) != 1 || ops[0].Args["value"].String() != "Income" {
		t.Fatalf("want value=Income, got %s", out)
	}
}

// TestFindToOpsFormulaReplaceKey verifies a formula hit uses --formula as the
// replacement key (not --value).
func TestFindToOpsFormulaReplaceKey(t *testing.T) {
	out, err := runFind(t, "--json", "find", "CONCAT", xlsxFindFixture, "--type", "formula", "--replace", "SUM(A1:A2)", "--to-ops")
	if err != nil {
		t.Fatalf("find formula --to-ops: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, out)
	}
	if len(ops) != 1 {
		t.Fatalf("want 1 op, got %d (%s)", len(ops), out)
	}
	if got := ops[0].Args["formula"].String(); got != "SUM(A1:A2)" {
		t.Errorf("formula arg = %q, want SUM(A1:A2)", got)
	}
	if _, ok := ops[0].Args["value"]; ok {
		t.Errorf("formula op should not carry a value arg: %s", out)
	}
}

// TestFindReplaceApplyDryRunPlan verifies --replace --apply --dry-run prints the
// resolved apply.Plan and executes nothing.
func TestFindReplaceApplyDryRunPlan(t *testing.T) {
	file := stageXLSXFindFixture(t)
	out, err := runFind(t, "--json", "find", "Revenue", file, "--replace", "Income", "--apply", "--dry-run")
	if err != nil {
		t.Fatalf("find --apply --dry-run: %v", err)
	}
	var plan apply.Plan
	if err := json.Unmarshal([]byte(out), &plan); err != nil {
		t.Fatalf("unmarshal plan: %v (%s)", err, out)
	}
	if !plan.DryRun || plan.OpsCount != 1 || len(plan.Plan) != 1 {
		t.Fatalf("unexpected plan: %+v", plan)
	}
	if plan.SchemaVersion != apply.SchemaVersion {
		t.Errorf("schemaVersion = %d", plan.SchemaVersion)
	}
	// The plan's argv must carry the structured replacement, not the placeholder.
	joined := plan.Plan[0].Argv
	found := false
	for i, a := range joined {
		if a == "--value" && i+1 < len(joined) && joined[i+1] == "Income" {
			found = true
		}
	}
	if !found {
		t.Errorf("plan argv missing --value Income: %v", joined)
	}
}

// TestFindReplaceApplyDryRunWritesNothing confirms --dry-run does not modify the
// input file.
func TestFindReplaceApplyDryRunWritesNothing(t *testing.T) {
	file := stageXLSXFindFixture(t)
	before, err := os.ReadFile(file)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := runFind(t, "--json", "find", "Revenue", file, "--replace", "Income", "--apply", "--dry-run"); err != nil {
		t.Fatalf("dry-run: %v", err)
	}
	after, err := os.ReadFile(file)
	if err != nil {
		t.Fatal(err)
	}
	if string(before) != string(after) {
		t.Fatalf("dry-run modified the input file")
	}
}

// useServeBinaryForFindApply injects the package-wide real ooxml binary as the
// apply engine's Self for the duration of the test (the in-process
// os.Executable() points at the test binary, which cannot dispatch ooxml
// subcommands).
func useServeBinaryForFindApply(t *testing.T) {
	t.Helper()
	if serveBinary == "" {
		t.Fatal("serveBinary was not built")
	}
	orig := findSelfExecutable
	findSelfExecutable = func() (string, error) { return serveBinary, nil }
	t.Cleanup(func() { findSelfExecutable = orig })
}

// TestFindReplaceApplyRoundTrip builds a real ooxml binary, injects it as the
// apply engine's Self, and verifies --replace --apply --out changes the file and
// the result validates (apply runs a single final validation by default).
func TestFindReplaceApplyRoundTrip(t *testing.T) {
	useServeBinaryForFindApply(t)

	file := stageXLSXFindFixture(t)
	outFile := filepath.Join(t.TempDir(), "out.xlsx")

	out, err := runFind(t, "--json", "find", "Revenue", file, "--replace", "Income", "--apply", "--out", outFile)
	if err != nil {
		t.Fatalf("find --apply --out: %v\n%s", err, out)
	}
	var result apply.Result
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal result: %v (%s)", err, out)
	}
	if result.DryRun || result.OpsCount != 1 || len(result.Applied) != 1 {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.Output != outFile {
		t.Errorf("output = %q, want %q", result.Output, outFile)
	}
	if _, statErr := os.Stat(outFile); statErr != nil {
		t.Fatalf("output file not written: %v", statErr)
	}

	// Confirm the value actually changed: "Income" now present, "Revenue" gone.
	gotIncome, err := runFind(t, "--json", "find", "Income", outFile)
	if err != nil {
		t.Fatalf("verify find Income: %v", err)
	}
	res := decodeFindResult(t, gotIncome)
	if res.TotalHits != 1 {
		t.Fatalf("want 1 Income hit after apply, got %d", res.TotalHits)
	}
	gotRevenue, err := runFind(t, "--json", "find", "Revenue", outFile)
	if err != nil {
		t.Fatalf("verify find Revenue: %v", err)
	}
	if decodeFindResult(t, gotRevenue).TotalHits != 0 {
		t.Fatalf("Revenue should be gone after apply")
	}
}

// TestFindReplaceApplyMultiOpChain exercises the rolling-temp chaining that is
// the whole point of compose: a query matching SEVERAL distinct cells produces
// several ops, each consuming the prior op's output. All targets must end up
// changed and the final package must validate. Distinct cell targets avoid the
// same-target zero-match pitfall (see report caveat).
func TestFindReplaceApplyMultiOpChain(t *testing.T) {
	useServeBinaryForFindApply(t)

	file := stageXLSXFindFixture(t)
	outFile := filepath.Join(t.TempDir(), "out.xlsx")

	// "a" (value text) matches multiple distinct cells in the fixture.
	probe, err := runFind(t, "--json", "find", "a", file, "--type", "text")
	if err != nil {
		t.Fatalf("probe find: %v", err)
	}
	nCells := decodeFindResult(t, probe).TotalHits
	if nCells < 2 {
		t.Fatalf("multi-op test needs >=2 distinct cell hits, got %d", nCells)
	}

	out, err := runFind(t, "--json", "find", "a", file, "--type", "text", "--replace", "ZZ", "--apply", "--out", outFile)
	if err != nil {
		t.Fatalf("multi-op --apply: %v\n%s", err, out)
	}
	var result apply.Result
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal result: %v (%s)", err, out)
	}
	if result.OpsCount != nCells || len(result.Applied) != nCells {
		t.Fatalf("want %d ops applied, got opsCount=%d applied=%d", nCells, result.OpsCount, len(result.Applied))
	}
	// All distinct cells should now hold the replacement; none should still be "a".
	gotZZ, err := runFind(t, "--json", "find", "ZZ", outFile)
	if err != nil {
		t.Fatalf("verify ZZ: %v", err)
	}
	if decodeFindResult(t, gotZZ).TotalHits != nCells {
		t.Fatalf("want %d ZZ hits after multi-op apply, got %d", nCells, decodeFindResult(t, gotZZ).TotalHits)
	}
	leftover, err := runFind(t, "--json", "find", "a", outFile, "--type", "text")
	if err != nil {
		t.Fatalf("verify leftover: %v", err)
	}
	if decodeFindResult(t, leftover).TotalHits != 0 {
		t.Fatalf("expected no 'a' value hits after apply, got %d", decodeFindResult(t, leftover).TotalHits)
	}
}

// TestFindToOpsEmitsCellHandleAsTarget proves the find->ops path emits a STABLE
// handle into the target arg (--cell) rather than the positional A1 ref, so an
// emitted XLSX op carries the sheetId-scoped handle.
func TestFindToOpsEmitsCellHandleAsTarget(t *testing.T) {
	out, err := runFind(t, "--json", "find", "Revenue", xlsxFindFixture, "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, out)
	}
	if len(ops) != 1 {
		t.Fatalf("want 1 op, got %d", len(ops))
	}
	if got := ops[0].Args["cell"].String(); got != "H:xlsx/ws:1/cell:a:B1" {
		t.Fatalf("cell target = %q, want the cell handle", got)
	}
}

// TestFindToOpsEmitsShapeHandleForPPTXText proves the PPTX find->ops path emits a
// SHAPE HANDLE into --for-shape (the op's target), so an emitted PPTX text op
// confines the replacement to the ONE matched shape by durable cNvPr@id within
// its durable sldId scope, rather than to the whole slide. "Content Slide" matches
// only the slide-2 (sldId 257) title shape (cNvPr id 2).
func TestFindToOpsEmitsShapeHandleForPPTXText(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	out, err := runFind(t, "--json", "find", "Content Slide", fixture, "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops on pptx: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, out)
	}
	if len(ops) != 1 {
		t.Fatalf("want 1 op, got %d (%s)", len(ops), out)
	}
	if got := ops[0].Args["for-shape"].String(); got != "H:pptx/s:257/shape:n:2" {
		t.Fatalf("for-shape target = %q, want the shape handle H:pptx/s:257/shape:n:2", got)
	}
	if _, hasSlide := ops[0].Args["for-slides"]; hasSlide {
		t.Fatalf("shape-scoped op must not carry --for-slides: %s", out)
	}
}

func TestFindToOpsDOCXParagraphHandleUsesScopedSet(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "docx", "paraid", "document.docx")
	out, err := runFind(t, "--json", "find", "marked", fixture, "--replace", "updated", "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops on docx paragraph handle: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, out)
	}
	if len(ops) != 1 {
		t.Fatalf("want 1 op, got %d (%s)", len(ops), out)
	}
	if ops[0].Command != "docx paragraphs set" {
		t.Fatalf("DOCX handled paragraph must emit scoped set op, got %q", ops[0].Command)
	}
	if got := ops[0].Args["handle"].String(); got != "H:docx/pt:doc/para:m:1A2B3C4D" {
		t.Fatalf("handle = %q", got)
	}
	if got := ops[0].Args["text"].String(); got != "First updated paragraph" {
		t.Fatalf("text = %q, want full paragraph replacement", got)
	}
}

func TestFindToOpsDOCXPlainParagraphSkipsGlobalReplace(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "docx", "paraid", "document.docx")
	out, err := runFind(t, "--json", "find", "plain", fixture, "--replace", "updated", "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops on plain docx paragraph: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, out)
	}
	if len(ops) != 0 {
		t.Fatalf("plain DOCX paragraph without handle must not emit global docx replace op: %s", out)
	}
}

// TestFindApplyPPTXTextSurvivesStructuralShift is the find->apply headline: a
// read-only find composes a HANDLE-bearing op for the slide-2 (sldId 257) text;
// then the deck's slides are structurally shifted; then the emitted op is applied
// against the shifted deck via the REAL ooxml binary. Because the op restricts
// replacement by durable sldId (not slide number), the edit lands on the correct
// slide even though it moved. This exercises the full read-only find -> handle
// emission -> apply path end to end across a structural shift.
func TestFindApplyPPTXTextSurvivesStructuralShift(t *testing.T) {
	bin := serveBinary

	src := filepath.Join("..", "..", "testdata", "pptx", "title-content", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	base := filepath.Join(t.TempDir(), "base.pptx")
	if err := os.WriteFile(base, data, 0o644); err != nil {
		t.Fatalf("stage fixture: %v", err)
	}

	// Emit the handle-bearing op for the slide-2 (sldId 257) "Content Slide" text,
	// with --replace so the op carries the real replacement value (not <NEW>).
	opsJSON, err := runFind(t, "--json", "find", "Content Slide", base, "--replace", "SURVIVED", "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops: %v", err)
	}
	ops, err := apply.ParseOps([]byte(opsJSON))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, opsJSON)
	}
	if len(ops) != 1 || ops[0].Args["for-shape"].String() != "H:pptx/s:257/shape:n:2" {
		t.Fatalf("emitted op for-shape = %q, want the shape handle H:pptx/s:257/shape:n:2", opsJSON)
	}
	if ops[0].Args["new-text"].String() != "SURVIVED" {
		t.Fatalf("emitted op new-text = %q, want SURVIVED", opsJSON)
	}

	// Structurally shift AND duplicate the matched text: clone slide 2 (the
	// target) into position 2 with a FRESH sldId, pushing the original sldId 257 to
	// position 3. Now BOTH the clone (pos 2) and the original (pos 3) carry
	// "Content Slide", so the slide handle is load-bearing: a deck-wide replace
	// would hit both, but the handle-restricted op must hit ONLY sldId 257.
	shifted := filepath.Join(t.TempDir(), "shifted.pptx")
	runBin(t, bin, "pptx", "clone-slide", base, "--slide", "2", "--insert-after", "1", "--out", shifted)

	// Apply the emitted handle op (built from the ORIGINAL deck) against the
	// shifted deck. The handle re-resolves to sldId 257 at its new position.
	applied := filepath.Join(t.TempDir(), "applied.pptx")
	opsFile := filepath.Join(t.TempDir(), "ops.json")
	if err := os.WriteFile(opsFile, []byte(opsJSON), 0o644); err != nil {
		t.Fatalf("write ops: %v", err)
	}
	runBin(t, bin, "apply", shifted, "--ops", opsFile, "--out", applied)

	// Exactly ONE shape changed: the title on sldId 257 (now at position 3). The
	// clone at position 2 still reads "Content Slide" — proving the --for-shape
	// handle restricted the replacement to the durable sldId+cNvPr id, not a
	// deck-wide replace. find now surfaces a SHAPE handle for the changed text.
	res := decodeFindResult(t, mustRunFind(t, "--json", "find", "SURVIVED", applied))
	if res.TotalHits != 1 {
		t.Fatalf("want exactly 1 SURVIVED hit (handle-restricted), got %d", res.TotalHits)
	}
	if res.Hits[0].Handle != "H:pptx/s:257/shape:n:2" {
		t.Fatalf("SURVIVED landed on the wrong shape: handle %q", res.Hits[0].Handle)
	}
	// The clone at position 2 must be UNTOUCHED.
	leftover := decodeFindResult(t, mustRunFind(t, "--json", "find", "Content Slide", applied))
	if leftover.TotalHits != 1 {
		t.Fatalf("want 1 surviving 'Content Slide' on the clone, got %d", leftover.TotalHits)
	}
	if strings.HasPrefix(leftover.Hits[0].Handle, "H:pptx/s:257/") || leftover.Hits[0].Handle == "H:pptx/s:257" {
		t.Fatalf("the untouched 'Content Slide' should be the CLONE, not sldId 257; got handle %q", leftover.Hits[0].Handle)
	}
}

// stagePPTXFixture copies a committed PPTX fixture into a temp file so mutation
// tests never touch the source fixture.
func stagePPTXFixture(t *testing.T, fixtureDir string) string {
	t.Helper()
	src := filepath.Join("..", "..", "testdata", "pptx", fixtureDir, "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read fixture %s: %v", fixtureDir, err)
	}
	dst := filepath.Join(t.TempDir(), "input.pptx")
	if err := os.WriteFile(dst, data, 0o644); err != nil {
		t.Fatalf("stage fixture %s: %v", fixtureDir, err)
	}
	return dst
}

// TestFindApplyPPTXShapeScopeIsolatesSiblingsAndDoesNotAbort is the discriminating
// END-TO-END pin for the two find->ops sins. It drives the REAL emitted path
// (find --to-ops / find --replace --apply) against title-content slide 2, where
// the substring "ontent" appears (a) in TWO sibling shapes (the title "Content
// Slide" and the body "main content area") and (b) as a SUB-WORD in each.
//
// It pins Finding 1 (cross-shape over-broadness): applying ONLY the first emitted
// op — the title shape's op — must leave the body shape's "content" UNTOUCHED. The
// pre-fix code emitted a slide-wide `--for-slides` op for that hit, which rewrote
// BOTH shapes, so this assertion fails against pre-fix behavior.
//
// It pins Finding 2 (duplicate-op abort): `find ontent --replace ... --apply`,
// with "ontent" recurring, must NOT abort. The pre-fix code emitted identical
// slide-wide ops per hit; under --apply the first replaced all and the second
// matched zero -> ErrTextOccurrencesNoMatches -> batch abort.
func TestFindApplyPPTXShapeScopeIsolatesSiblingsAndDoesNotAbort(t *testing.T) {
	useServeBinaryForFindApply(t)
	bin := serveBinary

	// --- Finding 1: apply ONLY the first (title-shape) op; sibling body untouched.
	base := stagePPTXFixture(t, "title-content")

	// Restrict to slide 2 via --max-free? find has no per-slide flag; instead emit
	// all ops and take the one targeting slide-2's title shape (sldId 257, cNvPr 2).
	opsJSON, err := runFind(t, "--json", "find", "ontent", base, "--replace", "ZZZ", "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops: %v\n%s", err, opsJSON)
	}
	ops, err := apply.ParseOps([]byte(opsJSON))
	if err != nil {
		t.Fatalf("ParseOps: %v (%s)", err, opsJSON)
	}
	// Every emitted op must be shape-scoped (no slide-wide op for a shape hit).
	for i, op := range ops {
		if _, hasShape := op.Args["for-shape"]; !hasShape {
			t.Fatalf("op %d is not shape-scoped (no --for-shape): %s", i, opsJSON)
		}
	}
	// Select the slide-2 title-shape op and the slide-2 body-shape op by handle.
	const titleHandle = "H:pptx/s:257/shape:n:2"
	const bodyHandle = "H:pptx/s:257/shape:n:3"
	var titleOpJSON []byte
	sawBody := false
	for _, op := range ops {
		switch op.Args["for-shape"].String() {
		case titleHandle:
			one, merr := json.Marshal([]apply.Operation{op})
			if merr != nil {
				t.Fatalf("marshal op: %v", merr)
			}
			titleOpJSON = one
		case bodyHandle:
			sawBody = true
		}
	}
	if titleOpJSON == nil || !sawBody {
		t.Fatalf("expected both a title (%s) and body (%s) shape op; got %s", titleHandle, bodyHandle, opsJSON)
	}

	opsFile := filepath.Join(t.TempDir(), "title-op.json")
	if err := os.WriteFile(opsFile, titleOpJSON, 0o644); err != nil {
		t.Fatalf("write op: %v", err)
	}
	applied := filepath.Join(t.TempDir(), "applied.pptx")
	runBin(t, bin, "apply", base, "--ops", opsFile, "--out", applied)

	// The title shape changed: "Content Slide" -> "CZZZ Slide". The SIBLING body
	// "This is the main content area" must be UNTOUCHED (still contains "ontent").
	titleGone := decodeFindResult(t, mustRunFind(t, "--json", "find", "Content Slide", applied))
	if titleGone.TotalHits != 0 {
		t.Fatalf("title shape should have changed; 'Content Slide' still present (%d hits)", titleGone.TotalHits)
	}
	bodyKept := decodeFindResult(t, mustRunFind(t, "--json", "find", "main content area", applied))
	if bodyKept.TotalHits != 1 {
		t.Fatalf("sibling body shape leaked: 'main content area' should survive applying only the title op, got %d hits", bodyKept.TotalHits)
	}

	// --- Finding 2: a recurring substring must NOT abort find --replace --apply.
	// animations-synthetic slide 2 has ONE shape with paragraphs First/Second/Third;
	// "r" recurs there (First, Third) -> two hits in the SAME shape -> identical
	// shape-scoped ops -> dedup to one. Pre-fix this aborted with a zero-match op.
	deck2 := stagePPTXFixture(t, "animations-synthetic")
	out2 := filepath.Join(t.TempDir(), "out2.pptx")
	res2JSON, err := runFind(t, "--json", "find", "r", deck2, "--replace", "Q", "--apply", "--out", out2)
	if err != nil {
		t.Fatalf("recurring-substring --apply aborted (Finding 2 regression): %v\n%s", err, res2JSON)
	}
	var res2 apply.Result
	if err := json.Unmarshal([]byte(res2JSON), &res2); err != nil {
		t.Fatalf("unmarshal apply result: %v (%s)", err, res2JSON)
	}
	if res2.OpsCount == 0 || len(res2.Applied) != res2.OpsCount {
		t.Fatalf("unexpected apply result: opsCount=%d applied=%d", res2.OpsCount, len(res2.Applied))
	}
	// Both "First" and "Third" in the single slide-2 shape must have been rewritten
	// by the ONE deduped op (FiQst, ThiQd), proving collapse loses nothing.
	leftover := decodeFindResult(t, mustRunFind(t, "--json", "find", "First", out2))
	if leftover.TotalHits != 0 {
		t.Fatalf("deduped op should still replace all matches in its shape; 'First' survived (%d)", leftover.TotalHits)
	}

	// --- Table-cell leg: a table-cell hit scopes to its enclosing graphicFrame
	// shape handle, which apply must resolve end-to-end (the task names tables
	// explicitly). table-simple slide 2 has a 3x3 table; replacing one cell value
	// must succeed and land.
	deck3 := stagePPTXFixture(t, "table-simple")
	out3 := filepath.Join(t.TempDir(), "out3.pptx")
	res3JSON, err := runFind(t, "--json", "find", "R0C1", deck3, "--replace", "ZZZ", "--apply", "--out", out3)
	if err != nil {
		t.Fatalf("table-cell shape-scoped --apply failed: %v\n%s", err, res3JSON)
	}
	if got := decodeFindResult(t, mustRunFind(t, "--json", "find", "ZZZ", out3)).TotalHits; got != 1 {
		t.Fatalf("table cell should now read ZZZ, got %d hits", got)
	}
	if got := decodeFindResult(t, mustRunFind(t, "--json", "find", "R0C1", out3)).TotalHits; got != 0 {
		t.Fatalf("original table cell R0C1 should be gone, got %d hits", got)
	}
}

func runBin(t *testing.T, bin string, args ...string) {
	t.Helper()
	cmd := exec.Command(bin, args...)
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		t.Fatalf("ooxml %v: %v", args, err)
	}
}

func mustRunFind(t *testing.T, args ...string) string {
	t.Helper()
	out, err := runFind(t, args...)
	if err != nil {
		t.Fatalf("find %v: %v", args, err)
	}
	return out
}

// TestFindComposeFlagValidation covers the flag-combination guard rules.
func TestFindComposeFlagValidation(t *testing.T) {
	file := stageXLSXFindFixture(t)
	cases := []struct {
		name string
		args []string
	}{
		{"apply without replace", []string{"find", "Revenue", file, "--apply", "--out", "x.xlsx"}},
		{"replace without to-ops or apply", []string{"find", "Revenue", file, "--replace", "x"}},
		{"to-ops and apply together", []string{"find", "Revenue", file, "--to-ops", "--apply", "--replace", "x", "--out", "y.xlsx"}},
		{"apply replace without output target", []string{"find", "Revenue", file, "--replace", "x", "--apply"}},
		{"apply with empty replace", []string{"find", "Revenue", file, "--replace", "", "--apply", "--out", "z.xlsx"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			args := append([]string{"--json"}, tc.args...)
			_, err := runFind(t, args...)
			assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
		})
	}
}

// TestFindToOpsSkipsNotes verifies PPTX speaker-notes hits (no mutation command)
// are skipped from emitted ops while text hits still emit, keeping stdout a pure
// apply-compatible array.
func TestFindToOpsSkipsNotes(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "pptx", "notes-slide", "presentation.pptx")
	if _, err := os.Stat(fixture); err != nil {
		t.Skipf("notes fixture unavailable: %v", err)
	}
	out, err := runFind(t, "--json", "find", "e", fixture, "--ignore-case", "--to-ops")
	if err != nil {
		t.Fatalf("find --to-ops on pptx: %v", err)
	}
	ops, err := apply.ParseOps([]byte(out))
	if err != nil {
		t.Fatalf("ParseOps rejected pptx ops: %v (%s)", err, out)
	}
	// Every emitted op must be a text-occurrences op (notes are skipped).
	for _, op := range ops {
		if op.Command != "pptx replace text-occurrences" {
			t.Errorf("unexpected op command (notes leaked?): %q", op.Command)
		}
	}
}
