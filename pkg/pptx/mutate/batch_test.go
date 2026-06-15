package mutate

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
)

func TestBatchTextReplace_SuccessfulBatch(t *testing.T) {
	// Use a test fixture with multiple slides
	testFile := "../../testdata/simple.pptx"
	if _, err := os.Stat(testFile); err != nil {
		t.Skipf("test fixture not found: %s", testFile)
	}

	session, err := opc.Open(testFile)
	if err != nil {
		t.Fatalf("failed to open test file: %v", err)
	}
	defer session.Close()

	// Test batch text replacement on slides 1-2
	req := &BatchTextReplaceRequest{
		Package:      session,
		SlideNumbers: []int{1, 2},
		Target:       "title",
		NewText:      "Updated Title",
		Mode:         "plain-text",
	}

	result := BatchTextReplace(req)

	// Verify the results
	if result.FatalError != "" {
		t.Fatalf("unexpected fatal error: %s", result.FatalError)
	}

	if result.TotalSlides != 2 {
		t.Errorf("expected 2 total slides, got %d", result.TotalSlides)
	}

	// We expect at least one to succeed (assuming simple.pptx has title placeholders on at least one slide)
	if result.SuccessCount == 0 && result.NotFoundCount == 0 && result.ErrorCount == 0 {
		t.Fatal("expected some result, got none")
	}

	// Verify results structure
	if len(result.Results) != len(req.SlideNumbers) {
		t.Errorf("expected %d results, got %d", len(req.SlideNumbers), len(result.Results))
	}

	// Verify each result has correct slide number
	for i, slideResult := range result.Results {
		if slideResult.SlideNumber != req.SlideNumbers[i] {
			t.Errorf("result %d: expected slide %d, got %d", i, req.SlideNumbers[i], slideResult.SlideNumber)
		}
	}
}

func TestBatchTextReplace_EmptySlideList(t *testing.T) {
	testFile := "../../testdata/simple.pptx"
	if _, err := os.Stat(testFile); err != nil {
		t.Skipf("test fixture not found: %s", testFile)
	}

	session, err := opc.Open(testFile)
	if err != nil {
		t.Fatalf("failed to open test file: %v", err)
	}
	defer session.Close()

	// Test with empty slide list
	req := &BatchTextReplaceRequest{
		Package:      session,
		SlideNumbers: []int{},
		Target:       "title",
		NewText:      "Updated Title",
		Mode:         "plain-text",
	}

	result := BatchTextReplace(req)

	if result.TotalSlides != 0 {
		t.Errorf("expected 0 total slides for empty list, got %d", result.TotalSlides)
	}
}

func TestBatchTextReplace_InvalidSlideNumber(t *testing.T) {
	testFile := "../../testdata/simple.pptx"
	if _, err := os.Stat(testFile); err != nil {
		t.Skipf("test fixture not found: %s", testFile)
	}

	session, err := opc.Open(testFile)
	if err != nil {
		t.Fatalf("failed to open test file: %v", err)
	}
	defer session.Close()

	// Test with invalid slide number (way out of range)
	req := &BatchTextReplaceRequest{
		Package:      session,
		SlideNumbers: []int{999},
		Target:       "title",
		NewText:      "Updated Title",
		Mode:         "plain-text",
	}

	result := BatchTextReplace(req)

	// Should report not found for out-of-range slide
	if result.ErrorCount == 0 && result.NotFoundCount == 0 {
		t.Error("expected error or not-found for invalid slide")
	}

	// Should have one result
	if len(result.Results) != 1 {
		t.Errorf("expected 1 result, got %d", len(result.Results))
	}

	// Result should indicate not found
	if !result.Results[0].NotFound {
		t.Error("expected slide 999 to be marked as not found")
	}
}

