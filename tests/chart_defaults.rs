use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TYPES: [&str; 5] = ["bar", "line", "area", "pie", "scatter"];
const VALUES: &str = r#"[["","Revenue","Forecast"],["Enterprise North",1200,1100],["Enterprise South",1850,1700],["Public Sector West",950,1050],["Public Sector East",2200,2050],["Commercial Central",1600,1550],["Online International",2450,2300],["Partner Ecosystem",1300,1400],["Small Business Direct",800,850]]"#;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "args={args:?}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn temp_dir(label: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-chart-defaults-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create chart test directory");
    path
}

fn zip_text(path: &Path, part: &str) -> String {
    let mut archive =
        zip::ZipArchive::new(File::open(path).expect("open package")).expect("open zip package");
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap_or_else(|_| panic!("missing {part} in {}", path.display()))
        .read_to_string(&mut text)
        .expect("read XML part");
    text
}

fn assert_schema_proofs(path: &Path) {
    let path_text = path.to_str().expect("UTF-8 package path");
    let strict = run(&["validate", "--strict", path_text]);
    assert!(
        strict.status.success(),
        "strict validation rejected {}: {}",
        path.display(),
        String::from_utf8_lossy(&strict.stderr)
    );
    let conformance = run_json(&["--json", "conformance", "check", path_text, "--openxml-sdk"]);
    assert_eq!(conformance["status"], "passed", "{conformance}");
    let schema = conformance["checks"]
        .as_array()
        .expect("conformance checks")
        .iter()
        .find(|check| check["name"] == "schema")
        .unwrap_or_else(|| panic!("missing schema result: {conformance}"));
    if schema["status"] == "skipped" {
        assert_eq!(
            schema["diagnostics"][0]["code"],
            "OOXML_OPENXML_SDK_SKIPPED"
        );
        if std::env::var("OOXML_REQUIRE_OPENXML_SDK")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        {
            panic!(
                "Open XML SDK was required but skipped for {}: {schema}",
                path.display()
            );
        }
        eprintln!(
            "SKIP Open XML SDK for {}: {}",
            path.display(),
            schema["diagnostics"][0]["remediation"]
        );
    } else {
        assert_eq!(schema["status"], "passed", "{schema}");
        assert_eq!(schema["schemaCheck"]["validator"], "openxml-sdk");
    }
}

fn chart_summary(xml: &str, chart_type: &str, expected_format: &str) -> Value {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut accents = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element) | Event::Empty(element))
                if element.local_name().as_ref() == b"schemeClr" =>
            {
                if let Some(value) = element.attributes().flatten().find_map(|attribute| {
                    (attribute.key.local_name().as_ref() == b"val")
                        .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
                }) && value.starts_with("accent")
                    && !accents.contains(&value)
                {
                    accents.push(value);
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("parse chart XML: {error}"),
        }
    }
    let chart_tag = format!("<c:{chart_type}Chart");
    assert!(xml.contains(&chart_tag), "missing {chart_tag}: {xml}");
    assert!(!xml.contains("3DChart"), "chart must remain 2D: {xml}");
    assert!(
        xml.contains(expected_format),
        "chart cache/axis must carry {expected_format:?}: {xml}"
    );
    let series_count = xml.matches("<c:ser>").count();
    let expected_gridlines = usize::from(chart_type != "pie");
    assert_eq!(
        xml.matches("<c:majorGridlines").count(),
        expected_gridlines,
        "major value gridline policy for {chart_type}"
    );
    assert!(!xml.contains("<c:minorGridlines"));
    assert_eq!(
        xml.contains("<c:legendPos val=\"b\""),
        chart_type != "pie",
        "legend policy for {chart_type}"
    );
    assert!(xml.contains("<a:t>Revenue</a:t>"), "header-derived title");
    json!({
        "chartElement": format!("{chart_type}Chart"),
        "seriesCount": series_count,
        "seriesColors": accents,
        "twoDimensional": true,
        "majorValueGridlines": expected_gridlines,
        "minorGridlines": 0,
        "numberFormat": expected_format,
        "legendPosition": if chart_type == "pie" { Value::Null } else { json!("bottom") },
        "dataLabels": false,
        "title": "Revenue",
        "categoryLabelsRotated": chart_type != "pie" && xml.contains("rot=\"-2700000\"")
    })
}

fn make_xlsx_source(root: &Path) -> PathBuf {
    let values = root.join("xlsx-values.xlsx");
    let formatted = root.join("xlsx-currency.xlsx");
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A1:C9",
        "--values",
        VALUES,
        "--data-format",
        "json",
        "--out",
        values.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        values.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "B2:C9",
        "--preset",
        "currency",
        "--currency-symbol",
        "$",
        "--out",
        formatted.to_str().unwrap(),
    ]);
    formatted
}

