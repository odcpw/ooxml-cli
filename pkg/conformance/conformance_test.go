package conformance

import (
	"bytes"
	"encoding/json"
	"errors"
	"image"
	"image/png"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"testing"
	"time"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	docxns "github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/officecheck"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxns "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestRepairInvariantsCatchWorksheetChildOrder(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dataValidations/><sheetData/></worksheet>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "XLSX_WORKSHEET_CHILD_ORDER")
}

func TestRepairInvariantsCatchSlideChildOrder(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slides/slide1.xml": contentTypePPTXSlide,
	}, map[string]string{
		"/ppt/slides/slide1.xml": `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:timing/><p:cSld/></p:sld>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "PPTX_SLIDE_CHILD_ORDER")
}

func TestRepairInvariantsCatchSlideAnimationTargetProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slides/slide1.xml": contentTypePPTXSlide,
	}, map[string]string{
		"/ppt/slides/slide1.xml": `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name="Group"/></p:nvGrpSpPr>
      <p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/></p:nvSpPr></p:sp>
    </p:spTree>
  </p:cSld>
  <p:timing>
    <p:tnLst><p:par><p:cTn><p:childTnLst>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="99"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="bad"/></p:tgtEl></p:cBhvr></p:set>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt/></p:tgtEl></p:cBhvr></p:set>
    </p:childTnLst></p:cTn></p:par></p:tnLst>
  </p:timing>
</p:sld>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "PPTX_ANIMATION_TARGET_REFERENCE", "missing slide shape id 99")
	assertDiagnosticMessageContains(t, diags, "PPTX_ANIMATION_TARGET_REFERENCE", `invalid spid "bad"`)
	assertDiagnosticMessageContains(t, diags, "PPTX_ANIMATION_TARGET_REFERENCE", "missing required spid")
}

func TestRepairInvariantsAllowSlideAnimationTargets(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slides/slide1.xml": contentTypePPTXSlide,
	}, map[string]string{
		"/ppt/slides/slide1.xml": `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name="Group"/></p:nvGrpSpPr>
      <p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/></p:nvSpPr></p:sp>
    </p:spTree>
  </p:cSld>
  <p:timing>
    <p:tnLst><p:par><p:cTn><p:childTnLst>
      <p:set><p:cBhvr><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:set>
    </p:childTnLst></p:cTn></p:par></p:tnLst>
  </p:timing>
</p:sld>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_ANIMATION_TARGET_REFERENCE")
}

func TestRepairInvariantsCatchSlideLayoutAndMasterProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slideLayouts/slideLayout1.xml": contentTypePPTXSlideLayout,
		"/ppt/slideLayouts/slideLayout2.xml": contentTypePPTXSlideLayout,
		"/ppt/slideLayouts/slideLayout3.xml": contentTypePPTXSlideLayout,
		"/ppt/slideMasters/slideMaster1.xml": contentTypePPTXSlideMaster,
		"/ppt/theme/theme1.xml":              contentTypePPTXTheme,
	}, map[string]string{
		"/ppt/slideLayouts/slideLayout1.xml": `<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:timing/><p:cSld/></p:sldLayout>`,
		"/ppt/slideLayouts/slideLayout2.xml": `<p:notLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
		"/ppt/slideLayouts/slideLayout3.xml": `<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld/></p:sldLayout>`,
		"/ppt/slideMasters/slideMaster1.xml": `<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:txStyles/>
  <p:cSld/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483648"/>
    <p:sldLayoutId id="2147483649" r:id="rIdMissingLayout"/>
    <p:sldLayoutId id="2147483650" r:id="rIdWrongLayout"/>
    <p:sldLayoutId id="2147483651" r:id="rIdExternalLayout"/>
    <p:sldLayoutId id="2147483652" r:id="rIdValidLayout"/>
  </p:sldLayoutIdLst>
</p:sldMaster>`,
		"/ppt/theme/theme1.xml": `<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Office Theme"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/slideLayouts/slideLayout3.xml": {
			{SourceURI: "/ppt/slideLayouts/slideLayout3.xml", ID: "rIdMaster", Type: relTypePPTXSlideMaster, Target: "../slideMasters/slideMaster1.xml"},
		},
		"/ppt/slideMasters/slideMaster1.xml": {
			{SourceURI: "/ppt/slideMasters/slideMaster1.xml", ID: "rIdWrongLayout", Type: relTypeOfficeTheme, Target: "../theme/theme1.xml"},
			{SourceURI: "/ppt/slideMasters/slideMaster1.xml", ID: "rIdExternalLayout", Type: relTypePPTXSlideLayout, Target: "http://example.com/layout.xml", TargetMode: "External"},
			{SourceURI: "/ppt/slideMasters/slideMaster1.xml", ID: "rIdValidLayout", Type: relTypePPTXSlideLayout, Target: "../slideLayouts/slideLayout3.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "PPTX_SLIDE_LAYOUT_CHILD_ORDER")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_LAYOUT_ROOT", "/ppt/slideLayouts/slideLayout2.xml")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_LAYOUT_MASTER_REFERENCE", "no slideMaster relationship")
	assertDiagnosticCode(t, diags, "PPTX_SLIDE_MASTER_CHILD_ORDER")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE", "<p:sldLayoutId")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE", "rIdMissingLayout")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE", "rIdWrongLayout")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE", "rIdExternalLayout")
}

func TestRepairInvariantsAllowSlideLayoutAndMasterReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slideLayouts/slideLayout1.xml": contentTypePPTXSlideLayout,
		"/ppt/slideMasters/slideMaster1.xml": contentTypePPTXSlideMaster,
	}, map[string]string{
		"/ppt/slideLayouts/slideLayout1.xml": `<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld/><p:clrMapOvr/><p:transition/><p:timing/><p:hf/><p:extLst/></p:sldLayout>`,
		"/ppt/slideMasters/slideMaster1.xml": `<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:cSld/><p:clrMap/><p:sldLayoutIdLst><p:sldLayoutId id="2147483648" r:id="rIdLayout"/></p:sldLayoutIdLst><p:txStyles/></p:sldMaster>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/slideLayouts/slideLayout1.xml": {
			{SourceURI: "/ppt/slideLayouts/slideLayout1.xml", ID: "rIdMaster", Type: relTypePPTXSlideMaster, Target: "../slideMasters/slideMaster1.xml"},
		},
		"/ppt/slideMasters/slideMaster1.xml": {
			{SourceURI: "/ppt/slideMasters/slideMaster1.xml", ID: "rIdLayout", Type: relTypePPTXSlideLayout, Target: "../slideLayouts/slideLayout1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "PPTX_SLIDE_LAYOUT_ROOT")
	assertNoDiagnosticCode(t, diags, "PPTX_SLIDE_LAYOUT_CHILD_ORDER")
	assertNoDiagnosticCode(t, diags, "PPTX_SLIDE_LAYOUT_MASTER_REFERENCE")
	assertNoDiagnosticCode(t, diags, "PPTX_SLIDE_MASTER_ROOT")
	assertNoDiagnosticCode(t, diags, "PPTX_SLIDE_MASTER_CHILD_ORDER")
	assertNoDiagnosticCode(t, diags, "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE")
}

func TestRepairInvariantsCatchDrawingAnchorOrderAndRequiredShape(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/drawings/drawing1.xml": contentTypeDrawing,
	}, map[string]string{
		"/xl/drawings/drawing1.xml": `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"><xdr:twoCellAnchor><xdr:from/><xdr:clientData/><xdr:to/></xdr:twoCellAnchor></xdr:wsDr>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "XLSX_DRAWING_ANCHOR_ORDER")
	assertDiagnosticCode(t, diags, "XLSX_DRAWING_ANCHOR_REQUIRED")
}

func TestRepairInvariantsCatchChartPartOrder(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/charts/chart1.xml": contentTypeChart,
	}, map[string]string{
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:externalData/><c:chart/></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CHARTSPACE_CHILD_ORDER")
}

func TestRepairInvariantsCatchNestedChartPartOrder(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/charts/chart1.xml": contentTypeChart,
	}, map[string]string{
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart>
  <c:plotVisOnly/><c:plotArea>
    <c:spPr/>
    <c:barChart><c:axId/><c:barDir/></c:barChart>
    <c:lineChart><c:axId/><c:grouping/></c:lineChart>
    <c:areaChart><c:axId/><c:grouping/></c:areaChart>
    <c:pieChart><c:firstSliceAng/><c:varyColors/></c:pieChart>
    <c:scatterChart><c:axId/><c:scatterStyle/></c:scatterChart>
  </c:plotArea>
</c:chart></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CHART_CHILD_ORDER")
	assertDiagnosticCode(t, diags, "OOXML_PLOTAREA_CHILD_ORDER")
	assertDiagnosticCode(t, diags, "OOXML_BARCHART_CHILD_ORDER")
	assertDiagnosticCode(t, diags, "OOXML_LINECHART_CHILD_ORDER")
	assertDiagnosticCode(t, diags, "OOXML_AREACHART_CHILD_ORDER")
	assertDiagnosticCode(t, diags, "OOXML_PIECHART_CHILD_ORDER")
	assertDiagnosticCode(t, diags, "OOXML_SCATTERCHART_CHILD_ORDER")
}

func TestRepairInvariantsCatchChartExternalDataRelationshipProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/charts/chart1.xml": contentTypeChart,
		"/ppt/charts/chart2.xml": contentTypeChart,
		"/ppt/media/image1.png":  "image/png",
	}, map[string]string{
		"/ppt/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <c:chart/>
  <c:externalData/>
  <c:externalData r:id="rIdMissingWorkbook"/>
  <c:externalData r:id="rIdWrongWorkbookType"/>
  <c:externalData r:id="rIdWrongWorkbookContent"/>
</c:chartSpace>`,
		"/ppt/charts/chart2.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/charts/chart1.xml": {
			{SourceURI: "/ppt/charts/chart1.xml", ID: "rIdWrongWorkbookType", Type: relTypeImage, Target: "../media/image1.png"},
			{SourceURI: "/ppt/charts/chart1.xml", ID: "rIdWrongWorkbookContent", Type: relTypePackage, Target: "chart2.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_EXTERNAL_DATA_REFERENCE", "missing required r:id")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_EXTERNAL_DATA_REFERENCE", "rIdMissingWorkbook")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_EXTERNAL_DATA_REFERENCE", "rIdWrongWorkbookType")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_EXTERNAL_DATA_REFERENCE", "rIdWrongWorkbookContent")
}

func TestRepairInvariantsCatchChartExternalDataCorruptEmbeddedWorkbookBytes(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/charts/chart1.xml":                      contentTypeChart,
		"/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
	}, map[string]string{
		"/ppt/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <c:chart/>
  <c:externalData r:id="rIdWorkbook"/>
</c:chartSpace>`,
	})
	session.rawParts = map[string][]byte{
		"/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx": []byte("not a zip package"),
	}
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/charts/chart1.xml": {
			{SourceURI: "/ppt/charts/chart1.xml", ID: "rIdWorkbook", Type: relTypePackage, Target: "../embeddings/Microsoft_Excel_Sheet1.xlsx"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN")
}

func TestRepairInvariantsAllowChartExternalDataRelationshipReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/charts/chart1.xml":                      contentTypeChart,
		"/ppt/charts/chart2.xml":                      contentTypeChart,
		"/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
	}, map[string]string{
		"/ppt/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <c:chart/>
  <c:externalData r:id="rIdWorkbook"/>
</c:chartSpace>`,
		"/ppt/charts/chart2.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <c:chart/>
  <c:externalData r:id="rIdExternalWorkbook"/>
</c:chartSpace>`,
	})
	session.rawParts = map[string][]byte{
		"/ppt/embeddings/Microsoft_Excel_Sheet1.xlsx": minimalWorkbookFixtureBytes(t),
	}
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/charts/chart1.xml": {
			{SourceURI: "/ppt/charts/chart1.xml", ID: "rIdWorkbook", Type: relTypePackage, Target: "../embeddings/Microsoft_Excel_Sheet1.xlsx"},
		},
		"/ppt/charts/chart2.xml": {
			{SourceURI: "/ppt/charts/chart2.xml", ID: "rIdExternalWorkbook", Type: relTypePackage, Target: "https://example.com/workbook.xlsx", TargetMode: "External"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "OOXML_CHART_EXTERNAL_DATA_REFERENCE")
	assertNoDiagnosticCode(t, diags, "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN")
}

func TestRepairInvariantsCatchChartAxisReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/charts/chart1.xml": contentTypeChart,
	}, map[string]string{
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>
  <c:barChart>
    <c:barDir val="col"/>
    <c:grouping val="clustered"/>
    <c:axId/>
    <c:axId val="999"/>
    <c:axId val="222"/>
    <c:axId val="222"/>
  </c:barChart>
  <c:catAx><c:axId val="111"/><c:crossAx val="999"/></c:catAx>
  <c:valAx><c:axId val="222"/><c:crossAx val="222"/></c:valAx>
  <c:dateAx><c:crossAx val="111"/></c:dateAx>
</c:plotArea></c:chart></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_AXIS_REFERENCE", "missing required <c:axId val>")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_AXIS_REFERENCE", "references missing axis id 999")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_AXIS_REFERENCE", "duplicates axis id 222")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_AXIS_REFERENCE", "crossAx references its own axis id 222")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_AXIS_REFERENCE", "is missing required val")
}

func TestRepairInvariantsAllowChartAxisReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/charts/chart1.xml": contentTypeChart,
		"/xl/charts/chart2.xml": contentTypeChart,
	}, map[string]string{
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>
  <c:barChart>
    <c:barDir val="col"/>
    <c:grouping val="clustered"/>
    <c:axId val="111"/>
    <c:axId val="222"/>
  </c:barChart>
  <c:catAx><c:axId val="111"/><c:crossAx val="222"/></c:catAx>
  <c:valAx><c:axId val="222"/><c:crossAx val="111"/></c:valAx>
</c:plotArea></c:chart></c:chartSpace>`,
		"/xl/charts/chart2.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>
  <c:pieChart><c:varyColors val="1"/></c:pieChart>
</c:plotArea></c:chart></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "OOXML_CHART_AXIS_REFERENCE")
}

