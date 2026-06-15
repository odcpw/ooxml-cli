package mutate

import (
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
)

func openMutateMetadataFixture(t *testing.T) *opc.Package {
	t.Helper()
	path := filepath.Join("..", "..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	pkg, err := opc.Open(path)
	if err != nil {
		t.Fatalf("failed to open fixture: %v", err)
	}
	t.Cleanup(func() { pkg.Close() })
	return pkg
}

func strp(s string) *string { return &s }
func boolp(b bool) *bool    { return &b }

func TestUpdateWorkbookMetadataFreshCoreAndApp(t *testing.T) {
	pkg := openMutateMetadataFixture(t)
	res, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package: pkg,
		Updates: WorkbookMetadataUpdate{
			Title:    strp("Hello"),
			Category: strp("Finance"),
			Company:  strp("Acme"),
			Manager:  strp("Carol"),
		},
	})
	if err != nil {
		t.Fatalf("update failed: %v", err)
	}
	if res.UpdatedCount != 4 {
		t.Fatalf("expected 4 updated, got %d (%v)", res.UpdatedCount, res.UpdatedFields)
	}
	meta, err := xlsxinspect.ReadWorkbookMetadata(pkg)
	if err != nil {
		t.Fatalf("read back failed: %v", err)
	}
	if meta.Title != "Hello" || meta.Category != "Finance" || meta.Company != "Acme" || meta.Manager != "Carol" {
		t.Fatalf("round-trip mismatch: %+v", meta)
	}
	// New parts must be relationship-resolvable, not just at the fallback path.
	if xlsxinspect.CorePropsURI(pkg) != "/docProps/core.xml" {
		t.Fatalf("core props relationship not registered: %q", xlsxinspect.CorePropsURI(pkg))
	}
	if xlsxinspect.AppPropsURI(pkg) != "/docProps/app.xml" {
		t.Fatalf("app props relationship not registered: %q", xlsxinspect.AppPropsURI(pkg))
	}
	// app.xml is an xsd:sequence: Manager must precede Company.
	appDoc, err := pkg.ReadXMLPart("/docProps/app.xml")
	if err != nil {
		t.Fatalf("read app.xml failed: %v", err)
	}
	children := appDoc.Root().ChildElements()
	if len(children) != 2 || children[0].Tag != "Manager" || children[1].Tag != "Company" {
		var tags []string
		for _, c := range children {
			tags = append(tags, c.Tag)
		}
		t.Fatalf("expected [Manager Company] order, got %v", tags)
	}
}

func TestUpdateWorkbookMetadataFullCalcOnLoad(t *testing.T) {
	pkg := openMutateMetadataFixture(t)
	if _, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package: pkg,
		Updates: WorkbookMetadataUpdate{FullCalcOnLoad: boolp(true)},
	}); err != nil {
		t.Fatalf("update failed: %v", err)
	}
	meta, err := xlsxinspect.ReadWorkbookMetadata(pkg)
	if err != nil {
		t.Fatalf("read back failed: %v", err)
	}
	if !meta.FullCalcOnLoad || !meta.ForceFullCalc {
		t.Fatalf("expected fullCalcOnLoad+forceFullCalc, got %+v", meta)
	}
}

func TestUpdateWorkbookMetadataCalcMode(t *testing.T) {
	pkg := openMutateMetadataFixture(t)
	if _, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package: pkg,
		Updates: WorkbookMetadataUpdate{CalcMode: strp("manual")},
	}); err != nil {
		t.Fatalf("update failed: %v", err)
	}
	meta, _ := xlsxinspect.ReadWorkbookMetadata(pkg)
	if meta.CalcMode != "manual" {
		t.Fatalf("expected calcMode manual, got %q", meta.CalcMode)
	}
}

func TestUpdateWorkbookMetadataPartialPreservesExisting(t *testing.T) {
	pkg := openMutateMetadataFixture(t)
	if _, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package: pkg,
		Updates: WorkbookMetadataUpdate{Title: strp("First"), Subject: strp("Sub")},
	}); err != nil {
		t.Fatalf("first update failed: %v", err)
	}
	// Second update touches only title; subject must survive.
	if _, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package: pkg,
		Updates: WorkbookMetadataUpdate{Title: strp("Second")},
	}); err != nil {
		t.Fatalf("second update failed: %v", err)
	}
	meta, _ := xlsxinspect.ReadWorkbookMetadata(pkg)
	if meta.Title != "Second" || meta.Subject != "Sub" {
		t.Fatalf("partial update mangled fields: %+v", meta)
	}
}

func TestUpdateWorkbookMetadataGuardMismatch(t *testing.T) {
	pkg := openMutateMetadataFixture(t)
	if _, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package:      pkg,
		Updates:      WorkbookMetadataUpdate{Title: strp("New")},
		ExpectValues: map[string]string{"title": "Old"},
	}); err == nil {
		t.Fatalf("expected guard mismatch error")
	}
}

func TestUpdateWorkbookMetadataGuardSuccess(t *testing.T) {
	pkg := openMutateMetadataFixture(t)
	if _, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package: pkg,
		Updates: WorkbookMetadataUpdate{Title: strp("Old")},
	}); err != nil {
		t.Fatalf("seed update failed: %v", err)
	}
	res, err := UpdateWorkbookMetadata(&UpdateWorkbookMetadataRequest{
		Package:      pkg,
		Updates:      WorkbookMetadataUpdate{Title: strp("New")},
		ExpectValues: map[string]string{"title": "Old"},
	})
	if err != nil {
		t.Fatalf("guard success update failed: %v", err)
	}
	if res.PreviousValues["title"] != "Old" {
		t.Fatalf("expected previous title Old, got %q", res.PreviousValues["title"])
	}
}
