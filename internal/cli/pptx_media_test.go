package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

// writeTempMedia writes opaque media bytes to a temp file with the given
// extension and returns its path. The bytes are not a real clip; OOXML only
// checks part existence, rel resolution, content type, and XML schema order.
func writeTempMedia(t *testing.T, ext string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "clip"+ext)
	if err := os.WriteFile(path, []byte("opaque-fake-media-bytes"), 0o644); err != nil {
		t.Fatalf("write temp media: %v", err)
	}
	return path
}

func listMedia(t *testing.T, path string) *inspect.MediaReport {
	t.Helper()
	out, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "list", path)
	if err != nil {
		t.Fatalf("media list failed: %v\n%s", err, out)
	}
	var rep inspect.MediaReport
	if err := json.Unmarshal([]byte(out), &rep); err != nil {
		t.Fatalf("unmarshal media list: %v\n%s", err, out)
	}
	return &rep
}

// TestPPTXMediaAddValidatesAndReadsBack drives the full mutation contract:
// --out write, validate-by-default, JSON readback envelope, follow-up commands,
// and `media list` confirming the embedded clip.
func TestPPTXMediaAddValidatesAndReadsBack(t *testing.T) {
	deck := getTestFilePath("minimal-title", "presentation.pptx")
	clip := writeTempMedia(t, ".mp4")
	out := filepath.Join(t.TempDir(), "withmedia.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "add", deck,
		"--slide", "1", "--file", clip, "--name", "Intro", "--out", out)
	if err != nil {
		t.Fatalf("media add failed: %v\n%s", err, output)
	}
	var res PPTXMediaAddResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal add: %v\n%s", err, output)
	}
	if res.Action != "pptx.media.add" || res.Output != out {
		t.Fatalf("unexpected envelope: %+v", res)
	}
	if res.Kind != "video" {
		t.Errorf("kind = %q, want video", res.Kind)
	}
	if res.PlayTrigger != "click" {
		t.Errorf("playTrigger = %q, want click", res.PlayTrigger)
	}
	if !res.PosterSynthesized {
		t.Error("expected synthesized poster")
	}
	if res.ValidateCommand == "" || res.ReadbackCommand == "" {
		t.Errorf("missing generated commands: %+v", res)
	}

	// Strict validate of the output.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", out); err != nil {
		t.Fatalf("strict validate after media add failed: %v", err)
	}

	// Read back via media list.
	rep := listMedia(t, out)
	found := false
	for _, s := range rep.Slides {
		for _, c := range s.Clips {
			if c.ShapeName == "Intro" && c.Kind == "video" {
				found = true
				if c.MediaPartURI == "" {
					t.Error("readback clip has empty mediaPartUri")
				}
			}
		}
	}
	if !found {
		t.Errorf("embedded clip not found in readback: %+v", rep)
	}
}

func TestPPTXMediaAddAudioAutoKind(t *testing.T) {
	deck := getTestFilePath("minimal-title", "presentation.pptx")
	clip := writeTempMedia(t, ".m4a")
	out := filepath.Join(t.TempDir(), "audio.pptx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "add", deck,
		"--slide", "1", "--file", clip, "--out", out)
	if err != nil {
		t.Fatalf("media add audio failed: %v\n%s", err, output)
	}
	var res PPTXMediaAddResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if res.Kind != "audio" {
		t.Errorf("auto-detected kind = %q, want audio", res.Kind)
	}
}

func TestPPTXMediaAddRejectsURL(t *testing.T) {
	deck := getTestFilePath("minimal-title", "presentation.pptx")
	out := filepath.Join(t.TempDir(), "x.pptx")
	_, err := executeRootForXLSXTest(t, "pptx", "media", "add", deck,
		"--slide", "1", "--file", "https://example.com/clip.mp4", "--out", out)
	if err == nil {
		t.Error("expected rejection of URL media source")
	}
}

