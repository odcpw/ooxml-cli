package inspect

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strconv"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// SlideComment is a single legacy p:cm comment with its author resolved from the
// shared commentAuthors part.
type SlideComment struct {
	ID              int      `json:"id"`
	AuthorID        int      `json:"authorId"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Author          string   `json:"author"`
	Initials        string   `json:"initials,omitempty"`
	Date            string   `json:"date,omitempty"`
	Text            string   `json:"text"`
	ContentHash     string   `json:"contentHash"`
}

// SlideComments is the per-slide comment listing returned by ListSlideComments.
type SlideComments struct {
	Slide        int            `json:"slide"`
	SlidePartURI string         `json:"slidePartUri"`
	CommentsPart string         `json:"commentsPart,omitempty"`
	Comments     []SlideComment `json:"comments"`
}

// CommentAuthor is one entry from the shared commentAuthors part.
type CommentAuthor struct {
	ID       int
	Name     string
	Initials string
	LastIdx  int
}

// FindSlideCommentsPart resolves the comments part for a slide, preferring the
// slide's comments relationship. The boolean reports whether the part exists.
func FindSlideCommentsPart(session opc.PackageSession, slideURI string) (string, bool) {
	for _, rel := range session.ListRelationships(slideURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelComments {
			uri := opc.ResolveRelationshipTarget(slideURI, rel.Target)
			for _, part := range session.ListParts() {
				if opc.NormalizeURI(part.URI) == uri {
					return uri, true
				}
			}
			return uri, false
		}
	}
	return "", false
}

// FindCommentAuthorsPart resolves the shared commentAuthors part via the
// presentation.xml relationship, falling back to the conventional URI. The
// boolean reports whether the part exists.
func FindCommentAuthorsPart(session opc.PackageSession) (string, bool) {
	uri := ""
	for _, rel := range session.ListRelationships(namespaces.PresentationPartURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelCommentAuthors {
			uri = opc.ResolveRelationshipTarget(namespaces.PresentationPartURI, rel.Target)
			break
		}
	}
	if uri == "" {
		uri = namespaces.CommentAuthorsPartURI
	}
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return uri, true
		}
	}
	return uri, false
}

// ReadCommentAuthors reads the shared commentAuthors part into a map keyed by
// author id. A missing part yields an empty map rather than an error.
func ReadCommentAuthors(session opc.PackageSession) (map[int]CommentAuthor, error) {
	authors := make(map[int]CommentAuthor)
	uri, exists := FindCommentAuthorsPart(session)
	if !exists {
		return authors, nil
	}
	doc, err := session.ReadXMLPart(uri)
	if err != nil {
		return nil, fmt.Errorf("failed to read comment authors part %s: %w", uri, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsP, "cmAuthorLst") {
		return authors, nil
	}
	for _, el := range namespaces.FindChildren(root, namespaces.NsP, "cmAuthor") {
		a := CommentAuthor{
			ID:       attrInt(el, "id"),
			Name:     el.SelectAttrValue("name", ""),
			Initials: el.SelectAttrValue("initials", ""),
			LastIdx:  attrInt(el, "lastIdx"),
		}
		authors[a.ID] = a
	}
	return authors, nil
}

// ListSlideComments reads the comments part for a slide (if any), resolving each
// comment's author against the shared commentAuthors part. A slide without a
// comments part yields an empty comments slice rather than an error.
func ListSlideComments(session opc.PackageSession, slideURI string, slideNumber int) (*SlideComments, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	result := &SlideComments{
		Slide:        slideNumber,
		SlidePartURI: slideURI,
		Comments:     make([]SlideComment, 0),
	}

	commentsURI, exists := FindSlideCommentsPart(session, slideURI)
	if !exists {
		return result, nil
	}
	result.CommentsPart = commentsURI

	authors, err := ReadCommentAuthors(session)
	if err != nil {
		return nil, err
	}

	doc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsP, "cmLst") {
		return result, nil
	}

	for _, el := range namespaces.FindChildren(root, namespaces.NsP, "cm") {
		result.Comments = append(result.Comments, ReportCommentElement(el, authors))
	}
	return result, nil
}

// ReportCommentElement reads the fields and content hash of one p:cm element,
// resolving its author from the supplied author table.
func ReportCommentElement(el *etree.Element, authors map[int]CommentAuthor) SlideComment {
	authorID := attrInt(el, "authorId")
	date := el.SelectAttrValue("dt", "")
	idx := attrInt(el, "idx")
	text := commentBodyText(el)

	author := authors[authorID]
	return SlideComment{
		ID:          idx,
		AuthorID:    authorID,
		Author:      author.Name,
		Initials:    author.Initials,
		Date:        date,
		Text:        text,
		ContentHash: CommentContentHash(author.Name, date, text),
	}
}

// commentBodyText returns the plain text of a p:cm (its p:text child).
func commentBodyText(cm *etree.Element) string {
	if t := namespaces.FindChild(cm, namespaces.NsP, "text"); t != nil {
		return t.Text()
	}
	return ""
}

// CommentContentHash hashes the semantic identity of a comment (author, date,
// text), matching the DOCX comment hash format.
func CommentContentHash(author, date, text string) string {
	hash := sha256.New()
	hash.Write([]byte(author))
	hash.Write([]byte{0})
	hash.Write([]byte(date))
	hash.Write([]byte{0})
	hash.Write([]byte(text))
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}

func attrInt(el *etree.Element, name string) int {
	if el == nil {
		return 0
	}
	n, _ := strconv.Atoi(el.SelectAttrValue(name, ""))
	return n
}
