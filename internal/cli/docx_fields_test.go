package cli

import (
	"archive/zip"
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// writeDOCXWithBody writes a minimal valid DOCX whose w:body inner XML is bodyInner.
// Used to build precise field fixtures (mixed simple/complex, table-nested) for the
// fields regressions.
func writeDOCXWithBody(t *testing.T, bodyInner string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "fields.docx")
	file, err := os.Create(path)
	if err != nil {
		t.Fatalf("create docx: %v", err)
	}
	defer file.Close()
	zw := zip.NewWriter(file)
	addZipFile(t, zw, "[Content_Types].xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>`)
	addZipFile(t, zw, "_rels/.rels", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>`)
	addZipFile(t, zw, "word/document.xml", `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>`+bodyInner+`
    <w:sectPr/>
  </w:body>
</w:document>`)
	if err := zw.Close(); err != nil {
		t.Fatalf("close docx: %v", err)
	}
	return path
}

func TestDOCXFieldsListJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", documentPath)
	if err != nil {
		t.Fatalf("docx fields list failed: %v", err)
	}
	var result DOCXFieldsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list JSON: %v\n%s", err, output)
	}
	if len(result.Fields) != 2 {
		t.Fatalf("field count = %d, want 2: %+v", len(result.Fields), result.Fields)
	}
	var sawSimple, sawComplex bool
	for _, f := range result.Fields {
		switch f.FieldType {
		case "simple":
			sawSimple = true
			if f.Instruction != "PAGE" || f.CachedResult != "1" || f.Location != "body:1" {
				t.Fatalf("unexpected simple field: %+v", f)
			}
		case "complex":
			sawComplex = true
			if f.Instruction != "NUMPAGES" || f.CachedResult != "3" || f.Location != "header1:1" {
				t.Fatalf("unexpected complex field: %+v", f)
			}
		}
		if !f.IsStale {
			t.Fatalf("field should be marked stale: %+v", f)
		}
	}
	if !sawSimple || !sawComplex {
		t.Fatalf("expected both simple and complex fields: %+v", result.Fields)
	}
}

func TestDOCXFieldsListTextAndTypeFilter(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	output, err := executeRootForXLSXTest(t, "docx", "fields", "list", documentPath)
	if err != nil {
		t.Fatalf("docx fields list (text) failed: %v", err)
	}
	if !strings.Contains(output, "PAGE") || !strings.Contains(output, "NUMPAGES") {
		t.Fatalf("unexpected text output: %q", output)
	}

	filtered, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", documentPath, "--type", "PAGE")
	if err != nil {
		t.Fatalf("type filter failed: %v", err)
	}
	var result DOCXFieldsListResult
	if err := json.Unmarshal([]byte(filtered), &result); err != nil {
		t.Fatalf("unmarshal filtered JSON: %v\n%s", err, filtered)
	}
	if len(result.Fields) != 1 || result.Fields[0].Instruction != "PAGE" {
		t.Fatalf("type filter mismatch: %+v", result.Fields)
	}
}

func TestDOCXFieldsListEmpty(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", documentPath)
	if err != nil {
		t.Fatalf("docx fields list on minimal failed: %v", err)
	}
	var result DOCXFieldsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list JSON: %v\n%s", err, output)
	}
	if len(result.Fields) != 0 {
		t.Fatalf("expected no fields, got %d", len(result.Fields))
	}
}

func TestDOCXFieldsInsertCreatesFieldAndValidates(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	outPath := filepath.Join(t.TempDir(), "inserted.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "fields", "insert", documentPath,
		"--location", "body:1",
		"--field-code", "PAGE",
		"--result", "1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx fields insert failed: %v", err)
	}
	var result DOCXFieldsInsertResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal insert JSON: %v\n%s", err, output)
	}
	if result.FieldType != "simple" || result.Instruction != "PAGE" {
		t.Fatalf("unexpected insert result: %+v", result)
	}
	if !result.KnownCode || result.Warning != "" {
		t.Fatalf("PAGE should be known with no warning: %+v", result)
	}
	if result.ListCommand == "" || result.ValidateCommand == "" {
		t.Fatalf("expected follow-up commands: %+v", result)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("inserted DOCX did not validate: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXFieldsListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, readback)
	}
	if len(listing.Fields) != 1 || listing.Fields[0].Instruction != "PAGE" || listing.Fields[0].CachedResult != "1" {
		t.Fatalf("readback mismatch: %+v", listing.Fields)
	}
}

func TestDOCXFieldsInsertUnknownCodeWarns(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "fields", "insert", documentPath,
		"--location", "body:1",
		"--field-code", "STYLEREF",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx fields insert dry-run failed: %v", err)
	}
	var result DOCXFieldsInsertResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal insert JSON: %v\n%s", err, output)
	}
	if result.KnownCode {
		t.Fatalf("STYLEREF should be flagged unknown: %+v", result)
	}
	if result.Warning == "" {
		t.Fatalf("expected a warning for unknown code: %+v", result)
	}
}

func TestDOCXFieldsInsertRequiresFlags(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	if _, err := executeRootForXLSXTest(t, "docx", "fields", "insert", documentPath, "--field-code", "PAGE", "--dry-run"); err == nil {
		t.Fatalf("expected error when --location missing")
	}
	if _, err := executeRootForXLSXTest(t, "docx", "fields", "insert", documentPath, "--location", "body:1", "--dry-run"); err == nil {
		t.Fatalf("expected error when --field-code missing")
	}
	if _, err := executeRootForXLSXTest(t, "docx", "fields", "insert", documentPath, "--location", "garbage", "--field-code", "PAGE", "--dry-run"); err == nil {
		t.Fatalf("expected error on invalid location")
	}
}

func TestDOCXFieldsSetResultSimple(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	outPath := filepath.Join(t.TempDir(), "set.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "fields", "set-result", documentPath,
		"--selector", "body:1:0",
		"--result", "42",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx fields set-result failed: %v", err)
	}
	var result DOCXFieldsSetResultResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-result JSON: %v\n%s", err, output)
	}
	if result.FieldType != "simple" || result.Instruction != "PAGE" {
		t.Fatalf("unexpected set-result: %+v", result)
	}
	if result.PreviousResult != "1" || result.CachedResult != "42" {
		t.Fatalf("unexpected result values: %+v", result)
	}
	if result.Note == "" {
		t.Fatalf("expected a cache note: %+v", result)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("set-result DOCX did not validate: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXFieldsListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, readback)
	}
	for _, f := range listing.Fields {
		if f.FieldType == "simple" && f.CachedResult != "42" {
			t.Fatalf("readback simple result = %q, want 42", f.CachedResult)
		}
	}
}

func TestDOCXFieldsSetResultComplexHeader(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	outPath := filepath.Join(t.TempDir(), "set-complex.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "fields", "set-result", documentPath,
		"--selector", "header1:1:0",
		"--result", "9",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx fields set-result (complex) failed: %v", err)
	}
	var result DOCXFieldsSetResultResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-result JSON: %v\n%s", err, output)
	}
	if result.FieldType != "complex" || result.Instruction != "NUMPAGES" {
		t.Fatalf("unexpected complex set-result: %+v", result)
	}
	if result.PreviousResult != "3" || result.CachedResult != "9" {
		t.Fatalf("unexpected complex result values: %+v", result)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("complex set-result DOCX did not validate: %v", err)
	}

	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXFieldsListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, readback)
	}
	var foundComplex bool
	for _, f := range listing.Fields {
		if f.FieldType == "complex" {
			foundComplex = true
			if f.CachedResult != "9" || f.Instruction != "NUMPAGES" {
				t.Fatalf("readback complex field = %+v", f)
			}
		}
	}
	if !foundComplex {
		t.Fatalf("complex field lost: %+v", listing.Fields)
	}
}

func TestDOCXFieldsSetResultHashGuard(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	if _, err := executeRootForXLSXTest(t,
		"docx", "fields", "set-result", documentPath,
		"--selector", "body:1:0",
		"--result", "x",
		"--expect-hash", "sha256:bogus",
		"--dry-run",
	); err == nil {
		t.Fatalf("expected hash mismatch error")
	}
}

func TestDOCXFieldsSetResultSelectorRequiresFieldIndex(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	if _, err := executeRootForXLSXTest(t,
		"docx", "fields", "set-result", documentPath,
		"--selector", "body:1",
		"--result", "x",
		"--dry-run",
	); err == nil {
		t.Fatalf("expected error when selector lacks field index")
	}
}

func TestDOCXFieldsSetResultNotFound(t *testing.T) {
	documentPath := getDOCXTestFilePath("with-fields")
	if _, err := executeRootForXLSXTest(t,
		"docx", "fields", "set-result", documentPath,
		"--selector", "body:1:9",
		"--result", "x",
		"--dry-run",
	); err == nil {
		t.Fatalf("expected target-not-found error for missing field index")
	}
}

// TestDOCXFieldsListMatchesSelectorOrder is the Finding A regression: a paragraph that
// mixes a complex field BEFORE a simple field must list them in document order, and
// set-result on field index i must hit the same field list[i] reports. The old code
// emitted all simple fields first, silently mis-targeting on such documents.
func TestDOCXFieldsListMatchesSelectorOrder(t *testing.T) {
	documentPath := writeDOCXWithBody(t, `
    <w:p>
      <w:r><w:fldChar w:fldCharType="begin"/></w:r>
      <w:r><w:instrText xml:space="preserve"> NUMPAGES </w:instrText></w:r>
      <w:r><w:fldChar w:fldCharType="separate"/></w:r>
      <w:r><w:t>3</w:t></w:r>
      <w:r><w:fldChar w:fldCharType="end"/></w:r>
      <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>`)

	listOut, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", documentPath)
	if err != nil {
		t.Fatalf("fields list failed: %v", err)
	}
	var listing DOCXFieldsListResult
	if err := json.Unmarshal([]byte(listOut), &listing); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, listOut)
	}
	if len(listing.Fields) != 2 {
		t.Fatalf("field count = %d, want 2: %+v", len(listing.Fields), listing.Fields)
	}
	// Document order: complex NUMPAGES first, simple PAGE second.
	if listing.Fields[0].FieldType != "complex" || listing.Fields[0].Instruction != "NUMPAGES" {
		t.Fatalf("list[0] = %+v, want complex NUMPAGES", listing.Fields[0])
	}
	if listing.Fields[1].FieldType != "simple" || listing.Fields[1].Instruction != "PAGE" {
		t.Fatalf("list[1] = %+v, want simple PAGE", listing.Fields[1])
	}

	// set-result on each per-block field index must address the same field list reports.
	for i, want := range listing.Fields {
		out, err := executeRootForXLSXTest(t,
			"--format", "json",
			"docx", "fields", "set-result", documentPath,
			"--selector", "body:1:"+strconv.Itoa(i),
			"--result", "changed",
			"--dry-run",
		)
		if err != nil {
			t.Fatalf("set-result body:1:%d failed: %v", i, err)
		}
		var res DOCXFieldsSetResultResult
		if err := json.Unmarshal([]byte(out), &res); err != nil {
			t.Fatalf("unmarshal set-result: %v\n%s", err, out)
		}
		if res.FieldType != want.FieldType || res.Instruction != want.Instruction {
			t.Fatalf("set-result body:1:%d hit %s %q, but list[%d] is %s %q (selector/list misalignment)",
				i, res.FieldType, res.Instruction, i, want.FieldType, want.Instruction)
		}
	}
}

// TestDOCXFieldsTypeFilterMatchesSwitches is the Finding B regression: --type PAGE must
// match a field whose instruction carries switches like "PAGE \* MERGEFORMAT".
func TestDOCXFieldsTypeFilterMatchesSwitches(t *testing.T) {
	documentPath := writeDOCXWithBody(t, `
    <w:p>
      <w:fldSimple w:instr=" PAGE \* MERGEFORMAT "><w:r><w:t>1</w:t></w:r></w:fldSimple>
    </w:p>`)
	out, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", documentPath, "--type", "PAGE")
	if err != nil {
		t.Fatalf("type filter failed: %v", err)
	}
	var listing DOCXFieldsListResult
	if err := json.Unmarshal([]byte(out), &listing); err != nil {
		t.Fatalf("unmarshal filtered: %v\n%s", err, out)
	}
	if len(listing.Fields) != 1 {
		t.Fatalf("type PAGE matched %d fields, want 1: %+v", len(listing.Fields), listing.Fields)
	}
	if listing.Fields[0].Instruction != `PAGE \* MERGEFORMAT` {
		t.Fatalf("matched instruction = %q, want switch-bearing PAGE", listing.Fields[0].Instruction)
	}
}

// TestDOCXFieldsTableFieldNotEditable is the Finding C regression: a field nested in a
// body table is listed with editable=false, and set-result targeting that block fails
// cleanly instead of silently editing a different field.
func TestDOCXFieldsTableFieldNotEditable(t *testing.T) {
	documentPath := writeDOCXWithBody(t, `
    <w:tbl>
      <w:tr>
        <w:tc>
          <w:p>
            <w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple>
          </w:p>
        </w:tc>
      </w:tr>
    </w:tbl>`)

	out, err := executeRootForXLSXTest(t, "--format", "json", "docx", "fields", "list", documentPath)
	if err != nil {
		t.Fatalf("fields list failed: %v", err)
	}
	var listing DOCXFieldsListResult
	if err := json.Unmarshal([]byte(out), &listing); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, out)
	}
	if len(listing.Fields) != 1 {
		t.Fatalf("field count = %d, want 1: %+v", len(listing.Fields), listing.Fields)
	}
	if listing.Fields[0].BlockKind != "table" {
		t.Fatalf("blockKind = %q, want table", listing.Fields[0].BlockKind)
	}
	if listing.Fields[0].Editable {
		t.Fatalf("table-nested field must be reported editable=false: %+v", listing.Fields[0])
	}

	if _, err := executeRootForXLSXTest(t,
		"docx", "fields", "set-result", documentPath,
		"--selector", "body:1:0",
		"--result", "x",
		"--dry-run",
	); err == nil {
		t.Fatalf("expected a clean error when targeting a table-nested field")
	} else if !strings.Contains(err.Error(), "table") {
		t.Fatalf("error should explain table fields are not addressable, got: %v", err)
	}
}
