// Package handle implements the PR-HANDLES-1 stable object handle codec for
// ooxml-cli. A handle is an additive, opaque-but-decodable string envelope that
// addresses a mutable OOXML object by its CONTAINER SCOPE plus a native id,
// rather than by a positional ordinal.
//
// Phase 1 covers PPTX shapes and slides. The handle is recognized as the FIRST
// branch in each resolver and falls through to every legacy selector unchanged
// when the input is not a handle (see IsHandle).
//
// Envelope grammar (version 1):
//
//		H:<format>/<scope-path>/<class>:<objref>
//
//	  - "H:"            version prefix. Any string not starting with this prefix
//	                    is NOT a handle and is left untouched for legacy parsing.
//	  - <format>        the package format tag: "pptx" (xlsx/docx land in later
//	                    phases). Picks the resolver back-end.
//	  - <scope-path>    the container identity, itself native-id-first:
//	                    "s:<sldId>" = the p:sldId@id of the owning slide. A bare
//	                    slide handle has no further path; a shape handle adds the
//	                    class+objref segment.
//	  - <class>:<objref> the addressed object. <class> is one of {slide, shape}.
//	                    <objref> is "n:<value>" for a NATIVE id (the only kind
//	                    used in phase 1): a shape cNvPr@id, e.g. "n:5".
//
// The "n:" tag is the stability contract made visible: a native attribute id
// survives BOTH unrelated structural edits (insert/delete/reorder of siblings,
// because resolution SEARCHES for the id rather than recomputing a position)
// AND a content edit of the target (the id is an attribute, not the text).
//
// Examples:
//
//	H:pptx/s:256              -> slide whose p:sldId@id == 256
//	H:pptx/s:256/shape:n:5    -> shape cNvPr@id==5 on slide sldId 256
//
// Scope is mandatory for shapes because cNvPr@id is unique only per slide, not
// per deck.
package handle

import (
	"fmt"
	"strconv"
	"strings"
)

// VersionPrefix is the envelope version marker. A string that does not start
// with this prefix is not a handle.
const VersionPrefix = "H:"

// FormatPPTX is the only format tag wired in phase 1.
const FormatPPTX = "pptx"

// Class tags for the addressed object kind.
const (
	ClassSlide   = "slide"
	ClassShape   = "shape"
	ClassComment = "comment"
)

// nativeTag marks an objref value as a native OOXML id (the only objref kind in
// phase 1).
const nativeTag = "n"

// Kind discriminates the addressed object class.
type Kind int

const (
	// KindSlide addresses a slide by its native p:sldId@id (scope only).
	KindSlide Kind = iota
	// KindShape addresses a shape by its native cNvPr@id within a slide scope.
	KindShape
	// KindComment addresses a legacy slide comment by its idx plus authorId
	// within a slide scope. Both values are required because some Office decks
	// allocate idx per author.
	KindComment
)

// Handle is a decoded handle envelope. In phase 1 the scope is always a slide
// identified by its native p:sldId@id (SlideID), and the optional object is a
// shape identified by its native cNvPr@id (ShapeID).
type Handle struct {
	// Format is the package format tag (always FormatPPTX in phase 1).
	Format string
	// Kind is the addressed object class.
	Kind Kind
	// SlideID is the native p:sldId@id of the scope slide.
	SlideID uint32
	// ShapeID is the native cNvPr@id of the shape (only set when Kind==KindShape).
	ShapeID int
	// CommentID is the legacy p:cm@idx value (only set when Kind==KindComment).
	CommentID int
	// AuthorID is the legacy p:cm@authorId value (only set when Kind==KindComment).
	AuthorID int
}

