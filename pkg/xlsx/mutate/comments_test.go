package mutate

import (
	"errors"
	"fmt"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestAddCommentCreatesPartRelAndAnchors(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	res, err := AddComment(&AddCommentRequest{
		Package: pkg,
		Sheet:   sheet,
		Cell:    "b2",
		Author:  "Ann",
		Text:    "Hello note",
	})
	if err != nil {
		t.Fatalf("AddComment: %v", err)
	}
	if res.CommentID != 0 || res.AnchoredToCell != "B2" || !res.CreatedPart || !res.CreatedRef {
		t.Fatalf("unexpected add result: %+v", res)
	}

	commentsURI, exists := xlsxinspect.FindCommentsPart(pkg, sheet.PartURI)
	if !exists || commentsURI != "/xl/comments1.xml" {
		t.Fatalf("comments part not registered: uri=%q exists=%v", commentsURI, exists)
	}

	listing, err := xlsxinspect.ListComments(pkg, sheet)
	if err != nil {
		t.Fatalf("ListComments: %v", err)
	}
	if len(listing.Comments) != 1 {
		t.Fatalf("expected 1 comment, got %d", len(listing.Comments))
	}
	got := listing.Comments[0]
	if got.Author != "Ann" || got.Text != "Hello note" || got.AnchoredToCell != "B2" {
		t.Fatalf("unexpected listed comment: %+v", got)
	}
	if got.AnchoredToCellColumn != 2 || got.AnchoredToCellRow != 2 {
		t.Fatalf("unexpected anchor coords: %+v", got)
	}
}

// TestAddCommentDoesNotEmitDateOrInitials confirms the non-conformant date and
// initials attributes (the WordprocessingML model) are never written onto the
// legacy SpreadsheetML <comment> element.
func TestAddCommentDoesNotEmitDateOrInitials(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "B2", Author: "Ann", Text: "note"}); err != nil {
		t.Fatalf("AddComment: %v", err)
	}
	doc, err := pkg.ReadXMLPart("/xl/comments1.xml")
	if err != nil {
		t.Fatalf("read comments part: %v", err)
	}
	commentList := namespaces.FindChild(doc.Root(), namespaces.NsSpreadsheetML, "commentList")
	comment := namespaces.FindChild(commentList, namespaces.NsSpreadsheetML, "comment")
	if comment == nil {
		t.Fatal("comment element not found")
	}
	if v := comment.SelectAttrValue("date", ""); v != "" {
		t.Fatalf("comment should not carry a date attribute, got %q", v)
	}
	if v := comment.SelectAttrValue("initials", ""); v != "" {
		t.Fatalf("comment should not carry an initials attribute, got %q", v)
	}
}

