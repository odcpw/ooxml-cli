// Package handle implements the PR-HANDLES-1 stable object handle codec for
// DOCX packages in ooxml-cli. It is the DOCX analog of pkg/pptx/handle and
// pkg/xlsx/handle: a handle is an additive, opaque-but-decodable string envelope
// that addresses a mutable OOXML object by its CONTAINER SCOPE plus a native id
// (or, for paragraphs which have no native id, an injected durable MARKER),
// rather than by a positional ordinal that shifts when blocks are inserted or
// deleted.
//
// Phase 3 covers DOCX comments, styles, and paragraphs. The handle is
// recognized as the FIRST branch in each resolver and falls through to every
// legacy selector unchanged when the input is not a handle (see IsHandle):
// body.b<n>, block:<n>, --comment-id, --style-id all keep working byte-for-byte.
//
// Envelope grammar (mirrors the PPTX and XLSX codecs):
//
//		H:<format>/<scope-path>/<class>:<objref>
//
//	  - "H:"            version prefix. Any string not starting with this prefix
//	                    is NOT a handle and is left untouched for legacy parsing.
//	  - <format>        the package format tag: "docx".
//	  - <scope-path>    the container identity. DOCX ids are per-part, so the
//	                    scope names the part:
//	                      "pt:doc"    = the main document part (word/document.xml).
//	                      "pt:styles" = the (document-global) styles part. Styles
//	                                    are global to the document, but the scope
//	                                    token keeps the grammar uniform.
//	  - <class>:<objref> the addressed object. <class> is one of
//	                    {comment, style, para}. <objref> is tagged so the grammar
//	                    itself encodes the stability class:
//	                      n:<value>  = a NATIVE id (comment w:id; style w:styleId).
//	                      m:<value>  = an injected/native MARKER (a paragraph's
//	                                   w14:paraId). It is tagged "m" rather than
//	                                   "n" because the tool may have INJECTED it on
//	                                   first mutate (the w:p has no native id slot);
//	                                   once present it is a real attribute that
//	                                   survives both structural and content edits,
//	                                   so resolution SEARCHES for it. When a real
//	                                   Word file already carries a w14:paraId the
//	                                   tool reuses it read-only.
//
// The "n:"/"m:" tag is the stability contract made visible: a native attribute
// id (or an injected paraId marker) survives BOTH unrelated structural edits
// (insert/delete/reorder of sibling blocks, because resolution SEARCHES for the
// id rather than recomputing a position) AND a content edit of the target (the
// id is an attribute, not the text — this is what makes the TRANSLATION
// round-trip work: editing a paragraph's text leaves its paraId untouched).
//
// Examples:
//
//	H:docx/pt:doc/comment:n:3        -> comment whose w:id == 3
//	H:docx/pt:styles/style:n:Heading1 -> style whose w:styleId == "Heading1"
//	H:docx/pt:doc/para:m:1C9E4F2A    -> paragraph whose w14:paraId == "1C9E4F2A"
package handle

import (
	"fmt"
	"strconv"
	"strings"
)

// VersionPrefix is the envelope version marker. A string that does not start
// with this prefix is not a handle. It is identical to the PPTX/XLSX prefix; the
// <format> tag is what routes to the correct resolver back-end.
const VersionPrefix = "H:"

// FormatDOCX is the format tag for DOCX handles.
const FormatDOCX = "docx"

// Class tags for the addressed object kind.
const (
	ClassComment   = "comment"
	ClassStyle     = "style"
	ClassParagraph = "para"
)

// objref tags. nativeTag marks a native OOXML id (comment w:id, style
// w:styleId); markerTag marks a paragraph marker (w14:paraId), deliberately
// distinct from nativeTag so the grammar discloses that the marker may have been
// injected by the tool rather than authored natively.
const (
	nativeTag = "n"
	markerTag = "m"
)

// Scope tokens. DOCX ids are per-part, so the scope names the part.
const (
	// scopeDocument is the scope token for the main document part.
	scopeDocument = "pt:doc"
	// scopeStyles is the scope token for the document-global styles part.
	scopeStyles = "pt:styles"
)

// Kind discriminates the addressed object class.
type Kind int

const (
	// KindComment addresses a comment by its native w:id within the document part.
	KindComment Kind = iota
	// KindStyle addresses a style by its native w:styleId (document-global).
	KindStyle
	// KindParagraph addresses a paragraph by its w14:paraId marker within the
	// document part. The marker is read opportunistically when present and
	// injected on the first mutate that targets a marker-less paragraph.
	KindParagraph
)