func TestRepairInvariantsCatchChartSeriesCacheProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/charts/chart1.xml": contentTypeChart,
	}, map[string]string{
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>
  <c:barChart>
    <c:barDir val="col"/>
    <c:grouping val="clustered"/>
    <c:ser>
      <c:idx val="0"/>
      <c:order val="0"/>
      <c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="3"/><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="0"><c:v>South</c:v></c:pt></c:strCache></c:strRef></c:cat>
      <c:val><c:numRef><c:numCache><c:ptCount val="2"/><c:pt><c:v>42</c:v></c:pt><c:pt idx="-1"><c:v>58</c:v></c:pt></c:numCache></c:numRef></c:val>
    </c:ser>
    <c:ser>
      <c:idx val="0"/>
      <c:order/>
      <c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>42</c:v></c:pt></c:strCache></c:numRef></c:val>
    </c:ser>
    <c:axId val="111"/>
    <c:axId val="222"/>
  </c:barChart>
  <c:catAx><c:axId val="111"/><c:crossAx val="222"/></c:catAx>
  <c:valAx><c:axId val="222"/><c:crossAx val="111"/></c:valAx>
</c:plotArea></c:chart></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "duplicates series idx 0")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "missing required <c:order val>")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "is missing required <c:f>")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "ptCount=3 but contains 2 <c:pt> elements")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "duplicates point idx 0")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "is missing required idx")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "has invalid idx \"-1\"")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_SERIES_CACHE", "has <c:strCache>; expected <c:numCache>")
}

func TestRepairInvariantsAllowChartSeriesCaches(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/charts/chart1.xml": contentTypeChart,
		"/xl/charts/chart1.xml":  contentTypeChart,
	}, map[string]string{
		"/ppt/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>
  <c:barChart>
    <c:barDir val="col"/>
    <c:grouping val="clustered"/>
    <c:ser>
      <c:idx val="0"/>
      <c:order val="0"/>
      <c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="1"><c:v>South</c:v></c:pt></c:strCache></c:strRef></c:cat>
      <c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numCache></c:numRef></c:val>
    </c:ser>
    <c:ser>
      <c:idx val="1"/>
      <c:order val="1"/>
      <c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="1"><c:v>South</c:v></c:pt></c:strCache></c:strRef></c:cat>
      <c:val><c:numRef><c:f>Sheet1!$C$2:$C$3</c:f><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>50</c:v></c:pt><c:pt idx="1"><c:v>65</c:v></c:pt></c:numCache></c:numRef></c:val>
    </c:ser>
    <c:axId val="111"/>
    <c:axId val="222"/>
  </c:barChart>
  <c:catAx><c:axId val="111"/><c:crossAx val="222"/></c:catAx>
  <c:valAx><c:axId val="222"/><c:crossAx val="111"/></c:valAx>
</c:plotArea></c:chart></c:chartSpace>`,
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart><c:plotArea>
  <c:pieChart>
    <c:ser><c:idx val="0"/><c:order val="0"/><c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>
  </c:pieChart>
</c:plotArea></c:chart></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "OOXML_CHART_SERIES_CACHE")
}

func TestRepairInvariantsCatchKnownPartRootMismatches(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/ppt/slides/slide1.xml":    contentTypePPTXSlide,
		"/xl/drawings/drawing1.xml": contentTypeDrawing,
		"/xl/charts/chart1.xml":     contentTypeChart,
		"/ppt/charts/chart1.xml":    contentTypeChart,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<notWorksheet/>`,
		"/ppt/slides/slide1.xml":    `<p:notSlide xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
		"/xl/drawings/drawing1.xml": `<xdr:notDrawing xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"/>`,
		"/xl/charts/chart1.xml":     `<c:notChart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"/>`,
		"/ppt/charts/chart1.xml":    `<c:notChart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"/>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "XLSX_WORKSHEET_ROOT")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_ROOT", "/xl/worksheets/sheet1.xml")
	assertDiagnosticCode(t, diags, "PPTX_SLIDE_ROOT")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_ROOT", "/ppt/slides/slide1.xml")
	assertDiagnosticCode(t, diags, "XLSX_DRAWING_ROOT")
	assertDiagnosticMessageContains(t, diags, "XLSX_DRAWING_ROOT", "/xl/drawings/drawing1.xml")
	assertDiagnosticCode(t, diags, "OOXML_CHART_ROOT")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_ROOT", "/xl/charts/chart1.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_ROOT", "/ppt/charts/chart1.xml")
}

func TestRepairInvariantsCatchKnownPartRootNamespaceMismatches(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/ppt/slides/slide1.xml":    contentTypePPTXSlide,
		"/xl/drawings/drawing1.xml": contentTypeDrawing,
		"/xl/charts/chart1.xml":     contentTypeChart,
		"/ppt/charts/chart1.xml":    contentTypeChart,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="urn:ooxml-cli:bad"><sheetData/></worksheet>`,
		"/ppt/slides/slide1.xml":    `<p:sld xmlns:p="urn:ooxml-cli:bad"><p:cSld/></p:sld>`,
		"/xl/drawings/drawing1.xml": `<xdr:wsDr xmlns:xdr="urn:ooxml-cli:bad"/>`,
		"/xl/charts/chart1.xml":     `<c:chartSpace xmlns:c="urn:ooxml-cli:bad"><c:chart/></c:chartSpace>`,
		"/ppt/charts/chart1.xml":    `<c:chartSpace xmlns:c="urn:ooxml-cli:bad"><c:chart/></c:chartSpace>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "XLSX_WORKSHEET_ROOT")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_ROOT", "urn:ooxml-cli:bad")
	assertDiagnosticCode(t, diags, "PPTX_SLIDE_ROOT")
	assertDiagnosticMessageContains(t, diags, "PPTX_SLIDE_ROOT", "urn:ooxml-cli:bad")
	assertDiagnosticCode(t, diags, "XLSX_DRAWING_ROOT")
	assertDiagnosticMessageContains(t, diags, "XLSX_DRAWING_ROOT", "urn:ooxml-cli:bad")
	assertDiagnosticCode(t, diags, "OOXML_CHART_ROOT")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_ROOT", "/xl/charts/chart1.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_ROOT", "/ppt/charts/chart1.xml")
}

func TestRepairInvariantsCatchHighValuePartRootMismatches(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":                         xlsxns.ContentTypeWorkbook,
		"/xl/sharedStrings.xml":                    xlsxns.ContentTypeSharedStrings,
		"/xl/styles.xml":                           xlsxns.ContentTypeStyles,
		"/xl/tables/table1.xml":                    xlsxns.ContentTypeTable,
		"/xl/pivotTables/pivotTable1.xml":          xlsxns.ContentTypePivotTable,
		"/xl/pivotCache/pivotCacheDefinition1.xml": xlsxns.ContentTypePivotCache,
		"/xl/pivotCache/pivotCacheRecords1.xml":    xlsxns.ContentTypePivotRecords,
		"/xl/calcChain.xml":                        xlsxns.ContentTypeCalcChain,
		"/ppt/presentation.xml":                    contentTypePPTXPresentation,
	}, map[string]string{
		"/xl/workbook.xml":                         `<notWorkbook/>`,
		"/xl/sharedStrings.xml":                    `<notSharedStrings/>`,
		"/xl/styles.xml":                           `<notStyles/>`,
		"/xl/tables/table1.xml":                    `<notTable/>`,
		"/xl/pivotTables/pivotTable1.xml":          `<notPivotTable/>`,
		"/xl/pivotCache/pivotCacheDefinition1.xml": `<notPivotCacheDefinition/>`,
		"/xl/pivotCache/pivotCacheRecords1.xml":    `<notPivotCacheRecords/>`,
		"/xl/calcChain.xml":                        `<notCalcChain/>`,
		"/ppt/presentation.xml":                    `<p:notPresentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_ROOT", "/xl/workbook.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_SHARED_STRINGS_ROOT", "/xl/sharedStrings.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_STYLES_ROOT", "/xl/styles.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_ROOT", "/xl/tables/table1.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_ROOT", "/xl/pivotTables/pivotTable1.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_ROOT", "/xl/pivotCache/pivotCacheDefinition1.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_RECORDS_ROOT", "/xl/pivotCache/pivotCacheRecords1.xml")
	assertDiagnosticMessageContains(t, diags, "XLSX_CALC_CHAIN_ROOT", "/xl/calcChain.xml")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_ROOT", "/ppt/presentation.xml")
}

func TestRepairInvariantsCatchTableDefinitionProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/tables/table1.xml": xlsxns.ContentTypeTable,
		"/xl/tables/table2.xml": xlsxns.ContentTypeTable,
		"/xl/tables/table3.xml": xlsxns.ContentTypeTable,
	}, map[string]string{
		"/xl/tables/table1.xml": `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="0" name="" displayName="" ref="bad">
  <autoFilter/>
  <tableColumns>
    <tableColumn/>
    <tableColumn id="-2" name=""/>
  </tableColumns>
</table>`,
		"/xl/tables/table2.xml": `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="2" name="Table2" displayName="Table2" ref="A1:C4">
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="1" name="Revenue"/>
  </tableColumns>
  <autoFilter ref="A1:B4"/>
</table>`,
		"/xl/tables/table3.xml": `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="3" name="Table3" displayName="Table3" ref="A1:A2"/>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "invalid id")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "missing required name")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "missing required displayName")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "invalid ref")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "<autoFilter> is missing required ref")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "<tableColumns> is missing required count")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "<tableColumn #1> is missing required id")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "<tableColumn #1> is missing required name")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", `invalid id "-2"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "count is 3 but contains 2")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "table ref spans 3 columns but <tableColumns> contains 2")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "does not match table ref")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "duplicates tableColumn id 1")
	assertDiagnosticMessageContains(t, diags, "XLSX_TABLE_DEFINITION", "missing required <tableColumns>")
	assertDiagnosticCode(t, diags, "XLSX_TABLE_CHILD_ORDER")
}

