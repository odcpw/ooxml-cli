package inspect

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func openMetadataFixture(t *testing.T) *opc.Package {
	t.Helper()
	path := filepath.Join("..", "..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

func TestReadWorkbookMetadataDefaultsWhenPartsMissing(t *testing.T) {
	pkg := openMetadataFixture(t)
	meta, err := ReadWorkbookMetadata(pkg)
	if err != nil {
		t.Fatalf("ReadWorkbookMetadata failed: %v", err)
	}
	if meta.Title != "" || meta.Company != "" || meta.Manager != "" {
		t.Fatalf("expected empty core/app fields, got %+v", meta)
	}
	if meta.CalcMode != "auto" || meta.FullCalcOnLoad || meta.Iterate {
		t.Fatalf("expected calc defaults, got %+v", meta)
	}
	if meta.IterateCount != 100 || meta.IterateDelta != 0.001 {
		t.Fatalf("expected iterate defaults, got count=%d delta=%v", meta.IterateCount, meta.IterateDelta)
	}
}

func TestPropsURIFallbacks(t *testing.T) {
	pkg := openMetadataFixture(t)
	if got := CorePropsURI(pkg); got != DefaultCorePropsURI {
		t.Fatalf("expected core fallback %q, got %q", DefaultCorePropsURI, got)
	}
	if got := AppPropsURI(pkg); got != DefaultAppPropsURI {
		t.Fatalf("expected app fallback %q, got %q", DefaultAppPropsURI, got)
	}
}
