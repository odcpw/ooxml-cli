// Package handle implements the PR-HANDLES-1 stable object handle codec for
// XLSX packages in ooxml-cli. It is the XLSX analog of pkg/pptx/handle: a
// handle is an additive, opaque-but-decodable string envelope that addresses a
// mutable OOXML object by its CONTAINER SCOPE plus a native id (or, where no
// native per-object id exists, an honestly-tagged positional locator), rather
// than by a positional ordinal that shifts when sheets are reordered.
//
// Phase 2 covers XLSX sheets, cells, comments, and defined names. The handle is
// recognized as the FIRST branch in each resolver and falls through to every
// legacy selector unchanged when the input is not a handle (see IsHandle):
// sheet:N, sheetId:, name:, ~name, A1 refs all keep working byte-for-byte.
//
// Envelope grammar (mirrors the PPTX codec):
//
//		H:<format>/<scope-path>/<class>:<objref>
//
//	  - "H:"            version prefix. Any string not starting with this prefix
//	                    is NOT a handle and is left untouched for legacy parsing.
//	  - <format>        the package format tag: "xlsx".
//	  - <scope-path>    the container identity, native-id-first:
//	                    "ws:<sheetId>" = the <sheet sheetId=> attribute of the
//	                    owning worksheet. This is the XLSX analog of the PPTX
//	                    slide sldId scope. A bare sheet handle has no further
//	                    path; cell/comment handles add the class+objref segment.
//	                    A workbook-scoped defined-name handle uses "wb" as the
//	                    scope (defined names live at workbook scope, not on a
//	                    single sheet's sheetId).
//	  - <class>:<objref> the addressed object. <class> is one of
//	                    {sheet, cell, comment, name}. <objref> is tagged so the
//	                    grammar itself encodes the stability class:
//	                      n:<value>  = a NATIVE id (sheet sheetId; defined name).
//	                      a:<A1ref>  = an A1 grid coordinate. Tagged "a", NOT
//	                                   "n", because an A1 ref is NOT a native
//	                                   per-cell id: it survives sheet reorder and
//	                                   rename, but it does NOT survive row/column
//	                                   insert/delete (the address shifts). The
//	                                   tag makes that honesty visible in the
//	                                   handle string. See the package doc on the
//	                                   cell-vs-row-insert limitation.
//
// Stability contract made visible by the tag:
//   - "ws:<sheetId>" (scope) and "n:" (defined name) survive sheet reorder,
//     sheet rename, and unrelated structural edits (inserting/deleting OTHER
//     sheets), because resolution SEARCHES for the id rather than recomputing a
//     position. DELETING the addressed sheet does NOT survive: the handle goes
//     stale (CodeScopeStale), and because new sheets get RANDOM sheetIds the
//     freed id is never reused, so a stale handle can never silently re-point at
//     a later-added sheet.
//   - "a:<A1>" (a cell address) survives sheet reorder/rename, but is honestly
//     positional within the grid: a row or column insert that shifts the
//     address is NOT survived. A content fingerprint is NEVER used as the
//     address (editing a cell value must not change its handle).
//
// Examples:
//
//	H:xlsx/ws:2                  -> sheet whose <sheet sheetId="2">
//	H:xlsx/ws:2/cell:a:B7        -> cell B7 on the sheet with sheetId 2
//	H:xlsx/ws:2/comment:a:B7     -> the comment anchored at B7 on sheetId 2
//	H:xlsx/wb/name:n:SalesTotal  -> the workbook-scoped defined name SalesTotal
package handle

import (
	"fmt"
	"strconv"
	"strings"
)

// VersionPrefix is the envelope version marker. A string that does not start
// with this prefix is not a handle. It is identical to the PPTX prefix; the
// <format> tag is what routes to the correct resolver back-end.
const VersionPrefix = "H:"

// FormatXLSX is the format tag for XLSX handles.
const FormatXLSX = "xlsx"

// Class tags for the addressed object kind.
const (
	ClassSheet   = "sheet"
	ClassCell    = "cell"
	ClassComment = "comment"
	ClassName    = "name"
)

// objref tags. nativeTag marks a native OOXML id (sheetId, defined name);
// addrTag marks an A1 grid coordinate, deliberately distinct from nativeTag so
// the grammar discloses that the address is positional within the grid.
const (
	nativeTag = "n"
	addrTag   = "a"
)

// scopeWorkbook is the scope token for workbook-scoped objects (defined names).
const scopeWorkbook = "wb"

// scopeSheetKind is the scope-path kind prefix for a sheet-scoped handle:
// "ws:<sheetId>".
const scopeSheetKind = "ws"

// Kind discriminates the addressed object class.
type Kind int