// TestAddCommentEmitsVisibleVmlDrawing asserts that adding comments creates the
// paired legacy VML drawing (with one v:shape per comment anchored to the right
// 0-based cell), the worksheet <legacyDrawing r:id> element, and the
// worksheet->vmlDrawing relationship, and that removing the last comment tears
// all of those down.
func TestAddCommentEmitsVisibleVmlDrawing(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	// B2 (row 1, col 1 zero-based) and D5 (row 4, col 3 zero-based).
	for _, cell := range []string{"B2", "D5"} {
		if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: cell, Author: "Ann", Text: cell}); err != nil {
			t.Fatalf("add %s: %v", cell, err)
		}
	}

	vmlURI, exists := findVmlDrawingPart(pkg, sheet.PartURI)
	if !exists {
		t.Fatalf("vml drawing part missing after add; uri=%q", vmlURI)
	}
	if got := pkg.GetContentType(vmlURI); got != namespaces.ContentTypeVml {
		t.Fatalf("vml content type = %q, want %q", got, namespaces.ContentTypeVml)
	}
	raw, err := pkg.ReadRawPart(vmlURI)
	if err != nil {
		t.Fatalf("read vml part: %v", err)
	}
	// The VML is not an +xml part (validators may skip its well-formedness), so
	// assert it parses here.
	if err := etree.NewDocument().ReadFromBytes(raw); err != nil {
		t.Fatalf("vml drawing is not well-formed XML: %v\n%s", err, raw)
	}
	vml := string(raw)
	if n := strings.Count(vml, "<v:shape "); n != 2 {
		t.Fatalf("expected 2 v:shape elements, got %d in:\n%s", n, vml)
	}
	if !strings.Contains(vml, `<x:Row>1</x:Row>`) || !strings.Contains(vml, `<x:Column>1</x:Column>`) {
		t.Fatalf("B2 anchor (row 1, col 1 zero-based) missing in:\n%s", vml)
	}
	if !strings.Contains(vml, `<x:Row>4</x:Row>`) || !strings.Contains(vml, `<x:Column>3</x:Column>`) {
		t.Fatalf("D5 anchor (row 4, col 3 zero-based) missing in:\n%s", vml)
	}
	if !strings.Contains(vml, `ObjectType="Note"`) || !strings.Contains(vml, `id="_x0000_t202"`) {
		t.Fatalf("VML preamble/ClientData missing in:\n%s", vml)
	}

	// worksheet <legacyDrawing r:id> exists and points at the vml relationship.
	wsDoc, err := pkg.ReadXMLPart(sheet.PartURI)
	if err != nil {
		t.Fatalf("read worksheet: %v", err)
	}
	legacy := namespaces.FindChild(wsDoc.Root(), namespaces.NsSpreadsheetML, "legacyDrawing")
	if legacy == nil {
		t.Fatal("worksheet <legacyDrawing> element missing")
	}
	legacyRID, _ := namespaces.Attr(legacy, namespaces.NsR, "id")
	if legacyRID == "" {
		t.Fatal("legacyDrawing has no r:id")
	}
	vmlRelFound := false
	for _, rel := range pkg.ListRelationships(sheet.PartURI) {
		if rel.Type == namespaces.RelVmlDrawing && rel.ID == legacyRID {
			vmlRelFound = true
		}
	}
	if !vmlRelFound {
		t.Fatalf("worksheet vmlDrawing relationship for %s not found", legacyRID)
	}

	// Removing one comment keeps the VML with a single shape.
	if _, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0}); err != nil {
		t.Fatalf("remove first: %v", err)
	}
	raw, err = pkg.ReadRawPart(vmlURI)
	if err != nil {
		t.Fatalf("read vml after one removal: %v", err)
	}
	if n := strings.Count(string(raw), "<v:shape "); n != 1 {
		t.Fatalf("expected 1 v:shape after one removal, got %d", n)
	}

	// Removing the last comment tears the VML part, rel, and element down.
	if _, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0}); err != nil {
		t.Fatalf("remove last: %v", err)
	}
	if _, exists := findVmlDrawingPart(pkg, sheet.PartURI); exists {
		t.Fatal("vml drawing part should be removed after last comment deleted")
	}
	for _, rel := range pkg.ListRelationships(sheet.PartURI) {
		if rel.Type == namespaces.RelVmlDrawing {
			t.Fatal("vmlDrawing relationship should be removed after last comment deleted")
		}
	}
	wsDoc, err = pkg.ReadXMLPart(sheet.PartURI)
	if err != nil {
		t.Fatalf("read worksheet after last removal: %v", err)
	}
	if namespaces.FindChild(wsDoc.Root(), namespaces.NsSpreadsheetML, "legacyDrawing") != nil {
		t.Fatal("worksheet <legacyDrawing> element should be removed after last comment deleted")
	}
}

