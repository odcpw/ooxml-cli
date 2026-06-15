package mutate

import (
	"fmt"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
)

func TestParseFitMode(t *testing.T) {
	tests := []struct {
		input    string
		expected FitMode
		valid    bool
	}{
		{"contain", FitModeContain, true},
		{"fit", FitModeContain, true},
		{"cover", FitModeCover, true},
		{"crop", FitModeCover, true},
		{"CONTAIN", FitModeContain, true},
		{"COVER", FitModeCover, true},
		{"invalid", "", false},
		{"stretch", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			mode, err := ParseFitMode(tt.input)
			if tt.valid {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
				if mode != tt.expected {
					t.Errorf("expected %v, got %v", tt.expected, mode)
				}
			} else {
				if err == nil {
					t.Errorf("expected error, got nil")
				}
			}
		})
	}
}

func TestValidateImagePayloadRejectsUnsupportedAndMismatchedImages(t *testing.T) {
	if _, err := validateImagePayload("application/octet-stream", []byte("not an image")); err == nil {
		t.Fatal("expected unsupported content type to fail")
	}
	if _, err := validateImagePayload("image/png", []byte("not a png")); err == nil {
		t.Fatal("expected mismatched PNG payload to fail")
	}
	contentType, err := validateImagePayload("image/svg+xml", []byte("<svg/>"))
	if err != nil {
		t.Fatalf("expected svg payload to be accepted without bitmap signature check: %v", err)
	}
	if contentType != "image/svg+xml" {
		t.Fatalf("unexpected normalized content type %q", contentType)
	}
}

func TestReplaceImage_SuccessfulReplacement(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation to get slide reference
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	// Get slide 2 (which has a picture)
	if len(graph.Slides) < 2 {
		t.Fatalf("expected at least 2 slides, got %d", len(graph.Slides))
	}

	slideRef := graph.Slides[1] // slide 2 (0-indexed)

	// Create a simple 1x1 PNG image (minimal valid PNG)
	// This is a valid 1x1 red pixel PNG
	newImage := []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}

	opts := ImageReplaceOptions{
		FitMode:             FitModeContain,
		NewImageData:        newImage,
		NewImageContentType: "image/png",
	}

	// Replace the image using shape ID selector
	selector := &selectors.ShapeIDSelector{ID: 2}
	result, err := ReplaceImage(selector, &slideRef, pkg, opts)
	if err != nil {
		t.Fatalf("failed to replace image: %v", err)
	}

	// Verify the result
	if result.ShapeID != 2 {
		t.Errorf("expected shape ID 2, got %d", result.ShapeID)
	}

	if result.OldContentType != "image/png" {
		t.Errorf("expected old content type image/png, got %s", result.OldContentType)
	}

	if result.NewContentType != "image/png" {
		t.Errorf("expected new content type image/png, got %s", result.NewContentType)
	}

	// Verify the image part was replaced
	imageData, err := pkg.ReadRawPart(result.NewTargetURI)
	if err != nil {
		t.Fatalf("failed to read new image: %v", err)
	}

	if len(imageData) != len(newImage) {
		t.Errorf("expected image size %d, got %d", len(newImage), len(imageData))
	}
}

func TestReplaceImage_ContentTypeChange(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation to get slide reference
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1] // slide 2

	newImage := testJPEGBytes(t)

	opts := ImageReplaceOptions{
		FitMode:             FitModeContain,
		NewImageData:        newImage,
		NewImageContentType: "image/jpeg",
	}

	selector := &selectors.ShapeIDSelector{ID: 2}
	result, err := ReplaceImage(selector, &slideRef, pkg, opts)
	if err != nil {
		t.Fatalf("failed to replace image: %v", err)
	}

	// Verify content type changed
	if result.OldContentType != "image/png" {
		t.Errorf("expected old content type image/png, got %s", result.OldContentType)
	}

	if result.NewContentType != "image/jpeg" {
		t.Errorf("expected new content type image/jpeg, got %s", result.NewContentType)
	}

	// Verify the filename changed (PNG -> JPEG)
	if result.OldTargetURI == result.NewTargetURI {
		t.Errorf("expected URI to change when content type changes")
	}
}

func TestReplaceImage_CoverFitMode(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1] // slide 2

	newImage := []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}

	opts := ImageReplaceOptions{
		FitMode:             FitModeCover,
		NewImageData:        newImage,
		NewImageContentType: "image/png",
	}

	selector := &selectors.ShapeIDSelector{ID: 2}
	result, err := ReplaceImage(selector, &slideRef, pkg, opts)
	if err != nil {
		t.Fatalf("failed to replace image: %v", err)
	}

	// Verify the operation succeeded
	if result.ShapeID != 2 {
		t.Errorf("expected shape ID 2, got %d", result.ShapeID)
	}

	// Read the slide back to verify the fit mode was set to tile (cover)
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		t.Fatalf("failed to read slide: %v", err)
	}

	pic := slideDoc.FindElement(".//pic")
	if pic == nil {
		t.Fatal("picture not found")
	}

	blipFill := pic.FindElement("blipFill")
	if blipFill == nil {
		t.Fatal("blipFill not found")
	}

	// Check for tile element (cover mode)
	tileElem := blipFill.FindElement("tile")
	if tileElem == nil {
		t.Error("expected tile element for cover fit mode")
	}
}

