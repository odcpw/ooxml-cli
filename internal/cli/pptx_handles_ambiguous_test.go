package cli

import (
	"archive/zip"
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	pptxhandle "github.com/ooxml-cli/ooxml-cli/pkg/pptx/handle"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// rewriteZipPart copies the src .pptx to dst, replacing the body of the named
// part by applying a literal string substitution. It is used to FORGE the
// malformed decks (duplicate cNvPr id, duplicate sldId) that no CLI path can
// produce, so the ambiguity contract can be exercised.
func rewriteZipPart(t *testing.T, dst, src, partName, oldStr, newStr string) {
	t.Helper()
	zr, err := zip.OpenReader(src)
	require.NoError(t, err)
	defer zr.Close()

	var buf bytes.Buffer
	zw := zip.NewWriter(&buf)
	replaced := false
	for _, f := range zr.File {
		rc, err := f.Open()
		require.NoError(t, err)
		body, err := io.ReadAll(rc)
		rc.Close()
		require.NoError(t, err)

		if f.Name == partName {
			updated := strings.Replace(string(body), oldStr, newStr, 1)
			require.NotEqual(t, string(body), updated, "substitution in %s changed nothing (oldStr not found)", partName)
			body = []byte(updated)
			replaced = true
		}

		w, err := zw.CreateHeader(&zip.FileHeader{Name: f.Name, Method: zip.Deflate})
		require.NoError(t, err)
		_, err = w.Write(body)
		require.NoError(t, err)
	}
	require.True(t, replaced, "part %s not found in %s", partName, src)
	require.NoError(t, zw.Close())
	require.NoError(t, os.WriteFile(dst, buf.Bytes(), 0o644))
}

// TestShapeShowOmitsHandleForDuplicateShapeID proves the surfacing contract: a
// slide carrying two shapes with the SAME cNvPr id never mints a handle for the
// colliding id (an agent must never receive a handle that would mis-resolve).
func TestShapeShowOmitsHandleForDuplicateShapeID(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-shape.pptx")
	// Force subtitle (cNvPr id 3) to share the title's id 2 on slide 1.
	rewriteZipPart(t, dup, fixture, "ppt/slides/slide1.xml",
		`cNvPr id="3" name="Subtitle 2"`, `cNvPr id="2" name="Subtitle 2"`)

	out, err := runOOXML(t, "--json", "pptx", "shapes", "show", dup, "--slide", "1", "--include-text")
	require.NoError(t, err)
	var res PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))

	// Both shapes that share id 2 must omit the handle; no positional handle is
	// minted. id 1 (the spTree group) is filtered out of the targets anyway.
	collidingSeen := 0
	for _, s := range res.Shapes {
		if s.ShapeID == 2 {
			collidingSeen++
			assert.Empty(t, s.Handle, "ambiguous cNvPr id 2 must not surface a handle (shape %q)", s.ShapeName)
		}
	}
	assert.Equal(t, 2, collidingSeen, "expected both shapes sharing cNvPr id 2 to be listed")
}

// TestReplaceTextDuplicateShapeIDErrorsAmbiguousNoMutation proves the resolution
// contract: a handle naming a duplicated cNvPr id errors HANDLE_AMBIGUOUS and
// performs NO mutation (the output file is never written; the input is intact).
func TestReplaceTextDuplicateShapeIDErrorsAmbiguousNoMutation(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-shape.pptx")
	rewriteZipPart(t, dup, fixture, "ppt/slides/slide1.xml",
		`cNvPr id="3" name="Subtitle 2"`, `cNvPr id="2" name="Subtitle 2"`)

	before, err := os.ReadFile(dup)
	require.NoError(t, err)

	outFile := filepath.Join(dir, "edited.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text", dup,
		"--target", "H:pptx/s:256/shape:n:2", "--text", "WRONG", "--out", outFile)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeAmbiguous)

	// No mutation: the output file must not exist and the input is byte-identical.
	_, statErr := os.Stat(outFile)
	assert.True(t, os.IsNotExist(statErr), "ambiguous resolution must not write the output file")
	after, err := os.ReadFile(dup)
	require.NoError(t, err)
	assert.Equal(t, before, after, "input deck must be untouched after an ambiguous error")
}

// TestReplaceTextDuplicateSldIDErrorsAmbiguous proves the slide-scope contract:
// a handle whose sldId is shared by more than one slide errors HANDLE_AMBIGUOUS
// rather than silently resolving to the first matching slide.
func TestReplaceTextDuplicateSldIDErrorsAmbiguous(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-sldid.pptx")
	// Force the second slide's sldId 257 to collide with the first's 256.
	rewriteZipPart(t, dup, fixture, "ppt/presentation.xml",
		`sldId id="257"`, `sldId id="256"`)

	outFile := filepath.Join(dir, "edited.pptx")
	_, err = runOOXML(t, "pptx", "replace", "text", dup,
		"--target", "H:pptx/s:256/shape:n:2", "--text", "WRONG", "--out", outFile)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeAmbiguous)

	_, statErr := os.Stat(outFile)
	assert.True(t, os.IsNotExist(statErr), "ambiguous slide scope must not write the output file")
}