func TestRepairInvariantsAllowTableDefinitions(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/tables/table1.xml": xlsxns.ContentTypeTable,
	}, map[string]string{
		"/xl/tables/table1.xml": `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="A1:C4" headerRowCount="1" totalsRowCount="0">
  <autoFilter ref="A1:C4"/>
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Product"/>
    <tableColumn id="3" name="Revenue"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showRowStripes="1"/>
</table>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_TABLE_DEFINITION")
	assertNoDiagnosticCode(t, diags, "XLSX_TABLE_CHILD_ORDER")
}

func TestRepairInvariantsCatchPivotDefinitionProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":                         xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml":                xlsxns.ContentTypeWorksheet,
		"/xl/pivotTables/pivotTable1.xml":          xlsxns.ContentTypePivotTable,
		"/xl/pivotTables/pivotTable2.xml":          xlsxns.ContentTypePivotTable,
		"/xl/pivotCache/pivotCacheDefinition1.xml": xlsxns.ContentTypePivotCache,
		"/xl/pivotCache/pivotCacheDefinition2.xml": xlsxns.ContentTypePivotCache,
		"/xl/pivotCache/pivotCacheRecords1.xml":    xlsxns.ContentTypePivotRecords,
		"/xl/pivotCache/pivotCacheRecords2.xml":    xlsxns.ContentTypePivotRecords,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Data" sheetId="1" r:id="rIdSheet1"/></sheets>
  <pivotCaches>
    <pivotCache/>
    <pivotCache cacheId="0" r:id="rIdMissingCache"/>
    <pivotCache cacheId="1" r:id="rIdWrongCache"/>
    <pivotCache cacheId="1" r:id="rIdExternalCache"/>
  </pivotCaches>
</workbook>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
		"/xl/pivotTables/pivotTable1.xml": `<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" cacheId="bad">
  <dataFields count="2"><dataField fld="4"/></dataFields>
  <pivotFields count="1"><pivotField/></pivotFields>
  <location ref="bad"/>
</pivotTableDefinition>`,
		"/xl/pivotTables/pivotTable2.xml": `<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="SalesPivot" cacheId="1">
  <location ref="D3:E6"/>
  <pivotFields count="2"><pivotField/><pivotField/></pivotFields>
  <rowFields count="2"><field x="0"/></rowFields>
  <colFields count="1"><field x="99"/></colFields>
  <pageFields count="1"><pageField fld="bad"/></pageFields>
  <dataFields count="1"><dataField fld="2"/></dataFields>
</pivotTableDefinition>`,
		"/xl/pivotCache/pivotCacheDefinition1.xml": `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rIdMissingRecords" recordCount="-1">
  <cacheFields count="2"><cacheField/><cacheField name="Amount"/></cacheFields>
  <cacheSource type="worksheet"><worksheetSource ref="bad"/></cacheSource>
</pivotCacheDefinition>`,
		"/xl/pivotCache/pivotCacheDefinition2.xml": `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cacheSource type="worksheet"/>
  <cacheFields count="3"><cacheField name="Region"/></cacheFields>
</pivotCacheDefinition>`,
		"/xl/pivotCache/pivotCacheRecords1.xml": `<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3"><r/><r/></pivotCacheRecords>`,
		"/xl/pivotCache/pivotCacheRecords2.xml": `<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="bad"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet1", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdWrongCache", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdExternalCache", Type: xlsxns.RelPivotCache, Target: "http://example.com/pivotCache.xml", TargetMode: "External"},
		},
		"/xl/pivotCache/pivotCacheDefinition1.xml": {
			{SourceURI: "/xl/pivotCache/pivotCacheDefinition1.xml", ID: "rIdWrongRecords", Type: xlsxns.RelWorksheet, Target: "../worksheets/sheet1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "missing required cacheId")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", `invalid cacheId "0"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "rIdMissingCache")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "rIdWrongCache")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "external target")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE", "duplicates")
	assertDiagnosticCode(t, diags, "XLSX_PIVOT_TABLE_CHILD_ORDER")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_DEFINITION", "missing required name")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_DEFINITION", `invalid cacheId "bad"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_DEFINITION", "invalid ref")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_DEFINITION", "count is 2 but contains 1")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_DEFINITION", "outside available fields")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_TABLE_DEFINITION", `invalid fld "bad"`)
	assertDiagnosticCode(t, diags, "XLSX_PIVOT_CACHE_CHILD_ORDER")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_DEFINITION", `recordCount "-1"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_RECORDS_REFERENCE", "rIdMissingRecords")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_DEFINITION", "invalid ref")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_DEFINITION", "is missing required name")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_DEFINITION", "missing required <worksheetSource>")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_CACHE_DEFINITION", "count is 3 but contains 1")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_RECORDS_DEFINITION", "count is 3 but contains 2")
	assertDiagnosticMessageContains(t, diags, "XLSX_PIVOT_RECORDS_DEFINITION", `count "bad"`)
}

func TestRepairInvariantsAllowPivotDefinitions(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":                         xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml":                xlsxns.ContentTypeWorksheet,
		"/xl/pivotTables/pivotTable1.xml":          xlsxns.ContentTypePivotTable,
		"/xl/pivotCache/pivotCacheDefinition1.xml": xlsxns.ContentTypePivotCache,
		"/xl/pivotCache/pivotCacheRecords1.xml":    xlsxns.ContentTypePivotRecords,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Data" sheetId="1" r:id="rIdSheet1"/></sheets>
  <pivotCaches><pivotCache cacheId="1" r:id="rIdCache1"/></pivotCaches>
</workbook>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
		"/xl/pivotTables/pivotTable1.xml": `<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" name="SalesPivot" cacheId="1" dataCaption="Values">
  <location ref="D3:E6" firstHeaderRow="1" firstDataRow="2" firstDataCol="1"/>
  <pivotFields count="3"><pivotField axis="axisRow"/><pivotField axis="axisCol"/><pivotField dataField="1"/></pivotFields>
  <rowFields count="1"><field x="0"/></rowFields>
  <colFields count="1"><field x="1"/></colFields>
  <dataFields count="1"><dataField name="Sum of Amount" fld="2"/></dataFields>
  <pivotTableStyleInfo name="PivotStyleLight16"/>
</pivotTableDefinition>`,
		"/xl/pivotCache/pivotCacheDefinition1.xml": `<pivotCacheDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="rIdRecords1" recordCount="2">
  <cacheSource type="worksheet"><worksheetSource ref="A1:C3" sheet="Data"/></cacheSource>
  <cacheFields count="3"><cacheField name="Region"/><cacheField name="Product"/><cacheField name="Amount"/></cacheFields>
</pivotCacheDefinition>`,
		"/xl/pivotCache/pivotCacheRecords1.xml": `<pivotCacheRecords xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2"><r/><r/></pivotCacheRecords>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet1", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdCache1", Type: xlsxns.RelPivotCache, Target: "pivotCache/pivotCacheDefinition1.xml"},
		},
		"/xl/pivotCache/pivotCacheDefinition1.xml": {
			{SourceURI: "/xl/pivotCache/pivotCacheDefinition1.xml", ID: "rIdRecords1", Type: xlsxns.RelPivotRecords, Target: "pivotCacheRecords1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE")
	assertNoDiagnosticCode(t, diags, "XLSX_PIVOT_TABLE_DEFINITION")
	assertNoDiagnosticCode(t, diags, "XLSX_PIVOT_TABLE_CHILD_ORDER")
	assertNoDiagnosticCode(t, diags, "XLSX_PIVOT_CACHE_DEFINITION")
	assertNoDiagnosticCode(t, diags, "XLSX_PIVOT_CACHE_CHILD_ORDER")
	assertNoDiagnosticCode(t, diags, "XLSX_PIVOT_CACHE_RECORDS_REFERENCE")
	assertNoDiagnosticCode(t, diags, "XLSX_PIVOT_RECORDS_DEFINITION")
}

func TestRepairInvariantsCatchCalcChainReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":          xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/worksheets/sheet2.xml": xlsxns.ContentTypeWorksheet,
		"/xl/calcChain.xml":         xlsxns.ContentTypeCalcChain,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="rId1"/>
    <sheet name="Data" sheetId="2" r:id="rId2"/>
  </sheets>
</workbook>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><f>1+1</f><v>2</v></c></row></sheetData>
</worksheet>`,
		"/xl/worksheets/sheet2.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="2"><c r="B2"><v>5</v></c></row></sheetData>
</worksheet>`,
		"/xl/calcChain.xml": `<calcChain xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <c r="A1" i="1"/>
  <c r="B2" i="2"/>
  <c r="C3" i="9"/>
  <c r="bad" i="1"/>
  <c i="1"/>
</calcChain>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rId1", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rId2", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet2.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdCalc", Type: xlsxns.RelCalcChain, Target: "calcChain.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_CALC_CHAIN_REFERENCE", "B2")
	assertDiagnosticMessageContains(t, diags, "XLSX_CALC_CHAIN_REFERENCE", `i="9"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_CALC_CHAIN_REFERENCE", "invalid cell reference")
	assertDiagnosticMessageContains(t, diags, "XLSX_CALC_CHAIN_REFERENCE", "missing required cell reference")
}

func TestRepairInvariantsAllowCalcChainReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":          xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/worksheets/sheet2.xml": xlsxns.ContentTypeWorksheet,
		"/xl/calcChain.xml":         xlsxns.ContentTypeCalcChain,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Summary" sheetId="1" r:id="rId1"/>
    <sheet name="Data" sheetId="20" r:id="rId2"/>
  </sheets>
</workbook>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><f>1+1</f><v>2</v></c></row></sheetData>
</worksheet>`,
		"/xl/worksheets/sheet2.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="2"><c r="B2"><f>SUM(A1:A2)</f><v>5</v></c></row></sheetData>
</worksheet>`,
		"/xl/calcChain.xml": `<calcChain xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <c r="A1" i="1"/>
  <c r="B2" i="20"/>
</calcChain>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rId1", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rId2", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet2.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdCalc", Type: xlsxns.RelCalcChain, Target: "calcChain.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_CALC_CHAIN_REFERENCE")
}

func TestRepairInvariantsCatchSharedStringsCountProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/sharedStrings.xml":  xlsxns.ContentTypeSharedStrings,
		"/xl/sharedStrings2.xml": xlsxns.ContentTypeSharedStrings,
		"/xl/sharedStrings3.xml": xlsxns.ContentTypeSharedStrings,
	}, map[string]string{
		"/xl/sharedStrings.xml": `<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="2" uniqueCount="1">
  <si><t>Alpha</t></si>
  <si><t>Beta</t></si>
</sst>`,
		"/xl/sharedStrings2.xml": `<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="abc" uniqueCount="1">
  <si><t>Alpha</t></si>
</sst>`,
		"/xl/sharedStrings3.xml": `<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="1">
  <si><t>Alpha</t></si>
</sst>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_SHARED_STRINGS_COUNTS", "uniqueCount is 1 but contains 2")
	assertDiagnosticMessageContains(t, diags, "XLSX_SHARED_STRINGS_COUNTS", `count "abc"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_SHARED_STRINGS_COUNTS", "count without required uniqueCount")
}

func TestRepairInvariantsAllowSharedStringsOmittedCountsAndReuseCount(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/sharedStrings.xml":  xlsxns.ContentTypeSharedStrings,
		"/xl/sharedStrings2.xml": xlsxns.ContentTypeSharedStrings,
	}, map[string]string{
		"/xl/sharedStrings.xml": `<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <si><t>Alpha</t></si>
</sst>`,
		"/xl/sharedStrings2.xml": `<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="17" uniqueCount="2">
  <si><t>Alpha</t></si>
  <si><t>Beta</t></si>
</sst>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_SHARED_STRINGS_COUNTS")
}

func TestRepairInvariantsCatchStylesCountProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/styles.xml":  xlsxns.ContentTypeStyles,
		"/xl/styles2.xml": xlsxns.ContentTypeStyles,
	}, map[string]string{
		"/xl/styles.xml": `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="2"><numFmt numFmtId="164" formatCode="0.00%"/></numFmts>
  <cellXfs count="2"><xf numFmtId="0"/></cellXfs>
</styleSheet>`,
		"/xl/styles2.xml": `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <numFmts count="bad"><numFmt numFmtId="164" formatCode="0.00%"/></numFmts>
</styleSheet>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_STYLES_COUNT_MISMATCH", "<numFmts> count is 2 but contains 1")
	assertDiagnosticMessageContains(t, diags, "XLSX_STYLES_COUNT_MISMATCH", "<cellXfs> count is 2 but contains 1")
	assertDiagnosticMessageContains(t, diags, "XLSX_STYLES_COUNT_MISMATCH", `count "bad"`)
}

func TestRepairInvariantsCatchWorksheetStyleReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/styles.xml":            xlsxns.ContentTypeStyles,
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/styles.xml": `<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14"/></cellXfs>
</styleSheet>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
    <row r="1">
      <c r="A1" s="2"><v>1</v></c>
      <c r="B1" s="-1"><v>2</v></c>
      <c r="C1" s="abc"><v>3</v></c>
    </row>
  </sheetData>
</worksheet>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE", "cell A1")
	assertDiagnosticMessageContains(t, diags, "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE", "cell B1")
	assertDiagnosticMessageContains(t, diags, "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE", "cell C1")
}

