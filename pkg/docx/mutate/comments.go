package mutate

import (
	"errors"
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	// ErrCommentHashMismatch is returned when an --expect-hash guard does not match.
	ErrCommentHashMismatch = errors.New("comment hash mismatch")
	// ErrCommentNotFound is returned when a comment id is absent from comments.xml.
	ErrCommentNotFound = errors.New("comment not found")
	// ErrCommentAnchorOutOfRange is returned when --anchor-block is out of range.
	ErrCommentAnchorOutOfRange = errors.New("comment anchor block out of range")
	// ErrCommentAnchorNotParagraph is returned when the anchor block is not a paragraph.
	ErrCommentAnchorNotParagraph = errors.New("comment anchor block is not a paragraph")
)

// AddCommentRequest anchors a new comment to a body block.
type AddCommentRequest struct {
	Package     opc.PackageSession
	DocumentURI string
	AnchorBlock int // 1-based body block; 0 anchors to the first block
	Author      string
	Initials    string
	Date        string
	Text        string
}

// AddCommentResult reports the created comment.
type AddCommentResult struct {
	CommentID       int    `json:"commentId"`
	Author          string `json:"author"`
	Date            string `json:"date,omitempty"`
	Initials        string `json:"initials,omitempty"`
	Text            string `json:"text"`
	ContentHash     string `json:"contentHash"`
	AnchoredToBlock int    `json:"anchoredToBlock"`
	CreatedPart     bool   `json:"createdPart"`
	CreatedRef      bool   `json:"createdRef"`
}

// EditCommentRequest updates an existing comment's text/author/date by id.
type EditCommentRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	CommentID    int
	ExpectedHash string
	Text         string
	TextSet      bool
	Author       string
	AuthorSet    bool
	Date         string
	DateSet      bool
}

// EditCommentResult reports the edited comment.
type EditCommentResult struct {
	CommentID    int    `json:"commentId"`
	Author       string `json:"author"`
	Date         string `json:"date,omitempty"`
	Initials     string `json:"initials,omitempty"`
	Text         string `json:"text"`
	ContentHash  string `json:"contentHash"`
	PreviousText string `json:"previousText"`
	PreviousHash string `json:"previousHash"`
}

// RemoveCommentRequest deletes a comment entry and its body markers by id.
type RemoveCommentRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	CommentID    int
	ExpectedHash string
}

// RemoveCommentResult reports the removed comment.
type RemoveCommentResult struct {
	CommentID           int    `json:"commentId"`
	PreviousAuthor      string `json:"previousAuthor"`
	PreviousText        string `json:"previousText"`
	PreviousHash        string `json:"previousHash"`
	RangeMarkersRemoved bool   `json:"rangeMarkersRemoved"`
}

// AddComment inserts a comment, creating the comments part, its content-type override,
// and the document relationship if they do not yet exist. It anchors the comment to the
// target body block via w:commentRangeStart/End and a w:commentReference run.
func AddComment(req *AddCommentRequest) (*AddCommentResult, error) {
	if req == nil {
		return nil, fmt.Errorf("add comment request is nil")
	}
	if req.Author == "" {
		return nil, fmt.Errorf("author is required")
	}

	doc, bodyElem, prefix, err := locateBody(req.Package, req.DocumentURI)
	if err != nil {
		return nil, err
	}

	blocks := docxbody.Blocks(bodyElem)
	if len(blocks) == 0 {
		return nil, fmt.Errorf("document has no body blocks to anchor a comment to")
	}
	anchorIndex := req.AnchorBlock
	if anchorIndex == 0 {
		anchorIndex = 1
	}
	if anchorIndex < 1 || anchorIndex > len(blocks) {
		return nil, fmt.Errorf("%w: %d", ErrCommentAnchorOutOfRange, anchorIndex)
	}
	target := blocks[anchorIndex-1]
	if target.Kind != "paragraph" {
		return nil, fmt.Errorf("%w: block %d is %s", ErrCommentAnchorNotParagraph, anchorIndex, target.Kind)
	}

	// Ensure the comments part, content type, and relationship exist.
	commentsURI, createdPart, createdRef, err := ensureCommentsPart(req.Package, req.DocumentURI)
	if err != nil {
		return nil, err
	}
	commentsDoc, err := req.Package.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	commentsRoot := commentsDoc.Root()
	if commentsRoot == nil || !namespaces.IsElement(commentsRoot, namespaces.NsW, "comments") {
		return nil, fmt.Errorf("comments part %s has no w:comments root", commentsURI)
	}

	newID := nextCommentID(commentsRoot)
	idStr := strconv.Itoa(newID)

	// Insert body markers into the target paragraph.
	insertCommentMarkers(target.Element, prefix, idStr)
	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}

	// Append the comment entry to comments.xml.
	comment := newCommentElement(commentsRoot.Space, idStr, req.Author, req.Date, req.Initials, req.Text)
	commentsRoot.AddChild(comment)
	if err := req.Package.ReplaceXMLPart(commentsURI, commentsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	return &AddCommentResult{
		CommentID:       newID,
		Author:          req.Author,
		Date:            req.Date,
		Initials:        req.Initials,
		Text:            req.Text,
		ContentHash:     docxinspect.CommentContentHash(req.Author, req.Date, req.Text),
		AnchoredToBlock: anchorIndex,
		CreatedPart:     createdPart,
		CreatedRef:      createdRef,
	}, nil
}

