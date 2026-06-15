package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// runOOXML executes a single CLI invocation with a freshly reset root command.
// It returns the captured stdout (JSON when --format json is passed).
func runOOXML(t *testing.T, args ...string) (string, error) {
	t.Helper()
	rootCmd := newTestRootCmd(t)
	outPath := filepath.Join(t.TempDir(), "out.json")
	rootCmd.SetArgs(append(args, "-o", outPath))
	err := rootCmd.Execute()
	if err != nil {
		return "", err
	}
	data, rerr := os.ReadFile(outPath)
	require.NoError(t, rerr)
	return string(data), nil
}

// shapeHandleOnSlide returns the issued handle for the named primary selector on
// a slide, plus the slide's own sldId-backed handle.
func shapeHandleOnSlide(t *testing.T, file string, slide int, primary string) (shapeHandle string) {
	t.Helper()
	out, err := runOOXML(t, "--json", "pptx", "shapes", "show", file, "--slide", itoa(slide), "--include-text")
	require.NoError(t, err)
	var res PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	for _, s := range res.Shapes {
		if s.PrimarySelector == primary {
			require.NotEmpty(t, s.Handle, "expected a handle to be surfaced for %s", primary)
			return s.Handle
		}
	}
	t.Fatalf("primary selector %q not found on slide %d", primary, slide)
	return ""
}

func textPreviewByHandle(t *testing.T, file string, slide int, handle string) (string, bool) {
	t.Helper()
	out, err := runOOXML(t, "--json", "pptx", "shapes", "show", file, "--slide", itoa(slide), "--include-text")
	require.NoError(t, err)
	var res PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	for _, s := range res.Shapes {
		if s.Handle == handle {
			return s.TextPreview, true
		}
	}
	return "", false
}

// TestHandleSurvivesUnrelatedStructuralEdit is the headline proof: a shape
// handle issued before an UNRELATED structural edit (deleting another slide,
// which shifts every later slide number) still resolves to the SAME shape after
// the edit — even when the mutation is invoked with a DELIBERATELY WRONG --slide.
// This asserts that the handle's sldId, not --slide, is authoritative for scope.
func TestHandleSurvivesUnrelatedStructuralEdit(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	// Issue a handle for the TITLE shape on slide 2 (sldId 257) before editing.
	titleHandle := shapeHandleOnSlide(t, base, 2, "title")
	require.Equal(t, "H:pptx/s:257/shape:n:2", titleHandle)

	// UNRELATED structural edit: delete slide 1. Slide 2 (sldId 257) becomes
	// slide 1; its slide NUMBER changed but its sldId did not.
	deleted := filepath.Join(dir, "deleted.pptx")
	_, err = runOOXML(t, "pptx", "slides", "delete", base, "1", "--out", deleted)
	require.NoError(t, err)

	// Mutate via the handle with a wrong --slide (5 does not exist). If --slide
	// were authoritative this would fail; the handle's sldId 257 must win.
	edited := filepath.Join(dir, "edited.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text", deleted,
		"--target", titleHandle, "--slide", "5", "--text", "SURVIVED", "--out", edited)
	require.NoError(t, err)

	// The same handle still resolves and the text landed on the title shape.
	preview, ok := textPreviewByHandle(t, edited, 1, titleHandle)
	require.True(t, ok, "handle %s must still resolve after the structural edit", titleHandle)
	assert.Contains(t, preview, "SURVIVED")
}

// TestHandleSurvivesShapeStructuralEdit proves intra-slide structural survival:
// after deleting ANOTHER shape on the same slide, the handle for the surviving
// shape still resolves to the same shape (search-by-id, not by position).
func TestHandleSurvivesShapeStructuralEdit(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	titleHandle := shapeHandleOnSlide(t, base, 1, "title") // H:pptx/s:256/shape:n:2

	// Structural edit on the SAME slide: delete the subtitle shape (id 3).
	deleted := filepath.Join(dir, "deleted.pptx")
	_, err = runOOXML(t, "pptx", "shapes", "delete", base, "--slide", "1", "--target", "shape:3", "--out", deleted)
	require.NoError(t, err)

	// The title handle still resolves to the title after a sibling was removed.
	edited := filepath.Join(dir, "edited.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text", deleted,
		"--target", titleHandle, "--text", "STILL HERE", "--out", edited)
	require.NoError(t, err)

	preview, ok := textPreviewByHandle(t, edited, 1, titleHandle)
	require.True(t, ok)
	assert.Contains(t, preview, "STILL HERE")
}