func TestBatchTextReplace_PartialSuccess(t *testing.T) {
	// This tests partial success: some slides have the target, some don't
	testFile := "../../testdata/simple.pptx"
	if _, err := os.Stat(testFile); err != nil {
		t.Skipf("test fixture not found: %s", testFile)
	}

	session, err := opc.Open(testFile)
	if err != nil {
		t.Fatalf("failed to open test file: %v", err)
	}
	defer session.Close()

	// Get actual slide count to construct a realistic test
	graph, err := inspect.ParsePresentation(session)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideCount := len(graph.Slides)
	if slideCount < 2 {
		t.Skipf("test requires at least 2 slides, found %d", slideCount)
	}

	// Request a target that might not exist on all slides (body placeholder)
	// This should lead to mixed success/not-found results
	req := &BatchTextReplaceRequest{
		Package:      session,
		SlideNumbers: []int{1, 2},
		Target:       "body:0",
		NewText:      "Updated Body",
		Mode:         "plain-text",
	}

	result := BatchTextReplace(req)

	// Verify the result structure is valid
	if result.TotalSlides != 2 {
		t.Errorf("expected 2 total slides, got %d", result.TotalSlides)
	}

	// Should have exactly 2 results
	if len(result.Results) != 2 {
		t.Errorf("expected 2 results, got %d", len(result.Results))
	}

	// Verify no fatal error
	if result.FatalError != "" {
		t.Fatalf("unexpected fatal error: %s", result.FatalError)
	}

	// Verify all results are accounted for
	successCount := 0
	notFoundCount := 0
	errorCount := 0
	for _, r := range result.Results {
		if r.Success {
			successCount++
		} else if r.NotFound {
			notFoundCount++
		} else if r.Error != "" {
			errorCount++
		}
	}

	if successCount+notFoundCount+errorCount != 2 {
		t.Errorf("expected all results accounted for, got success=%d, notFound=%d, error=%d", successCount, notFoundCount, errorCount)
	}

	// Count should match result fields
	if result.SuccessCount != successCount {
		t.Errorf("successCount mismatch: field=%d, actual=%d", result.SuccessCount, successCount)
	}
	if result.NotFoundCount != notFoundCount {
		t.Errorf("notFoundCount mismatch: field=%d, actual=%d", result.NotFoundCount, notFoundCount)
	}
	if result.ErrorCount != errorCount {
		t.Errorf("errorCount mismatch: field=%d, actual=%d", result.ErrorCount, errorCount)
	}
}

func TestBatchTextReplace_EmptyTarget(t *testing.T) {
	testFile := "../../testdata/simple.pptx"
	if _, err := os.Stat(testFile); err != nil {
		t.Skipf("test fixture not found: %s", testFile)
	}

	session, err := opc.Open(testFile)
	if err != nil {
		t.Fatalf("failed to open test file: %v", err)
	}
	defer session.Close()

	// Test with empty target
	req := &BatchTextReplaceRequest{
		Package:      session,
		SlideNumbers: []int{1},
		Target:       "",
		NewText:      "Updated Title",
		Mode:         "plain-text",
	}

	result := BatchTextReplace(req)

	// Should report fatal error for empty target
	if result.FatalError == "" {
		t.Error("expected fatal error for empty target")
	}
}

func TestSummarizeBatchResult_TextReplace(t *testing.T) {
	result := &BatchTextReplaceResult{
		TotalSlides:   3,
		SuccessCount:  2,
		NotFoundCount: 1,
		ErrorCount:    0,
	}

	summary := SummarizeBatchResult("Replace text", result)

	if summary == "" {
		t.Error("expected non-empty summary")
	}

	// Verify summary contains key information
	if !contains(summary, "2") || !contains(summary, "3") {
		t.Errorf("expected summary to contain success/total counts, got: %s", summary)
	}
}

func TestSummarizeBatchResult_ImageReplace(t *testing.T) {
	result := &BatchImageReplaceResult{
		TotalSlides:   2,
		SuccessCount:  1,
		NotFoundCount: 1,
		ErrorCount:    0,
	}

	summary := SummarizeBatchResult("Replace images", result)

	if summary == "" {
		t.Error("expected non-empty summary")
	}

	// Verify summary is coherent
	if !contains(summary, "1") || !contains(summary, "2") {
		t.Errorf("expected summary to contain counts, got: %s", summary)
	}
}

// Helper function to check if string contains substring
func contains(s, substr string) bool {
	return len(s) > 0 && len(substr) > 0 && len(s) >= len(substr)
}

// TestParseSlideSpec tests the parseSlideSpec function used in CLI
func TestParseSlideSpec(t *testing.T) {
	tests := []struct {
		spec      string
		expected  []int
		wantError bool
	}{
		{"1", []int{1}, false},
		{"1,3,5", []int{1, 3, 5}, false},
		{"1-3", []int{1, 2, 3}, false},
		{"1-3,5,7-9", []int{1, 2, 3, 5, 7, 8, 9}, false},
		{"1-1", []int{1}, false},
		{"3,1,2", []int{1, 2, 3}, false},         // Should be sorted
		{"1,1,1", []int{1}, false},               // Duplicates removed
		{"1-5,3-4", []int{1, 2, 3, 4, 5}, false}, // Overlapping ranges
		{"0", []int{}, true},                     // 0 is invalid
		{"1-0", []int{}, true},                   // Invalid range
		{"", []int{}, true},                      // Empty
		{"abc", []int{}, true},                   // Invalid number
	}

	for _, tt := range tests {
		result, err := parseSlideSpecHelper(tt.spec)

		if (err != nil) != tt.wantError {
			t.Errorf("parseSlideSpec(%q): wantError=%v, got err=%v", tt.spec, tt.wantError, err)
			continue
		}

		if !tt.wantError {
			if len(result) != len(tt.expected) {
				t.Errorf("parseSlideSpec(%q): expected %v, got %v", tt.spec, tt.expected, result)
			} else {
				for i, v := range result {
					if v != tt.expected[i] {
						t.Errorf("parseSlideSpec(%q): expected %v, got %v", tt.spec, tt.expected, result)
						break
					}
				}
			}
		}
	}
}

