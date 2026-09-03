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

#[test]
fn committed_dangling_style_fixture_fails_strict_and_check() {
    let fixture = repo_path("testdata/docx/scaffold-styles/dangling-style.docx");
    let (output, report) = run_ooxml(&["--json", "validate", "--strict", path_str(&fixture)], &[]);
    assert_eq!(output.status.code(), Some(5), "strict report: {report}");
    let diagnostic = diagnostics_with_code(&report, "DOCX_DANGLING_STYLE")
        .into_iter()
        .next()
        .expect("dangling style diagnostic");
    assert_eq!(diagnostic["part"], "/word/document.xml");
    assert_eq!(diagnostic["element"], "pStyle");
    assert_eq!(diagnostic["styleId"], "Heading1");
    assert_eq!(diagnostic["check"], "style-integrity");

    let (check_output, check) =
        run_ooxml(&["--json", "conformance", "check", path_str(&fixture)], &[]);
    assert_eq!(
        check_output.status.code(),
        Some(5),
        "conformance report: {check}"
    );
    assert!(
        !diagnostics_with_code(&check, "DOCX_DANGLING_STYLE").is_empty(),
        "conformance check must include style integrity: {check}"
    );
}

#[test]
fn strict_style_integrity_covers_paragraph_run_table_and_numbering_references() {
    let temp = temp_dir("all-dangling-reference-kinds");
    let source = temp.join("source.docx");
    let invalid = temp.join("invalid.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Reference seed",
    ]);
    make_dangling_reference_package(&source, &invalid);

    let (output, report) = run_ooxml(&["--json", "validate", "--strict", path_str(&invalid)], &[]);
    assert_eq!(output.status.code(), Some(5), "strict report: {report}");
    let style_diagnostics = diagnostics_with_code(&report, "DOCX_DANGLING_STYLE");
    let elements = style_diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic["element"].as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(elements, BTreeSet::from(["pStyle", "rStyle", "tblStyle"]));
    let numbering = diagnostics_with_code(&report, "DOCX_DANGLING_NUMBERING");
    assert_eq!(numbering.len(), 1, "numbering diagnostics: {numbering:?}");
    assert_eq!(numbering[0]["numId"], 77);
    assert_eq!(numbering[0]["part"], "/word/document.xml");

    fs::remove_dir_all(temp).expect("remove dangling references temp dir");
}

