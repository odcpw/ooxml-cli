use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

#[derive(Debug, Eq, PartialEq)]
struct ThemeSignature {
    colors: BTreeMap<String, String>,
    heading_font: String,
    body_font: String,
}

#[test]
fn published_schema_describes_seed_full_scheme_and_brand_defaults() {
    let report = run_ok(&["--json", "capabilities", "--schema", "brand"]);
    assert_eq!(report["schema"], "brand");
    let schema = &report["document"];
    assert_eq!(
        schema["$id"],
        "https://ooxml-cli.dev/schemas/brand.schema.json"
    );
    assert_eq!(
        schema["properties"]["colors"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        schema["properties"]["slideNumberPolicy"]["enum"],
        serde_json::json!(["none", "all", "except-title"])
    );
    assert_eq!(
        schema,
        &serde_json::from_str::<Value>(
            &fs::read_to_string(repo_path("testdata/brand/brand.schema.json")).unwrap()
        )
        .unwrap()
    );
}

#[test]
fn one_brand_scaffolds_three_schema_clean_matching_families_deterministically() {
    let temp = temp_dir("scaffolds");
    let brand = repo_path("testdata/brand/northwind.json");
    let first = scaffold_family_set(&temp.join("first"), &brand);
    let second = scaffold_family_set(&temp.join("second"), &brand);

    let signatures = first
        .iter()
        .map(|(family, path)| (family, theme_signature(path, family)))
        .collect::<Vec<_>>();
    for (_, signature) in &signatures[1..] {
        assert_eq!(signature, &signatures[0].1);
    }
    assert_eq!(signatures[0].1.colors["accent1"], "316F8A");
    assert_eq!(signatures[0].1.heading_font, "Arial");
    assert_eq!(signatures[0].1.body_font, "Liberation Sans");

    for ((family, first_path), (_, second_path)) in first.iter().zip(&second) {
        assert_eq!(
            fs::read(first_path).unwrap(),
            fs::read(second_path).unwrap(),
            "{family} branded scaffold bytes must be deterministic"
        );
        assert_strict_valid(first_path);
        assert_sdk_valid_if_available(first_path);
    }

    let docx_styles = zip_text(&first[1].1, "word/styles.xml");
    assert!(docx_styles.contains(r#"w:ascii="Arial""#));
    assert!(docx_styles.contains(r#"w:ascii="Liberation Sans""#));
    assert!(docx_styles.contains(r#"w:val="316F8A" w:themeColor="accent1""#));
    let docx_footer = zip_text(&first[1].1, "word/footer1.xml");
    assert!(docx_footer.contains("Northwind Confidential"));
    let docx_document = zip_text(&first[1].1, "word/document.xml");
    assert!(docx_document.contains(r#"w:orient="landscape""#));
    assert!(docx_document.contains(r#"w:top="864""#));
    assert!(docx_document.contains(r#"w:left="1152""#));

    let xlsx_styles = zip_text(&first[2].1, "xl/styles.xml");
    assert!(xlsx_styles.contains(r#"name val="Liberation Sans""#));
    assert!(xlsx_styles.contains(r#"defaultTableStyle="TableStyleMedium2""#));
    let sheet = zip_text(&first[2].1, "xl/worksheets/sheet1.xml");
    assert!(sheet.contains(r#"orientation="landscape""#));
    assert!(sheet.contains(r#"paperSize="9""#));
    assert!(sheet.contains(r#"top="0.6""#));
    assert!(sheet.contains(r#"left="0.8""#));
    assert!(sheet.contains("<oddFooter>Northwind Confidential</oddFooter>"));

    let master = zip_text(&first[0].1, "ppt/slideMasters/slideMaster1.xml");
    assert!(master.contains(r#"sldNum="1""#));
    let slide = zip_text(&first[0].1, "ppt/slides/slide1.xml");
    assert!(slide.contains("Northwind Confidential"));
    let presentation = zip_text(&first[0].1, "ppt/presentation.xml");
    assert!(presentation.contains(r#"cx="12192000" cy="6858000""#));
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn extraction_and_existing_package_application_round_trip_theme_tokens() {
    let temp = temp_dir("roundtrip");
    let brand = repo_path("testdata/brand/northwind.json");
    let source = temp.join("source.pptx");
    run_ok(&[
        "--json",
        "pptx",
        "scaffold",
        path(&source),
        "--brand",
        path(&brand),
    ]);
    let extracted = temp.join("extracted.json");
    let report = run_ok(&[
        "--json",
        "template",
        "brand",
        "extract",
        path(&source),
        "--out",
        path(&extracted),
    ]);
    assert_eq!(report["brand"]["name"], "Northwind");
    assert_eq!(report["brand"]["fonts"]["heading"], "Arial");
    assert_eq!(report["brand"]["fonts"]["body"], "Liberation Sans");
    assert_eq!(report["brand"]["colors"].as_object().unwrap().len(), 12);
    let golden: Value = serde_json::from_str(
        &fs::read_to_string(repo_path("testdata/brand/northwind.extracted.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["brand"], golden);

    let mut branded = BTreeMap::new();
    for family in ["pptx", "docx", "xlsx"] {
        let default = temp.join(format!("default.{family}"));
        let output = temp.join(format!("branded.{family}"));
        run_ok(&["--json", family, "scaffold", path(&default)]);
        let applied = run_ok(&[
            "--json",
            "template",
            "apply",
            path(&default),
            "--brand",
            path(&extracted),
            "--out",
            path(&output),
        ]);
        assert_eq!(applied["family"], family);
        assert_eq!(applied["brand"], "Northwind");
        assert_eq!(
            theme_signature(&source, "pptx"),
            theme_signature(&output, family)
        );
        assert_strict_valid(&output);
        assert_sdk_valid_if_available(&output);
        branded.insert(family, output);
    }

    let extracted_again = run_ok(&[
        "--json",
        "template",
        "brand",
        "extract",
        path(&branded["docx"]),
    ]);
    assert_eq!(
        report["brand"]["colors"],
        extracted_again["brand"]["colors"]
    );
    assert_eq!(report["brand"]["fonts"], extracted_again["brand"]["fonts"]);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn invalid_brand_is_actionable_and_never_publishes_output() {
    let temp = temp_dir("invalid");
    let brand = temp.join("invalid.json");
    let output = temp.join("invalid.xlsx");
    fs::write(
        &brand,
        r#"{"name":"Incomplete","colors":{"accent1":"4472C4"},"fonts":{"heading":"Arial","body":"Arial"}}"#,
    )
    .unwrap();
    let command = run(&[
        "--json",
        "xlsx",
        "scaffold",
        path(&output),
        "--brand",
        path(&brand),
    ]);
    assert_eq!(command.status.code(), Some(2));
    let bytes = if command.stdout.is_empty() {
        &command.stderr
    } else {
        &command.stdout
    };
    let error: Value = serde_json::from_slice(bytes).unwrap();
    let error = error.get("error").unwrap_or(&error);
    assert_eq!(error["code"], "invalid_args");
    assert!(error["message"].as_str().unwrap().contains("colors.dark1"));
    assert!(!output.exists());
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn dry_run_validates_without_publishing_and_title_layout_can_hide_slide_numbers() {
    let temp = temp_dir("policy-dry-run");
    let source = temp.join("source.pptx");
    let output = temp.join("must-not-exist.pptx");
    let brand = temp.join("brand.json");
    fs::write(
        &brand,
        r#"{"name":"Policy","colors":{"seed":"4472C4"},"fonts":{"heading":"Arial","body":"Arial"},"slideNumberPolicy":"except-title"}"#,
    )
    .unwrap();
    run_ok(&["--json", "pptx", "scaffold", path(&source)]);
    let before = fs::read(&source).unwrap();
    let report = run_ok(&[
        "--json",
        "template",
        "apply",
        path(&source),
        "--brand",
        path(&brand),
        "--dry-run",
    ]);
    assert_eq!(report["dryRun"], true);
    assert_eq!(fs::read(&source).unwrap(), before);
    assert!(!output.exists());

    let branded = temp.join("branded.pptx");
    run_ok(&[
        "--json",
        "pptx",
        "scaffold",
        path(&branded),
        "--brand",
        path(&brand),
    ]);
    let title_layout = zip_text(&branded, "ppt/slideLayouts/slideLayout1.xml");
    assert!(title_layout.contains(r#"<p:hf sldNum="0"/>"#));
    assert_strict_valid(&branded);
    assert_sdk_valid_if_available(&branded);
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn brand_logo_is_embedded_and_positioned_in_all_three_families() {
    let temp = temp_dir("logo");
    let brand = repo_path("testdata/brand/logo.json");
    let cases = [
        (
            "pptx",
            temp.join("deck.pptx"),
            "ppt/slides/slide1.xml",
            "Brand Logo",
            r#"x="11049000" y="5715000""#,
        ),
        (
            "xlsx",
            temp.join("workbook.xlsx"),
            "xl/drawings/drawing1.xml",
            "Brand Logo",
            "<xdr:col>7</xdr:col>",
        ),
        (
            "docx",
            temp.join("report.docx"),
            "word/footer1.xml",
            "Brand Logo",
            r#"w:jc w:val="right""#,
        ),
    ];
    for (family, output, part, marker, position_marker) in cases {
        run_ok(&[
            "--json",
            family,
            "scaffold",
            path(&output),
            "--brand",
            path(&brand),
        ]);
        let package_entries = zip_entries(&output);
        assert!(
            package_entries
                .iter()
                .any(|entry| entry.contains("/media/") && entry.ends_with(".png")),
            "{family} must embed the brand logo"
        );
        let placement = zip_text(&output, part);
        assert!(placement.contains(marker), "{family} logo marker missing");
        assert!(
            placement.contains(position_marker),
            "{family} logo placement missing"
        );
        assert!(
            placement.contains(r#"cx="914400" cy="914400""#),
            "{family} logo dimensions missing"
        );
        let repeat = temp.join(format!("repeat.{family}"));
        run_ok(&[
            "--json",
            family,
            "scaffold",
            path(&repeat),
            "--brand",
            path(&brand),
        ]);
        assert_eq!(
            fs::read(&output).unwrap(),
            fs::read(&repeat).unwrap(),
            "{family} branded logo bytes must be deterministic"
        );
        assert_strict_valid(&output);
        assert_sdk_valid_if_available(&output);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn brand_application_recolors_existing_charts_and_defines_document_table_style() {
    let temp = temp_dir("existing-brand");
    let brand = repo_path("testdata/brand/northwind.json");
    for (family, spec, part) in [
        (
            "xlsx",
            "testdata/xlsx/build-spec/sales.json",
            "xl/charts/chart1.xml",
        ),
        (
            "docx",
            "testdata/docx/build-spec/quarterly-report.json",
            "word/document.xml",
        ),
    ] {
        let source = temp.join(format!("source.{family}"));
        let output = temp.join(format!("branded.{family}"));
        run_ok(&[
            "--json",
            family,
            "build",
            "--spec",
            path(&repo_path(spec)),
            "--out",
            path(&source),
        ]);
        let before = fs::read(&source).unwrap();
        run_ok(&[
            "--json",
            "template",
            "apply",
            path(&source),
            "--brand",
            path(&brand),
            "--out",
            path(&output),
        ]);
        assert_eq!(fs::read(&source).unwrap(), before);
        let xml = zip_text(&output, part);
        if family == "xlsx" {
            assert!(xml.contains(r#"<a:srgbClr val="316F8A""#), "{xml}");
            assert!(xml.contains("<c:f>"), "chart source must survive");
            assert!(
                zip_text(&output, "xl/tables/table1.xml").contains(r#"name="TableStyleMedium2""#)
            );
        } else {
            assert!(xml.contains(r#"w:tblStyle w:val="TableStyleMedium2""#));
            let styles = zip_text(&output, "word/styles.xml");
            assert!(
                styles
                    .contains(r#"w:type="table" w:customStyle="1" w:styleId="TableStyleMedium2""#)
            );
            assert!(styles.contains(r#"w:fill="316F8A" w:themeFill="accent1""#));
        }
        assert_strict_valid(&output);
        assert_sdk_valid_if_available(&output);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn docx_brand_logo_preserves_body_and_existing_footer_text() {
    let temp = temp_dir("header-footer");
    for placement in ["top-left", "bottom-right"] {
        let source = temp.join(format!("{placement}.docx"));
        run_ok(&[
            "--json",
            "docx",
            "scaffold",
            path(&source),
            "--text",
            "Body sentinel",
        ]);
        let body = zip_text(&source, "word/document.xml");
        let mut kit: Value = serde_json::from_str(
            &fs::read_to_string(repo_path("testdata/brand/logo.json")).unwrap(),
        )
        .unwrap();
        kit["logo"]["path"] =
            serde_json::json!(repo_path("testdata/pptx/template-branded/test-image.png"));
        kit["logo"]["placement"] = serde_json::json!(placement);
        kit["footerText"] = serde_json::json!("Footer sentinel");
        let brand = temp.join("brand.json");
        fs::write(&brand, serde_json::to_vec(&kit).unwrap()).unwrap();
        let output = temp.join(format!("{placement}-branded.docx"));
        run_ok(&[
            "--json",
            "template",
            "apply",
            path(&source),
            "--brand",
            path(&brand),
            "--out",
            path(&output),
        ]);
        let document = zip_text(&output, "word/document.xml");
        assert!(body.contains("Body sentinel") && document.contains("Body sentinel"));
        assert!(!document.contains("Brand Logo"));
        let part = if placement.starts_with("top") {
            "word/header1.xml"
        } else {
            "word/footer1.xml"
        };
        assert!(zip_text(&output, part).contains("Brand Logo"));
        assert!(zip_text(&output, "word/footer1.xml").contains("Footer sentinel"));
        assert_strict_valid(&output);
        assert_sdk_valid_if_available(&output);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn five_branded_recipes_match_the_documented_family_audit() {
    let temp = temp_dir("recipe-parity");
    let mut kit: Value = serde_json::from_str(
        &fs::read_to_string(repo_path("testdata/brand/northwind.json")).unwrap(),
    )
    .unwrap();
    kit["logo"] = serde_json::json!({"path": repo_path("testdata/test_image.png"), "placement": "top-right", "widthEmu": 457200, "heightEmu": 457200});
    let brand = temp.join("brand.json");
    fs::write(&brand, serde_json::to_vec(&kit).unwrap()).unwrap();
    let mut observed = BTreeMap::<&str, Vec<&str>>::new();
    for (name, family, flag, input) in [
        (
            "deck",
            "pptx",
            "--spec",
            "testdata/pptx/build-spec/q3-review.json",
        ),
        (
            "workbook",
            "xlsx",
            "--spec",
            "testdata/xlsx/build-spec/sales.json",
        ),
        (
            "document",
            "docx",
            "--spec",
            "testdata/docx/build-spec/quarterly-report.json",
        ),
        (
            "markdown-deck",
            "pptx",
            "--from-markdown",
            "testdata/markdown/q3-review.md",
        ),
        (
            "markdown-document",
            "docx",
            "--from-markdown",
            "testdata/markdown/quarterly-report.md",
        ),
    ] {
        let source = temp.join(format!("{name}.{family}"));
        let output = temp.join(format!("{name}-branded.{family}"));
        run_ok(&[
            "--json",
            family,
            "build",
            flag,
            path(&repo_path(input)),
            "--out",
            path(&source),
        ]);
        run_ok(&[
            "--json",
            "template",
            "apply",
            path(&source),
            "--brand",
            path(&brand),
            "--out",
            path(&output),
        ]);
        assert_strict_valid(&output);
        assert_sdk_valid_if_available(&output);
        let signature = theme_signature(&output, family);
        assert_eq!(signature.colors["accent1"], "316F8A");
        assert_eq!(signature.heading_font, "Arial");
        assert_eq!(signature.body_font, "Liberation Sans");
        let parts = zip_entries(&output);
        let xml = parts
            .iter()
            .filter(|p| p.ends_with(".xml"))
            .map(|p| zip_text(&output, p))
            .collect::<String>();
        assert!(xml.contains("Brand Logo"));
        assert!(xml.contains("Northwind Confidential"));
        let (size, table, chart) = match family {
            "pptx" => (
                xml.contains(r#"cx="12192000" cy="6858000""#),
                "partial",
                "proven",
            ),
            "docx" => (xml.contains(r#"w:orient="landscape""#), "proven", "partial"),
            "xlsx" => (
                xml.contains(r#"paperSize="9""#) && xml.contains(r#"orientation="landscape""#),
                "proven",
                "proven",
            ),
            _ => unreachable!(),
        };
        assert!(size, "{name}: branded page dimensions missing");
        if table == "proven" {
            assert!(xml.contains("TableStyleMedium2"));
        }
        if chart == "proven" {
            assert!(xml.contains(r#"<a:srgbClr val="316F8A""#));
        }
        observed.insert(
            family,
            vec![
                "proven", "proven", "proven", chart, "proven", "proven", table,
            ],
        );
        if std::env::var("OOXML_BRAND_RENDER").as_deref() == Ok("1") {
            let render_dir = temp.join(format!("render-{name}"));
            let report = run_ok(&[
                "--json",
                "render",
                path(&output),
                "--out",
                path(&render_dir),
                "--dpi",
                "48",
            ]);
            assert_eq!(report["status"], "ok", "{report}");
            let collection = if family == "pptx" { "slides" } else { "pages" };
            let pages = report[collection].as_array().unwrap();
            assert!(!pages.is_empty());
            let branded_pixels: usize = pages
                .iter()
                .map(|page| {
                    let image = image::open(page["imagePath"].as_str().unwrap())
                        .unwrap()
                        .to_rgb8();
                    image
                        .pixels()
                        .filter(|pixel| {
                            pixel
                                .0
                                .iter()
                                .zip([49u8, 111, 138])
                                .all(|(actual, expected)| actual.abs_diff(expected) <= 15)
                        })
                        .count()
                })
                .sum();
            assert!(
                branded_pixels > 20,
                "{name}: rendered output must show the brand accent, got {branded_pixels} pixels"
            );
        }
    }
    let doc = fs::read_to_string(repo_path("docs/brand-parity.md")).unwrap();
    let labels = [
        "Theme colors",
        "Heading and body fonts",
        "Logo placement",
        "Chart palette",
        "Header/footer marks",
        "Page/slide size defaults",
        "Table styles",
    ];
    for (index, label) in labels.iter().enumerate() {
        let row = format!(
            "| {label} | {} | {} | {} |",
            observed["pptx"][index], observed["docx"][index], observed["xlsx"][index]
        );
        assert!(
            doc.lines().any(|line| line == row),
            "audit row differs from package evidence: {row}"
        );
    }
    fs::remove_dir_all(temp).unwrap();
}

fn scaffold_family_set(root: &Path, brand: &Path) -> Vec<(&'static str, PathBuf)> {
    fs::create_dir_all(root).unwrap();
    let paths = [
        ("pptx", root.join("deck.pptx")),
        ("docx", root.join("report.docx")),
        ("xlsx", root.join("workbook.xlsx")),
    ];
    for (family, output) in &paths {
        run_ok(&[
            "--json",
            family,
            "scaffold",
            path(output),
            "--brand",
            path(brand),
        ]);
    }
    paths.into_iter().collect()
}

fn theme_signature(file: &Path, family: &str) -> ThemeSignature {
    let part = match family {
        "pptx" => "ppt/theme/theme1.xml",
        "docx" => "word/theme/theme1.xml",
        "xlsx" => "xl/theme/theme1.xml",
        _ => panic!("unknown family {family}"),
    };
    let xml = zip_text(file, part);
    let mut reader = Reader::from_str(&xml);
    let mut colors = BTreeMap::new();
    let mut current_color = String::new();
    let mut current_font = String::new();
    let mut heading_font = String::new();
    let mut body_font = String::new();
    loop {
        match reader.read_event().unwrap() {
            Event::Start(element) => {
                let qualified_name = element.name();
                let name = local_name(qualified_name.as_ref());
                if matches!(
                    name,
                    "dk1"
                        | "lt1"
                        | "dk2"
                        | "lt2"
                        | "accent1"
                        | "accent2"
                        | "accent3"
                        | "accent4"
                        | "accent5"
                        | "accent6"
                        | "hlink"
                        | "folHlink"
                ) {
                    current_color = name.to_string();
                } else if matches!(name, "majorFont" | "minorFont") {
                    current_font = name.to_string();
                }
            }
            Event::Empty(element) => {
                let qualified_name = element.name();
                let name = local_name(qualified_name.as_ref());
                if name == "srgbClr" && !current_color.is_empty() {
                    colors.insert(current_color.clone(), attribute(&element, "val").unwrap());
                } else if name == "sysClr" && !current_color.is_empty() {
                    colors.insert(
                        current_color.clone(),
                        attribute(&element, "lastClr").unwrap(),
                    );
                } else if name == "latin" {
                    match current_font.as_str() {
                        "majorFont" => heading_font = attribute(&element, "typeface").unwrap(),
                        "minorFont" => body_font = attribute(&element, "typeface").unwrap(),
                        _ => {}
                    }
                }
            }
            Event::End(element) => {
                let qualified_name = element.name();
                let name = local_name(qualified_name.as_ref());
                if name == current_color {
                    current_color.clear();
                }
                if name == current_font {
                    current_font.clear();
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ThemeSignature {
        colors,
        heading_font,
        body_font,
    }
}

fn attribute(element: &quick_xml::events::BytesStart<'_>, wanted: &str) -> Option<String> {
    element.attributes().flatten().find_map(|attribute| {
        (local_name(attribute.key.as_ref()) == wanted)
            .then(|| String::from_utf8_lossy(attribute.value.as_ref()).to_string())
    })
}

fn local_name(name: &[u8]) -> &str {
    let name = std::str::from_utf8(name).unwrap();
    name.rsplit_once(':').map_or(name, |(_, local)| local)
}

fn assert_strict_valid(file: &Path) {
    let report = run_ok(&["--json", "validate", "--strict", path(file)]);
    assert_eq!(report["status"], "valid", "{report}");
}

fn assert_sdk_valid_if_available(file: &Path) {
    let validator = repo_path("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !validator.is_file() {
        eprintln!(
            "SKIP Open XML SDK: validator is unavailable at {}",
            validator.display()
        );
        return;
    }
    let Some(dotnet_root) = std::env::var_os("DOTNET_ROOT") else {
        eprintln!("SKIP Open XML SDK: DOTNET_ROOT is unavailable");
        return;
    };
    let dotnet = PathBuf::from(dotnet_root).join(if cfg!(windows) {
        "dotnet.exe"
    } else {
        "dotnet"
    });
    if !dotnet.is_file() {
        eprintln!(
            "SKIP Open XML SDK: dotnet host is unavailable at {}",
            dotnet.display()
        );
        return;
    }
    let output = Command::new(dotnet)
        .arg(validator)
        .arg(file)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "SDK validation failed for {}\nstdout:\n{}\nstderr:\n{}",
        file.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn zip_text(file: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(file).unwrap()).unwrap();
    let mut entry = archive
        .by_name(part)
        .unwrap_or_else(|_| panic!("missing {part}"));
    let mut text = String::new();
    entry.read_to_string(&mut text).unwrap();
    text
}

fn zip_entries(file: &Path) -> Vec<String> {
    let archive = ZipArchive::new(File::open(file).unwrap()).unwrap();
    archive.file_names().map(str::to_string).collect()
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "command failed: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "unexpected stderr");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .unwrap()
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-brand-{label}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}
