package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestDOCXHeadersListJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "headers", "list", documentPath)
	if err != nil {
		t.Fatalf("docx headers list failed: %v", err)
	}
	var result DOCXHeadersListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list JSON: %v\n%s", err, output)
	}
	if len(result.Sections) != 1 {
		t.Fatalf("section count = %d, want 1", len(result.Sections))
	}
	sec := result.Sections[0]
	if sec.Headers == nil || sec.Headers.Default == nil || sec.Headers.Default.PartURI != "/word/header1.xml" {
		t.Fatalf("unexpected header: %+v", sec.Headers)
	}
	if sec.Headers.Default.PrimarySelector != "header:1:default" ||
		!containsString(sec.Headers.Default.Selectors, "header:1:default") ||
		!containsString(sec.Headers.Default.Selectors, "id:rId10") ||
		!containsString(sec.Headers.Default.Selectors, "part:/word/header1.xml") {
		t.Fatalf("header selectors not published: %+v", sec.Headers.Default)
	}
	if sec.Footers == nil || sec.Footers.Default == nil || sec.Footers.Default.PartURI != "/word/footer1.xml" {
		t.Fatalf("unexpected footer: %+v", sec.Footers)
	}
	if sec.Footers.Default.PrimarySelector != "footer:1:default" ||
		!containsString(sec.Footers.Default.Selectors, "footer:1:default") ||
		!containsString(sec.Footers.Default.Selectors, "id:rId11") ||
		!containsString(sec.Footers.Default.Selectors, "part:/word/footer1.xml") {
		t.Fatalf("footer selectors not published: %+v", sec.Footers.Default)
	}
}

func TestDOCXHeadersListText(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	output, err := executeRootForXLSXTest(t, "docx", "headers", "list", documentPath)
	if err != nil {
		t.Fatalf("docx headers list failed: %v", err)
	}
	if !strings.Contains(output, "1 header") || !strings.Contains(output, "1 footer") || !strings.Contains(output, "1 section") {
		t.Fatalf("unexpected text output: %q", output)
	}
}

func TestDOCXHeadersShowJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "headers", "show", documentPath, "--type", "default")
	if err != nil {
		t.Fatalf("docx headers show failed: %v", err)
	}
	var result DOCXHeadersShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal show JSON: %v\n%s", err, output)
	}
	if result.Kind != "header" || result.PartURI != "/word/header1.xml" {
		t.Fatalf("unexpected show result: %+v", result)
	}
	if result.PrimarySelector != "header:1:default" || !containsString(result.Selectors, "id:rId10") {
		t.Fatalf("missing show selectors: %+v", result)
	}
	if len(result.Paragraphs) != 1 || result.Paragraphs[0].Text != "Page Header" {
		t.Fatalf("unexpected paragraphs: %+v", result.Paragraphs)
	}
	if result.Paragraphs[0].PrimarySelector != "header:1:default/p:1" ||
		!containsString(result.Paragraphs[0].Selectors, "header:1:default/paragraph:1") {
		t.Fatalf("missing paragraph selectors: %+v", result.Paragraphs[0])
	}
}

func TestDOCXHeadersShowBySelector(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "headers", "show", documentPath, "--selector", "header:1:default")
	if err != nil {
		t.Fatalf("docx headers show by selector failed: %v", err)
	}
	var result DOCXHeadersShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal show JSON: %v\n%s", err, output)
	}
	if result.PrimarySelector != "header:1:default" || result.Paragraphs[0].Text != "Page Header" {
		t.Fatalf("unexpected selector show result: %+v", result)
	}
}

func TestDOCXFootersShowByID(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "footers", "show", documentPath, "--id", "rId11")
	if err != nil {
		t.Fatalf("docx footers show failed: %v", err)
	}
	var result DOCXHeadersShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal show JSON: %v\n%s", err, output)
	}
	if result.Kind != "footer" || result.PartURI != "/word/footer1.xml" || result.Paragraphs[0].Text != "Page Footer" {
		t.Fatalf("unexpected footer show result: %+v", result)
	}
	if result.PrimarySelector != "footer:1:default" || result.Paragraphs[0].PrimarySelector != "footer:1:default/p:1" {
		t.Fatalf("missing footer selectors: %+v", result)
	}
}

func TestDOCXFootersShowBySelector(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "footers", "show", documentPath, "--selector", "footer:1:default")
	if err != nil {
		t.Fatalf("docx footers show by selector failed: %v", err)
	}
	var result DOCXHeadersShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal show JSON: %v\n%s", err, output)
	}
	if result.PrimarySelector != "footer:1:default" || result.Paragraphs[0].Text != "Page Footer" {
		t.Fatalf("unexpected selector footer result: %+v", result)
	}
}

