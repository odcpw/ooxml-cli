package inspect

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// CommentsPartURI is the conventional location of the comments part.
const CommentsPartURI = "/word/comments.xml"

// Comment is a single comment enriched with its body anchor (if found).
type Comment struct {
	ID                  int      `json:"id"`
	Author              string   `json:"author"`
	Date                string   `json:"date,omitempty"`
	Initials            string   `json:"initials,omitempty"`
	Text                string   `json:"text"`
	ContentHash         string   `json:"contentHash"`
	AnchoredToBlock     int      `json:"anchoredToBlock,omitempty"`
	AnchoredToBlockKind string   `json:"anchoredToBlockKind,omitempty"`
	PrimarySelector     string   `json:"primarySelector,omitempty"`
	Selectors           []string `json:"selectors,omitempty"`
	// Handle is the stable comment handle (H:docx/pt:doc/comment:n:<id>) built
	// from the native w:id. It is the same string the mutate side accepts.
	Handle string `json:"handle,omitempty"`
}

// DocumentComments is the top-level listing returned by ListComments.
type DocumentComments struct {
	DocumentPartURI string    `json:"documentPartUri"`
	CommentsPart    string    `json:"commentsPart,omitempty"`
	Comments        []Comment `json:"comments"`
}

// FindCommentsPart returns the URI of the comments part for a document, preferring
// the w:comments relationship from the document part and falling back to the
// conventional /word/comments.xml. The boolean reports whether the part exists.
func FindCommentsPart(session opc.PackageSession, documentURI string) (string, bool) {
	uri := ""
	for _, rel := range session.ListRelationships(documentURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelComments {
			uri = resolveTargetURI(documentURI, rel.Target)
			break
		}
	}
	if uri == "" {
		uri = CommentsPartURI
	}
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return uri, true
		}
	}
	return uri, false
}

// ListComments reads the comments part (if any) and annotates each comment with the
// 1-based body block index its commentRangeStart falls inside. A document without a
// comments part yields an empty comments slice rather than an error.
func ListComments(session opc.PackageSession, documentURI string) (*DocumentComments, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	result := &DocumentComments{
		DocumentPartURI: documentURI,
		Comments:        make([]Comment, 0),
	}

	commentsURI, exists := FindCommentsPart(session, documentURI)
	if !exists {
		return result, nil
	}
	result.CommentsPart = commentsURI

	commentsDoc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := commentsDoc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsW, "comments") {
		return result, nil
	}

	anchors, err := commentAnchors(session, documentURI)
	if err != nil {
		return nil, err
	}

	// Count w:id occurrences so a non-unique id NEVER advertises a handle that
	// would mis-resolve (the AMBIGUITY surface contract: omit the handle for
	// duplicates, mirroring the PPTX sldId / XLSX sheetId surfaces).
	idCounts := make(map[string]int)
	for _, el := range namespaces.FindChildren(root, namespaces.NsW, "comment") {
		idStr, _ := namespaces.Attr(el, namespaces.NsW, "id")
		idCounts[idStr]++
	}

	for _, el := range namespaces.FindChildren(root, namespaces.NsW, "comment") {
		comment := ReportCommentElement(el)
		idStr, _ := namespaces.Attr(el, namespaces.NsW, "id")
		if idCounts[idStr] > 1 {
			comment.Handle = ""
		}
		if anchor, ok := anchors[idStr]; ok {
			comment.AnchoredToBlock = anchor.index
			comment.AnchoredToBlockKind = anchor.kind
		}
		result.Comments = append(result.Comments, comment)
	}
	return result, nil
}

// ReportCommentElement reads the fields and content hash of one w:comment element.
func ReportCommentElement(el *etree.Element) Comment {
	idStr, _ := namespaces.Attr(el, namespaces.NsW, "id")
	author, _ := namespaces.Attr(el, namespaces.NsW, "author")
	date, _ := namespaces.Attr(el, namespaces.NsW, "date")
	initials, _ := namespaces.Attr(el, namespaces.NsW, "initials")
	text := commentText(el)
	numeric, ok := parseCommentID(idStr)
	comment := Comment{
		ID:          numeric,
		Author:      author,
		Date:        date,
		Initials:    initials,
		Text:        text,
		ContentHash: CommentContentHash(author, date, text),
	}
	if ok {
		comment.PrimarySelector = fmt.Sprintf("%d", numeric)
		comment.Selectors = []string{comment.PrimarySelector}
		comment.Handle = docxhandle.FormatComment(numeric)
	}
	return comment
}

// commentText joins the text of every paragraph in a w:comment, newline-separated.
func commentText(comment *etree.Element) string {
	var out string
	for i, p := range namespaces.FindChildren(comment, namespaces.NsW, "p") {
		if i > 0 {
			out += "\n"
		}
		out += docxbody.ParagraphText(p)
	}
	return out
}

// CommentContentHash hashes the semantic identity of a comment (author, date, text).
func CommentContentHash(author, date, text string) string {
	hash := sha256.New()
	hash.Write([]byte(author))
	hash.Write([]byte{0})
	hash.Write([]byte(date))
	hash.Write([]byte{0})
	hash.Write([]byte(text))
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}

func parseCommentID(id string) (int, bool) {
	if id == "" {
		return 0, false
	}
	n := 0
	for _, r := range id {
		if r < '0' || r > '9' {
			return 0, false
		}
		n = n*10 + int(r-'0')
	}
	return n, true
}

type commentAnchor struct {
	index int
	kind  string
}

// commentAnchors maps a comment id to the body block that contains its
// w:commentRangeStart marker.
func commentAnchors(session opc.PackageSession, documentURI string) (map[string]commentAnchor, error) {
	anchors := make(map[string]commentAnchor)
	doc, err := session.ReadXMLPart(documentURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read document part %s: %w", documentURI, err)
	}
	body, err := docxbody.FindBody(doc.Root())
	if err != nil {
		return nil, err
	}
	for _, block := range docxbody.Blocks(body) {
		for _, start := range namespaces.FindDescendants(block.Element, namespaces.NsW, "commentRangeStart") {
			if id, ok := namespaces.Attr(start, namespaces.NsW, "id"); ok {
				if _, exists := anchors[id]; !exists {
					anchors[id] = commentAnchor{index: block.Index, kind: string(block.Kind)}
				}
			}
		}
	}
	return anchors, nil
}
