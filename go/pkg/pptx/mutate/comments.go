package mutate

import (
	"errors"
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

var (
	// ErrCommentHashMismatch is returned when an --expect-hash guard does not match.
	ErrCommentHashMismatch = errors.New("comment hash mismatch")
	// ErrCommentNotFound is returned when a comment id is absent from a slide's comments part.
	ErrCommentNotFound = errors.New("comment not found")
	// ErrSlideOutOfRange is returned when the targeted slide does not exist.
	ErrSlideOutOfRange = errors.New("slide out of range")
	// ErrCommentAmbiguous is returned when a comment id (idx) matches more than one
	// p:cm on a slide because legacy decks allocate idx per-author. The caller must
	// disambiguate with --author-id.
	ErrCommentAmbiguous = errors.New("comment id is ambiguous; specify --author-id")
)

var commentsPartNamePattern = regexp.MustCompile(`^/ppt/comments/comment(\d+)\.xml$`)

// AddCommentRequest adds a comment anchored to a slide.
type AddCommentRequest struct {
	Package     opc.PackageSession
	SlideNumber int
	Author      string
	Initials    string
	Date        string
	Text        string
}

// AddCommentResult reports the created comment.
type AddCommentResult struct {
	Slide               int    `json:"slide"`
	SlidePartURI        string `json:"slidePartUri"`
	CommentsPart        string `json:"commentsPart"`
	CommentID           int    `json:"commentId"`
	AuthorID            int    `json:"authorId"`
	Author              string `json:"author"`
	Initials            string `json:"initials,omitempty"`
	Date                string `json:"date,omitempty"`
	Text                string `json:"text"`
	ContentHash         string `json:"contentHash"`
	CreatedPart         bool   `json:"createdPart"`
	CreatedRelationship bool   `json:"createdRelationship"`
	CreatedAuthorsPart  bool   `json:"createdAuthorsPart"`
	CreatedAuthor       bool   `json:"createdAuthor"`
}

// EditCommentRequest updates an existing comment's text/author/date by id.
type EditCommentRequest struct {
	Package      opc.PackageSession
	SlideNumber  int
	CommentID    int
	AuthorID     int
	AuthorIDSet  bool
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
	Slide        int    `json:"slide"`
	SlidePartURI string `json:"slidePartUri"`
	CommentsPart string `json:"commentsPart"`
	CommentID    int    `json:"commentId"`
	AuthorID     int    `json:"authorId"`
	Author       string `json:"author"`
	Initials     string `json:"initials,omitempty"`
	Date         string `json:"date,omitempty"`
	Text         string `json:"text"`
	ContentHash  string `json:"contentHash"`
	PreviousText string `json:"previousText"`
	PreviousHash string `json:"previousHash"`
}

// RemoveCommentRequest deletes a comment entry by id.
type RemoveCommentRequest struct {
	Package      opc.PackageSession
	SlideNumber  int
	CommentID    int
	AuthorID     int
	AuthorIDSet  bool
	ExpectedHash string
}

// RemoveCommentResult reports the removed comment.
type RemoveCommentResult struct {
	Slide          int    `json:"slide"`
	SlidePartURI   string `json:"slidePartUri"`
	CommentsPart   string `json:"commentsPart"`
	CommentID      int    `json:"commentId"`
	AuthorID       int    `json:"authorId"`
	PreviousAuthor string `json:"previousAuthor"`
	PreviousText   string `json:"previousText"`
	PreviousHash   string `json:"previousHash"`
	RemovedPart    bool   `json:"removedPart"`
}

// AddComment inserts a comment on the targeted slide, creating the per-slide
// comments part, the shared commentAuthors part, their content-type overrides,
// and the slide / presentation relationships when they do not yet exist.
func AddComment(req *AddCommentRequest) (*AddCommentResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("add comment requires an open package session")
	}
	if req.Author == "" {
		return nil, fmt.Errorf("author is required")
	}
	session := req.Package

	slide, err := resolveSlide(session, req.SlideNumber)
	if err != nil {
		return nil, err
	}

	// Ensure the per-slide comments part and its slide relationship exist.
	commentsURI, createdPart, createdRel, err := ensureSlideCommentsPart(session, slide.PartURI)
	if err != nil {
		return nil, err
	}

	doc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsP, "cmLst") {
		return nil, fmt.Errorf("comments part %s has no p:cmLst root", commentsURI)
	}

	// Allocate a slide-global comment index so --comment-id uniquely identifies a
	// comment on the slide regardless of author.
	nextIdx := nextSlideCommentIdx(root)

	// Resolve or create the author, bumping its lastIdx high-water mark to the
	// allocated index.
	authorID, createdAuthorsPart, createdAuthor, err := ensureCommentAuthor(session, req.Author, req.Initials, nextIdx)
	if err != nil {
		return nil, err
	}

	cm := newCommentElement(authorID, nextIdx, req.Date, req.Text)
	root.AddChild(cm)
	if err := session.ReplaceXMLPart(commentsURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	return &AddCommentResult{
		Slide:               req.SlideNumber,
		SlidePartURI:        slide.PartURI,
		CommentsPart:        commentsURI,
		CommentID:           nextIdx,
		AuthorID:            authorID,
		Author:              req.Author,
		Initials:            req.Initials,
		Date:                req.Date,
		Text:                req.Text,
		ContentHash:         inspect.CommentContentHash(req.Author, req.Date, req.Text),
		CreatedPart:         createdPart,
		CreatedRelationship: createdRel,
		CreatedAuthorsPart:  createdAuthorsPart,
		CreatedAuthor:       createdAuthor,
	}, nil
}

