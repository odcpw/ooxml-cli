package mutate

import (
	"errors"
	"testing"

	"github.com/beevik/etree"
	docxinspect "github.com/ooxml-cli/ooxml-cli/pkg/docx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

func listFieldsForTest(t *testing.T, pkg opc.PackageSession, documentURI string) *docxinspect.DocumentFields {
	t.Helper()
	listing, err := docxinspect.ListFields(pkg, documentURI)
	if err != nil {
		t.Fatalf("ListFields returned error: %v", err)
	}
	return listing
}

func TestListFieldsSimpleAndComplex(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-fields")
	defer pkg.Close()

	listing := listFieldsForTest(t, pkg, documentURI)
	if len(listing.Fields) != 2 {
		t.Fatalf("field count = %d, want 2: %+v", len(listing.Fields), listing.Fields)
	}

	var simple, complex *docxinspect.Field
	for i := range listing.Fields {
		switch listing.Fields[i].FieldType {
		case docxinspect.FieldTypeSimple:
			simple = &listing.Fields[i]
		case docxinspect.FieldTypeComplex:
			complex = &listing.Fields[i]
		}
	}
	if simple == nil || complex == nil {
		t.Fatalf("expected one simple and one complex field: %+v", listing.Fields)
	}
	if simple.Instruction != "PAGE" || simple.CachedResult != "1" {
		t.Fatalf("simple field = %+v", simple)
	}
	if simple.Location != "body:1" {
		t.Fatalf("simple location = %q", simple.Location)
	}
	if complex.Instruction != "NUMPAGES" || complex.CachedResult != "3" {
		t.Fatalf("complex field = %+v", complex)
	}
	if complex.Location != "header1:1" {
		t.Fatalf("complex location = %q", complex.Location)
	}
}

func TestInsertFieldSimpleIntoBody(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	result, err := InsertField(&InsertFieldRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		BlockIndex:  1,
		FieldCode:   "PAGE",
		ResultText:  "1",
	})
	if err != nil {
		t.Fatalf("InsertField returned error: %v", err)
	}
	if result.FieldType != docxinspect.FieldTypeSimple {
		t.Fatalf("fieldType = %q", result.FieldType)
	}
	if result.Instruction != "PAGE" {
		t.Fatalf("instruction = %q", result.Instruction)
	}
	if !result.KnownCode {
		t.Fatalf("PAGE should be a known code")
	}

	listing := listFieldsForTest(t, pkg, documentURI)
	if len(listing.Fields) != 1 || listing.Fields[0].Instruction != "PAGE" {
		t.Fatalf("readback mismatch: %+v", listing.Fields)
	}
	if listing.Fields[0].CachedResult != "1" {
		t.Fatalf("readback result = %q", listing.Fields[0].CachedResult)
	}
}

func TestInsertFieldUnknownCodeAllowed(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	result, err := InsertField(&InsertFieldRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		BlockIndex:  1,
		FieldCode:   "STYLEREF",
	})
	if err != nil {
		t.Fatalf("InsertField returned error: %v", err)
	}
	if result.KnownCode {
		t.Fatalf("STYLEREF should be flagged as unknown")
	}
}

func TestInsertFieldEmptyCodeRejected(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	_, err := InsertField(&InsertFieldRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		BlockIndex:  1,
		FieldCode:   "  ",
	})
	if !errors.Is(err, ErrInvalidFieldCode) {
		t.Fatalf("err = %v, want ErrInvalidFieldCode", err)
	}
}

func TestInsertFieldOutOfRange(t *testing.T) {
	pkg, documentURI := openFixture(t, "minimal")
	defer pkg.Close()

	_, err := InsertField(&InsertFieldRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		BlockIndex:  99,
		FieldCode:   "PAGE",
	})
	if !errors.Is(err, ErrFieldParaOutOfRange) {
		t.Fatalf("err = %v, want ErrFieldParaOutOfRange", err)
	}
}

func TestSetFieldResultSimple(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-fields")
	defer pkg.Close()

	result, err := SetFieldResult(&SetFieldResultRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		BlockIndex:  1,
		FieldIndex:  0,
		Result:      "42",
	})
	if err != nil {
		t.Fatalf("SetFieldResult returned error: %v", err)
	}
	if result.FieldType != docxinspect.FieldTypeSimple {
		t.Fatalf("fieldType = %q", result.FieldType)
	}
	if result.PreviousResult != "1" || result.CachedResult != "42" {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.Instruction != "PAGE" {
		t.Fatalf("instruction = %q", result.Instruction)
	}

	listing := listFieldsForTest(t, pkg, documentURI)
	for _, f := range listing.Fields {
		if f.FieldType == docxinspect.FieldTypeSimple && f.CachedResult != "42" {
			t.Fatalf("readback simple result = %q, want 42", f.CachedResult)
		}
	}
}

