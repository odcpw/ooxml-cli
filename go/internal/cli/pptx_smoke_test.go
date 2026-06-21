package cli

import (
	"bytes"
	"compress/zlib"
	"context"
	"encoding/binary"
	"encoding/json"
	"hash/crc32"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"testing"
	"time"

	"github.com/ooxml-cli/ooxml-cli/pkg/conformance"
)

// TestPPTXWorkbenchSmoke is the integration safety net over the practical PPTX
// CLI surface. It drives chained sequences of real mutations against the
// committed fixtures in testdata/pptx/, asserting that `validate --strict` stays
// clean after every step and that a JSON readback reflects each change. A
// separate opt-in OOXML_SMOKE_DECK test covers real PowerPoint-produced decks.
//
// Because the practical content is spread across several fixtures (the table
// lives on table-simple, the chart on chart-simple, notes on notes-slide) the
// suite is organized as one long compounding chain on title-content plus three
// focused per-fixture chains. The fixture-to-command-family mapping is:
//
//	title-content  -> inspect, slides show, layouts list, find, text set,
//	                  place image, animations (add appear+wipe, list, reorder,
//	                  prune-stale, remove), shapes delete, comments add/list,
//	                  media add/list
//	table-simple   -> tables show, tables set-cell
//	chart-simple   -> charts list, charts update-data
//	notes-slide    -> notes set, notes show, notes clear
//	header-footer  -> fields inspect, fields set
//
// The suite is hermetic and deterministic: all outputs go to t.TempDir(),
// committed fixtures are never mutated in place, and image/media payloads are
// synthesized in-process (no python/imagemagick dependency). A LibreOffice
// PDF render is attempted only when a binary is on PATH and skipped cleanly
// otherwise.
func TestPPTXWorkbenchSmoke(t *testing.T) {
	dir := t.TempDir()
	step := 0
	next := func() string {
		step++
		return filepath.Join(dir, "deck"+string(rune('a'+step))+".pptx")
	}

	// run executes a CLI command via a clean root command and fails the test on
	// error, returning the captured stdout (usually JSON) for readback.
	run := func(t *testing.T, args ...string) string {
		t.Helper()
		out, err := executePPTXShapesCommandErr(t, args...)
		if err != nil {
			t.Fatalf("step failed: %v\nargs=%v\noutput=%s", err, args, out)
		}
		return out
	}
	validateStrict := func(t *testing.T, path string) {
		t.Helper()
		if _, err := executePPTXShapesCommandErr(t, "validate", "--strict", path); err != nil {
			t.Fatalf("validate --strict failed for %s: %v", path, err)
		}
	}
	validate := func(t *testing.T, path string) {
		t.Helper()
		validateStrict(t, path)
		out, err := executePPTXShapesCommandErr(t, "--json", "conformance", "check", path)
		if err != nil {
			t.Fatalf("conformance check failed for %s: %v", path, err)
		}
		var report conformance.Report
		if err := json.Unmarshal([]byte(out), &report); err != nil {
			t.Fatalf("failed to parse conformance report for %s: %v\n%s", path, err, out)
		}
		if report.Status != "passed" {
			t.Fatalf("conformance check status for %s = %s, want passed\n%s", path, report.Status, out)
		}
	}
	mustContain := func(t *testing.T, haystack, needle, what string) {
		t.Helper()
		if !bytes.Contains([]byte(haystack), []byte(needle)) {
			t.Fatalf("%s: expected output to contain %q\ngot: %s", what, needle, haystack)
		}
	}
	expectConformanceFailure := func(t *testing.T, path, code string) {
		t.Helper()
		out, err := executePPTXShapesCommandErr(t, "--json", "conformance", "check", path)
		if err == nil {
			t.Fatalf("conformance check for %s unexpectedly passed\n%s", path, out)
		}
		assertPPTXShapesExitCode(t, err, ExitValidationFailed)
		mustContain(t, out, code, "conformance failure")
	}
	expectStrictValidationFailure := func(t *testing.T, path, code string) {
		t.Helper()
		out, err := executePPTXShapesCommandErr(t, "--json", "validate", "--strict", path)
		if err == nil {
			t.Fatalf("validate --strict for %s unexpectedly passed\n%s", path, out)
		}
		assertPPTXShapesExitCode(t, err, ExitValidationFailed)
		mustContain(t, out, code, "strict validation failure")
	}

	titleContent := pptxShapesFixturePath(t, "title-content")
	tableSimple := pptxShapesFixturePath(t, "table-simple")
	chartSimple := pptxShapesFixturePath(t, "chart-simple")
	notesSlide := pptxShapesFixturePath(t, "notes-slide")
	headerFooter := pptxShapesFixturePath(t, "header-footer")
	for _, f := range []string{titleContent, tableSimple, chartSimple, notesSlide, headerFooter} {
		if _, err := os.Stat(f); err != nil {
			t.Fatalf("required fixture missing: %s: %v", f, err)
		}
	}

	// ----------------------------------------------------------------------
	// Read-only sanity: inspect / slides show / layouts list / find.
	// ----------------------------------------------------------------------
	insp := run(t, "--json", "inspect", titleContent)
	mustContain(t, insp, `"type":"pptx"`, "inspect")

	run(t, "--json", "pptx", "slides", "show", titleContent, "--slide", "1", "--include-text")
	run(t, "--json", "pptx", "layouts", "show", titleContent, "--layout", "1")

	findOut := run(t, "--json", "find", "Title Content", titleContent)
	mustContain(t, findOut, `"totalHits":1`, "find")

	// ----------------------------------------------------------------------
	// Primary compounding chain on title-content. Each step writes to a fresh
	// output file derived from the previous one and is validated immediately.
	// ----------------------------------------------------------------------

	// 1. Style the title run (bold + color + font size). Readback asserts the
	//    mutation resolved the intended title shape; validate proves the write
	//    is well-formed.
	cur := next()
	setOut := run(t, "--json", "pptx", "text", "set", titleContent,
		"--slide", "1", "--target", "title",
		"--bold", "--color", "FF0000", "--font-size", "40", "--out", cur)
	mustContain(t, setOut, `"shapeId":2`, "text set readback")
	validate(t, cur)

	// 2. Place a synthesized PNG on slide 1.
	pngPath := filepath.Join(dir, "smoke.png")
	if err := os.WriteFile(pngPath, makeTinyPNG(), 0o644); err != nil {
		t.Fatalf("write png: %v", err)
	}
	prev := cur
	cur = next()
	run(t, "--json", "pptx", "place", "image", prev,
		"--slide", "1", "--image", pngPath,
		"--x", "914400", "--y", "457200", "--cx", "1828800", "--cy", "1828800",
		"--out", cur)
	validate(t, cur)
	if got := slide1ImageCount(t, run(t, "--json", "pptx", "slides", "list", cur)); got != 1 {
		t.Fatalf("expected 1 image on slide 1 after place, got %d", got)
	}

	// 3. Animations: add an "appear" on the title and a "wipe" on the subtitle.
	prev = cur
	cur = next()
	addA := run(t, "--json", "pptx", "animations", "add", prev,
		"--slide", "1", "--shape", "shape:2", "--effect", "appear", "--out", cur)
	appearID := firstEffectID(t, addA)
	validate(t, cur)

	prev = cur
	cur = next()
	addB := run(t, "--json", "pptx", "animations", "add", prev,
		"--slide", "1", "--shape", "shape:3", "--effect", "wipe", "--direction", "left", "--out", cur)
	wipeID := firstEffectID(t, addB)
	validate(t, cur)
	if appearID == wipeID {
		t.Fatalf("expected distinct effect ids, got appear=%d wipe=%d", appearID, wipeID)
	}

	// 4. animations list readback: both effects present and non-stale.
	listOut := run(t, "--json", "pptx", "animations", "list", cur)
	effects := parseSlide1Effects(t, listOut)
	if len(effects) != 2 {
		t.Fatalf("expected 2 animations after adds, got %d: %s", len(effects), listOut)
	}

	// 5. Reorder the per-click steps (wipe before appear).
	prev = cur
	cur = next()
	run(t, "--json", "pptx", "animations", "reorder", prev,
		"--slide", "1", "--order", itoa(wipeID)+","+itoa(appearID), "--out", cur)
	validate(t, cur)
	reordered := parseSlide1Effects(t, run(t, "--json", "pptx", "animations", "list", cur))
	if len(reordered) != 2 || reordered[0].EffectID != wipeID || reordered[0].SequencePos != 0 {
		t.Fatalf("reorder readback: expected wipe (id=%d) first at sequencePos 0, got %+v", wipeID, reordered)
	}

	// 6. Delete the subtitle shape so the wipe effect becomes stale, then prune.
	prev = cur
	cur = next()
	run(t, "--json", "pptx", "shapes", "delete", prev, "--slide", "1", "--target", "shape:3", "--out", cur)
	expectStrictValidationFailure(t, cur, "PPTX_STALE_ANIMATION_TARGET")
	expectConformanceFailure(t, cur, "PPTX_ANIMATION_TARGET_REFERENCE")

	staleList := run(t, "--json", "pptx", "animations", "list", cur)
	if !hasStaleEffect(t, staleList) {
		t.Fatalf("expected a stale effect after deleting its target shape: %s", staleList)
	}

	prev = cur
	cur = next()
	pruneOut := run(t, "--json", "pptx", "animations", "prune-stale", prev, "--slide", "0", "--out", cur)
	mustContain(t, pruneOut, `"prunedCount":1`, "prune-stale")
	validate(t, cur)

	// 7. Remove the remaining (appear) effect by id; assert none remain.
	prev = cur
	cur = next()
	run(t, "--json", "pptx", "animations", "remove", prev,
		"--slide", "1", "--effect-id", itoa(appearID), "--out", cur)
	validate(t, cur)
	afterRemove := run(t, "--json", "pptx", "animations", "list", cur)
	if got := len(parseSlide1Effects(t, afterRemove)); got != 0 {
		t.Fatalf("expected 0 animations after remove, got %d: %s", got, afterRemove)
	}

	// 8. Add and list a comment.
	prev = cur
	cur = next()
	run(t, "--json", "pptx", "comments", "add", prev,
		"--slide", "1", "--author", "Smoke Bot", "--text", "looks good", "--out", cur)
	validate(t, cur)
	cmList := run(t, "--json", "pptx", "comments", "list", cur)
	mustContain(t, cmList, "looks good", "comments list readback")

	// 9. Embed a synthesized audio clip, then list media.
	wavPath := filepath.Join(dir, "smoke.wav")
	if err := os.WriteFile(wavPath, makeTinyWAV(), 0o644); err != nil {
		t.Fatalf("write wav: %v", err)
	}
	prev = cur
	cur = next()
	run(t, "--json", "pptx", "media", "add", prev,
		"--slide", "1", "--file", wavPath, "--kind", "audio", "--out", cur)
	validate(t, cur)
	mediaList := run(t, "--json", "pptx", "media", "list", cur)
	mustContain(t, mediaList, `"kind":"audio"`, "media list readback")

	// Final headless render of the fully-mutated deck (gated on LibreOffice).
	renderPPTXWithLibreOfficeIfAvailable(t, cur)

	// ----------------------------------------------------------------------
	// Focused chain: tables (table-simple, slide 2, table:1 is 3x3).
	// ----------------------------------------------------------------------
	run(t, "--json", "pptx", "tables", "show", tableSimple, "--slide", "2")
	tbl := next()
	run(t, "--json", "pptx", "tables", "set-cell", tableSimple,
		"--slide", "2", "--target", "table:1", "--row", "1", "--col", "1", "--text", "SMOKE", "--out", tbl)
	validate(t, tbl)
	tblShow := run(t, "--json", "pptx", "tables", "show", tbl, "--slide", "2")
	mustContain(t, tblShow, "SMOKE", "tables set-cell readback")

	// ----------------------------------------------------------------------
	// Focused chain: charts (chart-simple, slide 1, chart:1, 3 value points).
	// ----------------------------------------------------------------------
	run(t, "--json", "pptx", "charts", "list", chartSimple)
	cht := next()
	run(t, "--json", "pptx", "charts", "update-data", chartSimple,
		"--slide", "1", "--chart", "chart:1", "--series", "1",
		"--values-json", `["200","220","240"]`, "--expect-point-count", "3", "--out", cht)
	validate(t, cht)
	chtShow := run(t, "--json", "pptx", "charts", "list", cht)
	mustContain(t, chtShow, `"240"`, "charts update-data readback")

	// ----------------------------------------------------------------------
	// Focused chain: notes (notes-slide, slide 1).
	// ----------------------------------------------------------------------
	nt := next()
	run(t, "--json", "pptx", "notes", "set", notesSlide, "--slide", "1", "--text", "Smoke note", "--out", nt)
	validate(t, nt)
	ntShow := run(t, "--json", "pptx", "notes", "show", nt, "--slide", "1")
	mustContain(t, ntShow, "Smoke note", "notes set readback")

	prevNt := nt
	nt = next()
	run(t, "--json", "pptx", "notes", "clear", prevNt, "--slide", "1", "--out", nt)
	validate(t, nt)
	clearedNotes := run(t, "--json", "pptx", "notes", "show", nt, "--slide", "1")
	mustContain(t, clearedNotes, `"plainText":""`, "notes clear readback")

	// ----------------------------------------------------------------------
	// Focused chain: fields (header-footer fixture has real footer/date
	// placeholders, so set actually rewrites placeholder text and inspect
	// reflects it; title-content has none).
	// ----------------------------------------------------------------------
	run(t, "--json", "pptx", "fields", "inspect", headerFooter)
	fld := next()
	run(t, "--json", "pptx", "fields", "set", headerFooter,
		"--footer", "Confidential Smoke", "--date-format", "datetime", "--out", fld)
	validate(t, fld)
	fldOut := run(t, "--json", "pptx", "fields", "inspect", fld)
	mustContain(t, fldOut, "Confidential Smoke", "fields inspect readback")
}