func TestRepairInvariantsCatchWorksheetStyleReferenceWithoutStyles(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/worksheets/sheet2.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1" s="1"><v>1</v></c></row></sheetData>
</worksheet>`,
		"/xl/worksheets/sheet2.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_CELL_STYLE_REFERENCE", "cell A1")
	assertNoDiagnosticCode(t, diags, "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE")
}

func TestRepairInvariantsCatchInvalidZipTimestamp(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
	})
	session.zipMetas = map[string]*opc.ZipEntryMeta{
		"/xl/worksheets/sheet1.xml": {ModifiedTime: time.Time{}},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_ZIP_TIMESTAMP_INVALID")
}

func TestRepairInvariantsCatchContentTypesPartProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/[Content_Types].xml": "application/xml",
	}, map[string]string{
		"/[Content_Types].xml": `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="xml" ContentType="text/xml"/>
  <Default Extension="rels"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/xml"/>
  <Override PartName="xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
  <Override PartName="/xl/sharedStrings.xml"/>
</Types>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_DEFAULT_DUPLICATE")
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_DEFAULT_REQUIRED")
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_OVERRIDE_DUPLICATE")
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_OVERRIDE_REQUIRED")
}

func TestRepairInvariantsCatchContentTypesRootProblem(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/[Content_Types].xml": "application/xml",
	}, map[string]string{
		"/[Content_Types].xml": `<Types/>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_ROOT")
}

func TestRepairInvariantsCatchContentTypesReadAndParseProblems(t *testing.T) {
	readErrorSession := newInvariantSession(map[string]string{
		contentTypesPartURI: "application/xml",
	}, map[string]string{
		contentTypesPartURI: `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>`,
	})
	readErrorSession.rawReadErrors = map[string]error{
		contentTypesPartURI: errors.New("cannot read content types"),
	}
	readDiags, err := CheckRepairInvariants(readErrorSession)
	if err != nil {
		t.Fatalf("CheckRepairInvariants(read error) returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, readDiags, "OOXML_CONTENT_TYPES_READ_ERROR", "cannot read content types")

	parseErrorSession := newInvariantSession(map[string]string{
		contentTypesPartURI: "application/xml",
	}, map[string]string{
		contentTypesPartURI: `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default`,
	})
	parseDiags, err := CheckRepairInvariants(parseErrorSession)
	if err != nil {
		t.Fatalf("CheckRepairInvariants(parse error) returned error: %v", err)
	}
	assertDiagnosticCode(t, parseDiags, "OOXML_CONTENT_TYPES_PARSE_ERROR")
}

func TestRepairInvariantsCatchContentTypesCoverageProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/[Content_Types].xml": "application/xml",
		"/xl/workbook.xml":     xlsxns.ContentTypeWorkbook,
		"/xl/media/image1.png": "image/png",
	}, map[string]string{
		"/[Content_Types].xml": `<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/missing.xml" ContentType="application/xml"/>
</Types>`,
		"/xl/workbook.xml":     `<workbook/>`,
		"/xl/media/image1.png": "",
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING")
	assertDiagnosticMessageContains(t, diags, "OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING", "/xl/missing.xml")
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPES_PART_UNMAPPED")
	assertDiagnosticMessageContains(t, diags, "OOXML_CONTENT_TYPES_PART_UNMAPPED", "/xl/media/image1.png")
}

func TestRepairInvariantsCatchKnownContentTypeMismatch(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/charts/chart1.xml":             "application/xml",
		"/ppt/slides/slide1.xml":            contentTypePPTXSlide,
		"/ppt/slides/_rels/slide1.xml.rels": "application/xml",
		"/ppt/presProps.xml":                "application/xml",
		"/ppt/viewProps.xml":                "application/xml",
	}, map[string]string{
		"/xl/charts/chart1.xml":             `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
		"/ppt/slides/slide1.xml":            `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld/></p:sld>`,
		"/ppt/slides/_rels/slide1.xml.rels": `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
		"/ppt/presProps.xml":                `<p:presentationPr xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
		"/ppt/viewProps.xml":                `<p:viewPr xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_CONTENT_TYPE_MISMATCH")
	assertDiagnosticMessageContains(t, diags, "OOXML_CONTENT_TYPE_MISMATCH", "/xl/charts/chart1.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_CONTENT_TYPE_MISMATCH", "/ppt/slides/_rels/slide1.xml.rels")
	assertDiagnosticMessageContains(t, diags, "OOXML_CONTENT_TYPE_MISMATCH", "/ppt/presProps.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_CONTENT_TYPE_MISMATCH", "/ppt/viewProps.xml")
}

func TestRepairInvariantsCatchMalformedRelationshipPart(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/_rels/.rels": opc.ContentTypeRelationships,
	}, map[string]string{
		"/_rels/.rels": `<Relationships/>`,
	})

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_RELS_PARSE_ERROR")
	assertDiagnosticMessageContains(t, diags, "OOXML_RELS_PARSE_ERROR", "/_rels/.rels")
}

func TestRepairInvariantsCatchRelationshipPartReadError(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slides/slide1.xml":            contentTypePPTXSlide,
		"/ppt/slides/_rels/slide1.xml.rels": opc.ContentTypeRelationships,
	}, map[string]string{
		"/ppt/slides/slide1.xml":            `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld/></p:sld>`,
		"/ppt/slides/_rels/slide1.xml.rels": `<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>`,
	})
	session.rawReadErrors = map[string]error{
		"/ppt/slides/_rels/slide1.xml.rels": errors.New("cannot read slide relationships"),
	}
	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_RELS_READ_ERROR", "cannot read slide relationships")
}

func TestRepairInvariantsCatchRelationshipClosureProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":                     "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
		"/xl/_rels/workbook.xml.rels":          opc.ContentTypeRelationships,
		"/xl/worksheets/_rels/sheet2.xml.rels": opc.ContentTypeRelationships,
	}, map[string]string{
		"/xl/workbook.xml":                     `<workbook/>`,
		"/xl/_rels/workbook.xml.rels":          `<Relationships/>`,
		"/xl/worksheets/_rels/sheet2.xml.rels": `<Relationships/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rId1", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet", Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rId1", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme", Target: "http://example.com/theme.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdBadMode", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme", Target: "theme/theme1.xml", TargetMode: "Bogus"},
			{SourceURI: "/xl/workbook.xml", ID: "", Type: "", Target: ""},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_TARGET_MISSING")
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_DUPLICATE_ID")
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_EXTERNAL_MODE_MISSING")
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_MISSING_ID")
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_MISSING_TYPE")
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_MISSING_TARGET")
	assertDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_TARGET_MODE")
	assertDiagnosticCode(t, diags, "OOXML_RELS_ORPHANED")
}

func TestRepairInvariantsCatchRelationshipTargetContentTypeMismatch(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":          xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/charts/chart1.xml":     contentTypeChart,
		"/ppt/presentation.xml":     contentTypePPTXPresentation,
		"/ppt/slides/slide1.xml":    contentTypePPTXSlide,
		"/ppt/charts/chart1.xml":    contentTypeChart,
	}, map[string]string{
		"/xl/workbook.xml":          `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
		"/xl/charts/chart1.xml":     `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
		"/ppt/presentation.xml":     `<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
		"/ppt/slides/slide1.xml":    `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld/></p:sld>`,
		"/ppt/charts/chart1.xml":    `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/": {
			{SourceURI: "/", ID: "rIdMainXLSX", Type: xlsxns.RelOfficeDocument, Target: "xl/charts/chart1.xml"},
			{SourceURI: "/", ID: "rIdMainPPTX", Type: xlsxns.RelOfficeDocument, Target: "ppt/slides/slide1.xml"},
		},
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet", Type: xlsxns.RelWorksheet, Target: "charts/chart1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdStyles", Type: xlsxns.RelStyles, Target: "worksheets/sheet1.xml"},
		},
		"/ppt/presentation.xml": {
			{SourceURI: "/ppt/presentation.xml", ID: "rIdMaster", Type: relTypePPTXSlideMaster, Target: "slides/slide1.xml"},
		},
		"/ppt/slides/slide1.xml": {
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdLayout", Type: relTypePPTXSlideLayout, Target: "../charts/chart1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "rIdMainXLSX")
	assertDiagnosticMessageContains(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "rIdMainPPTX")
	assertDiagnosticMessageContains(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "/xl/charts/chart1.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "/xl/worksheets/sheet1.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "/ppt/slides/slide1.xml")
	assertDiagnosticMessageContains(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE", "/ppt/charts/chart1.xml")
}

func TestRepairInvariantsAllowDOCXRelationshipTargetContentTypes(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/word/document.xml":  docxns.ContentTypeDocument,
		"/word/styles.xml":    docxns.ContentTypeStyles,
		"/word/numbering.xml": docxns.ContentTypeNumbering,
		"/word/header1.xml":   docxns.ContentTypeHeader,
		"/word/footer1.xml":   docxns.ContentTypeFooter,
		"/word/comments.xml":  docxns.ContentTypeComments,
	}, map[string]string{
		"/word/document.xml":  `<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body></w:document>`,
		"/word/styles.xml":    `<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>`,
		"/word/numbering.xml": `<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>`,
		"/word/header1.xml":   `<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:hdr>`,
		"/word/footer1.xml":   `<w:ftr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:p/></w:ftr>`,
		"/word/comments.xml":  `<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/": {
			{SourceURI: "/", ID: "rIdDocument", Type: docxns.RelOfficeDocument, Target: "word/document.xml"},
		},
		"/word/document.xml": {
			{SourceURI: "/word/document.xml", ID: "rStyles", Type: docxns.RelStyles, Target: "styles.xml"},
			{SourceURI: "/word/document.xml", ID: "rNumbering", Type: docxns.RelNumbering, Target: "numbering.xml"},
			{SourceURI: "/word/document.xml", ID: "rHeader", Type: docxns.RelHeader, Target: "header1.xml"},
			{SourceURI: "/word/document.xml", ID: "rFooter", Type: docxns.RelFooter, Target: "footer1.xml"},
			{SourceURI: "/word/document.xml", ID: "rComments", Type: docxns.RelComments, Target: "comments.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE")
}

func TestRepairInvariantsCatchXMLParseErrors(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>`,
	})
	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_XML_PARSE_ERROR", "/xl/worksheets/sheet1.xml")
}

func TestRepairInvariantsCatchWorkbookSheetReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":      xlsxns.ContentTypeWorkbook,
		"/xl/charts/chart1.xml": contentTypeChart,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="NoRel" sheetId="1"/>
    <sheet name="MissingRel" sheetId="2" r:id="rIdMissing"/>
    <sheet name="WrongType" sheetId="3" r:id="rIdChart"/>
    <sheet name="External" sheetId="4" r:id="rIdExternal"/>
  </sheets>
</workbook>`,
		"/xl/charts/chart1.xml": `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdChart", Type: xlsxns.RelChart, Target: "charts/chart1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdExternal", Type: xlsxns.RelWorksheet, Target: "http://example.com/sheet.xml", TargetMode: "External"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_SHEET_REFERENCE", "NoRel")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_SHEET_REFERENCE", "rIdMissing")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_SHEET_REFERENCE", "rIdChart")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKBOOK_SHEET_REFERENCE", "external target")
}

func TestRepairInvariantsAllowWorkbookChartSheetReference(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":           xlsxns.ContentTypeWorkbook,
		"/xl/chartsheets/sheet1.xml": contentTypeXLSXChartSheet,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Chart Sheet" sheetId="1" r:id="rIdChartSheet"/>
  </sheets>
</workbook>`,
		"/xl/chartsheets/sheet1.xml": `<chartsheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdChartSheet", Type: relTypeXLSXChartSheet, Target: "chartsheets/sheet1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_WORKBOOK_SHEET_REFERENCE")
}

func TestRepairInvariantsCatchWorkbookDefinedNameProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":          xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/worksheets/sheet2.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rIdSheet1"/>
    <sheet name="Report Data" sheetId="2" r:id="rIdSheet2"/>
  </sheets>
  <definedNames>
    <definedName>Sheet1!$A$1</definedName>
    <definedName name="BadScope" localSheetId="2">Sheet1!$A$1</definedName>
    <definedName name="BadScopeText" localSheetId="abc">Sheet1!$A$1</definedName>
    <definedName name="Sales">Sheet1!$A$1</definedName>
    <definedName name="sales">Sheet1!$B$1</definedName>
    <definedName name="MissingSheet">'Missing Sheet'!$A$1</definedName>
    <definedName name="BrokenRef">#REF!</definedName>
    <definedName name="Empty"></definedName>
  </definedNames>
