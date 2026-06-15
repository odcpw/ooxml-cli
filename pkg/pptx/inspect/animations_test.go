package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func openAnimFixture(t *testing.T, name string) opc.PackageSession {
	t.Helper()
	pkg, err := opc.Open("../../../testdata/pptx/" + name + "/presentation.pptx")
	require.NoError(t, err)
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

func readAnim(t *testing.T, name string) *AnimationsReport {
	t.Helper()
	rep, err := ReadAnimations(openAnimFixture(t, name))
	require.NoError(t, err)
	return rep
}

// slide returns the AnimationsSlideInfo for the given 1-based slide number.
func slide(t *testing.T, rep *AnimationsReport, n int) AnimationsSlideInfo {
	t.Helper()
	for _, s := range rep.Slides {
		if s.Slide == n {
			return s
		}
	}
	t.Fatalf("slide %d not found in report", n)
	return AnimationsSlideInfo{}
}

func TestReadAnimations_NoTimingFixtures(t *testing.T) {
	// Existing decks have no p:timing: every slide reports HasTiming=false with
	// empty effects and no error.
	for _, name := range []string{"minimal-title", "title-content"} {
		rep := readAnim(t, name)
		require.NotEmpty(t, rep.Slides, "%s should have slides", name)
		for _, s := range rep.Slides {
			assert.False(t, s.HasTiming, "%s slide %d should have no timing", name, s.Slide)
			assert.Empty(t, s.Effects, "%s slide %d should have no effects", name, s.Slide)
			assert.Zero(t, s.UnsupportedCount)
		}
	}
}

func TestReadAnimations_FourEntranceKinds(t *testing.T) {
	rep := readAnim(t, "animations-synthetic")
	s := slide(t, rep, 1)

	require.True(t, s.HasTiming)
	require.Len(t, s.Effects, 4, "appear/fade/wipe/flyIn collapse to one record each")
	assert.Zero(t, s.UnsupportedCount)

	// Ordered by mainSeq document order; each is supported; sequencePos 0..3.
	want := []struct {
		kind      string
		spid      int
		shapeName string
		filter    string
	}{
		{"appear", 2, "AppearShape", ""},
		{"fade", 3, "FadeShape", "fade"},
		{"wipe", 4, "WipeShape", "wipe(up)"},
		{"flyIn", 5, "FlyInShape", ""},
	}
	for i, w := range want {
		e := s.Effects[i]
		assert.Equal(t, i, e.SequencePos, "effect %d sequencePos", i)
		assert.Equal(t, w.kind, e.EffectKind, "effect %d kind", i)
		assert.True(t, e.Supported, "effect %d supported", i)
		assert.Equal(t, w.spid, e.Spid, "effect %d spid", i)
		assert.Equal(t, w.shapeName, e.ShapeName, "effect %d shapeName", i)
		assert.Equal(t, w.filter, e.Filter, "effect %d filter", i)
		assert.False(t, e.Stale, "effect %d not stale", i)
		assert.Equal(t, "entr", e.PresetClass)
	}

	// First effect is onClick (clickEffect); the rest play afterPrevious.
	assert.Equal(t, "onClick", s.Effects[0].StartType)
	assert.Equal(t, "afterPrevious", s.Effects[1].StartType)
	assert.Equal(t, "effect:5", s.Effects[0].PrimarySelector)
	assert.Contains(t, s.Effects[0].Selectors, "effect:5")
	assert.Contains(t, s.Effects[0].Selectors, "5")
	assert.Contains(t, s.Effects[0].Selectors, "clickStep:3")
}

// TestReadAnimations_ClassifierIgnoresPresetID confirms the classifier keys off
// (presetClass, behavior, filter) and NOT presetID: the wipe carries an
// arbitrary presetID/presetSubtype yet is still classified by its filter.
func TestReadAnimations_ClassifierIgnoresPresetID(t *testing.T) {
	rep := readAnim(t, "animations-synthetic")
	s := slide(t, rep, 1)
	wipe := s.Effects[2]
	assert.Equal(t, "wipe", wipe.EffectKind)
	// presetID is surfaced advisory-only, not interpreted.
	assert.Equal(t, "22", wipe.PresetID)
	assert.Equal(t, "8", wipe.PresetSubtype)
}

func TestReadAnimations_ParagraphBuild(t *testing.T) {
	rep := readAnim(t, "animations-synthetic")
	s := slide(t, rep, 2)

	require.Len(t, s.Effects, 3, "one per-paragraph effect per a:p")
	for i, e := range s.Effects {
		require.NotNil(t, e.ParagraphRange, "effect %d should carry a pRg", i)
		assert.Equal(t, i, e.ParagraphRange.Start)
		assert.Equal(t, i, e.ParagraphRange.End)
		assert.Equal(t, 2, e.Spid)
		assert.False(t, e.Stale)
	}

	require.Len(t, s.Builds, 1)
	b := s.Builds[0]
	assert.Equal(t, 2, b.Spid)
	assert.Equal(t, "BodyList", b.ShapeName)
	assert.Equal(t, "byParagraph", b.Build) // raw token surfaced verbatim
	assert.Equal(t, "0", b.GrpID)
	assert.False(t, b.Stale)
}

// TestReadAnimations_UnsupportedPreserved confirms an out-of-scope motion-path
// effect is reported (never dropped) with an unsupported: prefix and counted.
func TestReadAnimations_UnsupportedPreserved(t *testing.T) {
	rep := readAnim(t, "animations-synthetic")
	s := slide(t, rep, 3)

	require.Len(t, s.Effects, 1)
	e := s.Effects[0]
	assert.False(t, e.Supported)
	assert.Contains(t, e.EffectKind, "unsupported:")
	assert.Equal(t, "unsupported:path/animMotion", e.EffectKind)
	assert.Equal(t, "path", e.PresetClass)
	assert.Equal(t, 1, s.UnsupportedCount)
	// Still resolves its (present) target shape.
	assert.Equal(t, "MotionShape", e.ShapeName)
	assert.False(t, e.Stale)
}

func TestReadAnimations_StaleTargets(t *testing.T) {
	rep := readAnim(t, "animations-synthetic")
	s := slide(t, rep, 4)

	require.Len(t, s.Effects, 2)

	missing := s.Effects[0]
	assert.True(t, missing.Stale)
	assert.Equal(t, "missing-shape", missing.StaleReason)
	assert.Equal(t, 99, missing.Spid)
	assert.Empty(t, missing.ShapeName, "stale target resolves to no name")

	outOfRange := s.Effects[1]
	assert.True(t, outOfRange.Stale)
	assert.Equal(t, "pRg-out-of-range:3-5/1", outOfRange.StaleReason)
	assert.Equal(t, 2, outOfRange.Spid)
	assert.Equal(t, "SmallText", outOfRange.ShapeName)
}

func TestReadAnimations_Media(t *testing.T) {
	rep := readAnim(t, "animations-synthetic")
	s := slide(t, rep, 5)

	require.Len(t, s.Media, 1)
	m := s.Media[0]
	assert.Equal(t, "video", m.Kind)
	assert.Equal(t, 2, m.Spid)
	assert.Equal(t, "Clip.mp4", m.ShapeName)
	assert.Equal(t, "/ppt/media/media1.mp4", m.MediaPartURI)
	assert.Equal(t, "/ppt/media/image1.png", m.PosterPartURI)
	assert.True(t, m.HasClickToPlay)
	assert.False(t, m.Stale)

	// The click-to-play p:cmd is NOT a supported entrance: it is reported as an
	// unsupported effect (its media-trigger status lives on MediaInfo).
	require.Len(t, s.Effects, 1)
	assert.False(t, s.Effects[0].Supported)
	assert.Equal(t, "mediacall", s.Effects[0].PresetClass)
	assert.Equal(t, 1, s.UnsupportedCount)
}

// TestReadAnimations_StaleMedia exercises dangling-rel and missing-part media
// detection on a slide that carries a media p:pic but no p:timing.
func TestReadAnimations_StaleMedia(t *testing.T) {
	rep := readAnim(t, "animations-stale-media")
	s := slide(t, rep, 1)

	assert.False(t, s.HasTiming, "media is reported even without a timing tree")
	require.Len(t, s.Media, 1)
	m := s.Media[0]
	assert.Equal(t, "video", m.Kind)
	assert.True(t, m.Stale)
	// The video r:link rId3 is undeclared -> dangling-rel wins over the poster's
	// missing-part.
	assert.Equal(t, "dangling-rel:rId3", m.StaleReason)
	assert.False(t, m.HasClickToPlay)
}
