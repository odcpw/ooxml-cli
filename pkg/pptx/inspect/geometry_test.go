package inspect

import (
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
)

func TestGeometryFixture_Rotation90(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/rotation-90/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open rotation-90 fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Verify geometry with rotation
	if picShape.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	expectedRotation := 90 * 60000 // 90 degrees
	if picShape.Geometry.Rotation != expectedRotation {
		t.Errorf("expected rotation %d, got %d", expectedRotation, picShape.Geometry.Rotation)
	}

	// Flip flags should be false
	if picShape.Geometry.FlipH {
		t.Error("expected FlipH to be false")
	}
	if picShape.Geometry.FlipV {
		t.Error("expected FlipV to be false")
	}
}

func TestGeometryFixture_Rotation45(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/rotation-45/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open rotation-45 fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Verify geometry with rotation
	if picShape.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	expectedRotation := 45 * 60000 // 45 degrees
	if picShape.Geometry.Rotation != expectedRotation {
		t.Errorf("expected rotation %d, got %d", expectedRotation, picShape.Geometry.Rotation)
	}
}

func TestGeometryFixture_FlipH(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/flip-h/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open flip-h fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Verify geometry with flip
	if picShape.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	if !picShape.Geometry.FlipH {
		t.Error("expected FlipH to be true")
	}

	if picShape.Geometry.FlipV {
		t.Error("expected FlipV to be false")
	}
}

func TestGeometryFixture_FlipV(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/flip-v/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open flip-v fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Verify geometry with flip
	if picShape.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	if picShape.Geometry.FlipH {
		t.Error("expected FlipH to be false")
	}

	if !picShape.Geometry.FlipV {
		t.Error("expected FlipV to be true")
	}
}

func TestGeometryFixture_FlipBoth(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/flip-both/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open flip-both fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Verify geometry with both flips
	if picShape.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	if !picShape.Geometry.FlipH {
		t.Error("expected FlipH to be true")
	}

	if !picShape.Geometry.FlipV {
		t.Error("expected FlipV to be true")
	}
}

func TestGeometryFixture_Crop(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/crop/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open crop fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	images := EnumerateImageRelationships("/ppt/slides/slide1.xml", pkg, spTree)

	if len(images) == 0 {
		t.Fatal("expected to find at least one image")
	}

	img := images[0]

	// Verify geometry with crop
	if img.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	if img.Geometry.Crop == nil {
		t.Fatal("expected Crop to be set")
	}

	expectedLeft := 10000
	expectedTop := 20000
	expectedRight := 30000
	expectedBottom := 40000

	if img.Geometry.Crop.Left != expectedLeft {
		t.Errorf("expected left %d, got %d", expectedLeft, img.Geometry.Crop.Left)
	}
	if img.Geometry.Crop.Top != expectedTop {
		t.Errorf("expected top %d, got %d", expectedTop, img.Geometry.Crop.Top)
	}
	if img.Geometry.Crop.Right != expectedRight {
		t.Errorf("expected right %d, got %d", expectedRight, img.Geometry.Crop.Right)
	}
	if img.Geometry.Crop.Bottom != expectedBottom {
		t.Errorf("expected bottom %d, got %d", expectedBottom, img.Geometry.Crop.Bottom)
	}
}

func TestGeometryFixture_RotationAndFlip(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/rotation-and-flip/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open rotation-and-flip fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	shapes := EnumerateShapes(spTree)

	// Find the picture
	var picShape *model.ShapeInfo
	for i := range shapes {
		if shapes[i].Type == model.ShapeTypePic {
			picShape = &shapes[i]
			break
		}
	}

	if picShape == nil {
		t.Fatal("expected to find a pic shape")
	}

	// Verify geometry with both rotation and flip
	if picShape.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	expectedRotation := 90 * 60000
	if picShape.Geometry.Rotation != expectedRotation {
		t.Errorf("expected rotation %d, got %d", expectedRotation, picShape.Geometry.Rotation)
	}

	if !picShape.Geometry.FlipH {
		t.Error("expected FlipH to be true")
	}
}

func TestGeometryFixture_AllProperties(t *testing.T) {
	pkg, err := opc.Open("../../../testdata/pptx/geometry/all-properties/presentation.pptx")
	if err != nil {
		t.Fatalf("failed to open all-properties fixture: %v", err)
	}
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slides/slide1.xml")
	if err != nil {
		t.Fatalf("failed to read slide1.xml: %v", err)
	}

	spTree := doc.FindElement(".//spTree")
	if spTree == nil {
		t.Fatal("spTree not found")
	}

	images := EnumerateImageRelationships("/ppt/slides/slide1.xml", pkg, spTree)

	if len(images) == 0 {
		t.Fatal("expected to find at least one image")
	}

	img := images[0]

	// Verify geometry with all properties
	if img.Geometry == nil {
		t.Fatal("expected Geometry to be set")
	}

	// Check rotation
	expectedRotation := 45 * 60000
	if img.Geometry.Rotation != expectedRotation {
		t.Errorf("expected rotation %d, got %d", expectedRotation, img.Geometry.Rotation)
	}

	// Check flip
	if !img.Geometry.FlipH {
		t.Error("expected FlipH to be true")
	}

	// Check crop
	if img.Geometry.Crop == nil {
		t.Fatal("expected Crop to be set")
	}

	if img.Geometry.Crop.Left != 5000 {
		t.Errorf("expected left 5000, got %d", img.Geometry.Crop.Left)
	}
	if img.Geometry.Crop.Right != 10000 {
		t.Errorf("expected right 10000, got %d", img.Geometry.Crop.Right)
	}
}