</workbook>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
		"/xl/worksheets/sheet2.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet1", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet2", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet2.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_REQUIRED", "missing required name")
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_SCOPE", `localSheetId 2`)
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_SCOPE", `localSheetId "abc"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_DUPLICATE", `name="sales"`)
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_REFERENCE", "Missing Sheet")
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_REFERENCE", "#REF!")
	assertDiagnosticMessageContains(t, diags, "XLSX_DEFINED_NAME_REQUIRED", "empty formula text")
}

func TestRepairInvariantsAllowWorkbookDefinedNames(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/workbook.xml":          xlsxns.ContentTypeWorkbook,
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/worksheets/sheet2.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/workbook.xml": `<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Sheet1" sheetId="1" r:id="rIdSheet1"/>
    <sheet name="Report Data" sheetId="2" r:id="rIdSheet2"/>
  </sheets>
  <definedNames>
    <definedName name="_xlnm.Print_Area" localSheetId="0">'Report Data'!$A$1:$D$20</definedName>
    <definedName name="Sales">Sheet1!$A$1</definedName>
    <definedName name="LocalName" localSheetId="0">Sheet1!$A$1</definedName>
    <definedName name="LocalName" localSheetId="1">'Report Data'!$B$2</definedName>
    <definedName name="FormulaRef">SUM(Sheet1!$A$1,'Report Data'!$B$2)</definedName>
    <definedName name="StructuredRef">Table1[#All]</definedName>
    <definedName name="ExternalRef">[Book.xlsx]Other!$A$1</definedName>
    <definedName name="StringConstant">"#REF!"</definedName>
  </definedNames>
</workbook>`,
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
		"/xl/worksheets/sheet2.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData/></worksheet>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/workbook.xml": {
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet1", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet1.xml"},
			{SourceURI: "/xl/workbook.xml", ID: "rIdSheet2", Type: xlsxns.RelWorksheet, Target: "worksheets/sheet2.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_REQUIRED")
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_SCOPE")
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_DUPLICATE")
	assertNoDiagnosticCode(t, diags, "XLSX_DEFINED_NAME_REFERENCE")
}

func TestRepairInvariantsCatchWorksheetRelationshipReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml":       xlsxns.ContentTypeWorksheet,
		"/xl/drawings/drawing1.xml":       xlsxns.ContentTypeDrawing,
		"/xl/drawings/vmlDrawing1.vml":    xlsxns.ContentTypeVml,
		"/xl/tables/table1.xml":           xlsxns.ContentTypeTable,
		"/xl/pivotTables/pivotTable1.xml": xlsxns.ContentTypePivotTable,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <drawing/>
  <legacyDrawing r:id="rIdMissingLegacy"/>
  <tableParts count="4">
    <tablePart/>
    <tablePart r:id="rIdMissingTable"/>
    <tablePart r:id="rIdWrongTable"/>
    <tablePart r:id="rIdExternalTable"/>
  </tableParts>
  <pivotTableDefinition r:id="rIdWrongPivot"/>
</worksheet>`,
		"/xl/drawings/drawing1.xml":       `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"/>`,
		"/xl/drawings/vmlDrawing1.vml":    `<xml/>`,
		"/xl/tables/table1.xml":           `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Table1" displayName="Table1" ref="A1:A2"/>`,
		"/xl/pivotTables/pivotTable1.xml": `<pivotTableDefinition xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/worksheets/sheet1.xml": {
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdWrongTable", Type: xlsxns.RelDrawing, Target: "../drawings/drawing1.xml"},
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdExternalTable", Type: xlsxns.RelTable, Target: "http://example.com/table.xml", TargetMode: "External"},
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdWrongPivot", Type: xlsxns.RelTable, Target: "../tables/table1.xml"},
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdExternalPivot", Type: xlsxns.RelPivotTable, Target: "http://example.com/pivot.xml", TargetMode: "External"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE", "<drawing>")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE", "rIdMissingLegacy")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE", "<tablePart>")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE", "rIdMissingTable")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE", "rIdWrongTable")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE", "external target")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_PIVOT_REFERENCE", "rIdWrongPivot")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_PIVOT_REFERENCE", "rIdExternalPivot")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_PIVOT_REFERENCE", "not a valid worksheet child")
}

func TestRepairInvariantsCatchWorksheetHyperlinkReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/drawings/drawing1.xml": xlsxns.ContentTypeDrawing,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <hyperlinks>
    <hyperlink ref="A1" r:id="rIdMissingHyperlink"/>
    <hyperlink ref="A2" r:id="rIdWrongHyperlink"/>
    <hyperlink ref="A3" r:id="rIdInternalHyperlink"/>
    <hyperlink ref="A4" location="Sheet2!A1"/>
  </hyperlinks>
</worksheet>`,
		"/xl/drawings/drawing1.xml": `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/worksheets/sheet1.xml": {
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdWrongHyperlink", Type: xlsxns.RelDrawing, Target: "../drawings/drawing1.xml"},
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdInternalHyperlink", Type: xlsxns.RelHyperlink, Target: "Sheet2!A1"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_HYPERLINK_REFERENCE", "rIdMissingHyperlink")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_HYPERLINK_REFERENCE", "rIdWrongHyperlink")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_HYPERLINK_REFERENCE", "rIdInternalHyperlink")
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_HYPERLINK_REFERENCE", "TargetMode")
}

func TestRepairInvariantsAllowWorksheetHyperlinkReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <hyperlinks>
    <hyperlink ref="A1" r:id="rIdExternalHyperlink"/>
    <hyperlink ref="A2" location="Sheet2!A1"/>
  </hyperlinks>
</worksheet>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/worksheets/sheet1.xml": {
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rIdExternalHyperlink", Type: xlsxns.RelHyperlink, Target: "https://example.com", TargetMode: "External"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "XLSX_WORKSHEET_HYPERLINK_REFERENCE")
}

func TestRepairInvariantsCatchWorksheetTablePartsCountMismatch(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/xl/worksheets/sheet1.xml": xlsxns.ContentTypeWorksheet,
		"/xl/tables/table1.xml":     xlsxns.ContentTypeTable,
	}, map[string]string{
		"/xl/worksheets/sheet1.xml": `<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheetData/>
  <tableParts count="2"><tablePart r:id="rId1"/></tableParts>
</worksheet>`,
		"/xl/tables/table1.xml": `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Table1" displayName="Table1" ref="A1:A2"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/worksheets/sheet1.xml": {
			{SourceURI: "/xl/worksheets/sheet1.xml", ID: "rId1", Type: xlsxns.RelTable, Target: "../tables/table1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "XLSX_WORKSHEET_TABLEPARTS_COUNT", "contains 1")
	assertNoDiagnosticCode(t, diags, "XLSX_WORKSHEET_RELATIONSHIP_REFERENCE")
}

func TestRepairInvariantsCatchChartRelationshipReferenceProblems(t *testing.T) {
	drawingXML := `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:graphicFrame><a:graphic><a:graphicData><c:chart/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rIdMissingChart"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rIdWrongChart"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rIdExternalChart"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>
</xdr:wsDr>`
	session := newInvariantSession(map[string]string{
		"/xl/drawings/drawing1.xml":     xlsxns.ContentTypeDrawing,
		"/xl/tables/table1.xml":         xlsxns.ContentTypeTable,
		"/ppt/slides/slide1.xml":        contentTypePPTXSlide,
		"/ppt/slideLayouts/layout1.xml": contentTypePPTXSlideLayout,
	}, map[string]string{
		"/xl/drawings/drawing1.xml": drawingXML,
		"/xl/tables/table1.xml":     `<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Table1" displayName="Table1" ref="A1:A2"/>`,
		"/ppt/slides/slide1.xml": `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree><p:graphicFrame><a:graphic><a:graphicData><c:chart r:id="rIdWrongPptChart"/></a:graphicData></a:graphic></p:graphicFrame></p:spTree></p:cSld>
</p:sld>`,
		"/ppt/slideLayouts/layout1.xml": `<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/xl/drawings/drawing1.xml": {
			{SourceURI: "/xl/drawings/drawing1.xml", ID: "rIdWrongChart", Type: xlsxns.RelTable, Target: "../tables/table1.xml"},
			{SourceURI: "/xl/drawings/drawing1.xml", ID: "rIdExternalChart", Type: xlsxns.RelChart, Target: "http://example.com/chart.xml", TargetMode: "External"},
		},
		"/ppt/slides/slide1.xml": {
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdWrongPptChart", Type: relTypePPTXSlideLayout, Target: "../slideLayouts/layout1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_RELATIONSHIP_REFERENCE", "<c:chart>")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_RELATIONSHIP_REFERENCE", "rIdMissingChart")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_RELATIONSHIP_REFERENCE", "rIdWrongChart")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_RELATIONSHIP_REFERENCE", "external target")
	assertDiagnosticMessageContains(t, diags, "OOXML_CHART_RELATIONSHIP_REFERENCE", "rIdWrongPptChart")
}

func TestRepairInvariantsCatchDrawingMediaRelationshipProblems(t *testing.T) {
	drawingXML := `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:pic><xdr:blipFill><a:blip r:embed="rIdWrongXlsxImageType"/></xdr:blipFill></xdr:pic><xdr:clientData/></xdr:twoCellAnchor>
</xdr:wsDr>`
	session := newInvariantSession(map[string]string{
		"/ppt/slides/slide1.xml":             contentTypePPTXSlide,
		"/ppt/slideLayouts/slideLayout1.xml": contentTypePPTXSlideLayout,
		"/ppt/slideMasters/slideMaster1.xml": contentTypePPTXSlideMaster,
		"/ppt/charts/chart1.xml":             contentTypeChart,
		"/ppt/media/image1.png":              "image/png",
		"/ppt/media/bad.png":                 "image/png",
		"/ppt/media/media1.mp4":              "video/mp4",
		"/ppt/media/audio1.m4a":              "audio/x-m4a",
		"/xl/drawings/drawing1.xml":          contentTypeDrawing,
		"/xl/charts/chart1.xml":              contentTypeChart,
		"/xl/media/image1.png":               "image/png",
		"/xl/media/media1.mp4":               "video/mp4",
		"/xl/media/audio1.m4a":               "audio/x-m4a",
	}, map[string]string{
		"/ppt/slides/slide1.xml": `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree>
    <p:pic><p:blipFill><a:blip r:embed="rIdMissingImage"/></p:blipFill></p:pic>
    <p:pic><p:blipFill><a:blip r:embed="rIdWrongImageType"/></p:blipFill></p:pic>
    <p:pic><p:blipFill><a:blip r:embed="rIdImageWrongContent"/></p:blipFill></p:pic>
    <p:pic><p:blipFill><a:blip r:embed="rIdBadImagePayload"/></p:blipFill></p:pic>
    <p:pic><p:nvPicPr><p:nvPr>
      <a:videoFile r:link="rIdWrongVideoType"/>
      <a:audioFile r:link="rIdAudioWrongContent"/>
      <p:extLst><p:ext uri="{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}">
        <p14:media r:embed="rIdExternalMedia"/>
        <p14:media/>
      </p:ext></p:extLst>
    </p:nvPr></p:nvPicPr></p:pic>
  </p:spTree></p:cSld>
</p:sld>`,
		"/ppt/slideLayouts/slideLayout1.xml": `<p:sldLayout xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree><p:pic><p:blipFill><a:blip r:embed="rIdLayoutMissing"/></p:blipFill></p:pic></p:spTree></p:cSld>
</p:sldLayout>`,
		"/ppt/slideMasters/slideMaster1.xml": `<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree><p:pic><p:blipFill><a:blip r:embed="rIdMasterWrongContent"/></p:blipFill></p:pic></p:spTree></p:cSld>
  <p:clrMap/>
  <p:txStyles/>
</p:sldMaster>`,
		"/ppt/charts/chart1.xml":    `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
		"/xl/drawings/drawing1.xml": drawingXML,
		"/xl/charts/chart1.xml":     `<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></c:chartSpace>`,
	})
	session.rawParts = map[string][]byte{
		"/ppt/media/bad.png": []byte("not really a png"),
	}
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/slides/slide1.xml": {
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdWrongImageType", Type: xlsxns.RelChart, Target: "../charts/chart1.xml"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdImageWrongContent", Type: relTypeImage, Target: "../media/media1.mp4"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdBadImagePayload", Type: relTypeImage, Target: "../media/bad.png"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdWrongVideoType", Type: relTypeImage, Target: "../media/image1.png"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdAudioWrongContent", Type: relTypeAudio, Target: "../media/image1.png"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdExternalMedia", Type: relTypeMedia, Target: "http://example.com/media.mp4", TargetMode: "External"},
		},
		"/ppt/slideMasters/slideMaster1.xml": {
			{SourceURI: "/ppt/slideMasters/slideMaster1.xml", ID: "rIdMasterWrongContent", Type: relTypeImage, Target: "../media/media1.mp4"},
		},
		"/xl/drawings/drawing1.xml": {
			{SourceURI: "/xl/drawings/drawing1.xml", ID: "rIdWrongXlsxImageType", Type: xlsxns.RelChart, Target: "../charts/chart1.xml"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdMissingImage")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdWrongImageType")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdImageWrongContent")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdLayoutMissing")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdMasterWrongContent")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdWrongXlsxImageType")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_PAYLOAD", "rIdBadImagePayload")
	assertDiagnosticMessageContains(t, diags, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", "rIdWrongVideoType")
	assertDiagnosticMessageContains(t, diags, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", "rIdAudioWrongContent")
	assertDiagnosticMessageContains(t, diags, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", "rIdExternalMedia")
	assertDiagnosticMessageContains(t, diags, "PPTX_MEDIA_RELATIONSHIP_REFERENCE", "missing required r:embed")
}

func TestRepairInvariantsAllowDrawingMediaRelationshipReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/slides/slide1.xml":    contentTypePPTXSlide,
		"/ppt/media/image1.png":     "image/png",
		"/ppt/media/vector.svg":     "image/svg+xml",
		"/ppt/media/media1.mp4":     "video/mp4",
		"/xl/drawings/drawing1.xml": contentTypeDrawing,
		"/xl/media/image1.png":      "image/png",
	}, map[string]string{
		"/ppt/slides/slide1.xml": `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld><p:spTree><p:pic><p:nvPicPr><p:nvPr>
    <a:videoFile r:link="rIdExternalVideo"/>
    <p:extLst><p:ext uri="{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}"><p14:media r:embed="rIdMedia"/></p:ext></p:extLst>
  </p:nvPr></p:nvPicPr><p:blipFill><a:blip r:embed="rIdPoster"/><a:blip r:embed="rIdSvg"/></p:blipFill></p:pic></p:spTree></p:cSld>
</p:sld>`,
		"/xl/drawings/drawing1.xml": `<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:pic><xdr:blipFill><a:blip r:embed="rIdImage"/></xdr:blipFill></xdr:pic><xdr:clientData/></xdr:twoCellAnchor>
</xdr:wsDr>`,
	})
	session.rawParts = map[string][]byte{
		"/ppt/media/image1.png": minimalPNGBytes(),
		"/xl/media/image1.png":  minimalPNGBytes(),
	}
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/slides/slide1.xml": {
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdPoster", Type: relTypeImage, Target: "../media/image1.png"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdSvg", Type: relTypeImage, Target: "../media/vector.svg"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdExternalVideo", Type: relTypeVideo, Target: "http://example.com/video.mp4", TargetMode: "External"},
			{SourceURI: "/ppt/slides/slide1.xml", ID: "rIdMedia", Type: relTypeMedia, Target: "../media/media1.mp4"},
		},
		"/xl/drawings/drawing1.xml": {
			{SourceURI: "/xl/drawings/drawing1.xml", ID: "rIdImage", Type: relTypeImage, Target: "../media/image1.png"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE")
	assertNoDiagnosticCode(t, diags, "OOXML_IMAGE_PAYLOAD")
	assertNoDiagnosticCode(t, diags, "PPTX_MEDIA_RELATIONSHIP_REFERENCE")
}

func TestRepairInvariantsCatchDOCXImagePayloadProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/word/document.xml":         docxns.ContentTypeDocument,
		"/word/header1.xml":          docxns.ContentTypeHeader,
		"/word/media/good.png":       "image/png",
		"/word/media/bad.png":        "image/png",
		"/word/media/header-bad.png": "image/png",
		"/word/media/media1.mp4":     "video/mp4",
	}, map[string]string{
		"/word/document.xml": `<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:r><w:drawing><a:blip r:embed="rIdGood"/><a:blip r:embed="rIdBad"/><a:blip r:embed="rIdWrongContent"/></w:drawing></w:r></w:p></w:body>
</w:document>`,
		"/word/header1.xml": `<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:p><w:r><w:drawing><a:blip r:embed="rIdHeaderBad"/></w:drawing></w:r></w:p>
</w:hdr>`,
	})
	session.rawParts = map[string][]byte{
		"/word/media/good.png":       minimalPNGBytes(),
		"/word/media/bad.png":        []byte("not really a png"),
		"/word/media/header-bad.png": []byte("not really a png either"),
	}
	session.relationships = map[string][]opc.RelationshipInfo{
		"/word/document.xml": {
			{SourceURI: "/word/document.xml", ID: "rIdGood", Type: docxns.RelImage, Target: "media/good.png"},
			{SourceURI: "/word/document.xml", ID: "rIdBad", Type: docxns.RelImage, Target: "media/bad.png"},
			{SourceURI: "/word/document.xml", ID: "rIdWrongContent", Type: docxns.RelImage, Target: "media/media1.mp4"},
		},
		"/word/header1.xml": {
			{SourceURI: "/word/header1.xml", ID: "rIdHeaderBad", Type: docxns.RelImage, Target: "media/header-bad.png"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE", "rIdWrongContent")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_PAYLOAD", "rIdBad")
	assertDiagnosticMessageContains(t, diags, "OOXML_IMAGE_PAYLOAD", "rIdHeaderBad")
}

func TestRepairInvariantsAllowDOCXImagePayloadReferences(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/word/document.xml":     docxns.ContentTypeDocument,
		"/word/media/good.png":   "image/png",
		"/word/media/vector.svg": "image/svg+xml",
	}, map[string]string{
		"/word/document.xml": `<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body><w:p><w:r><w:drawing><a:blip r:embed="rIdGood"/><a:blip r:embed="rIdSvg"/></w:drawing></w:r></w:p></w:body>
</w:document>`,
	})
	session.rawParts = map[string][]byte{
		"/word/media/good.png": minimalPNGBytes(),
	}
	session.relationships = map[string][]opc.RelationshipInfo{
		"/word/document.xml": {
			{SourceURI: "/word/document.xml", ID: "rIdGood", Type: docxns.RelImage, Target: "media/good.png"},
			{SourceURI: "/word/document.xml", ID: "rIdSvg", Type: docxns.RelImage, Target: "media/vector.svg"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertNoDiagnosticCode(t, diags, "OOXML_IMAGE_RELATIONSHIP_REFERENCE")
	assertNoDiagnosticCode(t, diags, "OOXML_IMAGE_PAYLOAD")
}

func TestRepairInvariantsCatchPresentationReferenceProblems(t *testing.T) {
	session := newInvariantSession(map[string]string{
		"/ppt/presentation.xml":              contentTypePPTXPresentation,
		"/ppt/slides/slide1.xml":             contentTypePPTXSlide,
		"/ppt/slideMasters/slideMaster1.xml": contentTypePPTXSlideMaster,
	}, map[string]string{
		"/ppt/presentation.xml": `<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648"/>
    <p:sldMasterId id="2147483649" r:id="rIdMissingMaster"/>
    <p:sldMasterId id="2147483650" r:id="rIdWrongMaster"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
    <p:sldId id="256"/>
    <p:sldId id="257" r:id="rIdMissingSlide"/>
    <p:sldId id="258" r:id="rIdWrongSlide"/>
    <p:sldId id="259" r:id="rIdExternalSlide"/>
  </p:sldIdLst>
</p:presentation>`,
		"/ppt/slides/slide1.xml":             `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld/></p:sld>`,
		"/ppt/slideMasters/slideMaster1.xml": `<p:sldMaster xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>`,
	})
	session.relationships = map[string][]opc.RelationshipInfo{
		"/ppt/presentation.xml": {
			{SourceURI: "/ppt/presentation.xml", ID: "rIdWrongMaster", Type: relTypePPTXSlide, Target: "slides/slide1.xml"},
			{SourceURI: "/ppt/presentation.xml", ID: "rIdWrongSlide", Type: relTypePPTXSlideMaster, Target: "slideMasters/slideMaster1.xml"},
			{SourceURI: "/ppt/presentation.xml", ID: "rIdExternalSlide", Type: relTypePPTXSlide, Target: "http://example.com/slide.xml", TargetMode: "External"},
		},
	}

	diags, err := CheckRepairInvariants(session)
	if err != nil {
		t.Fatalf("CheckRepairInvariants returned error: %v", err)
	}
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "sldMasterId")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "rIdMissingMaster")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "rIdWrongMaster")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "sldId")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "rIdMissingSlide")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "rIdWrongSlide")
	assertDiagnosticMessageContains(t, diags, "PPTX_PRESENTATION_REFERENCE", "external target")
}

