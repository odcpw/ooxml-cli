package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// forgeDuplicateSheetID builds a two-sheet workbook then forges the second
// sheet's sheetId to collide with the first (sheetId 1), which no CLI path can
// produce, so the duplicate-sheetId ambiguity contract can be exercised. It
// reuses rewriteZipPart from pptx_handles_ambiguous_test.go (same cli package).
func forgeDuplicateSheetID(t *testing.T, dir string) string {
	t.Helper()
	two := xlsxTwoSheetWorkbook(t, dir)
	dup := filepath.Join(dir, "dup-sheetid.xlsx")
	// sheetIds are randomly allocated; discover "Second"'s id and forge it to
	// collide with Sheet1's id (1).
	secondID := sheetIDByName(t, two, "Second")
	rewriteZipPart(t, dup, two, "xl/workbook.xml", `sheetId="`+secondID+`"`, `sheetId="1"`)
	return dup
}

// TestXLSXSheetsListOmitsHandleForDuplicateSheetID proves the surfacing
// contract: a workbook with two sheets sharing a sheetId never mints a sheet
// handle for the colliding id (an agent must never receive a handle that would
// mis-resolve).
func TestXLSXSheetsListOmitsHandleForDuplicateSheetID(t *testing.T) {
	dir := t.TempDir()
	dup := forgeDuplicateSheetID(t, dir)

	out, err := runOOXML(t, "--json", "xlsx", "sheets", "list", dup)
	require.NoError(t, err)
	var res XLSXSheetsListResult
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	require.NotEmpty(t, res.Sheets)
	for _, s := range res.Sheets {
		if s.SheetID == "1" {
			assert.Empty(t, s.Handle, "duplicated sheetId 1 must not surface a sheet handle (sheet %q)", s.Name)
		}
	}
}

// TestXLSXCellSetDuplicateSheetIDErrorsAmbiguousNoMutation proves the resolution
// contract: a handle naming a duplicated sheetId errors HANDLE_AMBIGUOUS and
// performs NO mutation (the output file is never written; the input is intact).
func TestXLSXCellSetDuplicateSheetIDErrorsAmbiguousNoMutation(t *testing.T) {
	dir := t.TempDir()
	dup := forgeDuplicateSheetID(t, dir)

	before, err := os.ReadFile(dup)
	require.NoError(t, err)

	outFile := filepath.Join(dir, "edited.xlsx")
	_, err = runOOXML(t, "xlsx", "cells", "set", dup,
		"--cell", "H:xlsx/ws:1/cell:a:A1", "--value", "WRONG", "--out", outFile)
	require.Error(t, err)
	assert.Contains(t, err.Error(), xlsxhandle.CodeAmbiguous)

	_, statErr := os.Stat(outFile)
	assert.True(t, os.IsNotExist(statErr), "ambiguous resolution must not write the output file")
	after, err := os.ReadFile(dup)
	require.NoError(t, err)
	assert.Equal(t, before, after, "input workbook must be untouched after an ambiguous error")
}

// TestXLSXFindOmitsHandleForDuplicateSheetID proves the find surfacing path
// (distinct from sheets list) also omits a cell handle for a non-unique sheetId.
func TestXLSXFindOmitsHandleForDuplicateSheetID(t *testing.T) {
	dir := t.TempDir()
	two := xlsxTwoSheetWorkbook(t, dir)
	// Seed a searchable value on each sheet, then forge the duplicate sheetId.
	seeded := filepath.Join(dir, "seeded.xlsx")
	_, err := runOOXML(t, "xlsx", "cells", "set", two, "--sheet", "Sheet1", "--cell", "A1", "--value", "NEEDLE", "--out", seeded)
	require.NoError(t, err)
	seeded2 := filepath.Join(dir, "seeded2.xlsx")
	_, err = runOOXML(t, "xlsx", "cells", "set", seeded, "--sheet", "Second", "--cell", "A1", "--value", "NEEDLE", "--out", seeded2)
	require.NoError(t, err)

	dup := filepath.Join(dir, "dup.xlsx")
	secondID := sheetIDByName(t, two, "Second")
	rewriteZipPart(t, dup, seeded2, "xl/workbook.xml", `sheetId="`+secondID+`"`, `sheetId="1"`)

	out, err := runOOXML(t, "--json", "find", "NEEDLE", dup)
	require.NoError(t, err)
	var res struct {
		Hits []struct {
			Handle  string `json:"handle"`
			PartURI string `json:"partUri"`
		} `json:"hits"`
	}
	require.NoError(t, json.Unmarshal([]byte(out), &res))
	require.NotEmpty(t, res.Hits, "expected find hits on the forged workbook")
	for _, h := range res.Hits {
		assert.Empty(t, h.Handle, "find must omit a cell handle for the duplicated sheetId (part %s)", h.PartURI)
	}
}

func TestXLSXNamesListAndFindOmitHandleForDuplicateWorkbookName(t *testing.T) {
	dir := t.TempDir()
	fixture, err := filepath.Abs("../../testdata/xlsx/minimal-workbook/workbook.xlsx")
	require.NoError(t, err)
	base := filepath.Join(dir, "base.xlsx")
	require.NoError(t, copyFileForTest(base, fixture))

	first := filepath.Join(dir, "first.xlsx")
	_, err = runOOXML(t, "xlsx", "names", "add", base,
		"--name", "TargetName", "--sheet", "Sheet1", "--range", "A1", "--out", first)
	require.NoError(t, err)
	second := filepath.Join(dir, "second.xlsx")
	_, err = runOOXML(t, "xlsx", "names", "add", first,
		"--name", "OtherName", "--sheet", "Sheet1", "--range", "B1", "--out", second)
	require.NoError(t, err)

	dup := filepath.Join(dir, "dup-names.xlsx")
	rewriteZipPart(t, dup, second, "xl/workbook.xml", `name="OtherName"`, `name="TargetName"`)

	out, err := runOOXML(t, "--json", "xlsx", "names", "list", dup)
	require.NoError(t, err)
	var list XLSXNamesListResult
	require.NoError(t, json.Unmarshal([]byte(out), &list))
	seen := 0
	for _, name := range list.Names {
		if name.Name == "TargetName" && name.Scope == "workbook" {
			seen++
			assert.Empty(t, name.Handle, "duplicate workbook-scoped name must not surface a handle")
		}
	}
	assert.Equal(t, 2, seen, "forged workbook should list both duplicate workbook names")

	findOut, err := runOOXML(t, "--json", "find", "TargetName", dup)
	require.NoError(t, err)
	var found struct {
		Hits []struct {
			Kind   string `json:"kind"`
			Handle string `json:"handle"`
		} `json:"hits"`
	}
	require.NoError(t, json.Unmarshal([]byte(findOut), &found))
	require.NotEmpty(t, found.Hits, "expected duplicate defined-name hits")
	nameHits := 0
	for _, hit := range found.Hits {
		if hit.Kind == "xlsx-name" {
			nameHits++
			assert.Empty(t, hit.Handle, "find must omit handle for duplicate workbook-scoped defined name")
		}
	}
	assert.Equal(t, 2, nameHits, "expected both duplicate defined-name hits")
}
