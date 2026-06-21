package styles

import "testing"

func TestParseStylesAndDateDetection(t *testing.T) {
	xml := []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="4">
    <numFmt numFmtId="164" formatCode="yyyy-mm-dd"/>
    <numFmt numFmtId="165" formatCode="[h]:mm:ss"/>
    <numFmt numFmtId="166" formatCode="0.00"/>
    <numFmt numFmtId="167" formatCode="0 &quot;days&quot;"/>
  </numFmts>
  <cellXfs count="6">
    <xf numFmtId="0"/>
    <xf numFmtId="14" applyNumberFormat="1"/>
    <xf numFmtId="164"/>
    <xf numFmtId="165"/>
    <xf numFmtId="166"/>
    <xf numFmtId="167"/>
  </cellXfs>
</styleSheet>`)

	parsed, err := ParseBytes(xml)
	if err != nil {
		t.Fatalf("ParseBytes returned error: %v", err)
	}
	if got := parsed.NumFmts[164]; got != "yyyy-mm-dd" {
		t.Fatalf("custom format 164 = %q, want yyyy-mm-dd", got)
	}
	if len(parsed.CellXfs) != 6 {
		t.Fatalf("CellXfs length = %d, want 6", len(parsed.CellXfs))
	}
	if !parsed.CellXfs[1].ApplyNumberFormat {
		t.Fatal("ApplyNumberFormat = false, want true")
	}
	if id, code, ok := parsed.NumberFormat(2); !ok || id != 164 || code != "yyyy-mm-dd" {
		t.Fatalf("NumberFormat(2) = id %d code %q ok %t, want 164 yyyy-mm-dd true", id, code, ok)
	}
	if id, code, ok := parsed.NumberFormat(1); !ok || id != 14 || code != "m/d/yy" {
		t.Fatalf("NumberFormat(1) = id %d code %q ok %t, want 14 m/d/yy true", id, code, ok)
	}

	tests := []struct {
		styleIndex int
		want       bool
	}{
		{0, false},
		{1, true},
		{2, true},
		{3, true},
		{4, false},
		{5, false},
		{-1, false},
		{99, false},
	}
	for _, tt := range tests {
		got := parsed.IsDateStyle(tt.styleIndex)
		if got != tt.want {
			t.Fatalf("IsDateStyle(%d) = %v, want %v", tt.styleIndex, got, tt.want)
		}
	}
}

func TestIsDateLikeFormat(t *testing.T) {
	tests := []struct {
		format string
		want   bool
	}{
		{"yyyy-mm-dd", true},
		{"m/d/yy", true},
		{"h:mm AM/PM", true},
		{"[$-409]d-mmm-yy", true},
		{"[h]:mm:ss", true},
		{"0.00", false},
		{"#,##0", false},
		{"0 \"days\"", false},
		{"[Red]0", false},
		{"General", false},
	}

	for _, tt := range tests {
		got := IsDateLikeFormat(tt.format)
		if got != tt.want {
			t.Fatalf("IsDateLikeFormat(%q) = %v, want %v", tt.format, got, tt.want)
		}
	}
}

func TestParseStylesInvalidRoot(t *testing.T) {
	if _, err := ParseBytes([]byte(`<worksheet/>`)); err == nil {
		t.Fatal("ParseBytes expected error for invalid root")
	}
}