#[test]
fn style_mutations_canonicalize_names_and_reject_typos_helpfully() {
    let temp = temp_dir("style-resolution");
    let source = temp.join("source.docx");
    let named = temp.join("named.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Style seed",
    ]);
    let append = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&source),
        "--text",
        "Canonical heading",
        "--style",
        "hEaDiNg 1",
        "--out",
        path_str(&named),
    ]);
    assert_eq!(append["style"], "Heading1");
    assert!(append.get("createdStyle").is_none());
    assert!(zip_text(&named, "word/document.xml").contains(r#"w:val="Heading1""#));
    assert_strict_valid(&named);
    assert_sdk_valid_if_available(&named);

    let blocks = run_ooxml_ok(&["--json", "docx", "blocks", path_str(&source)]);
    let hash = blocks["blocks"][0]["contentHash"]
        .as_str()
        .expect("block content hash");
    let append_out = temp.join("append.docx");
    let insert_out = temp.join("insert.docx");
    let replace_out = temp.join("replace.docx");
    let block_insert_out = temp.join("block-insert.docx");
    let apply_out = temp.join("apply.docx");
    let commands = [
        vec![
            "--json",
            "docx",
            "paragraphs",
            "append",
            path_str(&source),
            "--text",
            "Rejected",
            "--style",
            "Heding1",
            "--no-validate",
            "--out",
            path_str(&append_out),
        ],
        vec![
            "--json",
            "docx",
            "paragraphs",
            "insert",
            path_str(&source),
            "--insert-after",
            "0",
            "--text",
            "Rejected",
            "--style",
            "Heding1",
            "--out",
            path_str(&insert_out),
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "replace",
            path_str(&source),
            "--block",
            "1",
            "--expect-hash",
            hash,
            "--text",
            "Rejected",
            "--style",
            "Heding1",
            "--out",
            path_str(&replace_out),
        ],
        vec![
            "--json",
            "docx",
            "blocks",
            "insert-after",
            path_str(&source),
            "--block",
            "1",
            "--expect-hash",
            hash,
            "--text",
            "Rejected",
            "--style",
            "Heding1",
            "--out",
            path_str(&block_insert_out),
        ],
        vec![
            "--json",
            "docx",
            "styles",
            "apply",
            path_str(&source),
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            "Heding1",
            "--out",
            path_str(&apply_out),
        ],
    ];
    for args in commands {
        let (output, error) = run_ooxml(&args, &[]);
        assert!(
            !output.status.success(),
            "typo unexpectedly succeeded: {args:?}"
        );
        let message = error["error"]["message"].as_str().unwrap_or_default();
        assert!(
            message.contains("available paragraph styles"),
            "missing available list for {args:?}: {error}"
        );
        assert!(
            message.contains("nearest match: Heading1 (Heading 1)"),
            "missing nearest match for {args:?}: {error}"
        );
    }

    fs::remove_dir_all(temp).expect("remove style resolution temp dir");
}

#[test]
fn create_style_repairs_the_original_minimal_case_atomically() {
    let temp = temp_dir("create-style");
    let source = repo_path("testdata/docx/scaffold-styles/dangling-style.docx");
    let output = temp.join("created.docx");
    let rejected_output = temp.join("rejected.docx");
    let (rejected, error) = run_ooxml(
        &[
            "--json",
            "docx",
            "paragraphs",
            "append",
            path_str(&source),
            "--text",
            "Would silently render as Normal",
            "--style",
            "Heading1",
            "--out",
            path_str(&rejected_output),
        ],
        &[],
    );
    assert!(
        !rejected.status.success(),
        "missing style unexpectedly succeeded"
    );
    assert!(
        !rejected_output.exists(),
        "failed mutation published an output"
    );
    let message = error["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains(r#"style not found: "Heading1""#),
        "{error}"
    );
    assert!(
        message.contains("no paragraph styles are defined"),
        "{error}"
    );

    let report = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&source),
        "--text",
        "Created heading style",
        "--style",
        "Heading1",
        "--create-style",
        "--out",
        path_str(&output),
    ]);
    assert_eq!(report["style"], "Heading1");
    assert_eq!(report["createdStyle"], true);
    assert!(zip_entries(&output).contains("word/styles.xml"));
    assert!(zip_entries(&output).contains("word/numbering.xml"));
    let styles = run_ooxml_ok(&[
        "--json",
        "docx",
        "styles",
        "show",
        path_str(&output),
        "--style",
        "Heading1",
    ]);
    assert_eq!(styles["found"], true);
    assert_eq!(styles["style"]["name"], "Heading 1");
    let document = zip_text(&output, "word/document.xml");
    assert_eq!(document.matches(r#"w:val="Heading1""#).count(), 2);
    assert_strict_valid(&output);
    assert_sdk_valid_if_available(&output);

    fs::remove_dir_all(temp).expect("remove create-style temp dir");
}

#[test]
fn create_list_style_adds_numbering_to_an_existing_styles_package() {
    let temp = temp_dir("create-list-style");
    let old_minimal = repo_path("testdata/docx/scaffold-styles/dangling-style.docx");
    let partial = temp.join("partial.docx");
    let output = temp.join("numbered.docx");
    make_partial_style_package(&old_minimal, &partial);
    let report = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&partial),
        "--text",
        "Numbered item",
        "--style",
        "list number",
        "--create-style",
        "--out",
        path_str(&output),
    ]);
    assert_eq!(report["style"], "ListNumber");
    assert_eq!(report["createdStyle"], true);
    let styles = zip_text(&output, "word/styles.xml");
    assert!(styles.contains(r#"w:styleId="ListNumber""#));
    assert!(styles.contains(r#"<w:numId w:val="2"/>"#));
    let numbering = zip_text(&output, "word/numbering.xml");
    assert!(numbering.contains(r#"<w:num w:numId="2">"#));
    let rels = zip_text(&output, "word/_rels/document.xml.rels");
    assert!(rels.contains("/relationships/numbering"));
    assert_strict_valid(&output);
    assert_sdk_valid_if_available(&output);
    fs::remove_dir_all(temp).expect("remove create-list-style temp dir");
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

fn make_dangling_reference_package(input: &Path, output: &Path) {
    let document = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:pStyle w:val="MissingParagraph"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="77"/></w:numPr></w:pPr><w:r><w:rPr><w:rStyle w:val="MissingCharacter"/></w:rPr><w:t>Dangling paragraph and run</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblStyle w:val="MissingTable"/><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tblGrid><w:gridCol w:w="2400"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="2400" w:type="dxa"/></w:tcPr><w:p/></w:tc></w:tr></w:tbl><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#;
    rewrite_zip_part(input, output, "word/document.xml", document.as_bytes());
}

fn make_partial_style_package(input: &Path, output: &Path) {
    let document = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Minimal</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>"#;
    let styles = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style></w:styles>"#;
    let document_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

    let mut archive =
        ZipArchive::new(File::open(input).expect("open old minimal package")).unwrap();
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
    let content_types = String::from_utf8(parts.remove("[Content_Types].xml").unwrap()).unwrap();
    parts.insert(
        "[Content_Types].xml".to_string(),
        content_types
            .replace(
                "</Types>",
                r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#,
            )
            .into_bytes(),
    );
    parts.insert(
        "word/document.xml".to_string(),
        document.as_bytes().to_vec(),
    );
    parts.insert("word/styles.xml".to_string(), styles.as_bytes().to_vec());
    parts.insert(
        "word/_rels/document.xml.rels".to_string(),
        document_rels.as_bytes().to_vec(),
    );
    let file = File::create(output).expect("create partial style package");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn rewrite_zip_part(input: &Path, output: &Path, part: &str, replacement: &[u8]) {
    let mut archive = ZipArchive::new(File::open(input).expect("open package to rewrite")).unwrap();
    let mut parts = BTreeMap::<String, Vec<u8>>::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        if entry.is_dir() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        let bytes = if entry.name() == part {
            replacement.to_vec()
        } else {
            bytes
        };
        parts.insert(entry.name().to_string(), bytes);
    }
    let file = File::create(output).expect("create rewritten package");
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer.start_file(name, options).unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
}

fn diagnostics_with_code<'a>(value: &'a Value, code: &str) -> Vec<&'a Value> {
    let mut matches = Vec::new();
    collect_diagnostics_with_code(value, code, &mut matches);
    matches
}

fn collect_diagnostics_with_code<'a>(value: &'a Value, code: &str, matches: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            if object.get("code").and_then(Value::as_str) == Some(code) {
                matches.push(value);
            }
            for child in object.values() {
                collect_diagnostics_with_code(child, code, matches);
            }
        }
        Value::Array(array) => {
            for child in array {
                collect_diagnostics_with_code(child, code, matches);
            }
        }
        _ => {}
    }
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
