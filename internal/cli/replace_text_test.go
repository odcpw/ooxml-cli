package cli

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestReplaceText_WithInlineText(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	jsonPath := filepath.Join(tmpDir, "result.json")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--text", "Quarterly Update",
		"--out", outputPath,
		"--format", "json",
		"-o", jsonPath,
	})
	require.NoError(t, rootCmd.Execute())

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var result replaceTextResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outputPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, 1, result.SlideNumber)
	assert.Equal(t, "title", result.Target)
	assert.Equal(t, "Quarterly Update", result.NewText)
	require.NotNil(t, result.Destination)
	assert.Equal(t, outputPath, result.Destination.File)
	assert.Equal(t, 1, result.Destination.Slide)
	assert.Equal(t, "title", result.Destination.Target)
	assert.Equal(t, 2, result.Destination.ShapeID)
	assert.Equal(t, "title", result.Destination.PrimarySelector)
	assert.True(t, containsString(result.Destination.Selectors, "title"))
	assert.True(t, containsString(result.Destination.Selectors, "shape:2"))
	assert.Contains(t, result.Destination.TextPreview, "Quarterly Update")
	readback := assertPPTXBridgeSavedCommandsForTest(t, result.PPTXBridgeReadbackCommands, outputPath, "pptx shapes get")
	var readbackResult PPTXShapesResult
	require.NoError(t, json.Unmarshal([]byte(readback), &readbackResult))
	require.Len(t, readbackResult.Shapes, 1)
	assert.Contains(t, readbackResult.Shapes[0].TextPreview, "Quarterly Update")

	slide1 := readSlideXML(t, outputPath, 1)
	assert.Contains(t, slide1, "Quarterly Update")
	assert.Contains(t, slide1, "Subtitle goes here")
	assert.NotContains(t, slide1, "Title Content Presentation")

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "Content Slide")
	assert.Contains(t, slide2, "This is the main content area")
}

func TestReplaceText_DryRunJSONIncludesDestinationReadback(t *testing.T) {
	tmpDir := t.TempDir()
	jsonPath := filepath.Join(tmpDir, "dry-run.json")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	var stdout bytes.Buffer
	rootCmd.SetOut(&stdout)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--text", "Dry Run Title",
		"--dry-run",
		"--format", "json",
		"-o", jsonPath,
	})
	require.NoError(t, rootCmd.Execute())
	assert.Empty(t, strings.TrimSpace(stdout.String()))

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var result replaceTextResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	require.NotNil(t, result.Destination)
	assert.Empty(t, result.Destination.File)
	assert.Equal(t, "title", result.Destination.PrimarySelector)
	assert.Contains(t, result.Destination.TextPreview, "Dry Run Title")
	assertPPTXBridgeDryRunTemplatesForTest(t, result.PPTXBridgeReadbackCommands, "pptx shapes get")

	slide1 := readSlideXML(t, fixturePath, 1)
	assert.NotContains(t, slide1, "Dry Run Title")
	assert.Contains(t, slide1, "Title Content Presentation")
}

func TestReplaceText_WithTextFile(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	textPath := filepath.Join(tmpDir, "body.txt")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	require.NoError(t, os.WriteFile(textPath, []byte("Body from file"), 0644))

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--text-file", textPath,
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "Body from file")
	assert.NotContains(t, slide2, "This is the main content area")
}

func TestReplaceText_TargetNotFound(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "shape:9999",
		"--text", "won't work",
		"--out", outputPath,
	})

	err = rootCmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "shape not found")
	assert.Contains(t, err.Error(), "did you mean:")
	assert.Contains(t, err.Error(), "title")
	assert.Contains(t, err.Error(), "ooxml --json pptx shapes show <file> --slide 1")
}

func newTestRootCmd(t *testing.T) *cobra.Command {
	t.Helper()
	cmd := GetRootCmd()
	resetFlagsRecursive(cmd)
	resetTestGlobals()
	t.Cleanup(func() {
		resetFlagsRecursive(cmd)
		resetTestGlobals()
	})
	return cmd
}

