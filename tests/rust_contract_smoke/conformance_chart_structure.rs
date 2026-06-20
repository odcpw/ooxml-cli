use super::*;

#[test]
fn conformance_check_matches_go_for_xlsx_drawing_and_chart_structure() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-chart-structure-xlsx-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("chart structure xlsx temp dir");

    let broken = temp_dir.join("chart-structure-broken.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/chart-workbook/workbook.xlsx",
        &broken,
        |name, data| {
            let body = match name {
                "xl/drawings/drawing1.xml" => Some(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing">
  <xdr:twoCellAnchor><xdr:from/><xdr:clientData/><xdr:to/></xdr:twoCellAnchor>
  <xdr:oneCellAnchor><xdr:clientData/><xdr:from/></xdr:oneCellAnchor>
  <xdr:absoluteAnchor><xdr:clientData/><xdr:pos/></xdr:absoluteAnchor>
</xdr:wsDr>"#
                        .to_string(),
                ),
                "xl/charts/chart1.xml" => Some(broken_axis_and_series_chart_xml(true)),
                _ => None,
            };
            Some((
                name.to_string(),
                body.map(|body| body.into_bytes()).unwrap_or(data),
            ))
        },
    );
    assert_chart_repair_invariants_match(&broken);

    let clean = temp_dir.join("chart-structure-clean.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/chart-workbook/workbook.xlsx",
        &clean,
        |name, data| {
            let body = match name {
                "xl/drawings/drawing1.xml" => Some(clean_drawing_xml()),
                "xl/charts/chart1.xml" => Some(clean_bar_chart_xml()),
                _ => None,
            };
            Some((
                name.to_string(),
                body.map(|body| body.into_bytes()).unwrap_or(data),
            ))
        },
    );
    assert_chart_repair_invariants_match(&clean);
}

#[test]
fn conformance_check_matches_go_for_pptx_chart_structure() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-conformance-chart-structure-pptx-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("chart structure pptx temp dir");

    let broken = temp_dir.join("chart-structure-broken.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/chart-simple/presentation.pptx",
        &broken,
        |name, data| {
            if name == "ppt/charts/chart1.xml" {
                Some((
                    name.to_string(),
                    broken_axis_and_series_chart_xml(false).into_bytes(),
                ))
            } else {
                Some((name.to_string(), data))
            }
        },
    );
    assert_chart_repair_invariants_match(&broken);
}

fn assert_chart_repair_invariants_match(file: &Path) {
    let file = file.to_string_lossy().to_string();
    let args = ["--json", "conformance", "check", file.as_str()];
    let (_, go_stdout, go_stderr) = run_go_ooxml(&args);
    let (_, rust_stdout, rust_stderr) = run_ooxml(&args);
    assert_eq!(rust_stderr, go_stderr, "stderr for {file}");
    let rust_report = rust_stdout.expect("rust conformance stdout");
    let go_report = go_stdout.expect("go conformance stdout");
    assert_eq!(
        check_by_name(&rust_report, "repair-invariants"),
        check_by_name(&go_report, "repair-invariants"),
        "repair-invariants check for {file}"
    );
}

fn check_by_name<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing check {name}: {}", report["checks"]))
}

fn broken_axis_and_series_chart_xml(include_chart_space_order_break: bool) -> String {
    let external_data = if include_chart_space_order_break {
        "<c:externalData/>"
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  {external_data}
  <c:chart>
    <c:plotVisOnly/>
    <c:plotArea>
      <c:spPr/>
      <c:barChart>
        <c:axId/>
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
        <c:axId val="999"/>
        <c:axId val="222"/>
        <c:axId val="222"/>
      </c:barChart>
      <c:catAx><c:axId val="111"/><c:crossAx val="999"/></c:catAx>
      <c:valAx><c:axId val="222"/><c:crossAx val="222"/></c:valAx>
      <c:dateAx><c:crossAx val="111"/></c:dateAx>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#
    )
}

fn clean_drawing_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing">
  <xdr:twoCellAnchor><xdr:from/><xdr:to/><xdr:graphicFrame/><xdr:clientData/></xdr:twoCellAnchor>
  <xdr:oneCellAnchor><xdr:from/><xdr:ext/><xdr:pic/><xdr:clientData/></xdr:oneCellAnchor>
  <xdr:absoluteAnchor><xdr:pos/><xdr:ext/><xdr:pic/><xdr:clientData/></xdr:absoluteAnchor>
</xdr:wsDr>"#
        .to_string()
}

fn clean_bar_chart_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart>
    <c:plotArea>
      <c:barChart>
        <c:barDir val="col"/>
        <c:grouping val="clustered"/>
        <c:ser>
          <c:idx val="0"/>
          <c:order val="0"/>
          <c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="1"><c:v>South</c:v></c:pt></c:strCache></c:strRef></c:cat>
          <c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numCache></c:numRef></c:val>
        </c:ser>
        <c:axId val="111"/>
        <c:axId val="222"/>
      </c:barChart>
      <c:catAx><c:axId val="111"/><c:crossAx val="222"/></c:catAx>
      <c:valAx><c:axId val="222"/><c:crossAx val="111"/></c:valAx>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#
        .to_string()
}