func TestCheckPackageOnKnownGoodFixtures(t *testing.T) {
	fixtures := []string{
		filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx"),
		filepath.Join("..", "..", "testdata", "xlsx", "chart-workbook", "workbook.xlsx"),
		filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx"),
		filepath.Join("..", "..", "testdata", "pptx", "chart-simple", "presentation.pptx"),
	}
	for _, fixture := range fixtures {
		report, err := CheckPackage(fixture, Options{})
		if err != nil {
			t.Fatalf("CheckPackage(%s) returned error: %v", fixture, err)
		}
		if report.Status != "passed" {
			data, _ := json.MarshalIndent(report, "", "  ")
			t.Fatalf("CheckPackage(%s) status = %s, want passed\n%s", fixture, report.Status, data)
		}
	}
}

func TestCheckPackageOfficeCheckPassedEvidence(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	checker := &fakeOfficeChecker{
		result: &officecheck.Result{
			Status:             "passed",
			Checked:            true,
			Engine:             "soffice",
			Method:             "libreoffice-headless-convert",
			ConversionFormat:   "csv",
			OutputBytes:        12,
			OfficeOpenVerified: true,
		},
	}
	report, err := CheckPackage(fixture, Options{RunOfficeCheck: true, OfficeChecker: checker, OfficeCheckOutDir: "evidence-dir"})
	if err != nil {
		t.Fatalf("CheckPackage returned error: %v", err)
	}
	if checker.calls != 1 || checker.path != fixture || checker.opts.OutDir != "evidence-dir" || checker.opts.Family != "xlsx" {
		t.Fatalf("office checker call mismatch: calls=%d path=%q opts=%+v", checker.calls, checker.path, checker.opts)
	}
	if report.Status != "passed" {
		data, _ := json.MarshalIndent(report, "", "  ")
		t.Fatalf("report status = %s, want passed\n%s", report.Status, data)
	}
	check := requireReportCheck(t, report, "office-open")
	if check.Status != "passed" || check.OfficeCheck == nil || !check.OfficeCheck.OfficeOpenVerified {
		t.Fatalf("unexpected office-open check: %+v", check)
	}
}

func TestCheckPackageOfficeCheckMissingEngineIsSkipped(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	checker := &fakeOfficeChecker{
		result: &officecheck.Result{Status: "skipped", ErrorCode: "missing_engine", Error: "required Office-compatible tool not available: soffice"},
		err:    &officecheck.MissingDependencyError{Tool: "soffice"},
	}
	report, err := CheckPackage(fixture, Options{RunOfficeCheck: true, OfficeChecker: checker})
	if err != nil {
		t.Fatalf("CheckPackage returned error: %v", err)
	}
	if report.Status != "passed" || report.Summary.Skipped != 1 {
		data, _ := json.MarshalIndent(report, "", "  ")
		t.Fatalf("missing engine should skip without failing report\n%s", data)
	}
	check := requireReportCheck(t, report, "office-open")
	if check.Status != "skipped" || check.OfficeCheck == nil || check.OfficeCheck.ErrorCode != "missing_engine" {
		t.Fatalf("unexpected skipped office-open check: %+v", check)
	}
	requireReportDiagnosticCode(t, report, "OOXML_OFFICE_CHECK_SKIPPED")
}

func TestCheckPackageOfficeCheckFailureFailsReport(t *testing.T) {
	fixture := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	checker := &fakeOfficeChecker{
		result: &officecheck.Result{Status: "failed", Checked: true, Engine: "soffice", ErrorCode: "engine_failed", Error: "soffice failed"},
		err:    errors.New("soffice failed"),
	}
	report, err := CheckPackage(fixture, Options{RunOfficeCheck: true, OfficeChecker: checker})
	if err != nil {
		t.Fatalf("CheckPackage returned error: %v", err)
	}
	if report.Status != "failed" {
		data, _ := json.MarshalIndent(report, "", "  ")
		t.Fatalf("report status = %s, want failed\n%s", report.Status, data)
	}
	check := requireReportCheck(t, report, "office-open")
	if check.Status != "failed" || check.OfficeCheck == nil || check.OfficeCheck.ErrorCode != "engine_failed" {
		t.Fatalf("unexpected failed office-open check: %+v", check)
	}
	requireReportDiagnosticCode(t, report, "OOXML_OFFICE_CHECK_FAILED")
}

