package validate

import (
	"testing"

	xlsxns "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestValidateChartAxisRequiredChildren(t *testing.T) {
	session := newChartValidationSession(`<c:catAx><c:axId val="1"/><c:scaling/><c:crossAx val="2"/></c:catAx>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "OOXML_CHART_AXIS_REQUIRED")
}

func TestValidateChartAxisPosition(t *testing.T) {
	session := newChartValidationSession(`<c:catAx><c:axId val="1"/><c:scaling/><c:axPos val="sideways"/><c:crossAx val="2"/></c:catAx>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "OOXML_CHART_AXIS_POSITION")
}

func TestValidateChartAxisChildOrder(t *testing.T) {
	session := newChartValidationSession(`<c:catAx><c:axId val="1"/><c:scaling/><c:crossAx val="2"/><c:axPos val="b"/></c:catAx>`)

	diags, err := ValidatePackage(session)
	if err != nil {
		t.Fatalf("ValidatePackage returned error: %v", err)
	}
	assertHasDiagnosticCode(t, diags, "OOXML_CHART_AXIS_ORDER")
}

func newChartValidationSession(axisXML string) *validationTestSession {
	session := newXLSXValidationSession()
	session.parts = append(session.parts, xmlPart("/xl/charts/chart1.xml", xlsxns.ContentTypeChart))
	session.xmlParts["/xl/charts/chart1.xml"] = `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>` + axisXML + `</c:plotArea></c:chart></c:chartSpace>`
	return session
}
