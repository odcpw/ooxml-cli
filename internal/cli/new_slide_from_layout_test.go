package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestNewSlideFromLayout_WithTexts(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "title=CLI Title",
		"--set-text", "body=CLI Body",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	text, err := extract.ExtractText(&extract.ExtractTextRequest{Session: pkg, Graph: graph, SlideNumbers: []int{3}})
	require.NoError(t, err)
	require.Len(t, text.Slides, 1)
	assert.Equal(t, "CLI Title", text.Slides[0].Shapes[0].Text.PlainText)
	assert.Equal(t, "CLI Body", text.Slides[0].Shapes[1].Text.PlainText)
}

func TestNewSlideFromLayout_WithImage(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-image.pptx")
	imagePath := writeTestJPEG(t)

	pkgForLayout, err := opc.Open(fixturePath)
	require.NoError(t, err)
	graphForLayout, err := inspect.ParsePresentation(pkgForLayout)
	require.NoError(t, err)
	layoutIndex := 1
	for i, layout := range graphForLayout.Layouts {
		if layout.PartURI == graphForLayout.Slides[1].LayoutPartURI {
			layoutIndex = i + 1
			break
		}
	}
	pkgForLayout.Close()

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", strconv.Itoa(layoutIndex),
		"--set-image", "shape:2=" + imagePath,
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	spTree := doc.Root().FindElement("//p:spTree")
	images := inspect.EnumerateImageRelationships(graph.Slides[2].PartURI, pkg, spTree)
	require.NotEmpty(t, images)
	assert.Equal(t, "image/jpeg", images[0].ContentType)
}