func TestReplaceImage_ShapeNotFound(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1] // slide 2

	newImage := []byte{0x89, 0x50, 0x4e, 0x47}

	opts := ImageReplaceOptions{
		FitMode:             FitModeContain,
		NewImageData:        newImage,
		NewImageContentType: "image/png",
	}

	// Try to replace an image on a non-existent shape
	selector := &selectors.ShapeIDSelector{ID: 9999}
	_, err = ReplaceImage(selector, &slideRef, pkg, opts)
	if err == nil {
		t.Errorf("expected error for non-existent shape, got nil")
	}
}

func TestReplaceImage_InvalidSelector(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1]

	newImage := []byte{0x89, 0x50, 0x4e, 0x47}

	opts := ImageReplaceOptions{
		FitMode:             FitModeContain,
		NewImageData:        newImage,
		NewImageContentType: "image/png",
	}

	// Try with an unsupported selector type (placeholder key)
	selector := &selectors.PlaceholderKeySelector{Key: "title"}
	_, err = ReplaceImage(selector, &slideRef, pkg, opts)
	if err == nil {
		t.Errorf("expected error for unsupported selector type, got nil")
	}
}

func TestGetExtensionForContentType(t *testing.T) {
	tests := []struct {
		contentType string
		expected    string
		wantErr     bool
	}{
		{"image/png", ".png", false},
		{"image/jpeg", ".jpg", false},
		{"image/jpg", ".jpg", false},
		{"image/gif", ".gif", false},
		{"image/bmp", ".bmp", false},
		{"image/tiff", ".tiff", false},
		{"image/webp", ".webp", false},
		{"image/svg+xml", ".svg", false},
		{"application/octet-stream", "", true},
		{"image/unknown", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.contentType, func(t *testing.T) {
			ext, err := getExtensionForContentType(tt.contentType)
			if tt.wantErr {
				if err == nil {
					t.Fatalf("expected error for %q", tt.contentType)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			if ext != tt.expected {
				t.Errorf("expected %q, got %q", tt.expected, ext)
			}
		})
	}
}

func TestInsertImage_AtCoordinates(t *testing.T) {
	// Open a test fixture with a slide
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[0] // slide 1

	// Create a simple 1x1 PNG image
	newImage := []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a,
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41,
		0x54, 0x08, 0x99, 0x63, 0xf8, 0xcf, 0xc0, 0x00,
		0x00, 0x00, 0x03, 0x00, 0x01, 0x49, 0xb4, 0xe8,
		0xe4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
		0x44, 0xae, 0x42, 0x60, 0x82,
	}

	// Insert image at specific coordinates
	const (
		x  = 1000000 // approx 1 inch
		y  = 2000000
		cx = 3000000 // 3 inches
		cy = 2000000 // 2 inches
	)

	result, err := InsertImage(&InsertImageRequest{
		Package:     pkg,
		SlideRef:    &slideRef,
		ImageData:   newImage,
		ContentType: "image/png",
		FitMode:     FitModeContain,
		X:           x,
		Y:           y,
		CX:          cx,
		CY:          cy,
	})

	if err != nil {
		t.Fatalf("failed to insert image: %v", err)
	}

	// Verify result
	if result.ShapeID <= 0 {
		t.Errorf("expected positive shape ID, got %d", result.ShapeID)
	}

	if result.ContentType != "image/png" {
		t.Errorf("expected content type image/png, got %s", result.ContentType)
	}

	if result.RelationshipID == "" {
		t.Errorf("expected non-empty relationship ID")
	}

	// Verify the image part exists
	imageData, err := pkg.ReadRawPart(result.TargetURI)
	if err != nil {
		t.Fatalf("failed to read inserted image: %v", err)
	}

	if len(imageData) != len(newImage) {
		t.Errorf("expected image size %d, got %d", len(newImage), len(imageData))
	}

	// Verify the slide was updated with the new picture element
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		t.Fatalf("failed to read updated slide: %v", err)
	}

	pics := slideDoc.FindElements(".//pic")
	if len(pics) == 0 {
		t.Fatalf("no picture elements found after insert")
	}

	// Find our inserted picture (should be the one with our shape ID)
	var insertedPic *etree.Element
	for _, pic := range pics {
		if id, _ := extractShapeID(pic); id == result.ShapeID {
			insertedPic = pic
			break
		}
	}

	if insertedPic == nil {
		t.Fatalf("inserted picture element not found with shape ID %d", result.ShapeID)
	}

	// Verify geometry
	spPr := insertedPic.FindElement("spPr")
	if spPr == nil {
		t.Fatalf("shape properties not found")
	}

	xfrm := spPr.FindElement("xfrm")
	if xfrm == nil {
		t.Fatalf("transform not found")
	}

	off := xfrm.FindElement("off")
	if off == nil {
		t.Fatalf("offset not found")
	}

	var readX, readY int64
	fmt.Sscanf(off.SelectAttrValue("x", ""), "%d", &readX)
	fmt.Sscanf(off.SelectAttrValue("y", ""), "%d", &readY)

	if readX != x || readY != y {
		t.Errorf("expected offset (%d, %d), got (%d, %d)", x, y, readX, readY)
	}

	ext := xfrm.FindElement("ext")
	if ext == nil {
		t.Fatalf("extent not found")
	}

	var readCX, readCY int64
	fmt.Sscanf(ext.SelectAttrValue("cx", ""), "%d", &readCX)
	fmt.Sscanf(ext.SelectAttrValue("cy", ""), "%d", &readCY)

	if readCX != cx || readCY != cy {
		t.Errorf("expected extent (%d, %d), got (%d, %d)", cx, cy, readCX, readCY)
	}
}

