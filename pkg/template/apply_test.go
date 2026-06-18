package template

import (
	"reflect"
	"testing"

	pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func sampleColorScheme() *pptxmodel.ColorScheme {
	return &pptxmodel.ColorScheme{
		Dark1: "000000", Light1: "FFFFFF", Dark2: "1F497D", Light2: "EEECE1",
		Accent1: "4F81BD", Accent2: "C0504D", Accent3: "9BBB59", Accent4: "8064A2",
		Accent5: "4BACC6", Accent6: "F79646", HypLink: "0000FF", FolLink: "800080",
	}
}

func TestBuildApplyPlan_ColorsAndFonts(t *testing.T) {
	src := &TemplateTokens{
		SchemaVersion: SchemaVersion, Type: KindPPTX,
		PPTX: &PPTXTokens{Theme: &pptxmodel.ThemeInfo{
			ColorScheme: sampleColorScheme(),
			FontScheme:  &pptxmodel.FontScheme{MajorFont: "Arial", MinorFont: "Georgia"},
		}},
	}
	plan, err := BuildApplyPlan(src, ApplySelection{Colors: true, Fonts: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(plan.Colors) != 12 {
		t.Fatalf("expected 12 colors, got %d", len(plan.Colors))
	}
	// First color must map dk1 -> Dark1 (OOXML name, not JSON name).
	if plan.Colors[0].Name != "dk1" || plan.Colors[0].Hex != "000000" {
		t.Fatalf("unexpected first color: %+v", plan.Colors[0])
	}
	// hlink/folHlink OOXML names must be used.
	if plan.Colors[10].Name != "hlink" || plan.Colors[11].Name != "folHlink" {
		t.Fatalf("hyperlink color names not mapped to OOXML: %+v", plan.Colors[10:])
	}
	if plan.Fonts == nil || plan.Fonts.MajorFont != "Arial" || plan.Fonts.MinorFont != "Georgia" {
		t.Fatalf("unexpected fonts: %+v", plan.Fonts)
	}
}

func TestBuildApplyPlan_SkipsInvalidAndEmptyColors(t *testing.T) {
	cs := sampleColorScheme()
	cs.Dark1 = "windowText" // unresolved system color (not hex)
	cs.Accent1 = ""         // missing
	src := &TemplateTokens{PPTX: &PPTXTokens{Theme: &pptxmodel.ThemeInfo{ColorScheme: cs}}}

	plan, err := BuildApplyPlan(src, ApplySelection{Colors: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	// 12 - 2 skipped = 10 applied.
	if len(plan.Colors) != 10 {
		t.Fatalf("expected 10 valid colors, got %d", len(plan.Colors))
	}
	for _, c := range plan.Colors {
		if c.Name == "dk1" || c.Name == "accent1" {
			t.Fatalf("invalid/empty color %s should have been skipped", c.Name)
		}
	}
	if len(plan.Skipped) != 2 {
		t.Fatalf("expected 2 skip reasons, got %d: %v", len(plan.Skipped), plan.Skipped)
	}
}

func TestBuildApplyPlan_NoThemeRecordsSkip(t *testing.T) {
	src := &TemplateTokens{XLSX: &XLSXTokens{}}
	_, err := BuildApplyPlan(src, ApplySelection{Colors: true, Fonts: true})
	if err == nil {
		t.Fatal("expected error when nothing applies")
	}
}

func TestBuildApplyPlan_ChartRepresentative(t *testing.T) {
	src := &TemplateTokens{PPTX: &PPTXTokens{ChartStyles: []ChartStyleSummary{
		{PartURI: "/a", ChartType: "barChart"}, // no colors
		{PartURI: "/b", SeriesFillColor: "112233", SeriesLineColor: "445566"},
	}}}
	plan, err := BuildApplyPlan(src, ApplySelection{Charts: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if plan.Chart == nil || plan.Chart.SeriesFillColor != "112233" || plan.Chart.SeriesLineColor != "445566" {
		t.Fatalf("unexpected chart plan: %+v", plan.Chart)
	}
}

func TestBuildApplyPlan_TextStyles(t *testing.T) {
	src := &TemplateTokens{PPTX: &PPTXTokens{DefaultTextStyles: []DefaultTextStyle{
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "Title", FontRef: "Major", SizePt: 24, ColorRef: "accent1"},
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "body", FontRef: "minor", SizePt: 20, Color: "#112233"},
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "other"},
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "unsupported", SizePt: 12},
	}}}
	plan, err := BuildApplyPlan(src, ApplySelection{TextStyles: true})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(plan.TextStyles) != 2 {
		t.Fatalf("expected 2 text styles, got %d: %+v", len(plan.TextStyles), plan.TextStyles)
	}
	if plan.TextStyles[0].Role != "title" || plan.TextStyles[0].FontRef != "major" {
		t.Fatalf("title text style was not normalized: %+v", plan.TextStyles[0])
	}
	if plan.TextStyles[1].Color != "112233" {
		t.Fatalf("literal color was not normalized: %+v", plan.TextStyles[1])
	}
	if len(plan.Skipped) != 2 {
		t.Fatalf("expected 2 skipped text style reasons, got %d: %v", len(plan.Skipped), plan.Skipped)
	}
}

func TestBuildApplyPlan_TextStylesRejectsInvalidValues(t *testing.T) {
	src := &TemplateTokens{PPTX: &PPTXTokens{DefaultTextStyles: []DefaultTextStyle{
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "title", FontRef: "display", SizePt: 24},
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "body", FontRef: "minor", Color: "ZZZZZZ"},
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "other", ColorRef: "notAThemeRef"},
		{MasterRef: "/ppt/slideMasters/slideMaster1.xml", Role: "body", Color: "112233", ColorRef: "tx1"},
	}}}
	plan, err := BuildApplyPlan(src, ApplySelection{TextStyles: true})
	if err == nil {
		t.Fatal("expected no usable text styles")
	}
	if len(plan.TextStyles) != 0 {
		t.Fatalf("invalid text styles should not be planned: %+v", plan.TextStyles)
	}
	if len(plan.Skipped) != 4 {
		t.Fatalf("expected 4 skipped reasons, got %d: %v", len(plan.Skipped), plan.Skipped)
	}
}

func TestIsValidHex(t *testing.T) {
	cases := map[string]bool{
		"FF0000": true, "abcdef": true, "00000": false,
		"FF00000": false, "GG0000": false, "": false, "windowText": false,
	}
	for in, want := range cases {
		if got := IsValidHex(in); got != want {
			t.Errorf("IsValidHex(%q) = %v, want %v", in, got, want)
		}
	}
}

func TestFindDecorativeKeys(t *testing.T) {
	raw := map[string]interface{}{
		"pptx":        map[string]interface{}{},
		"gradients":   map[string]interface{}{},
		"animations":  []interface{}{},
		"unsupported": 1,
	}
	got := FindDecorativeKeys(raw)
	want := []string{"gradients", "animations"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("FindDecorativeKeys = %v, want %v", got, want)
	}
}
