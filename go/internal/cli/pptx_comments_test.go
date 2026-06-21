package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	pptxmutate "github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// writePerAuthorIdxCollisionPPTX writes a deck with two p:cm sharing idx=1 under
// different authorId (Alice=0, Bob=1) to simulate real-Office per-author idx
// allocation, then returns the path.
func writePerAuthorIdxCollisionPPTX(t *testing.T, fixture string) string {
	t.Helper()
	src := pptxShapesFixturePath(t, fixture)
	out := filepath.Join(t.TempDir(), "collision.pptx")

	pkg, err := opc.Open(src)
	if err != nil {
		t.Fatalf("open fixture: %v", err)
	}
	defer pkg.Close()

	if _, err := pptxmutate.AddComment(&pptxmutate.AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Alice", Text: "from alice"}); err != nil {
		t.Fatalf("add alice: %v", err)
	}
	bob, err := pptxmutate.AddComment(&pptxmutate.AddCommentRequest{Package: pkg, SlideNumber: 1, Author: "Bob", Text: "from bob"})
	if err != nil {
		t.Fatalf("add bob: %v", err)
	}

	commentsURI, exists := pptxinspect.FindSlideCommentsPart(pkg, "/ppt/slides/slide1.xml")
	if !exists {
		t.Fatal("expected comments part")
	}
	doc, err := pkg.ReadXMLPart(commentsURI)
	if err != nil {
		t.Fatalf("read comments: %v", err)
	}
	for _, cm := range namespaces.FindChildren(doc.Root(), namespaces.NsP, "cm") {
		if cm.SelectAttrValue("authorId", "") == strconv.Itoa(bob.AuthorID) {
			cm.CreateAttr("idx", "1")
		}
	}
	if err := pkg.ReplaceXMLPart(commentsURI, doc); err != nil {
		t.Fatalf("replace comments: %v", err)
	}
	if err := pkg.SaveAs(out); err != nil {
		t.Fatalf("save collision deck: %v", err)
	}
	return out
}

func TestPPTXCommentsListExposesCollidingAuthorIds(t *testing.T) {
	path := writePerAuthorIdxCollisionPPTX(t, "title-content")

	out := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "list", path, "--slide", "1", "--comment-id", "1",
	)
	var listing PPTXCommentsListResult
	if err := json.Unmarshal([]byte(out), &listing); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, out)
	}
	var rows []pptxinspect.SlideComment
	for _, s := range listing.Slides {
		rows = append(rows, s.Comments...)
	}
	if len(rows) != 2 {
		t.Fatalf("expected 2 colliding rows for comment-id 1, got %d", len(rows))
	}
	if rows[0].ID != 1 || rows[1].ID != 1 {
		t.Fatalf("expected both rows to have id 1: %+v", rows)
	}
	if rows[0].AuthorID == rows[1].AuthorID {
		t.Fatalf("expected distinct authorIds to disambiguate rows: %+v", rows)
	}
}

func TestPPTXCommentsRemoveAmbiguousRequiresAuthorID(t *testing.T) {
	path := writePerAuthorIdxCollisionPPTX(t, "title-content")

	// No --author-id: ambiguous, must be rejected (no silent data loss).
	_, err := executePPTXShapesCommandErr(t,
		"--format", "json",
		"pptx", "comments", "remove", path,
		"--slide", "1", "--comment-id", "1",
		"--out", filepath.Join(t.TempDir(), "fail.pptx"),
	)
	if err == nil {
		t.Fatal("expected ambiguity error without --author-id")
	}
	assertPPTXShapesExitCode(t, err, 2)

	// With --author-id 1: removes Bob's comment, Alice's survives.
	removedPath := filepath.Join(t.TempDir(), "removed.pptx")
	out := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "remove", path,
		"--slide", "1", "--comment-id", "1", "--author-id", "1",
		"--out", removedPath,
	)
	var result PPTXCommentsRemoveResult
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal remove: %v\n%s", err, out)
	}
	if result.PreviousText != "from bob" {
		t.Fatalf("expected to remove Bob's comment, got %q", result.PreviousText)
	}

	listOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "list", removedPath, "--slide", "1",
	)
	var listing PPTXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listing); err != nil {
		t.Fatalf("unmarshal list: %v", err)
	}
	var remaining []pptxinspect.SlideComment
	for _, s := range listing.Slides {
		remaining = append(remaining, s.Comments...)
	}
	if len(remaining) != 1 || remaining[0].Text != "from alice" {
		t.Fatalf("expected only Alice's comment to survive, got %+v", remaining)
	}
}

