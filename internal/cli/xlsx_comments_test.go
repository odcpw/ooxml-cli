package cli

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
)

func TestXLSXCommentsCommandRegistration(t *testing.T) {
	xlsx := findSubcommand(GetRootCmd(), "xlsx")
	if xlsx == nil {
		t.Fatal("xlsx command is not registered")
	}
	comments := findSubcommand(xlsx, "comments")
	if comments == nil {
		t.Fatal("xlsx comments command is not registered")
	}
	for _, name := range []string{"list", "add", "update", "remove"} {
		if sub := findSubcommand(comments, name); sub == nil {
			t.Fatalf("xlsx comments %s command is not registered", name)
		}
	}
}

func TestXLSXCommentsAddJSONReadbackAndValidate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "added.xlsx")

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "comments", "add", workbookPath,
		"--cell", "b2",
		"--author", "Ann",
		"--text", "Hello note",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("xlsx comments add failed: %v", err)
	}
	var result XLSXCommentsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal add JSON: %v\n%s", err, output)
	}
	if result.File != workbookPath || result.Sheet != "Sheet1" || result.CommentID != 0 || result.AnchoredToCell != "B2" {
		t.Fatalf("unexpected add result: %+v", result)
	}
	if result.Author != "Ann" || result.Text != "Hello note" {
		t.Fatalf("unexpected add fields: %+v", result)
	}
	if result.Handle == "" || result.PrimarySelector != result.Handle || !containsString(result.Selectors, result.Handle) || !containsString(result.Selectors, "B2") {
		t.Fatalf("missing add comment selectors: %+v", result)
	}
	if !result.CreatedPart || !result.CreatedRef || result.Output != outPath || result.DryRun {
		t.Fatalf("unexpected add mutation metadata: %+v", result)
	}
	if result.ValidateCommand == "" || result.ListCommand == "" {
		t.Fatalf("generated readback commands missing: %+v", result)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate --strict failed: %v", err)
	}

	listOut := executeGeneratedOOXMLCommandForXLSXTest(t, result.ListCommand)
	var listResult XLSXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if len(listResult.Comments) != 1 {
		t.Fatalf("expected 1 comment, got %+v", listResult.Comments)
	}
	got := listResult.Comments[0]
	if got.Author != "Ann" || got.AnchoredToCell != "B2" || got.Text != "Hello note" {
		t.Fatalf("unexpected listed comment: %+v", got)
	}
	if got.Handle == "" || got.PrimarySelector != got.Handle || !containsString(got.Selectors, got.Handle) || !containsString(got.Selectors, "B2") {
		t.Fatalf("missing listed comment selectors: %+v", got)
	}
}

func TestXLSXCommentsAddDryRunDoesNotWrite(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "comments", "add", workbookPath,
		"--cell", "A1", "--author", "Ann", "--text", "dry",
		"--dry-run",
	)
	if err != nil {
		t.Fatalf("xlsx comments add dry-run failed: %v", err)
	}
	var result XLSXCommentsAddResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("failed to unmarshal dry-run JSON: %v\n%s", err, output)
	}
	if !result.DryRun || result.Output != "" {
		t.Fatalf("unexpected dry-run metadata: %+v", result)
	}

	listOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "comments", "list", workbookPath)
	if err != nil {
		t.Fatalf("list readback failed: %v", err)
	}
	var listResult XLSXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("failed to unmarshal list JSON: %v\n%s", err, listOut)
	}
	if len(listResult.Comments) != 0 {
		t.Fatalf("dry-run wrote to workbook: %+v", listResult.Comments)
	}
}

func TestXLSXCommentsAddInPlaceAndUpdate(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	addOut, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "comments", "add", workbookPath,
		"--cell", "C3", "--author", "Ann", "--text", "before",
		"--in-place",
	)
	if err != nil {
		t.Fatalf("add in-place failed: %v", err)
	}
	var addResult XLSXCommentsAddResult
	if err := json.Unmarshal([]byte(addOut), &addResult); err != nil {
		t.Fatalf("unmarshal add: %v\n%s", err, addOut)
	}
	if addResult.Output != workbookPath {
		t.Fatalf("in-place output = %q, want %q", addResult.Output, workbookPath)
	}

	// Update with the wrong hash is rejected.
	_, err = executeRootForXLSXTest(t,
		"xlsx", "comments", "update", workbookPath,
		"--comment-id", "0", "--text", "x", "--expect-hash", "sha256:wrong",
		"--in-place",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"update", "wrong-hash"}, err, ExitInvalidArgs)

	listBeforeUpdate, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "comments", "list", workbookPath)
	if err != nil {
		t.Fatalf("list before handle update failed: %v", err)
	}
	var beforeUpdate XLSXCommentsListResult
	if err := json.Unmarshal([]byte(listBeforeUpdate), &beforeUpdate); err != nil {
		t.Fatalf("unmarshal list before update: %v\n%s", err, listBeforeUpdate)
	}
	if len(beforeUpdate.Comments) != 1 || beforeUpdate.Comments[0].Handle == "" {
		t.Fatalf("expected listed comment handle before update: %+v", beforeUpdate.Comments)
	}

	// Update with the correct hash and a new author succeeds via the listed handle.
	updOut, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "comments", "update", workbookPath,
		"--handle", beforeUpdate.Comments[0].Handle, "--text", "after", "--author", "Bob",
		"--expect-hash", addResult.ContentHash,
		"--in-place",
	)
	if err != nil {
		t.Fatalf("update failed: %v", err)
	}
	var updResult XLSXCommentsUpdateResult
	if err := json.Unmarshal([]byte(updOut), &updResult); err != nil {
		t.Fatalf("unmarshal update: %v\n%s", err, updOut)
	}
	if updResult.Author != "Bob" || updResult.Text != "after" || updResult.PreviousText != "before" || updResult.AnchoredToCell != "C3" {
		t.Fatalf("unexpected update result: %+v", updResult)
	}
	if updResult.Handle != beforeUpdate.Comments[0].Handle || updResult.PrimarySelector != updResult.Handle || !containsString(updResult.Selectors, updResult.Handle) {
		t.Fatalf("missing update comment selectors: %+v", updResult)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", workbookPath); err != nil {
		t.Fatalf("validate after update failed: %v", err)
	}

	listOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "comments", "list", workbookPath)
	if err != nil {
		t.Fatalf("list failed: %v", err)
	}
	var listResult XLSXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, listOut)
	}
	if len(listResult.Comments) != 1 || listResult.Comments[0].ID != 0 || listResult.Comments[0].Author != "Bob" || listResult.Comments[0].Text != "after" {
		t.Fatalf("unexpected list after update: %+v", listResult.Comments)
	}
}