func TestSetFieldResultComplexPreservesBookends(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-fields")
	defer pkg.Close()

	// Resolve the header part URI.
	headerURI := resolveFieldTestHeaderURI(t, pkg, documentURI)

	result, err := SetFieldResult(&SetFieldResultRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		PartURI:     headerURI,
		BlockIndex:  1,
		FieldIndex:  0,
		Result:      "9",
	})
	if err != nil {
		t.Fatalf("SetFieldResult returned error: %v", err)
	}
	if result.FieldType != docxinspect.FieldTypeComplex {
		t.Fatalf("fieldType = %q", result.FieldType)
	}
	if result.PreviousResult != "3" || result.CachedResult != "9" {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.Instruction != "NUMPAGES" {
		t.Fatalf("instruction = %q", result.Instruction)
	}

	// Verify the field round-trips and bookends survive (still a complex field).
	listing := listFieldsForTest(t, pkg, documentURI)
	var foundComplex bool
	for _, f := range listing.Fields {
		if f.FieldType == docxinspect.FieldTypeComplex {
			foundComplex = true
			if f.CachedResult != "9" || f.Instruction != "NUMPAGES" {
				t.Fatalf("readback complex field = %+v", f)
			}
		}
	}
	if !foundComplex {
		t.Fatalf("complex field lost after set-result: %+v", listing.Fields)
	}
}

func TestDOCXFieldsSetResultComplexPreservesSeparateRunWhenResultSharesRun(t *testing.T) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(`<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:r><w:fldChar w:fldCharType="begin"/></w:r>
  <w:r><w:instrText xml:space="preserve"> NUMPAGES </w:instrText></w:r>
  <w:r><w:fldChar w:fldCharType="separate"/><w:t>3</w:t></w:r>
  <w:r><w:fldChar w:fldCharType="end"/></w:r>
</w:p>`); err != nil {
		t.Fatalf("parse paragraph XML: %v", err)
	}
	paragraph := doc.Root()
	fields := locateFieldsInParagraph(paragraph)
	if len(fields) != 1 {
		t.Fatalf("field count = %d, want 1", len(fields))
	}

	setFieldResultText(fields[0], "9")

	fields = locateFieldsInParagraph(paragraph)
	if len(fields) != 1 {
		t.Fatalf("field count after mutation = %d, want 1", len(fields))
	}
	instruction, result := readFieldInPlace(fields[0])
	if instruction != " NUMPAGES " || result != "9" {
		t.Fatalf("field after mutation instruction=%q result=%q, want NUMPAGES/9", instruction, result)
	}
	if fields[0].separateRun == nil {
		t.Fatalf("complex field lost separate run after mutation")
	}
	if got := countFieldCharsOfType(paragraph, "separate"); got != 1 {
		t.Fatalf("separate fldChar count = %d, want 1", got)
	}
	if got := countFieldCharsOfType(paragraph, "end"); got != 1 {
		t.Fatalf("end fldChar count = %d, want 1", got)
	}
}

func TestSetFieldResultHashGuard(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-fields")
	defer pkg.Close()

	_, err := SetFieldResult(&SetFieldResultRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		BlockIndex:   1,
		FieldIndex:   0,
		Result:       "x",
		ExpectedHash: "sha256:deadbeef",
	})
	if !errors.Is(err, ErrFieldHashMismatch) {
		t.Fatalf("err = %v, want ErrFieldHashMismatch", err)
	}

	// The correct hash must succeed.
	good := FieldContentHash("PAGE", "1")
	if _, err := SetFieldResult(&SetFieldResultRequest{
		Package:      pkg,
		DocumentURI:  documentURI,
		BlockIndex:   1,
		FieldIndex:   0,
		Result:       "x",
		ExpectedHash: good,
	}); err != nil {
		t.Fatalf("SetFieldResult with correct hash failed: %v", err)
	}
}

func TestSetFieldResultNotFound(t *testing.T) {
	pkg, documentURI := openFixture(t, "with-fields")
	defer pkg.Close()

	_, err := SetFieldResult(&SetFieldResultRequest{
		Package:     pkg,
		DocumentURI: documentURI,
		BlockIndex:  1,
		FieldIndex:  9,
		Result:      "x",
	})
	if !errors.Is(err, ErrFieldNotFound) {
		t.Fatalf("err = %v, want ErrFieldNotFound", err)
	}
}

func resolveFieldTestHeaderURI(t *testing.T, pkg opc.PackageSession, documentURI string) string {
	t.Helper()
	listing, err := docxinspect.ListHeadersFooters(pkg, documentURI)
	if err != nil {
		t.Fatalf("ListHeadersFooters: %v", err)
	}
	for _, section := range listing.Sections {
		if section.Headers != nil && section.Headers.Default != nil {
			return section.Headers.Default.PartURI
		}
	}
	t.Fatalf("no default header part found")
	return ""
}

func countFieldCharsOfType(root *etree.Element, fldCharType string) int {
	var count int
	for _, fldChar := range namespaces.FindDescendants(root, namespaces.NsW, "fldChar") {
		if got, _ := namespaces.Attr(fldChar, namespaces.NsW, "fldCharType"); got == fldCharType {
			count++
		}
	}
	return count
}