const (
	// KindSheet addresses a worksheet by its native <sheet sheetId=> (scope only).
	KindSheet Kind = iota
	// KindCell addresses a cell by its A1 ref within a sheet (sheetId) scope.
	KindCell
	// KindComment addresses a comment by its anchor A1 cell within a sheet scope.
	KindComment
	// KindDefinedName addresses a workbook-scoped defined name by its native name.
	KindDefinedName
)

// Handle is a decoded XLSX handle envelope.
//
// SheetID is kept as a STRING (not parsed to an integer) so it round-trips
// byte-for-byte against the SheetRef.SheetID attribute value and never drifts
// via leading-zero or radix normalization.
type Handle struct {
	// Format is the package format tag (always FormatXLSX).
	Format string
	// Kind is the addressed object class.
	Kind Kind
	// SheetID is the native <sheet sheetId=> of the scope worksheet (set for
	// KindSheet, KindCell, KindComment).
	SheetID string
	// CellRef is the A1 reference (KindCell), or the anchor A1 cell (KindComment).
	CellRef string
	// Name is the defined-name value (KindDefinedName).
	Name string
}

// Error codes for handle resolution failures. These mirror the PPTX typed
// error contract verbatim so agents see one stable vocabulary across formats.
const (
	// CodeMalformed: the string starts with the handle prefix but is not a valid
	// handle (wrong version, bad shape, unknown class, empty id).
	CodeMalformed = "HANDLE_MALFORMED"
	// CodeScopeStale: the handle is valid but its scope container (the worksheet
	// with that sheetId, or the workbook defined-name table) no longer holds the
	// addressed scope.
	CodeScopeStale = "HANDLE_SCOPE_STALE"
	// CodeStale: the scope exists but the addressed object (cell anchor, defined
	// name) no longer exists within it.
	CodeStale = "HANDLE_STALE"
	// CodeFormatMismatch: the handle's format tag does not match the package.
	CodeFormatMismatch = "HANDLE_FORMAT_MISMATCH"
	// CodeAmbiguous: the handle's native scope id is NOT unique (a duplicate
	// <sheet sheetId=> within the workbook). Such an id cannot be resolved to a
	// single object without guessing, so resolution refuses rather than silently
	// picking one and mis-targeting. Workbooks that are programmatically
	// generated or merged can carry duplicate sheetIds, so this is the contract
	// that prevents silent wrong-target data corruption.
	CodeAmbiguous = "HANDLE_AMBIGUOUS"
)

// Error is a typed handle error carrying a stable Code, the offending handle
// string, and a human message. It mirrors pkg/pptx/handle.Error.
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

// IsAddressPositional reports whether s is a valid XLSX handle whose addressed
// object is located by an A1 grid coordinate (the "a:" objref tag: cell and
// comment-anchor handles) rather than by a native id. Such handles survive sheet
// reorder/rename but are positional within the grid: a row/column insert/delete
// that shifts the A1 address is NOT survived (a row insert empties the address
// and fails HANDLE_STALE; a row delete can shift a populated cell onto the
// address, the silent-wrong-target case callers must guard against). It returns
// false for non-handles, handles of another format, native-id handles
// (sheet/defined-name), and malformed handles — so callers can use it as the
// single discriminator for "this target is position-dependent within its sheet"
// without sweeping in genuinely position-immune native-id handles.
func IsAddressPositional(s string) bool {
	if !IsHandle(s) {
		return false
	}
	h, err := Parse(s)
	if err != nil {
		return false
	}
	return h.Kind == KindCell || h.Kind == KindComment
}

// FormatSheet renders a sheet handle from a native sheetId.
func FormatSheet(sheetID string) string {
	return Format(Handle{Format: FormatXLSX, Kind: KindSheet, SheetID: sheetID})
}

// FormatCell renders a cell handle from a sheetId scope and an A1 ref.
func FormatCell(sheetID, cellRef string) string {
	return Format(Handle{Format: FormatXLSX, Kind: KindCell, SheetID: sheetID, CellRef: cellRef})
}

// FormatComment renders a comment handle from a sheetId scope and its anchor
// A1 cell.
func FormatComment(sheetID, anchorCell string) string {
	return Format(Handle{Format: FormatXLSX, Kind: KindComment, SheetID: sheetID, CellRef: anchorCell})
}

// FormatDefinedName renders a workbook-scoped defined-name handle.
func FormatDefinedName(name string) string {
	return Format(Handle{Format: FormatXLSX, Kind: KindDefinedName, Name: name})
}

