package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// listAnimations re-runs `animations list --json` and returns the parsed report.
func listAnimations(t *testing.T, path string) *inspect.AnimationsReport {
	t.Helper()
	out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "animations", "list", path)
	if err != nil {
		t.Fatalf("animations list failed: %v\n%s", err, out)
	}
	var rep inspect.AnimationsReport
	if err := json.Unmarshal([]byte(out), &rep); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, out)
	}
	return &rep
}

func reportSlide(t *testing.T, rep *inspect.AnimationsReport, n int) inspect.AnimationsSlideInfo {
	t.Helper()
	for _, s := range rep.Slides {
		if s.Slide == n {
			return s
		}
	}
	t.Fatalf("slide %d not in report", n)
	return inspect.AnimationsSlideInfo{}
}

// TestPPTXAnimationsAddAppearValidatesAndReadsBack drives the full mutation
// contract: --out write, validate-by-default + explicit strict validate, JSON
// readback envelope, and the reader confirming the authored effect.
func TestPPTXAnimationsAddAppearValidatesAndReadsBack(t *testing.T) {
	deck := getTestFilePath("title-content", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "appear.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "animations", "add", deck,
		"--slide", "1", "--shape", "shape:2", "--effect", "appear", "--out", out)
	if err != nil {
		t.Fatalf("animations add failed: %v\n%s", err, output)
	}
	var res PPTXAnimationsAddResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal add: %v\n%s", err, output)
	}
	if res.Action != "pptx.animations.add" || res.Output != out {
		t.Fatalf("unexpected envelope: %+v", res)
	}
	if res.Effect != "appear" || res.ShapeID != 2 || !res.CreatedTiming {
		t.Fatalf("unexpected result: %+v", res)
	}
	if len(res.AddedEffectIDs) != 1 {
		t.Fatalf("expected 1 effect id, got %v", res.AddedEffectIDs)
	}
	if res.RenderUnconfirmed {
		t.Fatal("appear is golden-confirmed; should not be render-unconfirmed")
	}
	if res.ValidateCommand == "" || res.ReadbackCommand == "" {
		t.Fatalf("missing generated commands: %+v", res)
	}

	// Explicit strict validation of the output.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("strict validate after add failed: %v", err)
	}

	// Reader confirms the authored effect.
	s1 := reportSlide(t, listAnimations(t, out), 1)
	if !s1.HasTiming || len(s1.Effects) != 1 || s1.Effects[0].EffectKind != "appear" {
		t.Fatalf("readback mismatch: %+v", s1)
	}
}

// TestPPTXAnimationsAddAllKindsValidate authors each entrance kind and validates.
func TestPPTXAnimationsAddAllKindsValidate(t *testing.T) {
	for _, kind := range []string{"fade", "wipe", "fly-in"} {
		kind := kind
		t.Run(kind, func(t *testing.T) {
			deck := getTestFilePath("title-content", "presentation.pptx")
			out := filepath.Join(t.TempDir(), kind+".pptx")
			args := []string{"--format", "json", "pptx", "animations", "add", deck,
				"--slide", "1", "--shape", "shape:2", "--effect", kind, "--out", out}
			if kind == "wipe" || kind == "fly-in" {
				args = append(args, "--direction", "up")
			}
			output, err := executeRootForXLSXTest(t, args...)
			if err != nil {
				t.Fatalf("add %s failed: %v\n%s", kind, err, output)
			}
			if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
				t.Fatalf("strict validate %s failed: %v", kind, err)
			}
			s1 := reportSlide(t, listAnimations(t, out), 1)
			if len(s1.Effects) != 1 {
				t.Fatalf("%s: expected 1 effect, got %d", kind, len(s1.Effects))
			}
		})
	}
}

// TestPPTXAnimationsByParagraphValidates authors a by-paragraph build and checks
// the per-paragraph fan-out and the build="p" token validate.
func TestPPTXAnimationsByParagraphValidates(t *testing.T) {
	deck := getTestFilePath("edge-large-deck", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "bp.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "animations", "add", deck,
		"--slide", "2", "--shape", "shape:3", "--effect", "fade", "--by-paragraph", "--out", out)
	if err != nil {
		t.Fatalf("by-paragraph add failed: %v\n%s", err, output)
	}
	var res PPTXAnimationsAddResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if !res.ByParagraph || res.ParagraphCount < 2 {
		t.Fatalf("expected a multi-paragraph build: %+v", res)
	}
	if len(res.AddedEffectIDs) != res.ParagraphCount {
		t.Fatalf("expected one effect per paragraph: ids=%v count=%d", res.AddedEffectIDs, res.ParagraphCount)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("strict validate failed: %v", err)
	}

	s2 := reportSlide(t, listAnimations(t, out), 2)
	if len(s2.Builds) != 1 || s2.Builds[0].Build != "p" {
		t.Fatalf("expected one build=p entry: %+v", s2.Builds)
	}
	if s2.Effects[0].StartType != "onClick" {
		t.Fatalf("first paragraph should start onClick: %+v", s2.Effects[0])
	}
	for i := 1; i < len(s2.Effects); i++ {
		if s2.Effects[i].StartType != "afterPrevious" {
			t.Fatalf("paragraph %d should be afterPrevious: %+v", i, s2.Effects[i])
		}
	}

	// --expect-paragraph-count guard: wrong count fails before write.
	out2 := filepath.Join(t.TempDir(), "bp2.pptx")
	_, err = executeRootForXLSXTest(t, "pptx", "animations", "add", deck,
		"--slide", "2", "--shape", "shape:3", "--effect", "fade", "--by-paragraph",
		"--expect-paragraph-count", "999", "--out", out2)
	if err == nil {
		t.Fatal("expected paragraph-count guard to fail")
	}
}

