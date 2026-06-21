package mutate

import (
	"errors"
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

var (
	// ErrCommentHashMismatch is returned when an --expect-hash guard does not match.
	ErrCommentHashMismatch = errors.New("comment hash mismatch")
	// ErrCommentNotFound is returned when a comment id is absent from the part.
	ErrCommentNotFound = errors.New("comment not found")
	// ErrCommentExists is returned when a cell already carries a comment.
	ErrCommentExists = errors.New("cell already has a comment")
)

// AddCommentRequest anchors a new comment to a worksheet cell.
type AddCommentRequest struct {
	Package opc.PackageSession
	Sheet   model.SheetRef
	Cell    string
	Author  string
	Text    string
}

// AddCommentResult reports the created comment.
type AddCommentResult struct {
	CommentID      int    `json:"commentId"`
	Author         string `json:"author"`
	Text           string `json:"text"`
	ContentHash    string `json:"contentHash"`
	AnchoredToCell string `json:"anchoredToCell"`
	CreatedPart    bool   `json:"createdPart"`
	CreatedRef     bool   `json:"createdRef"`
}

// UpdateCommentRequest updates an existing comment's text/author by id.
type UpdateCommentRequest struct {
	Package      opc.PackageSession
	Sheet        model.SheetRef
	CommentID    int
	ExpectedHash string
	Text         string
	TextSet      bool
	Author       string
	AuthorSet    bool
}

// UpdateCommentResult reports the updated comment.
type UpdateCommentResult struct {
	CommentID      int    `json:"commentId"`
	Author         string `json:"author"`
	Text           string `json:"text"`
	ContentHash    string `json:"contentHash"`
	AnchoredToCell string `json:"anchoredToCell"`
	PreviousText   string `json:"previousText"`
	PreviousHash   string `json:"previousHash"`
}

// RemoveCommentRequest deletes a comment entry by id.
type RemoveCommentRequest struct {
	Package      opc.PackageSession
	Sheet        model.SheetRef
	CommentID    int
	ExpectedHash string
}

// RemoveCommentResult reports the removed comment.
type RemoveCommentResult struct {
	CommentID      int    `json:"commentId"`
	PreviousAuthor string `json:"previousAuthor"`
	PreviousText   string `json:"previousText"`
	PreviousHash   string `json:"previousHash"`
	AnchoredToCell string `json:"anchoredToCell"`
	RemovedPart    bool   `json:"removedPart"`
}

// AddComment inserts a legacy cell comment, creating the comments part, its
// content-type override, and the worksheet relationship if they do not exist.
// The comment anchors to the cell via the <comment ref="A1"> attribute only;
// the worksheet XML body is never modified.
func AddComment(req *AddCommentRequest) (*AddCommentResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add comment request is nil")
	}
	if req.Author == "" {
		return nil, fmt.Errorf("author is required")
	}
	normCell, err := address.NormalizeCell(req.Cell)
	if err != nil {
		return nil, err
	}

	commentsURI, exists := xlsxinspect.FindCommentsPart(req.Package, req.Sheet.PartURI)
	createdPart := false
	createdRef := false

	var commentsDoc *etree.Document
	var root *etree.Element
	if !exists {
		commentsDoc = newCommentsDocument()
		root = commentsDoc.Root()
		if err := req.Package.AddPart(commentsURI, []byte(""), namespaces.ContentTypeComments, nil); err != nil {
			return nil, fmt.Errorf("failed to add comments part %s: %w", commentsURI, err)
		}
		createdPart = true
	} else {
		commentsDoc, err = req.Package.ReadXMLPart(commentsURI)
		if err != nil {
			return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
		}
		root = commentsDoc.Root()
		if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "comments") {
			return nil, fmt.Errorf("comments part %s has no comments root", commentsURI)
		}
	}

	authorsElem := ensureCommentsChild(root, "authors")
	commentList := ensureCommentsChild(root, "commentList")

	// Reject a duplicate comment on the same cell.
	for _, c := range namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment") {
		if existing, e := address.NormalizeCell(c.SelectAttrValue("ref", "")); e == nil && existing == normCell {
			return nil, fmt.Errorf("%w: %s", ErrCommentExists, normCell)
		}
	}

	authorID := appendAuthor(authorsElem, req.Author)

	// Legacy SpreadsheetML notes (CT_Comment) carry only ref/authorId; date and
	// initials are the WordprocessingML model and Excel drops them silently, so
	// we never emit them.
	comment := newElement("", "comment")
	comment.CreateAttr("ref", normCell)
	comment.CreateAttr("authorId", fmt.Sprintf("%d", authorID))
	comment.AddChild(newCommentText(req.Text))
	insertCommentInOrder(commentList, comment, normCell)

	newID := indexOfComment(commentList, comment)

	if err := req.Package.ReplaceXMLPart(commentsURI, commentsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	if !commentsRelExists(req.Package, req.Sheet.PartURI) {
		rels := req.Package.ListRelationships(req.Sheet.PartURI)
		relID := opc.AllocateRelationshipID(rels)
		rels = append(rels, opc.RelationshipInfo{
			SourceURI: req.Sheet.PartURI,
			ID:        relID,
			Type:      namespaces.RelComments,
			Target:    opc.RelationshipTarget(req.Sheet.PartURI, commentsURI),
		})
		if err := opc.WriteRelationships(req.Package, req.Sheet.PartURI, rels); err != nil {
			return nil, fmt.Errorf("failed to write comments relationship: %w", err)
		}
		createdRef = true
	}

	// Emit/refresh the paired legacy VML drawing so the note is VISIBLE as a box
	// in desktop Excel; without it the comment data exists but never renders.
	if _, err := syncCommentsVml(req.Package, req.Sheet, commentList); err != nil {
		return nil, err
	}

	return &AddCommentResult{
		CommentID:      newID,
		Author:         req.Author,
		Text:           req.Text,
		ContentHash:    xlsxinspect.CommentContentHash(req.Author, req.Text),
		AnchoredToCell: normCell,
		CreatedPart:    createdPart,
		CreatedRef:     createdRef,
	}, nil
}