// TestPPTXSmokeRealDeckOracle is an optional read-only oracle against a real
// PowerPoint deck. Point OOXML_SMOKE_DECK at a .pptx on disk (e.g. one of the
// PBI acquisition decks) and this exercises inspect / animations list /
// media list / validate --strict against it, asserting no error. The deck is
// never read into the repo and never mutated. The test skips cleanly when the
// env var is unset or the file is absent, so CI stays green without it.
func TestPPTXSmokeRealDeckOracle(t *testing.T) {
	path := os.Getenv("OOXML_SMOKE_DECK")
	if path == "" {
		t.Skip("OOXML_SMOKE_DECK not set; skipping real-deck oracle")
	}
	if _, err := os.Stat(path); err != nil {
		t.Skipf("OOXML_SMOKE_DECK=%s not found: %v", path, err)
	}
	read := func(args ...string) {
		t.Helper()
		if out, err := executePPTXShapesCommandErr(t, args...); err != nil {
			t.Fatalf("read-only step failed: %v\nargs=%v\noutput=%s", err, args, out)
		}
	}
	read("--json", "inspect", path)
	read("--json", "pptx", "animations", "list", path)
	read("--json", "pptx", "media", "list", path)
	read("validate", "--strict", path)
	read("conformance", "check", path)
}

