package cli

import (
	"encoding/json"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

func TestPPTXAnimationsCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	animations := findSubcommand(pptx, "animations")
	if animations == nil {
		t.Fatal("pptx animations command is not registered")
	}
	if findSubcommand(animations, "list") == nil {
		t.Fatal("pptx animations list command is not registered")
	}
}

func TestPPTXAnimationsListJSON(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "animations-synthetic")
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "animations", "list", fixture,
	)
	var report inspect.AnimationsReport
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		t.Fatalf("failed to unmarshal animations list JSON: %v\n%s", err, output)
	}
	if len(report.Slides) != 5 {
		t.Fatalf("expected 5 slides, got %d", len(report.Slides))
	}

	// Slide 1: four in-scope entrances, collapsed one record each, in order.
	s1 := report.Slides[0]
	if !s1.HasTiming {
		t.Fatal("slide 1 should report HasTiming")
	}
	wantKinds := []string{"appear", "fade", "wipe", "flyIn"}
	if len(s1.Effects) != len(wantKinds) {
		t.Fatalf("slide 1: expected %d effects, got %d", len(wantKinds), len(s1.Effects))
	}
	for i, want := range wantKinds {
		if s1.Effects[i].EffectKind != want {
			t.Fatalf("slide 1 effect %d kind = %q, want %q", i, s1.Effects[i].EffectKind, want)
		}
		if !s1.Effects[i].Supported {
			t.Fatalf("slide 1 effect %d should be supported", i)
		}
		if s1.Effects[i].PrimarySelector == "" || !containsString(s1.Effects[i].Selectors, s1.Effects[i].PrimarySelector) {
			t.Fatalf("slide 1 effect %d missing selectors: %+v", i, s1.Effects[i])
		}
	}

	// Slide 4: stale targets surfaced.
	s4 := report.Slides[3]
	if len(s4.Effects) != 2 {
		t.Fatalf("slide 4: expected 2 effects, got %d", len(s4.Effects))
	}
	if !s4.Effects[0].Stale || s4.Effects[0].StaleReason != "missing-shape" {
		t.Fatalf("slide 4 effect 0 should be stale missing-shape: %+v", s4.Effects[0])
	}

	// Slide 5: media resolved with click-to-play.
	s5 := report.Slides[4]
	if len(s5.Media) != 1 || !s5.Media[0].HasClickToPlay {
		t.Fatalf("slide 5 should carry one click-to-play media entry: %+v", s5.Media)
	}
	if s5.Media[0].MediaPartURI != "/ppt/media/media1.mp4" {
		t.Fatalf("slide 5 media part = %q", s5.Media[0].MediaPartURI)
	}
}

func TestPPTXAnimationsListText(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "animations-synthetic")
	output := executePPTXShapesCommand(t,
		"pptx", "animations", "list", fixture,
	)
	for _, want := range []string{
		"Slide 1:",
		"appear start=onClick",
		"filter=wipe(up)",
		"build=byParagraph",
		"STALE:missing-shape",
		"kind=video clickToPlay=true",
	} {
		if !strings.Contains(output, want) {
			t.Fatalf("text output missing %q\n%s", want, output)
		}
	}
}

func TestPPTXAnimationsListNoTiming(t *testing.T) {
	fixture := pptxShapesFixturePath(t, "minimal-title")
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "animations", "list", fixture,
	)
	var report inspect.AnimationsReport
	if err := json.Unmarshal([]byte(output), &report); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, output)
	}
	for _, s := range report.Slides {
		if s.HasTiming {
			t.Fatalf("slide %d should report no timing", s.Slide)
		}
		if len(s.Effects) != 0 {
			t.Fatalf("slide %d should have no effects", s.Slide)
		}
	}
}

func TestPPTXAnimationsListMissingFile(t *testing.T) {
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "animations", "list", "/nonexistent/deck.pptx",
	)
	assertPPTXShapesExitCode(t, err, ExitFileNotFound)
}

func TestPPTXAnimationsListNonPPTX(t *testing.T) {
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "animations", "list", "../../go.mod",
	)
	if err == nil {
		t.Fatal("expected an error for a non-pptx input")
	}
}
