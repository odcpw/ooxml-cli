package handle

import "testing"

func TestRoundTripSlide(t *testing.T) {
	s := FormatSlide(256)
	if s != "H:pptx/s:256" {
		t.Fatalf("FormatSlide = %q, want H:pptx/s:256", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindSlide || h.SlideID != 256 || h.Format != FormatPPTX {
		t.Fatalf("Parse(%q) = %+v, want slide 256", s, h)
	}
}

func TestRoundTripShape(t *testing.T) {
	s := FormatShape(256, 5)
	if s != "H:pptx/s:256/shape:n:5" {
		t.Fatalf("FormatShape = %q, want H:pptx/s:256/shape:n:5", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindShape || h.SlideID != 256 || h.ShapeID != 5 {
		t.Fatalf("Parse(%q) = %+v, want shape 5 on slide 256", s, h)
	}
	// Re-format must be byte-identical (round-trip stability).
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestRoundTripShapeIDZero(t *testing.T) {
	// cNvPr ids are non-negative; 0 is a legal (if unusual) value.
	s := FormatShape(1, 0)
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.ShapeID != 0 {
		t.Fatalf("ShapeID = %d, want 0", h.ShapeID)
	}
}

func TestRoundTripComment(t *testing.T) {
	s := FormatComment(256, 7, 3)
	if s != "H:pptx/s:256/comment:idx:7:authorId:3" {
		t.Fatalf("FormatComment = %q, want H:pptx/s:256/comment:idx:7:authorId:3", s)
	}
	h, err := Parse(s)
	if err != nil {
		t.Fatalf("Parse(%q) error: %v", s, err)
	}
	if h.Kind != KindComment || h.SlideID != 256 || h.CommentID != 7 || h.AuthorID != 3 {
		t.Fatalf("Parse(%q) = %+v, want comment 7 author 3 on slide 256", s, h)
	}
	if got := Format(h); got != s {
		t.Fatalf("Format(Parse(%q)) = %q, want identical", s, got)
	}
}

func TestIsHandle(t *testing.T) {
	handles := []string{"H:pptx/s:256", "H:pptx/s:256/shape:n:5", "H:pptx/s:256/comment:idx:7:authorId:3", "H:garbage"}
	for _, s := range handles {
		if !IsHandle(s) {
			t.Errorf("IsHandle(%q) = false, want true", s)
		}
	}
	// Every legacy selector must NOT be treated as a handle.
	legacy := []string{
		"title", "body:0", "@body", "#3", "~My Shape", "shape:5",
		"1", "1-3", "1,3,5-7", "@all-shapes", "h1:something", "",
		"H", "h:pptx/s:1", // lowercase prefix is not the handle prefix
	}
	for _, s := range legacy {
		if IsHandle(s) {
			t.Errorf("IsHandle(%q) = true, want false (legacy selector)", s)
		}
	}
}

func TestParseMalformed(t *testing.T) {
	cases := []string{
		"H:",                         // nothing after prefix
		"H:pptx",                     // no scope
		"H:pptx/",                    // empty scope
		"H:pptx/x:1",                 // unsupported scope kind
		"H:pptx/s:",                  // empty slide id
		"H:pptx/s:abc",               // non-numeric slide id
		"H:pptx/s:1/badseg",          // object segment not class:objref
		"H:pptx/s:1/widget:n:5",      // unknown class
		"H:pptx/s:1/shape:5",         // missing native tag
		"H:pptx/s:1/shape:x:5",       // unsupported objref tag
		"H:pptx/s:1/shape:n:",        // empty native id
		"H:pptx/s:1/shape:n:abc",     // non-numeric native id
		"H:pptx/s:1/shape:n:-1",      // negative native id
		"H:pptx/s:1/comment:idx:1",   // missing authorId
		"H:pptx/s:1/comment:n:1",     // wrong comment tag
		"H:pptx/s:1/shape:n:5/extra", // too many segments
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
	_, err := Parse("H:docx/pt:doc/para:m:1A2B3C4D")
	if !IsCode(err, CodeFormatMismatch) {
		t.Fatalf("Parse(wrong-format handle) code = %v, want %s", err, CodeFormatMismatch)
	}
}

func TestParseRejectsNonHandle(t *testing.T) {
	// Parse on a non-handle string is malformed (callers gate via IsHandle).
	_, err := Parse("title")
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