// EditComment updates a comment's text, author, and/or date by id on a slide.
func EditComment(req *EditCommentRequest) (*EditCommentResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("edit comment requires an open package session")
	}
	session := req.Package
	slide, err := resolveSlide(session, req.SlideNumber)
	if err != nil {
		return nil, err
	}
	commentsURI, exists := inspect.FindSlideCommentsPart(session, slide.PartURI)
	if !exists {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}
	doc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := doc.Root()
	cm, err := findCommentByID(root, req.AuthorID, req.AuthorIDSet, req.CommentID)
	if err != nil {
		return nil, err
	}
	if cm == nil {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}

	authors, err := inspect.ReadCommentAuthors(session)
	if err != nil {
		return nil, err
	}
	before := inspect.ReportCommentElement(cm, authors)
	if req.ExpectedHash != "" && req.ExpectedHash != before.ContentHash {
		return nil, fmt.Errorf("%w: comment %d expected %s but found %s", ErrCommentHashMismatch, req.CommentID, req.ExpectedHash, before.ContentHash)
	}

	authorID := before.AuthorID
	authorName := before.Author
	initials := before.Initials
	if req.AuthorSet {
		id, _, _, err := ensureCommentAuthor(session, req.Author, "", 0)
		if err != nil {
			return nil, err
		}
		authorID = id
		authorName = req.Author
		// Re-read authors to capture the resolved initials.
		authors, err = inspect.ReadCommentAuthors(session)
		if err != nil {
			return nil, err
		}
		initials = authors[id].Initials
		cm.CreateAttr("authorId", strconv.Itoa(id))
	}

	date := before.Date
	if req.DateSet {
		date = req.Date
		if date == "" {
			removeAttr(cm, "dt")
		} else {
			cm.CreateAttr("dt", date)
		}
	}

	text := before.Text
	if req.TextSet {
		text = req.Text
		setCommentBodyText(cm, text)
	}

	if err := session.ReplaceXMLPart(commentsURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
	}

	return &EditCommentResult{
		Slide:        req.SlideNumber,
		SlidePartURI: slide.PartURI,
		CommentsPart: commentsURI,
		CommentID:    req.CommentID,
		AuthorID:     authorID,
		Author:       authorName,
		Initials:     initials,
		Date:         date,
		Text:         text,
		ContentHash:  inspect.CommentContentHash(authorName, date, text),
		PreviousText: before.Text,
		PreviousHash: before.ContentHash,
	}, nil
}

