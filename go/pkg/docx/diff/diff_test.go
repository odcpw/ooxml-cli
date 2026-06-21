package diff

import (
	"archive/zip"
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const fixture = "../../../testdata/docx/mixed-blocks/document.docx"

func TestSemanticDiff_IdenticalDocuments(t *testing.T) {
	a := openDoc(t, fixture)
	defer a.Close()
	b := openDoc(t, fixture)
	defer b.Close()

	report, err := SemanticDiff(a, b)
	require.NoError(t, err)
	assert.Equal(t, SchemaVersion, report.SchemaVersion)
	assert.True(t, report.BlockCountEqual)
	assert.Empty(t, report.ChangedBlocks)
	assert.Empty(t, report.Blocks)
}

func TestSemanticDiff_ParagraphTextAndStyleChange(t *testing.T) {
	a := openDoc(t, fixture)
	defer a.Close()
	candidatePath := rewriteDocument(t, fixture, map[string]string{
		`<w:t>Tail paragraph</w:t>`:    `<w:t>Edited tail paragraph</w:t>`,
		`<w:pStyle w:val="Heading1"/>`: `<w:pStyle w:val="Heading2"/>`,
	})
	b := openDoc(t, candidatePath)
	defer b.Close()

	report, err := SemanticDiff(a, b)
	require.NoError(t, err)
	require.NotEmpty(t, report.Blocks)

	var textDiff, styleDiff *BlockDiff
	for i := range report.Blocks {
		d := &report.Blocks[i]
		switch d.Property {
		case "text":
			if d.After == "Edited tail paragraph" {
				textDiff = d
			}
		case "style":
			styleDiff = d
		}
	}
	require.NotNil(t, textDiff, "expected paragraph text diff in %+v", report.Blocks)
	assert.Equal(t, "Tail paragraph", textDiff.Before)
	// Reported index must match `docx text` 1-based block index (Tail paragraph is block 4).
	assert.Equal(t, 4, textDiff.Index)

	require.NotNil(t, styleDiff, "expected paragraph style diff")
	assert.Equal(t, "Heading1", styleDiff.Before)
	assert.Equal(t, "Heading2", styleDiff.After)
	assert.Equal(t, 2, styleDiff.Index)
}

func TestSemanticDiff_TableChange(t *testing.T) {
	a := openDoc(t, fixture)
	defer a.Close()
	candidatePath := rewriteDocument(t, fixture, map[string]string{
		`<w:t>Cell text</w:t>`: `<w:t>Updated cell</w:t>`,
	})
	b := openDoc(t, candidatePath)
	defer b.Close()

	report, err := SemanticDiff(a, b)
	require.NoError(t, err)

	var tableTextDiff *BlockDiff
	for i := range report.Blocks {
		d := &report.Blocks[i]
		if d.Kind == "table" && d.Property == "text" {
			tableTextDiff = d
		}
	}
	require.NotNil(t, tableTextDiff, "expected table text diff in %+v", report.Blocks)
	assert.Equal(t, "Cell text", tableTextDiff.Before)
	assert.Equal(t, "Updated cell", tableTextDiff.After)
}

func TestSemanticDiff_Deterministic(t *testing.T) {
	a := openDoc(t, fixture)
	defer a.Close()
	candidatePath := rewriteDocument(t, fixture, map[string]string{
		`<w:t>Tail paragraph</w:t>`:               `<w:t>X</w:t>`,
		`<w:t>Paragraph with section props</w:t>`: `<w:t>Y</w:t>`,
	})
	b := openDoc(t, candidatePath)
	defer b.Close()

	first, err := SemanticDiff(a, b)
	require.NoError(t, err)

	a2 := openDoc(t, fixture)
	defer a2.Close()
	b2 := openDoc(t, candidatePath)
	defer b2.Close()
	second, err := SemanticDiff(a2, b2)
	require.NoError(t, err)

	assert.Equal(t, first.Blocks, second.Blocks)
	for i := 1; i < len(first.ChangedBlocks); i++ {
		assert.Less(t, first.ChangedBlocks[i-1], first.ChangedBlocks[i], "changed blocks not sorted")
	}
}

func para(idx int, text string) model.Block {
	return model.Block{Index: idx, Kind: model.BlockKindParagraph, Text: text}
}

// TestAlignBlocks_RemovalIsLocalized proves a removed block does not cascade into
// spurious per-block "changed" reports (the bug positional comparison caused).
func TestAlignBlocks_RemovalIsLocalized(t *testing.T) {
	a := []model.Block{para(1, "intro"), para(2, "a"), para(3, "b"), para(4, "c")}
	b := []model.Block{para(1, "a"), para(2, "b"), para(3, "c")} // "intro" removed, rest re-indexed
	diffs := alignBlocks(a, b)
	if len(diffs) != 1 {
		t.Fatalf("expected exactly 1 diff (intro removed), got %d: %+v", len(diffs), diffs)
	}
	d := diffs[0]
	if d.Change != "removed" || d.Property != "presence" || d.Before != "intro" {
		t.Fatalf("expected 'intro' removed, got %+v", d)
	}
}

// TestAlignBlocks_InsertionIsLocalized is the mirror: an inserted block reports
// a single addition, not a cascade.
func TestAlignBlocks_InsertionIsLocalized(t *testing.T) {
	a := []model.Block{para(1, "a"), para(2, "b")}
	b := []model.Block{para(1, "a"), para(2, "new"), para(3, "b")}
	diffs := alignBlocks(a, b)
	if len(diffs) != 1 || diffs[0].Change != "added" || diffs[0].After != "new" {
		t.Fatalf("expected single 'new' addition, got %+v", diffs)
	}
}

// TestAlignBlocks_EditInPlace keeps in-place edits as a single 'modified' diff.
func TestAlignBlocks_EditInPlace(t *testing.T) {
	a := []model.Block{para(1, "a"), para(2, "b"), para(3, "c")}
	b := []model.Block{para(1, "a"), para(2, "B-edited"), para(3, "c")}
	diffs := alignBlocks(a, b)
	if len(diffs) != 1 || diffs[0].Change != "modified" || diffs[0].Property != "text" || diffs[0].Index != 2 {
		t.Fatalf("expected single text modification at index 2, got %+v", diffs)
	}
}

func openDoc(t *testing.T, path string) *opc.Package {
	t.Helper()
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	return pkg
}

// rewriteDocument copies a fixture DOCX and replaces literal substrings inside
// word/document.xml, returning the path to the modified copy.
func rewriteDocument(t *testing.T, src string, replacements map[string]string) string {
	t.Helper()
	reader, err := zip.OpenReader(src)
	require.NoError(t, err)
	defer reader.Close()

	dstPath := filepath.Join(t.TempDir(), "candidate.docx")
	out, err := os.Create(dstPath)
	require.NoError(t, err)
	defer out.Close()

	zw := zip.NewWriter(out)
	for _, f := range reader.File {
		rc, err := f.Open()
		require.NoError(t, err)
		data, err := io.ReadAll(rc)
		rc.Close()
		require.NoError(t, err)

		if f.Name == "word/document.xml" {
			content := string(data)
			for from, to := range replacements {
				require.Contains(t, content, from, "replacement target not found")
				content = strings.ReplaceAll(content, from, to)
			}
			data = []byte(content)
		}

		w, err := zw.CreateHeader(&zip.FileHeader{Name: f.Name, Method: zip.Deflate})
		require.NoError(t, err)
		_, err = io.Copy(w, bytes.NewReader(data))
		require.NoError(t, err)
	}
	require.NoError(t, zw.Close())
	return dstPath
}
