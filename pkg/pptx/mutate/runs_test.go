package mutate

import (
	"strings"
	"testing"

	"github.com/beevik/etree"
)

func boolPtr(b bool) *bool      { return &b }
func strPtr(s string) *string   { return &s }
func f64Ptr(f float64) *float64 { return &f }

const aNS = `xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"`

// newRunElement builds an <a:r><a:t>text</a:t></a:r> element with proper
// namespace bindings (parsed from XML) so xmlx namespace lookups resolve.
func newRunElement(text string) *etree.Element {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<a:r ` + aNS + `><a:t>` + text + `</a:t></a:r>`); err != nil {
		panic(err)
	}
	return doc.Root()
}

// newParagraphElement builds an a:p with the given inner XML (already a-prefixed).
func newParagraphElement(inner string) *etree.Element {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<a:p ` + aNS + `>` + inner + `</a:p>`); err != nil {
		panic(err)
	}
	return doc.Root()
}

func TestApplyRunOptionsSetsBoldItalicAttributes(t *testing.T) {
	r := newRunElement("hello")
	if err := ApplyRunOptions(r, &RunMutationOptions{Bold: boolPtr(true), Italic: boolPtr(false)}); err != nil {
		t.Fatalf("ApplyRunOptions: %v", err)
	}
	rPr := r.FindElement("rPr")
	if rPr == nil {
		t.Fatal("expected rPr to be created")
	}
	if rPr.SelectAttrValue("b", "") != "1" {
		t.Fatalf("expected b=1, got %q", rPr.SelectAttrValue("b", "missing"))
	}
	if rPr.SelectAttrValue("i", "") != "0" {
		t.Fatalf("expected i=0, got %q", rPr.SelectAttrValue("i", "missing"))
	}
	// a:t must remain after a:rPr.
	children := r.ChildElements()
	if len(children) != 2 || localTag(children[0].Tag) != "rPr" || localTag(children[1].Tag) != "t" {
		t.Fatalf("unexpected run child order: %v", childTags(r))
	}
}

func TestApplyRunOptionsFontSizeHundredths(t *testing.T) {
	r := newRunElement("x")
	if err := ApplyRunOptions(r, &RunMutationOptions{FontSize: f64Ptr(24)}); err != nil {
		t.Fatalf("ApplyRunOptions: %v", err)
	}
	if sz := r.FindElement("rPr").SelectAttrValue("sz", ""); sz != "2400" {
		t.Fatalf("expected sz=2400, got %q", sz)
	}
}

func TestApplyRunOptionsColorSolidFill(t *testing.T) {
	r := newRunElement("x")
	if err := ApplyRunOptions(r, &RunMutationOptions{Color: strPtr("ff0000")}); err != nil {
		t.Fatalf("ApplyRunOptions: %v", err)
	}
	srgb := r.FindElement("rPr/solidFill/srgbClr")
	if srgb == nil {
		t.Fatalf("expected solidFill/srgbClr, got %v", childTags(r.FindElement("rPr")))
	}
	if val := srgb.SelectAttrValue("val", ""); val != "FF0000" {
		t.Fatalf("expected val=FF0000 (upper), got %q", val)
	}
}

func TestApplyRunOptionsRPrChildOrderSolidFillBeforeLatin(t *testing.T) {
	r := newRunElement("x")
	// Apply font-family first, then color, to prove ordering is by schema, not call order.
	if err := ApplyRunOptions(r, &RunMutationOptions{FontFamily: strPtr("Arial")}); err != nil {
		t.Fatalf("ApplyRunOptions font: %v", err)
	}
	if err := ApplyRunOptions(r, &RunMutationOptions{Color: strPtr("00FF00"), HyperlinkRelID: strPtr("rId7")}); err != nil {
		t.Fatalf("ApplyRunOptions color: %v", err)
	}
	tags := childTags(r.FindElement("rPr"))
	want := []string{"solidFill", "latin", "hlinkClick"}
	if strings.Join(tags, ",") != strings.Join(want, ",") {
		t.Fatalf("rPr child order = %v, want %v", tags, want)
	}
}

func TestApplyRunOptionsRemoveBoldRemovesAttribute(t *testing.T) {
	r := newRunElement("x")
	if err := ApplyRunOptions(r, &RunMutationOptions{Bold: boolPtr(true)}); err != nil {
		t.Fatalf("set bold: %v", err)
	}
	if err := ApplyRunOptions(r, &RunMutationOptions{RemoveBold: true}); err != nil {
		t.Fatalf("remove bold: %v", err)
	}
	if r.FindElement("rPr").SelectAttr("b") != nil {
		t.Fatal("expected b attribute to be removed")
	}
}

func TestApplyRunOptionsRemoveColorRemovesSolidFill(t *testing.T) {
	r := newRunElement("x")
	if err := ApplyRunOptions(r, &RunMutationOptions{Color: strPtr("ABCDEF")}); err != nil {
		t.Fatalf("set color: %v", err)
	}
	if err := ApplyRunOptions(r, &RunMutationOptions{RemoveColor: true}); err != nil {
		t.Fatalf("remove color: %v", err)
	}
	if r.FindElement("rPr/solidFill") != nil {
		t.Fatal("expected solidFill to be removed")
	}
}

func TestApplyRunOptionsValidationErrors(t *testing.T) {
	cases := []struct {
		name string
		opts *RunMutationOptions
	}{
		{"bad color", &RunMutationOptions{Color: strPtr("ZZZ")}},
		{"short color", &RunMutationOptions{Color: strPtr("FFF")}},
		{"bad underline", &RunMutationOptions{Underline: strPtr("squiggly")}},
		{"zero font size", &RunMutationOptions{FontSize: f64Ptr(0)}},
		{"negative font size", &RunMutationOptions{FontSize: f64Ptr(-3)}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			r := newRunElement("x")
			if err := ApplyRunOptions(r, tc.opts); err == nil {
				t.Fatalf("expected error for %s", tc.name)
			}
			// On validation failure the run must be untouched (no rPr created).
			if r.FindElement("rPr") != nil {
				t.Fatalf("expected no rPr on validation failure for %s", tc.name)
			}
		})
	}
}

func TestTextRunAtSkipsNonRunChildren(t *testing.T) {
	p := newParagraphElement(`<a:pPr/><a:r><a:t>zero</a:t></a:r><a:br/><a:r><a:t>one</a:t></a:r>`)
	if CountTextRuns(p) != 2 {
		t.Fatalf("CountTextRuns = %d, want 2", CountTextRuns(p))
	}
	r1 := TextRunAt(p, 1)
	if r1 == nil {
		t.Fatal("TextRunAt(1) returned nil")
	}
	if txt := r1.FindElement("t"); txt == nil || txt.Text() != "one" {
		t.Fatalf("TextRunAt(1) did not return the second run, got %v", childTags(r1))
	}
	if TextRunAt(p, 2) != nil {
		t.Fatal("TextRunAt(2) should be nil (out of range)")
	}
}

func childTags(elem *etree.Element) []string {
	if elem == nil {
		return nil
	}
	var tags []string
	for _, c := range elem.ChildElements() {
		tags = append(tags, localTag(c.Tag))
	}
	return tags
}