func TestCommentSidecarsStayConsistentAcrossAddUpdateAndRemove(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	first, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "B2", Author: "Ann", Text: "first"})
	if err != nil {
		t.Fatalf("add first: %v", err)
	}
	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "D5", Author: "Ben", Text: "second"}); err != nil {
		t.Fatalf("add second: %v", err)
	}
	vmlURI := assertCommentSidecarState(t, pkg, sheet, []string{"B2", "D5"})

	if _, err := UpdateComment(&UpdateCommentRequest{
		Package: pkg, Sheet: sheet, CommentID: 1, ExpectedHash: xlsxinspect.CommentContentHash("Ben", "second"),
		Author: "Cara", AuthorSet: true, Text: "second updated", TextSet: true,
	}); err != nil {
		t.Fatalf("update second: %v", err)
	}
	afterUpdateVML := assertCommentSidecarState(t, pkg, sheet, []string{"B2", "D5"})
	if afterUpdateVML != vmlURI {
		t.Fatalf("update should keep the existing VML part: got %q, want %q", afterUpdateVML, vmlURI)
	}
	listing, err := xlsxinspect.ListComments(pkg, sheet)
	if err != nil {
		t.Fatalf("ListComments after update: %v", err)
	}
	if listing.Comments[1].Author != "Cara" || listing.Comments[1].Text != "second updated" {
		t.Fatalf("updated comment not reflected in comments XML: %+v", listing.Comments[1])
	}

	if _, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: first.CommentID, ExpectedHash: first.ContentHash}); err != nil {
		t.Fatalf("remove first: %v", err)
	}
	afterPartialRemoveVML := assertCommentSidecarState(t, pkg, sheet, []string{"D5"})
	if afterPartialRemoveVML != vmlURI {
		t.Fatalf("partial removal should keep the existing VML part: got %q, want %q", afterPartialRemoveVML, vmlURI)
	}

	last, err := xlsxinspect.ListComments(pkg, sheet)
	if err != nil {
		t.Fatalf("ListComments before final removal: %v", err)
	}
	if len(last.Comments) != 1 {
		t.Fatalf("expected one remaining comment, got %+v", last.Comments)
	}
	res, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: last.Comments[0].ID, ExpectedHash: last.Comments[0].ContentHash})
	if err != nil {
		t.Fatalf("remove last: %v", err)
	}
	if !res.RemovedPart {
		t.Fatalf("last comment removal should drop the comments part: %+v", res)
	}
	assertCommentSidecarState(t, pkg, sheet, nil)
}

func TestAddCommentRejectsBadCellAndMissingAuthor(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "not-a-cell", Author: "Ann"}); err == nil {
		t.Fatal("expected error for invalid cell")
	}
	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "A1"}); err == nil {
		t.Fatal("expected error for missing author")
	}
}

func TestAddCommentRejectsDuplicateCell(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "A1", Author: "Ann", Text: "first"}); err != nil {
		t.Fatalf("first add: %v", err)
	}
	_, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "a1", Author: "Bob", Text: "dup"})
	if !errors.Is(err, ErrCommentExists) {
		t.Fatalf("expected ErrCommentExists, got %v", err)
	}
}

func TestAddCommentsOrderedByCellAndStableIDs(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	for _, cell := range []string{"C3", "A1", "B2"} {
		if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: cell, Author: "Ann", Text: cell}); err != nil {
			t.Fatalf("add %s: %v", cell, err)
		}
	}
	listing, err := xlsxinspect.ListComments(pkg, sheet)
	if err != nil {
		t.Fatalf("ListComments: %v", err)
	}
	if len(listing.Comments) != 3 {
		t.Fatalf("expected 3 comments, got %d", len(listing.Comments))
	}
	want := []string{"A1", "B2", "C3"}
	for i, c := range listing.Comments {
		if c.ID != i || c.AnchoredToCell != want[i] {
			t.Fatalf("comment %d = %+v, want id %d cell %s", i, c, i, want[i])
		}
	}
}

