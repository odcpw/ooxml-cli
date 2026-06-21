package extract

import (
	"path/filepath"
	"testing"

	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func TestExtractImagesResolvesInlineBlip(t *testing.T) {
	pkg, documentURI := openImagesFixture(t, "with-image")
	defer pkg.Close()

	result, err := ExtractImages(&ExtractImagesRequest{Session: pkg, DocumentURI: documentURI})
	if err != nil {
		t.Fatalf("ExtractImages returned error: %v", err)
	}
	if len(result.Images) != 1 {
		t.Fatalf("image count = %d, want 1", len(result.Images))
	}
	image := result.Images[0]
	if image.Index != 1 || image.ID != "rId10" || image.BlipID != "rId10" {
		t.Fatalf("unexpected image identity: %+v", image)
	}
	if image.MediaURI != "/word/media/image1.png" || image.ContentType != "image/png" {
		t.Fatalf("unexpected media resolution: %+v", image)
	}
	if image.Width != 914400 || image.Height != 914400 {
		t.Fatalf("unexpected EMU extent: %dx%d", image.Width, image.Height)
	}
	if image.BlockIndex != 2 || image.BlockID != "body.b2" || image.BlockHash == "" {
		t.Fatalf("unexpected block anchoring: %+v", image)
	}
}

func TestExtractImagesEmptyWhenNoDrawings(t *testing.T) {
	pkg, documentURI := openImagesFixture(t, "minimal")
	defer pkg.Close()

	result, err := ExtractImages(&ExtractImagesRequest{Session: pkg, DocumentURI: documentURI})
	if err != nil {
		t.Fatalf("ExtractImages returned error: %v", err)
	}
	if len(result.Images) != 0 {
		t.Fatalf("image count = %d, want 0", len(result.Images))
	}
}

func openImagesFixture(t *testing.T, name string) (*opc.Package, string) {
	t.Helper()
	pkg, err := opc.Open(filepath.Join("..", "..", "..", "testdata", "docx", name, "document.docx"))
	if err != nil {
		t.Fatalf("failed to open fixture %s: %v", name, err)
	}
	documentURI, err := docxinspect.FindMainDocumentPart(pkg)
	if err != nil {
		pkg.Close()
		t.Fatalf("FindMainDocumentPart returned error: %v", err)
	}
	return pkg, documentURI
}