func TestXLSXCommentsRemoveLastDropsPart(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	outPath := filepath.Join(t.TempDir(), "removed.xlsx")

	if _, err := executeRootForXLSXTest(t,
		"xlsx", "comments", "add", workbookPath,
		"--cell", "B2", "--author", "Ann", "--text", "only",
		"--in-place",
	); err != nil {
		t.Fatalf("add failed: %v", err)
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "comments", "remove", workbookPath,
		"--comment-id", "0",
		"--out", outPath,
	)
	if err != nil {
		t.Fatalf("remove failed: %v", err)
	}
	var result XLSXCommentsRemoveResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal remove: %v\n%s", err, output)
	}
	if !result.RemovedPart || result.PreviousAuthor != "Ann" || result.AnchoredToCell != "B2" {
		t.Fatalf("unexpected remove result: %+v", result)
	}

	if _, err := executeRootForXLSXTest(t, "validate", "--strict", outPath); err != nil {
		t.Fatalf("validate after remove failed: %v", err)
	}
	listOut, err := executeRootForXLSXTest(t, "--format", "json", "xlsx", "comments", "list", outPath)
	if err != nil {
		t.Fatalf("list failed: %v", err)
	}
	var listResult XLSXCommentsListResult
	if err := json.Unmarshal([]byte(listOut), &listResult); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, listOut)
	}
	if listResult.CommentsPart != "" || len(listResult.Comments) != 0 {
		t.Fatalf("expected comments part gone, got %+v", listResult)
	}
}

func TestXLSXCommentsListText(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	if _, err := executeRootForXLSXTest(t,
		"xlsx", "comments", "add", workbookPath,
		"--cell", "A1", "--author", "Ann", "--text", "hi",
		"--in-place",
	); err != nil {
		t.Fatalf("add failed: %v", err)
	}
	output, err := executeRootForXLSXTest(t, "xlsx", "comments", "list", workbookPath)
	if err != nil {
		t.Fatalf("list text failed: %v", err)
	}
	for _, want := range []string{"comment 0", "A1", "Ann", "hi"} {
		if !strings.Contains(output, want) {
			t.Fatalf("text output missing %q:\n%s", want, output)
		}
	}
}

func TestXLSXCommentsListFilterByCommentID(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})
	for _, cell := range []string{"A1", "B2"} {
		if _, err := executeRootForXLSXTest(t,
			"xlsx", "comments", "add", workbookPath,
			"--cell", cell, "--author", "Ann", "--text", cell,
			"--in-place",
		); err != nil {
			t.Fatalf("add %s failed: %v", cell, err)
		}
	}

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"xlsx", "comments", "list", workbookPath,
		"--comment-id", "1",
	)
	if err != nil {
		t.Fatalf("list --comment-id failed: %v", err)
	}
	var result XLSXCommentsListResult
	if err := json.Unmarshal([]byte(output), &result); err != nil {
		t.Fatalf("unmarshal list: %v\n%s", err, output)
	}
	if len(result.Comments) != 1 || result.Comments[0].ID != 1 || result.Comments[0].AnchoredToCell != "B2" {
		t.Fatalf("unexpected filtered list: %+v", result.Comments)
	}

	// Filtering to an absent id is a target-not-found error.
	_, err = executeRootForXLSXTest(t,
		"xlsx", "comments", "list", workbookPath, "--comment-id", "9",
	)
	assertCLIExitCodeForXLSXTest(t, []string{"list", "missing-id"}, err, ExitTargetNotFound)
}

func TestXLSXCommentsRejectBadArgs(t *testing.T) {
	workbookPath := writeTestXLSX(t, []testSheet{{Name: "Sheet1", SheetID: "1"}})

	tests := []struct {
		args []string
		code int
	}{
		{[]string{"xlsx", "comments", "add", workbookPath, "--author", "Ann", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "add", workbookPath, "--cell", "A1", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "add", workbookPath, "--cell", "not-a-cell", "--author", "Ann", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "update", workbookPath, "--text", "x", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "update", workbookPath, "--handle", "H:xlsx/ws:1/comment:a:A1", "--comment-id", "0", "--text", "x", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "update", workbookPath, "--comment-id", "0", "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "update", workbookPath, "--comment-id", "0", "--text", "x", "--dry-run"}, ExitTargetNotFound},
		{[]string{"xlsx", "comments", "remove", workbookPath, "--dry-run"}, ExitInvalidArgs},
		{[]string{"xlsx", "comments", "remove", workbookPath, "--comment-id", "0", "--dry-run"}, ExitTargetNotFound},
	}
	for _, tt := range tests {
		_, err := executeRootForXLSXTest(t, tt.args...)
		assertCLIExitCodeForXLSXTest(t, tt.args, err, tt.code)
	}
}
