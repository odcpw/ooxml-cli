use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
        "args={args:?}\nstderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn temp_dir(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("ooxml-xlsx-polish-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn assert_all_schema_proofs(path: &Path) {
    let file = path.to_str().expect("utf8 path");
    let strict = run(&["validate", "--strict", file]);
    assert!(
        strict.status.success(),
        "strict validation: {}",
        String::from_utf8_lossy(&strict.stderr)
    );
    let conformance = run_json(&["--json", "conformance", "check", file, "--openxml-sdk"]);
    assert_eq!(conformance["status"], "passed", "{conformance}");
    let checks = conformance["checks"].as_array().expect("checks");
    let invariants = checks
        .iter()
        .find(|check| check["name"] == "repair-invariants")
        .unwrap_or_else(|| panic!("missing repair-invariants: {conformance}"));
    assert_eq!(invariants["status"], "passed", "{invariants}");
    let schema = checks
        .iter()
        .find(|check| check["name"] == "schema")
        .unwrap_or_else(|| panic!("missing schema: {conformance}"));
    if schema["status"] == "skipped" {
        assert_eq!(schema["schemaCheck"]["checked"], false, "{schema}");
        assert_eq!(
            schema["diagnostics"][0]["code"], "OOXML_OPENXML_SDK_SKIPPED",
            "{schema}"
        );
        if openxml_sdk_is_required() {
            panic!(
                "OOXML_REQUIRE_OPENXML_SDK is set but schema proof was skipped for {}: {}",
                path.display(),
                schema
            );
        }
        eprintln!(
            "SKIP Open XML SDK schema proof for {}: {}",
            path.display(),
            schema["diagnostics"][0]
                .get("remediation")
                .unwrap_or(&schema["diagnostics"][0]["message"])
        );
        return;
    }
    assert_eq!(schema["status"], "passed", "{schema}");
    assert_eq!(
        schema["schemaCheck"]["validator"], "openxml-sdk",
        "{schema}"
    );
}

fn openxml_sdk_is_required() -> bool {
    std::env::var("OOXML_REQUIRE_OPENXML_SDK")
        .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn zip_text(path: &Path, part: &str) -> String {
    let mut archive =
        zip::ZipArchive::new(File::open(path).expect("open package")).expect("open zip archive");
    let mut body = String::new();
    archive
        .by_name(part)
        .expect("find package part")
        .read_to_string(&mut body)
        .expect("read package part");
    body
}

fn zip_bytes(path: &Path, part: &str) -> Vec<u8> {
    let mut archive =
        zip::ZipArchive::new(File::open(path).expect("open package")).expect("open zip archive");
    let mut body = Vec::new();
    archive
        .by_name(part)
        .expect("find package part")
        .read_to_end(&mut body)
        .expect("read package part");
    body
}

fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CellXf {
    font_id: usize,
    fill_id: usize,
    border_id: usize,
    horizontal: Option<String>,
    vertical: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct StyleCatalog {
    fonts: usize,
    fills: usize,
    borders: usize,
    cell_xfs: Vec<CellXf>,
}

fn xml_attr(element: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (attribute.key.local_name().as_ref() == name)
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
    })
}

fn xml_usize_attr(element: &BytesStart<'_>, name: &[u8]) -> usize {
    xml_attr(element, name)
        .unwrap_or_else(|| panic!("missing XML attribute {}", String::from_utf8_lossy(name)))
        .parse()
        .expect("numeric XML attribute")
}