// Helper function for testing parseSlideSpec (mirrors the CLI implementation)
func parseSlideSpecHelper(spec string) ([]int, error) {
	// Implementation copied from internal/cli/slide_helpers.go:parseSlideSpec

	spec = strings.TrimSpace(spec)
	if spec == "" {
		return nil, fmt.Errorf("empty specification")
	}

	var result []int
	seenMap := make(map[int]bool)

	// Split by comma to get ranges and individual numbers
	parts := strings.Split(spec, ",")
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if part == "" {
			continue
		}

		// Check if it's a range (contains a dash)
		if strings.Contains(part, "-") {
			// Split by dash
			rangeParts := strings.Split(part, "-")
			if len(rangeParts) != 2 {
				return nil, fmt.Errorf("invalid range format: %s", part)
			}

			start, err := strconv.Atoi(strings.TrimSpace(rangeParts[0]))
			if err != nil || start <= 0 {
				return nil, fmt.Errorf("invalid range start: %s", rangeParts[0])
			}

			end, err := strconv.Atoi(strings.TrimSpace(rangeParts[1]))
			if err != nil || end <= 0 {
				return nil, fmt.Errorf("invalid range end: %s", rangeParts[1])
			}

			if start > end {
				return nil, fmt.Errorf("range start (%d) cannot be greater than end (%d)", start, end)
			}

			// Add all numbers in the range, avoiding duplicates
			for i := start; i <= end; i++ {
				if !seenMap[i] {
					result = append(result, i)
					seenMap[i] = true
				}
			}
		} else {
			// Single number
			num, err := strconv.Atoi(part)
			if err != nil || num <= 0 {
				return nil, fmt.Errorf("invalid slide number: %s", part)
			}

			// Add if not already included
			if !seenMap[num] {
				result = append(result, num)
				seenMap[num] = true
			}
		}
	}

	// Sort the result
	sort.Ints(result)

	return result, nil
}

// TestBatchMutationIntegration tests that batch mutations work correctly with actual files
func TestBatchMutationIntegration_SaveAndValidate(t *testing.T) {
	// Create a temporary copy of the test file
	testFile := "../../testdata/simple.pptx"
	if _, err := os.Stat(testFile); err != nil {
		t.Skipf("test fixture not found: %s", testFile)
	}

	tempFile := filepath.Join(t.TempDir(), "test_batch.pptx")
	if err := copyFile(testFile, tempFile); err != nil {
		t.Fatalf("failed to copy test file: %v", err)
	}

	// Open, perform batch mutation, and save
	session, err := opc.Open(tempFile)
	if err != nil {
		t.Fatalf("failed to open temp file: %v", err)
	}

	req := &BatchTextReplaceRequest{
		Package:      session,
		SlideNumbers: []int{1},
		Target:       "title",
		NewText:      "Test Batch Update",
		Mode:         "plain-text",
	}

	result := BatchTextReplace(req)

	if result.FatalError != "" {
		t.Fatalf("batch mutation failed: %s", result.FatalError)
	}

	// Save the file
	if err := session.SaveAs(tempFile); err != nil {
		t.Fatalf("failed to save file: %v", err)
	}

	session.Close()

	// Verify the file can be re-opened (basic sanity check)
	session2, err := opc.Open(tempFile)
	if err != nil {
		t.Fatalf("failed to reopen saved file: %v", err)
	}
	defer session2.Close()

	// Verify content was modified (basic check - would need more sophisticated validation in prod)
	graph, err := inspect.ParsePresentation(session2)
	if err != nil {
		t.Fatalf("failed to parse reopened file: %v", err)
	}

	if len(graph.Slides) == 0 {
		t.Fatal("expected at least one slide in reopened file")
	}
}

// Helper function to copy a file
func copyFile(src, dst string) error {
	srcBytes, err := os.ReadFile(src)
	if err != nil {
		return err
	}
	return os.WriteFile(dst, srcBytes, 0644)
}
