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

	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

const (
	xlsxFixture      = "../../testdata/xlsx/types-and-formulas/workbook.xlsx"
	docxFixture      = "../../testdata/docx/mixed-blocks/document.docx"
	validDocxFixture = "../../testdata/docx/minimal/document.docx"
)

func TestFamilyDiffCommand_XLSX(t *testing.T) {
	resetFamilyDiffFlags()
	baseline := absPath(t, xlsxFixture)
	candidate := rewriteCLIZipPart(t, baseline, "xl/worksheets/sheet1.xml", map[string]string{
		`<c r="B2"><v>1234.5</v></c>`: `<c r="B2"><v>4321</v></c>`,
	}, "candidate.xlsx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "diff", baseline, candidate)
	require.NoError(t, err)

	var result struct {
		SchemaVersion string `json:"schemaVersion"`
		Type          string `json:"type"`
		Semantic      struct {
			SchemaVersion string `json:"schemaVersion"`
			CellDiffs     []struct {
				Sheet    string `json:"sheet"`
				Cell     string `json:"cell"`
				Property string `json:"property"`
				Before   string `json:"before"`
				After    string `json:"after"`
			} `json:"cellDiffs"`
		} `json:"semantic"`
	}
	require.NoError(t, json.Unmarshal([]byte(output), &result), output)
	assert.Equal(t, "1.0", result.SchemaVersion)
	assert.Equal(t, "xlsx", result.Type)
	assert.Equal(t, "1.0", result.Semantic.SchemaVersion)

	var found bool
	for _, d := range result.Semantic.CellDiffs {
		if d.Cell == "B2" && d.Property == "value" {
			found = true
			assert.Equal(t, "1234.5", d.Before)
			assert.Equal(t, "4321", d.After)
		}
	}
	assert.True(t, found, "expected B2 value diff in %s", output)
}

func TestFamilyDiffCommand_DOCX(t *testing.T) {
	resetFamilyDiffFlags()
	baseline := absPath(t, docxFixture)
	candidate := rewriteCLIZipPart(t, baseline, "word/document.xml", map[string]string{
		`<w:t>Tail paragraph</w:t>`: `<w:t>Tail edited</w:t>`,
	}, "candidate.docx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "diff", baseline, candidate)
	require.NoError(t, err)

	var result struct {
		Type     string `json:"type"`
		Semantic struct {
			SchemaVersion string `json:"schemaVersion"`
			Blocks        []struct {
				Property string `json:"property"`
				Before   string `json:"before"`
				After    string `json:"after"`
			} `json:"blocks"`
		} `json:"semantic"`
	}
	require.NoError(t, json.Unmarshal([]byte(output), &result), output)
	assert.Equal(t, "docx", result.Type)
	assert.Equal(t, "1.0", result.Semantic.SchemaVersion)

	var found bool
	for _, b := range result.Semantic.Blocks {
		if b.Property == "text" && b.Before == "Tail paragraph" && b.After == "Tail edited" {
			found = true
		}
	}
	assert.True(t, found, "expected paragraph text diff in %s", output)
}

func TestFamilyDiffCommand_XLSXTableColumnRename(t *testing.T) {
	resetFamilyDiffFlags()
	baseline := writeTestXLSXWithTableColumns(t, "A1:B3", false, "", []string{"Region", "Amount"})
	candidate := rewriteCLIZipPart(t, baseline, "xl/tables/table1.xml", map[string]string{
		`<tableColumn id="2" name="Amount"/>`: `<tableColumn id="2" name="Revenue"/>`,
	}, "candidate.xlsx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "diff", baseline, candidate)
	require.NoError(t, err)

	var result struct {
		Type     string `json:"type"`
		Semantic struct {
			TableDiffs []struct {
				Table    string `json:"table"`
				Property string `json:"property"`
				Before   string `json:"before"`
				After    string `json:"after"`
			} `json:"tableDiffs"`
		} `json:"semantic"`
	}
	require.NoError(t, json.Unmarshal([]byte(output), &result), output)
	assert.Equal(t, "xlsx", result.Type)

	var found bool
	for _, d := range result.Semantic.TableDiffs {
		if d.Property == "columns" {
			found = true
			assert.Contains(t, d.Before, "Amount")
			assert.Contains(t, d.After, "Revenue")
		}
	}
	assert.True(t, found, "expected table columns diff in %s", output)
}

func TestFamilyDiffCommand_TypeMismatch(t *testing.T) {
	resetFamilyDiffFlags()
	_, err := executeRootForXLSXTest(t, "--format", "json", "diff", absPath(t, xlsxFixture), absPath(t, docxFixture))
	require.Error(t, err)
	cliErr, ok := AsCLIError(err)
	require.True(t, ok)
	assert.Equal(t, ExitUnsupportedType, cliErr.ExitCode)
}

func TestVerifyCommand_ValidDOCX(t *testing.T) {
	resetVerifyFlags()
	output, err := executeRootForXLSXTest(t, "--format", "json", "verify", absPath(t, validDocxFixture))
	require.NoError(t, err)

	var result VerifyResult
	require.NoError(t, json.Unmarshal([]byte(output), &result), output)
	assert.Equal(t, "1.0", result.SchemaVersion)
	assert.Equal(t, "docx", result.Type)
	assert.True(t, result.Valid)
	assert.Equal(t, "valid", result.Validation.Status)
	// Non-PPTX render is skipped, never attempted.
	assert.False(t, result.Rendered.Enabled)
	assert.Equal(t, "skipped", result.Rendered.Status)
	assert.Nil(t, result.Diff)
	assert.True(t, result.Summary.Valid)
}

func TestVerifyCommand_PPTXRenderUnavailableSkipsGracefully(t *testing.T) {
	resetVerifyFlags()
	orig := renderToPDFFn
	defer func() { renderToPDFFn = orig }()
	renderToPDFFn = func(string, string) (string, error) {
		return "", &pkgrender.MissingDependencyError{Tool: "soffice"}
	}

	output, err := executeRootForXLSXTest(t, "--format", "json", "verify", absPath(t, "../../testdata/pptx/title-content/presentation.pptx"))
	require.NoError(t, err)

	var result VerifyResult
	require.NoError(t, json.Unmarshal([]byte(output), &result), output)
	// Missing LibreOffice must not fail verify.
	assert.True(t, result.Valid)
	assert.True(t, result.Rendered.Enabled)
	assert.Equal(t, "unavailable", result.Rendered.Status)
	assert.NotEmpty(t, result.Rendered.Reason)
	assert.False(t, result.Summary.Rendered)
}

func TestVerifyCommand_WithBaselineDiff(t *testing.T) {
	resetVerifyFlags()
	baseline := absPath(t, xlsxFixture)
	candidate := rewriteCLIZipPart(t, baseline, "xl/worksheets/sheet1.xml", map[string]string{
		`<c r="B2"><v>1234.5</v></c>`: `<c r="B2"><v>7</v></c>`,
	}, "candidate.xlsx")

	output, err := executeRootForXLSXTest(t, "--format", "json", "verify", candidate, "--baseline", baseline)
	require.NoError(t, err)

	var result VerifyResult
	require.NoError(t, json.Unmarshal([]byte(output), &result), output)
	assert.True(t, result.Valid)
	require.NotNil(t, result.Diff)
	assert.Equal(t, "xlsx", result.Diff.Type)
	assert.Greater(t, result.Summary.Changes, 0)
	assert.Equal(t, baseline, result.Summary.Baseline)
}

func TestVerifyCommand_Deterministic(t *testing.T) {
	resetVerifyFlags()
	first, err := executeRootForXLSXTest(t, "--format", "json", "verify", absPath(t, validDocxFixture))
	require.NoError(t, err)
	resetVerifyFlags()
	second, err := executeRootForXLSXTest(t, "--format", "json", "verify", absPath(t, validDocxFixture))
	require.NoError(t, err)
	assert.Equal(t, first, second)
}

func resetFamilyDiffFlags() {
	familyDiffRender = false
	familyDiffThreshold = 0.01
}

func resetVerifyFlags() {
	verifyBaseline = ""
}

func absPath(t *testing.T, rel string) string {
	t.Helper()
	p, err := filepath.Abs(rel)
	require.NoError(t, err)
	return p
}

// rewriteCLIZipPart copies a fixture package and replaces literal substrings
// inside one part, returning the modified copy's path.
func rewriteCLIZipPart(t *testing.T, src, partName string, replacements map[string]string, outName string) string {
	t.Helper()
	reader, err := zip.OpenReader(src)
	require.NoError(t, err)
	defer reader.Close()

	dstPath := filepath.Join(t.TempDir(), outName)
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