// UpdateComment updates a comment's text and/or author by id.
func UpdateComment(req *UpdateCommentRequest) (*UpdateCommentResult, error) {
	if req == nil {
		return nil, fmt.Errorf("update comment request is nil")
	}
	commentsURI, exists := xlsxinspect.FindCommentsPart(req.Package, req.Sheet.PartURI)
	if !exists {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}
	commentsDoc, err := req.Package.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := commentsDoc.Root()
	commentList := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "commentList")
	comment := commentByID(commentList, req.CommentID)
	if comment == nil {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}

	authorsElem := ensureCommentsChild(root, "authors")
	authors := xlsxinspect.ParseAuthors(root)
	before := xlsxinspect.ReportCommentElement(comment, authors)
	if req.ExpectedHash != "" && req.ExpectedHash != before.ContentHash {
		return nil, fmt.Errorf("%w: comment %d expected %s but found %s", ErrCommentHashMismatch, req.CommentID, req.ExpectedHash, before.ContentHash)
	}

	author := before.Author
	if req.AuthorSet {
		author = req.Author
		comment.CreateAttr("authorId", fmt.Sprintf("%d", appendAuthor(authorsElem, author)))
	}
	text := before.Text
	if req.TextSet {
		text = req.Text
		setCommentText(comment, text)
	}

	if err := req.Package.ReplaceXMLPart(commentsURI, commentsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	return &UpdateCommentResult{
		CommentID:      req.CommentID,
		Author:         author,
		Text:           text,
		ContentHash:    xlsxinspect.CommentContentHash(author, text),
		AnchoredToCell: before.AnchoredToCell,
		PreviousText:   before.Text,
		PreviousHash:   before.ContentHash,
	}, nil
}

// RemoveComment deletes a comment entry by id. When the last comment is removed
// the comments part and its worksheet relationship are removed as well.
func RemoveComment(req *RemoveCommentRequest) (*RemoveCommentResult, error) {
	if req == nil {
		return nil, fmt.Errorf("remove comment request is nil")
	}
	commentsURI, exists := xlsxinspect.FindCommentsPart(req.Package, req.Sheet.PartURI)
	if !exists {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}
	commentsDoc, err := req.Package.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := commentsDoc.Root()
	commentList := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "commentList")
	comment := commentByID(commentList, req.CommentID)
	if comment == nil {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}

	authors := xlsxinspect.ParseAuthors(root)
	before := xlsxinspect.ReportCommentElement(comment, authors)
	if req.ExpectedHash != "" && req.ExpectedHash != before.ContentHash {
		return nil, fmt.Errorf("%w: comment %d expected %s but found %s", ErrCommentHashMismatch, req.CommentID, req.ExpectedHash, before.ContentHash)
	}

	commentList.RemoveChild(comment)
	removedPart := false
	if len(namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment")) == 0 {
		// The comments part requires at least one comment; remove the now-empty
		// part and its worksheet relationship rather than leaving an invalid file.
		if err := req.Package.RemovePart(commentsURI); err != nil {
			return nil, fmt.Errorf("failed to remove comments part %s: %w", commentsURI, err)
		}
		if err := removeCommentsRel(req.Package, req.Sheet.PartURI); err != nil {
			return nil, err
		}
		removedPart = true
	} else if err := req.Package.ReplaceXMLPart(commentsURI, commentsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	// Refresh (or, on last-removal, tear down) the paired legacy VML drawing so
	// the visible note boxes stay in sync with the remaining comments.
	if _, err := syncCommentsVml(req.Package, req.Sheet, commentList); err != nil {
		return nil, err
	}

	return &RemoveCommentResult{
		CommentID:      req.CommentID,
		PreviousAuthor: before.Author,
		PreviousText:   before.Text,
		PreviousHash:   before.ContentHash,
		AnchoredToCell: before.AnchoredToCell,
		RemovedPart:    removedPart,
	}, nil
}

