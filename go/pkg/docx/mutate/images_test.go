package mutate

import (
	"bytes"
	"errors"
	"image"
	"image/jpeg"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// tinyPNG is a 1x1 PNG used as replacement/insert payload in tests.
var tinyPNG = []byte{
	0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
	0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
	0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00,
	0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
	0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB4, 0x00, 0x00,
	0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
}

func tinyJPEG(t *testing.T) []byte {
	t.Helper()
	var buf bytes.Buffer
	if err := jpeg.Encode(&buf, image.NewRGBA(image.Rect(0, 0, 1, 1)), nil); err != nil {
		t.Fatalf("encode jpeg: %v", err)
	}
	return buf.Bytes()
}

func imagesForTest(t *testing.T, pkg opc.PackageSession, documentURI string) *extract.ExtractedImages {
	t.Helper()
	result, err := extract.ExtractImages(&extract.ExtractImagesRequest{Session: pkg, DocumentURI: documentURI})
	if err != nil {
		t.Fatalf("ExtractImages returned error: %v", err)
	}
	return result
}

func TestReplaceImageInPlaceGuardsBlockHash(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-image")
	defer pkg.Close()
	hash := blockHashForTest(t, pkg, documentURI, 2)

	result, err := ReplaceImage(&ReplaceImageRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		Selector:     "1",
		ExpectedHash: hash,
		ImageData:    tinyPNG,
		ContentType:  "image/png",
	})
	if err != nil {
		t.Fatalf("ReplaceImage returned error: %v", err)
	}
	if result.Index != 1 || result.ID != "rId10" || result.BlockIndex != 2 {
		t.Fatalf("unexpected replace result: %+v", result)
	}
	if result.PreviousURI != "/word/media/image1.png" || result.NewURI != "/word/media/image1.png" {
		t.Fatalf("expected in-place media swap, got %+v", result)
	}
}

func TestReplaceImageContentTypeChangeAllocatesNewPart(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-image")
	defer pkg.Close()

	result, err := ReplaceImage(&ReplaceImageRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Selector:    "rId10",
		ImageData:   tinyJPEG(t),
		ContentType: "image/jpeg",
		Width:       1828800,
		Height:      914400,
	})
	if err != nil {
		t.Fatalf("ReplaceImage returned error: %v", err)
	}
	if result.NewURI != "/word/media/image1.jpeg" || result.NewContentType != "image/jpeg" {
		t.Fatalf("unexpected content-type swap: %+v", result)
	}
	if result.Width != 1828800 || result.Height != 914400 {
		t.Fatalf("unexpected resize: %dx%d", result.Width, result.Height)
	}

	images := imagesForTest(t, pkg, documentURI)
	if len(images.Images) != 1 || images.Images[0].MediaURI != "/word/media/image1.jpeg" {
		t.Fatalf("unexpected readback: %+v", images.Images)
	}
	if images.Images[0].Width != 1828800 || images.Images[0].Height != 914400 {
		t.Fatalf("readback extent not updated: %+v", images.Images[0])
	}
}

func TestReplaceImageRejectsMismatchedContentType(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-image")
	defer pkg.Close()

	_, err := ReplaceImage(&ReplaceImageRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Selector:    "rId10",
		ImageData:   tinyPNG,
		ContentType: "image/jpeg",
	})
	if err == nil || !strings.Contains(err.Error(), "image payload does not match content type image/jpeg") {
		t.Fatalf("expected payload mismatch error, got %v", err)
	}
}

func TestReplaceImageRejectsBadHashAndMissing(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-image")
	defer pkg.Close()

	_, err := ReplaceImage(&ReplaceImageRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		Selector:     "1",
		ExpectedHash: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		ImageData:    tinyPNG,
		ContentType:  "image/png",
	})
	if !errors.Is(err, ErrBlockHashMismatch) {
		t.Fatalf("bad hash error = %v, want ErrBlockHashMismatch", err)
	}

	_, err = ReplaceImage(&ReplaceImageRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		Selector:    "99",
		ImageData:   tinyPNG,
		ContentType: "image/png",
	})
	if !errors.Is(err, ErrImageNotFound) {
		t.Fatalf("missing image error = %v, want ErrImageNotFound", err)
	}
}

func TestInsertImageAfterBlockCreatesPartRelAndDrawing(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-image")
	defer pkg.Close()
	hash := blockHashForTest(t, pkg, documentURI, 1)

	result, err := InsertImage(&InsertImageRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		AfterIndex:   1,
		ExpectedHash: hash,
		ImageData:    tinyPNG,
		ContentType:  "image/png",
		Width:        914400,
		Height:       914400,
	})
	if err != nil {
		t.Fatalf("InsertImage returned error: %v", err)
	}
	if result.Index != 2 || result.InsertAfter != 1 || result.AnchorHash != hash {
		t.Fatalf("unexpected insert result: %+v", result)
	}
	if result.MediaURI != "/word/media/image2.png" {
		t.Fatalf("unexpected media uri: %s", result.MediaURI)
	}

	images := imagesForTest(t, pkg, documentURI)
	if len(images.Images) != 2 {
		t.Fatalf("expected 2 images after insert, got %+v", images.Images)
	}
	if images.Images[0].MediaURI != "/word/media/image2.png" || images.Images[0].BlockIndex != 2 {
		t.Fatalf("unexpected first image after insert: %+v", images.Images[0])
	}
}

func TestInsertImageRejectsBadAnchorHash(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-image")
	defer pkg.Close()

	_, err := InsertImage(&InsertImageRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		AfterIndex:   1,
		ExpectedHash: "sha256:0000000000000000000000000000000000000000000000000000000000000000",
		ImageData:    tinyPNG,
		ContentType:  "image/png",
		Width:        914400,
		Height:       914400,
	})
	if !errors.Is(err, ErrBlockHashMismatch) {
		t.Fatalf("bad anchor hash error = %v, want ErrBlockHashMismatch", err)
	}

	_, err = InsertImage(&InsertImageRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		AfterIndex:  99,
		ImageData:   tinyPNG,
		ContentType: "image/png",
		Width:       914400,
		Height:      914400,
	})
	if !errors.Is(err, ErrBlockIndexOutOfRange) {
		t.Fatalf("out of range error = %v, want ErrBlockIndexOutOfRange", err)
	}
}