func TestNewSlideFromLayout_JSONOutput(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	outPath := filepath.Join(tmpDir, "new-slide.pptx")
	jsonPath := filepath.Join(tmpDir, "result.json")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "new-slide-from-layout", fixturePath, "--layout", "2", "--out", outPath, "--format", "json", "-o", jsonPath})
	require.NoError(t, cmd.Execute())

	data, err := os.ReadFile(jsonPath)
	require.NoError(t, err)
	var result newSlideResult
	require.NoError(t, json.Unmarshal(data, &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Equal(t, outPath, result.Output)
	assert.False(t, result.DryRun)
	assert.Equal(t, "2", result.Layout)
	assert.Equal(t, 3, result.NewSlideNumber)
	assert.NotZero(t, result.NewSlideID)
	assert.NotEmpty(t, result.NewSlideURI)
	require.NotNil(t, result.Destination)
	assert.Equal(t, outPath, result.Destination.File)
	assert.Equal(t, result.NewSlideNumber, result.Destination.Number)
	assert.Equal(t, result.NewSlideURI, result.Destination.PartURI)
	assert.NotEmpty(t, result.Destination.LayoutPartURI)
	assert.Contains(t, result.ReadbackCommand, "pptx slides show")
	assert.Contains(t, result.ReadbackCommand, outPath)
	assert.Contains(t, result.SlidesListCommand, "pptx slides list")
	assert.Contains(t, result.ValidateCommand, "validate --strict")
	assert.Contains(t, result.RenderCommand, "pptx render")

	executeGeneratedOOXMLCommandForXLSXTest(t, result.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForXLSXTest(t, result.ReadbackCommand)
	var show SlidesShowResult
	require.NoError(t, json.Unmarshal([]byte(readback), &show))
	require.Len(t, show.Slides, 1)
	assert.Equal(t, result.NewSlideURI, show.Slides[0].PartURI)

	listOutput := executeGeneratedOOXMLCommandForXLSXTest(t, result.SlidesListCommand)
	var list SlidesListResult
	require.NoError(t, json.Unmarshal([]byte(listOutput), &list))
	require.Len(t, list.Slides, 3)
	assert.Equal(t, result.NewSlideURI, list.Slides[2].PartURI)
}

func TestNewSlideFromLayout_DryRunJSONIncludesReadbackTemplates(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	output, err := executeRootForXLSXTest(t,
		"--format", "json",
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "title=Dry Run",
		"--dry-run",
	)
	require.NoError(t, err)

	var result newSlideResult
	require.NoError(t, json.Unmarshal([]byte(output), &result))
	assert.Equal(t, fixturePath, result.File)
	assert.Empty(t, result.Output)
	assert.True(t, result.DryRun)
	assert.Equal(t, "2", result.Layout)
	assert.Equal(t, 3, result.NewSlideNumber)
	assert.NotZero(t, result.NewSlideID)
	require.NotNil(t, result.Destination)
	assert.Empty(t, result.Destination.File)
	assert.Equal(t, result.NewSlideNumber, result.Destination.Number)
	assert.Equal(t, result.NewSlideURI, result.Destination.PartURI)
	assert.Empty(t, result.ReadbackCommand)
	assert.Empty(t, result.SlidesListCommand)
	assert.Empty(t, result.ValidateCommand)
	assert.Empty(t, result.RenderCommand)
	assert.Contains(t, result.ReadbackCommandTemplate, "pptx slides show")
	assert.Contains(t, result.ReadbackCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.SlidesListCommandTemplate, "pptx slides list")
	assert.Contains(t, result.SlidesListCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.ValidateCommandTemplate, "validate --strict")
	assert.Contains(t, result.ValidateCommandTemplate, "<out.pptx>")
	assert.Contains(t, result.RenderCommandTemplate, "pptx render")
	assert.Contains(t, result.RenderCommandTemplate, "<out.pptx>")

	pkg, err := opc.Open(fixturePath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	assert.Len(t, graph.Slides, 2)
}

func TestNewSlideFromLayout_WithRichText(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-rich.pptx")
	richTextPath := filepath.Join(t.TempDir(), "body.json")

	// Create a rich text JSON file with multi-paragraph styled content
	richTextJSON := `{
		"paragraphs": [
			{
				"text": "First paragraph with bold",
				"segments": [
					{
						"type": "text",
						"text": "First paragraph ",
						"properties": {}
					},
					{
						"type": "text",
						"text": "with bold",
						"properties": {
							"bold": true
						}
					}
				]
			},
			{
				"text": "Second paragraph",
				"segments": [
					{
						"type": "text",
						"text": "Second paragraph"
					}
				]
			}
		],
		"plainText": "First paragraph with bold\nSecond paragraph"
	}`
	require.NoError(t, os.WriteFile(richTextPath, []byte(richTextJSON), 0644))

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-rich-text", "body=" + richTextPath,
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Equal(t, 3, len(graph.Slides))

	// Verify the rich text was applied - should have multiple paragraphs
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slideXML, "First paragraph")
	assert.Contains(t, slideXML, "Second paragraph")
}

func TestNewSlideFromLayout_WithCoordinateImage(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-coord.pptx")
	imagePath := writeTestJPEG(t)

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-image-coords", "914400,914400,1828800,1371600=" + imagePath,
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	spTree := doc.Root().FindElement("//p:spTree")
	images := inspect.EnumerateImageRelationships(graph.Slides[2].PartURI, pkg, spTree)
	require.NotEmpty(t, images)
	assert.Equal(t, "image/jpeg", images[0].ContentType)
}

func TestNewSlideFromLayout_WithInvalidCoordinates(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide.pptx")
	imagePath := writeTestJPEG(t)

	cmd := newTestRootCmd(t)
	// Invalid coordinate format (only 3 values instead of 4)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-image-coords", "914400,914400,1828800=" + imagePath,
		"--out", outPath,
	})
	require.Error(t, cmd.Execute())
}

func TestNewSlideFromLayout_WithParagraphLevel(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-level.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "body=Indented content",
		"--level", "2",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that lvl attribute is set to 2
	assert.Contains(t, slideXML, `lvl="2"`)
}

func TestNewSlideFromLayout_WithBulletMode(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-bullet.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "body=Bulleted item",
		"--bullet-mode", "buChar",
		"--bullet-char", "•",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that bullet mode is set
	assert.Contains(t, slideXML, "buChar")
}

