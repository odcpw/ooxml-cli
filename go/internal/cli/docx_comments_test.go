package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestDOCXCommentsListJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-comments")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", documentPath)
	if err != nil {
		t.Fatalf("docx comments list failed: %v", err)
	}
	var result DOCXCommentsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list JSON: %v\n%s", err, output)
	}
	if result.CommentsPart != "/word/comments.xml" {
		t.Fatalf("comments part = %q", result.CommentsPart)
	}
	if len(result.Comments) != 1 {
		t.Fatalf("comment count = %d, want 1", len(result.Comments))
	}
	c := result.Comments[0]
	if c.ID != 0 || c.Author != "Alice" || c.Text != "This is a comment" {
		t.Fatalf("unexpected comment: %+v", c)
	}
	if c.AnchoredToBlock != 1 || c.AnchoredToBlockKind != "paragraph" {
		t.Fatalf("unexpected anchor: %+v", c)
	}
	if !strings.HasPrefix(c.ContentHash, "sha256:") {
		t.Fatalf("missing content hash: %+v", c)
	}
	if c.PrimarySelector != "0" || !containsString(c.Selectors, "0") {
		t.Fatalf("unexpected comment selectors: primary=%q selectors=%+v", c.PrimarySelector, c.Selectors)
	}
}

func TestDOCXCommentsListTextAndFilter(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-comments")
	output, err := executeRootForXLSXTest(t, "docx", "comments", "list", documentPath)
	if err != nil {
		t.Fatalf("docx comments list (text) failed: %v", err)
	}
	if !strings.Contains(output, "comment 0 by Alice") {
		t.Fatalf("unexpected text output: %q", output)
	}

	if _, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", documentPath, "--comment-id", "0"); err != nil {
		t.Fatalf("filter by id failed: %v", err)
	}
	if _, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", documentPath, "--comment-id", "99"); err == nil {
		t.Fatalf("expected error filtering missing comment id")
	}
}

func TestDOCXCommentsListEmptyNoPart(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", documentPath)
	if err != nil {
		t.Fatalf("docx comments list on minimal failed: %v", err)
	}
	var result DOCXCommentsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list JSON: %v\n%s", err, output)
	}
	if len(result.Comments) != 0 {
		t.Fatalf("expected no comments, got %d", len(result.Comments))
	}
}

func TestDOCXCommentsAddCreatesPartAndValidates(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	outPath := filepath.Join(t.TempDir(), "added.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "comments", "add", documentPath,
		"--anchor-block", "1",
		"--author", "Bob",
		"--text", "Brand new",
		"--date", "2025-06-06T10:30:00Z",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx comments add failed: %v", err)
	}
	var result DOCXCommentsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal add JSON: %v\n%s", err, output)
	}
	if result.CommentID != 0 || !result.CreatedPart || !result.CreatedRef {
		t.Fatalf("unexpected add result: %+v", result)
	}
	if result.AnchoredToBlock != 1 || result.Operation != "added" {
		t.Fatalf("unexpected add result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("added DOCX did not validate: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXCommentsListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, readback)
	}
	if len(listing.Comments) != 1 || listing.Comments[0].Text != "Brand new" || listing.Comments[0].Author != "Bob" {
		t.Fatalf("readback mismatch: %+v", listing.Comments)
	}
	if listing.Comments[0].ContentHash != result.ContentHash {
		t.Fatalf("readback hash %q != add hash %q", listing.Comments[0].ContentHash, result.ContentHash)
	}
}

func TestDOCXCommentsAddDryRun(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "comments", "add", documentPath,
		"--author", "Bob",
		"--text", "Dry run",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx comments add dry-run failed: %v", err)
	}
	var result DOCXCommentsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if result.Text != "Dry run" || result.AnchoredToBlock != 1 {
		t.Fatalf("unexpected dry-run result: %+v", result)
	}
}

func TestDOCXCommentsAddRequiresAuthor(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	if _, err := executeRootForXLSXTest(t,
		"docx", "comments", "add", documentPath,
		"--text", "no author",
		"--dry-run",
	); err == nil {
		t.Fatalf("expected error when --author omitted")
	}
}

func TestDOCXCommentsEditReadbackAndGuard(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-comments")
	outPath := filepath.Join(t.TempDir(), "edited.docx")

	hash := docxCommentHashForTest(t, documentPath, 0)
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "comments", "edit", documentPath,
		"--comment-id", "0",
		"--text", "Updated comment",
		"--date", "2030-01-02T03:04:05Z",
		"--expect-hash", hash,
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx comments edit failed: %v", err)
	}
	var result DOCXCommentsEditResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal edit JSON: %v\n%s", err, output)
	}
	if result.PreviousText != "This is a comment" || result.Text != "Updated comment" {
		t.Fatalf("unexpected edit result: %+v", result)
	}
	if result.Date != "2030-01-02T03:04:05Z" {
		t.Fatalf("date not updated: %+v", result)
	}
	if result.ContentHash == result.PreviousHash {
		t.Fatalf("content hash should change on edit: %+v", result)
	}

	// Readback must reflect the updated date.
	rb, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", outPath, "--comment-id", "0")
	if err != nil {
		t.Fatalf("readback failed: %v", err)
	}
	var listing DOCXCommentsListResult
	if err := json.Unmarshal([]byte(rb), &listing); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, rb)
	}
	if len(listing.Comments) != 1 || listing.Comments[0].Date != "2030-01-02T03:04:05Z" {
		t.Fatalf("readback date mismatch: %+v", listing.Comments)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("edited DOCX did not validate: %v", err)
	}

	// Wrong hash must fail.
	if _, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "comments", "edit", documentPath,
		"--comment-id", "0",
		"--text", "x",
		"--expect-hash", "sha256:bogus",
		"--out", filepath.Join(t.TempDir(), "x.docx"),
	); err == nil {
		t.Fatalf("expected hash mismatch error")
	}
}

func TestDOCXCommentsRemoveCleansMarkers(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-comments")
	outPath := filepath.Join(t.TempDir(), "removed.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "comments", "remove", documentPath,
		"--comment-id", "0",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx comments remove failed: %v", err)
	}
	var result DOCXCommentsRemoveResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal remove JSON: %v\n%s", err, output)
	}
	if result.PreviousAuthor != "Alice" || !result.RangeMarkersRemoved || result.Operation != "removed" {
		t.Fatalf("unexpected remove result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("removed DOCX did not validate: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXCommentsListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, readback)
	}
	if len(listing.Comments) != 0 {
		t.Fatalf("comment not removed: %+v", listing.Comments)
	}
}

func TestDOCXCommentsRemoveRequiresID(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-comments")
	if _, err := executeRootForXLSXTest(t,
		"docx", "comments", "remove", documentPath,
		"--out", filepath.Join(t.TempDir(), "x.docx"),
	); err == nil {
		t.Fatalf("expected error when --comment-id omitted")
	}
}

func docxCommentHashForTest(t *testing.T, documentPath string, id int) string {
	t.Helper()
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "comments", "list", documentPath, "--comment-id", "0")
	if err != nil {
		t.Fatalf("docx comments list (hash) failed: %v", err)
	}
	var result DOCXCommentsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list JSON: %v\n%s", err, output)
	}
	for _, c := range result.Comments {
		if c.ID == id {
			return c.ContentHash
		}
	}
	t.Fatalf("comment %d not found for hash", id)
	return ""
}
