package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
)

func TestPPTXNotesCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()
	pptx := findSubcommand(cmd, "pptx")
	if pptx == nil {
		t.Fatal("pptx command is not registered")
	}
	notes := findSubcommand(pptx, "notes")
	if notes == nil {
		t.Fatal("pptx notes command is not registered")
	}
	for _, sub := range []string{"set", "clear", "show"} {
		if findSubcommand(notes, sub) == nil {
			t.Fatalf("pptx notes %s command is not registered", sub)
		}
	}
}

func TestPPTXNotesSetCreatesPartAndReadback(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "title-content")
	outPath := filepath.Join(t.TempDir(), "notes-set.pptx")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "notes", "set", fixturePath,
		"--slide", "1",
		"--text", "First line\nSecond line",
		"--out", outPath,
	)
	var result PPTXNotesSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal notes set JSON: %v\n%s", err, output)
	}
	if result.File != fixturePath || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected file/output metadata: %+v", result)
	}
	if result.Slide != 1 || !result.CreatedPart || !result.CreatedRelationship {
		t.Fatalf("unexpected notes set result: %+v", result.SetNotesResult)
	}
	if result.NotesURI != "/ppt/notesSlides/notesSlide1.xml" {
		t.Fatalf("unexpected notes URI: %s", result.NotesURI)
	}
	if result.Text != "First line\nSecond line" {
		t.Fatalf("unexpected text: %q", result.Text)
	}

	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath, "pptx notes show")
	var report extract.NotesReport
	if err := json.Unmarshal([]byte(readback), &report); err != nil {
		t.Fatalf("failed to unmarshal notes readback JSON: %v\n%s", err, readback)
	}
	if report.Notes == nil || report.Notes.PlainText != "First line\nSecond line" {
		t.Fatalf("unexpected readback notes: %+v", report.Notes)
	}
}

func TestPPTXNotesSetDryRunDoesNotWrite(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "minimal-title")

	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "notes", "set", fixturePath,
		"--slide", "1",
		"--text", "draft notes",
		"--dry-run",
	)
	var result PPTXNotesSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal notes set dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun {
		t.Fatalf("expected dryRun=true: %+v", result)
	}
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx notes show")
}

func TestPPTXNotesClearEmptiesNotes(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "notes-slide")
	setPath := filepath.Join(t.TempDir(), "notes-cleared.pptx")

	// notes-slide slide 2 has existing notes; clear them.
	output := executePPTXShapesCommand(t,
		"--format", "json",
		"pptx", "notes", "clear", fixturePath,
		"--slide", "2",
		"--out", setPath,
	)
	var result PPTXNotesSetResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal notes clear JSON: %v\n%s", err, output)
	}
	if result.Text != "" || result.CreatedPart {
		t.Fatalf("unexpected clear result: %+v", result.SetNotesResult)
	}

	showOut := executePPTXShapesCommand(t, "--json", "pptx", "notes", "show", setPath, "--slide", "2")
	var report extract.NotesReport
	if err := json.Unmarshal([]byte(showOut), &report); err != nil {
		t.Fatalf("failed to unmarshal notes show JSON: %v\n%s", err, showOut)
	}
	if report.Notes == nil || report.Notes.PlainText != "" {
		t.Fatalf("expected empty notes after clear, got: %+v", report.Notes)
	}
}

func TestPPTXNotesShowTextFormat(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "notes-slide")
	output := executePPTXShapesCommand(t, "pptx", "notes", "show", fixturePath, "--slide", "2")
	if !strings.Contains(output, "Slide 2 notes") {
		t.Fatalf("unexpected show text output: %s", output)
	}
	if !strings.Contains(output, "speaker notes") {
		t.Fatalf("expected fixture notes text, got: %s", output)
	}
}

func TestPPTXNotesShowSlideOutOfRange(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "minimal-title")
	_, err := executePPTXShapesCommandErr(t, "pptx", "notes", "show", fixturePath, "--slide", "99")
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}

func TestPPTXNotesSetSlideOutOfRange(t *testing.T) {
	fixturePath := pptxShapesFixturePath(t, "minimal-title")
	outPath := filepath.Join(t.TempDir(), "oob.pptx")
	_, err := executePPTXShapesCommandErr(t,
		"pptx", "notes", "set", fixturePath,
		"--slide", "99", "--text", "x", "--out", outPath,
	)
	assertPPTXShapesExitCode(t, err, ExitInvalidArgs)
}
