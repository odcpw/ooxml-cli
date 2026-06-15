package mutate

import (
	"errors"
	"testing"

	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

func TestFindReplaceBasicLiteral(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	pattern, _ := BuildFindReplacePattern("text", false, false, false)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "copy",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	// "Heading Text" -> "Heading copy" (case-insensitive matches "Text"),
	// "Body text" -> "Body copy".
	if result.TotalReplacements != 2 || result.AffectedBlockCount != 2 {
		t.Fatalf("unexpected result: %+v", result)
	}
	extracted := extractText(t, pkg, documentURI)
	if extracted.Blocks[0].Text != "Heading copy" || extracted.Blocks[1].Text != "Body copy" {
		t.Fatalf("readback = %+v", extracted.Blocks)
	}
}

func TestFindReplaceCaseSensitive(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	pattern, _ := BuildFindReplacePattern("text", false, true, false)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "copy",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	// Only lowercase "text" in "Body text" matches.
	if result.TotalReplacements != 1 {
		t.Fatalf("case-sensitive count = %d, want 1", result.TotalReplacements)
	}
	extracted := extractText(t, pkg, documentURI)
	if extracted.Blocks[0].Text != "Heading Text" || extracted.Blocks[1].Text != "Body copy" {
		t.Fatalf("readback = %+v", extracted.Blocks)
	}
}

func TestFindReplaceMultipleMatchesInSingleRun(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	// "Hello world" with case-sensitive "o" -> "0" must replace both occurrences.
	pattern, _ := BuildFindReplacePattern("o", false, true, false)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "0",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	if result.TotalReplacements != 2 {
		t.Fatalf("count = %d, want 2", result.TotalReplacements)
	}
	extracted := extractText(t, pkg, documentURI)
	if got := extracted.Blocks[0].Text; got != "Hell0 w0rld" {
		t.Fatalf("text = %q, want %q", got, "Hell0 w0rld")
	}
}

func TestFindReplaceWholeWord(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	// "head" as whole word should not match inside "Heading".
	pattern, _ := BuildFindReplacePattern("head", false, false, true)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "X",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	if result.TotalReplacements != 0 {
		t.Fatalf("whole-word count = %d, want 0", result.TotalReplacements)
	}
}

func TestFindReplaceRegex(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	pattern, _ := BuildFindReplacePattern("[A-Z]ello", true, true, false)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "Howdy",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	if result.TotalReplacements != 1 {
		t.Fatalf("regex count = %d, want 1", result.TotalReplacements)
	}
	extracted := extractText(t, pkg, documentURI)
	if extracted.Blocks[0].Text != "Howdy world" {
		t.Fatalf("text = %q", extracted.Blocks[0].Text)
	}
}

func TestFindReplaceAcrossRuns(t *testing.T) {
	pkg, documentURI := openFixture(t, "split-runs")
	defer pkg.Close()

	// "hello" is split as "hel"+"lo" across runs in paragraph 1.
	pattern, _ := BuildFindReplacePattern("hello", false, false, false)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "hi",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	if result.TotalReplacements != 2 {
		t.Fatalf("count = %d, want 2", result.TotalReplacements)
	}
	extracted := extractText(t, pkg, documentURI)
	if extracted.Blocks[0].Text != "hi world" || extracted.Blocks[1].Text != "say hi again" {
		t.Fatalf("readback = %+v", extracted.Blocks)
	}
}

func TestFindReplaceTableCells(t *testing.T) {
	pkg, documentURI := openFixture(t, "table")
	defer pkg.Close()

	pattern, _ := BuildFindReplacePattern("A", false, true, false)
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "X",
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	if result.TotalReplacements != 2 || result.AffectedBlockCount != 1 {
		t.Fatalf("unexpected table replace result: %+v", result)
	}
	if len(result.AffectedBlockIndices) != 1 || result.AffectedBlockIndices[0] != 1 {
		t.Fatalf("affected blocks = %+v, want [1]", result.AffectedBlockIndices)
	}
	if len(result.BlockSummaries) != 2 {
		t.Fatalf("summary count = %d, want 2: %+v", len(result.BlockSummaries), result.BlockSummaries)
	}
	first := result.BlockSummaries[0]
	if first.Kind != "tableCell" || first.TableIndex != 1 || first.RowIndex != 1 || first.ColumnIndex != 1 || first.ParagraphIndex != 1 {
		t.Fatalf("unexpected first table summary: %+v", first)
	}
	extracted := extractText(t, pkg, documentURI)
	if extracted.Blocks[0].Text != "X1\tB1\nX2\tB2" {
		t.Fatalf("table readback = %+v", extracted.Blocks[0])
	}
}

func TestFindReplacePreservesRunProperties(t *testing.T) {
	pkg, documentURI := openFixture(t, "split-runs")
	defer pkg.Close()

	// Paragraph 2: "say " + bold "hello" + " again". Replacing "hello" must keep
	// the bold run's formatting on the replacement text.
	pattern, _ := BuildFindReplacePattern("hello", true, true, false)
	if _, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "hi",
	}); err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}

	doc, err := pkg.ReadXMLPart(documentURI)
	if err != nil {
		t.Fatalf("ReadXMLPart returned error: %v", err)
	}
	bodyElem, err := docxbody.FindBody(doc.Root())
	if err != nil {
		t.Fatalf("FindBody returned error: %v", err)
	}
	paragraphs := namespaces.FindChildren(bodyElem, namespaces.NsW, "p")
	boldRunFound := false
	for _, run := range namespaces.FindChildren(paragraphs[1], namespaces.NsW, "r") {
		rPr := namespaces.FindChild(run, namespaces.NsW, "rPr")
		if rPr != nil && namespaces.FindChild(rPr, namespaces.NsW, "b") != nil {
			tElem := namespaces.FindChild(run, namespaces.NsW, "t")
			if tElem != nil && tElem.Text() == "hi" {
				boldRunFound = true
			}
		}
	}
	if !boldRunFound {
		t.Fatal("expected bold run to retain formatting on replaced text 'hi'")
	}
}

func TestFindReplaceExpectCountMismatch(t *testing.T) {
	pkg, documentURI := openFixture(t, "styled-headings")
	defer pkg.Close()

	pattern, _ := BuildFindReplacePattern("text", false, false, false)
	expect := 5
	_, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "copy",
		ExpectCount: &expect,
	})
	if !errors.Is(err, ErrReplacementCountMismatch) {
		t.Fatalf("error = %v, want ErrReplacementCountMismatch", err)
	}
}

func TestFindReplaceExpectCountZeroMatches(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	pattern, _ := BuildFindReplacePattern("absent", false, false, false)
	zero := 0
	result, err := FindReplaceInDocument(&FindReplaceRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Pattern:     pattern,
		Replace:     "x",
		ExpectCount: &zero,
	})
	if err != nil {
		t.Fatalf("FindReplaceInDocument returned error: %v", err)
	}
	if result.TotalReplacements != 0 || result.AffectedBlockCount != 0 {
		t.Fatalf("unexpected zero-match result: %+v", result)
	}
}

func TestBuildFindReplacePatternErrors(t *testing.T) {
	if _, err := BuildFindReplacePattern("", false, false, false); err == nil {
		t.Fatal("expected error for empty find")
	}
	if _, err := BuildFindReplacePattern("(", true, false, false); err == nil {
		t.Fatal("expected error for invalid regex")
	}
	// Literal mode must not interpret metacharacters.
	if _, err := BuildFindReplacePattern("(", false, false, false); err != nil {
		t.Fatalf("literal '(' should compile: %v", err)
	}
}