fn pdftotext_available() -> bool {
    Command::new("pdftotext")
        .arg("-v")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn render_with_libreoffice(path: &Path, root: &Path, label: &str) -> Option<PathBuf> {
    let output_dir = root.join(format!("render-{label}"));
    let report = run_json(&[
        "--json",
        "render",
        path.to_str().unwrap(),
        "--out",
        output_dir.to_str().unwrap(),
    ]);
    if report["status"] == "skipped" {
        eprintln!(
            "SKIP LibreOffice chart render for {}: missingTools={} remediation={}",
            path.display(),
            report["missingTools"],
            report["remediation"]
        );
        return None;
    }
    assert_eq!(report["status"], "ok", "{report}");
    assert_eq!(report["engine"], "libreoffice", "{report}");
    let pdf = PathBuf::from(
        report["pdfPath"]
            .as_str()
            .unwrap_or_else(|| panic!("render report has no pdfPath: {report}")),
    );
    assert!(
        pdf.is_file(),
        "LibreOffice did not create {}",
        pdf.display()
    );
    assert!(
        fs::metadata(&pdf).expect("PDF metadata").len() > 1_000,
        "rendered PDF is unexpectedly small"
    );
    Some(pdf)
}

fn assert_golden(actual: &Value) {
    let path = Path::new("testdata/charts/house-style-summary.json");
    let bytes = serde_json::to_vec_pretty(actual).expect("serialize chart golden");
    let mut bytes_with_lf = bytes;
    bytes_with_lf.push(b'\n');
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(path.parent().unwrap()).expect("create chart golden directory");
        fs::write(path, &bytes_with_lf).expect("write chart golden");
    }
    let expected_bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "missing {}: {error}; rerun with UPDATE_GOLDENS=1",
            path.display()
        )
    });
    let expected: Value = serde_json::from_slice(&expected_bytes)
        .unwrap_or_else(|error| panic!("invalid chart golden {}: {error}", path.display()));
    assert_eq!(actual, &expected, "chart house-style golden drift");
}

