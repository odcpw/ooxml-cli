package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func openMediaInspectFixture(t *testing.T, name string) *opc.Package {
	t.Helper()
	pkg, err := opc.Open("../../../testdata/pptx/" + name + "/presentation.pptx")
	if err != nil {
		t.Fatalf("open fixture %s: %v", name, err)
	}
	return pkg
}

// TestReadMedia_NoMedia confirms that a deck whose pics are plain images reports
// zero clips (image pics are never reported as media).
func TestReadMedia_NoMedia(t *testing.T) {
	pkg := openMediaInspectFixture(t, "picture-placeholder")
	defer pkg.Close()

	report, err := ReadMedia(pkg)
	if err != nil {
		t.Fatalf("ReadMedia: %v", err)
	}
	for _, s := range report.Slides {
		if len(s.Clips) != 0 {
			t.Errorf("slide %d reported %d clips; expected 0 (only plain images present)", s.Slide, len(s.Clips))
		}
	}
}

// TestReadMedia_MinimalTitleEmpty confirms a no-media deck reports empty clip
// lists per slide without error.
func TestReadMedia_MinimalTitleEmpty(t *testing.T) {
	pkg := openMediaInspectFixture(t, "minimal-title")
	defer pkg.Close()

	report, err := ReadMedia(pkg)
	if err != nil {
		t.Fatalf("ReadMedia: %v", err)
	}
	if len(report.Slides) == 0 {
		t.Fatal("expected at least one slide")
	}
	for _, s := range report.Slides {
		if s.Clips == nil {
			t.Errorf("slide %d Clips should be non-nil (empty slice)", s.Slide)
		}
		if len(s.Clips) != 0 {
			t.Errorf("slide %d should have no clips", s.Slide)
		}
	}
}

// TestReadMedia_HealthyClip asserts the synthetic media fixture (slide 5: an
// embedded video with poster and click-to-play) is read correctly.
func TestReadMedia_HealthyClip(t *testing.T) {
	pkg := openMediaInspectFixture(t, "animations-synthetic")
	defer pkg.Close()

	report, err := ReadMedia(pkg)
	if err != nil {
		t.Fatalf("ReadMedia: %v", err)
	}
	var clip *MediaClip
	for i := range report.Slides {
		for j := range report.Slides[i].Clips {
			clip = &report.Slides[i].Clips[j]
		}
	}
	if clip == nil {
		t.Fatal("expected an embedded media clip in animations-synthetic")
	}
	if clip.Kind != "video" {
		t.Errorf("kind = %q, want video", clip.Kind)
	}
	if clip.MediaPartURI != "/ppt/media/media1.mp4" {
		t.Errorf("mediaPartUri = %q, want /ppt/media/media1.mp4", clip.MediaPartURI)
	}
	if clip.PosterPartURI != "/ppt/media/image1.png" {
		t.Errorf("posterPartUri = %q, want /ppt/media/image1.png", clip.PosterPartURI)
	}
	if clip.PlayTrigger == "none" {
		t.Errorf("playTrigger = none; expected click or cmd for a media pic with hlink")
	}
	if clip.Stale {
		t.Errorf("healthy clip flagged stale: %s", clip.StaleReason)
	}
}

// TestReadMedia_StaleMissingPart asserts that a media pic referencing a missing
// media part is reported as stale (the animations-stale-media fixture points its
// poster/media rels at parts that do not exist).
func TestReadMedia_StaleMissingPart(t *testing.T) {
	pkg := openMediaInspectFixture(t, "animations-stale-media")
	defer pkg.Close()

	report, err := ReadMedia(pkg)
	if err != nil {
		t.Fatalf("ReadMedia: %v", err)
	}
	foundStale := false
	for _, s := range report.Slides {
		for _, c := range s.Clips {
			if c.Stale && c.StaleReason != "" {
				foundStale = true
			}
		}
	}
	if !foundStale {
		t.Error("expected a stale media clip (dangling/missing media part) in animations-stale-media")
	}
}
