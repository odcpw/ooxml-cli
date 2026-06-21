package mutate

import (
	"errors"
	"testing"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func listCommentsForTest(t *testing.T, pkg opc.PackageSession, documentURI string) *docxinspect.DocumentComments {
	t.Helper()
	listing, err := docxinspect.ListComments(pkg, documentURI)
	if err != nil {
		t.Fatalf("ListComments returned error: %v", err)
	}
	return listing
}

func TestAddCommentCreatesCommentsPartIfMissing(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	if _, exists := docxinspect.FindCommentsPart(pkg, documentURI); exists {
		t.Fatalf("minimal fixture unexpectedly already has a comments part")
	}

	result, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AnchorBlock: 1,
		Author:      "Bob",
		Date:        "2025-06-06T10:30:00Z",
		Text:        "Brand new",
	})
	if err != nil {
		t.Fatalf("AddComment returned error: %v", err)
	}
	if !result.CreatedPart || !result.CreatedRef {
		t.Fatalf("expected created part and ref: %+v", result)
	}
	if result.CommentID != 0 {
		t.Fatalf("first comment id = %d, want 0", result.CommentID)
	}
	uri, exists := docxinspect.FindCommentsPart(pkg, documentURI)
	if !exists {
		t.Fatalf("comments part not created")
	}
	if pkg.GetContentType(uri) == "" {
		t.Fatalf("comments content type override not registered")
	}

	listing := listCommentsForTest(t, pkg, documentURI)
	if len(listing.Comments) != 1 || listing.Comments[0].Text != "Brand new" || listing.Comments[0].AnchoredToBlock != 1 {
		t.Fatalf("unexpected listing: %+v", listing.Comments)
	}
}

func TestAddCommentAssignsNextID(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-comments")
	defer pkg.Close()

	result, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AnchorBlock: 2,
		Author:      "Carol",
		Text:        "Second comment",
	})
	if err != nil {
		t.Fatalf("AddComment returned error: %v", err)
	}
	if result.CommentID != 1 {
		t.Fatalf("next comment id = %d, want 1 (existing id 0)", result.CommentID)
	}
	if result.CreatedPart {
		t.Fatalf("should reuse existing comments part: %+v", result)
	}
	if result.AnchoredToBlock != 2 {
		t.Fatalf("anchored to block = %d, want 2", result.AnchoredToBlock)
	}
}

func TestAddCommentWithoutAnchorUsesFirstBlock(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	result, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Author:      "Bob",
		Text:        "No anchor",
	})
	if err != nil {
		t.Fatalf("AddComment returned error: %v", err)
	}
	if result.AnchoredToBlock != 1 {
		t.Fatalf("default anchor block = %d, want 1", result.AnchoredToBlock)
	}
}

func TestAddCommentRejectsOutOfRangeAnchor(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	_, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AnchorBlock: 99,
		Author:      "Bob",
		Text:        "oops",
	})
	if !errors.Is(err, ErrCommentAnchorOutOfRange) {
		t.Fatalf("out-of-range anchor error = %v, want ErrCommentAnchorOutOfRange", err)
	}
}

func TestAddCommentRejectsNonParagraphAnchor(t *testing.T) {
	// The "table" fixture has a table as its first body block.
	pkg, documentURI := openFixture(t, "table")
	defer pkg.Close()

	_, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AnchorBlock: 1,
		Author:      "Bob",
		Text:        "on a table",
	})
	if !errors.Is(err, ErrCommentAnchorNotParagraph) {
		t.Fatalf("table anchor error = %v, want ErrCommentAnchorNotParagraph", err)
	}
}

func TestEditCommentUpdatesDate(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-comments")
	defer pkg.Close()

	before := listCommentsForTest(t, pkg, documentURI).Comments[0]
	result, err := EditComment(&EditCommentRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		CommentID:    0,
		ExpectedHash: before.ContentHash,
		Date:         "2030-01-02T03:04:05Z",
		DateSet:      true,
	})
	if err != nil {
		t.Fatalf("EditComment (date) returned error: %v", err)
	}
	if result.Date != "2030-01-02T03:04:05Z" {
		t.Fatalf("date not updated: %+v", result)
	}
	if result.Text != before.Text {
		t.Fatalf("text should be unchanged: %+v", result)
	}
	if result.ContentHash == before.ContentHash {
		t.Fatalf("hash should change when date changes: %+v", result)
	}
	after := listCommentsForTest(t, pkg, documentURI).Comments[0]
	if after.Date != "2030-01-02T03:04:05Z" {
		t.Fatalf("readback date = %q", after.Date)
	}
}