func TestUpdateCommentGuardAndFields(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	add, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "B2", Author: "Ann", Text: "before"})
	if err != nil {
		t.Fatalf("add: %v", err)
	}

	// Wrong hash is rejected.
	if _, err := UpdateComment(&UpdateCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0, ExpectedHash: "sha256:deadbeef", Text: "x", TextSet: true}); !errors.Is(err, ErrCommentHashMismatch) {
		t.Fatalf("expected hash mismatch, got %v", err)
	}

	// Correct hash + author change appends a new author and keeps anchor/id.
	res, err := UpdateComment(&UpdateCommentRequest{
		Package: pkg, Sheet: sheet, CommentID: 0, ExpectedHash: add.ContentHash,
		Text: "after", TextSet: true, Author: "Bob", AuthorSet: true,
	})
	if err != nil {
		t.Fatalf("update: %v", err)
	}
	if res.Author != "Bob" || res.Text != "after" || res.AnchoredToCell != "B2" || res.PreviousText != "before" {
		t.Fatalf("unexpected update result: %+v", res)
	}

	listing, _ := xlsxinspect.ListComments(pkg, sheet)
	if len(listing.Comments) != 1 || listing.Comments[0].Author != "Bob" || listing.Comments[0].Text != "after" {
		t.Fatalf("unexpected readback after update: %+v", listing.Comments)
	}
}

func TestUpdateCommentNotFound(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := UpdateComment(&UpdateCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0, Text: "x", TextSet: true}); !errors.Is(err, ErrCommentNotFound) {
		t.Fatalf("expected ErrCommentNotFound on file with no part, got %v", err)
	}
	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "A1", Author: "Ann", Text: "t"}); err != nil {
		t.Fatalf("add: %v", err)
	}
	if _, err := UpdateComment(&UpdateCommentRequest{Package: pkg, Sheet: sheet, CommentID: 5, Text: "x", TextSet: true}); !errors.Is(err, ErrCommentNotFound) {
		t.Fatalf("expected ErrCommentNotFound for out-of-range id, got %v", err)
	}
}

func TestRemoveCommentDropsPartWhenLast(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	add, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "B2", Author: "Ann", Text: "only"})
	if err != nil {
		t.Fatalf("add: %v", err)
	}

	res, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0, ExpectedHash: add.ContentHash})
	if err != nil {
		t.Fatalf("remove: %v", err)
	}
	if !res.RemovedPart || res.PreviousAuthor != "Ann" || res.AnchoredToCell != "B2" {
		t.Fatalf("unexpected remove result: %+v", res)
	}

	if _, exists := xlsxinspect.FindCommentsPart(pkg, sheet.PartURI); exists {
		t.Fatal("comments part should be removed after last comment deleted")
	}
	if commentsRelExists(pkg, sheet.PartURI) {
		t.Fatal("comments relationship should be removed after last comment deleted")
	}
	listing, _ := xlsxinspect.ListComments(pkg, sheet)
	if len(listing.Comments) != 0 {
		t.Fatalf("expected no comments after removal, got %+v", listing.Comments)
	}
}

func TestRemoveCommentKeepsPartWithRemaining(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	for _, cell := range []string{"A1", "B2"} {
		if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: cell, Author: "Ann", Text: cell}); err != nil {
			t.Fatalf("add %s: %v", cell, err)
		}
	}
	res, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0})
	if err != nil {
		t.Fatalf("remove: %v", err)
	}
	if res.RemovedPart {
		t.Fatalf("part should be kept; got %+v", res)
	}
	listing, _ := xlsxinspect.ListComments(pkg, sheet)
	if len(listing.Comments) != 1 || listing.Comments[0].AnchoredToCell != "B2" || listing.Comments[0].ID != 0 {
		t.Fatalf("unexpected remaining comment: %+v", listing.Comments)
	}
}

func TestRemoveCommentHashGuard(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	sheet := workbook.Sheets[0]

	if _, err := AddComment(&AddCommentRequest{Package: pkg, Sheet: sheet, Cell: "A1", Author: "Ann", Text: "t"}); err != nil {
		t.Fatalf("add: %v", err)
	}
	if _, err := RemoveComment(&RemoveCommentRequest{Package: pkg, Sheet: sheet, CommentID: 0, ExpectedHash: "sha256:nope"}); !errors.Is(err, ErrCommentHashMismatch) {
		t.Fatalf("expected hash mismatch, got %v", err)
	}
}

