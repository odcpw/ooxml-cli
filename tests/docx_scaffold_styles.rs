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

const STYLE_LIST_GOLDEN: [(&str, &str, &str, Option<&str>); 16] = [
    (
        "DefaultParagraphFont",
        "Default Paragraph Font",
        "character",
        None,
    ),
    ("TableNormal", "Normal Table", "table", None),
    ("Normal", "Normal", "paragraph", None),
    ("Title", "Title", "paragraph", Some("Normal")),
    ("Subtitle", "Subtitle", "paragraph", Some("Normal")),
    ("Heading1", "Heading 1", "paragraph", Some("Normal")),
    ("Heading2", "Heading 2", "paragraph", Some("Normal")),
    ("Heading3", "Heading 3", "paragraph", Some("Normal")),
    ("Heading4", "Heading 4", "paragraph", Some("Normal")),
    ("ListBullet", "List Bullet", "paragraph", Some("Normal")),
    ("ListNumber", "List Number", "paragraph", Some("Normal")),
    ("Quote", "Quote", "paragraph", Some("Normal")),
    ("Caption", "Caption", "paragraph", Some("Normal")),
    (
        "Hyperlink",
        "Hyperlink",
        "character",
        Some("DefaultParagraphFont"),
    ),
    ("TableGrid", "Table Grid", "table", Some("TableNormal")),
    ("TableLight", "Table Light", "table", Some("TableNormal")),
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
fn every_builtin_theme_is_proven_and_styles_list_matches_reviewable_golden() {
    let temp = temp_dir("themes-and-style-golden");
    for (theme, seed) in [
        ("neutral", "5B6573"),
        ("corporate-blue", "4472C4"),
        ("warm", "C55A11"),
        ("dark", "4F46E5"),
    ] {
        let output = temp.join(format!("{theme}.docx"));
        let report = run_ooxml_ok(&[
            "--json",
            "docx",
            "scaffold",
            path_str(&output),
            "--theme",
            theme,
            "--text",
            theme,
        ]);
        assert_eq!(report["theme"], theme);
        assert_eq!(report["themeSeed"], seed);
        assert!(
            zip_text(&output, "word/theme/theme1.xml")
                .contains(&format!(r#"name="ooxml-cli {theme}""#))
        );
        assert_all_docx_proofs(&output);
    }

    let styled = temp.join("corporate-blue.docx");
    let list = run_ooxml_ok(&["--json", "docx", "styles", "list", path_str(&styled)]);
    let actual = list["styles"]
        .as_array()
        .expect("styles list array")
        .iter()
        .map(|style| {
            (
                style["styleId"].as_str().unwrap_or_default(),
                style["name"].as_str().unwrap_or_default(),
                style["type"].as_str().unwrap_or_default(),
                style.get("basedOn").and_then(Value::as_str),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        STYLE_LIST_GOLDEN,
        "styles list drifted:\n{}",
        serde_json::to_string_pretty(&list).unwrap()
    );

    fs::remove_dir_all(temp).expect("remove themes/style golden temp dir");
}

#[test]
fn every_public_builtin_style_round_trips_through_its_cli_mutation_path() {
    let temp = temp_dir("every-public-style");
    let source = temp.join("source.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Style target",
    ]);
    assert_all_docx_proofs(&source);

    let paragraph_styles = [
        "Normal",
        "Title",
        "Subtitle",
        "Heading1",
        "Heading2",
        "Heading3",
        "Heading4",
        "ListBullet",
        "ListNumber",
        "Quote",
        "Caption",
    ];
    let mut current = source;
    for (index, style) in paragraph_styles.iter().enumerate() {
        let output = temp.join(format!("append-{index:02}-{style}.docx"));
        let report = run_ooxml_ok(&[
            "--json",
            "docx",
            "paragraphs",
            "append",
            path_str(&current),
            "--text",
            &format!("Applied {style}"),
            "--style",
            style,
            "--out",
            path_str(&output),
        ]);
        assert_eq!(report["style"], *style);
        assert_all_docx_proofs(&output);
        current = output;
    }

    for (index, style) in paragraph_styles.iter().enumerate() {
        let output = temp.join(format!("apply-{index:02}-{style}.docx"));
        let report = run_ooxml_ok(&[
            "--json",
            "docx",
            "styles",
            "apply",
            path_str(&current),
            "--index",
            "1",
            "--target",
            "paragraph",
            "--style",
            style,
            "--out",
            path_str(&output),
        ]);
        assert_eq!(report["style"], *style);
        assert_all_docx_proofs(&output);
        current = output;
    }

    let hyperlink = temp.join("apply-hyperlink.docx");
    let hyperlink_report = run_ooxml_ok(&[
        "--json",
        "docx",
        "styles",
        "apply",
        path_str(&current),
        "--index",
        "1",
        "--target",
        "run",
        "--style",
        "Hyperlink",
        "--out",
        path_str(&hyperlink),
    ]);
    assert_eq!(hyperlink_report["style"], "Hyperlink");
    assert_all_docx_proofs(&hyperlink);

    let table = temp.join("table.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "tables",
        "create",
        path_str(&hyperlink),
        "--values",
        r#"[["Heading","Value"],["One",1]]"#,
        "--out",
        path_str(&table),
    ]);
    assert_all_docx_proofs(&table);
    current = table;
    for style in ["TableGrid", "TableLight"] {
        let output = temp.join(format!("apply-{style}.docx"));
        let report = run_ooxml_ok(&[
            "--json",
            "docx",
            "styles",
            "apply",
            path_str(&current),
            "--index",
            "1",
            "--target",
            "table",
            "--style",
            style,
            "--out",
            path_str(&output),
        ]);
        assert_eq!(report["style"], style);
        assert_all_docx_proofs(&output);
        current = output;
    }

    let final_styles = run_ooxml_ok(&["--json", "docx", "styles", "list", path_str(&current)]);
    println!(
        "final exhaustive style list:\n{}",
        serde_json::to_string_pretty(&final_styles).unwrap()
    );
    assert_eq!(final_styles["count"], 16);

    fs::remove_dir_all(temp).expect("remove exhaustive style temp dir");
}

#[test]
fn dangling_numbering_fixture_fails_and_three_level_lists_round_trip_through_text() {
    let temp = temp_dir("numbering-fixtures");
    let source = temp.join("source.docx");
    let invalid = temp.join("dangling-numbering.docx");
    let lists = temp.join("three-level-lists.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Numbering fixture seed",
    ]);
    rewrite_zip_part(
        &source,
        &invalid,
        "word/document.xml",
        &fs::read(repo_path(
            "testdata/docx/scaffold-styles/dangling-numbering-document.xml",
        ))
        .unwrap(),
    );
    let (strict_output, strict) =
        run_ooxml(&["--json", "validate", "--strict", path_str(&invalid)], &[]);
    assert_eq!(strict_output.status.code(), Some(5), "strict: {strict}");
    let findings = diagnostics_with_code(&strict, "DOCX_DANGLING_NUMBERING");
    assert_eq!(findings.len(), 1, "strict diagnostics: {strict}");
    assert_eq!(findings[0]["numId"], 77);
    let (check_output, check) =
        run_ooxml(&["--json", "conformance", "check", path_str(&invalid)], &[]);
    assert_eq!(check_output.status.code(), Some(5), "check: {check}");
    assert_eq!(
        diagnostics_with_code(&check, "DOCX_DANGLING_NUMBERING").len(),
        1,
        "conformance diagnostics: {check}"
    );

    rewrite_zip_part(
        &source,
        &lists,
        "word/document.xml",
        &fs::read(repo_path(
            "testdata/docx/scaffold-styles/three-level-lists-document.xml",
        ))
        .unwrap(),
    );
    assert_all_docx_proofs(&lists);
    let text = run_ooxml_ok(&["--json", "docx", "text", path_str(&lists)]);
    let actual = text["blocks"]
        .as_array()
        .expect("docx text blocks")
        .iter()
        .map(|block| {
            (
                block["styleId"].as_str().unwrap_or_default(),
                block["listLevel"].as_u64().unwrap_or(u64::MAX),
                block["numId"].as_u64().unwrap_or_default(),
                block["text"].as_str().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            ("ListBullet", 0, 1, "Bullet level 1"),
            ("ListBullet", 1, 1, "Bullet level 2"),
            ("ListBullet", 2, 1, "Bullet level 3"),
            ("ListNumber", 0, 2, "Number level 1"),
            ("ListNumber", 1, 2, "Number level 2"),
            ("ListNumber", 2, 2, "Number level 3"),
        ],
        "docx text numbering readback:\n{}",
        serde_json::to_string_pretty(&text).unwrap()
    );

    fs::remove_dir_all(temp).expect("remove numbering fixture temp dir");
}

#[test]
fn libreoffice_render_preserves_heading_reading_order_and_page_count() {
    if !Path::new("/usr/bin/soffice").is_file()
        || !Path::new("/usr/bin/pdfinfo").is_file()
        || !Path::new("/usr/bin/pdftotext").is_file()
    {
        println!("SKIP LibreOffice DOCX render: soffice, pdfinfo, or pdftotext is unavailable");
        return;
    }

    let temp = temp_dir("libreoffice-render");
    let source = temp.join("source.docx");
    let heading_one = temp.join("heading-one.docx");
    let heading_two = temp.join("heading-two.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Quarterly report title",
    ]);
    run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&source),
        "--text",
        "First styled heading",
        "--style",
        "Heading1",
        "--out",
        path_str(&heading_one),
    ]);
    run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&heading_one),
        "--text",
        "Second styled heading",
        "--style",
        "Heading2",
        "--out",
        path_str(&heading_two),
    ]);
    for output in [&source, &heading_one, &heading_two] {
        assert_all_docx_proofs(output);
    }

    let profile = temp.join("lo-profile");
    let convert = Command::new("/usr/bin/soffice")
        .arg("--headless")
        .arg(format!(
            "-env:UserInstallation=file://{}",
            profile.display()
        ))
        .args(["--convert-to", "pdf", "--outdir"])
        .arg(&temp)
        .arg(&heading_two)
        .output()
        .expect("run LibreOffice DOCX render");
    assert!(
        convert.status.success(),
        "LibreOffice render failed: stdout={} stderr={}",
        String::from_utf8_lossy(&convert.stdout),
        String::from_utf8_lossy(&convert.stderr)
    );
    let pdf = temp.join("heading-two.pdf");
    assert!(
        pdf.is_file(),
        "LibreOffice did not produce {}",
        pdf.display()
    );

    let info = Command::new("/usr/bin/pdfinfo")
        .arg(&pdf)
        .output()
        .expect("run pdfinfo");
    assert!(info.status.success(), "pdfinfo failed");
    let info = String::from_utf8_lossy(&info.stdout);
    let pages = info
        .lines()
        .find_map(|line| line.strip_prefix("Pages:").map(str::trim))
        .and_then(|value| value.parse::<usize>().ok())
        .expect("PDF page count");
    assert_eq!(pages, 1, "unexpected PDF page count:\n{info}");

    let extracted = temp.join("rendered.txt");
    let text_output = Command::new("/usr/bin/pdftotext")
        .args(["-layout"])
        .arg(&pdf)
        .arg(&extracted)
        .output()
        .expect("run pdftotext");
    assert!(
        text_output.status.success(),
        "pdftotext failed: {}",
        String::from_utf8_lossy(&text_output.stderr)
    );
    let extracted = fs::read_to_string(&extracted).expect("read extracted PDF text");
    let title = extracted
        .find("Quarterly report title")
        .expect("rendered title");
    let first = extracted
        .find("First styled heading")
        .expect("rendered first heading");
    let second = extracted
        .find("Second styled heading")
        .expect("rendered second heading");
    assert!(
        title < first && first < second,
        "headings are not in reading order:\n{extracted}"
    );

    fs::remove_dir_all(temp).expect("remove LibreOffice render temp dir");
}

