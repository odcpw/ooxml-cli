package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPPTXReplaceTextOccurrencesSavedJSONReadbackAndPlanHash(t *testing.T) {
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "client.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	dryOutput, err := executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--match-text", "Content",
		"--new-text", "Client",
		"--expect-count", "2",
		"--dry-run",
	)
	require.NoError(t, err)
	var dryResult replaceTextOccurrencesResult
	require.NoError(t, json.Unmarshal([]byte(dryOutput), &dryResult))
	require.True(t, dryResult.DryRun)
	assert.Equal(t, 2, dryResult.Summary.ReplacementCount)
	assert.True(t, strings.HasPrefix(dryResult.StaleGuard.ActualPlanHash, "sha256:"))
	assertPPTXBridgeOutputVerificationTemplatesForTest(t, dryResult.PPTXBridgeReadbackCommands)

	output, err := executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--match-text", "Content",
		"--new-text", "Client",
		"--expect-count", "2",
		"--expect-plan-hash", dryResult.StaleGuard.ActualPlanHash,
		"--out", outPath,
	)
	require.NoError(t, err)
	var result replaceTextOccurrencesResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, "pptx.replace.text-occurrences", result.Operation)
	assert.Equal(t, "Content", result.MatchText)
	assert.Equal(t, "Client", result.NewText)
	assert.Equal(t, dryResult.StaleGuard.ActualPlanHash, result.StaleGuard.ExpectedPlanHash)
	assert.Equal(t, dryResult.StaleGuard.ActualPlanHash, result.StaleGuard.ActualPlanHash)
	assert.Equal(t, 2, result.Summary.ReplacementCount)
	assert.Equal(t, 2, result.Summary.ChangedTargetCount)
	require.Len(t, result.Matches, 2)
	assert.Equal(t, 1, result.Matches[0].SlideNumber)
	assert.Equal(t, "title", result.Matches[0].PrimarySelector)
	assert.Contains(t, result.Matches[0].AfterText, "Title Client Presentation")
	assertPPTXBridgeOutputVerificationCommandsForTest(t, result.PPTXBridgeReadbackCommands, outPath)
	readback := assertPPTXBridgeSavedCommandsForTest(t, result.Matches[0].PPTXBridgeReadbackCommands, outPath, "pptx shapes get")
	assert.Contains(t, readback, "Title Client Presentation")

	slide1 := readSlideXML(t, outPath, 1)
	slide2 := readSlideXML(t, outPath, 2)
	assert.Contains(t, slide1, "Title Client Presentation")
	assert.NotContains(t, slide1, "Title Content Presentation")
	assert.Contains(t, slide2, "Client Slide")
	assert.Contains(t, slide2, "This is the main content area")
}

func TestPPTXReplaceTextOccurrencesDryRunAllowsZeroAndDoesNotWrite(t *testing.T) {
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "should-not-exist.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--match-text", "Missing Brand",
		"--new-text", "New Brand",
		"--dry-run",
	)
	require.NoError(t, err)
	var result replaceTextOccurrencesResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.True(t, result.DryRun)
	assert.Equal(t, 0, result.Summary.ReplacementCount)
	assert.Empty(t, result.Matches)
	assertPPTXBridgeOutputVerificationTemplatesForTest(t, result.PPTXBridgeReadbackCommands)
	if _, err := os.Stat(outPath); !os.IsNotExist(err) {
		t.Fatalf("dry-run unexpectedly wrote %s", outPath)
	}
	assert.Contains(t, readSlideXML(t, fixturePath, 1), "Title Content Presentation")
}

func TestPPTXReplaceTextOccurrencesGuardsFailBeforeWrite(t *testing.T) {
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "guarded.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	_, err = executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--match-text", "Content",
		"--new-text", "Client",
		"--expect-count", "99",
		"--out", outPath,
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "replace", "text-occurrences"}, err, ExitInvalidArgs)
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("expect-count mismatch unexpectedly wrote %s", outPath)
	}

	_, err = executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--match-text", "Content",
		"--new-text", "Client",
		"--expect-plan-hash", "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		"--out", outPath,
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "replace", "text-occurrences"}, err, ExitInvalidArgs)
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("expect-plan-hash mismatch unexpectedly wrote %s", outPath)
	}

	_, err = executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--match-text", "Missing Brand",
		"--new-text", "New Brand",
		"--out", outPath,
	)
	assertCLIExitCodeForXLSXTest(t, []string{"pptx", "replace", "text-occurrences"}, err, ExitInvalidArgs)
	if _, statErr := os.Stat(outPath); !os.IsNotExist(statErr) {
		t.Fatalf("zero-match write unexpectedly wrote %s", outPath)
	}
}

func TestPPTXReplaceTextOccurrencesForSlidesIgnoreCase(t *testing.T) {
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "scoped.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--for-slides", "2",
		"--match-text", "content",
		"--new-text", "topic",
		"--ignore-case",
		"--expect-count", "2",
		"--out", outPath,
	)
	require.NoError(t, err)
	var result replaceTextOccurrencesResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.True(t, result.IgnoreCase)
	assert.Equal(t, "2", result.ForSlides)
	assert.Equal(t, []int{2}, result.Scope.Slides)
	assert.Equal(t, 2, result.Summary.ReplacementCount)

	assert.Contains(t, readSlideXML(t, outPath, 1), "Title Content Presentation")
	slide2 := readSlideXML(t, outPath, 2)
	assert.Contains(t, slide2, "topic Slide")
	assert.Contains(t, slide2, "This is the main topic area")
	assert.NotContains(t, slide2, "Content Slide")
	assert.NotContains(t, slide2, "main content area")
}

func TestPPTXReplaceTextOccurrencesIncludesTableCells(t *testing.T) {
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "table.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/table-simple/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--json",
		"pptx", "replace", "text-occurrences", fixturePath,
		"--for-slides", "2",
		"--match-text", "R1",
		"--new-text", "RowOne",
		"--expect-count", "3",
		"--out", outPath,
	)
	require.NoError(t, err)
	var result replaceTextOccurrencesResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, 3, result.Summary.ReplacementCount)
	assert.Equal(t, 1, result.Summary.ChangedTargetCount)
	require.Len(t, result.Matches, 3)
	assert.True(t, result.Scope.TableCellsIncluded)
	assert.Equal(t, "table", result.Matches[0].TargetKind)
	assert.Equal(t, "table:1", result.Matches[0].PrimarySelector)
	readback := assertPPTXBridgeSavedCommandsForTest(t, result.Matches[0].PPTXBridgeReadbackCommands, outPath, "pptx tables show")
	assert.Contains(t, readback, "RowOneC0")
	assert.Contains(t, readback, "RowOneC1")
	assert.Contains(t, readback, "RowOneC2")
}
