package body

import "testing"

func TestRunStyleReadsFirstStyledRun(t *testing.T) {
	bodyElem := fixtureBody(t, "apply-styles")
	blocks := Blocks(bodyElem)

	// The fixture's runs carry no run style by default.
	if got := RunStyle(blocks[0].Element); got != "" {
		t.Fatalf("run style = %q, want empty", got)
	}
}

func TestTableStyleReadsTblStyle(t *testing.T) {
	bodyElem := fixtureBody(t, "apply-styles")
	blocks := Blocks(bodyElem)

	// The fixture table has no tblPr/tblStyle.
	if got := TableStyle(blocks[2].Element); got != "" {
		t.Fatalf("table style = %q, want empty", got)
	}
}

func TestParagraphStyleReadsPStyle(t *testing.T) {
	bodyElem := fixtureBody(t, "apply-styles")
	blocks := Blocks(bodyElem)

	if got := ParagraphStyle(blocks[0].Element); got != "Heading1" {
		t.Fatalf("paragraph style = %q, want Heading1", got)
	}
	if got := ParagraphStyle(blocks[1].Element); got != "" {
		t.Fatalf("paragraph style = %q, want empty", got)
	}
}
