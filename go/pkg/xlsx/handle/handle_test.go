package handle

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
)

func intPtr(v int) *int { return &v }

// TestResolveDefinedNameSkipsSheetLocal is the Finding-2 regression: a
// workbook-scoped defined-name handle must NOT resolve to a sheet-local name of
// the same identifier. ResolveDefinedName matched purely on Name, ignoring
// scope, so it could (1) resolve to and mutate a sheet-local same-name and (2)
// return a false AMBIGUOUS when both a workbook and a sheet-local same-name
// exist. Resolution now restricts matching to workbook-scoped entries
// (LocalSheetID == nil).
func TestResolveDefinedNameSkipsSheetLocal(t *testing.T) {
	h, err := Parse("H:xlsx/wb/name:n:Print_Area")
	if err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}

	// Only a SHEET-LOCAL name of the same identifier exists: the workbook object
	// the handle addresses does not exist, so resolution is CodeStale (NOT a
	// silent resolve to the sheet-local name).
	sheetLocalOnly := []model.DefinedName{
		{Name: "Print_Area", LocalSheetID: intPtr(0)},
	}
	if _, err := ResolveDefinedName(sheetLocalOnly, h); !IsCode(err, CodeStale) {
		t.Fatalf("sheet-local only: got err %v, want CodeStale", err)
	}

	// Both a WORKBOOK and a SHEET-LOCAL name of the same identifier exist:
	// resolution returns the workbook one with NO false CodeAmbiguous.
	both := []model.DefinedName{
		{Name: "Print_Area", LocalSheetID: intPtr(0), Ref: "Sheet1!$A$1"},
		{Name: "Print_Area", Ref: "Sheet1!$Z$9"},
	}
	got, err := ResolveDefinedName(both, h)
	if err != nil {
		t.Fatalf("workbook+sheet-local: got err %v, want the workbook name", err)
	}
	if got.LocalSheetID != nil {
		t.Fatalf("resolved to a sheet-local name (LocalSheetID=%v); want the workbook-scoped one", *got.LocalSheetID)
	}
	if got.Ref != "Sheet1!$Z$9" {
		t.Fatalf("resolved to the wrong entry: ref=%q", got.Ref)
	}
}