// renderPPTXWithLibreOfficeIfAvailable confirms a real engine opens the
// finished deck by converting it to PDF headlessly. It skips when neither
// libreoffice nor soffice is on PATH so the suite stays hermetic.
func renderPPTXWithLibreOfficeIfAvailable(t *testing.T, path string) {
	t.Helper()
	bin := ""
	for _, name := range []string{"libreoffice", "soffice"} {
		if p, err := exec.LookPath(name); err == nil {
			bin = p
			break
		}
	}
	if bin == "" {
		t.Log("libreoffice not available; skipping headless render check")
		return
	}
	baselineOut := t.TempDir()
	baselineCmd := exec.Command(bin, libreOfficeUserInstallationArg(t), "--headless", "--convert-to", "pdf", "--outdir", baselineOut, pptxShapesFixturePath(t, "minimal-title"))
	if out, err := baselineCmd.CombinedOutput(); err != nil {
		t.Logf("libreoffice cannot render known-good PPTX fixture; skipping headless render check: %v\n%s", err, out)
		return
	}
	inputPath, err := filepath.Abs(filepath.Clean(path))
	if err != nil {
		t.Fatalf("failed to normalize render input path %s: %v", path, err)
	}
	outDir := t.TempDir()
	ctx, cancel := context.WithTimeout(context.Background(), 2*time.Minute)
	defer cancel()
	cmd := exec.CommandContext(ctx, bin, libreOfficeUserInstallationArg(t), "--headless", "--convert-to", "pdf", "--outdir", outDir, inputPath)
	if out, err := cmd.CombinedOutput(); err != nil {
		if ctx.Err() == context.DeadlineExceeded {
			t.Fatalf("libreoffice timed out rendering %s after 2m\n%s", inputPath, out)
		}
		t.Fatalf("libreoffice failed to render %s: %v\n%s", inputPath, err, out)
	}
	pdfs, err := filepath.Glob(filepath.Join(outDir, "*.pdf"))
	if err != nil {
		t.Fatalf("failed to inspect libreoffice output dir %s: %v", outDir, err)
	}
	if len(pdfs) == 0 {
		t.Fatalf("libreoffice reported success but produced no PDF in %s", outDir)
	}
	for _, pdf := range pdfs {
		info, err := os.Stat(pdf)
		if err != nil {
			t.Fatalf("failed to stat rendered PDF %s: %v", pdf, err)
		}
		if info.Size() == 0 {
			t.Fatalf("libreoffice produced empty PDF %s", pdf)
		}
	}
}

