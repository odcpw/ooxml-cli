package mutate

import (
	"bytes"
	"fmt"
	"image"
	"image/color"
	"image/jpeg"
	"path/filepath"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestNewSlideFromLayout_SetTexts(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetTexts: map[string]string{
			"title": "Fresh Title",
			"body":  "Fresh Body",
		},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err = inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	text, err := extract.ExtractText(&extract.ExtractTextRequest{Session: cloned, Graph: graph, SlideNumbers: []int{3}})
	require.NoError(t, err)
	require.Len(t, text.Slides, 1)
	assert.Contains(t, text.Slides[0].Shapes[0].Text.PlainText, "Fresh Title")
	assert.Contains(t, text.Slides[0].Shapes[1].Text.PlainText, "Fresh Body")
	requirePackageValid(t, cloned)
}

func testJPEGBytes(t *testing.T) []byte {
	t.Helper()

	img := image.NewRGBA(image.Rect(0, 0, 1, 1))
	img.Set(0, 0, color.White)

	var buf bytes.Buffer
	require.NoError(t, jpeg.Encode(&buf, img, nil))
	return buf.Bytes()
}

func TestNewSlideFromLayout_SetImage(t *testing.T) {
	fixture := "../../../testdata/pptx/picture-placeholder/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-image.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetImages: []NewSlideImageFill{{
			Target:      "shape:2",
			ImageData:   testJPEGBytes(t),
			ContentType: "image/jpeg",
			FitMode:     FitModeContain,
		}},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err = inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)
	images := slideImages(t, cloned, graph.Slides[2])
	require.NotEmpty(t, images)
	assert.Equal(t, "image/jpeg", images[0].ContentType)
	requirePackageValid(t, cloned)
}

func TestNewSlideFromLayout_UnknownLayout(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	defer pkg.Close()

	_, err = NewSlideFromLayout(&NewSlideFromLayoutRequest{Package: pkg, LayoutPartURI: "/ppt/slideLayouts/missing.xml"})
	require.Error(t, err)
	assert.Contains(t, err.Error(), "layout")
}

func TestNewSlideFromLayout_WithParagraphLevel(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-level.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	level := int32(2)
	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetTexts: map[string]string{
			"body": "Indented content",
		},
		ParagraphOptions: &ParagraphMutationOptions{
			Level: &level,
		},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	// Read the new slide (slide 3)
	newSlideURI := res.NewSlideURI
	doc, err := cloned.ReadXMLPart(newSlideURI)
	require.NoError(t, err)
	// Just check the XML contains the lvl attribute
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that level is set in the XML
	assert.Contains(t, slideXML, `lvl="2"`)
	requirePackageValid(t, cloned)
}