// EditComment updates a comment's text, author, and/or date by id.
func EditComment(req *EditCommentRequest) (*EditCommentResult, error) {
	if req == nil {
		return nil, fmt.Errorf("edit comment request is nil")
	}
	commentsURI, exists := docxinspect.FindCommentsPart(req.Package, req.DocumentURI)
	if !exists {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}
	commentsDoc, err := req.Package.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := commentsDoc.Root()
	comment := findCommentByID(root, req.CommentID)
	if comment == nil {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}

	before := docxinspect.ReportCommentElement(comment)
	if req.ExpectedHash != "" && req.ExpectedHash != before.ContentHash {
		return nil, fmt.Errorf("%w: comment %d expected %s but found %s", ErrCommentHashMismatch, req.CommentID, req.ExpectedHash, before.ContentHash)
	}

	author := before.Author
	if req.AuthorSet {
		author = req.Author
		setWordAttr(comment, "author", author)
	}
	date := before.Date
	if req.DateSet {
		date = req.Date
		setWordAttr(comment, "date", date)
	}
	text := before.Text
	if req.TextSet {
		// Editing text replaces the comment body with a single paragraph. Refuse
		// to silently collapse a multi-paragraph comment so structured content is
		// not lost without the caller knowing.
		if paraCount := len(namespaces.FindChildren(comment, namespaces.NsW, "p")); paraCount > 1 {
			return nil, fmt.Errorf("comment %d has %d paragraphs; editing its text would discard structure (remove and re-add the comment instead)", req.CommentID, paraCount)
		}
		text = req.Text
		setCommentText(comment, text)
	}

	if err := req.Package.ReplaceXMLPart(commentsURI, commentsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	return &EditCommentResult{
		CommentID:    req.CommentID,
		Author:       author,
		Date:         date,
		Initials:     before.Initials,
		Text:         text,
		ContentHash:  docxinspect.CommentContentHash(author, date, text),
		PreviousText: before.Text,
		PreviousHash: before.ContentHash,
	}, nil
}

// RemoveComment deletes a comment entry and removes its range markers and reference
// run from the document body. Orphaned comments (no body markers) are still removed.
func RemoveComment(req *RemoveCommentRequest) (*RemoveCommentResult, error) {
	if req == nil {
		return nil, fmt.Errorf("remove comment request is nil")
	}
	commentsURI, exists := docxinspect.FindCommentsPart(req.Package, req.DocumentURI)
	if !exists {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}
	commentsDoc, err := req.Package.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := commentsDoc.Root()
	comment := findCommentByID(root, req.CommentID)
	if comment == nil {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}

	before := docxinspect.ReportCommentElement(comment)
	if req.ExpectedHash != "" && req.ExpectedHash != before.ContentHash {
		return nil, fmt.Errorf("%w: comment %d expected %s but found %s", ErrCommentHashMismatch, req.CommentID, req.ExpectedHash, before.ContentHash)
	}

	// Remove the entry from comments.xml.
	root.RemoveChild(comment)
	if err := req.Package.ReplaceXMLPart(commentsURI, commentsDoc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	// Remove body markers (range start/end + reference run) for this id.
	markersRemoved, err := removeCommentMarkers(req.Package, req.DocumentURI, req.CommentID)
	if err != nil {
		return nil, err
	}

	return &RemoveCommentResult{
		CommentID:           req.CommentID,
		PreviousAuthor:      before.Author,
		PreviousText:        before.Text,
		PreviousHash:        before.ContentHash,
		RangeMarkersRemoved: markersRemoved,
	}, nil
}

// ensureCommentsPart guarantees the comments part, its content-type override (via
// AddPart), and the document relationship exist. It returns the resolved URI and
// whether the part / relationship were newly created.
func ensureCommentsPart(session opc.PackageSession, documentURI string) (string, bool, bool, error) {
	commentsURI, exists := docxinspect.FindCommentsPart(session, documentURI)
	createdPart := false
	if !exists {
		if err := session.AddPart(commentsURI, commentsTemplate(), namespaces.ContentTypeComments, nil); err != nil {
			return "", false, false, fmt.Errorf("failed to add comments part %s: %w", commentsURI, err)
		}
		createdPart = true
	}

	createdRef := false
	if !relationshipExists(session, documentURI, namespaces.RelComments) {
		rels := session.ListRelationships(documentURI)
		relID := opc.AllocateRelationshipID(rels)
		rels = append(rels, opc.RelationshipInfo{
			SourceURI: documentURI,
			ID:        relID,
			Type:      namespaces.RelComments,
			Target:    opc.RelationshipTarget(documentURI, commentsURI),
		})
		if err := opc.WriteRelationships(session, documentURI, rels); err != nil {
			return "", false, false, fmt.Errorf("failed to write comments relationship: %w", err)
		}
		createdRef = true
	}
	return commentsURI, createdPart, createdRef, nil
}

func relationshipExists(session opc.PackageSession, sourceURI, relType string) bool {
	for _, rel := range session.ListRelationships(sourceURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == relType {
			return true
		}
	}
	return false
}

// insertCommentMarkers wraps the existing run content of a paragraph with
// w:commentRangeStart/End siblings and appends a w:commentReference run.
func insertCommentMarkers(paragraph *etree.Element, prefix, idStr string) {
	start := newElement(prefix, "commentRangeStart")
	start.CreateAttr(wordAttrName(prefix, "id"), idStr)
	end := newElement(prefix, "commentRangeEnd")
	end.CreateAttr(wordAttrName(prefix, "id"), idStr)

	refRun := newElement(prefix, "r")
	ref := newElement(prefix, "commentReference")
	ref.CreateAttr(wordAttrName(prefix, "id"), idStr)
	refRun.AddChild(ref)

	// Find the first and last run children so the markers wrap the content.
	var firstRun, lastRun *etree.Element
	for _, child := range paragraph.ChildElements() {
		if docxbody.LocalName(child.Tag) == "r" {
			if firstRun == nil {
				firstRun = child
			}
			lastRun = child
		}
	}

	if firstRun != nil {
		paragraph.InsertChildAt(firstRun.Index(), start)
	} else {
		// No runs: place start after pPr (or at front).
		paragraph.InsertChildAt(afterPPrIndex(paragraph), start)
	}

	if lastRun != nil {
		paragraph.InsertChildAt(lastRun.Index()+1, end)
	} else {
		paragraph.InsertChildAt(start.Index()+1, end)
	}
	paragraph.InsertChildAt(end.Index()+1, refRun)
}

func afterPPrIndex(paragraph *etree.Element) int {
	for _, child := range paragraph.ChildElements() {
		if docxbody.LocalName(child.Tag) == "pPr" {
			return child.Index() + 1
		}
	}
	return 0
}

// removeCommentMarkers deletes w:commentRangeStart/End and the w:commentReference run
// matching the id from the body. Returns whether any markers were removed.
func removeCommentMarkers(session opc.PackageSession, documentURI string, commentID int) (bool, error) {
	doc, bodyElem, _, err := locateBody(session, documentURI)
	if err != nil {
		return false, err
	}
	idStr := strconv.Itoa(commentID)
	removed := false

	for _, tag := range []string{"commentRangeStart", "commentRangeEnd"} {
		for _, marker := range namespaces.FindDescendants(bodyElem, namespaces.NsW, tag) {
			if id, _ := namespaces.Attr(marker, namespaces.NsW, "id"); id == idStr {
				if parent := marker.Parent(); parent != nil {
					parent.RemoveChild(marker)
					removed = true
				}
			}
		}
	}

	// Remove reference runs: a w:r whose only meaningful child is a matching
	// w:commentReference.
	for _, ref := range namespaces.FindDescendants(bodyElem, namespaces.NsW, "commentReference") {
		if id, _ := namespaces.Attr(ref, namespaces.NsW, "id"); id != idStr {
			continue
		}
		run := ref.Parent()
		if run != nil && docxbody.LocalName(run.Tag) == "r" {
			if parent := run.Parent(); parent != nil {
				parent.RemoveChild(run)
				removed = true
				continue
			}
		}
		if run != nil {
			run.RemoveChild(ref)
			removed = true
		}
	}

	if !removed {
		return false, nil
	}
	ensureDocumentTableScaffolds(doc.Root(), doc.Root().Space)
	if err := session.ReplaceXMLPart(documentURI, doc); err != nil {
		return false, fmt.Errorf("failed to replace document part %s: %w", documentURI, err)
	}
	return true, nil
}

func nextCommentID(commentsRoot *etree.Element) int {
	max := -1
	for _, comment := range namespaces.FindChildren(commentsRoot, namespaces.NsW, "comment") {
		if id, ok := namespaces.Attr(comment, namespaces.NsW, "id"); ok {
			if n, err := strconv.Atoi(id); err == nil && n > max {
				max = n
			}
		}
	}
	return max + 1
}

func findCommentByID(commentsRoot *etree.Element, id int) *etree.Element {
	if commentsRoot == nil {
		return nil
	}
	idStr := strconv.Itoa(id)
	for _, comment := range namespaces.FindChildren(commentsRoot, namespaces.NsW, "comment") {
		if v, ok := namespaces.Attr(comment, namespaces.NsW, "id"); ok && v == idStr {
			return comment
		}
	}
	return nil
}

func newCommentElement(prefix, idStr, author, date, initials, text string) *etree.Element {
	comment := newElement(prefix, "comment")
	comment.CreateAttr(wordAttrName(prefix, "id"), idStr)
	comment.CreateAttr(wordAttrName(prefix, "author"), author)
	if date != "" {
		comment.CreateAttr(wordAttrName(prefix, "date"), date)
	}
	if initials != "" {
		comment.CreateAttr(wordAttrName(prefix, "initials"), initials)
	}
	p := newElement(prefix, "p")
	if text != "" {
		run := newElement(prefix, "r")
		appendTextChildren(run, prefix, text)
		p.AddChild(run)
	}
	comment.AddChild(p)
	return comment
}

func setCommentText(comment *etree.Element, text string) {
	prefix := comment.Space
	for _, child := range comment.ChildElements() {
		comment.RemoveChild(child)
	}
	p := newElement(prefix, "p")
	if text != "" {
		run := newElement(prefix, "r")
		appendTextChildren(run, prefix, text)
		p.AddChild(run)
	}
	comment.AddChild(p)
}

func setWordAttr(elem *etree.Element, local, value string) {
	if existing, ok := namespaces.Attr(elem, namespaces.NsW, local); ok {
		_ = existing
		// Update whichever attribute key currently carries the value.
		prefix := elem.Space
		key := local
		if prefix != "" {
			key = prefix + ":" + local
		}
		if attr := elem.SelectAttr(key); attr != nil {
			attr.Value = value
			return
		}
		if attr := elem.SelectAttr("w:" + local); attr != nil {
			attr.Value = value
			return
		}
	}
	elem.CreateAttr(wordAttrName(elem.Space, local), value)
}

func wordAttrName(prefix, local string) string {
	if prefix != "" {
		return prefix + ":" + local
	}
	return "w:" + local
}

func commentsTemplate() []byte {
	return []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
		`<w:comments xmlns:w="` + namespaces.NsW + `" xmlns:r="` + namespaces.NsR + `"></w:comments>`)
}
