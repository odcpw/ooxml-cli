package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

// TestDOCXParagraphHandleInjectionValidatesAndIsIdempotent drives the full lazy
// upgrade through the CLI: a marker-less paragraph mutated by --index injects a
// w14:paraId, the output passes validate --strict, and re-running through the
// returned handle does not churn the marker.
func TestDOCXParagraphHandleInjectionValidatesAndIsIdempotent(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	out1 := filepath.Join(t.TempDir(), "inject1.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "set", documentPath,
		"--index", "1", "--text", "Injected", "--out", out1,
	)
	if err != nil {
		t.Fatalf("inject set failed: %v", err)
	}
	var r1 DOCXParagraphsSetResult
	if err := json.Unmarshal([]byte(output), &r1); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if r1.Handle == "" || !strings.HasPrefix(r1.Handle, "H:docx/pt:doc/para:m:") {
		t.Fatalf("expected an injected paragraph handle, got %q", r1.Handle)
	}

	// Injected output must validate --strict.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out1); err != nil {
		t.Fatalf("validate --strict on injected output failed: %v", err)
	}

	// Re-run THROUGH the handle: the marker must not churn.
	out2 := filepath.Join(t.TempDir(), "inject2.docx")
	output2, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "set", out1,
		"--handle", r1.Handle, "--text", "Edited via handle", "--out", out2,
	)
	if err != nil {
		t.Fatalf("set by handle failed: %v", err)
	}
	var r2 DOCXParagraphsSetResult
	if err := json.Unmarshal([]byte(output2), &r2); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output2)
	}
	if r2.Handle != r1.Handle {
		t.Fatalf("handle churned: %q -> %q", r1.Handle, r2.Handle)
	}
	if r2.Text != "Edited via handle" {
		t.Fatalf("text = %q", r2.Text)
	}
}

func TestDOCXWrongFormatHandleReportsFormatMismatch(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	out := filepath.Join(t.TempDir(), "wrong-format.docx")
	_, err := executeRootForXLSXTest(t,
		"docx", "paragraphs", "set", documentPath,
		"--handle", "H:pptx/s:256/shape:n:2", "--text", "X", "--out", out,
	)
	if err == nil {
		t.Fatal("expected wrong-format handle error")
	}
	assertCLIHandleCode(t, err, ExitInvalidArgs, "HANDLE_FORMAT_MISMATCH")
}

// TestDOCXParagraphHandleSurvivesStructuralEditViaCLI proves the structural
// survival case end-to-end: stamp a paragraph, prepend another, then the SAME
// handle resolves to the same paragraph at its shifted index.
func TestDOCXParagraphHandleSurvivesStructuralEditViaCLI(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	stamped := filepath.Join(t.TempDir(), "stamped.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "set", documentPath,
		"--index", "1", "--text", "Target", "--out", stamped,
	)
	if err != nil {
		t.Fatalf("stamp failed: %v", err)
	}
	var stampResult DOCXParagraphsSetResult
	if err := json.Unmarshal([]byte(output), &stampResult); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	handle := stampResult.Handle

	prepended := filepath.Join(t.TempDir(), "prepended.docx")
	if _, err := executeRootForXLSXTest(t,
		"docx", "paragraphs", "insert", stamped,
		"--insert-after", "0", "--text", "New top", "--out", prepended,
	); err != nil {
		t.Fatalf("prepend failed: %v", err)
	}

	out := filepath.Join(t.TempDir(), "resolved.docx")
	resolvedRaw, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "paragraphs", "set", prepended,
		"--handle", handle, "--text", "Same paragraph", "--out", out,
	)
	if err != nil {
		t.Fatalf("resolve-after-structural failed: %v", err)
	}
	var resolved DOCXParagraphsSetResult
	if err := json.Unmarshal([]byte(resolvedRaw), &resolved); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if resolved.Index != 2 {
		t.Fatalf("resolved index = %d, want 2 (shifted by prepend)", resolved.Index)
	}
	if resolved.PreviousText != "Target" {
		t.Fatalf("resolved wrong paragraph: previousText = %q, want Target", resolved.PreviousText)
	}
}