// Format renders a Handle to its envelope string. It is the inverse of Parse
// for well-formed handles and must be byte-identical on round-trip.
func Format(h Handle) string {
	format := h.Format
	if format == "" {
		format = FormatXLSX
	}
	switch h.Kind {
	case KindSheet:
		return fmt.Sprintf("%s%s/%s:%s", VersionPrefix, format, scopeSheetKind, h.SheetID)
	case KindCell:
		return fmt.Sprintf("%s%s/%s:%s/%s:%s:%s", VersionPrefix, format, scopeSheetKind, h.SheetID, ClassCell, addrTag, h.CellRef)
	case KindComment:
		return fmt.Sprintf("%s%s/%s:%s/%s:%s:%s", VersionPrefix, format, scopeSheetKind, h.SheetID, ClassComment, addrTag, h.CellRef)
	case KindDefinedName:
		return fmt.Sprintf("%s%s/%s/%s:%s:%s", VersionPrefix, format, scopeWorkbook, ClassName, nativeTag, h.Name)
	default:
		return fmt.Sprintf("%s%s/%s:%s", VersionPrefix, format, scopeSheetKind, h.SheetID)
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
	if format != FormatXLSX {
		return Handle{}, &Error{Code: CodeFormatMismatch, Handle: s, Message: fmt.Sprintf("handle format tag %q does not match package format %q", format, FormatXLSX)}
	}

	scope := segments[1]

	// Workbook-scoped handles (defined names) use the bare "wb" scope token.
	if scope == scopeWorkbook {
		return parseWorkbookScoped(s, segments)
	}

	// Otherwise the scope is a worksheet identified by sheetId: "ws:<sheetId>".
	sheetID, err := parseSheetScope(scope)
	if err != nil {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: err.Error()}
	}

	h := Handle{Format: format, SheetID: sheetID}

	switch len(segments) {
	case 2:
		// Sheet-only handle.
		h.Kind = KindSheet
		return h, nil
	case 3:
		class, objref, ok := strings.Cut(segments[2], ":")
		if !ok {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("object segment %q must be <class>:<objref>", segments[2])}
		}
		switch class {
		case ClassCell:
			ref, err := parseAddrRef(objref)
			if err != nil {
				return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "cell objref: " + err.Error()}
			}
			h.Kind = KindCell
			h.CellRef = ref
			return h, nil
		case ClassComment:
			ref, err := parseAddrRef(objref)
			if err != nil {
				return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "comment objref: " + err.Error()}
			}
			h.Kind = KindComment
			h.CellRef = ref
			return h, nil
		default:
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("unknown object class %q (sheet-scoped classes: %q, %q)", class, ClassCell, ClassComment)}
		}
	default:
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "handle has too many segments"}
	}
}

// parseWorkbookScoped decodes an "H:xlsx/wb/<class>:<objref>" handle.
func parseWorkbookScoped(s string, segments []string) (Handle, error) {
	if len(segments) != 3 {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "workbook-scoped handle must be wb/<class>:<objref>"}
	}
	class, objref, ok := strings.Cut(segments[2], ":")
	if !ok {
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("object segment %q must be <class>:<objref>", segments[2])}
	}
	switch class {
	case ClassName:
		name, err := parseNativeRef(objref)
		if err != nil {
			return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: "name objref: " + err.Error()}
		}
		return Handle{Format: FormatXLSX, Kind: KindDefinedName, Name: name}, nil
	default:
		return Handle{}, &Error{Code: CodeMalformed, Handle: s, Message: fmt.Sprintf("unknown workbook-scoped class %q (supported: %q)", class, ClassName)}
	}
}

// parseSheetScope decodes a scope segment "ws:<sheetId>" into a sheetId string.
func parseSheetScope(scope string) (string, error) {
	kind, value, ok := strings.Cut(scope, ":")
	if !ok {
		return "", fmt.Errorf("scope %q must be %s:<sheetId> or %q", scope, scopeSheetKind, scopeWorkbook)
	}
	if kind != scopeSheetKind {
		return "", fmt.Errorf("unsupported scope kind %q (supported: %q sheet scope, %q workbook scope)", kind, scopeSheetKind, scopeWorkbook)
	}
	if value == "" {
		return "", fmt.Errorf("empty sheetId in scope")
	}
	return value, nil
}

// parseNativeRef decodes an objref of the form "n:<value>" into its value. The
// value may contain colons (e.g. a defined name is taken verbatim after the
// first tag separator), so only the leading "n:" tag is stripped.
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

// parseAddrRef decodes an objref of the form "a:<A1ref>" into the A1 ref. The
// "a" tag marks a positional grid address (see the package doc).
func parseAddrRef(objref string) (string, error) {
	tag, value, ok := strings.Cut(objref, ":")
	if !ok {
		return "", fmt.Errorf("objref %q must be %s:<A1ref>", objref, addrTag)
	}
	if tag != addrTag {
		return "", fmt.Errorf("unsupported objref tag %q (expected A1 address %q)", tag, addrTag)
	}
	if value == "" {
		return "", fmt.Errorf("empty A1 reference")
	}
	return value, nil
}