func TestEditCommentUpdatesTextAndHash(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-comments")
	defer pkg.Close()

	before := listCommentsForTest(t, pkg, documentURI).Comments[0]

	result, err := EditComment(&EditCommentRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		CommentID:    0,
		ExpectedHash: before.ContentHash,
		Text:         "Updated",
		TextSet:      true,
	})
	if err != nil {
		t.Fatalf("EditComment returned error: %v", err)
	}
	if result.PreviousText != before.Text || result.PreviousHash != before.ContentHash {
		t.Fatalf("unexpected previous fields: %+v", result)
	}
	if result.Text != "Updated" || result.ContentHash == before.ContentHash {
		t.Fatalf("hash should change on edit: %+v", result)
	}

	after := listCommentsForTest(t, pkg, documentURI).Comments[0]
	if after.Text != "Updated" {
		t.Fatalf("readback text = %q", after.Text)
	}
}

func TestEditCommentHashMismatch(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-comments")
	defer pkg.Close()

	_, err := EditComment(&EditCommentRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		CommentID:    0,
		ExpectedHash: "sha256:wrong",
		Text:         "Updated",
		TextSet:      true,
	})
	if !errors.Is(err, ErrCommentHashMismatch) {
		t.Fatalf("mismatch error = %v, want ErrCommentHashMismatch", err)
	}
}

func TestEditCommentNotFound(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-comments")
	defer pkg.Close()

	_, err := EditComment(&EditCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		CommentID:   99,
		Text:        "x",
		TextSet:     true,
	})
	if !errors.Is(err, ErrCommentNotFound) {
		t.Fatalf("missing comment error = %v, want ErrCommentNotFound", err)
	}
}

func TestRemoveCommentDeletesEntryAndMarkers(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-comments")
	defer pkg.Close()

	before := listCommentsForTest(t, pkg, documentURI).Comments[0]

	result, err := RemoveComment(&RemoveCommentRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		CommentID:    0,
		ExpectedHash: before.ContentHash,
	})
	if err != nil {
		t.Fatalf("RemoveComment returned error: %v", err)
	}
	if !result.RangeMarkersRemoved {
		t.Fatalf("expected range markers removed: %+v", result)
	}
	if result.PreviousAuthor != "Alice" || result.PreviousText != before.Text {
		t.Fatalf("unexpected previous fields: %+v", result)
	}

	listing := listCommentsForTest(t, pkg, documentURI)
	if len(listing.Comments) != 0 {
		t.Fatalf("comment not removed: %+v", listing.Comments)
	}

	// Document body should no longer contain markers for id 0.
	removed, err := removeCommentMarkers(pkg, documentURI, 0)
	if err != nil {
		t.Fatalf("removeCommentMarkers second pass error: %v", err)
	}
	if removed {
		t.Fatalf("markers should already be gone")
	}
}

func TestRemoveOrphanedComment(t *testing.T) {
	// Add a comment, then strip its body markers manually, then remove it.
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	if _, err := AddComment(&AddCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AnchorBlock: 1,
		Author:      "Bob",
		Text:        "to orphan",
	}); err != nil {
		t.Fatalf("AddComment returned error: %v", err)
	}
	// Strip markers, leaving the comment entry orphaned.
	if _, err := removeCommentMarkers(pkg, documentURI, 0); err != nil {
		t.Fatalf("removeCommentMarkers error: %v", err)
	}

	result, err := RemoveComment(&RemoveCommentRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		CommentID:   0,
	})
	if err != nil {
		t.Fatalf("RemoveComment on orphan returned error: %v", err)
	}
	if result.RangeMarkersRemoved {
		t.Fatalf("orphan should report no markers removed: %+v", result)
	}
	if len(listCommentsForTest(t, pkg, documentURI).Comments) != 0 {
		t.Fatalf("orphan comment not removed")
	}
}
