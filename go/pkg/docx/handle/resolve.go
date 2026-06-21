package handle

import (
	"fmt"
	"strconv"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

// ResolveComment resolves a comment handle to the single w:comment element whose
// native w:id matches, searching the comments part root. Resolution is a SEARCH,
// not a positional recount, so the handle survives insertion/deletion of other
// comments and edits of the comment's own text (the id is an attribute).
//
// A w:id absent from the part yields CodeStale; a w:id shared by more than one
// w:comment (a malformed/merged comments part) yields CodeAmbiguous rather than
// silently picking one.
func ResolveComment(commentsRoot *etree.Element, h Handle) (*etree.Element, error) {
	if h.Kind != KindComment {
		return nil, &Error{Code: CodeMalformed, Handle: Format(h), Message: "expected a comment handle"}
	}
	if commentsRoot == nil {
		return nil, &Error{Code: CodeScopeStale, Handle: Format(h), Message: "document has no comments part"}
	}
	idStr := strconv.Itoa(h.CommentID)
	var matches []*etree.Element
	for _, comment := range namespaces.FindChildren(commentsRoot, namespaces.NsW, "comment") {
		if v, ok := namespaces.Attr(comment, namespaces.NsW, "id"); ok && v == idStr {
			matches = append(matches, comment)
		}
	}
	switch len(matches) {
	case 0:
		return nil, &Error{Code: CodeStale, Handle: Format(h), Message: fmt.Sprintf("no comment with w:id %d in document", h.CommentID)}
	case 1:
		return matches[0], nil
	default:
		return nil, &Error{Code: CodeAmbiguous, Handle: Format(h), Message: fmt.Sprintf("w:id %d is not unique (%d comments share it); cannot resolve to a single comment", h.CommentID, len(matches))}
	}
}

// ResolveStyle resolves a style handle to the single w:style element whose
// native w:styleId matches, searching the styles part root. styleId is a native,
// document-global, position-independent id, so the handle is stable across body
// edits.
//
// A styleId absent from the part yields CodeStale; a styleId shared by more than
// one w:style (a malformed styles part) yields CodeAmbiguous.
func ResolveStyle(stylesRoot *etree.Element, h Handle) (*etree.Element, error) {
	if h.Kind != KindStyle {
		return nil, &Error{Code: CodeMalformed, Handle: Format(h), Message: "expected a style handle"}
	}
	if stylesRoot == nil {
		return nil, &Error{Code: CodeScopeStale, Handle: Format(h), Message: "document has no styles part"}
	}
	var matches []*etree.Element
	for _, style := range namespaces.FindChildren(stylesRoot, namespaces.NsW, "style") {
		if v, ok := namespaces.Attr(style, namespaces.NsW, "styleId"); ok && v == h.StyleID {
			matches = append(matches, style)
		}
	}
	switch len(matches) {
	case 0:
		return nil, &Error{Code: CodeStale, Handle: Format(h), Message: fmt.Sprintf("no style with w:styleId %q in document", h.StyleID)}
	case 1:
		return matches[0], nil
	default:
		return nil, &Error{Code: CodeAmbiguous, Handle: Format(h), Message: fmt.Sprintf("w:styleId %q is not unique (%d styles share it)", h.StyleID, len(matches))}
	}
}

// ResolveParagraphBlock resolves a paragraph handle to the 1-based body BLOCK
// index of the w:p whose w14:paraId marker matches, by SEARCHING the body. The
// returned block index can be fed straight into the existing positional mutate
// commands (SetParagraphText, ReplaceBlockWithParagraph, etc.) so the handle is
// authoritative for WHICH paragraph is targeted while the mutation machinery is
// reused unchanged.
//
// Because resolution searches for the marker attribute rather than recomputing a
// position, the handle survives:
//   - STRUCTURAL edits: inserting/deleting/reordering other blocks shifts the
//     index, and ResolveParagraphBlock recomputes the CURRENT index of the SAME
//     marked paragraph.
//   - CONTENT edits (the translation case): rewriting the paragraph's text leaves
//     its w14:paraId attribute untouched, so the same handle still resolves.
//
// A marker absent from the body yields CodeStale; a marker shared by more than
// one paragraph (e.g. a copy/paste duplicate in the host application) yields
// CodeAmbiguous — the handle is NEVER resolved positionally on a duplicate.
func ResolveParagraphBlock(body *etree.Element, h Handle) (blockIndex int, element *etree.Element, err error) {
	if h.Kind != KindParagraph {
		return 0, nil, &Error{Code: CodeMalformed, Handle: Format(h), Message: "expected a paragraph handle"}
	}
	if body == nil {
		return 0, nil, &Error{Code: CodeScopeStale, Handle: Format(h), Message: "document has no body"}
	}
	want := strings.ToUpper(strings.TrimSpace(h.ParaID))
	type match struct {
		index int
		elem  *etree.Element
	}
	var matches []match
	for _, block := range docxbody.Blocks(body) {
		if block.Kind != "paragraph" {
			continue
		}
		if id := ReadParaID(block.Element); id != "" && strings.ToUpper(id) == want {
			matches = append(matches, match{index: block.Index, elem: block.Element})
		}
	}
	switch len(matches) {
	case 0:
		return 0, nil, &Error{Code: CodeStale, Handle: Format(h), Message: fmt.Sprintf("no paragraph with w14:paraId %q in document body", h.ParaID)}
	case 1:
		return matches[0].index, matches[0].elem, nil
	default:
		return 0, nil, &Error{Code: CodeAmbiguous, Handle: Format(h), Message: fmt.Sprintf("w14:paraId %q is not unique (%d paragraphs share it); cannot resolve to a single paragraph", h.ParaID, len(matches))}
	}
}