func TestConformanceCommittedPPTXXLSXFixtureManifest(t *testing.T) {
	root := filepath.Join("..", "..", "testdata")
	expectedFailures := map[string][]string{
		"pptx/animations-synthetic/presentation.pptx": {
			"PPTX_ANIMATION_TARGET_REFERENCE",
		},
		"pptx/animations-stale-media/presentation.pptx": {
			"REL_DANGLING_TARGET",
			"OOXML_RELATIONSHIP_TARGET_MISSING",
		},
		"pptx/corrupted-dangling-layout/presentation.pptx": {
			"REL_DANGLING_TARGET",
			"OOXML_RELATIONSHIP_TARGET_MISSING",
		},
		"pptx/corrupted-missing-media/presentation.pptx": {
			"REL_DANGLING_TARGET",
			"OOXML_RELATIONSHIP_TARGET_MISSING",
		},
		"xlsx/corrupted-missing-worksheet/workbook.xlsx": {
			"REL_DANGLING_TARGET",
			"OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING",
			"OOXML_RELATIONSHIP_TARGET_MISSING",
		},
	}
	visitedExpectedFailures := make(map[string]bool, len(expectedFailures))

	err := filepath.WalkDir(root, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			return nil
		}
		ext := strings.ToLower(filepath.Ext(path))
		if ext != ".pptx" && ext != ".xlsx" {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		rel = filepath.ToSlash(rel)
		report, err := CheckPackage(path, Options{})
		if err != nil {
			t.Fatalf("CheckPackage(%s) returned error: %v", path, err)
		}
		if wantCodes, ok := expectedFailures[rel]; ok {
			visitedExpectedFailures[rel] = true
			if report.Status != "failed" {
				data, _ := json.MarshalIndent(report, "", "  ")
				t.Fatalf("CheckPackage(%s) status = %s, want failed expected-failure\n%s", rel, report.Status, data)
			}
			gotCodes := reportDiagnosticCodeSet(report)
			for _, code := range wantCodes {
				if !gotCodes[code] {
					data, _ := json.MarshalIndent(report, "", "  ")
					t.Fatalf("CheckPackage(%s) missing expected diagnostic %s\n%s", rel, code, data)
				}
			}
			return nil
		}
		if report.Status != "passed" {
			data, _ := json.MarshalIndent(report, "", "  ")
			t.Fatalf("CheckPackage(%s) status = %s, want passed\n%s", rel, report.Status, data)
		}
		for _, check := range report.Checks {
			for _, d := range check.Diagnostics {
				if strings.HasPrefix(d.Code, "OOXML_CONTENT_TYPES_") {
					t.Fatalf("unexpected content-types invariant diagnostic for %s: %s %s", rel, d.Code, d.Message)
				}
			}
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walk fixtures: %v", err)
	}
	for rel := range expectedFailures {
		if !visitedExpectedFailures[rel] {
			t.Fatalf("expected-failure fixture %s was not visited", rel)
		}
	}
}

func TestConformanceGoldenSummary(t *testing.T) {
	fixtures := []struct {
		name string
		path string
	}{
		{name: "xlsx-minimal-workbook", path: filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")},
		{name: "xlsx-chart-workbook", path: filepath.Join("..", "..", "testdata", "xlsx", "chart-workbook", "workbook.xlsx")},
		{name: "xlsx-corrupted-missing-worksheet", path: filepath.Join("..", "..", "testdata", "xlsx", "corrupted-missing-worksheet", "workbook.xlsx")},
		{name: "pptx-minimal-title", path: filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")},
		{name: "pptx-chart-simple", path: filepath.Join("..", "..", "testdata", "pptx", "chart-simple", "presentation.pptx")},
		{name: "pptx-animations-stale-media", path: filepath.Join("..", "..", "testdata", "pptx", "animations-stale-media", "presentation.pptx")},
		{name: "pptx-corrupted-dangling-layout", path: filepath.Join("..", "..", "testdata", "pptx", "corrupted-dangling-layout", "presentation.pptx")},
		{name: "pptx-corrupted-missing-media", path: filepath.Join("..", "..", "testdata", "pptx", "corrupted-missing-media", "presentation.pptx")},
	}
	var summaries []goldenSummary
	for _, fixture := range fixtures {
		report, err := CheckPackage(fixture.path, Options{})
		if err != nil {
			t.Fatalf("CheckPackage(%s) returned error: %v", fixture.path, err)
		}
		summaries = append(summaries, summarizeGoldenReport(fixture.name, report))
	}
	assertGoldenJSON(t, "repair-conformance-summary.json", summaries)
}

func TestConformanceCoverageReportGolden(t *testing.T) {
	report := RepairCoverageReport()
	if report.SchemaVersion != CoverageSchemaVersion {
		t.Fatalf("schemaVersion = %q, want %q", report.SchemaVersion, CoverageSchemaVersion)
	}
	if report.Scope != "pptx-xlsx-office-repair-plus-docx-targeted-invariants" || report.Status != "active" {
		t.Fatalf("unexpected scope/status: %s/%s", report.Scope, report.Status)
	}
	assertCoverageStage(t, report, "office-open")
	assertCoverageClass(t, report, "content-types", "OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING")
	assertCoverageClass(t, report, "relationships", "OOXML_RELATIONSHIP_TARGET_MISSING")
	assertCoverageClass(t, report, "part-roots", "PPTX_SLIDE_MASTER_ROOT")
	assertCoverageClass(t, report, "reference-lists", "PPTX_SLIDE_MASTER_LAYOUT_REFERENCE")
	assertCoverageClass(t, report, "schema-order", "PPTX_SLIDE_MASTER_CHILD_ORDER")
	assertCoverageClass(t, report, "pptx-animations", "PPTX_ANIMATION_TARGET_REFERENCE")
	assertCoverageClass(t, report, "drawing-media-references", "OOXML_IMAGE_RELATIONSHIP_REFERENCE")
	assertCoverageClass(t, report, "drawing-media-references", "OOXML_IMAGE_PAYLOAD")
	assertCoverageClass(t, report, "drawing-media-references", "PPTX_MEDIA_RELATIONSHIP_REFERENCE")
	assertCoverageClass(t, report, "chart-external-data", "OOXML_CHART_EXTERNAL_DATA_REFERENCE")
	assertCoverageClass(t, report, "chart-external-data", "OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN")
	assertCoverageClass(t, report, "chart-axis-references", "OOXML_CHART_AXIS_REFERENCE")
	assertCoverageClass(t, report, "chart-series-caches", "OOXML_CHART_SERIES_CACHE")
	assertCoverageClass(t, report, "xlsx-counts-styles", "XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE")
	assertCoverageClass(t, report, "xlsx-calc-chain", "XLSX_CALC_CHAIN_REFERENCE")
	assertCoverageClass(t, report, "xlsx-defined-names", "XLSX_DEFINED_NAME_REFERENCE")
	assertCoverageClass(t, report, "xlsx-tables", "XLSX_TABLE_DEFINITION")
	assertCoverageClass(t, report, "xlsx-pivots", "XLSX_PIVOT_TABLE_DEFINITION")
	assertCoverageClass(t, report, "xlsx-pivots", "XLSX_PIVOT_CACHE_RECORDS_REFERENCE")
	assertCoverageClass(t, report, "schema-order", "XLSX_TABLE_CHILD_ORDER")
	assertCoverageClass(t, report, "schema-order", "XLSX_PIVOT_TABLE_CHILD_ORDER")
	assertCoverageClass(t, report, "real-microsoft-office", "")
	assertConformanceCoverageListsAllDirectInvariantDiagnostics(t, report)
	assertGoldenJSON(t, "repair-conformance-coverage.json", report)
}

func TestConformanceOfficeOpenGoldenSummary(t *testing.T) {
	xlsxFixture := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	pptxFixture := filepath.Join("..", "..", "testdata", "pptx", "minimal-title", "presentation.pptx")
	scenarios := []struct {
		name    string
		path    string
		checker *fakeOfficeChecker
	}{
		{
			name: "xlsx-office-open-passed",
			path: xlsxFixture,
			checker: &fakeOfficeChecker{result: &officecheck.Result{
				Status:             "passed",
				Checked:            true,
				Engine:             "soffice",
				Method:             "libreoffice-headless-convert",
				ConversionFormat:   "csv",
				OutputBytes:        12,
				OfficeOpenVerified: true,
			}},
		},
		{
			name: "xlsx-office-open-missing-engine",
			path: xlsxFixture,
			checker: &fakeOfficeChecker{
				result: &officecheck.Result{Status: "skipped", ErrorCode: "missing_engine", Error: "required Office-compatible tool not available: soffice"},
				err:    &officecheck.MissingDependencyError{Tool: "soffice"},
			},
		},
		{
			name: "pptx-office-open-failed",
			path: pptxFixture,
			checker: &fakeOfficeChecker{
				result: &officecheck.Result{Status: "failed", Checked: true, Engine: "soffice", ErrorCode: "engine_failed", Error: "soffice failed"},
				err:    errors.New("soffice failed"),
			},
		},
	}
	var summaries []officeOpenGoldenSummary
	for _, scenario := range scenarios {
		report, err := CheckPackage(scenario.path, Options{RunOfficeCheck: true, OfficeChecker: scenario.checker})
		if err != nil {
			t.Fatalf("CheckPackage(%s) returned error: %v", scenario.name, err)
		}
		summaries = append(summaries, summarizeOfficeOpenGoldenReport(scenario.name, report))
	}
	assertGoldenJSON(t, "repair-conformance-office-open-summary.json", summaries)
}

func TestConformanceCoverageEvidenceReferencesExist(t *testing.T) {
	repoRoot := filepath.Join("..", "..")
	report := RepairCoverageReport()
	conformanceTests := mustReadText(t, "conformance_test.go")
	conformanceGo := mustReadText(t, "conformance.go")
	invariantsGo := mustReadText(t, "invariants.go")
	cliTests := mustReadTrackedGlobText(t, repoRoot, "internal/cli/*_test.go")
	validateGo := mustReadText(t, filepath.Join(repoRoot, "pkg", "validate", "validate.go"))

	for _, evidence := range coverageEvidenceStrings(report) {
		assertCoverageEvidenceReference(t, repoRoot, evidence, coverageEvidenceSources{
			conformanceTests: conformanceTests,
			conformanceGo:    conformanceGo,
			invariantsGo:     invariantsGo,
			cliTests:         cliTests,
			validateGo:       validateGo,
		})
	}
}

func assertCoverageStage(t *testing.T, report CoverageReport, name string) {
	t.Helper()
	for _, stage := range report.HarnessStages {
		if stage.Name == name {
			return
		}
	}
	t.Fatalf("missing coverage stage %q in %+v", name, report.HarnessStages)
}

func assertCoverageClass(t *testing.T, report CoverageReport, id, diagnosticCode string) {
	t.Helper()
	for _, class := range report.RepairClasses {
		if class.ID != id {
			continue
		}
		if diagnosticCode == "" {
			return
		}
		for _, code := range class.DiagnosticCodes {
			if code == diagnosticCode {
				return
			}
		}
		t.Fatalf("coverage class %q missing diagnostic code %q in %+v", id, diagnosticCode, class.DiagnosticCodes)
	}
	t.Fatalf("missing coverage class %q in %+v", id, report.RepairClasses)
}

type coverageEvidenceSources struct {
	conformanceTests string
	conformanceGo    string
	invariantsGo     string
	cliTests         string
	validateGo       string
}

func coverageEvidenceStrings(report CoverageReport) []string {
	var out []string
	for _, stage := range report.HarnessStages {
		out = append(out, stage.Evidence...)
	}
	for _, class := range report.RepairClasses {
		out = append(out, class.Evidence...)
	}
	for _, fixtureSet := range report.FixtureSets {
		out = append(out, fixtureSet.Evidence...)
	}
	return out
}

func assertConformanceCoverageListsAllDirectInvariantDiagnostics(t *testing.T, report CoverageReport) {
	t.Helper()
	covered := coverageDiagnosticCodeSet(report)
	emitted := directInvariantDiagnosticCodes(t)
	var missing []string
	for code := range emitted {
		if !covered[code] {
			missing = append(missing, code)
		}
	}
	sort.Strings(missing)
	if len(missing) > 0 {
		t.Fatalf("RepairCoverageReport is missing emitted invariant diagnostic codes: %s", strings.Join(missing, ", "))
	}
}

func coverageDiagnosticCodeSet(report CoverageReport) map[string]bool {
	out := make(map[string]bool)
	for _, class := range report.RepairClasses {
		for _, code := range class.DiagnosticCodes {
			out[code] = true
		}
	}
	return out
}

func directInvariantDiagnosticCodes(t *testing.T) map[string]bool {
	t.Helper()
	source := mustReadText(t, "invariants.go")
	re := regexp.MustCompile(`diag\.Errorf\("([A-Z0-9_]+)"`)
	out := make(map[string]bool)
	for _, match := range re.FindAllStringSubmatch(source, -1) {
		out[match[1]] = true
	}
	return out
}

func assertCoverageEvidenceReference(t *testing.T, repoRoot, evidence string, sources coverageEvidenceSources) {
	t.Helper()
	switch {
	case strings.HasPrefix(evidence, "pkg/conformance.Test"):
		assertTestReference(t, sources.conformanceTests, strings.TrimPrefix(evidence, "pkg/conformance."))
	case strings.HasPrefix(evidence, "internal/cli.Test"):
		assertTestReference(t, sources.cliTests, strings.TrimPrefix(evidence, "internal/cli."))
	case strings.HasPrefix(evidence, "Test"):
		assertTestReference(t, sources.conformanceTests, evidence)
	case evidence == "pkg/conformance.CheckPackage":
		assertContains(t, sources.conformanceGo, "func CheckPackage(", evidence)
	case evidence == "pkg/conformance.CheckRepairInvariants":
		assertContains(t, sources.invariantsGo, "func CheckRepairInvariants(", evidence)
	case evidence == "pkg/validate.ValidatePackage":
		assertContains(t, sources.validateGo, "func ValidatePackage(", evidence)
	case strings.HasPrefix(evidence, "pkg/"):
		assertPathOrGlobExists(t, repoRoot, evidence)
	case strings.HasPrefix(evidence, "testdata/"):
		assertPathOrGlobExists(t, repoRoot, evidence)
	default:
		assertPathOrGlobExists(t, repoRoot, evidence)
	}
}

func assertTestReference(t *testing.T, source, evidence string) {
	t.Helper()
	if strings.HasSuffix(evidence, "*") {
		prefix := strings.TrimSuffix(evidence, "*")
		if strings.Contains(source, "func "+prefix) {
			return
		}
		t.Fatalf("coverage evidence prefix %q did not match any test function", evidence)
	}
	assertContains(t, source, "func "+evidence+"(", evidence)
}

func assertContains(t *testing.T, source, needle, evidence string) {
	t.Helper()
	if !strings.Contains(source, needle) {
		t.Fatalf("coverage evidence %q is stale; missing %q", evidence, needle)
	}
}

func assertPathOrGlobExists(t *testing.T, repoRoot, evidence string) {
	t.Helper()
	path := filepath.Join(repoRoot, evidence)
	if strings.ContainsAny(evidence, "*?[") {
		matches, err := filepath.Glob(path)
		if err != nil {
			t.Fatalf("coverage evidence %q has invalid glob: %v", evidence, err)
		}
		if len(matches) == 0 {
			t.Fatalf("coverage evidence glob %q matched no paths", evidence)
		}
		return
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("coverage evidence path %q is stale: %v", evidence, err)
	}
}

func mustReadText(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return string(data)
}

func mustReadTrackedGlobText(t *testing.T, repoRoot, pattern string) string {
	t.Helper()
	cmd := exec.Command("git", "-C", repoRoot, "ls-files", "--", pattern)
	out, err := cmd.Output()
	if err != nil {
		t.Fatalf("git ls-files %s: %v", pattern, err)
	}
	var paths []string
	for _, path := range strings.Split(strings.TrimSpace(string(out)), "\n") {
		path = strings.TrimSpace(path)
		if path != "" {
			paths = append(paths, path)
		}
	}
	if len(paths) == 0 {
		t.Fatalf("git ls-files %s matched no files", pattern)
	}
	var b strings.Builder
	for _, path := range paths {
		b.WriteString(mustReadText(t, filepath.Join(repoRoot, filepath.FromSlash(path))))
		b.WriteByte('\n')
	}
	return b.String()
}

type goldenSummary struct {
	Fixture string             `json:"fixture"`
	Family  string             `json:"family"`
	Status  string             `json:"status"`
	Checks  []goldenCheckEntry `json:"checks"`
}

type goldenCheckEntry struct {
	Name        string `json:"name"`
	Status      string `json:"status"`
	Diagnostics int    `json:"diagnostics"`
}

type officeOpenGoldenSummary struct {
	Scenario string                  `json:"scenario"`
	Family   string                  `json:"family"`
	Status   string                  `json:"status"`
	Checks   []officeOpenGoldenCheck `json:"checks"`
}

type officeOpenGoldenCheck struct {
	Name               string `json:"name"`
	Status             string `json:"status"`
	Diagnostics        int    `json:"diagnostics"`
	OfficeCheckStatus  string `json:"officeCheckStatus,omitempty"`
	OfficeCheckCode    string `json:"officeCheckCode,omitempty"`
	OfficeOpenVerified bool   `json:"officeOpenVerified,omitempty"`
}

func summarizeGoldenReport(name string, report *Report) goldenSummary {
	out := goldenSummary{
		Fixture: name,
		Family:  report.Family,
		Status:  report.Status,
	}
	for _, check := range report.Checks {
		out.Checks = append(out.Checks, goldenCheckEntry{
			Name:        check.Name,
			Status:      check.Status,
			Diagnostics: len(check.Diagnostics),
		})
	}
	return out
}

func summarizeOfficeOpenGoldenReport(name string, report *Report) officeOpenGoldenSummary {
	out := officeOpenGoldenSummary{
		Scenario: name,
		Family:   report.Family,
		Status:   report.Status,
	}
	for _, check := range report.Checks {
		entry := officeOpenGoldenCheck{
			Name:        check.Name,
			Status:      check.Status,
			Diagnostics: len(check.Diagnostics),
		}
		if check.OfficeCheck != nil {
			entry.OfficeCheckStatus = check.OfficeCheck.Status
			entry.OfficeCheckCode = check.OfficeCheck.ErrorCode
			entry.OfficeOpenVerified = check.OfficeCheck.OfficeOpenVerified
		}
		out.Checks = append(out.Checks, entry)
	}
	return out
}

func reportDiagnosticCodeSet(report *Report) map[string]bool {
	out := make(map[string]bool)
	if report == nil {
		return out
	}
	for _, check := range report.Checks {
		for _, d := range check.Diagnostics {
			out[d.Code] = true
		}
	}
	return out
}

func assertGoldenJSON(t *testing.T, name string, value any) {
	t.Helper()
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		t.Fatalf("marshal golden: %v", err)
	}
	data = append(data, '\n')
	path := filepath.Join("..", "..", "testdata", "golden", name)
	if os.Getenv("UPDATE_GOLDENS") == "1" {
		if err := os.WriteFile(path, data, 0o644); err != nil {
			t.Fatalf("update golden %s: %v", path, err)
		}
		return
	}
	expected, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden %s: %v", path, err)
	}
	if !bytes.Equal(normalizeGoldenLineEndings(expected), normalizeGoldenLineEndings(data)) {
		t.Fatalf("golden mismatch for %s\nexpected:\n%s\nactual:\n%s", path, expected, data)
	}
}

func normalizeGoldenLineEndings(data []byte) []byte {
	return bytes.ReplaceAll(data, []byte("\r\n"), []byte("\n"))
}

func assertDiagnosticCode(t *testing.T, diags []result.Diagnostic, code string) {
	t.Helper()
	for _, d := range diags {
		if d.Code == code {
			return
		}
	}
	var codes []string
	for _, d := range diags {
		codes = append(codes, d.Code)
	}
	t.Fatalf("missing diagnostic code %s; got %s", code, strings.Join(codes, ", "))
}

func assertDiagnosticMessageContains(t *testing.T, diags []result.Diagnostic, code string, fragment string) {
	t.Helper()
	for _, d := range diags {
		if d.Code == code && strings.Contains(d.Message, fragment) {
			return
		}
	}
	t.Fatalf("missing diagnostic %s containing %q; got %+v", code, fragment, diags)
}

func assertNoDiagnosticCode(t *testing.T, diags []result.Diagnostic, code string) {
	t.Helper()
	for _, d := range diags {
		if d.Code == code {
			t.Fatalf("unexpected diagnostic code %s: %+v", code, d)
		}
	}
}

func requireReportCheck(t *testing.T, report *Report, name string) CheckResult {
	t.Helper()
	if report == nil {
		t.Fatal("nil report")
	}
	for _, check := range report.Checks {
		if check.Name == name {
			return check
		}
	}
	t.Fatalf("missing report check %q in %+v", name, report.Checks)
	return CheckResult{}
}

func requireReportDiagnosticCode(t *testing.T, report *Report, code string) {
	t.Helper()
	if report == nil {
		t.Fatal("nil report")
	}
	for _, check := range report.Checks {
		for _, d := range check.Diagnostics {
			if d.Code == code {
				return
			}
		}
	}
	t.Fatalf("missing report diagnostic code %q in %+v", code, report.Checks)
}

type invariantSession struct {
	contentTypes  map[string]string
	xmlParts      map[string]string
	rawParts      map[string][]byte
	zipMetas      map[string]*opc.ZipEntryMeta
	relationships map[string][]opc.RelationshipInfo
	rawReadErrors map[string]error
}

func newInvariantSession(contentTypes, xmlParts map[string]string) *invariantSession {
	return &invariantSession{contentTypes: contentTypes, xmlParts: xmlParts}
}

func (s *invariantSession) ListParts() []opc.PartInfo {
	out := make([]opc.PartInfo, 0, len(s.contentTypes))
	for uri, ct := range s.contentTypes {
		size := len(s.xmlParts[uri])
		if raw, ok := s.rawParts[uri]; ok {
			size = len(raw)
		}
		out = append(out, opc.PartInfo{URI: uri, ContentType: ct, IsXML: s.xmlParts[uri] != "", SizeBytes: int64(size)})
	}
	return out
}

func (s *invariantSession) ListRelationships(sourceURI string) []opc.RelationshipInfo {
	return s.relationships[opc.NormalizeURI(sourceURI)]
}
func (s *invariantSession) ReadRawPart(uri string) ([]byte, error) {
	uri = opc.NormalizeURI(uri)
	if err := s.rawReadErrors[uri]; err != nil {
		return nil, err
	}
	if raw, ok := s.rawParts[uri]; ok {
		return append([]byte(nil), raw...), nil
	}
	return []byte(s.xmlParts[uri]), nil
}

func (s *invariantSession) ReadXMLPart(uri string) (*etree.Document, error) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(s.xmlParts[opc.NormalizeURI(uri)]); err != nil {
		return nil, err
	}
	return doc, nil
}