// RemoveComment deletes a comment entry by id from a slide's comments part. When
// the part becomes empty, the part and its slide relationship are removed.
func RemoveComment(req *RemoveCommentRequest) (*RemoveCommentResult, error) {
	if req == nil || req.Package == nil {
		return nil, fmt.Errorf("remove comment requires an open package session")
	}
	session := req.Package
	slide, err := resolveSlide(session, req.SlideNumber)
	if err != nil {
		return nil, err
	}
	commentsURI, exists := inspect.FindSlideCommentsPart(session, slide.PartURI)
	if !exists {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}
	doc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := doc.Root()
	cm, err := findCommentByID(root, req.AuthorID, req.AuthorIDSet, req.CommentID)
	if err != nil {
		return nil, err
	}
	if cm == nil {
		return nil, fmt.Errorf("%w: %d", ErrCommentNotFound, req.CommentID)
	}

	authors, err := inspect.ReadCommentAuthors(session)
	if err != nil {
		return nil, err
	}
	before := inspect.ReportCommentElement(cm, authors)
	if req.ExpectedHash != "" && req.ExpectedHash != before.ContentHash {
		return nil, fmt.Errorf("%w: comment %d expected %s but found %s", ErrCommentHashMismatch, req.CommentID, req.ExpectedHash, before.ContentHash)
	}

	root.RemoveChild(cm)

	removedPart := false
	if len(namespaces.FindChildren(root, namespaces.NsP, "cm")) == 0 {
		// Drop the now-empty comments part and its slide relationship.
		if err := removeSlideCommentsPart(session, slide.PartURI, commentsURI); err != nil {
			return nil, err
		}
		removedPart = true
	} else {
		if err := session.ReplaceXMLPart(commentsURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace comments part %s: %w", commentsURI, err)
		}
	}

	return &RemoveCommentResult{
		Slide:          req.SlideNumber,
		SlidePartURI:   slide.PartURI,
		CommentsPart:   commentsURI,
		CommentID:      req.CommentID,
		AuthorID:       before.AuthorID,
		PreviousAuthor: before.Author,
		PreviousText:   before.Text,
		PreviousHash:   before.ContentHash,
		RemovedPart:    removedPart,
	}, nil
}

// resolveSlide maps a 1-based slide number to its SlideRef.
func resolveSlide(session opc.PackageSession, slideNumber int) (inspect.SlideRef, error) {
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		return inspect.SlideRef{}, fmt.Errorf("failed to parse presentation: %w", err)
	}
	if slideNumber < 1 || slideNumber > len(graph.Slides) {
		return inspect.SlideRef{}, fmt.Errorf("%w: slide %d (presentation has %d slides)", ErrSlideOutOfRange, slideNumber, len(graph.Slides))
	}
	return graph.Slides[slideNumber-1], nil
}

// nextSlideCommentIdx returns the next slide-global comment index (max existing
// idx + 1, starting at 1) so --comment-id uniquely identifies a comment on the
// slide regardless of author.
func nextSlideCommentIdx(root *etree.Element) int {
	max := 0
	for _, cm := range namespaces.FindChildren(root, namespaces.NsP, "cm") {
		if n, err := strconv.Atoi(cm.SelectAttrValue("idx", "")); err == nil && n > max {
			max = n
		}
	}
	return max + 1
}

