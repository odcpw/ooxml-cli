package cli

import (
	"fmt"
	"path/filepath"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestLayoutsCloneCLI(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-clone.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "clone", fixturePath,
		"--layout", "2",
		"--name", "Image Grid Clone",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graph.Layouts, 12)

	var found bool
	for _, layout := range graph.Layouts {
		if layout.Name == "Image Grid Clone" {
			found = true
			break
		}
	}
	assert.True(t, found, "cloned layout should exist")
}

func TestLayoutsRenameCLI(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-rename.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "rename", fixturePath,
		"--layout", "2",
		"--name", "Image Grid Renamed",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	var found bool
	for _, layout := range graph.Layouts {
		if layout.PartURI == "/ppt/slideLayouts/slideLayout2.xml" {
			found = true
			assert.Equal(t, "Image Grid Renamed", layout.Name)
		}
	}
	assert.True(t, found)
}

func TestLayoutsDeleteShapeCLI(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-delete-shape.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "delete-shape", fixturePath,
		"--layout", "2",
		"--target", "title",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	layouts, err := ParsePresentationLayouts(pkg)
	require.NoError(t, err)
	layout := GetLayoutByName(layouts, "Title and Content")
	require.NotNil(t, layout)
	for _, ph := range layout.Placeholders {
		assert.NotEqual(t, "title", ph.Key)
	}
}

func TestLayoutsDeleteShapeCLI_LiteralPlaceholderKey(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-delete-literal-key.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "delete-shape", fixturePath,
		"--layout", "2",
		"--target", "dt:10",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()
	layouts, err := ParsePresentationLayouts(pkg)
	require.NoError(t, err)
	layout := GetLayoutByName(layouts, "Title and Content")
	require.NotNil(t, layout)
	for _, ph := range layout.Placeholders {
		assert.NotEqual(t, "dt:10", ph.Key)
	}
}

func TestLayoutsSetBoundsCLI(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	outPath := filepath.Join(t.TempDir(), "layout-set-bounds.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "set-bounds", fixturePath,
		"--layout", "2",
		"--target", "shape:3",
		"--bounds", "111111,222222,333333,444444",
		"--out", outPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	layouts, err := ParsePresentationLayouts(pkg)
	require.NoError(t, err)
	layout := GetLayoutByName(layouts, "Title and Content")
	require.NotNil(t, layout)

	var found bool
	for _, ph := range layout.Placeholders {
		if ph.Key == "shape:3" {
			found = true
			require.NotNil(t, ph.Geometry)
			require.NotNil(t, ph.Geometry.Bounds)
			assert.Equal(t, int64(111111), ph.Geometry.Bounds.X)
			assert.Equal(t, int64(222222), ph.Geometry.Bounds.Y)
			assert.Equal(t, int64(333333), ph.Geometry.Bounds.CX)
			assert.Equal(t, int64(444444), ph.Geometry.Bounds.CY)
		}
	}
	assert.True(t, found)
}

func TestLayoutAuthoringWorkflow_CloneDeleteAddAndCreateSlide(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	workPath := filepath.Join(tmpDir, "layout-authoring-workflow.pptx")
	finalPath := filepath.Join(tmpDir, "layout-authoring-with-slide.pptx")

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "clone", fixturePath,
		"--layout", "2",
		"--name", "Image Grid Authoring",
		"--out", workPath,
	})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "delete-shape", workPath,
		"--layout", "Image Grid Authoring",
		"--target", "title",
		"--in-place",
	})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "delete-shape", workPath,
		"--layout", "Image Grid Authoring",
		"--target", "shape:3",
		"--in-place",
	})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "add-placeholder", workPath,
		"--layout", "Image Grid Authoring",
		"--type", "pic",
		"--bounds", "1000000,1200000,1800000,1800000",
		"--in-place",
	})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "add-placeholder", workPath,
		"--layout", "Image Grid Authoring",
		"--type", "pic",
		"--bounds", "3000000,1200000,1800000,1800000",
		"--in-place",
	})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", workPath,
		"--layout", "Image Grid Authoring",
		"--out", finalPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(finalPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	assert.GreaterOrEqual(t, len(graph.Slides), 3)

	layouts, err := ParsePresentationLayouts(pkg)
	require.NoError(t, err)
	layout := GetLayoutByName(layouts, "Image Grid Authoring")
	require.NotNil(t, layout)

	var picCount int
	for _, ph := range layout.Placeholders {
		if ph.Key == "title" || ph.Key == "shape:3" {
			t.Fatalf("unexpected placeholder left on authored layout: %s", ph.Key)
		}
		if ph.Role == "pic" {
			picCount++
		}
	}
	assert.GreaterOrEqual(t, picCount, 2)
}

func TestLayoutAuthoringWorkflow_FillAuthoredPicturePlaceholders(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)
	tmpDir := t.TempDir()
	workPath := filepath.Join(tmpDir, "layout-authoring-fill-workflow.pptx")
	finalPath := filepath.Join(tmpDir, "layout-authoring-filled-slide.pptx")
	imagePath := writeTestJPEG(t)

	cmd := newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "layouts", "clone", fixturePath,
		"--layout", "2",
		"--name", "Image Grid Fill",
		"--out", workPath,
	})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "delete-shape", workPath, "--layout", "Image Grid Fill", "--target", "title", "--in-place"})
	require.NoError(t, cmd.Execute())
	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "delete-shape", workPath, "--layout", "Image Grid Fill", "--target", "shape:3", "--in-place"})
	require.NoError(t, cmd.Execute())
	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "delete-shape", workPath, "--layout", "Image Grid Fill", "--target", "shape:4", "--in-place"})
	require.NoError(t, cmd.Execute())
	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "delete-shape", workPath, "--layout", "Image Grid Fill", "--target", "shape:5", "--in-place"})
	require.NoError(t, cmd.Execute())
	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "delete-shape", workPath, "--layout", "Image Grid Fill", "--target", "shape:6", "--in-place"})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "add-placeholder", workPath, "--layout", "Image Grid Fill", "--type", "pic", "--bounds", "1000,2000,3000,4000", "--in-place"})
	require.NoError(t, cmd.Execute())
	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{"pptx", "layouts", "add-placeholder", workPath, "--layout", "Image Grid Fill", "--type", "pic", "--bounds", "5000,6000,7000,8000", "--in-place"})
	require.NoError(t, cmd.Execute())

	cmd = newTestRootCmd(t)
	cmd.SetArgs([]string{
		"pptx", "new-slide-from-layout", workPath,
		"--layout", "Image Grid Fill",
		"--set-image-slot", "pic:0=" + imagePath,
		"--set-image-slot", "pic:1=" + imagePath,
		"--out", finalPath,
	})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(finalPath)
	require.NoError(t, err)
	defer pkg.Close()

	graph, err := inspect.ParsePresentation(pkg)
	require.NoError(t, err)
	require.Len(t, graph.Slides, 3)

	doc, err := pkg.ReadXMLPart(graph.Slides[2].PartURI)
	require.NoError(t, err)
	spTree := doc.Root().FindElement("//p:spTree")
	require.NotNil(t, spTree)
	assert.ElementsMatch(t, []string{
		"1000,2000,3000,4000",
		"5000,6000,7000,8000",
	}, pictureBounds(spTree))
	assert.Equal(t, 0, picturePlaceholderCount(spTree))
}

func pictureBounds(spTree *etree.Element) []string {
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

func picturePlaceholderCount(spTree *etree.Element) int {
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
		if ph != nil && ph.SelectAttrValue("type", "") == "pic" {
			count++
		}
	}
	return count
}