// --- readback parsing helpers -------------------------------------------------

func firstEffectID(t *testing.T, jsonOut string) int {
	t.Helper()
	var res struct {
		AddedEffectIDs []int `json:"addedEffectIds"`
	}
	if err := json.Unmarshal([]byte(jsonOut), &res); err != nil {
		t.Fatalf("parse animations add output: %v\n%s", err, jsonOut)
	}
	if len(res.AddedEffectIDs) == 0 {
		t.Fatalf("animations add returned no effect ids: %s", jsonOut)
	}
	return res.AddedEffectIDs[0]
}

type smokeAnimEffect struct {
	EffectID    int  `json:"effectId"`
	SequencePos int  `json:"sequencePos"`
	Stale       bool `json:"stale"`
}

func parseAnimSlides(t *testing.T, jsonOut string) []smokeAnimEffect {
	t.Helper()
	var res struct {
		Slides []struct {
			Slide   int               `json:"slide"`
			Effects []smokeAnimEffect `json:"effects"`
		} `json:"slides"`
	}
	if err := json.Unmarshal([]byte(jsonOut), &res); err != nil {
		t.Fatalf("parse animations list output: %v\n%s", err, jsonOut)
	}
	for _, s := range res.Slides {
		if s.Slide == 1 {
			return s.Effects
		}
	}
	return nil
}