func TestPPTXMediaAddDryRunWritesNothing(t *testing.T) {
	// Copy the deck into a temp dir so a stray write would be detectable.
	src := getTestFilePath("minimal-title", "presentation.pptx")
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read deck: %v", err)
	}
	deck := filepath.Join(t.TempDir(), "deck.pptx")
	if err := os.WriteFile(deck, data, 0o644); err != nil {
		t.Fatalf("copy deck: %v", err)
	}
	before, _ := os.Stat(deck)
	clip := writeTempMedia(t, ".mp4")

	output, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "add", deck,
		"--slide", "1", "--file", clip, "--dry-run")
	if err != nil {
		t.Fatalf("media add dry-run failed: %v\n%s", err, output)
	}
	var res PPTXMediaAddResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("unmarshal: %v\n%s", err, output)
	}
	if !res.DryRun {
		t.Error("expected dryRun=true")
	}
	after, _ := os.Stat(deck)
	if before.Size() != after.Size() || !before.ModTime().Equal(after.ModTime()) {
		t.Error("dry-run modified the input deck")
	}
}

func TestPPTXMediaReplaceRoundTrip(t *testing.T) {
	deck := getTestFilePath("minimal-title", "presentation.pptx")
	clip := writeTempMedia(t, ".mp4")
	withMedia := filepath.Join(t.TempDir(), "withmedia.pptx")

	addOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "add", deck,
		"--slide", "1", "--file", clip, "--name", "Clip", "--out", withMedia)
	if err != nil {
		t.Fatalf("media add failed: %v\n%s", err, addOut)
	}
	var addRes PPTXMediaAddResult
	_ = json.Unmarshal([]byte(addOut), &addRes)

	// Replace by shape id with a guard.
	newClip := writeTempMedia(t, ".mp4")
	replaced := filepath.Join(t.TempDir(), "replaced.pptx")
	repOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "replace", withMedia,
		"--slide", "1", "--shape", intArg(addRes.ShapeID), "--file", newClip,
		"--expect-shape-name", "Clip", "--expect-media-kind", "video", "--out", replaced)
	if err != nil {
		t.Fatalf("media replace failed: %v\n%s", err, repOut)
	}
	var repRes PPTXMediaReplaceResult
	if err := json.Unmarshal([]byte(repOut), &repRes); err != nil {
		t.Fatalf("unmarshal replace: %v\n%s", err, repOut)
	}
	if repRes.Action != "pptx.media.replace" || repRes.ShapeID != addRes.ShapeID {
		t.Fatalf("unexpected replace envelope: %+v", repRes)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", replaced); err != nil {
		t.Fatalf("strict validate after replace failed: %v", err)
	}
}

func TestPPTXMediaReplaceGuardFails(t *testing.T) {
	deck := getTestFilePath("minimal-title", "presentation.pptx")
	clip := writeTempMedia(t, ".mp4")
	withMedia := filepath.Join(t.TempDir(), "withmedia.pptx")
	addOut, err := executeRootForXLSXTest(t, "--format", "json", "pptx", "media", "add", deck,
		"--slide", "1", "--file", clip, "--name", "Clip", "--out", withMedia)
	if err != nil {
		t.Fatalf("media add failed: %v\n%s", err, addOut)
	}
	var addRes PPTXMediaAddResult
	_ = json.Unmarshal([]byte(addOut), &addRes)

	newClip := writeTempMedia(t, ".mp4")
	_, err = executeRootForXLSXTest(t, "pptx", "media", "replace", withMedia,
		"--slide", "1", "--shape", intArg(addRes.ShapeID), "--file", newClip,
		"--expect-shape-name", "WrongName", "--out", filepath.Join(t.TempDir(), "o.pptx"))
	if err == nil {
		t.Error("expected shape-name guard failure")
	}
}

func TestPPTXMediaListNoMedia(t *testing.T) {
	deck := getTestFilePath("minimal-title", "presentation.pptx")
	rep := listMedia(t, deck)
	for _, s := range rep.Slides {
		if len(s.Clips) != 0 {
			t.Errorf("slide %d unexpectedly reported clips", s.Slide)
		}
	}
}

func intArg(n int) string {
	return strconv.Itoa(n)
}
