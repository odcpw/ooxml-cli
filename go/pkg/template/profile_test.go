package template

import (
	"encoding/json"
	"testing"

	pptxmodel "github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func sampleTokens() *TemplateTokens {
	t := NewTokens(KindPPTX, "brand.potx")
	t.PPTX = &PPTXTokens{
		Theme: &pptxmodel.ThemeInfo{
			Name: "Acme",
			ColorScheme: &pptxmodel.ColorScheme{
				Dark1: "000000", Light1: "FFFFFF",
				Accent1: "4F81BD", Accent2: "C0504D",
			},
			FontScheme: &pptxmodel.FontScheme{MajorFont: "Calibri", MinorFont: "Calibri"},
		},
		DefaultTextStyles: []DefaultTextStyle{{Role: "title", FontRef: "major", SizePt: 44}},
		TableStyles:       []TableStyle{{StyleID: "{ABC}", Name: "Light"}},
		ChartStyles:       []ChartStyleSummary{{PartURI: "/ppt/charts/chart1.xml", SeriesFillColor: "FF0000"}},
	}
	return t
}

func TestProfileFromTokens_KeepsThemeDropsBulk(t *testing.T) {
	p := ProfileFromTokens(sampleTokens(), "Acme Brand", "desc")
	if p.SchemaVersion != ProfileSchemaVersion || p.Format != ProfileFormat {
		t.Fatalf("bad header: %+v", p)
	}
	if p.Metadata.Name != "Acme Brand" || p.Metadata.SourceFile != "brand.potx" || p.Metadata.SourceType != KindPPTX {
		t.Fatalf("bad metadata: %+v", p.Metadata)
	}
	if p.Design.Theme == nil || p.Design.Theme.ColorScheme.Accent1 != "4F81BD" {
		t.Fatalf("theme not carried verbatim: %+v", p.Design.Theme)
	}
	if p.Design.Theme.FontScheme.MajorFont != "Calibri" {
		t.Fatalf("font scheme not carried")
	}
	// Placeholder defaults retained (informational); chart/table styles dropped.
	if len(p.Design.Placeholders) != 1 {
		t.Fatalf("expected placeholders retained, got %d", len(p.Design.Placeholders))
	}
}

func TestProfile_RoundTripTokens(t *testing.T) {
	p := ProfileFromTokens(sampleTokens(), "x", "")
	lifted := p.ToTokens(KindPPTX)
	if lifted.Type != KindPPTX || lifted.PPTX == nil {
		t.Fatalf("lift produced wrong block: %+v", lifted)
	}
	if lifted.PPTX.Theme.ColorScheme.Accent1 != "4F81BD" {
		t.Fatalf("color lost on lift")
	}
	// BuildApplyPlan over the lifted tokens must match the plan over originals.
	sel := ApplySelection{Colors: true, Fonts: true}
	planOrig, err := BuildApplyPlan(sampleTokens(), sel)
	if err != nil {
		t.Fatalf("orig plan: %v", err)
	}
	planLifted, err := BuildApplyPlan(lifted, sel)
	if err != nil {
		t.Fatalf("lifted plan: %v", err)
	}
	if len(planOrig.Colors) != len(planLifted.Colors) {
		t.Fatalf("color count diverged: %d vs %d", len(planOrig.Colors), len(planLifted.Colors))
	}
	for i := range planOrig.Colors {
		if planOrig.Colors[i] != planLifted.Colors[i] {
			t.Fatalf("color %d diverged: %+v vs %+v", i, planOrig.Colors[i], planLifted.Colors[i])
		}
	}
	if (planOrig.Fonts == nil) != (planLifted.Fonts == nil) {
		t.Fatalf("font presence diverged")
	}
	if planOrig.Fonts != nil && *planOrig.Fonts != *planLifted.Fonts {
		t.Fatalf("fonts diverged: %+v vs %+v", *planOrig.Fonts, *planLifted.Fonts)
	}
	// Skipped lists must be identical (same not-present reasons), proving the
	// save -> apply round-trip is indistinguishable from apply --from.
	if len(planOrig.Skipped) != len(planLifted.Skipped) {
		t.Fatalf("skipped diverged: %v vs %v", planOrig.Skipped, planLifted.Skipped)
	}
	for i := range planOrig.Skipped {
		if planOrig.Skipped[i] != planLifted.Skipped[i] {
			t.Fatalf("skipped %d diverged: %q vs %q", i, planOrig.Skipped[i], planLifted.Skipped[i])
		}
	}
}

func TestProfile_LiftToXLSXBlock(t *testing.T) {
	p := ProfileFromTokens(sampleTokens(), "x", "")
	lifted := p.ToTokens(KindXLSX)
	if lifted.XLSX == nil || lifted.PPTX != nil {
		t.Fatalf("expected xlsx block only: %+v", lifted)
	}
	if lifted.XLSX.Theme.ColorScheme.Accent1 != "4F81BD" {
		t.Fatalf("colors are family-neutral; should carry into xlsx block")
	}
}

func TestProfile_JSONMarshalStable(t *testing.T) {
	p := ProfileFromTokens(sampleTokens(), "Acme", "d")
	b1, _ := json.Marshal(p)
	var rt DesignProfile
	if err := json.Unmarshal(b1, &rt); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	b2, _ := json.Marshal(&rt)
	if string(b1) != string(b2) {
		t.Fatalf("JSON not stable:\n%s\n%s", b1, b2)
	}
	if err := rt.Validate(); err != nil {
		t.Fatalf("round-tripped profile invalid: %v", err)
	}
}

func TestProfile_Validate(t *testing.T) {
	good := ProfileFromTokens(sampleTokens(), "x", "")
	if err := good.Validate(); err != nil {
		t.Fatalf("valid profile rejected: %v", err)
	}

	// Wrong format tag.
	bad := ProfileFromTokens(sampleTokens(), "x", "")
	bad.Format = "something-else"
	if err := bad.Validate(); err == nil {
		t.Fatalf("expected format rejection")
	}

	// Missing schema version.
	noVer := ProfileFromTokens(sampleTokens(), "x", "")
	noVer.SchemaVersion = ""
	if err := noVer.Validate(); err == nil {
		t.Fatalf("expected schemaVersion rejection")
	}

	// Invalid hex color.
	badHex := ProfileFromTokens(sampleTokens(), "x", "")
	badHex.Design.Theme.ColorScheme.Accent1 = "ZZZ"
	if err := badHex.Validate(); err == nil {
		t.Fatalf("expected invalid hex rejection")
	}
}

func TestProfile_HasDesign(t *testing.T) {
	if !ProfileFromTokens(sampleTokens(), "x", "").HasDesign() {
		t.Fatalf("expected HasDesign true")
	}
	empty := &DesignProfile{SchemaVersion: ProfileSchemaVersion, Format: ProfileFormat}
	if empty.HasDesign() {
		t.Fatalf("expected HasDesign false for empty profile")
	}
}

func TestProfileFromTokens_XLSXSource(t *testing.T) {
	tok := NewTokens(KindXLSX, "report.xltx")
	tok.XLSX = &XLSXTokens{
		Theme: &pptxmodel.ThemeInfo{ColorScheme: &pptxmodel.ColorScheme{Accent1: "112233"}},
	}
	p := ProfileFromTokens(tok, "", "")
	if p.Metadata.SourceType != KindXLSX || p.Design.Theme.ColorScheme.Accent1 != "112233" {
		t.Fatalf("xlsx-sourced profile wrong: %+v", p)
	}
}