func TestDOCXHeadersSetTextExistingReadbackValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	outPath := filepath.Join(t.TempDir(), "set-header.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "headers", "set-text", documentPath,
		"--type", "default",
		"--index", "1",
		"--text", "Modified Header",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx headers set-text failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-text JSON: %v\n%s", err, output)
	}
	if result.PreviousText != "Page Header" || result.Text != "Modified Header" {
		t.Fatalf("unexpected set-text result: %+v", result)
	}
	if result.Output != outPath || result.DryRun {
		t.Fatalf("missing mutation metadata: %+v", result)
	}
	if result.PrimarySelector != "header:1:default" || result.ParagraphPrimarySelector != "header:1:default/p:1" {
		t.Fatalf("missing mutation selectors: %+v", result)
	}
	if result.ValidateCommand == "" || result.ShowCommand == "" || result.ListCommand == "" {
		t.Fatalf("missing readback commands: %+v", result)
	}
	if result.CreatedPart || result.CreatedRef {
		t.Fatalf("should not create part/ref for existing header: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("mutated DOCX did not validate: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "headers", "show", outPath, "--type", "default")
	if err != nil {
		t.Fatalf("readback show failed: %v", err)
	}
	var shown DOCXHeadersShowResult
	if err := json.Unmarshal([]byte(readback), &shown); err != nil {
		t.Fatalf("unmarshal readback: %v\n%s", err, readback)
	}
	if shown.Paragraphs[0].Text != "Modified Header" {
		t.Fatalf("readback text = %q", shown.Paragraphs[0].Text)
	}
}

func TestDOCXHeadersSetTextBySelectorReadbackValidate(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	outPath := filepath.Join(t.TempDir(), "set-header-selector.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "headers", "set-text", documentPath,
		"--selector", "header:1:default/p:1",
		"--text", "Selector Header",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx headers set-text by selector failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-text JSON: %v\n%s", err, output)
	}
	if result.PrimarySelector != "header:1:default" || result.ParagraphIndex != 1 || result.PreviousText != "Page Header" || result.Text != "Selector Header" {
		t.Fatalf("unexpected selector set-text result: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("mutated DOCX did not validate: %v", err)
	}
}

func TestDOCXFootersSetTextBySelectorDryRun(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "footers", "set-text", documentPath,
		"--selector", "footer:1:default",
		"--index", "1",
		"--text", "Selector Footer",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx footers set-text by selector dry-run failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-text JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" || result.PrimarySelector != "footer:1:default" || result.Text != "Selector Footer" {
		t.Fatalf("unexpected footer dry-run selector result: %+v", result)
	}
	if result.ValidateCommandTemplate == "" || result.ShowCommandTemplate == "" || result.ListCommandTemplate == "" {
		t.Fatalf("missing dry-run readback command templates: %+v", result)
	}
	if !strings.Contains(result.ShowCommandTemplate, "<out.docx>") || strings.Contains(result.ShowCommandTemplate, "<out.pptx>") {
		t.Fatalf("wrong DOCX show command template: %q", result.ShowCommandTemplate)
	}
}

func TestDOCXHeadersMissingSelectorListsCandidates(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	_, err := executeRootForXLSXTest(t, "docx", "headers", "show", documentPath, "--selector", "header:99:default")
	if err == nil {
		t.Fatal("expected missing selector error")
	}
	assertCLIExitCodeForXLSXTest(t, []string{"docx", "headers", "show"}, err, ExitTargetNotFound)
	msg := err.Error()
	for _, want := range []string{"header not found: header:99:default", "did you mean:", "header:1:default", "ooxml --json docx headers list <file>"} {
		if !strings.Contains(msg, want) {
			t.Fatalf("missing %q in error: %v", want, err)
		}
	}
}

func TestDOCXHeadersSetTextParagraphOutOfRangeListsCandidates(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	_, err := executeRootForXLSXTest(t,
		"docx", "headers", "set-text", documentPath,
		"--selector", "header:1:default",
		"--index", "9",
		"--text", "Nope",
		"--dry-run",
	)
	if err == nil {
		t.Fatal("expected paragraph out-of-range error")
	}
	assertCLIExitCodeForXLSXTest(t, []string{"docx", "headers", "set-text"}, err, ExitTargetNotFound)
	msg := err.Error()
	for _, want := range []string{"header paragraph not found: header:1:default/p:9", "did you mean:", "header:1:default/p:1", "ooxml --json docx headers show <file> --selector header:1:default"} {
		if !strings.Contains(msg, want) {
			t.Fatalf("missing %q in error: %v", want, err)
		}
	}
}