// TestHandleSurvivesContentEdit proves a handle survives a CONTENT edit of its
// OWN target: after setting the shape's text, the same handle still resolves
// (the address is the cNvPr id attribute, not the text).
func TestHandleSurvivesContentEdit(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	titleHandle := shapeHandleOnSlide(t, base, 1, "title")

	first := filepath.Join(dir, "first.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text", base, "--target", titleHandle, "--text", "FIRST", "--out", first)
	require.NoError(t, err)

	// Re-resolve the SAME handle after its target's content changed.
	second := filepath.Join(dir, "second.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text", first, "--target", titleHandle, "--text", "SECOND", "--out", second)
	require.NoError(t, err)

	preview, ok := textPreviewByHandle(t, second, 1, titleHandle)
	require.True(t, ok)
	assert.Contains(t, preview, "SECOND")
}

// TestHandleStaleErrors asserts the typed error contract: a deleted target
// yields a clean stale-handle error (never a wrong-target hit), distinct from a
// scope-stale error and from a malformed-handle error.
func TestHandleStaleErrors(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	out := filepath.Join(dir, "x.pptx")

	// Shape gone within a live slide -> HANDLE_STALE.
	_, err = runOOXML(t, "pptx", "replace", "text", base,
		"--target", "H:pptx/s:256/shape:n:999", "--text", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeStale)
	assertCLIHandleCode(t, err, ExitTargetNotFound, pptxhandle.CodeStale)

	// Whole scope slide gone -> HANDLE_SCOPE_STALE.
	_, err = runOOXML(t, "pptx", "replace", "text", base,
		"--target", "H:pptx/s:9999/shape:n:2", "--text", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeScopeStale)
	assertCLIHandleCode(t, err, ExitTargetNotFound, pptxhandle.CodeScopeStale)

	// Bad envelope -> HANDLE_MALFORMED.
	_, err = runOOXML(t, "pptx", "replace", "text", base,
		"--target", "H:pptx/s:256/widget:n:2", "--text", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeMalformed)
	assertCLIHandleCode(t, err, ExitInvalidArgs, pptxhandle.CodeMalformed)

	// Valid handle envelope, wrong OOXML family -> HANDLE_FORMAT_MISMATCH.
	_, err = runOOXML(t, "pptx", "replace", "text", base,
		"--target", "H:xlsx/ws:1/cell:a:A1", "--text", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeFormatMismatch)
	assertCLIHandleCode(t, err, ExitInvalidArgs, pptxhandle.CodeFormatMismatch)

	// animations add: a handle whose shape id is absent on its live slide must
	// surface the SAME typed HANDLE_STALE contract as replace text / replace
	// image, not a generic "shape not found on slide" error.
	_, err = runOOXML(t, "pptx", "animations", "add", base,
		"--shape", "H:pptx/s:256/shape:n:999", "--effect", "appear", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeStale)
	assertCLIHandleCode(t, err, ExitTargetNotFound, pptxhandle.CodeStale)

	// animations add: a handle whose scope slide is gone -> HANDLE_SCOPE_STALE.
	_, err = runOOXML(t, "pptx", "animations", "add", base,
		"--shape", "H:pptx/s:9999/shape:n:2", "--effect", "appear", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeScopeStale)
	assertCLIHandleCode(t, err, ExitTargetNotFound, pptxhandle.CodeScopeStale)
}

// TestHandleAnimationSurvivesStructuralEdit proves the animations mutation also
// accepts a handle whose sldId is authoritative across an unrelated slide edit.
func TestHandleAnimationSurvivesStructuralEdit(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	titleHandle := shapeHandleOnSlide(t, base, 2, "title") // sldId 257

	deleted := filepath.Join(dir, "deleted.pptx")
	_, err = runOOXML(t, "pptx", "slides", "delete", base, "1", "--out", deleted)
	require.NoError(t, err)

	// Add an animation via the handle; --slide is omitted entirely.
	out, err := runOOXML(t, "--json", "pptx", "animations", "add", deleted,
		"--shape", titleHandle, "--effect", "appear", "--out", filepath.Join(dir, "anim.pptx"))
	require.NoError(t, err)
	var res PPTXAnimationsAddResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	assert.Equal(t, 1, res.Slide) // sldId 257 is now slide 1
	assert.Equal(t, 2, res.ShapeID)
}