#[test]
fn docx_hash_guards_support_a_five_step_chain_without_intermediate_reads() {
    let temp = temp_dir("five-step-guarded-chain");
    let source = temp.join("source.docx");
    let scaffold = run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Anchor",
    ]);
    let mut document_hash = json_sha256(&scaffold, "documentHash").to_string();
    assert_block_hashes(&scaffold, 1);
    assert_all_docx_proofs(&source);

    let first = temp.join("step-1.docx");
    let step_one = run_ooxml_ok(&[
        "--json",
        "docx",
        "blocks",
        "insert-after",
        path_str(&source),
        "--block",
        "1",
        "--text",
        "One",
        "--expect-document-hash",
        &document_hash,
        "--require-guard",
        "--out",
        path_str(&first),
    ]);
    assert!(step_one.get("warnings").is_none(), "{step_one}");
    document_hash = json_sha256(&step_one, "documentHash").to_string();
    let first_anchor_hash = block_hash(&step_one, 2).to_string();
    assert_all_docx_proofs(&first);

    let second = temp.join("step-2.docx");
    let step_two = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "insert",
        path_str(&first),
        "--after",
        "2",
        "--text",
        "Two",
        "--expect-hash",
        &first_anchor_hash,
        "--expect-document-hash",
        &document_hash,
        "--require-guard",
        "--out",
        path_str(&second),
    ]);
    document_hash = json_sha256(&step_two, "documentHash").to_string();
    let second_inserted_hash = block_hash(&step_two, 3).to_string();
    assert_all_docx_proofs(&second);

    let third = temp.join("step-3.docx");
    let step_three = run_ooxml_ok(&[
        "--json",
        "docx",
        "blocks",
        "replace",
        path_str(&second),
        "--block",
        "3",
        "--text",
        "Three",
        "--expect-hash",
        &second_inserted_hash,
        "--expect-document-hash",
        &document_hash,
        "--require-guard",
        "--out",
        path_str(&third),
    ]);
    document_hash = json_sha256(&step_three, "documentHash").to_string();
    assert_all_docx_proofs(&third);

    let fourth = temp.join("step-4.docx");
    let step_four = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "insert",
        path_str(&third),
        "--after",
        "3",
        "--text",
        "Four",
        "--expect-document-hash",
        &document_hash,
        "--require-guard",
        "--out",
        path_str(&fourth),
    ]);
    document_hash = json_sha256(&step_four, "documentHash").to_string();
    let delete_hash = block_hash(&step_four, 2).to_string();
    assert_all_docx_proofs(&fourth);

    let fifth = temp.join("step-5.docx");
    let step_five = run_ooxml_ok(&[
        "--json",
        "docx",
        "blocks",
        "delete",
        path_str(&fourth),
        "--block",
        "2",
        "--expect-hash",
        &delete_hash,
        "--expect-document-hash",
        &document_hash,
        "--require-guard",
        "--out",
        path_str(&fifth),
    ]);
    assert_block_hashes(&step_five, 3);
    assert_all_docx_proofs(&fifth);
    let text = run_ooxml_ok(&["--json", "docx", "text", path_str(&fifth)]);
    let texts = text["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["text"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(texts, ["Anchor", "Three", "Four"]);

    fs::remove_dir_all(temp).expect("remove five-step guarded chain temp dir");
}

