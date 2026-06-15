package mutate

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
)

func TestNormalizeCustomOperator(t *testing.T) {
	cases := map[string]string{
		"greaterThan":  "greaterThan",
		"gt":           "greaterThan",
		">":            "greaterThan",
		"greater-than": "greaterThan",
		"between":      "between",
		"not-between":  "notBetween",
		"eq":           "equal",
	}
	for in, want := range cases {
		got, err := NormalizeCustomOperator(in)
		if err != nil {
			t.Fatalf("NormalizeCustomOperator(%q) error: %v", in, err)
		}
		if got != want {
			t.Fatalf("NormalizeCustomOperator(%q) = %q, want %q", in, got, want)
		}
	}
	if _, err := NormalizeCustomOperator("bogus"); err == nil {
		t.Fatalf("expected error for invalid operator")
	}
}

func TestBuildCustomFilterSimple(t *testing.T) {
	el, err := buildCustomFilter("", "greaterThan", "50", "")
	if err != nil {
		t.Fatalf("buildCustomFilter error: %v", err)
	}
	if el.Tag != "customFilters" {
		t.Fatalf("expected customFilters, got %q", el.Tag)
	}
	if el.SelectAttrValue("op", "") != "" {
		t.Fatalf("operator must NOT be on customFilters element")
	}
	children := el.ChildElements()
	if len(children) != 1 {
		t.Fatalf("expected 1 customFilter child, got %d", len(children))
	}
	if children[0].SelectAttrValue("operator", "") != "greaterThan" {
		t.Fatalf("expected operator on customFilter child, got %q", children[0].SelectAttrValue("operator", ""))
	}
	if children[0].SelectAttrValue("val", "") != "50" {
		t.Fatalf("expected val=50, got %q", children[0].SelectAttrValue("val", ""))
	}
}

func TestBuildCustomFilterBetweenDesugars(t *testing.T) {
	el, err := buildCustomFilter("", "between", "10", "20")
	if err != nil {
		t.Fatalf("buildCustomFilter error: %v", err)
	}
	if el.SelectAttrValue("and", "") != "1" {
		t.Fatalf("between must set and=1")
	}
	children := el.ChildElements()
	if len(children) != 2 {
		t.Fatalf("between must desugar to 2 customFilter children, got %d", len(children))
	}
	if children[0].SelectAttrValue("operator", "") != "greaterThanOrEqual" {
		t.Fatalf("first child should be greaterThanOrEqual, got %q", children[0].SelectAttrValue("operator", ""))
	}
	if children[1].SelectAttrValue("operator", "") != "lessThanOrEqual" {
		t.Fatalf("second child should be lessThanOrEqual, got %q", children[1].SelectAttrValue("operator", ""))
	}
}

func TestBuildCustomFilterBetweenRequiresVal2(t *testing.T) {
	if _, err := buildCustomFilter("", "between", "10", ""); err == nil {
		t.Fatalf("between without val2 should error")
	}
}

func TestBuildCustomFilterRejectsVal2ForNonBetween(t *testing.T) {
	if _, err := buildCustomFilter("", "greaterThan", "50", "100"); err == nil {
		t.Fatalf("val2 with a non-between operator should error to avoid silent OR logic")
	}
}

func TestSortConditionRefIsRange(t *testing.T) {
	rng, _ := address.ParseRange("A1:D10")
	ref, err := sortConditionRef(rng, "B")
	if err != nil {
		t.Fatalf("sortConditionRef error: %v", err)
	}
	// Must be a single-column ST_Ref range, not a bare column letter.
	if ref != "B1:B10" {
		t.Fatalf("expected B1:B10, got %q", ref)
	}
}

func TestSortConditionRefOutOfBounds(t *testing.T) {
	rng, _ := address.ParseRange("A1:D10")
	if _, err := sortConditionRef(rng, "F"); err == nil {
		t.Fatalf("expected out-of-bounds error for column F outside A1:D10")
	}
}

func TestAutoFilterColumnCount(t *testing.T) {
	af := etree.NewElement("autoFilter")
	af.CreateAttr("ref", "A1:D10")
	count, err := autoFilterColumnCount(af)
	if err != nil {
		t.Fatalf("autoFilterColumnCount error: %v", err)
	}
	if count != 4 {
		t.Fatalf("expected 4 columns, got %d", count)
	}
}

func TestDedupeNonEmpty(t *testing.T) {
	got := dedupeNonEmpty([]string{"a", "", "b", "a", "c", "b"})
	want := []string{"a", "b", "c"}
	if len(got) != len(want) {
		t.Fatalf("expected %v, got %v", want, got)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Fatalf("expected %v, got %v", want, got)
		}
	}
}