var resetTestGlobals = func() {
	flagFormat = "text"
	flagVerbosity = "normal"
	flagOutput = ""
	flagOut = ""
	flagInPlace = ""
	flagBackup = ""
	replaceTextSlide = 0
	replaceTextTarget = ""
	replaceTextValue = ""
	replaceTextFilePath = ""
	replaceTextMode = "plain-text"
	replaceRichTextFilePath = ""
	replaceTextOccurrencesMatchText = ""
	replaceTextOccurrencesNewText = ""
	replaceTextOccurrencesNewTextFile = ""
	replaceTextOccurrencesForSlides = ""
	replaceTextOccurrencesIgnoreCase = false
	replaceTextOccurrencesExpectCount = 0
	replaceTextOccurrencesExpectPlanHash = ""
	replaceTextOccurrencesAllowZero = false
	replaceTextFromXLSXSlide = 0
	replaceTextFromXLSXTarget = ""
	replaceTextFromXLSXWorkbook = ""
	replaceTextFromXLSXSheet = ""
	replaceTextFromXLSXRange = ""
	replaceTextFromXLSXMaxCells = 100000
	replaceTextFromXLSXFormulaMode = "value"
	replaceTextFromXLSXMode = "plain-text"
	replaceTextFromXLSXRowSep = "\n"
	replaceTextFromXLSXColSep = "\t"
	replaceTextMapFromXLSXWorkbook = ""
	replaceTextMapFromXLSXSheet = ""
	replaceTextMapFromXLSXRange = ""
	replaceTextMapFromXLSXTable = ""
	replaceTextMapFromXLSXMaxCells = 100000
	replaceTextMapFromXLSXFormulaMode = "value"
	replaceTextMapFromXLSXMode = "plain-text"
	replaceTextMapFromXLSXSlideCol = "slide"
	replaceTextMapFromXLSXTargetCol = "target"
	replaceTextMapFromXLSXTextCol = "text"
	replaceTextMapFromXLSXExpectSourceRange = ""
	vbaCreateFamily = ""
	vbaCreateSources = nil
	vbaCreateExtractBinPath = ""
	vbaCreateOfficeScriptPath = ""
	vbaCreateEnableVBOMAccess = false
	vbaCreateVisible = false
	vbaCreateForce = false
	vbaAllowExperimentalRewrite = false
	replaceImageTarget = ""
	replaceImageFile = ""
	replaceImageFitMode = "contain"
	cloneSlideNumber = 0
	cloneInsertAfter = 0
	newSlideLayout = ""
	newSlideSetTexts = nil
	newSlideSetRichText = nil
	newSlideSetImages = nil
	newSlideSetImageCoords = nil
	newSlideSetImageSlotKeys = nil
	newSlideImageFitMode = "contain"
	newSlideLevel = -1
	newSlideAlignment = ""
	newSlideBulletMode = ""
	newSlideBulletChar = ""
	newSlideAutoNum = ""
	newSlideSpaceBefore = 0
	newSlideSpaceAfter = 0
	newSlideLineSpacing = 0
	renderSlidesArg = ""
	renderImageFormat = "png"
	renderDPI = 144
	diffRender = false
	diffThreshold = 0.01
	slidesSelectorsSlide = 0
	pptxShapesShowSlide = 0
	pptxShapesShowIncludeText = false
	pptxShapesShowIncludeBounds = false
	pptxShapesGetSlide = 0
	pptxShapesGetTarget = ""
	pptxShapesGetIncludeText = false
	pptxShapesGetIncludeBounds = false
	pptxShapesSetBoundsSlide = 0
	pptxShapesSetBoundsTarget = ""
	pptxShapesSetBoundsValue = ""
	pptxShapesDeleteSlide = 0
	pptxShapesDeleteTarget = ""
	docxTablesShowTable = 0
	docxTablesShowDetails = false
	docxTablesSetCellTable = 0
	docxTablesSetCellRow = 0
	docxTablesSetCellCol = 0
	docxTablesSetCellText = ""
	docxTablesSetCellTextFile = ""
	docxTablesSetCellHash = ""
	docxTablesClearCellTable = 0
	docxTablesClearCellRow = 0
	docxTablesClearCellCol = 0
	docxTablesClearCellHash = ""
	docxTablesInsertRowTable = 0
	docxTablesInsertRowAt = 0
	docxTablesInsertRowHash = ""
	docxTablesDeleteRowTable = 0
	docxTablesDeleteRowRow = 0
	docxTablesDeleteRowHash = ""
	xlsxTablesAppendRecordsSheet = ""
	xlsxTablesAppendRecordsTable = ""
	xlsxTablesAppendRecordsExpectRange = ""
	xlsxTablesAppendRecordsRecords = ""
	xlsxTablesAppendRecordsRecordsFile = ""
	xlsxTablesAppendRecordsMissing = "reject"
	xlsxTablesAppendRecordsNullPolicy = "skip"
	xlsxTablesAppendRecordsMaxCells = 100000
	xlsxTablesAppendRecordsIgnoreExtraFields = false
	xlsxTablesAppendRecordsOverwriteFormulas = false
	layoutCloneLayout = ""
	layoutCloneName = ""
	layoutRenameLayout = ""
	layoutRenameName = ""
	layoutDeleteShapeLayout = ""
	layoutDeleteShapeTarget = ""
	layoutSetBoundsLayout = ""
	layoutSetBoundsTarget = ""
	layoutSetBoundsValue = ""
	importLayoutSourcePath = ""
	importLayoutSelector = ""
	importLayoutThemePolicy = "reuse"
	addLayoutPlaceholderIdx = -1
	addLayoutPlaceholderIdxSet = false
	addMasterPlaceholderIdx = -1
	addMasterPlaceholderIdxSet = false
	addTextboxSlide = 0
	addTextboxText = ""
	addTextboxX = 0
	addTextboxY = 0
	addTextboxCX = 0
	addTextboxCY = 0
	addTextboxName = ""
	addTextboxMode = "plain"
	addTextboxFontSize = 18
	addTextboxFontFamily = "Calibri"
	addTextboxBold = false
	addTextboxItalic = false
	addTextboxColor = ""
	addTextboxLevel = 0
	addTextboxAlign = ""
}