#[test]
fn docx_guards_are_optional_warn_when_missing_and_reject_stale_documents_before_write() {
    let temp = temp_dir("guard-semantics");
    let source = temp.join("source.docx");
    let unguarded = temp.join("unguarded.docx");
    let rejected = temp.join("rejected.docx");
    let stale = temp.join("stale.docx");
    let scaffold = run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Guard seed",
    ]);
    let original_hash = json_sha256(&scaffold, "documentHash").to_string();

    let warning = run_ooxml_ok(&[
        "--json",
        "docx",
        "blocks",
        "insert-after",
        path_str(&source),
        "--block",
        "1",
        "--text",
        "Allowed with warning",
        "--out",
        path_str(&unguarded),
    ]);
    assert_eq!(warning["warnings"][0]["code"], "DOCX_GUARD_NOT_PROVIDED");
    assert_all_docx_proofs(&unguarded);

    let (required_output, required_error) = run_ooxml(
        &[
            "--json",
            "docx",
            "paragraphs",
            "insert",
            path_str(&source),
            "--after",
            "1",
            "--text",
            "Must not write",
            "--require-guard",
            "--out",
            path_str(&rejected),
        ],
        &[],
    );
    assert_eq!(required_output.status.code(), Some(2), "{required_error}");
    assert!(
        required_error["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("--require-guard requires"),
        "{required_error}"
    );
    assert!(!rejected.exists(), "missing guard published output");

    let (stale_output, stale_error) = run_ooxml(
        &[
            "--json",
            "docx",
            "blocks",
            "insert-after",
            path_str(&unguarded),
            "--block",
            "1",
            "--text",
            "Must not write stale",
            "--expect-document-hash",
            &original_hash,
            "--out",
            path_str(&stale),
        ],
        &[],
    );
    assert_eq!(stale_output.status.code(), Some(2), "{stale_error}");
    assert!(
        stale_error["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("document hash mismatch"),
        "{stale_error}"
    );
    assert!(!stale.exists(), "stale guard published output");

    fs::remove_dir_all(temp).expect("remove guard semantics temp dir");
}

#[test]
fn every_docx_read_surface_returns_document_and_block_hashes() {
    let temp = temp_dir("all-read-hashes");
    let source = temp.join("source.docx");
    run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Read hash seed",
    ]);
    let file = path_str(&source);
    let commands = [
        vec!["--json", "docx", "text", file],
        vec!["--json", "docx", "blocks", file],
        vec!["--json", "docx", "styles", "list", file],
        vec![
            "--json", "docx", "styles", "show", file, "--style", "Normal",
        ],
        vec!["--json", "docx", "fields", "list", file],
        vec!["--json", "docx", "headers", "list", file],
        vec!["--json", "docx", "footers", "list", file],
        vec!["--json", "docx", "images", "list", file],
        vec!["--json", "docx", "comments", "list", file],
        vec!["--json", "docx", "tables", "show", file],
    ];
    let mut expected_hash = None::<String>;
    for args in commands {
        let report = run_ooxml_ok(&args);
        let document_hash = json_sha256(&report, "documentHash");
        assert_block_hashes(&report, 1);
        match expected_hash.as_deref() {
            Some(expected) => assert_eq!(document_hash, expected, "read command {args:?}"),
            None => expected_hash = Some(document_hash.to_string()),
        }
    }

    fs::remove_dir_all(temp).expect("remove all read hashes temp dir");
}

