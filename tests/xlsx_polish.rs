use serde_json::Value;
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
    for name in ["repair-invariants", "schema"] {
        let check = conformance["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .find(|check| check["name"] == name)
            .unwrap_or_else(|| panic!("missing {name}: {conformance}"));
        assert_eq!(check["status"], "passed", "{check}");
        if name == "schema" {
            assert_eq!(check["schemaCheck"]["validator"], "openxml-sdk", "{check}");
        }
    }
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

fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
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
    assert_eq!(report["heuristic"], "simple-average-glyph-width-v1");
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
        let output = root.join(format!("{preset}.xlsx"));
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
        assert_all_schema_proofs(&output);
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
    assert_all_schema_proofs(&table);
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
    let scaffold = root.join("01-scaffold.xlsx");
    let data = root.join("02-data.xlsx");
    let table = root.join("03-table.xlsx");
    let fitted = root.join("04-fitted.xlsx");
    let colored = root.join("05-colored.xlsx");
    let final_book = root.join("sales-workbook.xlsx");
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