// Handle is a decoded DOCX handle envelope.
//
// CommentID is kept as an int (comment w:id values are integers, like the legacy
// --comment-id flag). StyleID and ParaID are strings carried verbatim so they
// round-trip byte-for-byte against the OOXML attribute values and never drift.
type Handle struct {
	// Format is the package format tag (always FormatDOCX).
	Format string
	// Kind is the addressed object class.
	Kind Kind
	// CommentID is the native w:id of a comment (KindComment).
	CommentID int
	// StyleID is the native w:styleId of a style (KindStyle).
	StyleID string
	// ParaID is the w14:paraId marker of a paragraph (KindParagraph).
	ParaID string
}

// Error codes for handle resolution failures. These mirror the PPTX and XLSX
// typed error contract VERBATIM so agents see one stable vocabulary across all
// three formats.
const (
	// CodeMalformed: the string starts with the handle prefix but is not a valid
	// handle (wrong version, bad shape, unknown class, empty id).
	CodeMalformed = "HANDLE_MALFORMED"
	// CodeScopeStale: the handle is valid but its scope container (the document or
	// styles part) no longer exists in the package.
	CodeScopeStale = "HANDLE_SCOPE_STALE"
	// CodeStale: the scope exists but the addressed object (comment id, style id,
	// paragraph marker) no longer exists within it.
	CodeStale = "HANDLE_STALE"
	// CodeFormatMismatch: the handle's format tag does not match the package.
	CodeFormatMismatch = "HANDLE_FORMAT_MISMATCH"
	// CodeAmbiguous: the handle's id/marker is NOT unique within its scope (a
	// duplicate comment w:id, a duplicate w:styleId, or a duplicate w14:paraId
	// produced by copy/paste in the host application). Such an id cannot be
	// resolved to a single object without guessing, so resolution refuses rather
	// than silently picking one and mis-targeting. This is the contract that
	// prevents silent wrong-target data corruption.
	CodeAmbiguous = "HANDLE_AMBIGUOUS"
)

// Error is a typed handle error carrying a stable Code, the offending handle
// string, and a human message. It mirrors pkg/pptx/handle.Error and
// pkg/xlsx/handle.Error.
type Error struct {
	Code    string
	Handle  string
	Message string
}

func (e *Error) Error() string {
	if e.Handle != "" {
		return fmt.Sprintf("%s: %s (handle %q)", e.Code, e.Message, e.Handle)
	}
	return fmt.Sprintf("%s: %s", e.Code, e.Message)
}

// IsCode reports whether err is a handle Error with the given code.
func IsCode(err error, code string) bool {
	he, ok := err.(*Error)
	return ok && he.Code == code
}

// IsHandle reports whether s is a handle envelope (starts with the version
// prefix). It performs no validation beyond the prefix so callers can use it as
// the first-branch discriminator: a true result means "route to the handle
// resolver"; a false result means "this is a legacy selector, leave it
// untouched". A string with the prefix but otherwise malformed is reported as a
// handle here and then rejected by Parse with CodeMalformed.
func IsHandle(s string) bool {
	return strings.HasPrefix(strings.TrimSpace(s), VersionPrefix)
}

// FormatComment renders a comment handle from a native w:id.
func FormatComment(commentID int) string {
	return Format(Handle{Format: FormatDOCX, Kind: KindComment, CommentID: commentID})
}

// FormatStyle renders a style handle from a native w:styleId.
func FormatStyle(styleID string) string {
	return Format(Handle{Format: FormatDOCX, Kind: KindStyle, StyleID: styleID})
}

// FormatParagraph renders a paragraph handle from a w14:paraId marker.
func FormatParagraph(paraID string) string {
	return Format(Handle{Format: FormatDOCX, Kind: KindParagraph, ParaID: paraID})
}

// Format renders a Handle to its envelope string. It is the inverse of Parse
// for well-formed handles and must be byte-identical on round-trip.
func Format(h Handle) string {
	format := h.Format
	if format == "" {
		format = FormatDOCX
	}
	switch h.Kind {
	case KindComment:
		return fmt.Sprintf("%s%s/%s/%s:%s:%d", VersionPrefix, format, scopeDocument, ClassComment, nativeTag, h.CommentID)
	case KindStyle:
		return fmt.Sprintf("%s%s/%s/%s:%s:%s", VersionPrefix, format, scopeStyles, ClassStyle, nativeTag, h.StyleID)
	case KindParagraph:
		return fmt.Sprintf("%s%s/%s/%s:%s:%s", VersionPrefix, format, scopeDocument, ClassParagraph, markerTag, h.ParaID)
	default:
		return fmt.Sprintf("%s%s/%s", VersionPrefix, format, scopeDocument)
	}
}