#[test]
fn default_recipes_for_every_chart_type_are_schema_clean_renderable_and_golden() {
    let root = temp_dir("matrix");
    let xlsx_source = make_xlsx_source(&root);
    let mut families = Map::new();
    let mut pptx_summaries = Map::new();
    let mut xlsx_summaries = Map::new();

    for chart_type in TYPES {
        let pptx = root.join(format!("pptx-{chart_type}.pptx"));
        let pptx_report = run_json(&[
            "--json",
            "pptx",
            "charts",
            "create",
            "testdata/pptx/multi-layout/presentation.pptx",
            "--slide",
            "1",
            "--type",
            chart_type,
            "--values-json",
            VALUES,
            "--out",
            pptx.to_str().unwrap(),
        ]);
        assert_eq!(pptx_report["houseStyle"], "default");
        assert_eq!(pptx_report["numberFormat"], "#,##0");
        assert_eq!(pptx_report["dataLabels"], false);
        assert_schema_proofs(&pptx);
        pptx_summaries.insert(
            chart_type.to_string(),
            chart_summary(
                &zip_text(&pptx, "ppt/charts/chart1.xml"),
                chart_type,
                "#,##0",
            ),
        );
        render_with_libreoffice(&pptx, &root, &format!("pptx-{chart_type}"));

        let xlsx = root.join(format!("xlsx-{chart_type}.xlsx"));
        let xlsx_report = run_json(&[
            "--json",
            "xlsx",
            "charts",
            "create",
            xlsx_source.to_str().unwrap(),
            "--sheet",
            "1",
            "--range",
            "A1:C9",
            "--type",
            chart_type,
            "--out",
            xlsx.to_str().unwrap(),
        ]);
        assert_eq!(xlsx_report["houseStyle"], "default");
        assert_eq!(xlsx_report["numberFormat"], "\"$\"#,##0.00");
        assert_eq!(xlsx_report["dataLabels"], false);
        assert_schema_proofs(&xlsx);
        xlsx_summaries.insert(
            chart_type.to_string(),
            chart_summary(
                &zip_text(&xlsx, "xl/charts/chart1.xml"),
                chart_type,
                "\"$\"#,##0.00",
            ),
        );
        let rendered = render_with_libreoffice(&xlsx, &root, &format!("xlsx-{chart_type}"));
        if chart_type == "bar"
            && pdftotext_available()
            && let Some(pdf) = rendered
        {
            let text = Command::new("pdftotext")
                .args([pdf.to_str().unwrap(), "-"])
                .output()
                .expect("extract rendered chart text");
            assert!(text.status.success(), "pdftotext failed");
            let rendered_text = String::from_utf8_lossy(&text.stdout);
            assert!(
                rendered_text.contains("$3,000.00"),
                "LibreOffice-rendered value axis must retain the inferred currency format; the source data never contains 3000: {rendered_text}"
            );
        }
    }
    families.insert("pptx".to_string(), Value::Object(pptx_summaries));
    families.insert("xlsx".to_string(), Value::Object(xlsx_summaries));
    assert_golden(&Value::Object(families));

    let repeat = root.join("xlsx-bar-repeat.xlsx");
    run_json(&[
        "--json",
        "xlsx",
        "charts",
        "create",
        xlsx_source.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "A1:C9",
        "--type",
        "bar",
        "--out",
        repeat.to_str().unwrap(),
    ]);
    assert_eq!(
        fs::read(root.join("xlsx-bar.xlsx")).unwrap(),
        fs::read(&repeat).unwrap(),
        "default chart recipe must be byte deterministic"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn style_number_format_and_data_label_overrides_share_one_contract() {
    let root = temp_dir("overrides");
    let xlsx_source = make_xlsx_source(&root);
    let xlsx = root.join("minimal-percent.xlsx");
    let xlsx_report = run_json(&[
        "--json",
        "xlsx",
        "charts",
        "create",
        xlsx_source.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "A1:B9",
        "--type",
        "bar",
        "--style",
        "minimal",
        "--number-format",
        "0.0%",
        "--data-labels",
        "--out",
        xlsx.to_str().unwrap(),
    ]);
    assert_eq!(xlsx_report["houseStyle"], "minimal");
    assert_eq!(xlsx_report["numberFormat"], "0.0%");
    assert_eq!(xlsx_report["dataLabels"], true);
    let xlsx_xml = zip_text(&xlsx, "xl/charts/chart1.xml");
    assert!(!xlsx_xml.contains("<c:majorGridlines"));
    assert!(xlsx_xml.contains("<c:dLbls>"));
    assert!(!xlsx_xml.contains("<c:legend>"));
    assert!(xlsx_xml.contains("0.0%"));
    assert_schema_proofs(&xlsx);

    let pptx = root.join("dense-percent.pptx");
    let pptx_report = run_json(&[
        "--json",
        "pptx",
        "charts",
        "create",
        "testdata/pptx/multi-layout/presentation.pptx",
        "--slide",
        "1",
        "--type",
        "line",
        "--values-json",
        VALUES,
        "--style",
        "dense",
        "--number-format",
        "0.0%",
        "--data-labels",
        "--out",
        pptx.to_str().unwrap(),
    ]);
    assert_eq!(pptx_report["houseStyle"], "dense");
    assert_eq!(pptx_report["numberFormat"], "0.0%");
    assert_eq!(pptx_report["dataLabels"], true);
    let pptx_xml = zip_text(&pptx, "ppt/charts/chart1.xml");
    assert!(pptx_xml.contains("<c:dLbls>"));
    assert!(pptx_xml.contains("<c:legendPos val=\"b\""));
    assert!(pptx_xml.contains("rot=\"-2700000\""));
    assert_schema_proofs(&pptx);

    for family in ["pptx", "xlsx"] {
        let fixture = if family == "pptx" {
            "testdata/pptx/multi-layout/presentation.pptx"
        } else {
            xlsx_source.to_str().unwrap()
        };
        let mut args = vec!["--json", family, "charts", "create", fixture];
        if family == "pptx" {
            args.extend(["--slide", "1", "--values-json", VALUES]);
        } else {
            args.extend(["--sheet", "1", "--range", "A1:B9"]);
        }
        args.extend(["--type", "bar", "--style", "ornate", "--dry-run"]);
        let rejected = run(&args);
        assert!(
            !rejected.status.success(),
            "{family} must reject unknown styles"
        );
        assert!(String::from_utf8_lossy(&rejected.stderr).contains("minimal, default, or dense"));
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn create_manifests_expose_the_same_house_style_flags() {
    let capabilities = run_json(&["--json", "capabilities"]);
    for command in ["ooxml pptx charts create", "ooxml xlsx charts create"] {
        let entry = capabilities["commands"]
            .as_array()
            .expect("commands")
            .iter()
            .find(|entry| entry["path"] == command)
            .unwrap_or_else(|| panic!("missing {command}"));
        let flags = entry["localFlags"]
            .as_array()
            .expect("flags")
            .iter()
            .filter_map(|flag| flag["name"].as_str())
            .collect::<BTreeSet<_>>();
        for flag in ["--style", "--number-format", "--data-labels"] {
            assert!(flags.contains(flag), "{command} missing {flag}: {entry}");
        }
    }
}
