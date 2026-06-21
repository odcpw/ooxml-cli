package handle

import (
	"strconv"
	"testing"
)

func TestRoundTripComment(t *testing.T) {
	s := FormatComment(3)
	if s != "H:docx/pt:doc/comment:n:3" {
		t.Fatalf("FormatComment = %q, want H:docx/pt:doc/comment:n:3", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindComment || h.CommentID != 3 || h.Format != FormatDOCX {
		t.Fatalf("Parse(%q) = %+v, want comment 3", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestRoundTripStyle(t *testing.T) {
	s := FormatStyle("Heading1")
	if s != "H:docx/pt:styles/style:n:Heading1" {
		t.Fatalf("FormatStyle = %q, want H:docx/pt:styles/style:n:Heading1", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindStyle || h.StyleID != "Heading1" {
		t.Fatalf("Parse(%q) = %+v, want style Heading1", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestRoundTripParagraph(t *testing.T) {
	s := FormatParagraph("1C9E4F2A")
	if s != "H:docx/pt:doc/para:m:1C9E4F2A" {
		t.Fatalf("FormatParagraph = %q, want H:docx/pt:doc/para:m:1C9E4F2A", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindParagraph || h.ParaID != "1C9E4F2A" {
		t.Fatalf("Parse(%q) = %+v, want paragraph 1C9E4F2A", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestMintParaIDStaysBelowOpenXMLSDKMax(t *testing.T) {
	existing := map[string]bool{}
	for i := 0; i < 256; i++ {
		id := mintParaID(existing)
		value, err := strconv.ParseUint(id, 16, 32)
		if err != nil {
			t.Fatalf("mintParaID produced non-hex value %q: %v", id, err)
		}
		if value >= 0x80000000 {
			t.Fatalf("mintParaID = %q (%#x), want < 0x80000000", id, value)
		}
		existing[id] = true
	}
}

func TestIsHandle(t *testing.T) {
	handles := []string{
		"H:docx/pt:doc/comment:n:3",
		"H:docx/pt:styles/style:n:Heading1",
		"H:docx/pt:doc/para:m:ABCD1234",
		"H:garbage",
	}
	for _, s := range handles {
		if !IsHandle(s) {
			t.Errorf("IsHandle(%q) = false, want true", s)
		}
	}
	// Every legacy DOCX selector must NOT be treated as a handle.
	legacy := []string{
		"body.b1", "block:2", "Heading1", "Normal", "0", "3", "", "h:docx/pt:doc/comment:n:3",
	}
	for _, s := range legacy {
		if IsHandle(s) {
			t.Errorf("IsHandle(%q) = true, want false (legacy selector)", s)
		}
	}
}

func TestParseMalformed(t *testing.T) {
	cases := []string{
		"H:",                            // nothing after prefix
		"H:docx",                        // no scope/object
		"H:docx/pt:doc",                 // missing object segment
		"H:docx/pt:doc/badseg",          // object not class:objref
		"H:docx/pt:doc/widget:n:1",      // unknown class
		"H:docx/pt:doc/comment:n:",      // empty native id
		"H:docx/pt:doc/comment:n:abc",   // non-numeric comment id
		"H:docx/pt:doc/comment:m:3",     // wrong tag (comment is native, not marker)
		"H:docx/pt:styles/comment:n:3",  // comment in wrong scope
		"H:docx/pt:doc/style:n:Heading", // style in wrong scope
		"H:docx/pt:styles/style:n:",     // empty styleId
		"H:docx/pt:styles/style:m:X",    // wrong tag for style
		"H:docx/pt:doc/para:m:",         // empty paraId
		"H:docx/pt:doc/para:n:X",        // wrong tag for paragraph (marker, not native)
		"H:docx/pt:styles/para:m:X",     // paragraph in wrong scope
		"H:docx/pt:doc/comment:n:3/x",   // too many segments
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

func TestStyleIDPreservedVerbatim(t *testing.T) {
	// A styleId with mixed case / digits round-trips byte-for-byte.
	s := FormatStyle("ListParagraph2")
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse error: %v", err)
	}
	if h.StyleID != "ListParagraph2" {
		t.Fatalf("StyleID = %q, want ListParagraph2", h.StyleID)
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