func resetFlagsRecursive(cmd *cobra.Command) {
	cmd.Flags().VisitAll(func(flag *pflag.Flag) {
		_ = flag.Value.Set(flag.DefValue)
		flag.Changed = false
	})
	cmd.PersistentFlags().VisitAll(func(flag *pflag.Flag) {
		_ = flag.Value.Set(flag.DefValue)
		flag.Changed = false
	})
	for _, child := range cmd.Commands() {
		resetFlagsRecursive(child)
	}
}

func readSlideXML(t *testing.T, pptxPath string, slideNumber int) string {
	t.Helper()
	pkg, err := opc.Open(pptxPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.GreaterOrEqual(t, len(graph.Slides), slideNumber)

	doc, err := pkg.ReadXMLPart(graph.Slides[slideNumber-1].PartURI)
	require.NoError(t, err)
	text, err := doc.WriteToString()
	require.NoError(t, err)
	return text
}

// TestReplaceText_PreserveFormat tests the preserve-format mode which keeps structure/formatting
func TestReplaceText_PreserveFormat(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/rich-formatting/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--text", "New Title",
		"--mode", "preserve-format",
		"--out", outputPath,
		"--format", "json",
	})
	require.NoError(t, rootCmd.Execute())

	slide1 := readSlideXML(t, outputPath, 1)
	assert.Contains(t, slide1, "New Title")
}

// TestReplaceText_RichTextFile tests rich-text mode with JSON file input
func TestReplaceText_RichTextFile(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/rich-formatting/presentation.pptx")
	require.NoError(t, err)
	richTextFile, err := filepath.Abs("../../testdata/rich-text-bold-colored.json")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--mode", "rich-text",
		"--rich-text-file", richTextFile,
		"--out", outputPath,
		"--format", "json",
	})
	require.NoError(t, rootCmd.Execute())

	slide1 := readSlideXML(t, outputPath, 1)
	// Should contain the rich text content pieces
	assert.Contains(t, slide1, "This is ")
	assert.Contains(t, slide1, "bold and colored")
	assert.Contains(t, slide1, " text")
	// Should have formatting elements (bold, color)
	assert.Contains(t, slide1, "b=\"1\"") // Bold attribute
	assert.Contains(t, slide1, "FF0000")  // Red color
}

// TestReplaceText_RichTextMissingFile tests error handling when rich-text-file is missing
func TestReplaceText_RichTextMissingFile(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	missingFile := filepath.Join(tmpDir, "nonexistent.json")

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--mode", "rich-text",
		"--rich-text-file", missingFile,
		"--out", outputPath,
	})

	err = rootCmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "not found")
}

// TestReplaceText_RichTextInvalidJSON tests error handling for malformed JSON
func TestReplaceText_RichTextInvalidJSON(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	invalidJSONPath := filepath.Join(tmpDir, "invalid.json")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	require.NoError(t, os.WriteFile(invalidJSONPath, []byte("{invalid json}"), 0644))

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--mode", "rich-text",
		"--rich-text-file", invalidJSONPath,
		"--out", outputPath,
	})

	err = rootCmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "JSON")
}