func TestNewSlideFromLayout_WithAlignment(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-align.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "body=Centered text",
		"--align", "ctr",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that alignment attribute is set to "ctr"
	assert.Contains(t, slideXML, `algn="ctr"`)
}

func TestNewSlideFromLayout_WithMultipleParagraphOptions(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-multi.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "body=Formatted bullet item",
		"--level", "1",
		"--align", "l",
		"--bullet-mode", "buAutoNum",
		"--auto-num", "stdAutoNum",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that all options were applied
	assert.Contains(t, slideXML, `lvl="1"`)
	assert.Contains(t, slideXML, `algn="l"`)
	assert.Contains(t, slideXML, "buAutoNum")
	assert.Contains(t, slideXML, "stdAutoNum")
}

func TestNewSlideFromLayout_WithRichTextAndBullets(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-rich-bullet.pptx")
	richTextPath := filepath.Join(t.TempDir(), "body.json")

	// Create a rich text JSON file with multi-paragraph content
	richTextJSON := `{
		"paragraphs": [
			{
				"text": "First point",
				"segments": [
					{
						"type": "text",
						"text": "First point"
					}
				]
			},
			{
				"text": "Second point",
				"segments": [
					{
						"type": "text",
						"text": "Second point"
					}
				]
			}
		],
		"plainText": "First point\nSecond point"
	}`
	require.NoError(t, os.WriteFile(richTextPath, []byte(richTextJSON), 0o644))

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-rich-text", "body=" + richTextPath,
		"--bullet-mode", "buChar",
		"--bullet-char", "-",
		"--level", "0",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that rich text was applied
	assert.Contains(t, slideXML, "First point")
	assert.Contains(t, slideXML, "Second point")
	// Check that bullet options were applied to all paragraphs
	assert.Contains(t, slideXML, "buChar")
}

func TestNewSlideFromLayout_InvalidLevel(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", "2",
		"--set-text", "body=Text",
		"--level", "9", // Invalid: must be 0-8
		"--out", outPath,
	})
	require.Error(t, cmd.Execute())
}

func TestNewSlideFromLayout_WithImageSlotCoverFit(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-image-slot-cover.pptx")
	imagePath := writeTestJPEG(t)

	pkgForLayout, err := opc.Open(fixturePath)
	require.NoError(t, err)
	graphForLayout, err := inspect.ParsePresentation(pkgForLayout)
	require.NoError(t, err)
	layoutIndex := 1
	for i, layout := range graphForLayout.Layouts {
		if layout.PartURI == graphForLayout.Slides[1].LayoutPartURI {
			layoutIndex = i + 1
			break
		}
	}
	pkgForLayout.Close()

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", strconv.Itoa(layoutIndex),
		"--set-image-slot", "pic:1=" + imagePath,
		"--image-fit", "cover",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	assert.Contains(t, slideXML, "tile")
}

func TestNewSlideFromLayout_WithImageSlot(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/picture-placeholder/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "new-slide-image-slot.pptx")
	imagePath := writeTestJPEG(t)

	pkgForLayout, err := opc.Open(fixturePath)
	require.NoError(t, err)
	graphForLayout, err := inspect.ParsePresentation(pkgForLayout)
	require.NoError(t, err)
	layoutIndex := 1
	for i, layout := range graphForLayout.Layouts {
		if layout.PartURI == graphForLayout.Slides[1].LayoutPartURI {
			layoutIndex = i + 1
			break
		}
	}
	pkgForLayout.Close()

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", fixturePath,
		"--layout", strconv.Itoa(layoutIndex),
		"--set-image-slot", "pic:1=" + imagePath,
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	spTree := doc.Root().FindElement("//p:spTree")
	images := inspect.EnumerateImageRelationships(graph.Slides[2].PartURI, pkg, spTree)
	require.NotEmpty(t, images, "slot-based image insertion should result in at least one image")
	// The content type may vary depending on the placeholder
	assert.True(t, images[0].ContentType == "image/jpeg" || images[0].ContentType == "image/png",
		"image should have a valid content type")
}
