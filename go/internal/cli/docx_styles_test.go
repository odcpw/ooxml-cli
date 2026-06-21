package cli

import (
	"encoding/json"
	"testing"
)

func TestDOCXStylesCommandRegistration(t *testing.T) {
	cmd := GetRootCmd()

	docx := findSubcommand(cmd, "docx")
	if docx == nil {
		t.Fatal("docx command is not registered")
	}
	styles := findSubcommand(docx, "styles")
	if styles == nil {
		t.Fatal("docx styles command is not registered")
	}
	for _, name := range []string{"list", "show"} {
		if command := findSubcommand(styles, name); command == nil {
			t.Fatalf("docx styles %s command is not registered", name)
		}
	}
}

func TestDOCXStylesListJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("styles-catalog")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "list", documentPath)
	if err != nil {
		t.Fatalf("docx styles list failed: %v", err)
	}

	var result DOCXStylesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal styles list JSON: %v\n%s", err, output)
	}
	if result.StylesPartURI == nil || *result.StylesPartURI != "/word/styles.xml" {
		t.Fatalf("stylesPartUri = %v, want /word/styles.xml", result.StylesPartURI)
	}
	if result.Count != 9 || len(result.Styles) != 9 {
		t.Fatalf("count = %d, len(styles) = %d, want 9", result.Count, len(result.Styles))
	}

	var heading *struct {
		basedOn         string
		next            string
		builtin         bool
		primarySelector string
		selectors       []string
	}
	for _, style := range result.Styles {
		if style.StyleID == "Heading1" {
			heading = &struct {
				basedOn         string
				next            string
				builtin         bool
				primarySelector string
				selectors       []string
			}{style.BasedOn, style.Next, style.Builtin, style.PrimarySelector, style.Selectors}
		}
	}
	if heading == nil {
		t.Fatal("Heading1 not present in styles list")
	}
	if heading.basedOn != "Normal" || heading.next != "BodyText" || !heading.builtin {
		t.Fatalf("Heading1 fields = %+v, want basedOn=Normal next=BodyText builtin=true", heading)
	}
	if heading.primarySelector != "Heading1" || !containsString(heading.selectors, "Heading1") {
		t.Fatalf("Heading1 selectors = primary=%q selectors=%+v", heading.primarySelector, heading.selectors)
	}
}

func TestDOCXStylesListFiltered(t *testing.T) {
	documentPath := getDOCXTestFilePath("styles-catalog")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "list", documentPath, "--type", "paragraph")
	if err != nil {
		t.Fatalf("docx styles list --type paragraph failed: %v", err)
	}
	var result DOCXStylesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal filtered styles list JSON: %v\n%s", err, output)
	}
	if result.Count != 4 {
		t.Fatalf("paragraph count = %d, want 4", result.Count)
	}
	for _, style := range result.Styles {
		if style.Type != "paragraph" {
			t.Fatalf("filtered style %q has type %q, want paragraph", style.StyleID, style.Type)
		}
	}
}

func TestDOCXStylesListInvalidType(t *testing.T) {
	documentPath := getDOCXTestFilePath("styles-catalog")

	_, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "list", documentPath, "--type", "list")
	if err == nil {
		t.Fatal("expected error for invalid --type list, got nil")
	}
}

func TestDOCXStylesListNoStylesPart(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "list", documentPath)
	if err != nil {
		t.Fatalf("docx styles list on minimal failed: %v", err)
	}
	var result DOCXStylesListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal styles list JSON: %v\n%s", err, output)
	}
	if result.StylesPartURI != nil {
		t.Fatalf("stylesPartUri = %v, want null", *result.StylesPartURI)
	}
	if result.Count != 0 || len(result.Styles) != 0 {
		t.Fatalf("count = %d, len(styles) = %d, want 0", result.Count, len(result.Styles))
	}
}

func TestDOCXStylesShowJSON(t *testing.T) {
	documentPath := getDOCXTestFilePath("styles-catalog")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "show", documentPath, "--style", "Heading1")
	if err != nil {
		t.Fatalf("docx styles show failed: %v", err)
	}
	var result DOCXStylesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal styles show JSON: %v\n%s", err, output)
	}
	if !result.Found || result.Style == nil {
		t.Fatalf("found = %t, style = %v, want found with style", result.Found, result.Style)
	}
	if result.StyleID != "Heading1" {
		t.Fatalf("styleId = %q, want Heading1", result.StyleID)
	}
	if result.Style.Name != "heading 1" || result.Style.Type != "paragraph" {
		t.Fatalf("style name/type = %q/%q", result.Style.Name, result.Style.Type)
	}
	if result.Style.BasedOn != "Normal" || result.Style.Next != "BodyText" {
		t.Fatalf("style basedOn/next = %q/%q", result.Style.BasedOn, result.Style.Next)
	}
	if result.Style.PrimarySelector != "Heading1" || !containsString(result.Style.Selectors, "Heading1") {
		t.Fatalf("style selectors = primary=%q selectors=%+v", result.Style.PrimarySelector, result.Style.Selectors)
	}
}

func TestDOCXStylesShowNotFound(t *testing.T) {
	documentPath := getDOCXTestFilePath("styles-catalog")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "show", documentPath, "--style", "NonExistent")
	if err != nil {
		t.Fatalf("docx styles show for missing style should not error: %v", err)
	}
	var result DOCXStylesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal styles show JSON: %v\n%s", err, output)
	}
	if result.Found || result.Style != nil {
		t.Fatalf("found = %t, style = %v, want not found", result.Found, result.Style)
	}
}

func TestDOCXStylesShowRequiresStyle(t *testing.T) {
	documentPath := getDOCXTestFilePath("styles-catalog")

	_, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "show", documentPath)
	if err == nil {
		t.Fatal("expected error when --style is omitted, got nil")
	}
}

func TestDOCXStylesShowNoStylesPart(t *testing.T) {
	documentPath := getDOCXTestFilePath("minimal")

	output, err := executeRootForXLSXTest(t, "--format", "json", "docx", "styles", "show", documentPath, "--style", "Heading1")
	if err != nil {
		t.Fatalf("docx styles show on minimal failed: %v", err)
	}
	var result DOCXStylesShowResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal styles show JSON: %v\n%s", err, output)
	}
	if result.StylesPartURI != nil {
		t.Fatalf("stylesPartUri = %v, want null", *result.StylesPartURI)
	}
	if result.Found || result.Style != nil {
		t.Fatalf("found = %t, style = %v, want not found", result.Found, result.Style)
	}
}