fn parse_style_catalog(xml: &str) -> StyleCatalog {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut fonts = None;
    let mut fills = None;
    let mut borders = None;
    let mut in_cell_xfs = false;
    let mut cell_xfs = Vec::new();
    let mut current_xf = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => match element.local_name().as_ref() {
                b"fonts" => fonts = Some(xml_usize_attr(&element, b"count")),
                b"fills" => fills = Some(xml_usize_attr(&element, b"count")),
                b"borders" => borders = Some(xml_usize_attr(&element, b"count")),
                b"cellXfs" => in_cell_xfs = true,
                b"xf" if in_cell_xfs => {
                    current_xf = Some(CellXf {
                        font_id: xml_usize_attr(&element, b"fontId"),
                        fill_id: xml_usize_attr(&element, b"fillId"),
                        border_id: xml_usize_attr(&element, b"borderId"),
                        horizontal: None,
                        vertical: None,
                    });
                }
                b"alignment" if current_xf.is_some() => {
                    let xf = current_xf.as_mut().expect("current cell format");
                    xf.horizontal = xml_attr(&element, b"horizontal");
                    xf.vertical = xml_attr(&element, b"vertical");
                }
                _ => {}
            },
            Ok(Event::Empty(element)) if in_cell_xfs => match element.local_name().as_ref() {
                b"xf" => cell_xfs.push(CellXf {
                    font_id: xml_usize_attr(&element, b"fontId"),
                    fill_id: xml_usize_attr(&element, b"fillId"),
                    border_id: xml_usize_attr(&element, b"borderId"),
                    horizontal: None,
                    vertical: None,
                }),
                b"alignment" if current_xf.is_some() => {
                    let xf = current_xf.as_mut().expect("current cell format");
                    xf.horizontal = xml_attr(&element, b"horizontal");
                    xf.vertical = xml_attr(&element, b"vertical");
                }
                _ => {}
            },
            Ok(Event::End(element)) => match element.local_name().as_ref() {
                b"xf" if in_cell_xfs => {
                    cell_xfs.push(current_xf.take().expect("cell format"));
                }
                b"cellXfs" => in_cell_xfs = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("parse styles.xml: {error}"),
        }
    }
    StyleCatalog {
        fonts: fonts.expect("fonts count"),
        fills: fills.expect("fills count"),
        borders: borders.expect("borders count"),
        cell_xfs,
    }
}

fn cell_style_index(worksheet: &str, reference: &str) -> usize {
    let mut reader = Reader::from_str(worksheet);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if element.local_name().as_ref() == b"c"
                    && xml_attr(&element, b"r").as_deref() == Some(reference) =>
            {
                return xml_usize_attr(&element, b"s");
            }
            Ok(Event::Eof) => panic!("missing cell {reference}"),
            Ok(_) => {}
            Err(error) => panic!("parse worksheet: {error}"),
        }
    }
}