// Error codes for handle resolution failures. These form the typed error
// contract: callers can distinguish a MALFORMED handle (bad envelope) from a
// STALE one (the addressed object no longer exists) and never silently fall
// back to a positional guess on a stale handle.
const (
	// CodeMalformed: the string starts with the handle prefix but is not a
	// valid handle (wrong version, bad shape, unknown class, non-numeric id).
	CodeMalformed = "HANDLE_MALFORMED"
	// CodeScopeStale: the handle is valid but its scope container (slide sldId)
	// no longer exists in the package.
	CodeScopeStale = "HANDLE_SCOPE_STALE"
	// CodeStale: the scope exists but the addressed object (shape cNvPr@id) no
	// longer exists within it.
	CodeStale = "HANDLE_STALE"
	// CodeFormatMismatch: the handle's format tag does not match the package.
	CodeFormatMismatch = "HANDLE_FORMAT_MISMATCH"
	// CodeAmbiguous: the handle's native id is NOT unique within its scope (a
	// duplicate cNvPr@id within a slide, or a duplicate p:sldId@id within the
	// presentation). Such an id cannot be resolved to a single object without
	// guessing, so resolution refuses rather than silently picking one and
	// mis-targeting. Decks that are programmatically generated or merged can
	// carry duplicate ids, so this is the contract that prevents silent
	// wrong-target data corruption.
	CodeAmbiguous = "HANDLE_AMBIGUOUS"
)

// Error is a typed handle error carrying a stable Code, the offending handle
// string, and a human message.
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
// prefix). It performs no validation beyond the prefix so that callers can use
// it as the first-branch discriminator: a true result means "route to the
// handle resolver"; a false result means "this is a legacy selector, leave it
// untouched". A string that has the prefix but is otherwise malformed will be
// reported as a handle here and then rejected by Parse with CodeMalformed,
// rather than being silently treated as a legacy selector.
func IsHandle(s string) bool {
	return strings.HasPrefix(strings.TrimSpace(s), VersionPrefix)
}

// FormatSlide renders a slide handle from a native sldId.
func FormatSlide(slideID uint32) string {
	return Format(Handle{Format: FormatPPTX, Kind: KindSlide, SlideID: slideID})
}

// FormatShape renders a shape handle from a slide sldId scope and a shape
// cNvPr@id.
func FormatShape(slideID uint32, shapeID int) string {
	return Format(Handle{Format: FormatPPTX, Kind: KindShape, SlideID: slideID, ShapeID: shapeID})
}

// FormatComment renders a comment handle from a slide sldId scope, comment idx,
// and authorId. idx alone is not unique in some legacy comment parts.
func FormatComment(slideID uint32, commentID, authorID int) string {
	return Format(Handle{Format: FormatPPTX, Kind: KindComment, SlideID: slideID, CommentID: commentID, AuthorID: authorID})
}

// Format renders a Handle to its envelope string. It is the inverse of Parse
// for well-formed handles.
func Format(h Handle) string {
	format := h.Format
	if format == "" {
		format = FormatPPTX
	}
	scope := fmt.Sprintf("s:%d", h.SlideID)
	switch h.Kind {
	case KindSlide:
		return fmt.Sprintf("%s%s/%s", VersionPrefix, format, scope)
	case KindShape:
		return fmt.Sprintf("%s%s/%s/%s:%s:%d", VersionPrefix, format, scope, ClassShape, nativeTag, h.ShapeID)
	case KindComment:
		return fmt.Sprintf("%s%s/%s/%s:idx:%d:authorId:%d", VersionPrefix, format, scope, ClassComment, h.CommentID, h.AuthorID)
	default:
		return fmt.Sprintf("%s%s/%s", VersionPrefix, format, scope)
	}
}

