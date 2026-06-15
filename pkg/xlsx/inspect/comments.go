package inspect

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"path"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

// Comment is a single worksheet cell comment (legacy note) enriched with its
// anchor cell. XLSX legacy comments anchor purely via the <comment ref="A1">
// attribute; there are no worksheet cell markers (a <commentReference> inside a
// <c> would be schema-invalid).
type Comment struct {
	ID                   int    `json:"id"`
	Author               string `json:"author"`
	Text                 string `json:"text"`
	ContentHash          string `json:"contentHash"`
	AnchoredToCell       string `json:"anchoredToCell"`
	AnchoredToCellRow    int    `json:"anchoredToCellRow,omitempty"`
	AnchoredToCellColumn int    `json:"anchoredToCellColumn,omitempty"`
}

// WorksheetComments is the listing returned by ListComments for one worksheet.
type WorksheetComments struct {
	WorksheetPartURI string    `json:"worksheetPartUri"`
	CommentsPart     string    `json:"commentsPart,omitempty"`
	Comments         []Comment `json:"comments"`
}

// ConventionalCommentsPartURI returns the conventional comments part URI for a
// worksheet, e.g. /xl/worksheets/sheet1.xml -> /xl/comments1.xml. When the
// worksheet number cannot be derived it falls back to /xl/comments1.xml.
func ConventionalCommentsPartURI(worksheetURI string) string {
	base := path.Base(opc.NormalizeURI(worksheetURI))
	base = strings.TrimSuffix(base, path.Ext(base))
	digits := ""
	for i := len(base) - 1; i >= 0; i-- {
		c := base[i]
		if c >= '0' && c <= '9' {
			digits = string(c) + digits
			continue
		}
		break
	}
	if digits == "" {
		digits = "1"
	}
	return "/xl/comments" + digits + ".xml"
}

// FindCommentsPart resolves the comments part for a worksheet, preferring the
// worksheet's comments relationship and falling back to the conventional
// /xl/commentsN.xml. The boolean reports whether the part currently exists.
func FindCommentsPart(session opc.PackageSession, worksheetURI string) (string, bool) {
	uri := ""
	for _, rel := range session.ListRelationships(worksheetURI) {
		if rel.TargetMode == "External" {
			continue
		}
		if rel.Type == namespaces.RelComments {
			uri = resolveTargetURI(worksheetURI, rel.Target)
			break
		}
	}
	if uri == "" {
		uri = ConventionalCommentsPartURI(worksheetURI)
	}
	for _, part := range session.ListParts() {
		if opc.NormalizeURI(part.URI) == uri {
			return uri, true
		}
	}
	return uri, false
}

// ListComments reads the worksheet's comments part (if any) and returns each
// comment with its resolved author name and anchor cell. A worksheet without a
// comments part yields an empty comments slice rather than an error.
func ListComments(session opc.PackageSession, sheet model.SheetRef) (*WorksheetComments, error) {
	if session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	result := &WorksheetComments{
		WorksheetPartURI: sheet.PartURI,
		Comments:         make([]Comment, 0),
	}

	commentsURI, exists := FindCommentsPart(session, sheet.PartURI)
	if !exists {
		return result, nil
	}
	result.CommentsPart = commentsURI

	commentsDoc, err := session.ReadXMLPart(commentsURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read comments part %s: %w", commentsURI, err)
	}
	root := commentsDoc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsSpreadsheetML, "comments") {
		return result, nil
	}

	authors := ParseAuthors(root)
	commentList := namespaces.FindChild(root, namespaces.NsSpreadsheetML, "commentList")
	if commentList == nil {
		return result, nil
	}
	for i, el := range namespaces.FindChildren(commentList, namespaces.NsSpreadsheetML, "comment") {
		comment := ReportCommentElement(el, authors)
		comment.ID = i
		result.Comments = append(result.Comments, comment)
	}
	return result, nil
}

// ParseAuthors returns the ordered list of author names from <authors>.
func ParseAuthors(commentsRoot *etree.Element) []string {
	var authors []string
	list := namespaces.FindChild(commentsRoot, namespaces.NsSpreadsheetML, "authors")
	if list == nil {
		return authors
	}
	for _, a := range namespaces.FindChildren(list, namespaces.NsSpreadsheetML, "author") {
		authors = append(authors, a.Text())
	}
	return authors
}

// ReportCommentElement reads one <comment> element. authors resolves authorId.
func ReportCommentElement(el *etree.Element, authors []string) Comment {
	ref := el.SelectAttrValue("ref", "")
	authorID := el.SelectAttrValue("authorId", "")
	author := ""
	if idx := atoiSafe(authorID); idx >= 0 && idx < len(authors) {
		author = authors[idx]
	}
	text := commentText(el)
	comment := Comment{
		Author:         author,
		Text:           text,
		ContentHash:    CommentContentHash(author, text),
		AnchoredToCell: ref,
	}
	if col, row, ok := splitCellRef(ref); ok {
		comment.AnchoredToCellColumn = col
		comment.AnchoredToCellRow = row
	}
	return comment
}

// commentText joins the text of the comment's <text> rich-string runs.
func commentText(comment *etree.Element) string {
	textElem := namespaces.FindChild(comment, namespaces.NsSpreadsheetML, "text")
	if textElem == nil {
		return ""
	}
	var b strings.Builder
	// Plain <text><t>..</t></text> form.
	for _, t := range namespaces.FindChildren(textElem, namespaces.NsSpreadsheetML, "t") {
		b.WriteString(t.Text())
	}
	// Rich <text><r><t>..</t></r></text> form.
	for _, r := range namespaces.FindChildren(textElem, namespaces.NsSpreadsheetML, "r") {
		for _, t := range namespaces.FindChildren(r, namespaces.NsSpreadsheetML, "t") {
			b.WriteString(t.Text())
		}
	}
	return b.String()
}

// CommentContentHash hashes the semantic identity of a comment. Legacy
// SpreadsheetML notes (CT_Comment) carry only author and text; date and
// initials have no conformant home and are not part of the identity.
func CommentContentHash(author, text string) string {
	hash := sha256.New()
	hash.Write([]byte(author))
	hash.Write([]byte{0})
	hash.Write([]byte(text))
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}

func atoiSafe(s string) int {
	if s == "" {
		return -1
	}
	n := 0
	for _, r := range s {
		if r < '0' || r > '9' {
			return -1
		}
		n = n*10 + int(r-'0')
	}
	return n
}

// splitCellRef returns the 1-based column and row of an A1 cell reference.
func splitCellRef(ref string) (col, row int, ok bool) {
	ref = strings.TrimSpace(ref)
	if ref == "" {
		return 0, 0, false
	}
	i := 0
	for i < len(ref) {
		c := ref[i]
		if (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') {
			cc := c
			if cc >= 'a' {
				cc -= 'a' - 'A'
			}
			col = col*26 + int(cc-'A'+1)
			i++
			continue
		}
		break
	}
	if i == 0 || i == len(ref) {
		return 0, 0, false
	}
	for ; i < len(ref); i++ {
		c := ref[i]
		if c < '0' || c > '9' {
			return 0, 0, false
		}
		row = row*10 + int(c-'0')
	}
	if col == 0 || row == 0 {
		return 0, 0, false
	}
	return col, row, true
}
