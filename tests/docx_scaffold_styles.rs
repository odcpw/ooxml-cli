use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const REQUIRED_STYLES: [(&str, &str); 14] = [
    ("Normal", "Normal"),
    ("Title", "Title"),
    ("Subtitle", "Subtitle"),
    ("Heading1", "Heading 1"),
    ("Heading2", "Heading 2"),
    ("Heading3", "Heading 3"),
    ("Heading4", "Heading 4"),
    ("ListBullet", "List Bullet"),
    ("ListNumber", "List Number"),
    ("Quote", "Quote"),
    ("Caption", "Caption"),
    ("Hyperlink", "Hyperlink"),
    ("TableGrid", "Table Grid"),
    ("TableLight", "Table Light"),
];

#[test]
fn scaffold_has_complete_deterministic_style_aware_package() {
    let temp = temp_dir("complete");
    let first = temp.join("first.docx");
    let second = temp.join("second.docx");
    for output in [&first, &second] {
        let report = run_ooxml_ok(&[
            "--json",
            "docx",
            "scaffold",
            "--out",
            path_str(output),
            "--text",
            "Deterministic styled document",
        ]);
        assert_eq!(report["builtInStyleCount"], 14);
        assert_eq!(report["theme"], "corporate-blue");
        assert_eq!(report["themeSeed"], "4472C4");
        assert_strict_valid(output);
        assert_sdk_valid_if_available(output);
    }
    assert_eq!(
        fs::read(&first).expect("read first scaffold"),
        fs::read(&second).expect("read second scaffold"),
        "identical scaffold inputs must produce identical package bytes"
    );

    let entries = zip_entries(&first);
    for expected in [
        "docProps/core.xml",
        "docProps/app.xml",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/styles.xml",
        "word/numbering.xml",
        "word/settings.xml",
        "word/fontTable.xml",
        "word/theme/theme1.xml",
    ] {
        assert!(
            entries.contains(expected),
            "missing scaffold part {expected}"
        );
    }
    let core = zip_text(&first, "docProps/core.xml");
    assert!(
        !core.contains("dcterms:created"),
        "unexpected created timestamp"
    );
    assert!(
        !core.contains("dcterms:modified"),
        "unexpected modified timestamp"
    );
    let settings = zip_text(&first, "word/settings.xml");
    assert!(settings.contains(r#"<w:updateFields w:val="false"/>"#));
    let fonts = zip_text(&first, "word/fontTable.xml");
    for font in ["Aptos", "Calibri", "Arial", "Liberation Sans"] {
        assert!(
            fonts.contains(&format!(r#"w:name="{font}""#)),
            "missing font {font}"
        );
    }
    let numbering = zip_text(&first, "word/numbering.xml");
    assert_eq!(numbering.matches("<w:abstractNum ").count(), 2);
    assert_eq!(numbering.matches("<w:lvl w:ilvl=").count(), 6);
    assert!(numbering.contains(r#"<w:num w:numId="1">"#));
    assert!(numbering.contains(r#"<w:num w:numId="2">"#));

    let styles = run_ooxml_ok(&["--json", "docx", "styles", "list", path_str(&first)]);
    let listed = styles["styles"].as_array().expect("styles array");
    assert_eq!(
        listed.len(),
        16,
        "14 public styles plus two required base styles"
    );
    for (id, name) in REQUIRED_STYLES {
        let style = listed
            .iter()
            .find(|style| style["styleId"] == id)
            .unwrap_or_else(|| panic!("missing style {id}: {styles}"));
        assert_eq!(style["name"], name, "style name for {id}");
    }
    let document = zip_text(&first, "word/document.xml");
    assert!(document.contains(r#"<w:pStyle w:val="Normal"/>"#));

    fs::remove_dir_all(temp).expect("remove complete scaffold temp dir");
}

#[test]
fn heading_style_append_and_seeded_theme_are_schema_clean() {
    let temp = temp_dir("heading-theme");
    let source = temp.join("source.docx");
    let headed = temp.join("headed.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--theme-seed",
        "#7A3E9D",
        "--text",
        "Body",
    ]);
    let theme = zip_text(&source, "word/theme/theme1.xml");
    assert!(theme.contains(r#"name="ooxml-cli custom""#));
    assert!(theme.contains(r#"<a:accent1><a:srgbClr val="7A3E9D"/>"#));

    run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&source),
        "--text",
        "A real heading",
        "--style",
        "Heading1",
        "--out",
        path_str(&headed),
    ]);
    let document = zip_text(&headed, "word/document.xml");
    assert!(document.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert_strict_valid(&source);
    assert_strict_valid(&headed);
    assert_sdk_valid_if_available(&source);
    assert_sdk_valid_if_available(&headed);

    fs::remove_dir_all(temp).expect("remove heading/theme temp dir");
}

#[test]
fn source_date_epoch_is_the_only_timestamp_source() {
    let temp = temp_dir("source-date-epoch");
    let output = temp.join("dated.docx");
    run_ooxml_with_env_ok(
        &[
            "--json",
            "docx",
            "scaffold",
            path_str(&output),
            "--text",
            "Reproducible metadata",
        ],
        &[("SOURCE_DATE_EPOCH", "946684800")],
    );
    let core = zip_text(&output, "docProps/core.xml");
    assert!(core.contains(
        r#"<dcterms:created xsi:type="dcterms:W3CDTF">2000-01-01T00:00:00Z</dcterms:created>"#
    ));
    assert!(core.contains(
        r#"<dcterms:modified xsi:type="dcterms:W3CDTF">2000-01-01T00:00:00Z</dcterms:modified>"#
    ));
    assert_strict_valid(&output);
    assert_sdk_valid_if_available(&output);
    fs::remove_dir_all(temp).expect("remove SOURCE_DATE_EPOCH temp dir");
}

#[test]
fn template_inherits_styles_theme_and_page_setup_deterministically() {
    let temp = temp_dir("template");
    let base = temp.join("base.docx");
    let template = temp.join("template.docx");
    let first = temp.join("first.docx");
    let second = temp.join("second.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&base),
        "--theme",
        "warm",
        "--text",
        "Template seed",
    ]);
    make_distinct_template(&base, &template);

    for output in [&first, &second] {
        let report = run_ooxml_ok(&[
            "--json",
            "docx",
            "scaffold",
            path_str(output),
            "--template",
            path_str(&template),
            "--text",
            "From template",
        ]);
        assert_eq!(report["template"], path_str(&template));
        assert!(report["theme"].is_null());
        assert_strict_valid(output);
        assert_sdk_valid_if_available(output);
    }
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let styles = zip_text(&first, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="TemplateBody""#));
    let document = zip_text(&first, "word/document.xml");
    assert!(document.contains(r#"<w:pgSz w:w="11906" w:h="16838"/>"#));
    assert!(document.contains("From template"));
    assert!(!document.contains("Template seed"));
    assert_eq!(
        zip_text(&first, "word/theme/theme1.xml"),
        zip_text(&template, "word/theme/theme1.xml"),
        "template theme must be inherited byte-for-byte"
    );

    fs::remove_dir_all(temp).expect("remove template temp dir");
}

fn make_distinct_template(input: &Path, output: &Path) {
    let mut archive = ZipArchive::new(File::open(input).expect("open base template")).unwrap();
    let mut parts = BTreeMap::<String, Vec<u8>>::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if entry.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        parts.insert(entry.name().to_string(), bytes);
    }
    let styles = String::from_utf8(parts.remove("word/styles.xml").unwrap()).unwrap();
    parts.insert(
        "word/styles.xml".to_string(),
        styles
            .replace(
                "</w:styles>",
                r#"<w:style w:type="paragraph" w:customStyle="1" w:styleId="TemplateBody"><w:name w:val="Template Body"/><w:basedOn w:val="Normal"/><w:qFormat/></w:style></w:styles>"#,
            )
            .into_bytes(),
    );
    let document = String::from_utf8(parts.remove("word/document.xml").unwrap()).unwrap();
    parts.insert(
        "word/document.xml".to_string(),
        document
            .replace(
                r#"<w:pgSz w:w="12240" w:h="15840"/>"#,
                r#"<w:pgSz w:w="11906" w:h="16838"/>"#,
            )
            .into_bytes(),
    );

    let file = File::create(output).expect("create distinct template");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn run_ooxml(args: &[&str], envs: &[(&str, &str)]) -> (Output, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .envs(envs.iter().copied())
        .output()
        .expect("run ooxml");
    let stream = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let report = serde_json::from_slice(stream).unwrap_or_else(|error| {
        panic!(
            "parse ooxml JSON for {args:?}: {error}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output, report)
}

fn run_ooxml_ok(args: &[&str]) -> Value {
    run_ooxml_with_env_ok(args, &[])
}

fn run_ooxml_with_env_ok(args: &[&str], envs: &[(&str, &str)]) -> Value {
    let (output, report) = run_ooxml(args, envs);
    assert!(
        output.status.success(),
        "ooxml failed for {args:?}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    report
}

fn assert_strict_valid(package: &Path) {
    let (output, report) = run_ooxml(&["--json", "validate", "--strict", path_str(package)], &[]);
    assert!(
        output.status.success(),
        "strict validation failed for {}: {report}",
        package.display()
    );
    assert_eq!(report["status"], "valid");
}

fn assert_sdk_valid_if_available(package: &Path) {
    let Some((dotnet, validator)) = sdk_validator() else {
        println!(
            "SKIP Open XML SDK validation for {}: ~/dotnet/dotnet or validator DLL is unavailable",
            package.display()
        );
        return;
    };
    let output = Command::new(dotnet)
        .args([
            validator.as_os_str(),
            "--json".as_ref(),
            package.as_os_str(),
        ])
        .output()
        .expect("run Open XML SDK validator");
    assert!(
        output.stderr.is_empty(),
        "Open XML SDK stderr for {}: {}",
        package.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "parse Open XML SDK JSON for {}: {error}: {}",
            package.display(),
            String::from_utf8_lossy(&output.stdout)
        )
    });
    assert!(
        output.status.success(),
        "Open XML SDK rejected {}: {report}",
        package.display()
    );
    assert_eq!(
        report["Valid"],
        true,
        "SDK report for {}",
        package.display()
    );
    assert_eq!(
        report["ErrorCount"],
        0,
        "SDK report for {}",
        package.display()
    );
}

fn sdk_validator() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dotnet = home.join("dotnet/dotnet");
    let validator = repo_path("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    (dotnet.is_file() && validator.is_file()).then_some((dotnet, validator))
}

fn zip_entries(package: &Path) -> BTreeSet<String> {
    let mut archive = ZipArchive::new(File::open(package).expect("open package")).unwrap();
    (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().to_string())
        .collect()
}

fn zip_text(package: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(package).expect("open package")).unwrap();
    let mut entry = archive.by_name(part).unwrap();
    let mut text = String::new();
    entry.read_to_string(&mut text).unwrap();
    text
}

fn repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-docx-scaffold-styles-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create test temp dir");
    path
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("UTF-8 test path")
}