// ensureCommentAuthor resolves an author by name in the shared commentAuthors
// part, creating the part and/or the author entry when absent. It returns the
// author id and whether the part / author entry were newly created. forIdx, when
// non-zero, raises the author's lastIdx high-water mark to at least forIdx.
func ensureCommentAuthor(session opc.PackageSession, name, initials string, forIdx int) (authorID int, createdPart bool, createdAuthor bool, err error) {
	uri, exists := inspect.FindCommentAuthorsPart(session)
	if !exists {
		if err := session.AddPart(uri, commentAuthorsTemplate(), namespaces.ContentTypeCommentAuthors, nil); err != nil {
			return 0, false, false, fmt.Errorf("failed to add comment authors part %s: %w", uri, err)
		}
		if err := ensurePresentationCommentAuthorsRel(session, uri); err != nil {
			return 0, false, false, err
		}
		createdPart = true
	}

	doc, err := session.ReadXMLPart(uri)
	if err != nil {
		return 0, false, false, fmt.Errorf("failed to read comment authors part %s: %w", uri, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsP, "cmAuthorLst") {
		return 0, false, false, fmt.Errorf("comment authors part %s has no p:cmAuthorLst root", uri)
	}

	// Look for an existing author with the same name.
	var target *etree.Element
	maxID := -1
	for _, a := range namespaces.FindChildren(root, namespaces.NsP, "cmAuthor") {
		id, _ := strconv.Atoi(a.SelectAttrValue("id", ""))
		if id > maxID {
			maxID = id
		}
		if a.SelectAttrValue("name", "") == name {
			target = a
		}
	}

	if target == nil {
		authorID = maxID + 1
		target = newCommentAuthorElement(authorID, name, initials)
		root.AddChild(target)
		createdAuthor = true
	} else {
		authorID, _ = strconv.Atoi(target.SelectAttrValue("id", ""))
	}

	// Keep lastIdx as a high-water mark of indices used by this author.
	if forIdx > 0 {
		lastIdx, _ := strconv.Atoi(target.SelectAttrValue("lastIdx", "0"))
		if forIdx > lastIdx {
			target.CreateAttr("lastIdx", strconv.Itoa(forIdx))
		}
	}

	if err := session.ReplaceXMLPart(uri, doc); err != nil {
		return 0, false, false, fmt.Errorf("failed to replace comment authors part %s: %w", uri, err)
	}
	return authorID, createdPart, createdAuthor, nil
}

// ensurePresentationCommentAuthorsRel adds the presentation.xml -> commentAuthors
// relationship if absent.
func ensurePresentationCommentAuthorsRel(session opc.PackageSession, authorsURI string) error {
	src := namespaces.PresentationPartURI
	for _, rel := range session.ListRelationships(src) {
		if rel.Type == namespaces.RelCommentAuthors && rel.TargetMode != "External" {
			return nil
		}
	}
	rels := session.ListRelationships(src)
	target, err := relationshipTarget(src, authorsURI)
	if err != nil {
		return err
	}
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: src,
		ID:        opc.AllocateRelationshipID(rels),
		Type:      namespaces.RelCommentAuthors,
		Target:    target,
	})
	if err := opc.WriteRelationships(session, src, rels); err != nil {
		return fmt.Errorf("failed to write comment authors relationship: %w", err)
	}
	return nil
}

// ensureSlideCommentsPart returns the comments part URI for a slide, creating the
// part, its content-type override, and the slide relationship when absent.
func ensureSlideCommentsPart(session opc.PackageSession, slideURI string) (string, bool, bool, error) {
	commentsURI, exists := inspect.FindSlideCommentsPart(session, slideURI)
	if exists {
		return commentsURI, false, false, nil
	}

	uri, err := allocateNumberedPartName(session, commentsPartNamePattern, "/ppt/comments/comment%d.xml")
	if err != nil {
		return "", false, false, err
	}
	if err := session.AddPart(uri, commentsTemplate(), namespaces.ContentTypeComments, nil); err != nil {
		return "", false, false, fmt.Errorf("failed to add comments part %s: %w", uri, err)
	}

	rels := session.ListRelationships(slideURI)
	target, err := relationshipTarget(slideURI, uri)
	if err != nil {
		return "", false, false, err
	}
	rels = append(rels, opc.RelationshipInfo{
		SourceURI: slideURI,
		ID:        opc.AllocateRelationshipID(rels),
		Type:      namespaces.RelComments,
		Target:    target,
	})
	if err := opc.WriteRelationships(session, slideURI, rels); err != nil {
		return "", false, false, fmt.Errorf("failed to write slide comments relationship: %w", err)
	}
	return uri, true, true, nil
}

// removeSlideCommentsPart deletes the comments part and removes the slide's
// comments relationship.
func removeSlideCommentsPart(session opc.PackageSession, slideURI, commentsURI string) error {
	if err := session.RemovePart(commentsURI); err != nil {
		return fmt.Errorf("failed to remove comments part %s: %w", commentsURI, err)
	}
	rels := session.ListRelationships(slideURI)
	kept := make([]opc.RelationshipInfo, 0, len(rels))
	for _, rel := range rels {
		if rel.Type == namespaces.RelComments && rel.TargetMode != "External" &&
			opc.ResolveRelationshipTarget(slideURI, rel.Target) == commentsURI {
			continue
		}
		kept = append(kept, rel)
	}
	if err := opc.WriteRelationships(session, slideURI, kept); err != nil {
		return fmt.Errorf("failed to rewrite slide comments relationship: %w", err)
	}
	return nil
}

