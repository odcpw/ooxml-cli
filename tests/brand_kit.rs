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
            "word/document.xml",
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