func TestListCommentsNoPart(t *testing.T) {
	pkg, workbook := openTestWorkbook(t, defaultWorksheetXML())
	defer pkg.Close()
	listing, err := xlsxinspect.ListComments(pkg, workbook.Sheets[0])
	if err != nil {
		t.Fatalf("ListComments: %v", err)
	}
	if listing.CommentsPart != "" || len(listing.Comments) != 0 {
		t.Fatalf("expected empty listing, got %+v", listing)
	}
}

func assertCommentSidecarState(t *testing.T, pkg opc.PackageSession, sheet model.SheetRef, wantCells []string) string {
	t.Helper()

	listing, err := xlsxinspect.ListComments(pkg, sheet)
	if err != nil {
		t.Fatalf("ListComments: %v", err)
	}
	if len(listing.Comments) != len(wantCells) {
		t.Fatalf("comments count = %d, want %d: %+v", len(listing.Comments), len(wantCells), listing.Comments)
	}
	for i, want := range wantCells {
		if got := listing.Comments[i].AnchoredToCell; got != want {
			t.Fatalf("comment %d cell = %q, want %q", i, got, want)
		}
	}

	if len(wantCells) == 0 {
		if _, exists := xlsxinspect.FindCommentsPart(pkg, sheet.PartURI); exists {
			t.Fatal("comments part should be absent")
		}
		if commentsRelExists(pkg, sheet.PartURI) {
			t.Fatal("comments relationship should be absent")
		}
		if vmlURI, exists := findVmlDrawingPart(pkg, sheet.PartURI); exists {
			t.Fatalf("vml drawing part should be absent, got %s", vmlURI)
		}
		assertNoVMLRelationship(t, pkg, sheet)
		assertNoLegacyDrawing(t, pkg, sheet)
		return ""
	}

	commentsURI, exists := xlsxinspect.FindCommentsPart(pkg, sheet.PartURI)
	if !exists {
		t.Fatal("comments part should exist")
	}
	if got := pkg.GetContentType(commentsURI); got != namespaces.ContentTypeComments {
		t.Fatalf("comments content type = %q, want %q", got, namespaces.ContentTypeComments)
	}
	assertRelationshipTarget(t, pkg, sheet.PartURI, namespaces.RelComments, commentsURI, "")

	commentsDoc, err := pkg.ReadXMLPart(commentsURI)
	if err != nil {
		t.Fatalf("read comments part: %v", err)
	}
	commentList := namespaces.FindChild(commentsDoc.Root(), namespaces.NsSpreadsheetML, "commentList")
	if got := len(namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment")); got != len(wantCells) {
		t.Fatalf("comments XML count = %d, want %d", got, len(wantCells))
	}

	vmlURI, exists := findVmlDrawingPart(pkg, sheet.PartURI)
	if !exists {
		t.Fatal("vml drawing part should exist")
	}
	if got := pkg.GetContentType(vmlURI); got != namespaces.ContentTypeVml {
		t.Fatalf("vml content type = %q, want %q", got, namespaces.ContentTypeVml)
	}
	vmlRID := assertRelationshipTarget(t, pkg, sheet.PartURI, namespaces.RelVmlDrawing, vmlURI, "")

	wsDoc, err := pkg.ReadXMLPart(sheet.PartURI)
	if err != nil {
		t.Fatalf("read worksheet: %v", err)
	}
	legacy := namespaces.FindChild(wsDoc.Root(), namespaces.NsSpreadsheetML, "legacyDrawing")
	if legacy == nil {
		t.Fatal("worksheet <legacyDrawing> element missing")
	}
	legacyRID, _ := namespaces.Attr(legacy, namespaces.NsR, "id")
	if legacyRID != vmlRID {
		t.Fatalf("legacyDrawing r:id = %q, want vml relationship %q", legacyRID, vmlRID)
	}

	rawVML, err := pkg.ReadRawPart(vmlURI)
	if err != nil {
		t.Fatalf("read vml part: %v", err)
	}
	anchors := vmlAnchorPairs(t, rawVML)
	if len(anchors) != len(wantCells) {
		t.Fatalf("vml anchor count = %d, want %d in:\n%s", len(anchors), len(wantCells), rawVML)
	}
	for i, comment := range listing.Comments {
		wantAnchor := fmt.Sprintf("%d:%d", comment.AnchoredToCellRow-1, comment.AnchoredToCellColumn-1)
		if anchors[i] != wantAnchor {
			t.Fatalf("vml anchor %d = %q, want %q for %s", i, anchors[i], wantAnchor, comment.AnchoredToCell)
		}
	}
	return vmlURI
}