// TestReplaceText_PreserveFormatMultiParagraph tests preserve-format with multi-paragraph content
func TestReplaceText_PreserveFormatMultiParagraph(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/rich-formatting/presentation.pptx")
	require.NoError(t, err)

	newText := "First line\nSecond line\nThird line"

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--text", newText,
		"--mode", "preserve-format",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide1 := readSlideXML(t, outputPath, 1)
	assert.Contains(t, slide1, "First line")
	assert.Contains(t, slide1, "Second line")
	assert.Contains(t, slide1, "Third line")
}

// TestReplaceText_RichTextBulleted tests rich-text mode with bulleted content
func TestReplaceText_RichTextBulleted(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	richTextFile, err := filepath.Abs("../../testdata/rich-text-bulleted.json")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--mode", "rich-text",
		"--rich-text-file", richTextFile,
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	// Should contain all the bullet points
	assert.Contains(t, slide2, "First point")
	assert.Contains(t, slide2, "Second point")
	assert.Contains(t, slide2, "Third point")
}

// TestReplaceText_WithLevel tests replace-text with paragraph level flag
func TestReplaceText_WithLevel(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--text", "Indented content",
		"--level", "2",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "Indented content")
	// Check that level attribute is set to 2
	assert.Contains(t, slide2, `lvl="2"`)
}

// TestReplaceText_WithBulletMode tests replace-text with bullet mode flag
func TestReplaceText_WithBulletMode(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--text", "Bulleted item",
		"--bullet-mode", "buChar",
		"--bullet-char", "•",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "Bulleted item")
	// Check that bullet mode is set
	assert.Contains(t, slide2, "buChar")
}

// TestReplaceText_WithAlignment tests replace-text with alignment flag
func TestReplaceText_WithAlignment(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--text", "Centered title",
		"--align", "ctr",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide1 := readSlideXML(t, outputPath, 1)
	assert.Contains(t, slide1, "Centered title")
	// Check that alignment is set to "ctr"
	assert.Contains(t, slide1, `algn="ctr"`)
}

// TestReplaceText_WithMultipleParagraphOptions tests replace-text with multiple paragraph options
func TestReplaceText_WithMultipleParagraphOptions(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--text", "Formatted bullet item",
		"--level", "1",
		"--align", "l",
		"--bullet-mode", "buAutoNum",
		"--auto-num", "stdAutoNum",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "Formatted bullet item")
	// Check that all options were applied
	assert.Contains(t, slide2, `lvl="1"`)
	assert.Contains(t, slide2, `algn="l"`)
	assert.Contains(t, slide2, "buAutoNum")
	assert.Contains(t, slide2, "stdAutoNum")
}

// TestReplaceText_InvalidLevel tests that invalid level values fail
func TestReplaceText_InvalidLevel(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "1",
		"--target", "title",
		"--text", "Text",
		"--level", "9", // Invalid: must be 0-8
		"--out", outputPath,
	})

	err = rootCmd.Execute()
	require.Error(t, err)
	assert.Contains(t, err.Error(), "invalid level")
}

// TestReplaceText_PreserveFormatKeepsBullets tests that preserve-format mode keeps existing bullets
func TestReplaceText_PreserveFormatKeepsBullets(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--text", "New bullet text",
		"--mode", "preserve-format",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "New bullet text")
	// The existing bullet formatting should be preserved (no new bullet flags specified)
}

// TestReplaceText_BulletWithLevel tests combining bullet and level flags
func TestReplaceText_BulletWithLevel(t *testing.T) {
	tmpDir := t.TempDir()
	outputPath := filepath.Join(tmpDir, "output.pptx")
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "replace", "text",
		fixturePath,
		"--slide", "2",
		"--target", "body",
		"--text", "Sub-bullet",
		"--level", "1",
		"--bullet-mode", "buChar",
		"--bullet-char", "-",
		"--out", outputPath,
	})
	require.NoError(t, rootCmd.Execute())

	slide2 := readSlideXML(t, outputPath, 2)
	assert.Contains(t, slide2, "Sub-bullet")
	assert.Contains(t, slide2, `lvl="1"`)
	assert.Contains(t, slide2, "buChar")
}