// Parse decodes a handle envelope string. The caller is expected to have already
// gated on IsHandle; Parse re-checks the prefix and returns a CodeMalformed
// Error for any string that does not decode to a valid handle.
func Parse(s string) (Handle, error) {
	trimmed := strings.TrimSpace(s)
	if !strings.HasPrefix(trimmed, VersionPrefix) {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "missing handle version prefix " + strconv.Quote(VersionPrefix)}
	}

	body := strings.TrimPrefix(trimmed, VersionPrefix)
	segments := strings.Split(body, "/")
	// scope tokens contain a "/" themselves ("pt:doc" does not, but the grammar
	// is format/scope/object, three slash-separated segments after the prefix).
	if len(segments) != 3 {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "handle must be H:docx/<scope>/<class>:<objref>"}
	}

	format := segments[0]
	if format == "" {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "empty format tag"}
	}
	if format != FormatDOCX {
		return Handle{}, &Error{Code: CodeFormatMismatch, Handle: s, Message: fmt.Sprintf("handle format tag %q does not match package format %q", format, FormatDOCX)}
	}

	scope := segments[1]
	class, objref, ok := strings.Cut(segments[2], ":")
	if !ok {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("object segment %q must be <class>:<objref>", segments[2])}
	}

	switch class {
	case ClassComment:
		if scope != scopeDocument {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("comment handle scope must be %q, got %q", scopeDocument, scope)}
		}
		id, err := parseNativeInt(objref)
		if err != nil {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "comment objref: " + err.Error()}
		}
		return Handle{Format: format, Kind: KindComment, CommentID: id}, nil
	case ClassStyle:
		if scope != scopeStyles {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("style handle scope must be %q, got %q", scopeStyles, scope)}
		}
		styleID, err := parseNativeRef(objref)
		if err != nil {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "style objref: " + err.Error()}
		}
		return Handle{Format: format, Kind: KindStyle, StyleID: styleID}, nil
	case ClassParagraph:
		if scope != scopeDocument {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("paragraph handle scope must be %q, got %q", scopeDocument, scope)}
		}
		paraID, err := parseMarkerRef(objref)
		if err != nil {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "paragraph objref: " + err.Error()}
		}
		return Handle{Format: format, Kind: KindParagraph, ParaID: paraID}, nil
	default:
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("unknown object class %q (supported: %q, %q, %q)", class, ClassComment, ClassStyle, ClassParagraph)}
	}
}

// parseNativeInt decodes an objref of the form "n:<int>" into an int.
func parseNativeInt(objref string) (int, error) {
	tag, value, ok := strings.Cut(objref, ":")
	if !ok {
		return 0, fmt.Errorf("objref %q must be %s:<id>", objref, nativeTag)
	}
	if tag != nativeTag {
		return 0, fmt.Errorf("unsupported objref tag %q (expected native id %q)", tag, nativeTag)
	}
	if value == "" {
		return 0, fmt.Errorf("empty native id")
	}
	id, err := strconv.Atoi(value)
	if err != nil {
		return 0, fmt.Errorf("invalid native id %q: %w", value, err)
	}
	if id < 0 {
		return 0, fmt.Errorf("native id must be non-negative, got %d", id)
	}
	return id, nil
}

// parseNativeRef decodes an objref of the form "n:<value>" into its value
// verbatim (only the leading "n:" tag is stripped).
func parseNativeRef(objref string) (string, error) {
	tag, value, ok := strings.Cut(objref, ":")
	if !ok {
		return "", fmt.Errorf("objref %q must be %s:<value>", objref, nativeTag)
	}
	if tag != nativeTag {
		return "", fmt.Errorf("unsupported objref tag %q (expected native id %q)", tag, nativeTag)
	}
	if value == "" {
		return "", fmt.Errorf("empty native id")
	}
	return value, nil
}

// parseMarkerRef decodes an objref of the form "m:<value>" into the marker
// value (a w14:paraId). The "m" tag marks an injected/native paragraph marker
// (see the package doc).
func parseMarkerRef(objref string) (string, error) {
	tag, value, ok := strings.Cut(objref, ":")
	if !ok {
		return "", fmt.Errorf("objref %q must be %s:<paraId>", objref, markerTag)
	}
	if tag != markerTag {
		return "", fmt.Errorf("unsupported objref tag %q (expected paragraph marker %q)", tag, markerTag)
	}
	if value == "" {
		return "", fmt.Errorf("empty paragraph marker")
	}
	return value, nil
}