// findCommentByID locates a comment by its compound (authorId, idx) address.
//
// Legacy decks allocate p:cm/@idx per-author, so a slide may legitimately carry
// <p:cm authorId="0" idx="1"> and <p:cm authorId="1" idx="1"> simultaneously.
// When authorIDSet is true the match requires BOTH authorId and idx. When it is
// false the match is by idx alone, but only if exactly one p:cm has that idx;
// otherwise ErrCommentAmbiguous is returned so the caller can disambiguate with
// --author-id. A nil result with a nil error means the comment was not found.
func findCommentByID(root *etree.Element, authorID int, authorIDSet bool, idx int) (*etree.Element, error) {
	if root == nil {
		return nil, nil
	}
	idStr := strconv.Itoa(idx)
	authorStr := strconv.Itoa(authorID)

	var matches []*etree.Element
	for _, cm := range namespaces.FindChildren(root, namespaces.NsP, "cm") {
		if cm.SelectAttrValue("idx", "") != idStr {
			continue
		}
		if authorIDSet {
			if cm.SelectAttrValue("authorId", "") == authorStr {
				return cm, nil
			}
			continue
		}
		matches = append(matches, cm)
	}

	if authorIDSet {
		return nil, nil
	}
	switch len(matches) {
	case 0:
		return nil, nil
	case 1:
		return matches[0], nil
	default:
		ids := make([]string, 0, len(matches))
		for _, cm := range matches {
			ids = append(ids, cm.SelectAttrValue("authorId", ""))
		}
		return nil, fmt.Errorf("%w: comment idx %d matches authorIds %s on this slide", ErrCommentAmbiguous, idx, strings.Join(ids, ", "))
	}
}

// newCommentElement builds a schema-ordered p:cm (p:pos then p:text).
func newCommentElement(authorID, idx int, date, text string) *etree.Element {
	cm := etree.NewElement("p:cm")
	cm.CreateAttr("authorId", strconv.Itoa(authorID))
	if date != "" {
		cm.CreateAttr("dt", date)
	}
	cm.CreateAttr("idx", strconv.Itoa(idx))

	pos := etree.NewElement("p:pos")
	pos.CreateAttr("x", "0")
	pos.CreateAttr("y", "0")
	cm.AddChild(pos)

	t := etree.NewElement("p:text")
	t.SetText(text)
	cm.AddChild(t)
	return cm
}

// setCommentBodyText replaces the p:text content of a p:cm, preserving child order.
func setCommentBodyText(cm *etree.Element, text string) {
	t := namespaces.FindChild(cm, namespaces.NsP, "text")
	if t == nil {
		t = etree.NewElement("p:text")
		cm.AddChild(t)
	}
	t.SetText(text)
}

func newCommentAuthorElement(id int, name, initials string) *etree.Element {
	a := etree.NewElement("p:cmAuthor")
	a.CreateAttr("id", strconv.Itoa(id))
	a.CreateAttr("name", name)
	a.CreateAttr("initials", initials)
	a.CreateAttr("lastIdx", "0")
	a.CreateAttr("clrIdx", strconv.Itoa(id))
	return a
}

func removeAttr(el *etree.Element, key string) {
	el.RemoveAttr(key)
}

func commentsTemplate() []byte {
	return []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
		`<p:cmLst xmlns:a="` + namespaces.NsA + `" xmlns:p="` + namespaces.NsP + `" xmlns:r="` + namespaces.NsR + `"></p:cmLst>`)
}

func commentAuthorsTemplate() []byte {
	return []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>` +
		`<p:cmAuthorLst xmlns:a="` + namespaces.NsA + `" xmlns:p="` + namespaces.NsP + `" xmlns:r="` + namespaces.NsR + `"></p:cmAuthorLst>`)
}