// TestPPTXAnimationsRemoveAndReorder authors three effects, reorders, and removes.
func TestPPTXAnimationsRemoveAndReorder(t *testing.T) {
	deck := getTestFilePath("title-content", "presentation.pptx")
	dir := t.TempDir()
	step1 := filepath.Join(dir, "s1.pptx")
	step2 := filepath.Join(dir, "s2.pptx")
	step3 := filepath.Join(dir, "s3.pptx")

	mustAdd := func(in, out, effect string) {
		args := []string{"pptx", "animations", "add", in, "--slide", "1", "--shape", "shape:2", "--effect", effect, "--out", out}
		if effect == "wipe" {
			args = append(args, "--direction", "up")
		}
		if _, err := executeRootForXLSXTest(t, args...); err != nil {
			t.Fatalf("add %s: %v", effect, err)
		}
	}
	mustAdd(deck, step1, "appear")
	mustAdd(step1, step2, "wipe")
	mustAdd(step2, step3, "fade")

	s1 := reportSlide(t, listAnimations(t, step3), 1)
	if len(s1.Effects) != 3 {
		t.Fatalf("expected 3 effects, got %d", len(s1.Effects))
	}
	clickIDs := []int{s1.Effects[0].ClickStepID, s1.Effects[1].ClickStepID, s1.Effects[2].ClickStepID}

	// Reorder to reverse.
	reordered := filepath.Join(dir, "reordered.pptx")
	orderArg := joinIntList([]int{clickIDs[2], clickIDs[1], clickIDs[0]})
	if _, err := executeRootForXLSXTest(t, "pptx", "animations", "reorder", step3,
		"--slide", "1", "--order", orderArg, "--out", reordered); err != nil {
		t.Fatalf("reorder: %v", err)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", reordered); err != nil {
		t.Fatalf("validate reorder: %v", err)
	}
	r := reportSlide(t, listAnimations(t, reordered), 1)
	if r.Effects[0].EffectKind != "fade" || r.Effects[2].EffectKind != "appear" {
		t.Fatalf("reorder did not reverse: %v", []string{r.Effects[0].EffectKind, r.Effects[1].EffectKind, r.Effects[2].EffectKind})
	}

	// Remove the middle effect by id.
	removed := filepath.Join(dir, "removed.pptx")
	removeID := r.Effects[1].EffectID
	if _, err := executeRootForXLSXTest(t, "pptx", "animations", "remove", reordered,
		"--slide", "1", "--effect-id", itoaTest(removeID), "--out", removed); err != nil {
		t.Fatalf("remove: %v", err)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", removed); err != nil {
		t.Fatalf("validate remove: %v", err)
	}
	after := reportSlide(t, listAnimations(t, removed), 1)
	if len(after.Effects) != 2 {
		t.Fatalf("expected 2 effects after remove, got %d", len(after.Effects))
	}
}

func TestPPTXAnimationsRemoveMissingEffectListsDiscovery(t *testing.T) {
	deck := getTestFilePath("animations-synthetic", "presentation.pptx")
	_, err := executeRootForXLSXTest(t, "pptx", "animations", "remove", deck,
		"--slide", "1", "--effect-id", "9999", "--dry-run")
	if err == nil {
		t.Fatal("expected missing effect to fail")
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != ExitTargetNotFound {
		t.Fatalf("expected ExitTargetNotFound, got %T %v", err, err)
	}
	msg := err.Error()
	for _, want := range []string{
		"animation effect not found: effect:9999",
		"did you mean: effect:5",
		"ooxml --json pptx animations list <file>",
	} {
		if !strings.Contains(msg, want) {
			t.Fatalf("missing %q in error: %v", want, err)
		}
	}
}

func itoaTest(n int) string {
	return joinIntList([]int{n})
}
