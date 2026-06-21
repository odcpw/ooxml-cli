package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func TestEnumerateImageRelationships_PictureSlide(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the picture)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	// Get the shape tree
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	// Enumerate image relationships
	images := EnumerateImageRelationships("/ppt/slides/slide2.xml", pkg, spTree)

	// Should have 1 image
	if len(images) != 1 {
		t.Errorf("expected 1 image, got %d", len(images))
		return
	}

	// Check the image info
	img := images[0]
	if img.SourcePartURI != "/ppt/slides/slide2.xml" {
		t.Errorf("source part URI: expected /ppt/slides/slide2.xml, got %s", img.SourcePartURI)
	}

	if img.ShapeID != 2 {
		t.Errorf("shape ID: expected 2, got %d", img.ShapeID)
	}

	if img.ShapeName != "Picture 1" {
		t.Errorf("shape name: expected 'Picture 1', got %s", img.ShapeName)
	}

	if img.RelationshipID != "rId2" {
		t.Errorf("relationship ID: expected rId2, got %s", img.RelationshipID)
	}

	if img.TargetURI != "/ppt/media/image1.png" {
		t.Errorf("target URI: expected /ppt/media/image1.png, got %s", img.TargetURI)
	}

	if img.ContentType != "image/png" {
		t.Errorf("content type: expected image/png, got %s", img.ContentType)
	}

	if img.FilePath != "image1.png" {
		t.Errorf("file path: expected image1.png, got %s", img.FilePath)
	}

	if img.FileSize <= 0 {
		t.Errorf("file size: expected > 0, got %d", img.FileSize)
	}
}

func TestEnumerateImageRelationships_NoImages(t *testing.T) {
	// Open the minimal-title test fixture (no images)
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Read slide1
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	// Get the shape tree
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	// Enumerate image relationships (should be empty)
	images := EnumerateImageRelationships("/ppt/slides/slide1.xml", pkg, spTree)

	if len(images) != 0 {
		t.Errorf("expected 0 images, got %d", len(images))
	}
}

func TestEnumerateImageRelationships_NilTree(t *testing.T) {
	// Open any package
	pkg, err := opc.Open("../../../testdata/pptx/minimal-title/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Call with nil tree
	images := EnumerateImageRelationships("/ppt/slides/slide1.xml", pkg, nil)

	if len(images) != 0 {
		t.Errorf("expected 0 images for nil tree, got %d", len(images))
	}
}

func TestExtractedImageInfo_Manifest(t *testing.T) {
	// Open the picture-placeholder test fixture
	pkg, err := opc.Open("../../../testdata/pptx/picture-placeholder/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open presentation: %v", err)
	}
	defer pkg.Close()

	// Read slide2 (which contains the picture)
	doc, err := pkg.ReadXMLPart("/ppt/slides/slide2.xml")
	if err != nil {
		t.Fatalf("failed to read slide2.xml: %v", err)
	}

	// Get the shape tree
	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	// Enumerate image relationships
	images := EnumerateImageRelationships("/ppt/slides/slide2.xml", pkg, spTree)

	// Create a manifest
	manifest := &model.ExtractImagesManifest{
		File:            "picture-placeholder/presentation.pptx",
		SlideNumber:     2,
		OutputDirectory: "/tmp/extracted",
		IncludeLayout:   false,
		ImagesCount:     len(images),
		Images:          images,
	}

	// Verify manifest structure
	if manifest.ImagesCount != 1 {
		t.Errorf("expected 1 image in manifest, got %d", manifest.ImagesCount)
	}

	if len(manifest.Images) != 1 {
		t.Errorf("expected 1 image in list, got %d", len(manifest.Images))
	}

	// Verify manifest image matches
	if manifest.Images[0].ShapeName != "Picture 1" {
		t.Errorf("manifest image shape name: expected 'Picture 1', got %s", manifest.Images[0].ShapeName)
	}
}