#[test]
fn hash_bearing_docx_mutation_outputs_remain_byte_deterministic() {
    let temp = temp_dir("hash-mutation-determinism");
    let source = temp.join("source.docx");
    let first = temp.join("first.docx");
    let second = temp.join("second.docx");
    let scaffold = run_ooxml_ok(&[
        "--json",
        "docx",
        "scaffold",
        path_str(&source),
        "--text",
        "Original",
    ]);
    let document_hash = json_sha256(&scaffold, "documentHash");
    let first_report = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "set",
        path_str(&source),
        "--index",
        "1",
        "--text",
        "Deterministic replacement",
        "--expect-document-hash",
        document_hash,
        "--out",
        path_str(&first),
    ]);
    let second_report = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "set",
        path_str(&source),
        "--index",
        "1",
        "--text",
        "Deterministic replacement",
        "--expect-document-hash",
        document_hash,
        "--out",
        path_str(&second),
    ]);
    assert_eq!(first_report["documentHash"], second_report["documentHash"]);
    assert_eq!(
        fs::read(&first).unwrap(),
        fs::read(&second).unwrap(),
        "identical guarded mutations must produce identical DOCX bytes"
    );
    assert_all_docx_proofs(&first);
    assert_all_docx_proofs(&second);

    fs::remove_dir_all(temp).expect("remove hash mutation determinism temp dir");
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
    let repeated = temp.join("created-again.docx");
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

    let repeated_report = run_ooxml_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path_str(&output),
        "--text",
        "Reused heading style",
        "--style",
        "Heading1",
        "--create-style",
        "--out",
        path_str(&repeated),
    ]);
    assert_eq!(repeated_report["createdStyle"], false);
    assert_eq!(
        zip_text(&repeated, "word/styles.xml")
            .matches(r#"w:styleId="Heading1""#)
            .count(),
        1,
        "--create-style must be idempotent"
    );
    assert_all_docx_proofs(&repeated);

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

fn assert_all_docx_proofs(package: &Path) {
    assert_strict_valid(package);
    let (output, report) = run_ooxml(&["--json", "conformance", "check", path_str(package)], &[]);
    assert!(
        output.status.success(),
        "conformance failed for {}: {report}",
        package.display()
    );
    assert_eq!(report["status"], "passed");
    assert_sdk_valid_if_available(package);
}

fn json_sha256<'a>(report: &'a Value, field: &str) -> &'a str {
    let value = report[field]
        .as_str()
        .unwrap_or_else(|| panic!("missing {field} in report: {report}"));
    assert!(
        value.starts_with("sha256:")
            && value.len() == 71
            && value[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
        "invalid {field}: {value}"
    );
    value
}

fn assert_block_hashes(report: &Value, expected_count: usize) {
    let blocks = report["blockHashes"]
        .as_array()
        .unwrap_or_else(|| panic!("missing blockHashes: {report}"));
    assert_eq!(blocks.len(), expected_count, "block hashes: {report}");
    for (offset, block) in blocks.iter().enumerate() {
        assert_eq!(block["index"], offset + 1);
        json_sha256(block, "contentHash");
    }
}

fn block_hash(report: &Value, index: usize) -> &str {
    let block = report["blockHashes"]
        .as_array()
        .unwrap_or_else(|| panic!("missing blockHashes: {report}"))
        .iter()
        .find(|block| block["index"] == index)
        .unwrap_or_else(|| panic!("missing block {index} hash: {report}"));
    json_sha256(block, "contentHash")
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