func TestPPTXCommentsEditAmbiguousRequiresAuthorID(t *testing.T) {
	path := writePerAuthorIdxCollisionPPTX(t, "title-content")

	// No --author-id: ambiguous edit must be rejected.
	_, err := executePPTXShapesCommandErr(t,
		"--format", "json",
		"pptx", "comments", "edit", path,
		"--slide", "1", "--comment-id", "1", "--text", "x",
		"--out", filepath.Join(t.TempDir(), "fail.pptx"),
	)
	if err == nil {
		t.Fatal("expected ambiguity error without --author-id")
	}
	assertPPTXShapesExitCode(t, err, 2)

	// With --author-id 0: edits Alice's comment only.
	editedPath := filepath.Join(t.TempDir(), "edited.pptx")
	out := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "edit", path,
		"--slide", "1", "--comment-id", "1", "--author-id", "0",
		"--text", "alice edited",
		"--out", editedPath,
	)
	var edited PPTXCommentsEditResult
	if err := json.Unmarshal([]byte(out), &edited); err != nil {
		t.Fatalf("unmarshal edit: %v\n%s", err, out)
	}
	if edited.PreviousText != "from alice" || edited.Text != "alice edited" {
		t.Fatalf("unexpected edit result: %+v", edited.EditCommentResult)
	}

	listOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "list", editedPath, "--slide", "1",
	)
	var listing PPTXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listing); err != nil {
		t.Fatalf("unmarshal list: %v", err)
	}
	byAuthor := map[int]string{}
	for _, s := range listing.Slides {
		for _, c := range s.Comments {
			byAuthor[c.AuthorID] = c.Text
		}
	}
	if byAuthor[0] != "alice edited" || byAuthor[1] != "from bob" {
		t.Fatalf("expected only Alice's comment edited: %+v", byAuthor)
	}
}

func TestPPTXCommentsCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	comments := findSubcommand(pptx, "comments")
	if comments == nil {
		t.Fatal("pptx comments command is not registered")
	}
	for _, sub := range []string{"list", "add", "edit", "remove"} {
		if findSubcommand(comments, sub) == nil {
			t.Fatalf("pptx comments %s command is not registered", sub)
		}
	}
}

func TestPPTXCommentsListEmpty(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "list", fixturePath,
	)
	var result PPTXCommentsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath {
		t.Fatalf("unexpected file: %s", result.File)
	}
	for _, s := range result.Slides {
		if len(s.Comments) != 0 {
			t.Fatalf("expected no comments on slide %d, got %d", s.Slide, len(s.Comments))
		}
	}
}