func TestNewSlideFromLayout_WithBulletMode(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-bullet.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	bulletChar := "•"
	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetTexts: map[string]string{
			"body": "Bulleted item",
		},
		BulletOptions: &BulletMutationOptions{
			Mode:      "buChar",
			Character: &bulletChar,
		},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	// Read the new slide
	newSlideURI := res.NewSlideURI
	doc, err := cloned.ReadXMLPart(newSlideURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that bullet mode is set
	assert.Contains(t, slideXML, "buChar")
	requirePackageValid(t, cloned)
}

func TestNewSlideFromLayout_WithAlignment(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-align.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	align := "ctr"
	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetTexts: map[string]string{
			"body": "Centered text",
		},
		ParagraphOptions: &ParagraphMutationOptions{
			Alignment: &align,
		},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	// Read the new slide
	newSlideURI := res.NewSlideURI
	doc, err := cloned.ReadXMLPart(newSlideURI)
	require.NoError(t, err)
	slideXML, err := doc.WriteToString()
	require.NoError(t, err)
	// Check that alignment attribute is set to "ctr"
	assert.Contains(t, slideXML, `algn="ctr"`)
	requirePackageValid(t, cloned)
}

func TestNewSlideFromLayout_SetImageSlot(t *testing.T) {
	fixture := "../../../testdata/pptx/picture-placeholder/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-image-slot.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	jpegData := testJPEGBytes(t)

	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetImages: []NewSlideImageFill{{
			Target:      "slot:pic:1",
			ImageData:   jpegData,
			ContentType: "image/jpeg",
			FitMode:     FitModeContain,
		}},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err = inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	// Verify the image was inserted on the new slide
	images := slideImages(t, cloned, graph.Slides[2])
	require.NotEmpty(t, images, "slot-based image insertion should result in at least one image")
	// The content type may be preserved from the original placeholder
	assert.True(t, images[0].ContentType == "image/jpeg" || images[0].ContentType == "image/png",
		"image should have a valid content type")
	requirePackageValid(t, cloned)
}

func TestNewSlideFromLayout_SetImageSlotFallbackInsert(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-image-slot-fallback.pptx")

	pkg := openMutatePackage(t, fixture)
	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	layoutURI := graph.Slides[1].LayoutPartURI

	jpegData := testJPEGBytes(t)

	// Use a slot that doesn't exist in title-content layout
	// This should fall back to InsertImage with default positioning
	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: layoutURI,
		SetImages: []NewSlideImageFill{{
			Target:      "slot:pic:5",
			ImageData:   jpegData,
			ContentType: "image/jpeg",
			FitMode:     FitModeContain,
		}},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err = inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	// Verify the image was inserted via fallback
	images := slideImages(t, cloned, graph.Slides[2])
	require.NotEmpty(t, images, "fallback image insertion should result in at least one image")
	assert.Equal(t, "image/jpeg", images[0].ContentType)
	requirePackageValid(t, cloned)
}

func TestNewSlideFromLayout_FillsAuthoredPicturePlaceholdersByKeyAndSlot(t *testing.T) {
	fixture := "../../../testdata/pptx/title-content/presentation.pptx"
	outPath := filepath.Join(t.TempDir(), "new-slide-authored-pics.pptx")

	pkg := openMutatePackage(t, fixture)
	clonedLayout, err := CloneLayout(&CloneLayoutRequest{
		Package:       pkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout2.xml",
		NewName:       "Picture Grid",
	})
	require.NoError(t, err)

	_, err = DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		Target:        "title",
	})
	require.NoError(t, err)
	_, err = DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		Target:        "shape:3",
	})
	require.NoError(t, err)
	_, err = DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		Target:        "shape:4",
	})
	require.NoError(t, err)
	_, err = DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		Target:        "shape:5",
	})
	require.NoError(t, err)
	_, err = DeleteLayoutShape(&DeleteLayoutShapeRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		Target:        "shape:6",
	})
	require.NoError(t, err)

	_, err = AddPicturePlaceholder(&AddPicturePlaceholderRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		X:             1000,
		Y:             2000,
		CX:            3000,
		CY:            4000,
		Idx:           -1,
	})
	require.NoError(t, err)
	_, err = AddPicturePlaceholder(&AddPicturePlaceholderRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		X:             5000,
		Y:             6000,
		CX:            7000,
		CY:            8000,
		Idx:           -1,
	})
	require.NoError(t, err)

	jpegData := testJPEGBytes(t)
	res, err := NewSlideFromLayout(&NewSlideFromLayoutRequest{
		Package:       pkg,
		LayoutPartURI: clonedLayout.NewLayoutURI,
		SetImages: []NewSlideImageFill{
			{Target: "pic:0", ImageData: jpegData, ContentType: "image/jpeg", FitMode: FitModeContain},
			{Target: "slot:pic:1", ImageData: jpegData, ContentType: "image/jpeg", FitMode: FitModeContain},
		},
	})
	require.NoError(t, err)
	assert.Equal(t, 3, res.NewSlideNumber)
	require.NoError(t, pkg.SaveAs(outPath))
	pkg.Close()

	cloned := openMutatePackage(t, outPath)
	defer cloned.Close()
	graph, err := inspect.ParsePresentation(cloned)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	images := slideImages(t, cloned, graph.Slides[2])
	require.Len(t, images, 2)
	for _, image := range images {
		assert.Equal(t, "image/jpeg", image.ContentType)
	}

	doc, err := cloned.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	bounds := slidePictureBounds(doc.Root().FindElement("//p:spTree"))
	assert.ElementsMatch(t, []string{
		"1000,2000,3000,4000",
		"5000,6000,7000,8000",
	}, bounds)

	placeholderCount := countPicturePlaceholders(doc.Root().FindElement("//p:spTree"))
	assert.Equal(t, 0, placeholderCount)
	requirePackageValid(t, cloned)
}

func slidePictureBounds(spTree *etree.Element) []string {
	pics := spTree.FindElements("pic")
	bounds := make([]string, 0, len(pics))
	for _, pic := range pics {
		spPr := pic.FindElement("spPr")
		if spPr == nil {
			continue
		}
		xfrm := spPr.FindElement("xfrm")
		if xfrm == nil {
			continue
		}
		off := xfrm.FindElement("off")
		ext := xfrm.FindElement("ext")
		if off == nil || ext == nil {
			continue
		}
		bounds = append(bounds, fmt.Sprintf("%s,%s,%s,%s",
			off.SelectAttrValue("x", ""),
			off.SelectAttrValue("y", ""),
			ext.SelectAttrValue("cx", ""),
			ext.SelectAttrValue("cy", ""),
		))
	}
	return bounds
}

func countPicturePlaceholders(spTree *etree.Element) int {
	count := 0
	for _, sp := range spTree.FindElements("sp") {
		nvSpPr := sp.FindElement("nvSpPr")
		if nvSpPr == nil {
			continue
		}
		nvPr := nvSpPr.FindElement("nvPr")
		if nvPr == nil {
			continue
		}
		ph := nvPr.FindElement("ph")
		if ph == nil {
			continue
		}
		if ph.SelectAttrValue("type", "") == "pic" {
			count++
		}
	}
	return count
}