func TestRepositionShape_UpdatePosition(t *testing.T) {
	// Open a test fixture with a picture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1] // slide 2 (has picture)

	// Reposition the picture using shape ID selector
	newX := int64(500000)
	newY := int64(1500000)

	result, err := RepositionShape(&RepositionShapeRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		Selector: &selectors.ShapeIDSelector{ID: 2},
		X:        &newX,
		Y:        &newY,
	})

	if err != nil {
		t.Fatalf("failed to reposition shape: %v", err)
	}

	// Verify result
	if result.ShapeID != 2 {
		t.Errorf("expected shape ID 2, got %d", result.ShapeID)
	}

	if result.NewX != newX || result.NewY != newY {
		t.Errorf("expected new position (%d, %d), got (%d, %d)", newX, newY, result.NewX, result.NewY)
	}

	// Verify that dimensions didn't change
	if result.NewCX != result.OldCX || result.NewCY != result.OldCY {
		t.Errorf("dimensions should not change, but got (%d, %d) vs (%d, %d)",
			result.NewCX, result.NewCY, result.OldCX, result.OldCY)
	}

	// Read the slide and verify the change
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		t.Fatalf("failed to read slide: %v", err)
	}

	// Find the picture element
	pics := slideDoc.FindElements(".//pic")
	var targetPic *etree.Element
	for _, pic := range pics {
		if id, _ := extractShapeID(pic); id == 2 {
			targetPic = pic
			break
		}
	}

	if targetPic == nil {
		t.Fatalf("picture with shape ID 2 not found")
	}

	// Verify geometry was updated
	spPr := targetPic.FindElement("spPr")
	xfrm := spPr.FindElement("xfrm")
	off := xfrm.FindElement("off")

	var readX, readY int64
	fmt.Sscanf(off.SelectAttrValue("x", ""), "%d", &readX)
	fmt.Sscanf(off.SelectAttrValue("y", ""), "%d", &readY)

	if readX != newX || readY != newY {
		t.Errorf("expected position (%d, %d), got (%d, %d)", newX, newY, readX, readY)
	}
}

func TestRepositionShape_UpdateSize(t *testing.T) {
	// Open a test fixture with a picture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1] // slide 2

	// Reposition using name selector
	newCX := int64(4000000)
	newCY := int64(3000000)

	result, err := RepositionShape(&RepositionShapeRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		Selector: &selectors.ShapeNameSelector{Name: "Picture 1"},
		CX:       &newCX,
		CY:       &newCY,
	})

	if err != nil {
		t.Fatalf("failed to reposition shape: %v", err)
	}

	// Verify result
	if result.NewCX != newCX || result.NewCY != newCY {
		t.Errorf("expected new size (%d, %d), got (%d, %d)", newCX, newCY, result.NewCX, result.NewCY)
	}

	// Position should not change
	if result.NewX != result.OldX || result.NewY != result.OldY {
		t.Errorf("position should not change, but got (%d, %d) vs (%d, %d)",
			result.NewX, result.NewY, result.OldX, result.OldY)
	}
}

func TestRepositionShape_NotFound(t *testing.T) {
	// Open a test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1]

	newX := int64(500000)

	// Try to reposition non-existent shape
	_, err = RepositionShape(&RepositionShapeRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		Selector: &selectors.ShapeIDSelector{ID: 9999},
		X:        &newX,
	})

	if err == nil {
		t.Errorf("expected error for non-existent shape, got nil")
	}
}

func TestRepositionShape_InvalidDimensions(t *testing.T) {
	// Open a test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Parse the presentation
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		t.Fatalf("failed to parse presentation: %v", err)
	}

	slideRef := graph.Slides[1]

	// Try to set invalid dimension
	zeroCX := int64(0)
	_, err = RepositionShape(&RepositionShapeRequest{
		Package:  pkg,
		SlideRef: &slideRef,
		Selector: &selectors.ShapeIDSelector{ID: 2},
		CX:       &zeroCX,
	})

	if err == nil {
		t.Errorf("expected error for zero dimension, got nil")
	}
}