// TestHandleImageReplaceSurvivesStructuralEdit proves the image-replace
// mutation accepts a handle whose sldId stays authoritative across an unrelated
// slide deletion.
func TestHandleImageReplaceSurvivesStructuralEdit(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	require.NoError(t, err)
	image, err := filepath.Abs("../../testdata/test_image.png")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	picHandle := shapeHandleOnSlide(t, base, 2, "shape:2") // picture, sldId 257
	require.Equal(t, "H:pptx/s:257/shape:n:2", picHandle)

	deleted := filepath.Join(dir, "deleted.pptx")
	_, err = runOOXML(t, "pptx", "slides", "delete", base, "1", "--out", deleted)
	require.NoError(t, err)

	// Replace the image via the handle; --slide is omitted (sldId is authority).
	out, err := runOOXML(t, "--json", "pptx", "replace", "images", deleted,
		"--target", picHandle, "--image", image, "--out", filepath.Join(dir, "img.pptx"))
	require.NoError(t, err)
	var res replaceImageResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	assert.Equal(t, 1, res.SlideNumber) // sldId 257 is now slide 1
	assert.Equal(t, 2, res.ShapeID)
}

// TestForSlidesHandleSurvivesSlideShift proves `replace text-occurrences
// --for-slides <slideHandle>` restricts the replacement to the slide named by the
// durable sldId even after an unrelated structural edit shifts that slide's
// position. This is the per-command basis for find->apply batch survival of PPTX
// text ops (find emits the slide handle into --for-slides).
func TestForSlidesHandleSurvivesSlideShift(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	// Clone slide 1 to position 2, pushing sldId 257 (slide 2) to position 3.
	shifted := filepath.Join(dir, "shifted.pptx")
	_, err = runOOXML(t, "pptx", "clone-slide", base, "--slide", "1", "--insert-after", "0", "--out", shifted)
	require.NoError(t, err)

	// Restrict to sldId 257 via the slide handle; the original slide-2 title is
	// "Content Slide". --for-slides with a positional "2" would hit the clone.
	edited := filepath.Join(dir, "edited.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text-occurrences", shifted,
		"--for-slides", "H:pptx/s:257", "--match-text", "Content Slide", "--new-text", "RESTRICTED", "--out", edited)
	require.NoError(t, err)

	// sldId 257 is now slide 3; the replacement landed there, not on the clone.
	out, err := runOOXML(t, "--json", "pptx", "shapes", "show", edited, "--slide", "3", "--include-text")
	require.NoError(t, err)
	var res PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	var slide3Title string
	for _, s := range res.Shapes {
		if s.ShapeID == 2 {
			slide3Title = s.TextPreview
		}
	}
	assert.Equal(t, "RESTRICTED", slide3Title)

	// The clone at slide 2 keeps the original slide-1 title (untouched).
	out2, err := runOOXML(t, "--json", "pptx", "shapes", "show", edited, "--slide", "2", "--include-text")
	require.NoError(t, err)
	var res2 PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(out2), &res2))
	for _, s := range res2.Shapes {
		if s.ShapeID == 2 {
			assert.NotEqual(t, "RESTRICTED", s.TextPreview)
		}
	}
}

// TestForSlidesHandleScopeStale asserts the slide-handle --for-slides path
// returns the typed HANDLE_SCOPE_STALE error when the sldId no longer exists,
// rather than silently replacing across the whole deck.
func TestForSlidesHandleScopeStale(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	base := filepath.Join(dir, "base.pptx")
	require.NoError(t, copyFileForTest(base, fixture))

	out := filepath.Join(dir, "x.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text-occurrences", base,
		"--for-slides", "H:pptx/s:9999", "--match-text", "Content", "--new-text", "X", "--out", out)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeScopeStale)
}

// copyFileForTest copies src to dst.
func copyFileForTest(dst, src string) error {
	data, err := os.ReadFile(src)
	if err != nil {
		return err
	}
	return os.WriteFile(dst, data, 0o644)
}
