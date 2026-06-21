package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXHyperlinksAddListReadback(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "hl.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "hyperlinks", "add", workbookPath,
		"--sheet", "1", "--cell", "A1",
		"--url", "https://example.com",
		"--tooltip", "Visit",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("hyperlinks add failed: %v", err)
	}
	var addResult XLSXHyperlinkMutationResult
	if err := json.Unmarshal([]byte(output), &addResult); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, output)
	}
	if addResult.Ref != "A1" || addResult.Hyperlink == nil || addResult.Hyperlink.URL != "https://example.com" || addResult.Hyperlink.RelID == "" {
		t.Fatalf("unexpected add result: %+v", addResult)
	}
	if addResult.Hyperlink.PrimarySelector != "A1" || !containsString(addResult.Hyperlink.Selectors, "A1") {
		t.Fatalf("missing add hyperlink selectors: %+v", addResult.Hyperlink)
	}
	if addResult.HyperlinksListCommand == "" || !strings.Contains(addResult.HyperlinksListCommand, "--json") {
		t.Fatalf("missing hyperlinks list readback command: %+v", addResult)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, addResult.HyperlinksListCommand)
	var listResult XLSXHyperlinksListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if listResult.Count != 1 || listResult.Hyperlinks[0].Ref != "A1" || listResult.Hyperlinks[0].URL != "https://example.com" {
		t.Fatalf("unexpected list result: %+v", listResult)
	}
	if listResult.Hyperlinks[0].PrimarySelector != "A1" || !containsString(listResult.Hyperlinks[0].Selectors, "A1") {
		t.Fatalf("missing list hyperlink selectors: %+v", listResult.Hyperlinks[0])
	}
}

func TestXLSXHyperlinksAddInternalLocation(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	outPath := filepath.Join(t.TempDir(), "hl.xlsx")
	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "hyperlinks", "add", workbookPath,
		"--sheet", "1", "--cell", "B2",
		"--location", "Sheet1!A1",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("hyperlinks add internal failed: %v", err)
	}
	var res XLSXHyperlinkMutationResult
	if err := json.Unmarshal([]byte(output), &res); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, output)
	}
	if res.Hyperlink == nil || res.Hyperlink.Location != "Sheet1!A1" || res.Hyperlink.RelID != "" {
		t.Fatalf("unexpected internal hyperlink result: %+v", res)
	}
}

func TestXLSXHyperlinksAddRequiresExactlyOneTarget(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	_, err := executeRootForXLSXTest(t,
		"xlsx", "hyperlinks", "add", workbookPath,
		"--sheet", "1", "--cell", "A1",
		"--out", filepath.Join(t.TempDir(), "x.xlsx"),
	)
	if err == nil {
		t.Fatalf("expected error when neither url nor location given")
	}
}

func TestXLSXHyperlinksUpdateGuardMismatch(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	added := filepath.Join(t.TempDir(), "hl.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "add", workbookPath, "--sheet", "1", "--cell", "A1", "--url", "https://a.com", "--out", added); err != nil {
		t.Fatalf("add failed: %v", err)
	}
	_, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "update", added, "--sheet", "1", "--cell", "A1", "--url", "https://b.com", "--expect-url", "https://WRONG.com", "--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected guard mismatch error")
	}
	cliErr, ok := err.(*CLIError)
	if !ok || cliErr.ExitCode != ExitInvalidArgs {
		t.Fatalf("expected ExitInvalidArgs, got %v", err)
	}
}

func TestXLSXHyperlinksUpdateRejectsBothTargets(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	added := filepath.Join(t.TempDir(), "hl.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "add", workbookPath, "--sheet", "1", "--cell", "A1", "--url", "https://a.com", "--out", added); err != nil {
		t.Fatalf("add failed: %v", err)
	}
	_, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "update", added, "--sheet", "1", "--cell", "A1",
		"--url", "https://b.com", "--location", "Sheet1!A1", "--out", filepath.Join(t.TempDir(), "x.xlsx"))
	if err == nil {
		t.Fatalf("expected error when both --url and --location given to update")
	}
}

func TestXLSXHyperlinksDeleteCleansRelationship(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	added := filepath.Join(t.TempDir(), "hl.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "add", workbookPath, "--sheet", "1", "--cell", "A1", "--url", "https://a.com", "--out", added); err != nil {
		t.Fatalf("add failed: %v", err)
	}
	deleted := filepath.Join(t.TempDir(), "del.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "delete", added, "--sheet", "1", "--cell", "A1", "--out", deleted); err != nil {
		t.Fatalf("delete failed: %v", err)
	}
	out, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "hyperlinks", "list", deleted, "--sheet", "1")
	if err != nil {
		t.Fatalf("list failed: %v", err)
	}
	var listResult XLSXHyperlinksListResult
	if err := json.Unmarshal([]byte(out), &listResult); err != nil {
		t.Fatalf("failed to unmarshal: %v\n%s", err, out)
	}
	if listResult.Count != 0 {
		t.Fatalf("expected no hyperlinks after delete: %+v", listResult)
	}
	// Validate the output to confirm no dangling relationship corrupts the package.
	if _, err := executeRootForXLSXTest(t, "validate", "--strict", deleted); err != nil {
		t.Fatalf("validate after delete failed: %v", err)
	}
}

func TestXLSXHyperlinksShowNotFoundListsAvailable(t *testing.T) {
	workbookPath := getXLSXTestFilePath("minimal-workbook")
	added := filepath.Join(t.TempDir(), "hl.xlsx")
	if _, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "add", workbookPath, "--sheet", "1", "--cell", "A1", "--url", "https://a.com", "--out", added); err != nil {
		t.Fatalf("add failed: %v", err)
	}
	_, err := executeRootForXLSXTest(t, "xlsx", "hyperlinks", "show", added, "--sheet", "1", "--cell", "Z9")
	if err == nil {
		t.Fatalf("expected not-found error")
	}
	if !strings.Contains(err.Error(), "A1") {
		t.Fatalf("expected available refs in error, got: %v", err)
	}
	if !strings.Contains(err.Error(), "did you mean: A1") || !strings.Contains(err.Error(), "ooxml --json xlsx hyperlinks list <file> --sheet sheetId:1") {
		t.Fatalf("expected selector candidates and discovery command, got: %v", err)
	}
}