func parseSlide1Effects(t *testing.T, jsonOut string) []smokeAnimEffect {
	t.Helper()
	return parseAnimSlides(t, jsonOut)
}

func slide1ImageCount(t *testing.T, jsonOut string) int {
	t.Helper()
	var res struct {
		Slides []struct {
			Number int `json:"number"`
			Images int `json:"images"`
		} `json:"slides"`
	}
	if err := json.Unmarshal([]byte(jsonOut), &res); err != nil {
		t.Fatalf("parse slides list output: %v\n%s", err, jsonOut)
	}
	for _, s := range res.Slides {
		if s.Number == 1 {
			return s.Images
		}
	}
	return -1
}

func hasStaleEffect(t *testing.T, jsonOut string) bool {
	t.Helper()
	for _, e := range parseSlide1Effects(t, jsonOut) {
		if e.Stale {
			return true
		}
	}
	return false
}

func itoa(n int) string {
	return strconv.Itoa(n)
}

// --- synthesized binary payloads ---------------------------------------------

// makeTinyPNG returns a minimal valid 2x2 RGB PNG.
func makeTinyPNG() []byte {
	chunk := func(typ string, data []byte) []byte {
		var b bytes.Buffer
		_ = binary.Write(&b, binary.BigEndian, uint32(len(data)))
		b.WriteString(typ)
		b.Write(data)
		crc := crc32.NewIEEE()
		crc.Write([]byte(typ))
		crc.Write(data)
		_ = binary.Write(&b, binary.BigEndian, crc.Sum32())
		return b.Bytes()
	}
	var ihdr bytes.Buffer
	_ = binary.Write(&ihdr, binary.BigEndian, uint32(2)) // width
	_ = binary.Write(&ihdr, binary.BigEndian, uint32(2)) // height
	ihdr.WriteByte(8)                                    // bit depth
	ihdr.WriteByte(2)                                    // color type: truecolor
	ihdr.WriteByte(0)                                    // compression
	ihdr.WriteByte(0)                                    // filter
	ihdr.WriteByte(0)                                    // interlace

	// Two scanlines, each filter byte 0 followed by two RGB pixels (red).
	var raw bytes.Buffer
	for y := 0; y < 2; y++ {
		raw.WriteByte(0)
		for x := 0; x < 2; x++ {
			raw.Write([]byte{0xFF, 0x00, 0x00})
		}
	}
	var comp bytes.Buffer
	zw := zlib.NewWriter(&comp)
	_, _ = zw.Write(raw.Bytes())
	_ = zw.Close()

	var out bytes.Buffer
	out.Write([]byte{0x89, 'P', 'N', 'G', '\r', '\n', 0x1a, '\n'})
	out.Write(chunk("IHDR", ihdr.Bytes()))
	out.Write(chunk("IDAT", comp.Bytes()))
	out.Write(chunk("IEND", nil))
	return out.Bytes()
}

// makeTinyWAV returns a minimal valid (empty-data) PCM WAV container.
func makeTinyWAV() []byte {
	var b bytes.Buffer
	b.WriteString("RIFF")
	_ = binary.Write(&b, binary.LittleEndian, uint32(36)) // chunk size
	b.WriteString("WAVE")
	b.WriteString("fmt ")
	_ = binary.Write(&b, binary.LittleEndian, uint32(16))   // subchunk1 size
	_ = binary.Write(&b, binary.LittleEndian, uint16(1))    // PCM
	_ = binary.Write(&b, binary.LittleEndian, uint16(1))    // mono
	_ = binary.Write(&b, binary.LittleEndian, uint32(8000)) // sample rate
	_ = binary.Write(&b, binary.LittleEndian, uint32(8000)) // byte rate
	_ = binary.Write(&b, binary.LittleEndian, uint16(1))    // block align
	_ = binary.Write(&b, binary.LittleEndian, uint16(8))    // bits per sample
	b.WriteString("data")
	_ = binary.Write(&b, binary.LittleEndian, uint32(0)) // data size
	return b.Bytes()
}