func TestDOCXHeadersSelectorRejectsConflictingFlags(t *testing.T) {
	documentPath := getDOCXTestFilePath("headers")
	cases := [][]string{
		{"docx", "headers", "show", documentPath, "--selector", "header:1:default", "--type", "default"},
		{"docx", "headers", "set-text", documentPath, "--selector", "header:1:default", "--section", "1", "--text", "x", "--dry-run"},
		{"docx", "headers", "set-text", documentPath, "--selector", "header:1:default/p:1", "--index", "2", "--text", "x", "--dry-run"},
		{"docx", "footers", "show", documentPath, "--selector", "header:1:default"},
	}
	for _, args := range cases {
		_, err := executeRootForXLSXTest(t, args...)
		assertCLIExitCodeForXLSXTest(t, args, err, ExitInvalidArgs)
	}
}

func TestDOCXHeadersSetTextCreatesPart(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	outPath := filepath.Join(t.TempDir(), "create-header.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "headers", "set-text", documentPath,
		"--type", "default",
		"--index", "1",
		"--text", "Brand New Header",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx headers set-text (create) failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-text JSON: %v\n%s", err, output)
	}
	if !result.CreatedPart || !result.CreatedRef {
		t.Fatalf("expected created part and ref: %+v", result)
	}
	if result.PrimarySelector != "header:1:default" {
		t.Fatalf("created header selector = %q", result.PrimarySelector)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("created-header DOCX did not validate: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "headers", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXHeadersListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal readback list: %v\n%s", err, readback)
	}
	if listing.Sections[0].Headers.Default == nil {
		t.Fatalf("created header not listed: %+v", listing.Sections[0])
	}
}

func TestDOCXFootersSetTextAddsRefToExistingPart(t *testing.T) {
	// with-media has footer1.xml + relationship but an empty sectPr.
	documentPath := getDOCXTestFilePath("with-media")
	outPath := filepath.Join(t.TempDir(), "add-ref.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "footers", "set-text", documentPath,
		"--type", "default",
		"--index", "1",
		"--text", "Footer Wired",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx footers set-text failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-text JSON: %v\n%s", err, output)
	}
	if result.CreatedPart {
		t.Fatalf("should reuse existing footer part: %+v", result)
	}
	if !result.CreatedRef || result.PartURI != "/word/footer1.xml" {
		t.Fatalf("expected reference wired to existing part: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("mutated DOCX did not validate: %v", err)
	}
}

func TestDOCXHeadersSetTextFirstTypeCreatesDistinctReference(t *testing.T) {
	// headers fixture already has a default header; adding a "first" header must
	// create a separate part/reference and list under .First, leaving .Default intact.
	documentPath := getDOCXTestFilePath("headers")
	outPath := filepath.Join(t.TempDir(), "first-header.docx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "headers", "set-text", documentPath,
		"--type", "first",
		"--index", "1",
		"--text", "First Page Header",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("docx headers set-text (first) failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal set-text JSON: %v\n%s", err, output)
	}
	if result.Type != "first" || !result.CreatedPart || !result.CreatedRef {
		t.Fatalf("expected new first-type part: %+v", result)
	}
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("mutated DOCX did not validate: %v", err)
	}
	readback, err := executeRootForXLSXTest(t, "--format", "json", "docx", "headers", "list", outPath)
	if err != nil {
		t.Fatalf("readback list failed: %v", err)
	}
	var listing DOCXHeadersListResult
	if err := json.Unmarshal([]byte(readback), &listing); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, readback)
	}
	hdr := listing.Sections[0].Headers
	if hdr.Default == nil || hdr.Default.Type != "default" {
		t.Fatalf("default header lost: %+v", hdr.Default)
	}
	if hdr.First == nil || hdr.First.Type != "first" || hdr.First.PartURI == hdr.Default.PartURI {
		t.Fatalf("first header not distinct: %+v", hdr.First)
	}
}

func TestDOCXHeadersSetTextDryRun(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"docx", "headers", "set-text", documentPath,
		"--type", "default",
		"--index", "1",
		"--text", "X",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("docx headers set-text dry-run failed: %v", err)
	}
	var result DOCXHeadersSetTextResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if result.Text != "X" {
		t.Fatalf("unexpected dry-run result: %+v", result)
	}
}