func TestRoundTripSheet(t *testing.T) {
	s := FormatSheet("2")
	if s != "H:xlsx/ws:2" {
		t.Fatalf("FormatSheet = %q, want H:xlsx/ws:2", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindSheet || h.SheetID != "2" || h.Format != FormatXLSX {
		t.Fatalf("Parse(%q) = %+v, want sheet 2", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestRoundTripCell(t *testing.T) {
	s := FormatCell("2", "B7")
	if s != "H:xlsx/ws:2/cell:a:B7" {
		t.Fatalf("FormatCell = %q, want H:xlsx/ws:2/cell:a:B7", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindCell || h.SheetID != "2" || h.CellRef != "B7" {
		t.Fatalf("Parse(%q) = %+v, want cell B7 on sheet 2", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestRoundTripComment(t *testing.T) {
	s := FormatComment("3", "C4")
	if s != "H:xlsx/ws:3/comment:a:C4" {
		t.Fatalf("FormatComment = %q, want H:xlsx/ws:3/comment:a:C4", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindComment || h.SheetID != "3" || h.CellRef != "C4" {
		t.Fatalf("Parse(%q) = %+v, want comment C4 on sheet 3", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestRoundTripDefinedName(t *testing.T) {
	s := FormatDefinedName("SalesTotal")
	if s != "H:xlsx/wb/name:n:SalesTotal" {
		t.Fatalf("FormatDefinedName = %q, want H:xlsx/wb/name:n:SalesTotal", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindDefinedName || h.Name != "SalesTotal" {
		t.Fatalf("Parse(%q) = %+v, want defined name SalesTotal", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

// TestSheetIDStringPreserved proves the sheetId is carried verbatim (no integer
// normalization), so an unusual but legal value round-trips unchanged.
func TestSheetIDStringPreserved(t *testing.T) {
	s := FormatSheet("007")
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if h.SheetID != "007" {
		t.Fatalf("SheetID = %q, want 007 (no radix/leading-zero normalization)", h.SheetID)
	}
}

func TestIsHandle(t *testing.T) {
	handles := []string{"H:xlsx/ws:2", "H:xlsx/ws:2/cell:a:B7", "H:xlsx/wb/name:n:X", "H:garbage"}
	for _, s := range handles {
		if !IsHandle(s) {
			t.Errorf("IsHandle(%q) = false, want true", s)
		}
	}
	// Every legacy XLSX selector must NOT be treated as a handle.
	legacy := []string{
		"sheet:1", "sheetId:2", "name:Sales", "~Sales", "#1", "Sheet1",
		"A1", "Sheet1!A1", "$B$2", "A1:C5", "rId:rId1", "tableId:1",
		"definedName:1", "1", "", "h:xlsx/ws:1", // lowercase prefix is not a handle
	}
	for _, s := range legacy {
		if IsHandle(s) {
			t.Errorf("IsHandle(%q) = true, want false (legacy selector)", s)
		}
	}
}

func TestIsAddressPositional(t *testing.T) {
	// A1-tagged cell/comment handles are address-positional.
	positional := []string{
		"H:xlsx/ws:2/cell:a:B7",
		"H:xlsx/ws:10/comment:a:C3",
	}
	for _, s := range positional {
		if !IsAddressPositional(s) {
			t.Errorf("IsAddressPositional(%q) = false, want true", s)
		}
	}
	// Native-id handles, other-format handles, non-handles, and malformed
	// handles are NOT address-positional (must not be swept into position-
	// dependent classification).
	notPositional := []string{
		"H:xlsx/ws:2",                    // native sheet id (scope only)
		"H:xlsx/wb/name:n:SalesTotal",    // native defined name
		"H:pptx/s:256/shape:n:2",         // wrong format -> Parse rejects
		"H:xlsx/ws:2/cell:n:B7",          // wrong objref tag -> malformed
		"H:garbage",                      // malformed
		"A1", "Sheet1!A1", "sheet:1", "", // legacy selectors / non-handles
	}
	for _, s := range notPositional {
		if IsAddressPositional(s) {
			t.Errorf("IsAddressPositional(%q) = true, want false", s)
		}
	}
}

func TestParseMalformed(t *testing.T) {
	cases := []string{
		"H:",                      // nothing after prefix
		"H:xlsx",                  // no scope
		"H:xlsx/",                 // empty scope
		"H:xlsx/x:1",              // unsupported scope kind
		"H:xlsx/ws:",              // empty sheetId
		"H:xlsx/ws:2/badseg",      // object segment not class:objref
		"H:xlsx/ws:2/widget:a:B7", // unknown sheet-scoped class
		"H:xlsx/ws:2/cell:B7",     // missing addr tag
		"H:xlsx/ws:2/cell:n:B7",   // wrong objref tag (cell is positional, not native)
		"H:xlsx/ws:2/cell:a:",     // empty A1 ref
		"H:xlsx/wb",               // workbook scope with no class
		"H:xlsx/wb/name:B7",       // name objref missing native tag
		"H:xlsx/wb/name:n:",       // empty name
		"H:xlsx/wb/widget:n:X",    // unknown workbook-scoped class
		"H:xlsx/ws:2/cell:a:B7/x", // too many segments
	}
	for _, s := range cases {
		_, err := Parse(s)
		if err == nil {
			t.Errorf("Parse(%q) = nil error, want malformed", s)
			continue
		}
		if !IsCode(err, CodeMalformed) {
			t.Errorf("Parse(%q) error code = %v, want %s", s, err, CodeMalformed)
		}
	}
}

func TestParseFormatMismatch(t *testing.T) {
	_, err := Parse("H:pptx/s:256/shape:n:2")
	if !IsCode(err, CodeFormatMismatch) {
		t.Fatalf("Parse(wrong-format handle) code = %v, want %s", err, CodeFormatMismatch)
	}
}

func TestParseRejectsNonHandle(t *testing.T) {
	_, err := Parse("Sheet1!A1")
	if !IsCode(err, CodeMalformed) {
		t.Fatalf("Parse(non-handle) code = %v, want %s", err, CodeMalformed)
	}
}

func TestErrorIsCode(t *testing.T) {
	if IsCode(nil, CodeStale) {
		t.Fatal("IsCode(nil) = true, want false")
	}
	err := &Error{Code: CodeStale, Message: "gone"}
	if !IsCode(err, CodeStale) {
		t.Fatal("IsCode(stale) = false, want true")
	}
	if IsCode(err, CodeMalformed) {
		t.Fatal("IsCode wrong code = true, want false")
	}
}