// TestAnimationsAddDuplicateSldIDErrorsAmbiguous proves the animations-add path
// also routes slide-scope resolution through the shared ambiguity check.
func TestAnimationsAddDuplicateSldIDErrorsAmbiguous(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-sldid.pptx")
	rewriteZipPart(t, dup, fixture, "ppt/presentation.xml",
		`sldId id="257"`, `sldId id="256"`)

	_, err = runOOXML(t, "--json", "pptx", "animations", "add", dup,
		"--shape", "H:pptx/s:256/shape:n:2", "--effect", "appear",
		"--out", filepath.Join(dir, "anim.pptx"))
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeAmbiguous)
}

// TestAnimationsAddDuplicateShapeIDErrorsAmbiguousNoMutation proves animation
// handle targets also enforce duplicate cNvPr id ambiguity before mutation.
func TestAnimationsAddDuplicateShapeIDErrorsAmbiguousNoMutation(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-shape.pptx")
	rewriteZipPart(t, dup, fixture, "ppt/slides/slide1.xml",
		`cNvPr id="3" name="Subtitle 2"`, `cNvPr id="2" name="Subtitle 2"`)

	before, err := os.ReadFile(dup)
	require.NoError(t, err)

	outFile := filepath.Join(dir, "anim.pptx")
	_, err = runOOXML(t, "--json", "pptx", "animations", "add", dup,
		"--shape", "H:pptx/s:256/shape:n:2", "--effect", "appear",
		"--out", outFile)
	require.Error(t, err)
	assert.Contains(t, err.Error(), pptxhandle.CodeAmbiguous)

	_, statErr := os.Stat(outFile)
	assert.True(t, os.IsNotExist(statErr), "ambiguous animation target must not write the output file")
	after, err := os.ReadFile(dup)
	require.NoError(t, err)
	assert.Equal(t, before, after, "input deck must be untouched after an ambiguous animation target")
}

// TestSlidesListOmitsHandleForDuplicateSldID proves the slide-surfacing contract:
// a duplicated sldId never mints a slide handle.
func TestSlidesListOmitsHandleForDuplicateSldID(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-sldid.pptx")
	rewriteZipPart(t, dup, fixture, "ppt/presentation.xml",
		`sldId id="257"`, `sldId id="256"`)

	out, err := runOOXML(t, "--json", "pptx", "slides", "list", dup)
	require.NoError(t, err)
	var res struct {
		Slides []SlidesListItem `json:"slides"`
	}
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	require.NotEmpty(t, res.Slides)
	for _, s := range res.Slides {
		if s.SlideID == 256 {
			assert.Empty(t, s.Handle, "duplicated sldId 256 must not surface a slide handle")
		}
	}
}

// TestShapeShowOmitsHandleForDuplicateSldID proves a shape handle is not minted
// when its slide scope has a duplicated p:sldId@id. The shape itself may have a
// unique cNvPr id, but the full handle would still resolve ambiguously.
func TestShapeShowOmitsHandleForDuplicateSldID(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-sldid.pptx")
	rewriteZipPart(t, dup, fixture, "ppt/presentation.xml",
		`sldId id="257"`, `sldId id="256"`)

	out, err := runOOXML(t, "--json", "pptx", "shapes", "show", dup, "--slide", "1", "--include-text")
	require.NoError(t, err)
	var res PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	require.NotEmpty(t, res.Shapes)
	for _, s := range res.Shapes {
		assert.Empty(t, s.Handle, "duplicated sldId 256 must not surface a shape handle (shape %q)", s.ShapeName)
	}
}

// TestFindOmitsHandleForDuplicateSldID proves the find surfacing path (distinct
// from slides list) also omits a slide handle for a non-unique sldId.
func TestFindOmitsHandleForDuplicateSldID(t *testing.T) {
	fixture, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	dir := t.TempDir()
	dup := filepath.Join(dir, "dup-sldid.pptx")
	rewriteZipPart(t, dup, fixture, "ppt/presentation.xml",
		`sldId id="257"`, `sldId id="256"`)

	// Search for a token present on both slides' title text ("Title").
	out, err := runOOXML(t, "--json", "find", "Title", dup)
	require.NoError(t, err)
	var res struct {
		Hits []struct {
			Handle  string `json:"handle"`
			PartURI string `json:"partUri"`
		} `json:"hits"`
	}
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	require.NotEmpty(t, res.Hits, "expected find hits on the forged deck")
	for _, h := range res.Hits {
		assert.Empty(t, h.Handle, "find must omit slide handle for duplicated sldId (part %s)", h.PartURI)
	}
}