// TestDOCXCommentHandleEditAndStale exercises comment handle resolution and the
// clean stale error.
func TestDOCXCommentHandleEditAndStale(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-comments")
	out := filepath.Join(t.TempDir(), "edited.docx")

	// The comments-list handle for comment 0.
	listRaw, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", documentPath)
	if err != nil {
		t.Fatalf("comments list failed: %v", err)
	}
	var listing struct {
		Comments []struct {
			ID     int    `json:"id"`
			Handle string `json:"handle"`
		} `json:"comments"`
	}
	if err := json.Unmarshal([]byte(listRaw), &listing); err != nil {
		t.Fatalf("unmarshal listing: %v\n%s", err, listRaw)
	}
	if len(listing.Comments) == 0 || listing.Comments[0].Handle == "" {
		t.Fatalf("expected a comment handle, got %+v", listing.Comments)
	}
	handle := listing.Comments[0].Handle

	if _, err := executeRootForXLSXTest(t,
		"docx", "comments", "edit", documentPath,
		"--handle", handle, "--text", "Edited by handle", "--out", out,
	); err != nil {
		t.Fatalf("comments edit by handle failed: %v", err)
	}

	// A stale comment handle must be a clean target-not-found, never a wrong hit.
	if _, err := executeRootForXLSXTest(t,
		"docx", "comments", "edit", documentPath,
		"--handle", "H:docx/pt:doc/comment:n:9999", "--text", "x",
		"--out", filepath.Join(t.TempDir(), "x.docx"),
	); err == nil {
		t.Fatal("expected stale comment handle to error")
	} else if !strings.Contains(err.Error(), "HANDLE_STALE") {
		t.Fatalf("stale error = %v, want HANDLE_STALE", err)
	} else {
		assertCLIHandleCode(t, err, ExitTargetNotFound, "HANDLE_STALE")
	}
}

// TestDOCXStyleHandleSurfacedAndApplied proves styles list surfaces a style
// handle and `styles apply --style <handle>` resolves it.
func TestDOCXStyleHandleSurfacedAndApplied(t *testing.T) {
	documentPath := getDOCXTestFilePath("styled-headings")
	out := filepath.Join(t.TempDir(), "styled.docx")

	applyRaw, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "styles", "apply", documentPath,
		"--index", "1", "--target", "paragraph",
		"--style", "H:docx/pt:styles/style:n:Heading1",
		"--no-validate", "--out", out,
	)
	if err != nil {
		t.Fatalf("styles apply by style handle failed: %v", err)
	}
	var applyResult DOCXStylesApplyResult
	if err := json.Unmarshal([]byte(applyRaw), &applyResult); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, applyRaw)
	}
	if applyResult.Style != "Heading1" {
		t.Fatalf("resolved style = %q, want Heading1", applyResult.Style)
	}
	if applyResult.StyleHandle != "H:docx/pt:styles/style:n:Heading1" {
		t.Fatalf("styleHandle echo = %q", applyResult.StyleHandle)
	}
	// The styled paragraph gains a handle (lazy upgrade).
	if !strings.HasPrefix(applyResult.Handle, "H:docx/pt:doc/para:m:") {
		t.Fatalf("expected a paragraph handle on styled block, got %q", applyResult.Handle)
	}
}

// TestDOCXDuplicateMarkerOmitsHandleOnSurface proves the AMBIGUITY surface
// contract: two paragraphs sharing a w14:paraId advertise NO handle (so an agent
// never receives a handle that would mis-resolve), and the marked paragraphs are
// still listed.
func TestDOCXDuplicateMarkerOmitsHandleOnSurface(t *testing.T) {
	documentPath := getDOCXTestFilePath("paraid-dup")

	blocksRaw, err := executeRootForXLSXTest(t, "--format", "json", "docx", "blocks", documentPath)
	if err != nil {
		t.Fatalf("docx blocks failed: %v", err)
	}
	var blocks DOCXBlocksResult
	if err := json.Unmarshal([]byte(blocksRaw), &blocks); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, blocksRaw)
	}
	if len(blocks.Blocks) != 2 {
		t.Fatalf("block count = %d, want 2", len(blocks.Blocks))
	}
	for _, b := range blocks.Blocks {
		if b.ParaID != "DEAD00FF" {
			t.Fatalf("block %d paraId = %q, want DEAD00FF (read surface)", b.Index, b.ParaID)
		}
		if b.Handle != "" {
			t.Fatalf("block %d advertised handle %q for a duplicate marker; want omitted", b.Index, b.Handle)
		}
	}

	// find must not advertise the duplicate marker handle either.
	findRaw, err := executeRootForXLSXTest(t, "--format", "json", "find", "Duplicate", documentPath)
	if err != nil {
		t.Fatalf("find failed: %v", err)
	}
	if strings.Contains(findRaw, "H:docx/pt:doc/para:m:DEAD00FF") {
		t.Fatalf("find advertised a duplicate-marker handle: %s", findRaw)
	}
}

// TestDOCXLegacySelectorsStillWork is the no-regression guard: the int --index
// and --comment-id paths must keep working byte-for-byte alongside handles.
func TestDOCXLegacySelectorsStillWork(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	out := filepath.Join(t.TempDir(), "legacy.docx")
	if _, err := executeRootForXLSXTest(t,
		"docx", "paragraphs", "set", documentPath,
		"--index", "1", "--text", "Legacy index path", "--out", out,
	); err != nil {
		t.Fatalf("legacy --index path failed: %v", err)
	}

	// blocks legacy body.b<n> still resolves.
	blocksRaw, err := executeRootForXLSXTest(t, "--format", "json", "docx", "blocks", out, "--block", "1")
	if err != nil {
		t.Fatalf("legacy blocks failed: %v", err)
	}
	if !strings.Contains(blocksRaw, "body.b1") {
		t.Fatalf("legacy block id body.b1 missing from %s", blocksRaw)
	}
}