func TestPPTXCommentsAddCreatesPartAndReadback(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "comments-add.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1",
		"--author", "Alice",
		"--initials", "AB",
		"--text", "Fix the title",
		"--date", "2026-06-06T10:30:00Z",
		"--out", outPath,
	)
	var result PPTXCommentsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected metadata: %+v", result)
	}
	if result.CommentID != 1 || result.Author != "Alice" || result.Text != "Fix the title" {
		t.Fatalf("unexpected add result: %+v", result.AddCommentResult)
	}
	if result.Handle == "" || result.PrimarySelector != result.Handle || !containsString(result.Selectors, result.Handle) || !containsString(result.Selectors, "comment:1") {
		t.Fatalf("missing add selectors: %+v", result)
	}
	if !result.CreatedPart || !result.CreatedRelationship || !result.CreatedAuthorsPart || !result.CreatedAuthor {
		t.Fatalf("expected all parts/rels created: %+v", result.AddCommentResult)
	}
	if result.ContentHash == "" || !strings.HasPrefix(result.ContentHash, "sha256:") {
		t.Fatalf("unexpected content hash: %q", result.ContentHash)
	}

	// The generated validate command runs (strict) and the readback returns the comment.
	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx comments list")
	var listing PPTXCommentsListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("failed to unmarshal readback JSON: %v\n%s", err, readback)
	}
	found := false
	for _, s := range listing.Slides {
		for _, c := range s.Comments {
			if c.ID == result.CommentID && c.Author == "Alice" && c.Text == "Fix the title" {
				if c.Handle == "" || c.PrimarySelector != c.Handle || !containsString(c.Selectors, c.Handle) || !containsString(c.Selectors, "comment:1") {
					t.Fatalf("missing listed comment selectors: %+v", c)
				}
				found = true
			}
		}
	}
	if !found {
		t.Fatalf("readback did not contain the added comment: %s", readback)
	}
}

func TestPPTXCommentsEditAndRemoveByHandle(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	dir := t.TempDir()
	addedPath := filepath.Join(dir, "added.pptx")

	addOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1", "--author", "Alice", "--text", "handle me",
		"--out", addedPath,
	)
	var added PPTXCommentsAddResult
	if err := json.Unmarshal([]byte(addOut), &added); err != nil {
		t.Fatalf("unmarshal add: %v\n%s", err, addOut)
	}
	if added.Handle == "" {
		t.Fatalf("expected add handle: %+v", added)
	}

	editedPath := filepath.Join(dir, "edited.pptx")
	editOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "edit", addedPath,
		"--handle", added.Handle,
		"--text", "edited by handle",
		"--out", editedPath,
	)
	var edited PPTXCommentsEditResult
	if err := json.Unmarshal([]byte(editOut), &edited); err != nil {
		t.Fatalf("unmarshal edit: %v\n%s", err, editOut)
	}
	if edited.Handle != added.Handle || edited.Text != "edited by handle" || edited.PreviousText != "handle me" {
		t.Fatalf("unexpected edit-by-handle result: %+v", edited)
	}

	removedPath := filepath.Join(dir, "removed.pptx")
	removeOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "remove", editedPath,
		"--handle", added.Handle,
		"--out", removedPath,
	)
	var removed PPTXCommentsRemoveResult
	if err := json.Unmarshal([]byte(removeOut), &removed); err != nil {
		t.Fatalf("unmarshal remove: %v\n%s", err, removeOut)
	}
	if removed.Handle != added.Handle || removed.PreviousText != "edited by handle" {
		t.Fatalf("unexpected remove-by-handle result: %+v", removed)
	}
}

func TestPPTXCommentsMissingCommentListsCandidates(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	dir := t.TempDir()
	addedPath := filepath.Join(dir, "added.pptx")

	executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1", "--author", "Alice", "--text", "candidate",
		"--out", addedPath,
	)

	_, err := executePPTXShapesCommandErr(t,
		"pptx", "comments", "list", addedPath,
		"--slide", "1", "--comment-id", "999",
	)
	if err == nil {
		t.Fatal("expected missing comment error")
	}
	assertPPTXShapesExitCode(t, err, ExitTargetNotFound)
	msg := err.Error()
	for _, want := range []string{"comment not found: comment:999", "did you mean:", "H:pptx/s:", "/comment:idx:1:authorId:0", "ooxml --json pptx comments list <file> --slide 1"} {
		if !strings.Contains(msg, want) {
			t.Fatalf("missing %q in error: %v", want, err)
		}
	}
}

