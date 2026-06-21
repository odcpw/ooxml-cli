package diff

import (
	"archive/zip"
	"bytes"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const fixture = "../../../testdata/xlsx/types-and-formulas/workbook.xlsx"

func TestSemanticDiff_IdenticalWorkbooks(t *testing.T) {
	a := openWorkbook(t, fixture)
	defer a.Close()
	b := openWorkbook(t, fixture)
	defer b.Close()

	report, err := SemanticDiff(a, b)
	require.NoError(t, err)
	assert.Equal(t, SchemaVersion, report.SchemaVersion)
	assert.True(t, report.SheetCountEqual)
	assert.Empty(t, report.ChangedSheets)
	assert.Empty(t, report.CellDiffs)
	assert.Empty(t, report.DefinedNameDiffs)
	assert.Empty(t, report.TableDiffs)
}

func TestSemanticDiff_CellValueChange(t *testing.T) {
	a := openWorkbook(t, fixture)
	defer a.Close()

	// Change cell B2 numeric value and the E2 formula.
	candidatePath := rewriteWorksheet(t, fixture, map[string]string{
		`<c r="B2"><v>1234.5</v></c>`:          `<c r="B2"><v>9999</v></c>`,
		`<c r="E2"><f>B2*2</f><v>2469</v></c>`: `<c r="E2"><f>B2*3</f><v>2469</v></c>`,
	})
	b := openWorkbook(t, candidatePath)
	defer b.Close()

	report, err := SemanticDiff(a, b)
	require.NoError(t, err)
	assert.Contains(t, report.ChangedSheets, "Types")

	var valueDiff, formulaDiff *CellDiff
	for i := range report.CellDiffs {
		d := &report.CellDiffs[i]
		switch {
		case d.Cell == "B2" && d.Property == "value":
			valueDiff = d
		case d.Cell == "E2" && d.Property == "formula":
			formulaDiff = d
		}
	}
	require.NotNil(t, valueDiff, "expected B2 value diff in %+v", report.CellDiffs)
	assert.Equal(t, "1234.5", valueDiff.Before)
	assert.Equal(t, "9999", valueDiff.After)

	require.NotNil(t, formulaDiff, "expected E2 formula diff")
	assert.Equal(t, "B2*2", formulaDiff.Before)
	assert.Equal(t, "B2*3", formulaDiff.After)
}

func TestSemanticDiff_DefinedNameAdded(t *testing.T) {
	a := openWorkbook(t, fixture)
	defer a.Close()

	candidatePath := rewriteZipPart(t, fixture, "xl/workbook.xml", map[string]string{
		`</sheets>`: `</sheets><definedNames><definedName name="Total">Types!$B$2</definedName></definedNames>`,
	})
	b := openWorkbook(t, candidatePath)
	defer b.Close()

	report, err := SemanticDiff(a, b)
	require.NoError(t, err)
	require.Len(t, report.DefinedNameDiffs, 1, "%+v", report.DefinedNameDiffs)
	d := report.DefinedNameDiffs[0]
	assert.Equal(t, "Total", d.Name)
	assert.Equal(t, "added", d.Change)
	assert.Equal(t, "Types!$B$2", d.After)
}

func TestSemanticDiff_Deterministic(t *testing.T) {
	a := openWorkbook(t, fixture)
	defer a.Close()
	candidatePath := rewriteWorksheet(t, fixture, map[string]string{
		`<c r="A2" t="s"><v>8</v></c>`: `<c r="A2" t="inlineStr"><is><t>South</t></is></c>`,
		`<c r="B2"><v>1234.5</v></c>`:  `<c r="B2"><v>1</v></c>`,
	})
	b := openWorkbook(t, candidatePath)
	defer b.Close()

	first, err := SemanticDiff(a, b)
	require.NoError(t, err)

	a2 := openWorkbook(t, fixture)
	defer a2.Close()
	b2 := openWorkbook(t, candidatePath)
	defer b2.Close()
	second, err := SemanticDiff(a2, b2)
	require.NoError(t, err)

	assert.Equal(t, first.CellDiffs, second.CellDiffs)
	// Cell diffs must be sorted by row then column.
	for i := 1; i < len(first.CellDiffs); i++ {
		prev := first.CellDiffs[i-1].Cell
		cur := first.CellDiffs[i].Cell
		assert.LessOrEqual(t, prev, cur, "cell diffs not in stable order: %s before %s", prev, cur)
	}
}

func openWorkbook(t *testing.T, path string) *opc.Package {
	t.Helper()
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	return pkg
}

// rewriteWorksheet copies a fixture XLSX and replaces literal substrings inside
// xl/worksheets/sheet1.xml, returning the path to the modified copy.
func rewriteWorksheet(t *testing.T, src string, replacements map[string]string) string {
	t.Helper()
	return rewriteZipPart(t, src, "xl/worksheets/sheet1.xml", replacements)
}

func rewriteZipPart(t *testing.T, src, partName string, replacements map[string]string) string {
	t.Helper()
	reader, err := zip.OpenReader(src)
	require.NoError(t, err)
	defer reader.Close()

	dstPath := filepath.Join(t.TempDir(), "candidate.xlsx")
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

		if f.Name == partName {
			content := string(data)
			for from, to := range replacements {
				require.Contains(t, content, from, "replacement target not found in %s", partName)
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