fn pdf_word_x_positions(pdf: &Path, words: &[&str]) -> Vec<f64> {
    let output = Command::new("pdftotext")
        .args(["-bbox-layout", pdf.to_str().expect("UTF-8 PDF path"), "-"])
        .output()
        .expect("extract PDF bounding boxes");
    assert!(
        output.status.success(),
        "pdftotext failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let xml = String::from_utf8(output.stdout).expect("UTF-8 PDF bounding boxes");
    let mut reader = Reader::from_str(&xml);
    let mut positions = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if element.local_name().as_ref() == b"word" => {
                let x = xml_attr(&element, b"xMin")
                    .expect("word xMin")
                    .parse::<f64>()
                    .expect("numeric word xMin");
                let text = reader.read_text(element.name()).expect("PDF word text");
                if words.contains(&text.as_ref()) {
                    positions.push((text.into_owned(), x));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("parse PDF bounding boxes: {error}"),
        }
    }
    words
        .iter()
        .map(|word| {
            positions
                .iter()
                .find_map(|(found, x)| (found == word).then_some(*x))
                .unwrap_or_else(|| panic!("missing rendered marker {word:?}"))
        })
        .collect()
}

fn excel_width_to_points(width: f64) -> f64 {
    // ECMA-376 column widths are based on the maximum digit width. For the
    // 11 pt sans-serif workbook default this is the conventional 7 px scale,
    // plus five pixels of cell padding, converted from 96 dpi to points.
    (width * 7.0 + 5.0).floor() * 0.75
}

fn cached_cell_number(worksheet: &str, reference: &str) -> f64 {
    let mut reader = Reader::from_str(worksheet);
    let mut in_cell = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if element.local_name().as_ref() == b"c" => {
                in_cell = xml_attr(&element, b"r").as_deref() == Some(reference);
            }
            Ok(Event::Start(element)) if in_cell && element.local_name().as_ref() == b"v" => {
                return reader
                    .read_text(element.name())
                    .expect("cached formula value")
                    .parse()
                    .expect("numeric cached formula value");
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"c" => in_cell = false,
            Ok(Event::Eof) => panic!("missing cached value for {reference}"),
            Ok(_) => {}
            Err(error) => panic!("parse worksheet values: {error}"),
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn assert_or_update_golden(path: &Path, actual: &Value) {
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(actual).expect("serialize golden")
    );
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(path.parent().expect("golden parent")).expect("create golden parent");
        fs::write(path, &rendered).expect("update golden");
    }
    let expected = fs::read_to_string(path).expect("read golden");
    assert_eq!(rendered, expected, "golden mismatch: {}", path.display());
}

fn build_sales_workbook(root: &Path, label: &str) -> PathBuf {
    let scaffold = root.join(format!("{label}-01-scaffold.xlsx"));
    let data = root.join(format!("{label}-02-data.xlsx"));
    let table = root.join(format!("{label}-03-table.xlsx"));
    let fitted = root.join(format!("{label}-04-fitted.xlsx"));
    let colored = root.join(format!("{label}-05-colored.xlsx"));
    let final_book = root.join(format!("{label}-sales-workbook.xlsx"));
    run_json(&[
        "--json",
        "xlsx",
        "scaffold",
        "--out",
        scaffold.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--theme",
        "corporate",
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        scaffold.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--range",
        "A1:C4",
        "--values",
        r#"[["Region","Units","Revenue"],["North",120,18400],["Central",96,15120],["South",141,22050]]"#,
        "--data-format",
        "json",
        "--out",
        data.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "tables",
        "create",
        data.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--range",
        "A1:C4",
        "--table",
        "Sales",
        "--header-style",
        "header",
        "--total-row",
        "--totals",
        "Units:sum,Revenue:sum",
        "--out",
        table.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "colwidths",
        "autofit",
        table.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--range",
        "A:C",
        "--min",
        "10",
        "--max",
        "28",
        "--out",
        fitted.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "sheets",
        "set-tab-color",
        fitted.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--color",
        "4472C4",
        "--out",
        colored.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "sheets",
        "set-print",
        colored.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--landscape",
        "--fit-to-width",
        "1",
        "--repeat-header-rows",
        "1",
        "--gridlines",
        "off",
        "--out",
        final_book.to_str().unwrap(),
    ]);
    final_book
}

#[test]
fn autofit_and_theme_header_preset_are_schema_clean() {
    let root = temp_dir("autofit-style");
    let autofit = root.join("autofit.xlsx");
    let styled = root.join("styled.xlsx");
    let report = run_json(&[
        "--json",
        "xlsx",
        "colwidths",
        "autofit",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A:B",
        "--min",
        "8",
        "--max",
        "20",
        "--out",
        autofit.to_str().unwrap(),
    ]);
    assert_eq!(report["heuristic"], "per-character-font-metrics-v1");
    for column in ["A", "B"] {
        let width = report["widths"][column].as_f64().expect("width");
        assert!((8.0..=20.0).contains(&width), "{report}");
    }
    assert_all_schema_proofs(&autofit);

    let styled_report = run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        autofit.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "A1:B1",
        "--preset",
        "header",
        "--out",
        styled.to_str().unwrap(),
    ]);
    assert_eq!(styled_report["preset"], "header");
    assert!(
        styled_report["styleIndexes"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    assert_all_schema_proofs(&styled);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn style_preset_rejects_unknown_names() {
    let root = temp_dir("bad-preset");
    let output = run(&[
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A1",
        "--preset",
        "glitter",
        "--out",
        root.join("bad.xlsx").to_str().unwrap(),
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("header, total, band, input, or muted")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn every_theme_style_preset_is_a_schema_clean_positive_path() {
    let root = temp_dir("all-presets");
    for preset in ["header", "total", "band", "input", "muted"] {
        let output = root.join(format!("{preset}-first.xlsx"));
        let repeated = root.join(format!("{preset}-repeated.xlsx"));
        let report = run_json(&[
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:C1",
            "--preset",
            preset,
            "--out",
            output.to_str().unwrap(),
        ]);
        assert_eq!(report["preset"], preset);
        let first_catalog = parse_style_catalog(&zip_text(&output, "xl/styles.xml"));
        let first_sheet = zip_text(&output, "xl/worksheets/sheet1.xml");
        let xf = &first_catalog.cell_xfs[cell_style_index(&first_sheet, "A1")];
        match preset {
            "header" => {
                assert!(
                    xf.font_id > 0 && xf.fill_id > 1 && xf.border_id > 0,
                    "{xf:?}"
                );
                assert_eq!(xf.horizontal.as_deref(), Some("center"));
                assert_eq!(xf.vertical.as_deref(), Some("center"));
            }
            "total" => {
                assert!(
                    xf.font_id > 0 && xf.fill_id > 1 && xf.border_id > 0,
                    "{xf:?}"
                );
                assert_eq!(xf.vertical.as_deref(), Some("center"));
            }
            "band" => {
                assert_eq!(xf.font_id, 0, "{xf:?}");
                assert!(xf.fill_id > 1, "{xf:?}");
                assert_eq!(xf.border_id, 0, "{xf:?}");
            }
            "input" => {
                assert!(
                    xf.font_id > 0 && xf.fill_id > 1 && xf.border_id > 0,
                    "{xf:?}"
                );
            }
            "muted" => {
                assert!(xf.font_id > 0 && xf.fill_id > 1, "{xf:?}");
                assert_eq!(xf.border_id, 0, "{xf:?}");
            }
            _ => unreachable!(),
        }
        run_json(&[
            "--json",
            "xlsx",
            "ranges",
            "set-style",
            output.to_str().unwrap(),
            "--sheet",
            "1",
            "--range",
            "A2:C2",
            "--preset",
            preset,
            "--out",
            repeated.to_str().unwrap(),
        ]);
        let repeated_catalog = parse_style_catalog(&zip_text(&repeated, "xl/styles.xml"));
        assert_eq!(
            repeated_catalog, first_catalog,
            "reapplying {preset} must reuse font/fill/border/xf records"
        );
        assert_all_schema_proofs(&output);
        assert_all_schema_proofs(&repeated);
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn autofit_estimates_track_libreoffice_rendered_column_widths() {
    if !command_exists("soffice") || !command_exists("pdftotext") {
        eprintln!("skipping autofit render calibration: LibreOffice tools are unavailable");
        return;
    }
    let root = temp_dir("autofit-calibration");
    let data = root.join("01-data.xlsx");
    let currency = root.join("02-currency.xlsx");
    let dates = root.join("03-dates.xlsx");
    let wrapped = root.join("04-wrapped.xlsx");
    let fitted = root.join("05-fitted.xlsx");
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A1:E2",
        "--values",
        r#"[["A0","B1","C2","D3","E4"],["Trustworthy text width",1234567.89,45292,"This wrapped cell contains several words",1]]"#,
        "--data-format",
        "json",
        "--out",
        data.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        data.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "B2",
        "--preset",
        "currency",
        "--currency-symbol",
        "$",
        "--out",
        currency.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set-format",
        currency.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "C2",
        "--preset",
        "date",
        "--out",
        dates.to_str().unwrap(),
    ]);
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        dates.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "D2",
        "--alignment-wrap-text",
        "--out",
        wrapped.to_str().unwrap(),
    ]);
    let report = run_json(&[
        "--json",
        "xlsx",
        "colwidths",
        "autofit",
        wrapped.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "A:E",
        "--min",
        "4",
        "--max",
        "20",
        "--out",
        fitted.to_str().unwrap(),
    ]);
    assert_eq!(report["heuristic"], "per-character-font-metrics-v1");
    assert_all_schema_proofs(&fitted);

    let render = run_json(&[
        "--json",
        "render",
        fitted.to_str().unwrap(),
        "--out",
        root.join("render").to_str().unwrap(),
        "--sheet",
        "1",
        "--dpi",
        "48",
        "--pages",
        "1",
    ]);
    let pdf = Path::new(render["pdfPath"].as_str().expect("render PDF"));
    let markers = ["A0", "B1", "C2", "D3", "E4"];
    let x = pdf_word_x_positions(pdf, &markers);
    eprintln!("column\tpredicted_width\tpredicted_points\trendered_points\terror_ratio");
    for (index, column) in ["A", "B", "C", "D"].iter().enumerate() {
        let width = report["widths"][column].as_f64().expect("reported width");
        let predicted = excel_width_to_points(width);
        let rendered = x[index + 1] - x[index];
        let error_ratio = (rendered - predicted).abs() / predicted;
        eprintln!("{column}\t{width:.2}\t{predicted:.2}\t{rendered:.2}\t{error_ratio:.3}");
        assert!(
            error_ratio <= 0.20 || (rendered - predicted).abs() <= 8.0,
            "{column}: predicted {predicted:.2}pt from width {width:.2}, LibreOffice rendered {rendered:.2}pt"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn tab_color_and_print_setup_are_schema_clean() {
    let root = temp_dir("sheet-polish");
    let colored = root.join("colored.xlsx");
    let printed = root.join("printed.xlsx");
    let tab = run_json(&[
        "--json",
        "xlsx",
        "sheets",
        "set-tab-color",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--color",
        "#2F5597",
        "--out",
        colored.to_str().unwrap(),
    ]);
    assert_eq!(tab["tabColor"], "2F5597");
    assert_all_schema_proofs(&colored);

    let print = run_json(&[
        "--json",
        "xlsx",
        "sheets",
        "set-print",
        colored.to_str().unwrap(),
        "--sheet",
        "1",
        "--landscape",
        "--fit-to-width",
        "1",
        "--repeat-header-rows",
        "1",
        "--gridlines",
        "off",
        "--out",
        printed.to_str().unwrap(),
    ]);
    assert_eq!(print["landscape"], true);
    assert_eq!(print["fitToWidth"], 1);
    assert_eq!(print["repeatHeaderRows"], 1);
    assert_eq!(print["gridlines"], "off");

    let worksheet = zip_text(&printed, "xl/worksheets/sheet1.xml");
    assert!(worksheet.contains(r#"<tabColor rgb="FF2F5597"/>"#));
    assert!(worksheet.contains(r#"showGridLines="0""#));
    assert!(worksheet.contains(r#"gridLines="0""#));
    assert!(worksheet.contains(r#"orientation="landscape""#));
    assert!(worksheet.contains(r#"fitToWidth="1""#));
    let workbook = zip_text(&printed, "xl/workbook.xml");
    assert!(workbook.contains(r#"name="_xlnm.Print_Titles" localSheetId="0""#));
    assert!(workbook.contains("'Sheet1'!$1:$1"));
    assert_all_schema_proofs(&printed);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn table_header_preset_and_totals_are_schema_clean() {
    let root = temp_dir("table-polish");
    let data = root.join("data.xlsx");
    let table = root.join("table.xlsx");
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "--sheet",
        "1",
        "--range",
        "A1:C3",
        "--values",
        r#"[["Region","Units","Revenue"],["North",10,20],["South",5,12]]"#,
        "--data-format",
        "json",
        "--out",
        data.to_str().unwrap(),
    ]);

    let report = run_json(&[
        "--json",
        "xlsx",
        "tables",
        "create",
        data.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "A1:C3",
        "--table",
        "Sales",
        "--header-style",
        "header",
        "--total-row",
        "--totals",
        "Units:sum,Revenue:sum",
        "--out",
        table.to_str().unwrap(),
    ]);
    assert_eq!(report["range"], "A1:C4");
    assert_eq!(report["dataRowCount"], 2);
    assert_eq!(report["headerStyle"], "header");
    assert_eq!(report["styleName"], "TableStyleMedium2");
    assert_eq!(report["totalRow"], true);
    assert_eq!(report["totals"].as_array().expect("totals").len(), 2);
    let table_xml = zip_text(&table, "xl/tables/table1.xml");
    assert!(table_xml.contains(r#"ref="A1:C4""#));
    assert!(table_xml.contains(r#"totalsRowLabel="Total""#));
    assert_eq!(table_xml.matches(r#"totalsRowFunction="sum""#).count(), 2);
    let worksheet_xml = zip_text(&table, "xl/worksheets/sheet1.xml");
    assert!(worksheet_xml.contains(">Total</t>"));
    assert!(worksheet_xml.contains("SUBTOTAL(109,Sales[Units])"));
    assert!(worksheet_xml.contains("SUBTOTAL(109,Sales[Revenue])"));
    let catalog = parse_style_catalog(&zip_text(&table, "xl/styles.xml"));
    let header_xf = &catalog.cell_xfs[cell_style_index(&worksheet_xml, "A1")];
    assert!(
        header_xf.font_id > 0 && header_xf.fill_id > 1 && header_xf.border_id > 0,
        "table --header-style must reference concrete font/fill/border records: {header_xf:?}"
    );
    assert_eq!(header_xf.horizontal.as_deref(), Some("center"));
    assert_eq!(header_xf.vertical.as_deref(), Some("center"));
    assert_all_schema_proofs(&table);

    if command_exists("soffice") {
        let recalculated_dir = root.join("recalculated");
        fs::create_dir_all(&recalculated_dir).expect("create recalculation output dir");
        let profile_arg = format!(
            "-env:UserInstallation=file://{}",
            root.join("lo-profile").display()
        );
        let office = Command::new("soffice")
            .args([
                "--headless",
                &profile_arg,
                "--convert-to",
                "xlsx",
                "--outdir",
                recalculated_dir.to_str().unwrap(),
                table.to_str().unwrap(),
            ])
            .output()
            .expect("recalculate totals with LibreOffice");
        assert!(
            office.status.success(),
            "LibreOffice recalculation failed: {}",
            String::from_utf8_lossy(&office.stderr)
        );
        let recalculated = recalculated_dir.join("table.xlsx");
        assert!(
            recalculated.is_file(),
            "LibreOffice did not save recalculated workbook"
        );
        let recalculated_sheet = zip_text(&recalculated, "xl/worksheets/sheet1.xml");
        assert_eq!(cached_cell_number(&recalculated_sheet, "B4"), 15.0);
        assert_eq!(cached_cell_number(&recalculated_sheet, "C4"), 32.0);
    } else {
        eprintln!("skipping table totals calculation proof: LibreOffice is unavailable");
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn scaffold_theme_seed_brand_and_repeatable_sheets_are_schema_clean() {
    let root = temp_dir("scaffold-theme");
    let seeded = root.join("seeded.xlsx");
    let themed = root.join("themed.xlsx");
    let branded = root.join("branded.xlsx");
    let brand_styled = root.join("brand-styled.xlsx");
    let brand = root.join("brand.json");
    fs::write(
        &brand,
        r##"{"name":"Northwind","themeSeed":"#7F6000","fonts":{"major":"Arial","minor":"Arial"}}"##,
    )
    .expect("write brand fixture");

    let seed_report = run_json(&[
        "--json",
        "xlsx",
        "scaffold",
        "--out",
        seeded.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--sheet",
        "Inputs",
        "--theme-seed",
        "#4472C4",
    ]);
    assert_eq!(seed_report["sheetCount"], 2);
    assert_eq!(
        seed_report["sheets"],
        serde_json::json!(["Sales", "Inputs"])
    );
    assert_eq!(seed_report["theme"], "custom");
    assert_eq!(seed_report["themeSeed"], "4472C4");
    assert!(zip_text(&seeded, "xl/theme/theme1.xml").contains("ooxml-cli custom"));
    assert_all_schema_proofs(&seeded);

    let theme_report = run_json(&[
        "--json",
        "xlsx",
        "scaffold",
        "--out",
        themed.to_str().unwrap(),
        "--theme",
        "warm",
    ]);
    assert_eq!(theme_report["theme"], "warm");
    assert_eq!(theme_report["themeSeed"], "C55A11");
    assert_all_schema_proofs(&themed);

    let brand_report = run_json(&[
        "--json",
        "xlsx",
        "scaffold",
        "--out",
        branded.to_str().unwrap(),
        "--brand",
        brand.to_str().unwrap(),
    ]);
    assert_eq!(brand_report["theme"], "Northwind");
    assert_eq!(brand_report["themeSeed"], "7F6000");
    let brand_theme = zip_text(&branded, "xl/theme/theme1.xml");
    assert!(brand_theme.contains(r#"typeface="Arial""#));
    assert_all_schema_proofs(&branded);
    run_json(&[
        "--json",
        "xlsx",
        "ranges",
        "set-style",
        branded.to_str().unwrap(),
        "--sheet",
        "1",
        "--range",
        "A1",
        "--preset",
        "header",
        "--out",
        brand_styled.to_str().unwrap(),
    ]);
    assert!(zip_text(&brand_styled, "xl/styles.xml").contains(r#"name val="Arial""#));
    assert_all_schema_proofs(&brand_styled);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sales_workbook_recipe_has_a_real_libreoffice_render_fixture() {
    if !command_exists("soffice") || !command_exists("pdftoppm") || !command_exists("pdftotext") {
        eprintln!("skipping sales workbook render: LibreOffice render tools are unavailable");
        return;
    }
    let root = temp_dir("sales-render");
    let final_book = build_sales_workbook(&root, "render");
    assert_all_schema_proofs(&final_book);

    let render_dir = root.join("render");
    let render = run_json(&[
        "--json",
        "render",
        final_book.to_str().unwrap(),
        "--out",
        render_dir.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--dpi",
        "48",
        "--pages",
        "1",
    ]);
    assert_eq!(render["status"], "ok");
    assert_eq!(render["engine"], "libreoffice");
    assert_eq!(render["sheet"]["name"], "Sales");
    let pdf_path = Path::new(render["pdfPath"].as_str().expect("render PDF"));
    assert!(pdf_path.is_file());
    let pages = render["pages"].as_array().expect("render pages");
    assert_eq!(pages.len(), 1);
    assert!(Path::new(pages[0]["imagePath"].as_str().expect("render PNG")).is_file());
    let pdf_text = Command::new("pdftotext")
        .arg(pdf_path)
        .arg("-")
        .output()
        .expect("extract rendered PDF text");
    assert!(pdf_text.status.success());
    let pdf_text = String::from_utf8_lossy(&pdf_text.stdout);
    for expected in ["Region", "North", "Central", "South", "Total"] {
        assert!(
            pdf_text.contains(expected),
            "missing {expected:?}: {pdf_text}"
        );
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn sales_workbook_recipe_is_byte_deterministic_and_matches_semantic_golden() {
    let root = temp_dir("sales-golden");
    let first = build_sales_workbook(&root, "first");
    let second = build_sales_workbook(&root, "second");
    let first_bytes = fs::read(&first).expect("read first sales workbook");
    let second_bytes = fs::read(&second).expect("read second sales workbook");
    assert_eq!(
        first_bytes, second_bytes,
        "the same sales recipe must produce identical package bytes"
    );
    assert_all_schema_proofs(&first);

    let widths = run_json(&[
        "--json",
        "xlsx",
        "colwidths",
        "show",
        first.to_str().unwrap(),
        "--sheet",
        "Sales",
        "--range",
        "A:C",
    ]);
    let styles_xml = zip_text(&first, "xl/styles.xml");
    let styles = parse_style_catalog(&styles_xml);
    let worksheet_xml = zip_text(&first, "xl/worksheets/sheet1.xml");
    let table_xml = zip_text(&first, "xl/tables/table1.xml");
    let theme_xml = zip_text(&first, "xl/theme/theme1.xml");
    let summary = serde_json::json!({
        "recipe": "corporate sales workbook v1",
        "packageSha256": sha256_hex(&first_bytes),
        "columns": widths["columns"],
        "styles": {
            "fonts": styles.fonts,
            "fills": styles.fills,
            "borders": styles.borders,
            "cellXfs": styles.cell_xfs.len(),
            "headerStyleIndex": cell_style_index(&worksheet_xml, "A1"),
        },
        "table": {
            "range": "A1:C5",
            "totalFunctions": table_xml.matches("totalsRowFunction=\"sum\"").count(),
            "hasUnitsSubtotal": worksheet_xml.contains("SUBTOTAL(109,Sales[Units])"),
            "hasRevenueSubtotal": worksheet_xml.contains("SUBTOTAL(109,Sales[Revenue])"),
        },
        "sheet": {
            "tabColor": "4472C4",
            "landscape": worksheet_xml.contains("orientation=\"landscape\""),
            "fitToWidth": worksheet_xml.contains("fitToWidth=\"1\""),
            "gridlinesHidden": worksheet_xml.contains("showGridLines=\"0\""),
        },
        "partSha256": {
            "worksheet": sha256_hex(worksheet_xml.as_bytes()),
            "styles": sha256_hex(styles_xml.as_bytes()),
            "table": sha256_hex(table_xml.as_bytes()),
            "theme": sha256_hex(theme_xml.as_bytes()),
            "contentTypes": sha256_hex(&zip_bytes(&first, "[Content_Types].xml")),
        },
    });
    assert_or_update_golden(
        Path::new("testdata/golden/xlsx-polish/sales-workbook-summary.json"),
        &summary,
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn xlsx_exposes_every_pptx_chart_set_verb() {
    let capabilities = run_json(&["--json", "capabilities"]);
    let commands = capabilities["commands"].as_array().expect("commands");
    let verbs = |family: &str| {
        commands
            .iter()
            .filter_map(|command| command["path"].as_str())
            .filter_map(|path| path.strip_prefix(&format!("ooxml {family} charts set-")))
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
    };
    let pptx = verbs("pptx");
    let xlsx = verbs("xlsx");
    assert_eq!(xlsx, pptx, "XLSX and PPTX chart set-* verbs diverged");
    assert_eq!(
        pptx.len(),
        6,
        "expected the six established chart style verbs"
    );
}