func assertRelationshipTarget(t *testing.T, pkg opc.PackageSession, sourceURI, relType, targetURI, id string) string {
	t.Helper()
	var found []string
	for _, rel := range pkg.ListRelationships(sourceURI) {
		if rel.Type != relType {
			continue
		}
		if id != "" && rel.ID != id {
			continue
		}
		if rel.TargetMode == "External" {
			t.Fatalf("%s relationship %s should be internal", relType, rel.ID)
		}
		resolved := opc.ResolveRelationshipTarget(sourceURI, rel.Target)
		if resolved != targetURI {
			t.Fatalf("%s relationship %s target = %q (%q), want %q", relType, rel.ID, rel.Target, resolved, targetURI)
		}
		found = append(found, rel.ID)
	}
	if len(found) != 1 {
		t.Fatalf("found %d %s relationships targeting %s, want 1", len(found), relType, targetURI)
	}
	return found[0]
}

func assertNoVMLRelationship(t *testing.T, pkg opc.PackageSession, sheet model.SheetRef) {
	t.Helper()
	for _, rel := range pkg.ListRelationships(sheet.PartURI) {
		if rel.Type == namespaces.RelVmlDrawing {
			t.Fatal("vmlDrawing relationship should be absent")
		}
	}
}

func assertNoLegacyDrawing(t *testing.T, pkg opc.PackageSession, sheet model.SheetRef) {
	t.Helper()
	wsDoc, err := pkg.ReadXMLPart(sheet.PartURI)
	if err != nil {
		t.Fatalf("read worksheet: %v", err)
	}
	if namespaces.FindChild(wsDoc.Root(), namespaces.NsSpreadsheetML, "legacyDrawing") != nil {
		t.Fatal("worksheet <legacyDrawing> element should be absent")
	}
}

func vmlAnchorPairs(t *testing.T, raw []byte) []string {
	t.Helper()
	doc := etree.NewDocument()
	if err := doc.ReadFromBytes(raw); err != nil {
		t.Fatalf("vml drawing is not well-formed XML: %v\n%s", err, raw)
	}
	var anchors []string
	for _, clientData := range findVMLDescendants(doc.Root(), "ClientData") {
		row := firstVMLChildText(clientData, "Row")
		col := firstVMLChildText(clientData, "Column")
		if row == "" || col == "" {
			t.Fatalf("VML ClientData missing row/column: %s", clientData.Tag)
		}
		anchors = append(anchors, row+":"+col)
	}
	return anchors
}

func firstVMLChildText(elem *etree.Element, localName string) string {
	for _, child := range elem.ChildElements() {
		if child.Tag == localName {
			return child.Text()
		}
	}
	return ""
}

func findVMLDescendants(elem *etree.Element, localName string) []*etree.Element {
	if elem == nil {
		return nil
	}
	var found []*etree.Element
	for _, child := range elem.ChildElements() {
		if child.Tag == localName {
			found = append(found, child)
		}
		found = append(found, findVMLDescendants(child, localName)...)
	}
	return found
}