// Parse decodes a handle envelope string. The caller is expected to have
// already gated on IsHandle; Parse re-checks the prefix and returns a
// CodeMalformed Error for any string that does not decode to a valid handle.
func Parse(s string) (Handle, error) {
	trimmed := strings.TrimSpace(s)
	if !strings.HasPrefix(trimmed, VersionPrefix) {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "missing handle version prefix " + strconv.Quote(VersionPrefix)}
	}

	body := strings.TrimPrefix(trimmed, VersionPrefix)
	segments := strings.Split(body, "/")
	if len(segments) < 2 {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "handle must have at least a format and scope segment"}
	}

	format := segments[0]
	if format == "" {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "empty format tag"}
	}
	if format != FormatPPTX {
		return Handle{}, &Error{Code: CodeFormatMismatch, Handle: s, Message: fmt.Sprintf("handle format tag %q does not match package format %q", format, FormatPPTX)}
	}

	scope := segments[1]
	slideID, err := parseScope(scope)
	if err != nil {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: err.Error()}
	}

	h := Handle{Format: format, SlideID: slideID}

	switch len(segments) {
	case 2:
		// Slide-only handle.
		h.Kind = KindSlide
		return h, nil
	case 3:
		class, objref, ok := strings.Cut(segments[2], ":")
		if !ok {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("object segment %q must be <class>:<objref>", segments[2])}
		}
		switch class {
		case ClassShape:
			shapeID, err := parseNativeInt(objref)
			if err != nil {
				return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "shape objref: " + err.Error()}
			}
			h.Kind = KindShape
			h.ShapeID = shapeID
			return h, nil
		case ClassComment:
			commentID, authorID, err := parseCommentRef(objref)
			if err != nil {
				return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "comment objref: " + err.Error()}
			}
			h.Kind = KindComment
			h.CommentID = commentID
			h.AuthorID = authorID
			return h, nil
		default:
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("unknown object class %q (supported: %q, %q)", class, ClassShape, ClassComment)}
		}
	default:
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "handle has too many segments"}
	}
}

func parseCommentRef(objref string) (int, int, error) {
	parts := strings.Split(objref, ":")
	if len(parts) != 4 || parts[0] != "idx" || parts[2] != "authorId" {
		return 0, 0, fmt.Errorf("must be idx:<comment-id>:authorId:<author-id>")
	}
	commentID, err := parseNonNegativeInt(parts[1], "comment id")
	if err != nil {
		return 0, 0, err
	}
	authorID, err := parseNonNegativeInt(parts[3], "author id")
	if err != nil {
		return 0, 0, err
	}
	return commentID, authorID, nil
}

func parseNonNegativeInt(value, label string) (int, error) {
	if value == "" {
		return 0, fmt.Errorf("empty %s", label)
	}
	id, err := strconv.Atoi(value)
	if err != nil {
		return 0, fmt.Errorf("invalid %s %q: %w", label, value, err)
	}
	if id < 0 {
		return 0, fmt.Errorf("%s must be non-negative, got %d", label, id)
	}
	return id, nil
}

// parseScope decodes a scope segment "s:<sldId>" into a slide id.
func parseScope(scope string) (uint32, error) {
	kind, value, ok := strings.Cut(scope, ":")
	if !ok {
		return 0, fmt.Errorf("scope %q must be s:<sldId>", scope)
	}
	if kind != "s" {
		return 0, fmt.Errorf("unsupported scope kind %q (phase 1 supports only slide scope \"s\")", kind)
	}
	if value == "" {
		return 0, fmt.Errorf("empty slide id in scope")
	}
	id, err := strconv.ParseUint(value, 10, 32)
	if err != nil {
		return 0, fmt.Errorf("invalid slide id %q: %w", value, err)
	}
	return uint32(id), nil
}

// parseNativeInt decodes an objref of the form "n:<int>" into an int.
func parseNativeInt(objref string) (int, error) {
	tag, value, ok := strings.Cut(objref, ":")
	if !ok {
		return 0, fmt.Errorf("objref %q must be %s:<id>", objref, nativeTag)
	}
	if tag != nativeTag {
		return 0, fmt.Errorf("unsupported objref tag %q (phase 1 supports only native ids %q)", tag, nativeTag)
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