// ---- helpers ----

func newCommentsDocument() *etree.Document {
	doc := etree.NewDocument()
	doc.CreateProcInst("xml", `version="1.0" encoding="UTF-8" standalone="yes"`)
	root := doc.CreateElement("comments")
	root.CreateAttr("xmlns", namespaces.NsSpreadsheetML)
	root.CreateElement("authors")
	root.CreateElement("commentList")
	return doc
}

// ensureCommentsChild returns the named direct child of <comments>, creating it
// in schema order (authors before commentList) when missing.
func ensureCommentsChild(root *etree.Element, localName string) *etree.Element {
	if child := namespaces.FindChild(root, namespaces.NsSpreadsheetML, localName); child != nil {
		return child
	}
	child := newElement(root.Space, localName)
	if localName == "authors" {
		if commentList := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "commentList"); commentList != nil {
			root.InsertChildAt(commentList.Index(), child)
			return child
		}
	}
	root.AddChild(child)
	return child
}

// appendAuthor appends a new <author> (no dedup) and returns its 0-based id.
func appendAuthor(authorsElem *etree.Element, name string) int {
	author := newElement(authorsElem.Space, "author")
	author.SetText(name)
	authorsElem.AddChild(author)
	return len(namespaces.FindChildren(authorsElem, namespaces.NsSpreadsheetML, "author")) - 1
}

func newCommentText(text string) *etree.Element {
	textElem := newElement("", "text")
	t := newElement("", "t")
	if needsSpacePreserve(text) {
		t.CreateAttr("xml:space", "preserve")
	}
	t.SetText(text)
	textElem.AddChild(t)
	return textElem
}

func setCommentText(comment *etree.Element, text string) {
	if existing := namespaces.FindChild(comment, namespaces.NsSpreadsheetML, "text"); existing != nil {
		comment.RemoveChild(existing)
	}
	comment.AddChild(newCommentText(text))
}

// insertCommentInOrder inserts a comment keeping commentList sorted by cell ref
// (row-major), matching how Excel orders notes.
func insertCommentInOrder(commentList, comment *etree.Element, normCell string) {
	target, err := address.ParseCell(normCell)
	if err != nil {
		commentList.AddChild(comment)
		return
	}
	for _, existing := range namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment") {
		ref, e := address.ParseCell(existing.SelectAttrValue("ref", ""))
		if e != nil {
			continue
		}
		if ref.Row > target.Row || (ref.Row == target.Row && ref.Column > target.Column) {
			commentList.InsertChildAt(existing.Index(), comment)
			return
		}
	}
	commentList.AddChild(comment)
}

func indexOfComment(commentList, comment *etree.Element) int {
	for i, c := range namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment") {
		if c == comment {
			return i
		}
	}
	return -1
}

func commentByID(commentList *etree.Element, id int) *etree.Element {
	if commentList == nil || id < 0 {
		return nil
	}
	comments := namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment")
	if id >= len(comments) {
		return nil
	}
	return comments[id]
}

func commentsRelExists(session opc.PackageSession, worksheetURI string) bool {
	for _, rel := range session.ListRelationships(worksheetURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelComments {
			return true
		}
	}
	return false
}

func removeCommentsRel(session opc.PackageSession, worksheetURI string) error {
	rels := session.ListRelationships(worksheetURI)
	var kept []opc.RelationshipInfo
	for _, rel := range rels {
		if rel.Type == namespaces.RelComments {
			continue
		}
		kept = append(kept, rel)
	}
	if len(kept) == len(rels) {
		return nil
	}
	if err := opc.WriteRelationships(session, worksheetURI, kept); err != nil {
		return fmt.Errorf("failed to write worksheet relationships: %w", err)
	}
	return nil
}