func TestPPTXCommentsAddWithTextFile(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	dir := t.TempDir()
	textFile := filepath.Join(dir, "note.txt")
	if err := os.WriteFile(textFile, []byte("multi\nline note"), 0o644); err != nil {
		t.Fatal(err)
	}
	outPath := filepath.Join(dir, "out.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1", "--author", "Bob",
		"--text-file", textFile,
		"--out", outPath,
	)
	var result PPTXCommentsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, output)
	}
	if result.Text != "multi\nline note" {
		t.Fatalf("unexpected text: %q", result.Text)
	}
}

func TestPPTXCommentsEditHashGuard(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	dir := t.TempDir()
	addedPath := filepath.Join(dir, "added.pptx")

	addOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1", "--author", "Alice", "--text", "original",
		"--out", addedPath,
	)
	var added PPTXCommentsAddResult
	if err := json.Unmarshal([]byte(addOut), &added); err != nil {
		t.Fatalf("unmarshal add: %v", err)
	}

	// Wrong hash is rejected.
	_, err := executePPTXShapesCommandErr(t,
		"--format", "json",
		"pptx", "comments", "edit", addedPath,
		"--slide", "1", "--comment-id", "1",
		"--text", "changed", "--expect-hash", "sha256:bogus",
		"--out", filepath.Join(dir, "fail.pptx"),
	)
	if err == nil {
		t.Fatal("expected hash mismatch error")
	}
	assertPPTXShapesExitCode(t, err, 2)

	// Correct hash succeeds.
	editPath := filepath.Join(dir, "edited.pptx")
	editOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "edit", addedPath,
		"--slide", "1", "--comment-id", "1",
		"--text", "changed", "--expect-hash", added.ContentHash,
		"--out", editPath,
	)
	var edited PPTXCommentsEditResult
	if err := json.Unmarshal([]byte(editOut), &edited); err != nil {
		t.Fatalf("unmarshal edit: %v\n%s", err, editOut)
	}
	if edited.Text != "changed" || edited.PreviousText != "original" {
		t.Fatalf("unexpected edit result: %+v", edited.EditCommentResult)
	}
}

func TestPPTXCommentsRemove(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	dir := t.TempDir()
	addedPath := filepath.Join(dir, "added.pptx")

	executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1", "--author", "Alice", "--text", "to remove",
		"--out", addedPath,
	)

	removedPath := filepath.Join(dir, "removed.pptx")
	out := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "remove", addedPath,
		"--slide", "1", "--comment-id", "1",
		"--out", removedPath,
	)
	var result PPTXCommentsRemoveResult
	if err := json.Unmarshal([]byte(out), &result); err != nil {
		t.Fatalf("unmarshal remove: %v\n%s", err, out)
	}
	if result.PreviousText != "to remove" || !result.RemovedPart {
		t.Fatalf("unexpected remove result: %+v", result.RemoveCommentResult)
	}

	// Readback shows no comments.
	listOut := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "comments", "list", removedPath, "--slide", "1",
	)
	var listing PPTXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listing); err != nil {
		t.Fatalf("unmarshal list: %v", err)
	}
	for _, s := range listing.Slides {
		if len(s.Comments) != 0 {
			t.Fatalf("expected no comments after remove, got %d", len(s.Comments))
		}
	}
}

func TestPPTXCommentsAddSlideOutOfRange(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "999", "--author", "Alice", "--text", "x",
		"--out", filepath.Join(t.TempDir(), "out.pptx"),
	)
	if err == nil {
		t.Fatal("expected slide-out-of-range error")
	}
	assertPPTXShapesExitCode(t, err, 2)
}

func TestPPTXCommentsAddRequiresAuthor(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	_, err := executePPTXShapesCommandErr(t,
		"--format", "json",
		"pptx", "comments", "add", fixturePath,
		"--slide", "1", "--text", "x",
		"--out", filepath.Join(t.TempDir(), "out.pptx"),
	)
	if err == nil {
		t.Fatal("expected missing-author error")
	}
	assertPPTXShapesExitCode(t, err, 2)
}