func (s *invariantSession) GetContentType(uri string) string                        { return s.contentTypes[uri] }
func (s *invariantSession) GetZipMeta(uri string) *opc.ZipEntryMeta                 { return s.zipMetas[uri] }
func (s *invariantSession) ReplaceRawPart(string, []byte, string) error             { return nil }
func (s *invariantSession) ReplaceXMLPart(string, *etree.Document) error            { return nil }
func (s *invariantSession) AddPart(string, []byte, string, *opc.ZipEntryMeta) error { return nil }
func (s *invariantSession) RemovePart(string) error                                 { return nil }
func (s *invariantSession) SaveAs(string) error                                     { return nil }
func (s *invariantSession) Close() error                                            { return nil }
func (s *invariantSession) IsDirty() bool                                           { return false }
func (s *invariantSession) Warnings() []string                                      { return nil }

type fakeOfficeChecker struct {
	result *officecheck.Result
	err    error
	calls  int
	path   string
	opts   officecheck.Options
}

func (c *fakeOfficeChecker) Check(filePath string, opts officecheck.Options) (*officecheck.Result, error) {
	c.calls++
	c.path = filePath
	c.opts = opts
	return c.result, c.err
}

func minimalWorkbookFixtureBytes(t *testing.T) []byte {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx"))
	if err != nil {
		t.Fatalf("read minimal workbook fixture: %v", err)
	}
	return raw
}

func minimalPNGBytes() []byte {
	var buf bytes.Buffer
	if err := png.Encode(&buf, image.NewRGBA(image.Rect(0, 0, 1, 1))); err != nil {
		panic(err)
	}
	return buf.Bytes()
}
