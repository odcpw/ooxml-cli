use serde_json::Value;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

#[test]
fn paragraph_lists_support_three_levels_restart_insert_and_determinism() {
    let temp = temp_dir("lists");
    let first = build_list_document(&temp, "first");
    let second = build_list_document(&temp, "second");
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());

    let numbering = zip_text(&first, "word/numbering.xml");
    assert!(numbering.contains(r#"<w:num w:numId="3"><w:abstractNumId w:val="1"/>"#));
    assert!(numbering.contains(r#"<w:lvlOverride w:ilvl="0"><w:startOverride w:val="1"/>"#));
    let document = zip_text(&first, "word/document.xml");
    for (style, num_id, level) in [
        ("ListBullet", 1, 0),
        ("ListBullet", 1, 1),
        ("ListBullet", 1, 2),
        ("ListNumber", 3, 0),
    ] {
        assert!(document.contains(&format!(r#"<w:pStyle w:val="{style}"/>"#)));
        assert!(document.contains(&format!(
            r#"<w:ilvl w:val="{level}"/><w:numId w:val="{num_id}"/>"#
        )));
    }
    let blocks = run_ok(&["--json", "docx", "blocks", path(&first)]);
    let blocks = blocks["blocks"].as_array().expect("blocks array");
    for text in ["Bullet zero", "Bullet one", "Bullet two", "Restart one"] {
        let block = blocks.iter().find(|block| block["text"] == text).unwrap();
        assert!(
            block["contentHash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(block["numId"].is_number());
        assert!(block["listLevel"].is_number());
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn page_and_section_breaks_are_schema_clean_and_hash_visible() {
    let temp = temp_dir("breaks");
    let source = temp.join("source.docx");
    let page = temp.join("page.docx");
    let page_again = temp.join("page-again.docx");
    let section = temp.join("section.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "First page",
    ]);
    let page_report = run_ok(&[
        "--json",
        "docx",
        "breaks",
        "insert",
        path(&source),
        "--page",
        "--out",
        path(&page),
    ]);
    run_ok(&[
        "--json",
        "docx",
        "breaks",
        "insert",
        path(&source),
        "--page",
        "--out",
        path(&page_again),
    ]);
    assert_eq!(fs::read(&page).unwrap(), fs::read(&page_again).unwrap());
    assert_eq!(page_report["break"], "page");
    assert!(
        page_report["documentHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let section_report = run_ok(&[
        "--json",
        "docx",
        "breaks",
        "insert",
        path(&page),
        "--section",
        "--out",
        path(&section),
    ]);
    assert_eq!(section_report["break"], "section");
    assert_eq!(
        zip_text(&page, "word/document.xml")
            .matches(r#"w:type="page""#)
            .count(),
        1
    );
    assert_eq!(
        zip_text(&section, "word/document.xml")
            .matches("<w:sectPr")
            .count(),
        2
    );
    for package in [&source, &page, &page_again, &section] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn indexed_sections_set_size_orientation_and_margins_in_schema_order() {
    let temp = temp_dir("section-setup");
    let source = temp.join("source.docx");
    let split = temp.join("split.docx");
    let first = temp.join("first.docx");
    let first_again = temp.join("first-again.docx");
    let second = temp.join("second.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "Section one",
    ]);
    run_ok(&[
        "--json",
        "docx",
        "breaks",
        "insert",
        path(&source),
        "--section",
        "--out",
        path(&split),
    ]);
    let first_report = run_ok(&[
        "--json",
        "docx",
        "sections",
        "set",
        path(&split),
        "--section",
        "1",
        "--orientation",
        "landscape",
        "--size",
        "A4",
        "--margins",
        "0.5in,0.75in,1in,1.25in",
        "--out",
        path(&first),
    ]);
    run_ok(&[
        "--json",
        "docx",
        "sections",
        "set",
        path(&split),
        "--section",
        "1",
        "--orientation",
        "landscape",
        "--size",
        "A4",
        "--margins",
        "0.5in,0.75in,1in,1.25in",
        "--out",
        path(&first_again),
    ]);
    assert_eq!(fs::read(&first).unwrap(), fs::read(&first_again).unwrap());
    assert_eq!(first_report["marginsTwips"]["top"], 720);
    let second_report = run_ok(&[
        "--json",
        "docx",
        "sections",
        "set",
        path(&first),
        "--section",
        "2",
        "--orientation",
        "portrait",
        "--size",
        "Letter",
        "--margins",
        "1in,1in,1in,1in",
        "--out",
        path(&second),
    ]);
    assert_eq!(second_report["section"], 2);
    let xml = zip_text(&second, "word/document.xml");
    assert!(xml.contains(r#"<w:pgSz w:w="16838" w:h="11906" w:orient="landscape"/><w:pgMar w:top="720" w:right="1080" w:bottom="1440" w:left="1800"/>"#));
    assert!(xml.contains(r#"<w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>"#));
    for package in [&source, &split, &first, &first_again, &second] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn styled_tables_have_widths_repeating_headers_numeric_alignment_and_set_style() {
    let temp = temp_dir("styled-table");
    let source = temp.join("source.docx");
    let created = temp.join("created.docx");
    let created_again = temp.join("created-again.docx");
    let restyled = temp.join("restyled.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "Quarterly data",
    ]);
    let create = run_ok(&[
        "--json",
        "docx",
        "tables",
        "create",
        path(&source),
        "--values",
        r#"[["Product","Revenue","Trend"],["Widgets",1200,"Up"],["Gadgets",950,"Flat"]]"#,
        "--style",
        "TableGrid",
        "--header-row",
        "--widths",
        "2in,1in,auto",
        "--align",
        "center",
        "--caption",
        "Quarterly revenue",
        "--out",
        path(&created),
    ]);
    run_ok(&[
        "--json",
        "docx",
        "tables",
        "create",
        path(&source),
        "--values",
        r#"[["Product","Revenue","Trend"],["Widgets",1200,"Up"],["Gadgets",950,"Flat"]]"#,
        "--style",
        "TableGrid",
        "--header-row",
        "--widths",
        "2in,1in,auto",
        "--align",
        "center",
        "--caption",
        "Quarterly revenue",
        "--out",
        path(&created_again),
    ]);
    assert_eq!(
        fs::read(&created).unwrap(),
        fs::read(&created_again).unwrap()
    );
    let hash = create["contentHash"].as_str().unwrap();
    assert!(
        create["documentHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let created_xml = zip_text(&created, "word/document.xml");
    for wanted in [
        r#"<w:tblStyle w:val="TableGrid"/>"#,
        r#"<w:jc w:val="center"/>"#,
        r#"<w:tblCaption w:val="Quarterly revenue"/>"#,
        r#"<w:trPr><w:tblHeader/></w:trPr>"#,
        r#"<w:gridCol w:w="2880"/><w:gridCol w:w="1440"/>"#,
        r#"<w:pPr><w:jc w:val="right"/></w:pPr>"#,
    ] {
        assert!(created_xml.contains(wanted), "missing {wanted}");
    }
    let set = run_ok(&[
        "--json",
        "docx",
        "tables",
        "set-style",
        path(&created),
        "--table",
        "1",
        "--expect-hash",
        hash,
        "--style",
        "TableLight",
        "--out",
        path(&restyled),
    ]);
    assert_eq!(set["style"], "TableLight");
    let restyled_xml = zip_text(&restyled, "word/document.xml");
    assert!(restyled_xml.contains(r#"<w:tblStyle w:val="TableLight"/>"#));
    assert!(!restyled_xml.contains(r#"<w:tblStyle w:val="TableGrid"/>"#));
    for package in [&source, &created, &created_again, &restyled] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn image_caption_uses_caption_style_seq_field_and_alignment() {
    let temp = temp_dir("image-caption");
    let source = temp.join("source.docx");
    let output = temp.join("captioned.docx");
    let output_again = temp.join("captioned-again.docx");
    let scaffold = run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "Figure follows",
    ]);
    let anchor = scaffold["blockHashes"][0]["contentHash"].as_str().unwrap();
    let image = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/test_image.png");
    let report = run_ok(&[
        "--json",
        "docx",
        "images",
        "insert",
        path(&source),
        "--after",
        "1",
        "--expect-hash",
        anchor,
        "--file",
        path(&image),
        "--width",
        "2in",
        "--height",
        "1in",
        "--caption",
        "Quarterly trend",
        "--align",
        "center",
        "--out",
        path(&output),
    ]);
    run_ok(&[
        "--json",
        "docx",
        "images",
        "insert",
        path(&source),
        "--after",
        "1",
        "--expect-hash",
        anchor,
        "--file",
        path(&image),
        "--width",
        "2in",
        "--height",
        "1in",
        "--caption",
        "Quarterly trend",
        "--align",
        "center",
        "--out",
        path(&output_again),
    ]);
    assert_eq!(fs::read(&output).unwrap(), fs::read(&output_again).unwrap());
    assert_eq!(report["caption"], "Quarterly trend");
    assert_eq!(report["captionBlock"], 3);
    assert!(report["blockHashes"].as_array().unwrap().len() >= 3);
    let xml = zip_text(&output, "word/document.xml");
    assert!(xml.contains(r#"<w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:drawing>"#));
    assert!(xml.contains(r#"<w:pStyle w:val="Caption"/><w:jc w:val="center"/>"#));
    assert!(xml.contains(r#"w:instr=" SEQ Figure \* ARABIC ""#));
    assert!(xml.contains("Quarterly trend"));
    for package in [&source, &output, &output_again] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn toc_field_has_placeholder_update_fields_and_readback_warning() {
    let temp = temp_dir("toc");
    let source = temp.join("source.docx");
    let output = temp.join("toc.docx");
    let output_again = temp.join("toc-again.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "Quarterly report",
    ]);
    let inserted = run_ok(&[
        "--json",
        "docx",
        "fields",
        "insert",
        path(&source),
        "--toc",
        "--levels",
        "1-3",
        "--out",
        path(&output),
    ]);
    run_ok(&[
        "--json",
        "docx",
        "fields",
        "insert",
        path(&source),
        "--toc",
        "--levels",
        "1-3",
        "--out",
        path(&output_again),
    ]);
    assert_eq!(fs::read(&output).unwrap(), fs::read(&output_again).unwrap());
    assert_eq!(inserted["levels"], "1-3");
    assert_eq!(inserted["updateFields"], true);
    assert_eq!(
        inserted["warnings"][0]["code"],
        "DOCX_FIELD_UPDATE_REQUIRED"
    );
    let settings = zip_text(&output, "word/settings.xml");
    assert!(settings.contains(r#"<w:updateFields w:val="true"/>"#));
    let xml = zip_text(&output, "word/document.xml");
    assert!(xml.contains(r#"w:instr="TOC \o &quot;1-3&quot; \h \z \u""#));
    assert!(xml.contains("Table of contents — update field to refresh"));
    let fields = run_ok(&[
        "--json",
        "docx",
        "fields",
        "list",
        path(&output),
        "--type",
        "TOC",
    ]);
    assert_eq!(fields["fields"].as_array().unwrap().len(), 1);
    assert_eq!(fields["warnings"][0]["code"], "DOCX_FIELD_UPDATE_REQUIRED");
    for package in [&source, &output, &output_again] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn footer_page_numbers_use_page_and_numpages_fields() {
    let temp = temp_dir("page-numbers");
    let source = temp.join("source.docx");
    let output = temp.join("numbered.docx");
    let output_again = temp.join("numbered-again.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&source),
        "--text",
        "Quarterly report",
    ]);
    let report = run_ok(&[
        "--json",
        "docx",
        "footers",
        "set-text",
        path(&source),
        "--page-numbers",
        "--out",
        path(&output),
    ]);
    run_ok(&[
        "--json",
        "docx",
        "footers",
        "set-text",
        path(&source),
        "--page-numbers",
        "--out",
        path(&output_again),
    ]);
    assert_eq!(fs::read(&output).unwrap(), fs::read(&output_again).unwrap());
    assert_eq!(report["pageNumbers"], true);
    assert_eq!(report["text"], "Page 1 of 1");
    assert!(
        report["documentHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let footer = zip_text(&output, "word/footer1.xml");
    assert!(footer.contains(r#"w:instr=" PAGE ""#));
    assert!(footer.contains(r#"w:instr=" NUMPAGES ""#));
    assert!(footer.contains("Page "));
    assert!(footer.contains(" of "));
    let fields = run_ok(&["--json", "docx", "fields", "list", path(&output)]);
    assert_eq!(fields["fields"].as_array().unwrap().len(), 2);
    for package in [&source, &output, &output_again] {
        assert_all_proofs(package);
    }
    fs::remove_dir_all(temp).unwrap();
}

#[test]
fn quarterly_report_recipe_renders_all_structure_features() {
    let temp = temp_dir("quarterly-report");
    let mut produced = Vec::new();
    let scaffold = temp.join("00-scaffold.docx");
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&scaffold),
        "--text",
        "Quarterly Report",
    ]);
    produced.push(scaffold.clone());
    let numbered = temp.join("01-numbered.docx");
    run_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path(&scaffold),
        "--text",
        "Executive summary",
        "--list",
        "number",
        "--restart",
        "--out",
        path(&numbered),
    ]);
    produced.push(numbered.clone());
    let bullet = temp.join("02-bullet.docx");
    run_ok(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        path(&numbered),
        "--text",
        "Delivery remained on plan",
        "--list",
        "bullet",
        "--out",
        path(&bullet),
    ]);
    produced.push(bullet.clone());

    let mut rows = vec![serde_json::json!(["Metric", "Value", "Status"])];
    for index in 1..=90 {
        rows.push(serde_json::json!([
            format!("Quarterly metric {index}"),
            index * 100,
            "On plan"
        ]));
    }
    let values = serde_json::to_string(&rows).unwrap();
    let table = temp.join("03-table.docx");
    let report = run_owned_ok(vec![
        "--json".into(),
        "docx".into(),
        "tables".into(),
        "create".into(),
        path(&bullet).into(),
        "--values".into(),
        values,
        "--style".into(),
        "TableLight".into(),
        "--header-row".into(),
        "--widths".into(),
        "3in,1in,1.5in".into(),
        "--caption".into(),
        "Quarterly metrics".into(),
        "--out".into(),
        path(&table).into(),
    ]);
    produced.push(table.clone());
    let image_after = report["blockHashes"].as_array().unwrap().len();
    let image_hash = report["blockHashes"][image_after - 1]["contentHash"]
        .as_str()
        .unwrap()
        .to_string();
    let image_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/test_image.png");
    let image = temp.join("04-image.docx");
    run_owned_ok(vec![
        "--json".into(),
        "docx".into(),
        "images".into(),
        "insert".into(),
        path(&table).into(),
        "--after".into(),
        image_after.to_string(),
        "--expect-hash".into(),
        image_hash,
        "--file".into(),
        path(&image_file).into(),
        "--width".into(),
        "2in".into(),
        "--height".into(),
        "1in".into(),
        "--caption".into(),
        "Quarterly trend".into(),
        "--align".into(),
        "center".into(),
        "--out".into(),
        path(&image).into(),
    ]);
    produced.push(image.clone());
    let toc = temp.join("05-toc.docx");
    run_ok(&[
        "--json",
        "docx",
        "fields",
        "insert",
        path(&image),
        "--toc",
        "--levels",
        "1-3",
        "--out",
        path(&toc),
    ]);
    produced.push(toc.clone());
    let final_docx = temp.join("quarterly-report.docx");
    run_ok(&[
        "--json",
        "docx",
        "footers",
        "set-text",
        path(&toc),
        "--page-numbers",
        "--out",
        path(&final_docx),
    ]);
    produced.push(final_docx.clone());
    for package in &produced {
        assert_all_proofs(package);
    }
    assert_quarterly_report_render(&final_docx, &temp);
    fs::remove_dir_all(temp).unwrap();
}

fn build_list_document(temp: &Path, label: &str) -> PathBuf {
    let mut current = temp.join(format!("{label}-0.docx"));
    run_ok(&[
        "--json",
        "docx",
        "scaffold",
        path(&current),
        "--text",
        "List report",
    ]);
    assert_all_proofs(&current);
    for (offset, text, kind, level, restart, insert) in [
        (1, "Bullet zero", "bullet", "0", false, false),
        (2, "Bullet one", "bullet", "1", false, false),
        (3, "Bullet two", "bullet", "2", false, true),
        (4, "Restart one", "number", "0", true, false),
    ] {
        let output = temp.join(format!("{label}-{offset}.docx"));
        let mut args = vec![
            "--json",
            "docx",
            "paragraphs",
            if insert { "insert" } else { "append" },
            path(&current),
            "--text",
            text,
            "--list",
            kind,
            "--level",
            level,
        ];
        if insert {
            args.extend(["--after", "3"]);
        }
        if restart {
            args.push("--restart");
        }
        args.extend(["--out", path(&output)]);
        let report = run_ok(&args);
        assert_eq!(report["list"], kind);
        assert_eq!(report["listLevel"], level.parse::<u32>().unwrap());
        assert!(
            report["documentHash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert!(report["blockHashes"].is_array());
        assert_all_proofs(&output);
        current = output;
    }
    current
}

fn run(args: &[&str]) -> (Output, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .unwrap();
    let stream = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    let report = serde_json::from_slice(stream)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(stream)));
    (output, report)
}

fn run_ok(args: &[&str]) -> Value {
    let (output, report) = run(args);
    assert!(output.status.success(), "{args:?}: {report}");
    report
}

fn run_owned_ok(args: Vec<String>) -> Value {
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_ok(&refs)
}

fn assert_quarterly_report_render(package: &Path, temp: &Path) {
    if !Path::new("/usr/bin/soffice").is_file() {
        println!("SKIP LibreOffice quarterly report render: /usr/bin/soffice unavailable");
        return;
    }
    let output_dir = temp.join("rendered");
    fs::create_dir_all(&output_dir).unwrap();
    let profile = temp.join("lo-profile");
    let profile_url = format!("-env:UserInstallation=file://{}", path(&profile));
    let output = Command::new("/usr/bin/soffice")
        .args([
            "--headless",
            &profile_url,
            "--convert-to",
            "pdf",
            "--outdir",
            path(&output_dir),
            path(package),
        ])
        .output()
        .expect("run LibreOffice");
    assert!(
        output.status.success(),
        "LibreOffice: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pdf = output_dir.join("quarterly-report.pdf");
    assert!(
        pdf.is_file(),
        "LibreOffice did not produce {}",
        pdf.display()
    );
    let info = Command::new("pdfinfo")
        .arg(&pdf)
        .output()
        .expect("run pdfinfo");
    assert!(info.status.success());
    let info = String::from_utf8_lossy(&info.stdout);
    let pages = info
        .lines()
        .find_map(|line| line.strip_prefix("Pages:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    assert!(pages >= 2, "quarterly report should span pages: {info}");
    let text_file = temp.join("quarterly-report.txt");
    let extracted = Command::new("pdftotext")
        .args([pdf.as_os_str(), text_file.as_os_str()])
        .output()
        .expect("run pdftotext");
    assert!(extracted.status.success());
    let text = fs::read_to_string(text_file).unwrap();
    assert!(
        text.matches("Metric").count() >= 2,
        "repeating table header missing: {text}"
    );
    for expected in [
        "Executive summary",
        "Delivery remained on plan",
        "Quarterly trend",
        "Page",
    ] {
        assert!(
            text.contains(expected),
            "rendered report missing {expected:?}"
        );
    }
}

fn assert_all_proofs(package: &Path) {
    let (output, report) = run(&["--json", "validate", "--strict", path(package)]);
    assert!(
        output.status.success(),
        "strict rejected {}: {report}",
        package.display()
    );
    let (output, report) = run(&["--json", "conformance", "check", path(package)]);
    assert!(
        output.status.success(),
        "child-order/style integrity rejected {}: {report}",
        package.display()
    );
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };
    let dotnet = home.join("dotnet/dotnet");
    let validator = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.is_file() || !validator.is_file() {
        println!(
            "SKIP Open XML SDK validation for {}: validator unavailable",
            package.display()
        );
        return;
    }
    let output = Command::new(dotnet)
        .args([
            validator.as_os_str(),
            "--json".as_ref(),
            package.as_os_str(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "SDK rejected {}: {}",
        package.display(),
        String::from_utf8_lossy(&output.stdout)
    );
}

fn zip_text(package: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(package).unwrap()).unwrap();
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap()
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-docx-structure-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn path(path: &Path) -> &str {
    path.to_str().unwrap()
}
