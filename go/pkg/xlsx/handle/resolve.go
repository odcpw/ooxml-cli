package handle

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

// ResolveSheetRef resolves a handle's worksheet scope to the single SheetRef
// whose native <sheet sheetId=> matches. It is the shared scope-resolution
// primitive: every sheet-scoped handle path (sheet selection, cell resolution,
// comment resolution) routes through it so the duplicate-sheetId ambiguity
// contract is enforced uniformly and NEVER silently first-wins on a duplicate.
//
// This is the XLSX analog of pkg/pptx/selectors.ResolveSlideRefForHandle. It
// delivers structural-edit survival: the sheet is found by its durable sheetId,
// not by any positional sheet:N, so a handle keeps pointing at the same sheet
// after OTHER sheets are inserted or reordered, or after this sheet is renamed.
// A DELETE of the addressed sheet does NOT survive — the handle goes stale (see
// below). This is only safe because new sheets are assigned RANDOM sheetIds (not
// max+1): a deleted sheet's id is never reused, so a stale handle resolves to
// ZERO matches (a clean CodeScopeStale) and can never silently re-point at a
// later-added sheet that inherited the freed id.
//
// A scope that no longer exists yields a typed CodeScopeStale error; a sheetId
// shared by MORE THAN ONE sheet yields a typed CodeAmbiguous error. Resolution
// does its OWN sheetId search rather than delegating to the legacy sheetId:
// selector, because the legacy matcher silently first-wins on a duplicate.
func ResolveSheetRef(sheets []model.SheetRef, h Handle) (model.SheetRef, error) {
	if h.Kind == KindDefinedName {
		return model.SheetRef{}, &Error{
			Code:    CodeMalformed,
			Handle:  Format(h),
			Message: "defined-name handle is workbook-scoped and has no worksheet scope",
		}
	}
	if strings.TrimSpace(h.SheetID) == "" {
		return model.SheetRef{}, &Error{
			Code:    CodeMalformed,
			Handle:  Format(h),
			Message: "handle has an empty sheetId scope",
		}
	}

	var matches []model.SheetRef
	for _, sheet := range sheets {
		if sheet.SheetID == h.SheetID {
			matches = append(matches, sheet)
		}
	}
	switch len(matches) {
	case 0:
		return model.SheetRef{}, &Error{
			Code:    CodeScopeStale,
			Handle:  Format(h),
			Message: fmt.Sprintf("no sheet with sheetId %q in workbook", h.SheetID),
		}
	case 1:
		return matches[0], nil
	default:
		return model.SheetRef{}, &Error{
			Code:    CodeAmbiguous,
			Handle:  Format(h),
			Message: fmt.Sprintf("sheetId %q is not unique in workbook (%d sheets share it); cannot resolve to a single sheet", h.SheetID, len(matches)),
		}
	}
}

// ResolveDefinedName resolves a workbook-scoped defined-name handle to the
// single WORKBOOK-scoped DefinedName whose native name matches. A name is
// workbook-unique in the workbook scope (Excel forbids two workbook-scoped names
// with the same name), so this is a native, position-independent address: it
// survives sheet reorder/rename and an edit of the name's own ref.
//
// The handle (H:xlsx/wb/name:n:<name>) is workbook-scoped and is only ever
// minted for workbook-scoped names, so resolution restricts matching to
// workbook-scoped entries: any name carrying a localSheetId (LocalSheetID !=
// nil) is SHEET-LOCAL and addresses a different object that this handle does not
// name. Skipping sheet-local entries prevents two wrong-target outcomes that a
// name-only match would produce: (1) silently resolving to — and mutating — a
// sheet-local name of the same identifier (e.g. a sheet-local Print_Area), and
// (2) a false CodeAmbiguous when both a workbook and a sheet-local same-name
// exist.
//
// A workbook-scoped name that no longer exists yields CodeStale (correct even
// when only a same-named sheet-local survives: the workbook object the handle
// addresses does not exist); a name that somehow matches more than one
// workbook-scoped defined name (a malformed/corrupt workbook) yields
// CodeAmbiguous rather than silently picking one.
func ResolveDefinedName(names []model.DefinedName, h Handle) (model.DefinedName, error) {
	if h.Kind != KindDefinedName {
		return model.DefinedName{}, &Error{
			Code:    CodeMalformed,
			Handle:  Format(h),
			Message: "expected a defined-name handle",
		}
	}
	var matches []model.DefinedName
	for _, name := range names {
		// Skip sheet-local names: a localSheetId means the entry is scoped to a
		// single sheet, not the workbook this handle addresses.
		if name.LocalSheetID != nil {
			continue
		}
		if name.Name == h.Name {
			matches = append(matches, name)
		}
	}
	switch len(matches) {
	case 0:
		return model.DefinedName{}, &Error{
			Code:    CodeStale,
			Handle:  Format(h),
			Message: fmt.Sprintf("no defined name %q in workbook", h.Name),
		}
	case 1:
		return matches[0], nil
	default:
		return model.DefinedName{}, &Error{
			Code:    CodeAmbiguous,
			Handle:  Format(h),
			Message: fmt.Sprintf("defined name %q is not unique in workbook (%d entries share it)", h.Name, len(matches)),
		}
	}
}
